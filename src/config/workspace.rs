
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
}

fn mk_false () -> bool {
    false
}