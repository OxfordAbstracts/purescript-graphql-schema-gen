use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::{
    purescript_import::PurescriptImport, purescript_instance::DeriveInstance,
    purescript_record::PurescriptRecord, purescript_type::PurescriptType,
    purescript_variant::Variant,
};

/// Splits a role schema into many small modules instead of one giant one.
///
/// PureScript forbids cyclic imports, but only types in the same
/// strongly-connected component of the reference graph actually need to share
/// a module. Everything else is grouped into chunks by topological level
/// (a declaration only ever references declarations on strictly lower levels,
/// so modules within a level never import each other) and the top module
/// re-exports the lot, keeping the public API identical to the unsplit output.
///
/// Returns (module_name, module_contents) pairs, top module included.
pub fn print_split_modules(
    role: &str,
    types: &Vec<PurescriptType>,
    schema_records: &Vec<PurescriptRecord>,
    imports: &Vec<PurescriptImport>,
    variants: &Vec<Variant>,
    instances: &Vec<DeriveInstance>,
    max_chunk_lines: usize,
) -> Vec<(String, String)> {
    // Instances (only newtype derives today) must live in the same module as
    // the type they are derived for.
    let mut instances_by_type: HashMap<&str, Vec<String>> = HashMap::new();
    for instance in instances {
        instances_by_type
            .entry(instance.type_name())
            .or_default()
            .push(instance.to_string());
    }

    // Collect every declaration with its printed body and referenced tokens
    let mut decls: Vec<Decl> = vec![];
    let mut seen: HashSet<String> = HashSet::new();
    let mut add_decl = |name: &str, mut body: String| {
        if !seen.insert(name.to_string()) {
            return;
        }
        if let Some(instance_strs) = instances_by_type.get(name) {
            for i in instance_strs {
                body.push_str("\n");
                body.push_str(i);
            }
        }
        decls.push(Decl::new(name, body));
    };
    for t in types {
        add_decl(&t.name, t.to_string());
    }
    for v in variants {
        add_decl(v.name(), v.to_string());
    }
    decls.sort_by(|a, b| a.name.cmp(&b.name));

    let mut index_of = build_index(&decls);
    let mut graph = build_graph(&decls, &index_of);

    // Prune declarations unreachable from the Schema record. This happens
    // when exclude_type_patterns removed every field that referenced a helper
    // type, or when the introspected schema carries genuinely orphaned types.
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
        .filter_map(|t| index_of.get(t))
        .copied()
        .collect();
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
    if pruned > 0 {
        println!("Pruned {pruned} types unreachable from the {role} schema");
        decls = decls
            .into_iter()
            .zip(reachable)
            .filter_map(|(d, r)| if r { Some(d) } else { None })
            .collect();
        index_of = build_index(&decls);
        graph = build_graph(&decls, &index_of);
    }

    let components = strongly_connected_components(&graph);
    let levels = component_levels(&graph, &components);

    // Group the components by level, then greedily pack each level into
    // chunks of roughly max_chunk_lines. Modules on the same level never
    // reference one another, so any grouping within a level is import-safe.
    let max_level = levels.iter().copied().max().unwrap_or(0);
    let mut chunks: Vec<Vec<usize>> = vec![]; // decl indices per chunk
    for level in 0..=max_level {
        let mut level_components: Vec<Vec<usize>> = components
            .iter()
            .zip(levels.iter())
            .filter(|(_, l)| **l == level)
            .map(|(c, _)| {
                let mut c = c.clone();
                c.sort_by(|a, b| decls[*a].name.cmp(&decls[*b].name));
                c
            })
            .collect();
        level_components.sort_by(|a, b| decls[a[0]].name.cmp(&decls[b[0]].name));

        let mut chunk: Vec<usize> = vec![];
        let mut chunk_lines = 0;
        for component in level_components {
            let component_lines: usize = component.iter().map(|i| decls[*i].lines).sum();
            if !chunk.is_empty() && chunk_lines + component_lines > max_chunk_lines {
                chunks.push(std::mem::take(&mut chunk));
                chunk_lines = 0;
            }
            chunk.extend(component);
            chunk_lines += component_lines;
        }
        if !chunk.is_empty() {
            chunks.push(chunk);
        }
    }

    // Name each chunk module after its alphabetically first declaration.
    // Chunks partition the declarations, so the names are unique and remain
    // stable under small schema changes.
    let chunk_names: Vec<String> = chunks
        .iter()
        .map(|c| {
            let first = c
                .iter()
                .map(|i| &decls[*i].name)
                .min()
                .expect("Chunks are never empty");
            format!("Schema.{role}.{first}")
        })
        .collect();
    let mut chunk_of: HashMap<usize, usize> = HashMap::new();
    for (chunk_id, chunk) in chunks.iter().enumerate() {
        for i in chunk {
            chunk_of.insert(*i, chunk_id);
        }
    }

    let merged_imports = PurescriptImport::merge(imports);
    let mut modules: Vec<(String, String)> = vec![];

    for (chunk_id, chunk) in chunks.iter().enumerate() {
        let mut decl_indices = chunk.clone();
        decl_indices.sort_by(|a, b| decls[*a].name.cmp(&decls[*b].name));

        let mut tokens: HashSet<String> = HashSet::new();
        for i in &decl_indices {
            tokens.extend(decls[*i].tokens.iter().cloned());
        }

        // Imports of other generated chunk modules
        let mut internal: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for i in &decl_indices {
            for r in &graph[*i] {
                let target_chunk = chunk_of[r];
                if target_chunk != chunk_id {
                    internal
                        .entry(&chunk_names[target_chunk])
                        .or_default()
                        .insert(&decls[*r].name);
                }
            }
        }

        let mut import_lines = filter_external_imports(&merged_imports, &tokens);
        for (module, names) in internal {
            let names = names.into_iter().collect::<Vec<&str>>().join(", ");
            import_lines.push(format!("import {module} ({names})"));
        }
        import_lines.sort();

        let bodies = decl_indices
            .iter()
            .map(|i| decls[*i].body.clone())
            .collect::<Vec<String>>()
            .join("\n\n");

        modules.push((
            chunk_names[chunk_id].clone(),
            format!(
                "-- @generated\nmodule {} where\n\n{}\n\n{}\n",
                chunk_names[chunk_id],
                import_lines.join("\n"),
                bodies
            ),
        ));
    }

    modules.push(top_module(
        role,
        schema_records,
        &merged_imports,
        &chunk_names,
        &index_of,
    ));

    modules
}

