use std::io::{self, Write};

pub struct History {
    entries: Vec<String>,
    last_appended: usize,
}

impl History {
    pub fn new() -> Self {
        Self { entries: Vec::new(), last_appended: 0 }
    }

    pub fn push(&mut self, entry: String) {
        self.entries.push(entry);
    }

    pub fn write_to_file(&self, path: &str) -> io::Result<()> {
        let mut file = std::fs::File::create(path)?;
        for entry in &self.entries {
            writeln!(file, "{}", entry)?;
        }
        Ok(())
    }

    pub fn mark_all_appended(&mut self) {
        self.last_appended = self.entries.len();
    }

    pub fn append_to_file(&mut self, path: &str) -> io::Result<()> {
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        for entry in &self.entries[self.last_appended..] {
            writeln!(file, "{}", entry)?;
        }
        self.last_appended = self.entries.len();
        Ok(())
    }

    pub fn read_from_file(&mut self, path: &str) -> io::Result<()> {
        let contents = std::fs::read_to_string(path)?;
        for line in contents.lines().filter(|l| !l.is_empty()) {
            self.entries.push(line.to_string());
        }
        Ok(())
    }

    pub fn write_to(&self, out: &mut dyn Write, limit: Option<usize>) -> io::Result<()> {
        let start = limit
            .map(|n| self.entries.len().saturating_sub(n))
            .unwrap_or(0);
        for (i, entry) in self.entries[start..].iter().enumerate() {
            writeln!(out, "{:5}  {}", start + i + 1, entry)?;
        }
        Ok(())
    }
}
