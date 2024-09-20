use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use stringcase::pascal_case;

use crate::{
    outside_types::OUTSIDE_TYPES, purescript_argument::Argument,
    purescript_import::PurescriptImport,
};

pub fn as_gql_field(
    object: &str,
    field: &str,
    name: &str,
    imports: &mut Vec<PurescriptImport>,
    purs_types: &Arc<Mutex<HashMap<String, (String, String)>>>,
) -> Argument {
    let (import, type_) = outside_type(object, field, name, &purs_types);
    if let Some(i) = import {
        imports.push(PurescriptImport::new(&i).add_specified(&type_));
        return Argument::new_type("AsGql")
            .add_argument(Argument::new_type(&format!("\"{}\"", name)))
            .add_argument(Argument::new_type(&type_));
    }
    Argument::new_type("AsGql")
        .add_argument(Argument::new_type(&format!("\"{}\"", name)))
        .add_argument(Argument::new_type(&pascal_case(&type_)))
}

fn outside_type(
    object: &str,
    field: &str,
    name: &str,
    purs_types: &Arc<Mutex<HashMap<String, (String, String)>>>,
) -> (Option<String>, String) {
    if let Some((import, type_)) = get_outside_type(object, field) {
        (Some(import), type_)
    } else if let Some((import, type_)) = purs_types.lock().unwrap().get(name) {
        (Some(import.clone()), type_.clone())
    } else {
        (None, base_types(name).to_string())
    }
}

fn get_outside_type(object: &str, field: &str) -> Option<(String, String)> {
    OUTSIDE_TYPES
        .get(object)
        .map(|table| table.get(field))
        .flatten()
        .map(|(import, type_)| (import.to_string(), type_.to_string()))
}

pub fn base_types(type_name: &str) -> &str {
    match type_name {
        "date" => "Date",
        "json" | "jsonb" => "Json",
        "uuid" => "String",
        "time" => "Time",
        "timestamp" | "timestamptz" => "DateTime",
        "bigint" | "smallint" => "Int",
        "numeric" => "Number",
        "citext" => "String",
        "Float" => "Number",
        _ => type_name,
    }
}
