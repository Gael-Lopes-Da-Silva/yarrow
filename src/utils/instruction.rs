#[derive(Debug)]
pub struct Instruction {
    pub name: String,
    pub content: Vec<Box<dyn std::any::Any>>,
    pub token: Vec<String>,
}

impl Instruction {
    pub fn new(name: String, content: Vec<Box<dyn std::any::Any>>, token: Vec<String>) -> Self {
        Instruction {
            name,
            content,
            token,
        }
    }
}
