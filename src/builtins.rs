use std::io::Write;
use std::sync::{Arc, Mutex};
use crate::bg_jobs::BackgroundJobRegistry;
use crate::executables::ExecutableMap;
use crate::history::History;
use crate::variables::Variables;

// ── Builtin registry ───────────────────────────────────────────────────────────

pub fn builtin_commands() -> Vec<&'static str> {
    vec!["exit", "echo", "type", "pwd", "cd", "jobs", "history", "declare"]
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

enum DeclareBuiltin {
    Print(Vec<String>),
    UnknownFlag(String),
}
impl DeclareBuiltin {
    fn parse(args: &[String]) -> Self {
        match args.first().map(|s| s.as_str()) {
            Some("-p") => Self::Print(args[1..].to_vec()),
            Some(flag) => Self::UnknownFlag(flag.to_string()),
            None       => Self::Print(vec![]),
        }
    }
    fn run(self, ctx: &mut BuiltinContext) -> bool {
        match self {
            Self::Print(names) => {
                for name in &names {
                    match ctx.variables.lock().unwrap().get(name) {
                        Some(val) => writeln!(ctx.out, "declare -- {}={}", name, val).unwrap(),
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
) -> bool {
    let mut ctx = BuiltinContext { out, err, executables, bg_jobs, history, variables };
    match name {
        "exit"    => ExitBuiltin::parse(args).run(&mut ctx),
        "echo"    => EchoBuiltin::parse(args).run(&mut ctx),
        "type"    => TypeBuiltin::parse(args).run(&mut ctx),
        "pwd"     => PwdBuiltin::parse(args).run(&mut ctx),
        "cd"      => CdBuiltin::parse(args).run(&mut ctx),
        "jobs"    => JobsBuiltin::parse(args).run(&mut ctx),
        "history" => HistoryBuiltin::parse(args).run(&mut ctx),
        "declare" => DeclareBuiltin::parse(args).run(&mut ctx),
        _         => { writeln!(ctx.err, "panic: unknown builtin '{}'", name).unwrap(); false }
    }
}
