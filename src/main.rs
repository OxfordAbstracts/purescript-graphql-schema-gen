use std::{
    sync::{Arc, Mutex},
    thread::Result,
};

use build_schema::build_schema;
use config::{
    parse_outside_types::{fetch_all_outside_types, OutsideTypes},
    parse_roles::parse_roles,
    workspace::parse_workspace,
};
use dotenv::dotenv;
use enums::postgres_types::fetch_types;
use tokio::spawn;
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
    let workspace_config = parse_workspace().await?;

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

    // Fetch role config
    let roles: Vec<String> = parse_roles();
    let num_roles = roles.len();

    // Postgres types are shared between all roles
    let types_ = Arc::new(Mutex::new(postgres_types));
    let outside_types = Arc::new(Mutex::new(outside_types));

    // Run schema gen for each role concurrently
    let mut tasks = Vec::with_capacity(num_roles);
    for role in roles.iter() {
        tasks.push(spawn(build_schema(
            role.clone(),
            types_.clone(),
            outside_types.clone(),
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

    println!(
        "Generated {num_roles} schemas in {:.2}s",
        start.elapsed().as_secs_f32()
    );

    Ok(())
}
