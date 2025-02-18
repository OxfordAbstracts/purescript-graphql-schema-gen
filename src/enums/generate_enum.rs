use cynic_introspection::EnumType;
use stringcase::{camel_case, pascal_case};

use crate::config::workspace::WorkspaceConfig;
use crate::purescript_gen::purescript_enum::Enum;
use crate::purescript_gen::purescript_import::PurescriptImport;
use crate::purescript_gen::purescript_variant::Variant;
use crate::purescript_gen::upper_first::upper_first;
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
        en.values.iter().map(|v| upper_first(&v.name)).collect()
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
            let module_name = format!("{package_name}.{name}");
            let helper_module = format!("{package_name}.Utils.VariantHelpers");
            if let Some(variant) = variant_mod(
                &name,
                &original_values,
                &module_name,
                &format!("\nimport {helper_module} (var, match)"),
            ) {
                imports
                    .push(PurescriptImport::new(&module_name, "oa-gql-enums").add_specified(&name));

                write(
                    &format!("{lib_path}/src/{package_name}/{name}.purs"),
                    &variant,
                );
                write(
                    &format!("{lib_path}/src/{package_name}/Utils/VariantHelpers.purs"),
                    &format!("module {helper_module} where \n{VARIANT_HELPERS_MOD}"),
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
        Some(Variant::new(&name).with_values(&original_values).clone())
    }
}

