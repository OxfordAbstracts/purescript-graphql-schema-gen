use std::{
    collections::HashMap,
    fmt::format,
    sync::{Arc, Mutex},
    thread::Result,
};

use cynic::{http::ReqwestExt, QueryBuilder};
use cynic_introspection::{
    Directive, DirectiveLocation, FieldWrapping, InputValue, InterfaceType, IntrospectionQuery,
    Type, UnionType, WrappingType,
};
use stringcase::{kebab_case};
use tokio::task::spawn_blocking;

use crate::{
    config::{parse_outside_types::OutsideTypes, workspace::WorkspaceConfig},
    enums::generate_enum::generate_enum,
    hasura_types::as_gql_field,
    purescript_gen::{
        purescript_argument::Argument,
        purescript_gql_union::GqlUnion,
        purescript_import::PurescriptImport,
        purescript_instance::{derive_new_type_instance, DeriveInstance},
        purescript_print_module::print_module,
        purescript_record::{show_field_name, Field, PurescriptRecord},
        purescript_type::PurescriptType,
        purescript_variant::Variant,
        upper_first::*,
    },
    write::write,
};

pub async fn build_schema(
    role: String,
    postgres_types: Arc<Mutex<HashMap<String, (String, String, String)>>>,
    outside_types: Arc<Mutex<OutsideTypes>>,
    workspace_config: WorkspaceConfig,
) -> Result<()> {
    // Fetch the introspection schema
    let graphql_url = std::env::var("GRAPHQL_URL").expect("GRAPHQL_URL must be set");
    let hasura_url = std::env::var("HASURA_URL");
    let graphql_url = hasura_url.unwrap_or(graphql_url);
    let graphql_secret = std::env::var("GRAPHQL_SECRET").expect("GRAPHQL_SECRET must be set");
    let introspection_data = reqwest::Client::new()
        .post(graphql_url)
        .header("x-hasura-admin-secret", graphql_secret)
        .header("x-hasura-role", &role)
        .run_graphql(IntrospectionQuery::build(()))
        .await
        .expect("Failed to fetch GraphQL introspection schema.")
        .data
        .expect("Failed to parse GraphQL introspection schema.");

    let schema = introspection_data
        .into_schema()
        .expect("Failed to convert introspection data.");

    // Create the purescript types;
    let mut records: Vec<PurescriptRecord> = vec![];
    let mut types: Vec<PurescriptType> = vec![];
    let mut imports: Vec<PurescriptImport> = vec![];
    let mut variants: Vec<Variant> = vec![];
    let mut unions: Vec<GqlUnion> = vec![];
    let mut instances: Vec<DeriveInstance> = vec![];

    // Add the purescript GraphQL client imports that are always used,
    add_import(
        "graphql-client",
        "GraphQL.Client.Args",
        "NotNull",
        &mut imports,
    );
    add_import(
        "graphql-client",
        "GraphQL.Client.AsGql",
        "AsGql",
        &mut imports,
    );
    add_import("prelude", "Type.Proxy", "Proxy", &mut imports);
    add_import("newtype", "Data.Newtype", "class Newtype", &mut imports);

    // as well as the import for the role directives that we're about to create
    add_import(
        "prelude", // It's not from here but we always import this so can use as a default
        &format!("{role}.Directives"),
        "Directives",
        &mut imports,
    );

    // Add the directives module
    // In another thread to handle the sync file creation
    let directive_role = role.clone();

    // Adds the root schema record
    let mut schema_record = PurescriptRecord::new("Schema");

    // The schema must always at least have a query type, add it now.
    let query_type = PurescriptType::new(
        "Query",
        vec![],
        Argument::new_type(&upper_first(schema.query_type.as_str())),
    );
    schema_record.add_field(Field::new("query").with_type(&query_type.name));
    types.push(query_type);

    // Add the directives field (imported above)
    schema_record.add_field(Field::new("directives").with_type_arg(
        Argument::new_type("Proxy").with_argument(Argument::new_type("Directives")),
    ));

    // Optionally add mutation
    if let Some(mut_type) = &schema.mutation_type {
        let mutation_type = PurescriptType::new(
            "Mutation",
            vec![],
            Argument::new_type(&upper_first(&mut_type)),
        );
        schema_record.add_field(Field::new("mutation").with_type(&mutation_type.name));
        types.push(mutation_type);
    };

    // and subscription types
    if let Some(mut_type) = &schema.subscription_type {
        let mutation_type = PurescriptType::new(
            "Subscription",
            vec![],
            Argument::new_type(&upper_first(&mut_type)),
        );
        schema_record.add_field(Field::new("subscription").with_type(&mutation_type.name));
        types.push(mutation_type);
    };

    // Process the schema types
    for type_ in schema.types.iter() {
        let mut handle_obj = |obj: &cynic_introspection::ObjectType| {
            // There are a builting of `__` prefixed graphql types that we can safely ignore
            if obj.name.starts_with("__") {
                return;
            }

            // Convert the gql_type_name to a PurescriptTypeName
            let name = upper_first(&obj.name);

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
                            &field.name,
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
            let mut query_type = PurescriptType::new(&name, vec![], Argument::new_record(record));
            query_type.set_newtype(true);
            instances.push(derive_new_type_instance(&query_type.name));
            types.push(query_type);
        };
        match type_ {
            Type::Object(obj) => handle_obj(obj),
            Type::Scalar(scalar) => {
                // Add imports for common scalar types if they are used.
                // TODO maybe move these to config so they can be updated outside of rust
                match scalar.name.as_str() {
                    _ if scalar.is_builtin() => {} // ignore built in types like String, Int, etc.
                    "date" => add_import("datetime", "Data.Date", "Date", &mut imports),
                    "timestamp" | "timestamptz" => {
                        add_import("datetime", "Data.DateTime", "DateTime", &mut imports);
                    }
                    "json" | "jsonb" => {
                        add_import("argonaut-core", "Data.Argonaut.Core", "Json", &mut imports)
                    }
                    "time" => add_import("datetime", "Data.Time", "Time", &mut imports),
                    "ID" => add_import("graphql-client", "GraphQL.Client.ID", "ID", &mut imports),
                    _ => {}
                }
            }
            Type::Enum(en) => {
                // Ignore internal graphql enums beginning with `__`
                if en.name.starts_with("__") {
                    continue;
                }

                // Generate purescript enums for all graphql types
                // These include table select columns as well as custom enums
                let enum_to_add = generate_enum(&en, &mut imports, &workspace_config).await;
                if let Some(variant) = enum_to_add {
                    add_import("prelude", "Prelude", "Unit", &mut imports);
                    add_import("variant", "Data.Variant", "Variant", &mut imports);
                    variants.push(variant);
                }
            }
            Type::InputObject(obj) => {
                // Ignore internal graphql input objects beginning with `__`
                if obj.name.starts_with("__") {
                    continue;
                }

                // Convert the gql_type_name to a PurescriptTypeName
                let name = upper_first(&obj.name);

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
            Type::Interface(InterfaceType {
                name,
                fields,
                description,
                ..
            }) => {
                handle_obj(&cynic_introspection::ObjectType {
                    name: name.clone(),
                    fields: fields.clone(),
                    description: description.clone(),
                    interfaces: vec![],
                });
            }
            Type::Union(UnionType {
                name,
                description: _,
                possible_types,
                ..
            }) => {
                // Currently ignored as we don't have any in our schemas
                add_import(
                    "graphql-client",
                    "GraphQL.Client.Union",
                    "GqlUnion",
                    &mut imports,
                );

                let mut union = GqlUnion::new(&name);

                union.with_values(
                    &possible_types
                        .iter()
                        .map(|t| (t.clone(), upper_first(&t)))
                        .collect(),
                );

                unions.push(union);
            }
        }
    }

    // The top level schema record must have four fields. If any are missing at this point
    // then we should add them with a Void type to satisfy the compiler.
    for schema_field in ["query", "mutation", "subscription", "directives"] {
        if !schema_record.has_field(schema_field) {
            add_import("prelude", "Data.Void", "Void", &mut imports);
            schema_record.add_field(Field::new(schema_field).with_type("Void"));
        }
    }
    records.push(schema_record);

    let lib_path = format!(
        "{}{}{}",
        workspace_config.schema_libs_dir,
        workspace_config.schema_libs_prefix,
        kebab_case(&role)
    );

    // Write the schema module to the file system
    let schema_module_path = format!("{lib_path}/src/Schema/{role}.purs");

    let printed = print_module(
        &role,
        &mut types,
        &mut records,
        &mut imports,
        &mut variants,
        &mut unions,
        &mut instances,
    );

    write(&schema_module_path, &printed);

    // Write the directives module
    let path_clone = lib_path.clone();
    spawn_blocking(move || build_directives(path_clone, directive_role, schema.directives));

    write(
        &format!("{lib_path}/spago.yaml"),
        &to_spago_yaml(&workspace_config.schema_libs_prefix, &role, &imports),
    );

    write(&format!("{lib_path}/.gitignore"), GIT_IGNORE);

    Ok(())
}

fn to_spago_yaml(prefix: &str, role: &str, imports: &Vec<PurescriptImport>) -> String {
    let mut spago_yaml = "".to_string();
    let kebab_role = kebab_case(role);
    spago_yaml.push_str(&format!(
        r#"package:
  name: {prefix}{kebab_role}
  dependencies:"#,
    ));
    let mut all_packages: Vec<String> = imports.iter().map(|i| i.package.clone()).collect();
    all_packages.push("typelevel-lists".to_string()); // This is required for directives
    all_packages.sort();
    all_packages.dedup();

    for name in all_packages.iter() {
        spago_yaml.push_str(&format!("\n    - {name}"));
    }
    spago_yaml
}

/// Simplified import add via plain strings
fn add_import(
    package: &str,
    import: &str,
    specified: &str,
    imports: &mut Vec<PurescriptImport>,
) -> () {
    imports.push(PurescriptImport::new(import, package).add_specified(specified));
}

/// Optionally wraps the return type in Maybe/Array types,
/// depending on the FieldWrapping property of the field
fn return_type_wrapper(
    mut return_type: Argument,
    wrapping: &FieldWrapping,
    mut imports: &mut Vec<PurescriptImport>,
) -> Argument {
    let mut last_was_non_null = false;
    let wrapping: Vec<WrappingType> = wrapping.into_iter().collect();
    for wrapper in wrapping.iter().rev() {
        match wrapper {
            WrappingType::NonNull => {
                last_was_non_null = true;
            }
            WrappingType::List => {
                if !last_was_non_null {
                    add_import("maybe", "Data.Maybe", "Maybe", &mut imports);
                    return_type = Argument::new_type("Maybe").with_argument(return_type);
                }
                return_type = Argument::new_type("Array").with_argument(return_type);
                last_was_non_null = false;
            }
        }
    }
    if !last_was_non_null {
        add_import("maybe", "Data.Maybe", "Maybe", &mut imports);
        return_type = Argument::new_type("Maybe").with_argument(return_type);
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
    let wrapping: Vec<WrappingType> = wrapping.into_iter().collect();
    for wrapper in wrapping.iter().rev() {
        match wrapper {
            WrappingType::NonNull => {
                add_import(
                    "graphql-client",
                    "GraphQL.Client.Args",
                    "NotNull",
                    &mut imports,
                );
                argument = Argument::new_type("NotNull").with_argument(argument);
            }
            WrappingType::List => {
                argument = Argument::new_type("Array").with_argument(argument);
            }
        }
    }
    argument
}

fn wrap_type_str(mut str: String, wrapping: &FieldWrapping) -> String {
    let wrapping: Vec<WrappingType> = wrapping.into_iter().collect();
    for wrapper in wrapping.iter().rev() {
        match wrapper {
            WrappingType::NonNull => str = format!("NotNull {str}"),
            WrappingType::List => str = format!("Array {str}"),
        }
    }
    str
}

fn directive_type_str(directive: &Directive) -> Option<String> {
    let name = &directive.name;
    let description = directive.description.clone().unwrap_or("".to_string());
    let args: String = directive
        .args
        .iter()
        .map(input_value_type_str)
        .collect::<Vec<String>>()
        .join("\n    , ");
    let locations = directive_locations_str(&directive.locations);

    if locations.is_none() {
       return None
    }

    let locations = locations.unwrap();

    Some(format!("( Directive \"{name}\" \n    \"{description}\" \n    {{ {args} }}  \n    {locations} \n  )"))
}

fn directive_locations_str(locations: &Vec<DirectiveLocation>) -> Option<String> {
    let locations_strs = locations
        .iter()
        .filter_map(directive_location_str)
        .map(|s| format!("{s} :> "))
        .collect::<Vec<_>>();

    if locations_strs.len() == 0 {
        return None;
    }
    let location_str = locations_strs.join("");

    Some(format!("({location_str} Nil' )"))
}

fn directive_location_str(location: &DirectiveLocation) -> Option<&str> {
    match location {
        DirectiveLocation::Query => Some("QUERY"),
        DirectiveLocation::Mutation => Some("MUTATION"),
        DirectiveLocation::Subscription => Some("SUBSCRIPTION"),
        _ => None,
    }
}

fn input_value_type_str(value: &InputValue) -> String {
    let name = show_field_name(value.name.clone());
    let prop_value = wrap_type_str(value.ty.name.clone(), &value.ty.wrapping);
    format!("\"{name}\" :: {prop_value}")
}

fn directive_fn(directive: &Directive) -> String {
    let directive_name = &directive.name;
    format!(
        r#"
{directive_name} :: forall q args. args -> q -> ApplyDirective "{directive_name}" args q
{directive_name} = applyDir (Proxy :: _ "{directive_name}")
"#
    )
}

/// Format the schema directives into a separate module.
/// TODO stop directives from being hardcoded string mods with bad imports just for our simple use...
fn build_directives(lib_path: String, role: String, directives: Vec<Directive>) {
    let directive_str: String = directives
        .iter()
        .filter_map(|directive| directive_type_str(&directive))
        .map(|d| format!("{d}\n  :>\n"))
        .collect::<Vec<String>>()
        .join("");

    let directive_str = format!("type Directives =\n {directive_str}\n  Nil'");

    let directive_fns = directives
        .iter()
        .map(|directive| directive_fn(&directive))
        .collect::<Vec<_>>()
        .join("\n");

    let mut directive_mod = "".to_string();
    // Push the module header + types type + declaration to the directive module
    directive_mod.push_str(&format!(
        "module {role}.Directives where \n{DIRECTIVE_IMPORTS}\ntype Directives :: List' Type\n{directive_str}\n\n{directive_fns}"
    ));

   
    write(
        &format!("{lib_path}/src/{role}/Directives.purs"),
        &directive_mod,
    );
}

const DIRECTIVE_IMPORTS: &str = r#"
import Prelude
import GraphQL.Client.Args (NotNull)
import GraphQL.Client.Directive (ApplyDirective, applyDir)
import GraphQL.Client.Directive.Definition (Directive)
import GraphQL.Client.Directive.Location (MUTATION, QUERY, SUBSCRIPTION)
import GraphQL.Client.Operation (OpMutation(..), OpQuery(..), OpSubscription(..))
import Type.Data.List (type (:>), List', Nil')
import Type.Proxy (Proxy(..))

"#;

const GIT_IGNORE: &str = r#"
bower_components/
node_modules/
.pulp-cache/
output/
output-es/
generated-docs/
.psc-package/
.psc*
.purs*
.psa*
.spago
"#;
