use std::{fs::File, io::Read};

use yaml_rust2::{yaml, Yaml};

pub fn parse_roles() -> Vec<String> {
    let file_path: String = std::env::var("ROLES_YAML").expect("GRAPHQL_URL must be set");
    let mut f = File::open(file_path).unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();

    let docs = yaml::YamlLoader::load_from_str(&s).unwrap();
    if let Yaml::Array(docs) = &docs[0] {
        let mut roles: Vec<String> = Vec::new();
        for docs in docs {
            if let Yaml::String(role) = docs {
                roles.push(role.to_string());
            } else {
                panic!("Invalid roles array. The roles YAML array should just contain plain string values.")
            }
        }
        roles
    } else {
        panic!("Invalid roles YAML. The roles YAML just be an array of strings.")
    }
}
