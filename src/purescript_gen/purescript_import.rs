#[derive(Clone)]
pub struct PurescriptImport {
    pub module: String,
    specified: Vec<Specified>,
    pub as_name: Option<String>,
    pub package: String,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct Specified {
    import: String,
}

impl Specified {
    fn to_string(&self) -> String {
        self.import.clone()
    }
}

impl PurescriptImport {
    pub fn new(module: &str, package: &str) -> Self {
        PurescriptImport {
            module: module.to_string(),
            specified: vec![],
            as_name: None,
            package: package.to_string(),
        }
    }

    pub fn merge(imports: &Vec<PurescriptImport>) -> Vec<PurescriptImport> {
        let mut merged: Vec<PurescriptImport> = vec![];
        for import in imports {
            let mut found = false;
            {
                let im = &import;
                for m in merged.iter_mut() {
                    if m.module == im.module && (m.as_name.is_none() || m.as_name == im.as_name) {
                        m.specified.extend(im.specified.clone());
                        m.specified.sort();
                        m.specified.dedup();
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                merged.push(import.clone());
            }
        }
        merged.sort_by_cached_key(|i| i.module.clone());
        merged
    }

    pub fn add_specified(mut self, import: &str) -> Self {
        self.specified.push(Specified {
            import: import.to_string(),
        });
        self
    }

    pub fn to_string(&mut self) -> String {
        self.specified.sort_by_key(|s| s.to_string());
        let specified = self
            .specified
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        let specified = match specified.join(", ").as_str() {
            "" => "",
            s => &format!("({})", s).to_string(),
        };
        let as_name = match &self.as_name {
            Some(name) => format!("as {}", name),
            None => "".to_string(),
        };
        format!("import {} {}{}", self.module, specified, as_name)
            .trim()
            .to_string()
    }
}
