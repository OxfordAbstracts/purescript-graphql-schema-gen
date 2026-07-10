use std::{collections::HashMap, fs::File};

use super::parse_outside_types::Mod;

pub type ScalarTypes = HashMap<String, Mod>;

pub fn fetch_all_scalar_types() -> Option<ScalarTypes> {
    let scalar_types_env = std::env::var("SCALAR_TYPES_YAML").ok()?;

    let scalar_type_locs: Vec<&str> = scalar_types_env.split(",").collect();

    let mut scalar_types: ScalarTypes = HashMap::new();
    for loc in scalar_type_locs.iter() {
        let types = fetch_scalar_types(loc);
        scalar_types.extend(types);
    }
    Some(scalar_types)
}

fn fetch_scalar_types(location: &str) -> ScalarTypes {
    let f = File::open(location).expect(&format!("Scalar types yaml file not found at {location}"));

    serde_yaml::from_reader(f).unwrap()
}