/// The top module keeps the original `Schema.{role}` name and public API:
/// it re-exports every chunk module and declares the Schema record itself.
fn top_module(
    role: &str,
    schema_records: &Vec<PurescriptRecord>,
    merged_imports: &Vec<PurescriptImport>,
    chunk_names: &Vec<String>,
    index_of: &HashMap<String, usize>,
) -> (String, String) {
    let record_bodies = schema_records
        .iter()
        .map(|r| r.to_string())
        .collect::<Vec<String>>()
        .join("\n\n");
    let tokens = tokenize(&record_bodies);

    // Generated types the Schema record refers to are used through the Export
    // qualifier: importing a chunk both explicitly and `as Export` would raise
    // DuplicateSelectiveImport warnings.
    let record_bodies = qualify_tokens(&record_bodies, "Export", index_of);

    let mut import_lines = filter_external_imports(merged_imports, &tokens);
    for chunk_name in chunk_names {
        import_lines.push(format!("import {chunk_name} as Export"));
    }
    import_lines.sort();

    let exports = schema_records
        .iter()
        .map(|r| format!("\n  , {}", r.name))
        .collect::<String>();

    let module_name = format!("Schema.{role}");
    let contents = format!(
        "-- @generated\nmodule {module_name}\n  ( module Export{exports}\n  ) where\n\n{}\n\n{record_bodies}\n",
        import_lines.join("\n"),
    );
    (module_name, contents)
}

