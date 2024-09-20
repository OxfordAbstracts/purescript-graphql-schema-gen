use std::collections::HashMap;

use dotenv::dotenv;
use sqlx::{postgres::PgPoolOptions, Result};
use stringcase::pascal_case;

use crate::{generate_enum::write, purescript_enum::Enum};

pub async fn fetch_types() -> Result<HashMap<String, (String, String)>> {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("Failed to create pool");

    let res = sqlx::query_as!(
        EnumType,
        r#"SELECT pg_type.typname AS enumtype, array_agg(pg_enum.enumlabel) as enumlabel
      FROM pg_type
      INNER JOIN pg_enum ON pg_enum.enumtypid = pg_type.oid
      GROUP BY typname
      ORDER BY 1 ASC;"#,
    )
    .fetch_all(&pool)
    .await?;

    let mut hash_map = HashMap::new();

    for enum_row in res.iter() {
        let name = enum_row.enumtype.clone();
        let type_ = pascal_case(&name);
        let import = format!("GeneratedPostgres.Enum.{}", &type_);

        let contents = write_enum_module(&enum_row);

        write(
            &format!("./purs/src/GeneratedPostgres/Enum/{}.purs", &type_),
            &contents,
        );
        hash_map.insert(name, (import, type_));
    }

    Ok(hash_map)
}

#[derive(sqlx::Type)]
struct EnumType {
    enumtype: String,
    enumlabel: Option<Vec<String>>,
}

fn write_enum_module(enum_row: &EnumType) -> String {
    let name = pascal_case(&enum_row.enumtype);
    let original_values: Vec<String> = match enum_row.enumlabel.as_ref() {
        Some(v) => v.clone(),
        None => vec!["ENUM_PLACEHOLDER".to_string()],
    };
    let values = original_values
        .iter()
        .map(|v| pascal_case(v).to_uppercase())
        .collect();

    let mod_name = format!("module GeneratedPostgres.Enum.{} ({}) where", &name, &name);
    let enum_definition = Enum::new(&name).with_values(&values).to_string();
    let instances_and_fns = enum_body(&name, &values, &original_values);

    format!(
        "{}\n\n{}\n\n{}\n\n{}",
        mod_name, MODULE_IMPORTS, enum_definition, instances_and_fns
    )
}

const MODULE_IMPORTS: &str = r#"import Prelude

import Control.Monad.Except (except)
import Data.Argonaut (encodeJson, fromString, printJsonDecodeError)
import Data.Argonaut.Decode (class DecodeJson, JsonDecodeError(..), decodeJson)
import Data.Array (findIndex, (!!))
import Data.Bifunctor (lmap)
import Data.Either (Either(..))
import Data.Function (on)
import Data.Enum (class Enum)
import Foreign (ForeignError(ForeignError), readString)
import Foreign.Class (class Decode, class Encode, decode, encode)
import GraphQL.Client.ToGqlString (class GqlArgString)
import GraphQL.Hasura.Decode (class DecodeHasura)
import GraphQL.Hasura.Encode (class EncodeHasura)
-- import OaMakeFixture (class MakeFixture)
import Simple.JSON (class ReadForeign, class WriteForeign)"#;

pub fn enum_body(name: &str, values: &Vec<String>, original_values: &Vec<String>) -> String {
    let mut instances = String::new();

    let in_array: String = values
        .iter()
        .enumerate()
        .map(|(index, v)| {
            let prefix = if index == 0 { "  [" } else { "  ," };
            format!("{} {}", prefix, v)
        })
        .collect::<Vec<String>>()
        .join("\n");

    instances.push_str(&format!(
        "all{} :: Array {}
all{} =
{}
  ]",
        name, name, name, in_array
    ));

    instances.push_str(&format!(
        "\n\ninstance Eq {} where\n  eq = eq `on` show",
        name
    ));

    instances.push_str(&format!(
        "\n\ninstance Ord {} where\n  compare = compare `on` show",
        name
    ));

    instances.push_str(&format!(
        "\n\ninstance Enum {} where\n  pred a = do\n    idx <- findIndex (eq a) all{}\n    all{} !! (idx - 1)\n  succ a = do\n    idx <- findIndex (eq a) all{}\n    all{} !! (idx + 1)",
        name, name, name, name, name
    ));

    instances.push_str(&format!(
        "\n\ninstance Bounded {} where\n  top = {}\n  bottom = {}",
        name,
        values.last().unwrap(),
        values.first().unwrap()
    ));

    instances.push_str(&format!(
        "\n\ninstance Decode {} where\n  decode =\n    readString\n      >=>\n        ( fromString\n            >>> decodeJson\n            >>> lmap (printJsonDecodeError >>> ForeignError >>> pure)\n            >>> except\n        )",
         name,
    ));

    instances.push_str(&format!(
        "\n\ninstance Encode {} where\n  encode = show >>> encode",
        name
    ));

    instances.push_str(&format!(
        "\n\ninstance WriteForeign {} where\n  writeImpl = encode",
        name
    ));

    instances.push_str(&format!(
        "\n\ninstance ReadForeign {} where\n  readImpl = decode",
        name
    ));

    instances.push_str(&format!(
        "\n\ninstance DecodeHasura {} where\n  decodeHasura = decodeJson",
        name
    ));

    instances.push_str(&format!(
        "\n\ninstance EncodeHasura {} where\n  encodeHasura = show >>> encodeJson",
        name
    ));

    instances.push_str(&format!(
        "\n\ninstance Show {} where\n  show = case _ of\n    {}",
        name,
        values
            .iter()
            .zip(original_values.iter())
            .map(|(v, o)| format!("{} -> \"{}\"", v, o))
            .collect::<Vec<String>>()
            .join("\n    ")
    ));

    instances.push_str(&format!(
        "\n\ninstance GqlArgString {} where\n  toGqlArgStringImpl = show >>> show",
        name
    ));

    instances.push_str(&format!(
        "\n\ninstance DecodeJson {} where\n  decodeJson =\n    decodeJson\n      >=> case _ of\n        {}\n        str ->\n          Left\n            $ TypeMismatch\n            $ \"Failed to decode {} from string: \"\n                <> str",
        name,
        values
            .iter()
            .map(|v| format!("\"{}\" -> pure {}", v, v))
            .collect::<Vec<String>>()
            .join("\n        "),
        name
    ));

    // instances.push_str(&format!(
    //     "\n\ninstance MakeFixture {} where\n  mkFixture = {}",
    //     name,
    //     values.first().unwrap()
    // ));

    instances
}
