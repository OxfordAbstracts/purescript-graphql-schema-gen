
pub fn upper_first(str: &str) -> String { 
    let mut c = str.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}