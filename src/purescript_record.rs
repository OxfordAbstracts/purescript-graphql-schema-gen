use crate::purescript_argument::Argument;

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
    pub fn for_all(mut self, type_name: &str) -> Self {
        self.type_name = Argument::new_for_all(type_name);
        self
    }
    pub fn add_argument(mut self, arg: Argument) -> Self {
        match &mut self.type_name {
            Argument::Type(_, args) => args.push(arg),
            Argument::ForAll(_, args) => args.push(arg),
            _ => panic!("Cannot add argument to non-type field"),
        }
        self
    }
    pub fn maybe(self, type_name: &str) -> Self {
        let s = self
            .with_type("Maybe")
            .add_argument(Argument::new_type(type_name));
        s
    }
    pub fn maybe_for_all(self, type_name: &str) -> Self {
        let s = self
            .with_type("Maybe")
            .add_argument(Argument::new_for_all(type_name));
        s
    }
    pub fn maybe_arg(self, arg: Argument) -> Self {
        let s = self.with_type("Maybe").add_argument(arg);
        s
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
    pub fn add_argument(&mut self, arg: Argument) -> &mut Self {
        self.arguments.push(arg);
        self
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
            if self.fields.iter().any(|f| f.name == field.name) {
                return Some(format!("Field with name '{}' already exists", field.name));
            }
            if to_add.iter().filter(|&n| n == &field.name).count() > 1 {
                return Some(format!(
                    "Cannot add multiple fields with the same name: '{}'",
                    field.name
                ));
            }
            if field
                .type_name
                .get_all_forall_types()
                .iter()
                .any(|t| !all_forall_args.contains(t))
            {
                return Some(format!(
                    "Field '{}' uses a forall type '{}' that is not defined in the record arguments",
                    field.name,
                    field.type_name.to_string()
                ));
            }
        }
        None
    }
    pub fn add_field(&mut self, field: Field) -> &mut Self {
        if let Some(err) = self.validate_fields(vec![&field]) {
            panic!("{}", err);
        }

        self.fields.push(field);
        self
    }
    pub fn add_arguments(&mut self, args: Vec<Argument>) -> &mut Self {
        self.arguments.extend(args);
        self
    }
    pub fn add_fields(&mut self, fields: Vec<Field>) -> &mut Self {
        if let Some(err) = self.validate_fields(fields.iter().collect()) {
            panic!("{}", err);
        }

        self.fields.extend(fields);
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
            x => &format!(" {}", x),
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
                return format!("type {}{} = {{ {} }}", self.name, arguments, fields);
            }
            format!("type {}{} =\n  {{ {}\n  }}", self.name, arguments, fields)
        } else {
            if self.fields.len() == 0 {
                return "{}".to_string();
            }
            if self.fields.len() == 1 {
                return format!("{{ {} }}", fields);
            }
            format!("{{ {}\n  }}", fields)
        }
    }

    pub fn to_type_string(&self) -> String {
        self.to_string_opts(false)
    }

    pub fn to_string(&self) -> String {
        self.to_string_opts(true)
    }
}
