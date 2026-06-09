use std::sync::Arc;
use rustyline::{Helper, Hinter, Highlighter, Validator, Context};
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use crate::command_parser::builtin_commands;
use crate::executables::ExecutableMap;

#[derive(Helper, Hinter, Highlighter, Validator)]
pub struct Autocomplete {
    pub executables: Arc<ExecutableMap>,
    file_completer: FilenameCompleter,
}

impl Autocomplete {
    pub fn new(executables: Arc<ExecutableMap>) -> Self {
        Self { executables, file_completer: FilenameCompleter::new() }
    }
}

impl Completer for Autocomplete {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        let word = &line[..pos];
        if word.contains(' ') {
            let (start, candidates) = self.file_completer.complete(line, pos, _ctx)?;
            let candidates = candidates.into_iter().map(|p| {
                if p.replacement.ends_with('/') {
                    Pair { display: format!("{}/", p.display), replacement: p.replacement }
                } else {
                    Pair { display: p.display, replacement: format!("{} ", p.replacement) }
                }
            }).collect();
            return Ok((start, candidates));
        }
        let mut candidates: Vec<Pair> = builtin_commands()
            .iter()
            .filter(|cmd| cmd.starts_with(word))
            .map(|cmd| Pair { display: cmd.to_string(), replacement: format!("{} ", cmd) })
            .collect();
        candidates.extend(
            self.executables.keys()
                .filter(|name| name.starts_with(word))
                .map(|name| Pair { display: name.to_string(), replacement: format!("{} ", name) })
        );
        candidates.sort_by(|a, b| a.display.cmp(&b.display));
        candidates.dedup_by(|a, b| a.display == b.display);
        Ok((0, candidates))
    }
}