/// Keep only the imports whose specified names actually occur in the module
/// body, dropping import lines that end up empty. When two modules provide
/// the same name (e.g. Time from both Data.Time and Data.DateTime via outside
/// types) it is kept on one line only, avoiding redundant-import warnings.
fn filter_external_imports(
    merged: &Vec<PurescriptImport>,
    tokens: &HashSet<String>,
) -> Vec<String> {
    let mut kept: Vec<PurescriptImport> = vec![];
    for import in merged {
        let mut import = import.clone();
        // Special case kept identical to print_module: these two modules are
        // always imported by their head type only.
        if import.module == "GraphQL.Hasura.ComparisonExp" {
            import.specified = vec![];
            import.add_specified_mut("ComparisonExp");
        } else if import.module == "Data.ComparisonExpString" {
            import.specified = vec![];
            import.add_specified_mut("ComparisonExpString");
        }
        import.specified.retain(|s| {
            let bare = s
                .import
                .trim_start_matches("class ")
                .trim_start_matches("type ");
            tokens.contains(bare)
        });
        kept.push(import);
    }
    kept.sort_by(|a, b| a.module.cmp(&b.module));
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut lines: Vec<String> = vec![];
    for mut import in kept {
        import
            .specified
            .retain(|s| seen_names.insert(s.import.clone()));
        if import.specified.is_empty() {
            continue;
        }
        lines.push(import.to_string());
    }
    lines.sort();
    lines.dedup();
    lines
}

struct Decl {
    name: String,
    body: String,
    lines: usize,
    tokens: HashSet<String>,
}

impl Decl {
    fn new(name: &str, body: String) -> Self {
        let mut tokens = tokenize(&body);
        tokens.remove(name);
        Decl {
            name: name.to_string(),
            lines: body.lines().count(),
            tokens,
            body,
        }
    }
}

/// Prefix every token that names a generated declaration with a module
/// qualifier, leaving string literals and other tokens untouched.
fn qualify_tokens(s: &str, qualifier: &str, names: &HashMap<String, usize>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut current = String::new();
    let mut in_string = false;
    let flush = |current: &mut String, out: &mut String| {
        if !current.is_empty() {
            if names.contains_key(current.as_str()) {
                out.push_str(qualifier);
                out.push('.');
            }
            out.push_str(current);
            current.clear();
        }
    };
    for c in s.chars() {
        if c == '"' {
            flush(&mut current, &mut out);
            in_string = !in_string;
            out.push(c);
        } else if in_string {
            out.push(c);
        } else if c.is_ascii_alphanumeric() || c == '_' || c == '\'' {
            current.push(c);
        } else {
            flush(&mut current, &mut out);
            out.push(c);
        }
    }
    flush(&mut current, &mut out);
    out
}

fn build_index(decls: &Vec<Decl>) -> HashMap<String, usize> {
    decls
        .iter()
        .enumerate()
        .map(|(i, d)| (d.name.clone(), i))
        .collect()
}

/// Reference graph between the declarations of this schema
fn build_graph(decls: &Vec<Decl>, index_of: &HashMap<String, usize>) -> Vec<Vec<usize>> {
    decls
        .iter()
        .map(|d| {
            let mut refs: Vec<usize> = d
                .tokens
                .iter()
                .filter_map(|t| index_of.get(t))
                .copied()
                .collect();
            refs.sort();
            refs.dedup();
            refs
        })
        .collect()
}

/// All identifier-shaped tokens in a printed declaration, ignoring string
/// literals (which hold GraphQL names, not PureScript references). Type
/// references always appear as standalone tokens outside strings, so this
/// can never miss an edge or an import.
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

