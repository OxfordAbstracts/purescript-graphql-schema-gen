use crate::purescript_argument::Argument;

pub struct PurescriptType {
    pub name: String,
    arguments: Vec<String>,
    pub value: Argument,
    newtype: bool,
}

impl PurescriptType {
    pub fn new(name: &str, arguments: Vec<&str>, value: Argument) -> Self {
        for forall in value.get_all_forall_types() {
            if !arguments.contains(&forall.as_str()) {
                panic!(
                    "There's a forall type in the body of type '{}' that doesn't appear in the head: '{}'",
                    name, forall
                );
            }
        }

        PurescriptType {
            name: name.to_string(),
            arguments: arguments.iter().map(|a| a.to_string()).collect(),
            value,
            newtype: false,
        }
    }

    pub fn set_newtype(&mut self, newtype: bool) {
        self.newtype = newtype;
    }

    pub fn set_value(&mut self, value: Argument) {
        self.value = value;
    }

    pub fn to_string(&self) -> String {
        let args = match self.arguments.len() {
            0 => "".to_string(),
            _ => format!(
                " {}",
                self.arguments
                    .iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
        };
        if self.newtype {
            format!(
                "newtype {}{} = {}\n  {}",
                self.name,
                args,
                self.name,
                self.value.to_string()
            )
        } else {
            format!("type {}{} = {}", self.name, args, self.value.to_string())
        }
    }
}
