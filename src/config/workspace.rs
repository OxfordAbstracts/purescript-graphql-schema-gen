use std::thread::Result;

use hashlink::LinkedHashMap;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use yaml_rust2::{yaml, Yaml};

pub async fn parse_workspace() -> Result<WorkspaceConfig> {
    let file_path: String = std::env::var("SPAGO_WORKSPACE_CONFIG_YAML")
        .expect("SPAGO_WORKSPACE_CONFIG_YAML must be set");
    let mut f = File::open(file_path.clone())
        .await
        .expect(format!("Failed to locate or open spago workspace config yaml at: {}", file_path).as_str());
    let mut s = String::new();

    f.read_to_string(&mut s)
        .await
        .expect("Failed to read spago workspace config yaml file to string.");

    let docs =
        yaml::YamlLoader::load_from_str(&s).expect("Failed to parse workspace YAML as YAML.");

    if let Yaml::Hash(hash) = &docs[0] {
        match WorkspaceConfig::new(hash) {
            Some(config) => return Ok(config),
            None => (),
        }
    }

    panic!("Invalid workspace YAML. Please check your workspaces YAML file matches the example in the README.");
}

#[derive(Clone)]
pub struct WorkspaceConfig {
    pub postgres_enums_lib: Option<String>,
    pub postgres_enums_dir: Option<String>,
    pub shared_graphql_enums_lib: String,
    pub shared_graphql_enums_dir: String,
    pub schema_libs_prefix: String,
    pub schema_libs_dir: String,
    pub variant_enums: Vec<String>,
}

impl WorkspaceConfig {
    fn new(yaml_hash: &LinkedHashMap<Yaml, Yaml>) -> Option<Self> {
        let postgres_enums_lib = yaml_hash.get(&Yaml::String("postgres_enums_lib".to_string()))?;
        let postgres_enums_dir = yaml_hash.get(&Yaml::String("postgres_enums_dir".to_string()))?;
        let shared_graphql_enums_lib =
            yaml_hash.get(&Yaml::String("shared_graphql_enums_lib".to_string()))?;
        let shared_graphql_enums_dir =
            yaml_hash.get(&Yaml::String("shared_graphql_enums_dir".to_string()))?;
        let schema_libs_prefix = yaml_hash.get(&Yaml::String("schema_libs_prefix".to_string()))?;
        let schema_libs_dir = yaml_hash.get(&Yaml::String("schema_libs_dir".to_string()))?;
        let variant_enums = yaml_hash.get(&Yaml::String("variant_enums".to_string()));
        Some(Self {
            postgres_enums_lib: postgres_enums_lib
                .as_str()
                .map(|s| s.to_string()),
            postgres_enums_dir: postgres_enums_dir
                .as_str()
                .map(|s| s.to_string()),

                // .expect("Workspace yaml should contain postgres_enums_dir key.")
                // .to_string(),
            shared_graphql_enums_lib: shared_graphql_enums_lib
                .as_str()
                .expect("Workspace yaml should contain shared_graphql_enums_lib key.")
                .to_string(),
            shared_graphql_enums_dir: shared_graphql_enums_dir
                .as_str()
                .expect("Workspace yaml should contain shared_graphql_enums_dir key.")
                .to_string(),
            schema_libs_prefix: schema_libs_prefix
                .as_str()
                .expect("Workspace yaml should contain schema_libs_prefix key.")
                .to_string(),
            schema_libs_dir: schema_libs_dir
                .as_str()
                .expect("Workspace yaml should contain schema_libs_dir key.")
                .to_string(),
            variant_enums: variant_enums
                .unwrap_or(&Yaml::Array(vec![]))
                .as_vec()
                .unwrap_or(&vec![])
                .iter()
                .map(|v| {
                    v.as_str()
                        .expect("Workspace yaml variant enums should all be strings")
                        .to_string()
                })
                .collect(),
        })
    }
}
