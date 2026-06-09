use std::sync::{Arc, Mutex};
use rustyline::{Helper, Hinter, Highlighter, Validator, Context};
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use crate::builtins::builtin_commands;
use crate::completions::CompletionRegistry;
use crate::executables::ExecutableMap;

#[derive(Helper, Hinter, Highlighter, Validator)]
pub struct Autocomplete {
    pub executables: Arc<ExecutableMap>,
    completions: Arc<Mutex<CompletionRegistry>>,
    file_completer: FilenameCompleter,
}

impl Autocomplete {
    pub fn new(executables: Arc<ExecutableMap>, completions: Arc<Mutex<CompletionRegistry>>) -> Self {
        Self { executables, completions, file_completer: FilenameCompleter::new() }
    }
}

impl Autocomplete {
    fn complete_with_script(&self, command: &str, current_word: &str, prev_word: &str, comp_line: &str, comp_point: usize) -> Option<Vec<Pair>> {
        let script = self.completions.lock().unwrap().get(command).cloned()?;
        let output = std::process::Command::new(&script)
            .args([command, current_word, prev_word])
            .env("COMP_LINE", comp_line)
            .env("COMP_POINT", comp_point.to_string())
            .output()
            .ok()?;
        let candidates: Vec<Pair> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|l| Pair { display: l.to_string(), replacement: format!("{} ", l) })
            .collect();
        if candidates.is_empty() { None } else { Some(candidates) }
    }
}

impl Completer for Autocomplete {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        let word = &line[..pos];
        if word.contains(' ') {
            let command = word.split_whitespace().next().unwrap_or("");
            let word_start = word.rfind(' ').map(|i| i + 1).unwrap_or(0);
            let current_word = &word[word_start..];
            let prev_word = word[..word_start].trim().split_whitespace().last().unwrap_or(command);

            if let Some(candidates) = self.complete_with_script(command, current_word, prev_word, word, pos) {
                return Ok((word_start, candidates));
            }

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
