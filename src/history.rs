use std::io::{self, Write};

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
