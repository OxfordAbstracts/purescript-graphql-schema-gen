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

    pub fn add_value(&mut self, value: &str) {
        self.values.push(value.to_string());
    }

    pub fn to_string(&self) -> String {
        format!("data {}\n  = {}", self.name, self.values.join("\n  | "))
    }
}