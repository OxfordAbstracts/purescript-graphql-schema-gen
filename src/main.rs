use std::{
    collections::HashMap, sync::{Arc, Mutex}, thread::Result
};

use build_schema::build_schema;
use config::{
    parse_outside_types::{fetch_all_outside_types, OutsideTypes}, parse_roles::parse_roles, parse_scalar_types::{fetch_all_scalar_types, ScalarTypes}, workspace::parse_workspace
};
use dotenv::dotenv;
use enums::postgres_types::fetch_types;
use tokio::spawn;
use write::remove_stale_files;
mod build_schema;
mod config;
mod enums;
mod hasura_types;
mod main_check_needs_migrations;
mod purescript_gen;
mod write;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // time the postgres enum type generation
    let type_gen_timer = std::time::Instant::now();

    // Fetch the workspace config
    let workspace_config = parse_workspace();

    // Existing files are left in place so their mtimes survive when the
    // content is unchanged; stale files are removed after generation instead
    // (see the end of main).

    // Generate postgres enum types
    let postgres_types = fetch_types(&workspace_config)
        .await
        .expect("Failed to generate postgres enum types.");
    let num_types = postgres_types.len();

    println!(
        "Generated {num_types} Postgres enums in {:.2}s",
        type_gen_timer.elapsed().as_secs_f32()
    );

    // Time the schema generation
    let start = std::time::Instant::now();

    // Parse all outside type config
    let outside_types: OutsideTypes = fetch_all_outside_types(&workspace_config);

    let scalar_types: ScalarTypes = fetch_all_scalar_types().unwrap_or(HashMap::new());

    // Fetch role config
    let roles: Vec<String> = parse_roles();
    let num_roles = roles.len();

    // Postgres types are shared between all roles
    let types_ = Arc::new(Mutex::new(postgres_types));
    let outside_types = Arc::new(Mutex::new(outside_types));
    let scalar_types = Arc::new(Mutex::new(scalar_types));

    // Run schema gen for each role concurrently
    let mut tasks = Vec::with_capacity(num_roles);

    for role in roles.iter() {
        tasks.push(spawn(build_schema(
            role.clone(),
            types_.clone(),
            outside_types.clone(),
            scalar_types.clone(),
            workspace_config.clone(),
        )));
    }
    // Join the results
    let mut outputs = Vec::with_capacity(tasks.len());
    for task in tasks {
        outputs.push(
            task.await
                .expect("Failed to build schema gen task output")
                .expect("Failed to join schema gen task output"),
        );
    }

    // Remove files left over from previous runs (dropped types, roles or
    // enums) and any directories left empty.
    for dir in [
        workspace_config.postgres_enums_dir.as_ref(),
        Some(&workspace_config.shared_graphql_enums_dir),
        Some(&workspace_config.schema_libs_dir),
    ]
    .into_iter()
    .flatten()
    {
        remove_stale_files(dir);
    }

    println!(
        "Generated {num_roles} schemas in {:.2}s",
        start.elapsed().as_secs_f32()
    );

    Ok(())
}
