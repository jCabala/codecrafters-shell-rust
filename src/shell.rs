use std::sync::{Arc, Mutex};
use rustyline::{Editor, Config, CompletionType, error::ReadlineError};
use rustyline::history::DefaultHistory;
use crate::autocomplete::Autocomplete;
use crate::command::executor::execute_pipeline;
use crate::command::parser::parse_pipeline;
use crate::shell_state::{
    build_executables, BackgroundJobRegistry, CompletionRegistry,
    ExecutableMap, History, Variables,
};

pub struct Shell {
    executables: Arc<ExecutableMap>,
    bg_jobs:     Arc<Mutex<BackgroundJobRegistry>>,
    history:     Arc<Mutex<History>>,
    variables:   Arc<Mutex<Variables>>,
    completions: Arc<Mutex<CompletionRegistry>>,
    editor:      Editor<Autocomplete, DefaultHistory>,
}

impl Shell {
    pub fn new() -> Self {
        let executables = Arc::new(build_executables());
        let bg_jobs     = Arc::new(Mutex::new(BackgroundJobRegistry::new()));
        let history     = Arc::new(Mutex::new(History::new()));
        let variables   = Arc::new(Mutex::new(Variables::new()));
        let completions = Arc::new(Mutex::new(CompletionRegistry::new()));

        if let Ok(path) = std::env::var("HISTFILE") {
            let mut h = history.lock().unwrap();
            let _ = h.read_from_file(&path);
            h.mark_all_appended();
        }

        let config = Config::builder().completion_type(CompletionType::List).build();
        let mut editor = Editor::with_config(config).expect("Failed to create line editor");
        editor.set_helper(Some(Autocomplete::new(Arc::clone(&executables), Arc::clone(&completions))));

        Self { executables, bg_jobs, history, variables, completions, editor }
    }

    pub fn run(&mut self) {
        while !self.step() {}
        if let Ok(path) = std::env::var("HISTFILE") {
            let _ = self.history.lock().unwrap().append_to_file(&path);
        }
    }

    fn step(&mut self) -> bool {
        for (id, marker, cmd) in self.bg_jobs.lock().unwrap().drain_completed() {
            eprintln!("[{}]{}  {:<24}{}", id, marker, "Done", cmd);
        }

        let input = match self.editor.readline("$ ") {
            Ok(line) => { self.editor.add_history_entry(&line).ok(); line }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => return true,
            Err(e) => { eprintln!("shell: {}", e); return true; }
        };

        self.history.lock().unwrap().push(input.clone());

        let pipeline = {
            let vars = self.variables.lock().unwrap();
            parse_pipeline(&input, &self.executables, &vars)
        };
        let Some(pipeline) = pipeline else { return false; };

        execute_pipeline(
            pipeline,
            Arc::clone(&self.executables),
            Arc::clone(&self.bg_jobs),
            Arc::clone(&self.history),
            Arc::clone(&self.variables),
            Arc::clone(&self.completions),
        )
    }
}
