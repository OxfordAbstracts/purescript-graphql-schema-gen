
use regex::Regex;
use serde::Deserialize;
use std::fs::File;

pub fn parse_workspace() -> WorkspaceConfig {
    let file_path: String = std::env::var("SPAGO_WORKSPACE_CONFIG_YAML")
        .expect("SPAGO_WORKSPACE_CONFIG_YAML must be set");

    let f = File::open(file_path.clone())
        // .await
        .expect(format!("Failed to locate or open spago workspace config yaml at: {}", file_path).as_str());
    serde_yaml::from_reader(f).unwrap()
}

/// A regex matched against GraphQL type or field names. Prefix the pattern
/// with `!` to negate it; the last matching rule wins.
#[derive(Clone)]
pub struct SkipRule {
    pub pattern: Regex,
    pub negated: bool,
}

impl SkipRule {
    pub fn parse(s: &str) -> Result<Self, regex::Error> {
        match s.strip_prefix('!') {
            Some(pattern) => Ok(Self {
                pattern: Regex::new(pattern)?,
                negated: true,
            }),
            None => Ok(Self {
                pattern: Regex::new(s)?,
                negated: false,
            }),
        }
    }
}

fn deserialize_skip_rules<'de, D>(deserializer: D) -> Result<Vec<SkipRule>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let patterns = Vec::<String>::deserialize(deserializer)?;
    patterns
        .iter()
        .map(|p| SkipRule::parse(p).map_err(serde::de::Error::custom))
        .collect()
}

#[derive(Clone, Deserialize)]
pub struct WorkspaceConfig {
    pub postgres_enums_lib: Option<String>,
    pub postgres_enums_dir: Option<String>,
    pub shared_graphql_enums_lib: String,
    pub shared_graphql_enums_dir: String,
    pub schema_libs_prefix: String,
    pub schema_libs_dir: String,
    #[serde(default = "Vec::new")]
    pub variant_enums: Vec<String>,
    #[serde(default = "mk_false")]
    pub create_root_aliases: bool,
    pub enums_package_name: String,
    /// GraphQL types matching these rules are not generated, fields returning
    /// them are dropped, and arguments taking them are dropped.
    #[serde(default, deserialize_with = "deserialize_skip_rules")]
    pub skip_types: Vec<SkipRule>,
    /// Fields whose name matches these rules are dropped.
    #[serde(default, deserialize_with = "deserialize_skip_rules")]
    pub skip_keys: Vec<SkipRule>,
}

fn mk_false () -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_rules_negate_with_last_match_winning() {
        let rules = vec![
            SkipRule::parse("_stddev").unwrap(),
            SkipRule::parse("!keep_this_stddev").unwrap(),
        ];
        let should_skip = |name: &str| {
            let mut skip = false;
            for rule in &rules {
                if rule.pattern.is_match(name) {
                    skip = !rule.negated;
                }
            }
            skip
        };
        assert!(should_skip("foo_stddev_fields"));
        assert!(!should_skip("keep_this_stddev"));
        assert!(!should_skip("unrelated"));
    }
}
