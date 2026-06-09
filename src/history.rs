pub struct History {
    entries: Vec<String>,
}

impl History {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn push(&mut self, entry: String) {
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}
