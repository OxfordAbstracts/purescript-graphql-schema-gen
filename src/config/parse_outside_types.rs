use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
};

use stringcase::pascal_case;
use yaml_rust2::{yaml, Yaml};

use crate::write::write;

use super::workspace::WorkspaceConfig;

pub type OutsideTypes = HashMap<String, Object>;
type Object = HashMap<String, Mod>;

pub fn fetch_all_outside_types(workspace_config: &WorkspaceConfig) -> OutsideTypes {
    let outside_types_env =
        std::env::var("OUTSIDE_TYPES_YAML").expect("OUTSIDE_TYPES_YAML must be set");

    let outside_type_locs: Vec<&str> = outside_types_env.split(",").collect();

    let mut outside_types: OutsideTypes = HashMap::new();
    for loc in outside_type_locs.iter() {
        let types = fetch_outside_types(loc, workspace_config);
        outside_types.extend(types);
    }
    outside_types
}

pub fn fetch_outside_types(location: &str, workspace_config: &WorkspaceConfig) -> OutsideTypes {
    let mut f = File::open(location).unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();

    let docs = yaml::YamlLoader::load_from_str(&s).unwrap();
    if let Yaml::Hash(hash) = &docs[0] {
        let types = to_types(hash.get(&Yaml::String("types".to_string())));
        let templates: HashMap<String, Object> =
            to_templates(hash.get(&Yaml::String("templates".to_string())), &types);

        let outside_types = to_outside_types(
            hash.get(&Yaml::String("outside_types".to_string()))
                .unwrap(),
            &types,
            &templates,
        );

        write_types(&outside_types, workspace_config);

        return outside_types;
    } else {
        panic!("Your outside types yaml should be a hash of at least one key: 'outside_types'");
    }
}

fn to_types(yaml: Option<&Yaml>) -> impl Fn(&str, &str) -> Option<Mod> {
    let types: HashMap<String, String> = match yaml {
        Some(Yaml::Hash(types_hash)) => {
            let mut hash = HashMap::new();
            for key_value in types_hash.iter() {
                if let (Yaml::String(key), Yaml::String(value)) = key_value {
                    hash.insert(key.clone(), value.clone());
                }
            }
            hash
        }
        Some(_) => {
            panic!(
                "Your outside types .yaml should have a types key with a hash of tables to types"
            );
        }
        None => HashMap::new(),
    };

    move |name: &str, type_name: &str| -> Option<Mod> {
        match types.get(name) {
            Some(import) => Some(Mod {
                import: import
                    .split(", ")
                    .collect::<Vec<&str>>()
                    .get(1)
                    .unwrap()
                    .replace("$", type_name),
                name: type_name.to_string(),
                package: import
                    .split(", ")
                    .collect::<Vec<&str>>()
                    .get(2)
                    .unwrap()
                    .to_string(),
            }),
            None => None,
        }
    }
}

fn to_outside_types(
    yaml: &Yaml,
    types_fn: &impl Fn(&str, &str) -> Option<Mod>,
    templates: &HashMap<String, Object>,
) -> OutsideTypes {
    let mut outside_types: OutsideTypes = HashMap::new();

    if let Yaml::Hash(outside_types_hash) = yaml {
        for module_entries in outside_types_hash.iter() {
            if let (Yaml::String(module_name), Yaml::Hash(module_hash)) = module_entries {
                let mut table: Object = HashMap::new();
                // Add the template types if the table has a 'with' key
                if let Some(Yaml::String(template_str)) =
                    module_hash.get(&Yaml::String("with".to_string()))
                {
                    if let Some(template) = templates.get(template_str) {
                        table.extend(template.iter().map(|(k, v)| (k.clone(), v.clone())));
                    }
                }

                // Add the types from the module
                for field_type in module_hash.iter() {
                    if let (Yaml::String(field_name), Yaml::String(type_name)) = field_type {
                        if field_name == "with" {
                            continue;
                        }
                        let value = to_type_value(type_name, types_fn);
                        table.insert(field_name.clone(), value);
                    }
                }
                outside_types.insert(module_name.clone(), table);
            }
        }
    }

    outside_types
}

fn to_templates(
    yaml: Option<&Yaml>,
    types_fn: &impl Fn(&str, &str) -> Option<Mod>,
) -> HashMap<String, Object> {
    let mut templates: HashMap<String, Object> = HashMap::new();

    match yaml {
        Some(Yaml::Hash(templates_hash)) => {
            for key_value in templates_hash.iter() {
                if let (Yaml::String(key), Yaml::Hash(template_types)) = key_value {
                    let mut values = HashMap::new();
                    for name_value in template_types.iter() {
                        if let (Yaml::String(type_name), Yaml::String(type_value)) = name_value {
                            let value = to_type_value(type_value, types_fn);
                            values.insert(type_name.clone(), value);
                        } else {
                            panic!("Mismated yaml type name");
                        }
                    }
                    templates.insert(key.clone(), values);
                }
            }
        }
        Some(_) => {
            panic!("Your outside types .yaml should have a templates key with a hash of templates");
        }
        None => {}
    }
    templates
}

fn to_type_value(type_value: &String, types_fn: &impl Fn(&str, &str) -> Option<Mod>) -> Mod {
    if type_value.contains('=') {
        types_fn(
            type_value.split('=').next().unwrap(),
            type_value.split('=').last().unwrap(),
        )
        .unwrap_or_else(|| panic!("Type not found: {}", type_value))
    } else if type_value.contains(", ") {
        let mut values = type_value.split(", ");
        let name = values.next().unwrap();
        let import = values.next().unwrap();
        let package: &str = values.next().unwrap();
        Mod {
            import: import.to_string(),
            name: name.to_string(),
            package: package.to_string(),
        }
    } else {
        panic!("Only the 'with' key can contain string template types");
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct Mod {
    pub import: String,
    pub name: String,
    pub package: String,
}

fn write_types(outside_types: &OutsideTypes, workspace_config: &WorkspaceConfig) {
    let mut to_write = HashSet::new();
    for (_, table) in outside_types.iter() {
        for (_, module) in table.iter() {
            to_write.insert(module.clone());
        }
    }
    let mock_outside_types = std::env::var("MOCK_OUTSIDE_TYPES");
    if mock_outside_types.is_ok() {
        let lib_path = workspace_config.shared_graphql_enums_dir.clone();
        let postgres_enums_lib = pascal_case(&workspace_config.postgres_enums_lib);
        let gql_enums_lib = pascal_case(&workspace_config.shared_graphql_enums_lib);
        for module in to_write.iter() {
            let is_enum_mod = module.import.contains(&postgres_enums_lib)
                || module.import.contains(&gql_enums_lib);
            if is_enum_mod {
                // Don't bother generating mock for generated enum types
                continue;
            }
            write(
                &format!("{lib_path}/oa-ids/src/{}.purs", &module.import),
                &mocked_id_module(&module),
            );
            write(&format!("{lib_path}/oa-ids/spago.yaml"), &ids_spago_yaml());
        }
    }
}

fn ids_spago_yaml() -> String {
    r#"package:
  name: oa-ids
  dependencies:
    - prelude
"#
    .to_string()
}

fn mocked_id_module(module: &Mod) -> String {
    format!(
        "module {} ({}) where

newtype {} = {} String",
        module.import, module.name, module.name, module.name
    )
}
