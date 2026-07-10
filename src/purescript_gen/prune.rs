use std::collections::{HashMap, HashSet};

use super::{
    purescript_gql_union::GqlUnion, purescript_import::PurescriptImport,
    purescript_instance::DeriveInstance, purescript_record::PurescriptRecord,
    purescript_type::PurescriptType, purescript_variant::Variant,
    upper_first::upper_first,
};

/// Drop declarations unreachable from the Schema record.
///
/// Needed for correctness when skip rules are configured: a type whose only
/// referents were skipped fields would otherwise still be printed, and may
/// itself reference skipped (now missing) types.
pub fn prune_unreachable(
    role: &str,
    types: &mut Vec<PurescriptType>,
    variants: &mut Vec<Variant>,
    unions: &mut Vec<GqlUnion>,
    instances: &mut Vec<DeriveInstance>,
    schema_records: &Vec<PurescriptRecord>,
) {
    let mut decls: Vec<Decl> = vec![];
    let mut seen: HashSet<String> = HashSet::new();
    for t in types.iter() {
        if seen.insert(t.name.clone()) {
            decls.push(Decl::new(&t.name, &t.to_string()));
        }
    }
    for v in variants.iter() {
        if seen.insert(v.name().to_string()) {
            decls.push(Decl::new(v.name(), &v.to_string()));
        }
    }
    for u in unions.iter() {
        if seen.insert(u.name().to_string()) {
            decls.push(Decl::new(u.name(), &u.to_string()));
        }
    }

    let index_of: HashMap<String, usize> = decls
        .iter()
        .enumerate()
        .map(|(i, d)| (d.name.clone(), i))
        .collect();

    // Reference graph between the declarations of this schema
    let graph: Vec<Vec<usize>> = decls
        .iter()
        .map(|d| {
            d.tokens
                .iter()
                .filter_map(|t| index_of.get(t))
                .copied()
                .collect()
        })
        .collect();

    // With create_root_aliases: false the Schema record refers to the raw
    // GraphQL root names (e.g. query_root), not the upper_first'd declaration
    // names, so try both spellings when resolving roots.
    let record_tokens = tokenize(
        &schema_records
            .iter()
            .map(|r| r.to_string())
            .collect::<Vec<String>>()
            .join("\n"),
    );
    let mut reachable = vec![false; decls.len()];
    let mut queue: Vec<usize> = record_tokens
        .iter()
        .filter_map(|t| index_of.get(t).or_else(|| index_of.get(&upper_first(t))))
        .copied()
        .collect();
    if queue.is_empty() {
        // No roots resolved: pruning would silently delete the whole schema.
        println!("WARNING: no schema roots found for {role}; skipping unreachable-type pruning");
        return;
    }
    for i in &queue {
        reachable[*i] = true;
    }
    while let Some(i) = queue.pop() {
        for r in &graph[i] {
            if !reachable[*r] {
                reachable[*r] = true;
                queue.push(*r);
            }
        }
    }

    let pruned = reachable.iter().filter(|r| !**r).count();
    if pruned == 0 {
        return;
    }
    println!("Pruned {pruned} types unreachable from the {role} schema");
    let is_reachable = |name: &str| index_of.get(name).map(|i| reachable[*i]).unwrap_or(true);
    types.retain(|t| is_reachable(&t.name));
    variants.retain(|v| is_reachable(v.name()));
    unions.retain(|u| is_reachable(u.name()));
    instances.retain(|i| is_reachable(i.type_name()));
}

/// Drop imports (and spago dependencies) that no remaining declaration uses,
/// and names provided by more than one module. Skipping and pruning can leave
/// imports behind that would otherwise trip UnusedImport warnings.
pub fn drop_unused_imports(
    imports: &mut Vec<PurescriptImport>,
    types: &Vec<PurescriptType>,
    variants: &Vec<Variant>,
    unions: &Vec<GqlUnion>,
    instances: &Vec<DeriveInstance>,
    schema_records: &Vec<PurescriptRecord>,
) {
    let mut printed = String::new();
    for t in types {
        printed.push_str(&t.to_string());
        printed.push('\n');
    }
    for v in variants {
        printed.push_str(&v.to_string());
        printed.push('\n');
    }
    for u in unions {
        printed.push_str(&u.to_string());
        printed.push('\n');
    }
    for i in instances {
        printed.push_str(&i.to_string());
        printed.push('\n');
    }
    for r in schema_records {
        printed.push_str(&r.to_string());
        printed.push('\n');
    }
    let used = tokenize(&printed);

    let mut merged = PurescriptImport::merge(imports);
    merged.sort_by(|a, b| a.module.cmp(&b.module));
    let mut seen_names: HashSet<String> = HashSet::new();
    merged.retain_mut(|import| {
        // These two modules get their specified list rewritten to just the
        // head type by print_module; keep them iff that head type is used.
        if import.module == "GraphQL.Hasura.ComparisonExp" {
            return used.contains("ComparisonExp");
        }
        if import.module == "Data.ComparisonExpString" {
            return used.contains("ComparisonExpString");
        }
        import.specified.retain(|s| {
            let bare = s
                .import
                .trim_start_matches("class ")
                .trim_start_matches("type ");
            used.contains(bare) && seen_names.insert(bare.to_string())
        });
        !import.specified.is_empty()
    });
    *imports = merged;
}

struct Decl {
    name: String,
    tokens: HashSet<String>,
}

impl Decl {
    fn new(name: &str, body: &str) -> Self {
        let mut tokens = tokenize(body);
        tokens.remove(name);
        Decl {
            name: name.to_string(),
            tokens,
        }
    }
}

/// All identifier-shaped tokens in a printed declaration, ignoring string
/// literals (which hold GraphQL names, not PureScript references). Type
/// references always appear as standalone tokens outside strings, so this
/// can never miss a reference.
fn tokenize(s: &str) -> HashSet<String> {
    let mut tokens = HashSet::new();
    let mut current = String::new();
    let mut in_string = false;
    for c in s.chars() {
        if c == '"' {
            in_string = !in_string;
            if !current.is_empty() {
                tokens.insert(std::mem::take(&mut current));
            }
        } else if in_string {
            continue;
        } else if c.is_ascii_alphanumeric() || c == '_' || c == '\'' {
            current.push(c);
        } else if !current.is_empty() {
            tokens.insert(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.insert(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::purescript_gen::purescript_argument::Argument;
    use crate::purescript_gen::purescript_record::Field;

    #[test]
    fn prunes_orphans_and_keeps_the_reachable_graph() {
        // Schema -> Query -> A; B is orphaned
        let mut types = vec![
            PurescriptType::new("Query", vec![], Argument::new_type("A")),
            PurescriptType::new("A", vec![], Argument::new_type("Int")),
            PurescriptType::new("B", vec![], Argument::new_type("A")),
        ];
        let mut variants = vec![];
        let mut unions = vec![];
        let mut instances = vec![];
        let mut record = PurescriptRecord::new("Schema");
        record.add_field(Field::new("query").with_type("Query"));
        let records = vec![record];

        prune_unreachable(
            "Test",
            &mut types,
            &mut variants,
            &mut unions,
            &mut instances,
            &records,
        );

        let names: Vec<&str> = types.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["Query", "A"]);
    }

    #[test]
    fn tokenizer_skips_string_literals() {
        let tokens = tokenize("newtype A = A\n  { b :: AsGql \"x_exp\" (Maybe B) }");
        assert!(tokens.contains("A"));
        assert!(tokens.contains("B"));
        assert!(!tokens.contains("x_exp"));
    }
}