/// Kosaraju's algorithm (iterative). Returns components in topological order:
/// if any member of component A references a member of component B, then A
/// comes before B in the result.
fn strongly_connected_components(graph: &Vec<Vec<usize>>) -> Vec<Vec<usize>> {
    let n = graph.len();

    // First pass: record finish order via iterative DFS
    let mut visited = vec![false; n];
    let mut finish_order: Vec<usize> = Vec::with_capacity(n);
    for start in 0..n {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        while let Some((node, edge)) = stack.last().copied() {
            if edge < graph[node].len() {
                stack.last_mut().expect("stack is non-empty").1 += 1;
                let next = graph[node][edge];
                if !visited[next] {
                    visited[next] = true;
                    stack.push((next, 0));
                }
            } else {
                finish_order.push(node);
                stack.pop();
            }
        }
    }

    // Second pass: DFS over the transposed graph in reverse finish order
    let mut transposed: Vec<Vec<usize>> = vec![vec![]; n];
    for (node, refs) in graph.iter().enumerate() {
        for r in refs {
            transposed[*r].push(node);
        }
    }
    let mut component_of = vec![usize::MAX; n];
    let mut components: Vec<Vec<usize>> = vec![];
    for &start in finish_order.iter().rev() {
        if component_of[start] != usize::MAX {
            continue;
        }
        let id = components.len();
        component_of[start] = id;
        let mut members = vec![start];
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            for &next in &transposed[node] {
                if component_of[next] == usize::MAX {
                    component_of[next] = id;
                    members.push(next);
                    stack.push(next);
                }
            }
        }
        components.push(members);
    }
    components
}

/// Longest-path level of each component in the condensation DAG. Level 0
/// components reference nothing outside themselves; every reference points to
/// a strictly lower level.
fn component_levels(graph: &Vec<Vec<usize>>, components: &Vec<Vec<usize>>) -> Vec<usize> {
    let mut component_of = vec![usize::MAX; graph.len()];
    for (id, members) in components.iter().enumerate() {
        for m in members {
            component_of[*m] = id;
        }
    }
    // Components arrive in topological order (references point to higher
    // ids), so one reverse sweep resolves all levels.
    let mut levels = vec![0usize; components.len()];
    for id in (0..components.len()).rev() {
        let mut level = 0;
        for member in &components[id] {
            for r in &graph[*member] {
                let target = component_of[*r];
                if target != id {
                    level = level.max(levels[target] + 1);
                }
            }
        }
        levels[id] = level;
    }
    levels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sccs_and_levels() {
        // 0 <-> 1 form a cycle, 2 depends on the cycle, 3 is independent
        let graph = vec![vec![1], vec![0], vec![0], vec![]];
        let components = strongly_connected_components(&graph);
        let cycle = components
            .iter()
            .find(|c| c.len() == 2)
            .expect("cycle component");
        let mut cycle = cycle.clone();
        cycle.sort();
        assert_eq!(cycle, vec![0, 1]);

        let levels = component_levels(&graph, &components);
        let level_of = |node: usize| {
            components
                .iter()
                .zip(levels.iter())
                .find(|(c, _)| c.contains(&node))
                .map(|(_, l)| *l)
                .expect("node in some component")
        };
        assert_eq!(level_of(0), 0);
        assert_eq!(level_of(1), 0);
        assert_eq!(level_of(2), 1);
        assert_eq!(level_of(3), 0);
    }

    #[test]
    fn qualifies_only_known_declaration_tokens() {
        let names = HashMap::from([("Query".to_string(), 0), ("Mutation".to_string(), 1)]);
        let out = qualify_tokens(
            "type Schema =\n  { query :: Query\n  , directives :: Proxy Directives\n  , mutation :: Mutation\n  }",
            "Export",
            &names,
        );
        assert!(out.contains("query :: Export.Query"));
        assert!(out.contains("mutation :: Export.Mutation"));
        assert!(out.contains("Proxy Directives"), "external names untouched");
    }

    #[test]
    fn tokenizer_splits_identifiers_and_skips_strings() {
        let tokens = tokenize("newtype A = A\n  { b :: AsGql \"x_exp\" (Maybe B) }");
        assert!(tokens.contains("A"));
        assert!(tokens.contains("B"));
        assert!(tokens.contains("Maybe"));
        assert!(!tokens.contains("x_exp"), "string literals are not references");
        assert!(!tokens.contains("A = A"));
    }
}
