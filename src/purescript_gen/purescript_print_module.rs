use super::{
    purescript_import::PurescriptImport, purescript_instance::DeriveInstance,
    purescript_record::PurescriptRecord, purescript_type::PurescriptType,
    purescript_variant::Variant,
};

pub fn print_module(
    role: &str,
    types: &mut Vec<PurescriptType>,
    records: &mut Vec<PurescriptRecord>,
    imports: &mut Vec<PurescriptImport>,
    variants: &mut Vec<Variant>,
    instances: &mut Vec<DeriveInstance>,
) -> String {
    let mut module = format!("module Schema.{role} where");
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
        .map(|i| {
            if i.module == "GraphQL.Hasura.ComparisonExp" || i.module == "Data.ComparisonExpString"
            {
                i.specified = vec![];
                i.add_specified_mut(if i.module == "GraphQL.Hasura.ComparisonExp" {
                    "ComparisonExp"
                } else {
                    "ComparisonExpString"
                });
            }
            i.to_string()
        })
        .collect::<Vec<String>>()
        .join("\n")
        .to_string();
    let variants: String = variants
        .iter_mut()
        .map(|v| v.to_string())
        .collect::<Vec<String>>()
        .join("\n\n");
    let instances = instances
        .iter_mut()
        .map(|i| i.to_string())
        .collect::<Vec<String>>()
        .join("\n")
        .to_string();

    module.push_str("\n\n");
    module.push_str(&imports);
    module = module.trim().to_string();
    module.push_str("\n\n");
    module.push_str(&records);
    module = module.trim().to_string();
    module.push_str("\n\n");
    module.push_str(&types);
    module = module.trim().to_string();
    module.push_str("\n\n");
    module.push_str(&variants);
    module = module.trim().to_string();
    module.push_str("\n\n");
    module.push_str(&instances);
    module.trim().to_string()
}
