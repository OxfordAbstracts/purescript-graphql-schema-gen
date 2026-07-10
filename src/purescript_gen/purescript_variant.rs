use super::purescript_row::Row;

#[derive(Debug, Clone)]
pub struct Variant {
    name: String,
    row: Row,
}

impl Variant {
    pub fn new(name: &str) -> Self {
        Variant {
            name: name.to_string(),
            row: Row::new(),
        }
    }

    pub fn with_values(&mut self, values: &Vec<String>) -> &mut Self {
        self.row.with_unit_values(values);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn to_string(&self) -> String {
        let values = self
            .row
            .to_string();
        format!("type {} = Variant\n  {values}", self.name)
    }
}
