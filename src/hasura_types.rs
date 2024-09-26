use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use stringcase::pascal_case;

use crate::{
    config::parse_outside_types::{Mod, OutsideTypes},
    purescript_gen::{purescript_argument::Argument, purescript_import::PurescriptImport},
};

pub fn as_gql_field(
    object: &str,
    field: &str,
    name: &str,
    imports: &mut Vec<PurescriptImport>,
    purs_types: &Arc<Mutex<HashMap<String, (String, String, String)>>>,
    outside_types: &Arc<Mutex<OutsideTypes>>,
) -> Argument {
    let (import, type_) = outside_type(object, field, name, &purs_types, &outside_types);
    if let Some((field_package, field_import)) = import {
        imports.push(PurescriptImport::new(&field_import, &field_package).add_specified(&type_));
        return Argument::new_type("AsGql")
            .with_argument(Argument::new_type(&format!("\"{}\"", name)))
            .with_argument(Argument::new_type(&type_));
    }
    Argument::new_type("AsGql")
        .with_argument(Argument::new_type(&format!("\"{}\"", name)))
        .with_argument(Argument::new_type(&pascal_case(&type_)))
}

fn outside_type(
    object: &str,
    field: &str,
    name: &str,
    purs_types: &Arc<Mutex<HashMap<String, (String, String, String)>>>,
    outside_types: &Arc<Mutex<OutsideTypes>>,
) -> (Option<(String, String)>, String) {
    if let Some((package, import, type_)) = get_outside_type(object, field, outside_types) {
        (Some((package, import)), type_)
    } else if let Some((package, import, type_)) = purs_types.lock().unwrap().get(name) {
        (Some((package.clone(), import.clone())), type_.clone())
    } else {
        (None, base_types(name).to_string())
    }
}

fn get_outside_type(
    object: &str,
    field: &str,
    outside_types: &Arc<Mutex<OutsideTypes>>,
) -> Option<(String, String, String)> {
    outside_types
        .lock()
        .unwrap()
        .get(object)
        .map(|table| table.get(field))
        .flatten()
        .map(
            |Mod {
                 package,
                 import,
                 name,
             }| (package.to_string(), import.to_string(), name.to_string()),
        )
}

pub fn base_types(type_name: &str) -> &str {
    match type_name {
        "date" => "Date",
        "json" | "jsonb" => "Json",
        "uuid" => "String",
        "time" => "Time",
        "timestamp" | "timestamptz" => "DateTime",
        "smallint" => "Int",
        "bigint" => "Number",
        "numeric" => "Number",
        "citext" => "String",
        "Float" => "Number",
        _ => type_name,
    }
}
