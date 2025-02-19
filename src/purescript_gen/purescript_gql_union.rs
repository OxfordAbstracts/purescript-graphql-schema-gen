use super::purescript_row::Row;

#[derive(Debug, Clone)]
pub struct GqlUnion {
    name: String,
    row: Row,
}

impl GqlUnion {
    pub fn new(name: &str) -> Self {
        GqlUnion {
            name: name.to_string(),
            row: Row::new(),
        }
    }

    pub fn with_values(&mut self, values: &Vec<(String, String)>) -> &mut Self {
        self.row.with_values(values);
        self
    }

    pub fn to_string(&self) -> String {
        let values = self
            .row
            .to_string();
        format!("type {} = GqlUnion\n  {values}", self.name)
    }
}
