pub struct Enum {
    name: String,
    values: Vec<String>,
}

impl Enum {
    pub fn new(name: &str) -> Self {
        Enum {
            name: name.to_string(),
            values: vec![],
        }
    }

    pub fn with_values(&mut self, values: &Vec<String>) -> &Self {
        self.values = values.clone();
        self
    }

    pub fn to_string(&self) -> String {
        let Self { name, values } = self;
        format!("data {name}\n  = {}", values.join("\n  | "))
    }
}
