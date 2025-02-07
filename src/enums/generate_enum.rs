use cynic_introspection::EnumType;
use stringcase::pascal_case;

use crate::config::workspace::WorkspaceConfig;
use crate::purescript_gen::purescript_enum::Enum;
use crate::purescript_gen::purescript_import::PurescriptImport;
use crate::purescript_gen::purescript_variant::Variant;
use crate::write::write;

pub async fn generate_enum(
    en: &EnumType,
    imports: &mut Vec<PurescriptImport>,
    workspace_config: &WorkspaceConfig,
) -> Option<Variant> {
    // TODO this env could be faster if it was pulled in and parsed once at the start
    // TODO check timings to see if it makes a difference
    // Fetch the global enum suffixes
    let global_enum_suffixes_env =
        std::env::var("SHARED_ENUM_SUFFIXES").expect("SHARED_ENUM_SUFFIXES must be set");
    let global_enum_suffixes: Vec<&str> = global_enum_suffixes_env.split(",").collect();

    // Empty enums in Hasura are represented as a single value with the name "_PLACEHOLDER"
    // purescript enums cannot start with an underscore, so we need to replace it with a different placeholder
    let values = if en
        .values
        .iter()
        .next()
        .expect("Enums should have at least one value.")
        .name
        == "_PLACEHOLDER"
    {
        vec!["ENUM_PLACEHOLDER".to_string()]
    } else {
        en.values.iter().map(|v| first_upper(&v.name)).collect()
    };
    let original_values: Vec<String> = en.values.iter().map(|v| v.name.clone()).collect();
    let name: String = pascal_case(&en.name);

    // Some enums are shared between all schemas
    // Hasura suffixes 'Enum' to the end of custom enums created
    // via a table with 'value' and 'comment' columns.
    if global_enum_suffixes
        .iter()
        .any(|suffix| name.ends_with(suffix))
    {
        if use_variant(&name, &workspace_config) {
            let lib_path = format!(
                "{}{}",
                &workspace_config.shared_graphql_enums_dir,
                &workspace_config.shared_graphql_enums_lib
            );
            let package_name = pascal_case(&workspace_config.shared_graphql_enums_lib);

            if let Some(variant) = variant_mod(&name, &original_values) {
                write(
                    &format!("{lib_path}/src/{package_name}/{name}.purs"),
                    &variant,
                );
                write(&format!("{lib_path}/spago.yaml"), &enums_spago_yaml());
            }

            None
        } else {
            let e = Enum::new(&name).with_values(&values).to_string();

            let instances = enum_instances(&name, &values, &original_values);
            let package_name = pascal_case(&workspace_config.shared_graphql_enums_lib);
            let module_name = format!("{package_name}.{name}");
            imports.push(PurescriptImport::new(&module_name, "oa-gql-enums").add_specified(&name));

            let lib_path = format!(
                "{}{}",
                &workspace_config.shared_graphql_enums_dir,
                &workspace_config.shared_graphql_enums_lib
            );
            write(
                &format!("{lib_path}/src/{package_name}/{name}.purs"),
                &format!(
                    "module {module_name} ({name}(..)) where\n\n{MODULE_IMPORTS}\n\n{e}{instances}"
                ),
            );
            write(&format!("{lib_path}/spago.yaml"), &enums_spago_yaml());
            None
        }
    // Otherwise write schema-specific variant enums
    } else {
        Some(Variant::new(&name).with_values(&original_values))
    }
}

