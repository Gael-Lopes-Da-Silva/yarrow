pub struct Command {
    pub name: String,
    pub description: String,
    pub commands: Vec<Command>,
}

impl Command {
    pub fn new(name: String, description: String, commands: Vec<Command>) -> Self {
        Self {
            name,
            description,
            commands,
        }
    }
}
