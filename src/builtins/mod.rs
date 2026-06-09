use std::io::Write;
use std::sync::{Arc, Mutex};
use crate::shell_state::BackgroundJobRegistry;
use crate::shell_state::CompletionRegistry;
use crate::shell_state::ExecutableMap;
use crate::shell_state::History;
use crate::shell_state::Variables;

mod commands;
mod registry;


pub fn builtin_commands() -> Vec<&'static str> {
    registry::names()
}

pub fn is_builtin_command(name: &str) -> bool {
    registry::contains(name)
}

pub struct BuiltinContext<'a> {
    pub out:         &'a mut dyn Write,
    pub err:         &'a mut dyn Write,
    pub executables: &'a ExecutableMap,
    pub bg_jobs:     &'a Arc<Mutex<BackgroundJobRegistry>>,
    pub history:     &'a Arc<Mutex<History>>,
    pub variables:   &'a Arc<Mutex<Variables>>,
    pub completions: &'a Arc<Mutex<CompletionRegistry>>,
}

pub fn run_builtin(
    name: &str,
    args: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
    executables: &ExecutableMap,
    bg_jobs: &Arc<Mutex<BackgroundJobRegistry>>,
    history: &Arc<Mutex<History>>,
    variables: &Arc<Mutex<Variables>>,
    completions: &Arc<Mutex<CompletionRegistry>>,
) -> bool {
    let mut ctx = BuiltinContext { out, err, executables, bg_jobs, history, variables, completions };
    match registry::get(name) {
        Some(handler) => handler(args, &mut ctx),
        None => { writeln!(ctx.err, "panic: unknown builtin '{}'", name).unwrap(); false }
    }
}