fn use_variant(name: &str, workspace_config: &WorkspaceConfig) -> bool {
    workspace_config
        .variant_enums
        .iter()
        .any(|suffix| name.ends_with(suffix))
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
    instances.push_str(&format!(
        "\n\ninstance MakeFixture {name} where mkFixture = {}",
        values[0]
    ));
    instances.push_str(&format!(
        "\n\ninstance FC.Decode {name} where\n  decode = unsafeFromForeign >>> decodeJson >>> lmap (D.printJsonDecodeError >>> F.ForeignError >>> pure) >>> except",
    ));
    instances.push_str(&format!(
        "\n\ninstance FC.Encode {name} where\n  encode = encodeJson >>> unsafeToForeign"
    ));
    instances.push_str(&format!(
        "\n\ninstance Eq {name} where\n  eq = eq `on` show"
    ));
    instances.push_str(&format!(
        "\n\ninstance Ord {name} where\n  compare = compare `on` show"
    ));
    instances.push_str(&format!(
        "\n\ninstance GqlArgString {name} where\n  toGqlArgStringImpl = show"
    ));
    instances.push_str(&format!(
        "\n\ninstance DecodeJson {name} where\n  decodeJson = decodeJson >=> case _ of\n    {}\n    s -> Left $ TypeMismatch $ \"Not a {name}: \" <> s",
        values.iter().zip(original_values.iter())
            .map(|(v, ov)| format!("\"{ov}\" -> pure {v}")).collect::<Vec<String>>().join("\n    ")
    ));
    instances.push_str(&format!(
        "\n\ninstance EncodeJson {name} where\n  encodeJson = show >>> encodeJson"
    ));
    instances.push_str(&format!(
        "\n\ninstance DecodeHasura {name} where\n  decodeHasura = decodeJson"
    ));
    instances.push_str(&format!(
        "\n\ninstance EncodeHasura {name} where\n  encodeHasura = encodeJson"
    ));
    instances.push_str(&format!(
        "\n\ninstance DecodeOa {name} where
    decodeOa = FC.decode"
    ));
    instances.push_str(&format!(
        "\n\ninstance EncodeOa {name} where
    encodeOa = FC.encode"
    ));
    instances.push_str(&format!(
        "\n\ninstance Show {name} where\n  show a = case a of\n    {}",
        values
            .iter()
            .zip(original_values.iter())
            .map(|(v, ov)| format!("{} -> \"{}\"", v, ov))
            .collect::<Vec<String>>()
            .join("\n    ")
    ));

    instances.push_str(&format!(
        "\n\ninstance Enum {name} where\n  succ a = case a of\n    {}\n  pred a = case a of\n    {}",
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
        "\n\ninstance Bounded {name} where\n  top = {}\n  bottom = {}",
        values
            .last()
            .expect("Enums should have at least one value in order to get last."),
        values
            .first()
            .expect("Enums should have at least one value in order to get first.")
    ));

    instances.push_str(&format!(
        "\n\ninstance BoundedEnum {name} where\n  cardinality = Cardinality {}\n  toEnum a = case a of\n    {}\n    _ -> Nothing\n  fromEnum a = case a of\n    {}",
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

fn variant_mod(name: &str, original_values: &Vec<String>) -> Option<String> {
    if original_values.len() == 0 {
        return None;
    }

    let values: Vec<String> = original_values.iter().map(|v| v.to_lowercase()).collect();

    let mut instances = String::new();
    let first_value = &values[0];
    let last_value = values
        .last()
        .expect("Enums should have at least one value in order to get last.");

    let mut variant = String::new();
    let mut variant_fns = String::new();

    // Define the type
    variant.push_str(&format!(
        r#"
type {name} = Variant"#
    ));

    for value in &values {
        let variant_name = value.to_lowercase();
        let variant_member = if value == first_value {
            format!("  ( {variant_name}\n")
        } else {
            format!("  , {variant_name}\n")
        };

        // Add the variant member to the row
        variant.push_str(&variant_member);

        // Define the variant fn for easy calling
        variant_fns.push_str(&to_variant(name, &variant_name));
    }

    // Add the variant type closing bracket
    variant.push_str("\n  )");

    // instances:

    // Next values in the enum
    let succ_values = values
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
        .join("\n    ");

    // Previous values in the enum
    let pred_values = values
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
        .join("\n    ");

    // Cardinality of the enum
    let cardinality = values.len();

    // Convert an enum index to the corresponding value
    let to_enum = values
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{} -> Just {}", i, v))
        .collect::<Vec<String>>()
        .join("\n    ");

    // Convert an enum value to the corresponding index
    let from_enum = values
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{} -> {}", v, i))
        .collect::<Vec<String>>()
        .join("\n    ");

    let decode_json = values
        .iter()
        .zip(original_values.iter())
        .map(|(v, original)| format!(r#""{original}" -> pure {v}"#))
        .collect::<Vec<String>>()
        .join("\n    ");

    let encode_json = values
        .iter()
        .zip(original_values.iter())
        .map(|(v, original)| format!(r#"on (Proxy @"{v}") (\_ -> show {original})"#))
        .collect::<Vec<String>>()
        .join("\n    ");

    instances.push_str(&format!(
        r#"

instance DecodeJson {name} where
  decodeJson = decodeJson >=> case _ of
    {decode_json}
    s -> Left $ TypeMismatch $ \"Not a {name}: \" <> s

instance EncodeJson {name} where
  encodeJson = case_
   {encode_json}

instance MakeFixture {name} where mkFixture = {first_value}

instance Show {name} where
  show a = unvariant q.data_type # \(Unvariant f) -> f \p _ -> reflectSymbol p

instance Eq {name} where
  eq = eq `on` show

instance Ord {name} where
  compare = compare `on` show

instance GqlArgString {name} where
  toGqlArgStringImpl = show

instance DecodeHasura {name} where
  decodeHasura = decodeJson

instance EncodeHasura {name} where
  encodeHasura = encodeJson

instance Enum {name} where
  succ a = case a of
    {succ_values}
  pred a = case a of
    {pred_values}

instance Bounded {name} where
  top = {last_value}
  bottom = {first_value}

instance BoundedEnum {name} where
  cardinality = Cardinality {cardinality}
  toEnum a = case a of
    {to_enum}
    _ -> Nothing
  fromEnum a = case a of
    {from_enum}
"#
    ));

    Some(format!("module {name} where\n\n{VARIANT_MODULE_IMPORTS}\n\n{variant}\n\n{variant_fns}\n\n{instances}"))
}

fn to_variant(type_name: &str, name: &str) -> String {
    format!(
        r#"
var :: {type_name}
var = inj (Proxy @"{name}") unit
"#
    )
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
    - variant
    - oa-make-fixture
    - oa-encode-decode
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
import OaMakeFixture (class MakeFixture)
import Foreign (unsafeFromForeign, unsafeToForeign)
import Foreign as F
import Data.Argonaut.Decode as D
import Control.Monad.Except (except)
import Data.Bifunctor (lmap)
import Foreign.Class as FC
import Class.EncodeOa (class EncodeOa)
import Class.DecodeOa (class DecodeOa)"#;

const VARIANT_MODULE_IMPORTS: &str = r#"import Prelude

import Data.Variant (Unvariant(..), Variant, inj, unvariant)
import Data.Enum (class Enum, class BoundedEnum, Cardinality(..))
import Proxy (Proxy(..))"#;
