use std::io::Write;
use std::sync::{Arc, Mutex};
use crate::bg_jobs::BackgroundJobRegistry;
use crate::completions::CompletionRegistry;
use crate::executables::ExecutableMap;
use crate::history::History;
use crate::variables::Variables;

// ── Builtin registry ───────────────────────────────────────────────────────────

pub fn builtin_commands() -> Vec<&'static str> {
    vec!["exit", "echo", "type", "pwd", "cd", "jobs", "history", "declare", "complete"]
}

pub fn is_builtin_command(name: &str) -> bool {
    builtin_commands().contains(&name)
}

// ── Context ────────────────────────────────────────────────────────────────────

pub struct BuiltinContext<'a> {
    pub out:         &'a mut dyn Write,
    pub err:         &'a mut dyn Write,
    pub executables: &'a ExecutableMap,
    pub bg_jobs:     &'a Arc<Mutex<BackgroundJobRegistry>>,
    pub history:     &'a Arc<Mutex<History>>,
    pub variables:   &'a Arc<Mutex<Variables>>,
    pub completions: &'a Arc<Mutex<CompletionRegistry>>,
}

// ── Builtin structs ────────────────────────────────────────────────────────────

struct ExitBuiltin;
impl ExitBuiltin {
    fn parse(_: &[String]) -> Self { Self }
    fn run(self, _: &mut BuiltinContext) -> bool { true }
}

struct EchoBuiltin { words: Vec<String> }
impl EchoBuiltin {
    fn parse(args: &[String]) -> Self { Self { words: args.to_vec() } }
    fn run(self, ctx: &mut BuiltinContext) -> bool {
        writeln!(ctx.out, "{}", self.words.join(" ")).unwrap();
        false
    }
}

struct TypeBuiltin { names: Vec<String> }
impl TypeBuiltin {
    fn parse(args: &[String]) -> Self { Self { names: args.to_vec() } }
    fn run(self, ctx: &mut BuiltinContext) -> bool {
        for n in &self.names {
            if is_builtin_command(n) {
                writeln!(ctx.out, "{} is a shell builtin", n).unwrap();
            } else if let Some(path) = ctx.executables.get(n.as_str()) {
                writeln!(ctx.out, "{} is {}", n, path).unwrap();
            } else {
                writeln!(ctx.err, "{}: not found", n).unwrap();
            }
        }
        false
    }
}

struct PwdBuiltin;
impl PwdBuiltin {
    fn parse(_: &[String]) -> Self { Self }
    fn run(self, ctx: &mut BuiltinContext) -> bool {
        match std::env::current_dir() {
            Ok(path) => writeln!(ctx.out, "{}", path.display()).unwrap(),
            Err(_)   => writeln!(ctx.err, "pwd: error getting current directory").unwrap(),
        }
        false
    }
}

struct CdBuiltin { path: Option<String> }
impl CdBuiltin {
    fn parse(args: &[String]) -> Self {
        Self { path: args.first().filter(|p| p.as_str() != "~").cloned() }
    }
    fn run(self, ctx: &mut BuiltinContext) -> bool {
        let home;
        let path: &str = match self.path.as_deref() {
            Some(p) => p,
            None => { home = std::env::var("HOME").unwrap_or_default(); &home }
        };
        if let Err(_) = std::env::set_current_dir(path) {
            writeln!(ctx.err, "cd: {}: No such file or directory", path).unwrap();
        }
        false
    }
}

struct JobsBuiltin;
impl JobsBuiltin {
    fn parse(_: &[String]) -> Self { Self }
    fn run(self, ctx: &mut BuiltinContext) -> bool {
        ctx.bg_jobs.lock().unwrap().list_jobs(ctx.out);
        false
    }
}

