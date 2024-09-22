use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::Result,
};

use cynic::{http::ReqwestExt, QueryBuilder};
use cynic_introspection::{FieldWrapping, IntrospectionQuery, Type, WrappingType};
use dotenv::dotenv;
use generate_enum::{generate_enum, write};
use hasura_types::as_gql_field;
use parse_outside_types::{fetch_all_outside_types, OutsideTypes};
use parse_roles::parse_roles;
use postgres_types::fetch_types;
use purescript_argument::Argument;
use purescript_import::PurescriptImport;
use purescript_instance::{derive_new_type_instance, DeriveInstance};
use purescript_record::{Field, PurescriptRecord};
use purescript_type::PurescriptType;
use stringcase::pascal_case;
use tokio::spawn;
mod generate_enum;
mod hasura_types;
mod parse_outside_types;
mod parse_roles;
mod postgres_types;
mod purescript_argument;
mod purescript_enum;
mod purescript_import;
mod purescript_instance;
mod purescript_record;
mod purescript_type;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // time the main function
    let type_gen_timer = std::time::Instant::now();

    // Generate postgres enum types
    let postgres_types = fetch_types().await.unwrap();
    let num_types = postgres_types.len();
    println!(
        "Generated {} postgres enums in {:.2}s",
        num_types,
        type_gen_timer.elapsed().as_secs_f32()
    );

    let start = std::time::Instant::now();

    // Parse all outside type config
    let outside_types: OutsideTypes = fetch_all_outside_types();

    // Fetch role config
    let roles: Vec<String> = parse_roles();
    let num_roles = roles.len();

    // Postgres types are shared between all roles
    let types_ = Arc::new(Mutex::new(postgres_types));
    let outside_types = Arc::new(Mutex::new(outside_types));

    let mut tasks = Vec::with_capacity(num_roles);
    for role in roles.iter() {
        tasks.push(spawn(fetch(
            role.clone(),
            types_.clone(),
            outside_types.clone(),
        )));
    }

    let mut outputs = Vec::with_capacity(tasks.len());
    for task in tasks {
        outputs.push(task.await.unwrap().unwrap());
    }

    for (output, role) in outputs.into_iter().zip(roles.into_iter()) {
        write(
            &format!("./purs/src/Schema/{}/{}.purs", role, role),
            &output,
        );
    }

    println!(
        "Generated {} schemas in {:.2}s",
        num_roles,
        start.elapsed().as_secs_f32()
    );

    Ok(())
}

