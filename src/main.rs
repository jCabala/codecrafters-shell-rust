use std::sync::{Arc, Mutex};
use rustyline::{Editor, Config, CompletionType, error::ReadlineError};
use rustyline::history::DefaultHistory;

mod autocomplete;
mod bg_jobs;
mod builtins;
mod command;
mod command_executor;
mod command_parser;

use autocomplete::Autocomplete;
use bg_jobs::BackgroundJobRegistry;
use command::Streams;
use command_executor::execute;
use command_parser::{build_executables, parse_command};

fn main() {
    let executables = Arc::new(build_executables());
    let bg_jobs = Arc::new(Mutex::new(BackgroundJobRegistry::new()));
    let config = Config::builder().completion_type(CompletionType::List).build();
    let mut editor: Editor<Autocomplete, DefaultHistory> = Editor::with_config(config).expect("Failed to create line editor");
    editor.set_helper(Some(Autocomplete::new(Arc::clone(&executables))));
    
    loop {
        for (id, name) in bg_jobs.lock().unwrap().drain_completed() {
            eprintln!("[{}]+  Done    {}", id, name);
        }

        let input = match editor.readline("$ ") {
            Ok(line) => { editor.add_history_entry(&line).ok(); line }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(e) => { eprintln!("shell: {}", e); break; }
        };

        let command = parse_command(&input, &executables);
        if command.name.is_empty() {
            continue;
        }

        let streams = Streams::from_redirects(&command.redirects);
        if execute(command, streams, Arc::clone(&executables), Arc::clone(&bg_jobs)) {
            break;
        }
    }
}
