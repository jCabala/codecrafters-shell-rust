use rustc_hash::FxHashMap;

pub struct CompletionRegistry {
    completers: FxHashMap<String, String>,
}

impl CompletionRegistry {
    pub fn new() -> Self { Self { completers: FxHashMap::default() } }

    pub fn register(&mut self, command: String, script: String) {
        self.completers.insert(command, script);
    }

    pub fn get(&self, command: &str) -> Option<&String> {
        self.completers.get(command)
    }
}