fn use_variant(name: &str, workspace_config: &WorkspaceConfig) -> bool {
    workspace_config.variant_enums.iter().any(|e| name == e)
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

fn variant_mod(
    name: &str,
    original_values: &Vec<String>,
    module_name: &str,
    helper_import: &str,
) -> Option<String> {
    if original_values.len() == 0 {
        return None;
    }

    let values: Vec<String> = original_values.iter().map(|v| v.to_lowercase()).collect();
    let zipped: Vec<(&String, &String)> = values.iter().zip(original_values.iter()).collect();

    let mut instances = String::new();
    let first_value = &values[0];
    let first_fn = camel_case(&original_values[0]);
    let last_fn = camel_case(
        values
            .last()
            .expect("Enums should have at least one value in order to get last."),
    );

    let mut variant = String::new();
    let mut variant_fns = String::new();

    // Define the type
    variant.push_str(&format!(
        r#"newtype {name} = {name} {name}Variant

type {name}Variant = Variant
"#
    ));

    for (lower, original) in zipped {
        let variant_member = if lower == first_value {
            format!("  ( \"{original}\" :: Unit\n")
        } else {
            format!("  , \"{original}\" :: Unit\n")
        };

        // Add the variant member to the row
        variant.push_str(&variant_member);

        // Define the variant fn for easy calling
        variant_fns.push_str(&to_variant(name, &original));
    }

    // Add the variant type closing bracket
    variant.push_str("  )");

    // instances:

    // Next values in the enum
    let succ_values = original_values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            if i == values.len() - 1 {
                format!(r#"# match @"{v}" Nothing"#)
            } else {
                let next_fn = camel_case(&values[i + 1]);
                format!(r#"# match @"{v}" (Just {next_fn})"#)
            }
        })
        .collect::<Vec<String>>()
        .join("\n        ");

    // Previous values in the enum
    let pred_values = original_values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            if i == 0 {
                format!(r#"# match @"{v}" Nothing"#)
            } else {
                let prev_fn = camel_case(&values[i - 1]);
                format!(r#"# match @"{v}" (Just {prev_fn})"#)
            }
        })
        .collect::<Vec<String>>()
        .join("\n        ");

    // Cardinality of the enum
    let cardinality = values.len();

    // Convert an enum index to the corresponding value
    let to_enum = values
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{i} -> Just {}", camel_case(v)))
        .collect::<Vec<String>>()
        .join("\n    ");

    // Convert an enum value to the corresponding index
    let from_enum = original_values
        .iter()
        .enumerate()
        .map(|(i, v)| format!(r#"# match @"{v}" {i}"#))
        .collect::<Vec<String>>()
        .join("\n        ");

    let decode_json = original_values
        .iter()
        .map(|original| {
            let fn_name = camel_case(original);
            format!(r#""{original}" -> pure {fn_name}"#)
        })
        .collect::<Vec<String>>()
        .join("\n    ");

    instances.push_str(&format!(
        r#"
derive instance Newtype {name} _

instance Show {name} where
  show = unwrap >>> unvariant >>> \(Unvariant f) -> f \p _ -> reflectSymbol p

instance MakeFixture {name} where
  mkFixture = {first_fn}

instance Eq {name} where
  eq = eq `on` show

instance Ord {name} where
  compare = compare `on` show

instance GqlArgString {name} where
  toGqlArgStringImpl = show

instance DecodeJson {name} where
  decodeJson = decodeJson >=> case _ of
    {decode_json}
    s -> Left $ TypeMismatch $ "Not a {name}: " <> s

instance EncodeJson {name} where
  encodeJson = show >>> encodeJson

instance DecodeHasura {name} where
  decodeHasura = decodeJson

instance EncodeHasura {name} where
  encodeHasura = encodeJson

instance Enum {name} where
  succ = unwrap >>>
    ( case_
        {succ_values}
    )
  pred = unwrap >>>
    ( case_
        {pred_values}
    )

instance Bounded {name} where
  bottom = {first_fn}
  top = {last_fn}

instance BoundedEnum {name} where
  cardinality = Cardinality {cardinality}
  toEnum a = case a of
    {to_enum}
    _ -> Nothing
  fromEnum = unwrap >>>
    ( case_
        {from_enum}
    )
"#
    ));

    Some(format!("module {module_name} where\n\n{VARIANT_MODULE_IMPORTS}{helper_import}\n\n{variant}\n\n{variant_fns}{instances}"))
}

fn to_variant(type_name: &str, name: &str) -> String {
    let fn_name = camel_case(name);
    format!(
        r#"{fn_name} = var @"{name}" :: {type_name}
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

import Data.Argonaut.Decode (class DecodeJson, JsonDecodeError(..), decodeJson)
import Data.Argonaut.Encode (class EncodeJson, encodeJson)
import Data.Either (Either(..))
import Data.Enum (class Enum, class BoundedEnum, Cardinality(..))
import Data.Function (on)
import Data.Maybe (Maybe(..))
import Data.Newtype (class Newtype, unwrap)
import Data.Symbol (reflectSymbol)
import Data.Variant (Unvariant(..), Variant, case_, unvariant)
import GraphQL.Client.ToGqlString (class GqlArgString)
import GraphQL.Hasura.Decode (class DecodeHasura)
import GraphQL.Hasura.Encode (class EncodeHasura)
import OaMakeFixture (class MakeFixture)"#;

const VARIANT_HELPERS_MOD: &str = r#"
import Prelude

import Data.Maybe (Maybe)
import Data.Newtype (class Newtype, wrap, unwrap)
import Data.Symbol (class IsSymbol)
import Data.Variant (Variant, inj)
import Data.Variant as V
import Data.Variant.Internal (class VariantTags)
import Prim.Row as R
import Prim.RowList as RL
import Type.Proxy (Proxy(..))

var :: ∀ @sym r1 r2 t. R.Cons sym Unit r1 r2 ⇒ IsSymbol sym => Newtype t (Variant r2) => t
var = wrap $ inj (Proxy @sym) unit

match
  :: ∀ @sym a r1 r2 b
   . R.Cons sym a r1 r2
  => IsSymbol sym
  => b
  → (Variant r1 → b)
  → Variant r2
  → b
match b = V.on (Proxy @sym) (\_ -> b)

-- | Expand a newtyped variant into a target newtyped variant.
-- | Requires the input variant be a sub-variant of the target variant.
expand
  :: forall t1 t2 v1 r_ v2
   . Newtype t1 (Variant v1)
  => R.Union v1 r_ v2
  => Newtype t2 (Variant v2)
  => t1
  -> t2
expand = unwrap >>> V.expand >>> wrap

-- | Contract one newtyped variant into another.
-- | Will return Nothing if the variant is not a variant of the target type.
contract
  :: forall t1 v1 v2 t2 rl r_
   . Newtype t1 (Variant v1)
  => RL.RowToList v2 rl
  => R.Union v2 r_ v1
  => VariantTags rl
  => Newtype t2 (Variant v2)
  => t1
  -> Maybe t2
contract = unwrap >>> V.contract >>> map wrap"#;
