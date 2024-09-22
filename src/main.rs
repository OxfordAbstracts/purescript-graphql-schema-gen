use std::{
    sync::{Arc, Mutex},
    thread::Result,
};

use build_schema::build_schema;
use config::{
    parse_outside_types::{fetch_all_outside_types, OutsideTypes},
    parse_roles::parse_roles,
};
use dotenv::dotenv;
use enums::postgres_types::fetch_types;
use tokio::spawn;
use write::write;
mod build_schema;
mod config;
mod enums;
mod hasura_types;
mod purescript_gen;
mod write;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    // time the postgres enum type generation
    let type_gen_timer = std::time::Instant::now();

    // Generate postgres enum types
    let postgres_types = fetch_types().await.unwrap();
    let num_types = postgres_types.len();

    println!(
        "Generated {} postgres enums in {:.2}s",
        num_types,
        type_gen_timer.elapsed().as_secs_f32()
    );

    // Time the schema generation
    let start = std::time::Instant::now();

    // Parse all outside type config
    let outside_types: OutsideTypes = fetch_all_outside_types();

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
        )));
    }
    // Join the results
    let mut outputs = Vec::with_capacity(tasks.len());
    for task in tasks {
        outputs.push(task.await.unwrap().unwrap());
    }
    // Write the output of each schema to a file
    for (output, role) in outputs.into_iter().zip(roles.into_iter()) {
        write(
            &format!("./purs/src/Schema/{}/{}.purs", role, role),
            &output,
        );
    }

    println!(
        "Generated {} schemas in {:.2}s",
        num_roles,
        start.elapsed().as_secs_f32()
    );

    Ok(())
}
