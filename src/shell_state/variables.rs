use rustc_hash::FxHashMap;

pub struct Variables {
    store: FxHashMap<String, String>,
}

impl Variables {
    pub fn new() -> Self {
        Self { store: FxHashMap::default() }
    }

    pub fn get(&self, name: &str) -> Option<&String> {
        self.store.get(name)
    }

    pub fn set(&mut self, name: String, value: String) {
        self.store.insert(name, value);
    }
}