async fn fetch(
    role: String,
    postgres_types: Arc<Mutex<HashMap<String, (String, String)>>>,
    outside_types: Arc<Mutex<OutsideTypes>>,
) -> Result<String> {
    let graphql_url = std::env::var("GRAPHQL_URL").expect("GRAPHQL_URL must be set");
    let graphql_secret = std::env::var("GRAPHQL_SECRET").expect("GRAPHQL_SECRET must be set");
    let introspection_data = reqwest::Client::new()
        .post(graphql_url)
        .header("x-hasura-admin-secret", graphql_secret)
        .header("x-hasura-role", &role)
        .run_graphql(IntrospectionQuery::build(()))
        .await
        .unwrap()
        .data
        .unwrap();

    let schema = introspection_data.into_schema().unwrap();

    let mut records: Vec<PurescriptRecord> = vec![];
    let mut types: Vec<PurescriptType> = vec![];
    let mut imports: Vec<PurescriptImport> = vec![];
    let mut instances: Vec<DeriveInstance> = vec![];

    // Adds graphql arg imports
    add_import("GraphQL.Client.Args", "NotNull", &mut imports);
    add_import("GraphQL.Client.AsGql", "AsGql", &mut imports);
    add_import("Data.Newtype", "class Newtype", &mut imports);

    // // TODO generate directives
    // add_import(
    //     format!("GeneratedGql.Directives.{}", role).as_str(),
    //     "Directives",
    //     &mut imports,
    // );

    // Adds the root schema record
    let mut schema_record = PurescriptRecord::new("Schema");

    // Always add the query type
    let query_type = PurescriptType::new(
        "Query",
        vec![],
        Argument::new_type(&pascal_case(schema.query_type.as_str())),
    );
    schema_record.add_field(Field::new("query").with_type(&query_type.name));
    types.push(query_type);

    // // TODO use generated directives instead
    // schema_record.add_field(Field::new("directives").with_type("Directives"));

    // Optionally add subscription and mutation types
    if let Some(mut_type) = &schema.mutation_type {
        let mutation_type = PurescriptType::new(
            "Mutation",
            vec![],
            Argument::new_type(&pascal_case(&mut_type)),
        );
        schema_record.add_field(Field::new("mutation").with_type(&mutation_type.name));
        types.push(mutation_type);
    };
    if let Some(mut_type) = &schema.subscription_type {
        let mutation_type = PurescriptType::new(
            "Subscription",
            vec![],
            Argument::new_type(&pascal_case(&mut_type)),
        );
        schema_record.add_field(Field::new("subscription").with_type(&mutation_type.name));
        types.push(mutation_type);
    };

    // Add all types
    for type_ in schema.types.iter() {
        match type_ {
            Type::Object(obj) => {
                if obj.name.starts_with("__") {
                    continue;
                }

                let name = pascal_case(&obj.name);

                let mut query_type =
                    PurescriptType::new(&name, vec![], Argument::new_type("Placeholder"));
                let mut record = PurescriptRecord::new("Query");
                for field in obj.fields.iter() {
                    let mut record_field = Field::new(&field.name);
                    let mut args = PurescriptRecord::new("Arguments");
                    for arg in &field.args {
                        let arg_type = wrap_type(
                            as_gql_field(
                                &obj.name,
                                &arg.name,
                                &arg.ty.name,
                                &mut imports,
                                &postgres_types,
                                &outside_types,
                            ),
                            &arg.ty.wrapping,
                            &mut imports,
                        );
                        let mut arg_field = Field::new(&arg.name);
                        arg_field.type_name = arg_type;
                        args.add_field(arg_field);
                    }

                    let return_type = return_type_wrapper(
                        as_gql_field(
                            &obj.name,
                            &field.name,
                            &field.ty.name,
                            &mut imports,
                            &postgres_types,
                            &outside_types,
                        ),
                        &field.ty.wrapping,
                        &mut imports,
                    );

                    record_field.type_name =
                        Argument::new_function(vec![Argument::new_record(args)], return_type);
                    record.add_field(record_field);
                }
                query_type.set_newtype(true);
                query_type.set_value(Argument::new_record(record));
                instances.push(derive_new_type_instance(&query_type.name));
                types.push(query_type);
            }
            Type::Scalar(scalar) => {
                match scalar.name.as_str() {
                    _ if scalar.is_builtin() => {} // ignore built in types like String, Int, etc.
                    "date" => add_import("Data.Date", "Date", &mut imports),
                    "timestamp" | "timestamptz" => {
                        add_import("Data.DateTime", "DateTime", &mut imports);
                    }
                    "json" | "jsonb" => add_import("Data.Argonaut.Core", "Json", &mut imports),
                    "time" => add_import("Data.Time", "Time", &mut imports),
                    _ => {}
                }
            }
            Type::Enum(en) => {
                if en.name.starts_with("__") {
                    continue;
                }

                generate_enum(&en, &role, &mut imports).await;
            }
            Type::InputObject(obj) => {
                if obj.name.starts_with("__") {
                    continue;
                }

                let name = pascal_case(&obj.name);

                let mut query_type =
                    PurescriptType::new(&name, vec![], Argument::new_type("Placeholder"));
                let mut record = PurescriptRecord::new("Query");
                for field in obj.fields.iter() {
                    let mut record_field = Field::new(&field.name);
                    let arg_type = wrap_type(
                        as_gql_field(
                            &obj.name,
                            &field.name,
                            &field.ty.name,
                            &mut imports,
                            &postgres_types,
                            &outside_types,
                        ),
                        &field.ty.wrapping,
                        &mut imports,
                    );
                    record_field.type_name = arg_type;
                    record.add_field(record_field);
                }
                query_type.set_newtype(true);
                query_type.set_value(Argument::new_record(record));
                instances.push(derive_new_type_instance(&query_type.name));
                types.push(query_type);
            }
            Type::Interface(int) => {
                println!("Interface: {}", int.name);
            }
            Type::Union(uni) => {
                println!("Union: {}", uni.name);
            }
        }
    }

    for schema_field in ["query", "mutation", "subscription", "directives"] {
        if !schema_record.has_field(schema_field) {
            add_import("Data.Void", "Void", &mut imports);
            schema_record.add_field(Field::new(schema_field).with_type("Void"));
        }
    }
    records.push(schema_record);

    let module = format!("module {} where", &role);

    types.sort_by_key(|t| t.name.clone());
    types.dedup_by_key(|t| t.name.clone());

    let types = types
        .iter_mut()
        .map(|t| t.to_string())
        .collect::<Vec<String>>()
        .join("\n\n")
        .to_string();
    let records: String = records
        .iter_mut()
        .map(|r| r.to_string())
        .collect::<Vec<String>>()
        .join("\n\n")
        .to_string();
    let imports = PurescriptImport::merge(&imports)
        .iter_mut()
        .map(|i| i.to_string())
        .collect::<Vec<String>>()
        .join("\n")
        .to_string();
    let instances = instances
        .iter_mut()
        .map(|i| i.to_string())
        .collect::<Vec<String>>()
        .join("\n")
        .to_string();

    Ok(format!(
        "{}\n\n{}\n\n{}\n\n{}\n\n{}",
        module, imports, records, types, instances
    )
    .trim()
    .to_string())
}

fn add_import(import: &str, specified: &str, imports: &mut Vec<PurescriptImport>) -> () {
    imports.push(PurescriptImport::new(import).add_specified(specified));
}

fn return_type_wrapper(
    mut return_type: Argument,
    wrapping: &FieldWrapping,
    mut imports: &mut Vec<PurescriptImport>,
) -> Argument {
    let mut nullable = true;
    let mut array = false;
    for wrapper in wrapping.into_iter() {
        match wrapper {
            WrappingType::NonNull => {
                nullable = false;
            }
            WrappingType::List => {
                array = true;
            }
        }
    }
    if nullable {
        add_import("Data.Maybe", "Maybe", &mut imports);
        return_type = Argument::new_type("Maybe").add_argument(return_type);
    } else if array {
        return_type = Argument::new_type("Array").add_argument(return_type);
    }

    return_type
}

fn wrap_type(
    mut argument: Argument,
    wrapping: &FieldWrapping,
    mut imports: &mut Vec<PurescriptImport>,
) -> Argument {
    for wrapper in wrapping.into_iter() {
        match wrapper {
            WrappingType::NonNull => {
                add_import("GraphQL.Client.Args", "NotNull", &mut imports);
                argument = Argument::new_type("NotNull").add_argument(argument);
            }
            WrappingType::List => {
                argument = Argument::new_type("Array").add_argument(argument);
            }
        }
    }
    argument
}
