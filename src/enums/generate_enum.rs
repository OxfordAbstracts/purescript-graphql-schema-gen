use cynic_introspection::EnumType;
use stringcase::pascal_case;

use crate::purescript_gen::purescript_enum::Enum;
use crate::purescript_gen::purescript_import::PurescriptImport;
use crate::purescript_gen::purescript_variant::Variant;
use crate::write::write;

pub async fn generate_enum(en: &EnumType, imports: &mut Vec<PurescriptImport>) -> Option<Variant> {
    // TODO this env could be faster if it was pulled in and parsed once at the start
    // TODO check timings to see if it makes a difference
    // Fetch the global enum suffixes
    let global_enum_suffixes_env =
        std::env::var("SHARED_ENUM_SUFFIXES").expect("SHARED_ENUM_SUFFIXES must be set");
    let global_enum_suffixes: Vec<&str> = global_enum_suffixes_env.split(",").collect();

    // Empty enums in Hasura are represented as a single value with the name "_PLACEHOLDER"
    // purescript enums cannot start with an underscore, so we need to replace it with a different placeholder
    let values = if en.values.iter().next().unwrap().name == "_PLACEHOLDER" {
        vec!["ENUM_PLACEHOLDER".to_string()]
    } else {
        en.values.iter().map(|v| first_upper(&v.name)).collect()
    };
    let original_values: Vec<String> = en.values.iter().map(|v| v.name.clone()).collect();
    let name: String = pascal_case(&en.name);

    // Some enums are shared between all schemas
    if global_enum_suffixes
        .iter()
        .any(|suffix| name.ends_with(suffix))
    {
        let e = Enum::new(&name).with_values(&values).to_string();

        let instances = enum_instances(&name, &values, &original_values);

        let module_name = format!("GeneratedGql.{}", name);
        imports.push(PurescriptImport::new(&module_name, "oa-gql-enums").add_specified(&name));

        write(
            &format!("./purs/lib/oa-gql-enums/src/GeneratedGql/{name}.purs"),
            &format!("module {module_name} ({name}) where\n\n{MODULE_IMPORTS}\n\n{e}{instances}"),
        );
        write(
            &format!("./purs/lib/oa-gql-enums/spago.yaml"),
            &enums_spago_yaml(),
        );
        None
    // Otherwise write schema-specific variant enums
    } else {
        Some(Variant::new(&name).with_values(&original_values))
    }
}

fn first_upper(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn enum_instances(name: &str, values: &Vec<String>, original_values: &Vec<String>) -> String {
    let mut instances = String::new();
    // instances.push_str(&format!(
    //     "\ninstance MakeFixture {} where mkFixture = {}",
    //     name, values[0]
    // ));
    instances.push_str(&format!(
        "\n\ninstance FC.Decode {} where\n  decode = unsafeFromForeign >>> decodeJson >>> lmap (D.printJsonDecodeError >>> F.ForeignError >>> pure) >>> except",
        name
    ));
    instances.push_str(&format!(
        "\n\ninstance FC.Encode {} where\n  encode = encodeJson >>> unsafeToForeign",
        name
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
        "\n\ninstance GqlArgString {} where\n  toGqlArgStringImpl = show",
        name
    ));
    instances.push_str(&format!(
        "\n\ninstance DecodeJson {} where\n  decodeJson = decodeJson >=> case _ of\n    {}\n    s -> Left $ TypeMismatch $ \"Not a {}: \" <> s",
        name, values.iter().map(|v| format!("\"{}\" -> pure {}", v, v)).collect::<Vec<String>>().join("\n    "), name
    ));
    instances.push_str(&format!(
        "\n\ninstance EncodeJson {} where\n  encodeJson = show >>> encodeJson",
        name
    ));
    instances.push_str(&format!(
        "\n\ninstance DecodeHasura {} where\n  decodeHasura = decodeJson",
        name
    ));
    instances.push_str(&format!(
        "\n\ninstance EncodeHasura {} where\n  encodeHasura = encodeJson",
        name
    ));
    instances.push_str(&format!(
        "\n\ninstance Show {} where\n  show a = case a of\n    {}",
        name,
        values
            .iter()
            .zip(original_values.iter())
            .map(|(v, ov)| format!("{} -> \"{}\"", v, ov))
            .collect::<Vec<String>>()
            .join("\n    ")
    ));

    instances.push_str(&format!(
        "\n\ninstance Enum {} where\n  succ a = case a of\n    {}\n  pred a = case a of\n    {}",
        name,
        values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if i == values.len() - 1 {
                    format!("{} -> Nothing", v)
                } else {
                    format!("{} -> Just {}", v, values[i + 1])
                }
            })
            .collect::<Vec<String>>()
            .join("\n    "),
        values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if i == 0 {
                    format!("{} -> Nothing", v)
                } else {
                    format!("{} -> Just {}", v, values[i - 1])
                }
            })
            .collect::<Vec<String>>()
            .join("\n    ")
    ));

    instances.push_str(&format!(
        "\n\ninstance Bounded {} where\n  top = {}\n  bottom = {}",
        name,
        values.last().unwrap(),
        values.first().unwrap()
    ));

    instances.push_str(&format!(
        "\n\ninstance BoundedEnum {} where\n  cardinality = Cardinality {}\n  toEnum a = case a of\n    {}\n    _ -> Nothing\n  fromEnum a = case a of\n    {}",
        name,
        values.len(),
        values
            .iter()
            .enumerate()
            .map(|(i, v)| format!("{} -> Just {}", i, v))
            .collect::<Vec<String>>()
            .join("\n    "),
        values
            .iter()
            .enumerate()
            .map(|(i, v)| format!("{} -> {}", v, i))
            .collect::<Vec<String>>()
            .join("\n    ")
    ));

    instances
}

fn enums_spago_yaml() -> String {
    r#"package:
  name: oa-gql-enums
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
    .to_string()
}

const MODULE_IMPORTS: &str = r#"import Prelude

import Data.Argonaut.Decode (class DecodeJson, JsonDecodeError(..), decodeJson)
import Data.Argonaut.Encode (class EncodeJson, encodeJson)
import Data.Enum (class Enum, class BoundedEnum, Cardinality(..))
import Data.Either (Either(..))
import Data.Function (on)
import Data.Maybe (Maybe(..))
import GraphQL.Client.ToGqlString (class GqlArgString)
import GraphQL.Hasura.Decode (class DecodeHasura)
import GraphQL.Hasura.Encode (class EncodeHasura)
-- import OaMakeFixture (class MakeFixture)
import Foreign (unsafeFromForeign, unsafeToForeign)
import Foreign as F
import Data.Argonaut.Decode as D
import Control.Monad.Except (except)
import Data.Bifunctor (lmap)
import Foreign.Class as FC"#;
