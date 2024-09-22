use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::Result,
};

use cynic::{http::ReqwestExt, QueryBuilder};
use cynic_introspection::{FieldWrapping, IntrospectionQuery, Type, WrappingType};
use stringcase::pascal_case;

use crate::{
    config::parse_outside_types::OutsideTypes,
    enums::generate_enum::generate_enum,
    hasura_types::as_gql_field,
    purescript_gen::{
        purescript_argument::Argument,
        purescript_import::PurescriptImport,
        purescript_instance::{derive_new_type_instance, DeriveInstance},
        purescript_print_module::print_module,
        purescript_record::{Field, PurescriptRecord},
        purescript_type::PurescriptType,
    },
};

pub async fn build_schema(
    role: String,
    postgres_types: Arc<Mutex<HashMap<String, (String, String)>>>,
    outside_types: Arc<Mutex<OutsideTypes>>,
) -> Result<String> {
    // Fetch the introspection schema
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

    // Create the purescript types;
    let mut records: Vec<PurescriptRecord> = vec![];
    let mut types: Vec<PurescriptType> = vec![];
    let mut imports: Vec<PurescriptImport> = vec![];
    let mut instances: Vec<DeriveInstance> = vec![];

    // Add the purescript GraphQL client imports that are always used
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

    // The schema must always at least have a query type, add it now.
    let query_type = PurescriptType::new(
        "Query",
        vec![],
        Argument::new_type(&pascal_case(schema.query_type.as_str())),
    );
    schema_record.add_field(Field::new("query").with_type(&query_type.name));
    types.push(query_type);

    // // TODO use generated directives instead
    // schema_record.add_field(Field::new("directives").with_type("Directives"));

    // Optionally add mutation
    if let Some(mut_type) = &schema.mutation_type {
        let mutation_type = PurescriptType::new(
            "Mutation",
            vec![],
            Argument::new_type(&pascal_case(&mut_type)),
        );
        schema_record.add_field(Field::new("mutation").with_type(&mutation_type.name));
        types.push(mutation_type);
    };

    // and subscription types
    if let Some(mut_type) = &schema.subscription_type {
        let mutation_type = PurescriptType::new(
            "Subscription",
            vec![],
            Argument::new_type(&pascal_case(&mut_type)),
        );
        schema_record.add_field(Field::new("subscription").with_type(&mutation_type.name));
        types.push(mutation_type);
    };

    // Process the schema types
    for type_ in schema.types.iter() {
        match type_ {
            Type::Object(obj) => {
                // There are a couple of `__` prefixed Hasura types that we can safely ignore
                if obj.name.starts_with("__") {
                    continue;
                }

                // Convert the hasura_type_name to a PurescriptTypeName
                let name = pascal_case(&obj.name);

                // Creates a new record for the object
                let mut record = PurescriptRecord::new("Ignored");

                // Add type fields to the record
                for field in obj.fields.iter() {
                    // If the field has arguments then the purescript representation will be:
                    // field_name :: { | Arguments } -> ReturnType

                    // Build the arguments record:
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

                    // Build the return type,
                    // potentially wrapping values in Array or Maybe
                    // and resolving any matched outside types
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

                    // Add the function argument to the new record field
                    // and add it to the object record
                    let function_arg =
                        Argument::new_function(vec![Argument::new_record(args)], return_type);
                    let record_field = Field::new(&field.name).with_type_arg(function_arg);
                    record.add_field(record_field);
                }

                // Create the newtype record for the object and append it to the schema module types
                let mut query_type =
                    PurescriptType::new(&name, vec![], Argument::new_record(record));
                query_type.set_newtype(true);
                instances.push(derive_new_type_instance(&query_type.name));
                types.push(query_type);
            }
            Type::Scalar(scalar) => {
                // Add imports for common scalar types if they are used.
                // TODO maybe move these to config so they can be updated outside of rust
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
                // Ignore internal Hasura enums beginning with `__`
                if en.name.starts_with("__") {
                    continue;
                }

                // Generate purescript enums for all graphql types
                // These include table select columns as well as custom enums
                generate_enum(&en, &role, &mut imports).await;
            }
            Type::InputObject(obj) => {
                // Ignore internal Hasura input objects beginning with `__`
                if obj.name.starts_with("__") {
                    continue;
                }

                // Convert the hasura_type_name to a PurescriptTypeName
                let name = pascal_case(&obj.name);

                // Build a purescript record with all fields
                let mut record = PurescriptRecord::new("Query");
                for field in obj.fields.iter() {
                    // Work out the type of the field, wrapping in NotNull or Array as required.
                    // This will also resolve any outside types.
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
                    // Add the new field type to the record
                    let record_field = Field::new(&field.name).with_type_arg(arg_type);
                    record.add_field(record_field);
                }

                // Create the newtyped record and append it to the schemas types
                let mut query_type =
                    PurescriptType::new(&name, vec![], Argument::new_record(record));
                query_type.set_newtype(true);
                instances.push(derive_new_type_instance(&query_type.name));
                types.push(query_type);
            }
            Type::Interface(int) => {
                // Currently ignored as we don't have any in our schemas
                println!("Interface: {}", int.name);
            }
            Type::Union(uni) => {
                // Currently ignored as we don't have any in our schemas
                println!("Union: {}", uni.name);
            }
        }
    }

    // The top level schema record must have four fields. If any are missing at this point
    // then we should add them with a Void type to satisfy the compiler.
    for schema_field in ["query", "mutation", "subscription", "directives"] {
        if !schema_record.has_field(schema_field) {
            add_import("Data.Void", "Void", &mut imports);
            schema_record.add_field(Field::new(schema_field).with_type("Void"));
        }
    }
    records.push(schema_record);

    Ok(print_module(
        &role,
        &mut types,
        &mut records,
        &mut imports,
        &mut instances,
    ))
}

/// Simplified import add via plain strings
fn add_import(import: &str, specified: &str, imports: &mut Vec<PurescriptImport>) -> () {
    imports.push(PurescriptImport::new(import).add_specified(specified));
}

/// Optionally wraps the return type in Maybe/Array types,
/// depending on the FieldWrapping property of the field
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

/// Optionally wrap the argument in NonNull and/or Array types
/// depending on the FieldWrapping property of the field
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
