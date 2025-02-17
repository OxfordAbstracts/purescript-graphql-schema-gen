#[derive(Debug, Clone)]
pub struct Row {
    pub values: Vec<(String, String)>,
}

impl Row {
    pub fn new() -> Self {
        Row { values: vec![] }
    }

    pub fn with_values(&mut self, values: &Vec<(String, String)>) -> &mut Self {
        self.values = values.clone();
        self
    }
    pub fn with_unit_values(&mut self, values: &Vec<String>) -> &mut Self {
        self.values = values
            .into_iter()
            .map(|k| (k.clone(), "Unit".to_string()))
            .collect::<Vec<(String, String)>>();
        self
    }

    pub fn to_string(&self) -> String {
        let values = self
            .values
            .iter()
            .map(|(k, v): &(String, String)| format!("\"{}\" :: {}", k, v))
            .collect::<Vec<String>>()
            .join("\n  , ");
        format!("( {values}\n  )")
    }
}
