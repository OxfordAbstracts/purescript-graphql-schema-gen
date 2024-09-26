use super::purescript_argument::Argument;

pub struct PurescriptRecord {
    pub name: String,
    arguments: Vec<Argument>,
    pub fields: Vec<Field>,
}

pub struct Field {
    name: String,
    pub type_name: Argument,
}

impl Field {
    pub fn new(name: &str) -> Self {
        Field {
            name: name.to_string(),
            type_name: Argument::new_type("String"),
        }
    }
    pub fn with_type(mut self, type_name: &str) -> Self {
        self.type_name = Argument::new_type(type_name);
        self
    }
    pub fn with_type_arg(mut self, type_name: Argument) -> Self {
        self.type_name = type_name;
        self
    }
    pub fn show_field(&self) -> String {
        // if first character is uppercase, wrap in quotes
        if self.name.chars().next().unwrap().is_uppercase() {
            format!("\"{}\"", self.name)
        } else {
            self.name.clone()
        }
    }
}

impl PurescriptRecord {
    pub fn new(name: &str) -> Self {
        PurescriptRecord {
            name: name.to_string(),
            arguments: vec![],
            fields: vec![],
        }
    }
    fn validate_fields(&self, fields: Vec<&Field>) -> Option<String> {
        let to_add = fields
            .iter()
            .map(|f| f.name.clone())
            .collect::<Vec<String>>();
        let all_forall_args = self
            .arguments
            .iter()
            .flat_map(|arg| arg.get_all_forall_types())
            .collect::<Vec<String>>();
        for field in fields {
            let name = &field.name;
            if self.fields.iter().any(|f| &f.name == name) {
                return Some(format!("Field with name '{name}' already exists"));
            }
            if to_add.iter().filter(|&n| n == &field.name).count() > 1 {
                return Some(format!(
                    "Cannot add multiple fields with the same name: '{name}'"
                ));
            }
            if field
                .type_name
                .get_all_forall_types()
                .iter()
                .any(|t| !all_forall_args.contains(t))
            {
                return Some(format!(
                    "Field '{name}' uses a forall type '{}' that is not defined in the record arguments",
                    field.type_name.to_string()
                ));
            }
        }
        None
    }
    pub fn add_field(&mut self, field: Field) -> &mut Self {
        if let Some(err) = self.validate_fields(vec![&field]) {
            panic!("{err}");
        }

        self.fields.push(field);
        self
    }

    pub fn has_field(&self, name: &str) -> bool {
        for field in &self.fields {
            if field.name == name {
                return true;
            }
        }
        false
    }

    fn to_string_opts(&self, with_type: bool) -> String {
        let arguments = match self
            .arguments
            .iter()
            .map(|arg| arg.to_string())
            .collect::<Vec<String>>()
            .join(" ")
            .as_str()
        {
            "" => "",
            x => &format!(" {x}"),
        };
        let fields = self
            .fields
            .iter()
            .map(|field| format!("{} :: {}", &field.show_field(), field.type_name.to_string()))
            .collect::<Vec<String>>()
            .join("\n  , ");

        if with_type {
            if self.fields.len() == 0 {
                return format!("type {}{} = {{}}", self.name, arguments);
            }
            if self.fields.len() == 1 {
                return format!("type {}{} = {{ {fields} }}", self.name, arguments);
            }
            format!("type {}{} =\n  {{ {fields}\n  }}", self.name, arguments)
        } else {
            if self.fields.len() == 0 {
                return "{}".to_string();
            }
            if self.fields.len() == 1 {
                return format!("{{ {fields} }}");
            }
            format!("{{ {fields}\n  }}")
        }
    }

    pub fn to_type_string(&self) -> String {
        self.to_string_opts(false)
    }

    pub fn to_string(&self) -> String {
        self.to_string_opts(true)
    }
}
