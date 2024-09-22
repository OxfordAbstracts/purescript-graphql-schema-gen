pub struct DeriveInstance {
    class: String,
    type_name: String,
    arguments: Vec<String>,
}

pub fn derive_new_type_instance(for_type: &str) -> DeriveInstance {
    DeriveInstance::new(for_type, "Newtype").with_argument("_")
}

impl DeriveInstance {
    pub fn new(name: &str, class: &str) -> Self {
        DeriveInstance {
            class: class.to_string(),
            type_name: name.to_string(),
            arguments: vec![],
        }
    }

    pub fn with_argument(mut self, arg: &str) -> Self {
        self.arguments.push(arg.to_string());
        self
    }

    pub fn to_string(&self) -> String {
        format!(
            "derive instance {} {} {}",
            self.class,
            self.type_name,
            self.arguments.join(" ")
        )
        .trim()
        .to_string()
    }
}
