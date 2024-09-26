use super::purescript_record::PurescriptRecord;
pub enum Argument {
    Type(String, Vec<Argument>),
    Function(Box<PurescriptFunctionType>),
    Record(PurescriptRecord),
}

pub struct PurescriptFunctionType {
    pub arguments: Vec<Argument>,
    pub return_type: Argument,
}

impl PurescriptFunctionType {
    pub fn to_string(&self) -> String {
        if self.arguments.len() == 1 {
            format!(
                "{} -> {}",
                self.arguments
                    .iter()
                    .map(|arg| arg.to_string_nestable(true))
                    .collect::<String>(),
                self.return_type.to_string_nestable(true)
            )
        } else {
            format!(
                "\n    {}\n    -> {}",
                self.arguments
                    .iter()
                    .map(|arg| arg.to_string_nestable(true))
                    .collect::<Vec<String>>()
                    .join("\n -> "),
                self.return_type.to_string_nestable(true)
            )
        }
    }
}

fn format_args(name: &String, args: &Vec<Argument>) -> String {
    format!(
        "{name} {}",
        args.iter()
            .map(|arg| arg.to_string_nestable(false))
            .collect::<Vec<String>>()
            .join(" ")
    )
    .trim()
    .to_string()
}

fn format_args_wrapped(name: &String, args: &Vec<Argument>) -> String {
    format!(
        "({name} {})",
        args.iter()
            .map(|arg| arg.to_string_nestable(false))
            .collect::<Vec<String>>()
            .join(" ")
    )
    .trim()
    .to_string()
}

impl Argument {
    pub fn new_type(name: &str) -> Self {
        Argument::Type(name.to_string(), vec![])
    }
    pub fn new_record(record: PurescriptRecord) -> Self {
        Argument::Record(record)
    }
    pub fn new_function(arguments: Vec<Argument>, return_type: Argument) -> Self {
        Argument::Function(Box::new(PurescriptFunctionType {
            arguments,
            return_type,
        }))
    }
    pub fn with_argument(mut self, arg: Argument) -> Self {
        match &mut self {
            Argument::Type(_, args) => args.push(arg),
            Argument::Function(args) => args.arguments.push(arg),
            Argument::Record(_) => panic!("Can't add arguments to a record"),
        }
        self
    }
    pub fn add_argument(&mut self, arg: Argument) {
        match self {
            Argument::Type(_, args) => args.push(arg),
            Argument::Function(args) => args.arguments.push(arg),
            Argument::Record(_) => panic!("Can't add arguments to a record"),
        };
    }

    /// Top level arguments won't be wrapped in parentheses
    pub fn to_string(&self) -> String {
        self.to_string_nestable(true)
    }
    /// Nested arguments will be wrapped in parentheses
    fn to_string_nestable(&self, top_level: bool) -> String {
        match self {
            Argument::Type(name, args) if !top_level && args.len() > 0 => {
                format_args_wrapped(name, args).trim().to_string()
            }
            Argument::Type(name, args) => format_args(name, args).trim().to_string(),
            Argument::Function(function) if !top_level => {
                format!("({})", function.to_string())
            }
            Argument::Function(function) => function.to_string(),
            Argument::Record(record) => record.to_type_string(),
        }
    }
    /// Pulls the names of all forall types out of the argument
    /// to allow checks on type arguments
    pub fn get_all_forall_types(&self) -> Vec<String> {
        match self {
            Argument::Type(_, args) => args
                .iter()
                .flat_map(|arg| arg.get_all_forall_types())
                .collect(),
            Argument::Function(function) => {
                let mut types: Vec<String> = vec![];
                for arg in &function.arguments {
                    types.extend(arg.get_all_forall_types());
                }
                types.extend(function.return_type.get_all_forall_types());
                types
            }
            Argument::Record(record) => record
                .fields
                .iter()
                .map(|field| field.type_name.get_all_forall_types())
                .flatten()
                .collect(),
        }
    }
}