enum HistoryBuiltin {
    Print { limit: Option<usize> },
    ReadFile(String),
    WriteFile(String),
    AppendFile(String),
    MissingFilename(char),
}
impl HistoryBuiltin {
    fn parse(args: &[String]) -> Self {
        match args.first().map(|s| s.as_str()) {
            Some(flag @ ("-r" | "-w" | "-a")) => {
                let ch = flag.chars().nth(1).unwrap();
                match args.get(1) {
                    Some(path) => match ch {
                        'r' => Self::ReadFile(path.clone()),
                        'w' => Self::WriteFile(path.clone()),
                        _   => Self::AppendFile(path.clone()),
                    },
                    None => Self::MissingFilename(ch),
                }
            }
            _ => Self::Print { limit: args.first().and_then(|a| a.parse().ok()) },
        }
    }
    fn run(self, ctx: &mut BuiltinContext) -> bool {
        match self {
            Self::Print { limit } =>
                ctx.history.lock().unwrap().write_to(ctx.out, limit).unwrap(),
            Self::ReadFile(path) =>
                if let Err(e) = ctx.history.lock().unwrap().read_from_file(&path) {
                    writeln!(ctx.err, "history: {}: {}", path, e).unwrap();
                },
            Self::WriteFile(path) =>
                if let Err(e) = ctx.history.lock().unwrap().write_to_file(&path) {
                    writeln!(ctx.err, "history: {}: {}", path, e).unwrap();
                },
            Self::AppendFile(path) =>
                if let Err(e) = ctx.history.lock().unwrap().append_to_file(&path) {
                    writeln!(ctx.err, "history: {}: {}", path, e).unwrap();
                },
            Self::MissingFilename(flag) =>
                writeln!(ctx.err, "history: -{}: missing filename", flag).unwrap(),
        }
        false
    }
}

fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {},
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

enum DeclareBuiltin {
    Set { name: String, value: String },
    InvalidIdentifier(String),
    Print(Vec<String>),
    UnknownFlag(String),
}
impl DeclareBuiltin {
    fn parse(args: &[String]) -> Self {
        match args.first().map(|s| s.as_str()) {
            Some("-p")                          => Self::Print(args[1..].to_vec()),
            Some(flag) if flag.starts_with('-') => Self::UnknownFlag(flag.to_string()),
            Some(assignment) => match assignment.split_once('=') {
                Some((name, value)) if is_valid_identifier(name) =>
                    Self::Set { name: name.to_string(), value: value.to_string() },
                Some(_) => Self::InvalidIdentifier(assignment.to_string()),
                None    => Self::UnknownFlag(assignment.to_string()),
            },
            None => Self::Print(vec![]),
        }
    }
    fn run(self, ctx: &mut BuiltinContext) -> bool {
        match self {
            Self::Set { name, value } =>
                ctx.variables.lock().unwrap().set(name, value),
            Self::InvalidIdentifier(assignment) =>
                writeln!(ctx.err, "declare: `{}': not a valid identifier", assignment).unwrap(),
            Self::Print(names) => {
                for name in &names {
                    match ctx.variables.lock().unwrap().get(name) {
                        Some(val) => writeln!(ctx.out, "declare -- {}=\"{}\"", name, val).unwrap(),
                        None      => writeln!(ctx.err, "bash: declare: {}: not found", name).unwrap(),
                    }
                }
            }
            Self::UnknownFlag(flag) =>
                writeln!(ctx.err, "declare: {}: invalid option", flag).unwrap(),
        }
        false
    }
}

enum CompleteBuiltin {
    Register { script: String, command: String },
    InvalidUsage,
}
impl CompleteBuiltin {
    fn parse(args: &[String]) -> Self {
        if args.get(0).map(|s| s.as_str()) == Some("-C") {
            match (args.get(1), args.get(2)) {
                (Some(script), Some(command)) =>
                    Self::Register { script: script.clone(), command: command.clone() },
                _ => Self::InvalidUsage,
            }
        } else {
            Self::InvalidUsage
        }
    }
    fn run(self, ctx: &mut BuiltinContext) -> bool {
        match self {
            Self::Register { script, command } =>
                ctx.completions.lock().unwrap().register(command, script),
            Self::InvalidUsage =>
                writeln!(ctx.err, "complete: usage: complete -C script command").unwrap(),
        }
        false
    }
}

// ── Dispatch ───────────────────────────────────────────────────────────────────

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
    match name {
        "exit"     => ExitBuiltin::parse(args).run(&mut ctx),
        "echo"     => EchoBuiltin::parse(args).run(&mut ctx),
        "type"     => TypeBuiltin::parse(args).run(&mut ctx),
        "pwd"      => PwdBuiltin::parse(args).run(&mut ctx),
        "cd"       => CdBuiltin::parse(args).run(&mut ctx),
        "jobs"     => JobsBuiltin::parse(args).run(&mut ctx),
        "history"  => HistoryBuiltin::parse(args).run(&mut ctx),
        "declare"  => DeclareBuiltin::parse(args).run(&mut ctx),
        "complete" => CompleteBuiltin::parse(args).run(&mut ctx),
        _          => { writeln!(ctx.err, "panic: unknown builtin '{}'", name).unwrap(); false }
    }
}
