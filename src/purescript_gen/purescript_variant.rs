pub struct Variant {
    name: String,
    values: Vec<String>,
}

impl Variant {
    pub fn new(name: &str) -> Self {
        Variant {
            name: name.to_string(),
            values: vec![],
        }
    }

    pub fn with_values(mut self, values: &Vec<String>) -> Self {
        self.values = values.clone();
        self
    }

    pub fn to_string(&self) -> String {
        let values = self
            .values
            .iter()
            .map(|v| format!("\"{}\" :: Unit", v))
            .collect::<Vec<String>>()
            .join("\n  , ");
        format!("type {} = Variant\n  ( {values}\n  )", self.name)
    }
}
