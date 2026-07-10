use stringcase::pascal_case;

pub fn upper_first(str: &str) -> String {
    let mut c = str.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Convert a GraphQL type name to a PureScript type name.
/// With `pascal_case_types` set, `users_insert_input` becomes
/// `UsersInsertInput`; otherwise only the first letter is uppercased,
/// giving `Users_insert_input`.
pub fn type_name(name: &str, pascal_case_types: bool) -> String {
    if pascal_case_types {
        pascal_case(name)
    } else {
        upper_first(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_name_respects_casing_config() {
        assert_eq!(type_name("users_insert_input", true), "UsersInsertInput");
        assert_eq!(type_name("users_insert_input", false), "Users_insert_input");
        // Names that are already PascalCase are left intact either way,
        // so shared-enum suffix matching (e.g. "Enum") keeps working.
        assert_eq!(type_name("QuestionTypesEnum", true), "QuestionTypesEnum");
        assert_eq!(type_name("QuestionTypesEnum", false), "QuestionTypesEnum");
        assert_eq!(
            type_name("dr_attendee_status_enum", true),
            "DrAttendeeStatusEnum"
        );
    }
}
