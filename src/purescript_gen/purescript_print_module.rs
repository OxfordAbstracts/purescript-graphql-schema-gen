use super::{
    purescript_import::PurescriptImport, purescript_instance::DeriveInstance,
    purescript_record::PurescriptRecord, purescript_type::PurescriptType,
};

pub fn print_module(
    role: &str,
    types: &mut Vec<PurescriptType>,
    records: &mut Vec<PurescriptRecord>,
    imports: &mut Vec<PurescriptImport>,
    instances: &mut Vec<DeriveInstance>,
) -> String {
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

    format!(
        "{}\n\n{}\n\n{}\n\n{}\n\n{}",
        module, imports, records, types, instances
    )
    .trim()
    .to_string()
}
