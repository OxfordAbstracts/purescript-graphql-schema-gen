use std::collections::HashMap;

use sqlx::{postgres::PgPoolOptions, Result};
use stringcase::{pascal_case, snake_case};

use crate::{
    config::workspace::WorkspaceConfig, purescript_gen::purescript_enum::Enum, write::write,
};

pub async fn fetch_types(
    workspace_config: &WorkspaceConfig,
) -> Result<HashMap<String, (String, String, String)>> {
    let db_env = std::env::var("DATABASE_URL");

    // when no postgres enums are included, skip the enum generation
    if db_env.is_err() {
        return Ok(HashMap::new());
    }
    let database_url = db_env.unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("Failed to create pool");

    let res: Vec<EnumType> = sqlx::query_as::<_, EnumType>(
        r#"SELECT pg_type.typname AS enumtype, array_agg(pg_enum.enumlabel) as enumlabel
      FROM pg_type
      INNER JOIN pg_enum ON pg_enum.enumtypid = pg_type.oid
      GROUP BY typname
      ORDER BY 1 ASC;"#,
    )
    .fetch_all(&pool)
    .await?;

    let mut hash_map = HashMap::new();

    let package_name = pascal_case(&workspace_config.postgres_enums_lib);
    let lib_path = format!(
        "{}{}",
        &workspace_config.postgres_enums_dir, &workspace_config.postgres_enums_lib
    );
    let package = &workspace_config.postgres_enums_lib;

    for enum_row in res.iter() {
        let name = enum_row.enumtype.clone();
        let type_ = pascal_case(&name);
        let import = format!("{package_name}.{type_}");
        let contents = write_enum_module(&enum_row, &package_name);

        write(
            &format!("{lib_path}/src/{package_name}/{type_}.purs"),
            &contents,
        );
        write(
            &format!("{lib_path}/spago.yaml"),
            &enums_spago_yaml(package),
        );
        hash_map.insert(name, (package.clone(), import, type_));
    }

    Ok(hash_map)
}

fn enums_spago_yaml(name: &str) -> String {
    format!(
        r#"package:
  name: {name}
  dependencies:
    - argonaut
    - argonaut-codecs
    - arrays
    - bifunctors
    - either
    - enums
    - foreign
    - foreign-generic
    - graphql-client
    - prelude
    - simple-json
    - transformers
"#
    )
}

#[derive(sqlx::Type, sqlx::FromRow)]
struct EnumType {
    enumtype: String,
    enumlabel: Option<Vec<String>>,
}

fn write_enum_module(enum_row: &EnumType, package_name: &str) -> String {
    let name = pascal_case(&enum_row.enumtype);
    let original_values: Vec<String> = match enum_row.enumlabel.as_ref() {
        Some(v) => v.clone(),
        None => vec!["ENUM_PLACEHOLDER".to_string()],
    };
    let values = original_values
        .iter()
        .map(|v| snake_case(v).to_uppercase())
        .collect();

    let mod_name = format!("module {package_name}.{name} ({name}(..)) where");
    let enum_definition = Enum::new(&name).with_values(&values).to_string();
    let instances_and_fns = enum_body(&name, &values, &original_values);

    format!("{mod_name}\n\n{MODULE_IMPORTS}\n\n{enum_definition}\n\n{instances_and_fns}")
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
        "\n\ninstance Eq {name} where\n  eq = eq `on` show",
    ));

    instances.push_str(&format!(
        "\n\ninstance Ord {name} where\n  compare = compare `on` show",
    ));

    instances.push_str(&format!(
        "\n\ninstance Enum {name} where\n  pred a = do\n    idx <- findIndex (eq a) all{name}\n    all{name} !! (idx - 1)\n  succ a = do\n    idx <- findIndex (eq a) all{name}\n    all{name} !! (idx + 1)",
    ));

    instances.push_str(&format!(
        "\n\ninstance Bounded {name} where\n  top = {}\n  bottom = {}",
        values.last().unwrap(),
        values.first().unwrap()
    ));

    instances.push_str(&format!(
        "\n\ninstance Decode {name} where\n  decode =\n    readString\n      >=>\n        ( fromString\n            >>> decodeJson\n            >>> lmap (printJsonDecodeError >>> ForeignError >>> pure)\n            >>> except\n        )",
    ));

    instances.push_str(&format!(
        "\n\ninstance Encode {name} where\n  encode = show >>> encode",
    ));

    instances.push_str(&format!(
        "\n\ninstance WriteForeign {name} where\n  writeImpl = encode",
    ));

    instances.push_str(&format!(
        "\n\ninstance ReadForeign {name} where\n  readImpl = decode",
    ));

    instances.push_str(&format!(
        "\n\ninstance DecodeHasura {name} where\n  decodeHasura = decodeJson",
    ));

    instances.push_str(&format!(
        "\n\ninstance EncodeHasura {name} where\n  encodeHasura = show >>> encodeJson",
    ));

    instances.push_str(&format!(
        "\n\ninstance Show {name} where\n  show = case _ of\n    {}",
        values
            .iter()
            .zip(original_values.iter())
            .map(|(v, o)| format!("{} -> \"{}\"", v, o))
            .collect::<Vec<String>>()
            .join("\n    ")
    ));

    instances.push_str(&format!(
        "\n\ninstance GqlArgString {name} where\n  toGqlArgStringImpl = show >>> show",
    ));

    instances.push_str(&format!(
        "\n\ninstance DecodeJson {name} where\n  decodeJson =\n    decodeJson\n      >=> case _ of\n        {}\n        str ->\n          Left\n            $ TypeMismatch\n            $ \"Failed to decode {name} from string: \"\n                <> str",
        values
            .iter()
            .zip(original_values.iter())
            .map(|(v, o)| format!("\"{}\" -> pure {}", o, v))
            .collect::<Vec<String>>()
            .join("\n        "),
    ));

    // instances.push_str(&format!(
    //     "\n\ninstance MakeFixture {} where\n  mkFixture = {}",
    //     name,
    //     values.first().unwrap()
    // ));

    instances
}
