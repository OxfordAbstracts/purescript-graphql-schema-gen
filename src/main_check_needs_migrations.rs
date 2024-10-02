use std::fs;

use dotenv::dotenv;
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, Result};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let db_env = std::env::var("DATABASE_URL").expect("DATABASE_URL var must be set.");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&db_env)
        .await
        .expect("Failed to create pool");

    let standard_result: Migrations = sqlx::query_as::<_, Migrations>(
        r#"SELECT (cli_state->'migrations'->'default')::text as migrations from hdb_catalog.hdb_version;"#,
    )
    .fetch_one(&pool)
    .await?;

    let test_db = std::env::var("CODEGEN_DATABASE_URL")
        .expect("CODEGEN_DATABASE_URL must be set so we can use it as a codegen base.");
    let hasura_migrations_dir =
        std::env::var("HASURA_MIGRATIONS_DIR").expect("HASURA_MIGRATIONS_DIR must be set.");
    let test_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&test_db)
        .await
        .expect("Failed to create pool");

    let test_result: Migrations = sqlx::query_as::<_, Migrations>(
            r#"SELECT (cli_state->'migrations'->'default')::text as migrations from hdb_catalog.hdb_version;"#,
        )
        .fetch_one(&test_pool)
        .await?;

    let migrations: Value = serde_json::from_str(&standard_result.migrations).expect(
        "Failed to parse hasura migrations json. Perhaps your Hasura version is mismatched.",
    );
    let test_migrations: Value = serde_json::from_str(&test_result.migrations)
        .expect("Failed to parse test migrations json.");

    let migrations_obj = migrations
        .as_object()
        .expect("Failed to parse hasura migrations as an object.");
    let test_migrations_obj = test_migrations
        .as_object()
        .expect("Failed to parse test migrations as an object.");

    for (k, _) in test_migrations_obj.iter() {
        if !migrations_obj.contains_key(k) {
            println!("Migrations missing from test db");
            return Ok(());
        }
    }
    for (k, _) in migrations_obj.iter() {
        if !test_migrations_obj.contains_key(k) {
            println!("Migrations missing from main db");
            return Ok(());
        }
    }

    let last_test = test_migrations_obj
        .iter()
        .last()
        .expect("Must have at least one migration in test database.") // TODO make this optional?
        .0
        .clone();
    let last = migrations_obj
        .iter()
        .last()
        .expect("Must have at least one migration in your database.") // TODO again, make this optional?
        .0
        .clone();

    let mut time_stamps = vec![];
    let files =
        fs::read_dir(hasura_migrations_dir).expect("The chosen Hasura directory must exist.");
    for entry in files {
        let path = entry
            .expect("Failed to parse path in Hasura directory.")
            .path();
        let timestamp = path
            .file_name()
            .expect("Failed to parse file name in Hasura dir.")
            .to_str()
            .expect("Failed to convert file name to string in Hasura dir.")
            .split("_")
            .next()
            .expect("Failed to split timestamp out of file name in Hasura migrations dir.");
        time_stamps.push(timestamp.to_string());
    }
    time_stamps.sort();
    let tz = time_stamps[time_stamps.len() - 1].clone();

    if last_test < tz || last < tz {
        println!("New migration exists");
        return Ok(());
    }

    println!("Up to date");
    Ok(())
}

#[derive(sqlx::Type, sqlx::FromRow, Debug)]
struct Migrations {
    migrations: String,
}
