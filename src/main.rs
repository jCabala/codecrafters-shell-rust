use std::io::{self, Write};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use rustyline::{Editor, Helper, Hinter, Highlighter, Validator, Context, Config, CompletionType, error::ReadlineError};
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::history::DefaultHistory;

#[derive(Debug, Clone, Eq, PartialEq)]
enum CommandKind {
    Builtin,
    External(String),
    NotFound,
}

#[derive(PartialEq)]
enum Fd { Stdout, Stderr }

#[derive(PartialEq)]
enum WriteMode { Overwrite, Append }

struct Redirect {
    fd: Fd,
    mode: WriteMode,
    target: String,
}

struct Command {
    name: String,
    args: Vec<String>,
    redirects: Vec<Redirect>,
    command_type: CommandKind,
}

fn builtin_commands() -> Vec<&'static str> {
    vec!["exit", "echo", "type", "pwd", "cd"]
}

fn build_executables() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let paths = std::env::var("PATH").unwrap_or_default();
    for dir in paths.split(':') {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.permissions().mode() & 0o111 == 0 { continue; }
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else { continue };
            map.entry(name.to_string()).or_insert_with(|| format!("{}/{}", dir, name));
        }
    }
    map
}

fn get_command_type(command: &str, executables: &OnceLock<HashMap<String, String>>) -> CommandKind {
    if builtin_commands().contains(&command) {
        CommandKind::Builtin
    } else if let Some(path) = executables.get_or_init(build_executables).get(command) {
        CommandKind::External(path.clone())
    } else {
        CommandKind::NotFound
    }
}

fn parse_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = input.chars();

    while let Some(c) = chars.next() {
        match c {
            '\\' if !in_single_quote && !in_double_quote => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            '\\' if in_double_quote => {
                match chars.next() {
                    Some(next @ '"') | Some(next @ '\\') => current.push(next),
                    Some(next) => { current.push('\\'); current.push(next); }
                    None => current.push('\\'),
                }
            }
            '\'' if !in_single_quote && !in_double_quote => in_single_quote = true,
            '\'' if in_single_quote => in_single_quote = false,
            '"' if !in_single_quote && !in_double_quote => in_double_quote = true,
            '"' if in_double_quote => in_double_quote = false,
            ' ' | '\t' if !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn parse_redirect_op(s: &str) -> Option<(Fd, WriteMode)> {
    match s {
        ">" | "1>"   => Some((Fd::Stdout, WriteMode::Overwrite)),
        "2>"         => Some((Fd::Stderr, WriteMode::Overwrite)),
        ">>" | "1>>" => Some((Fd::Stdout, WriteMode::Append)),
        "2>>"        => Some((Fd::Stderr, WriteMode::Append)),
        _            => None,
    }
}

fn parse_redirects(all_args: Vec<String>) -> (Vec<String>, Vec<Redirect>) {
    let mut args = Vec::new();
    let mut redirects = Vec::new();
    let mut iter = all_args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        if let Some((fd, mode)) = parse_redirect_op(&arg) {
            if let Some(target) = iter.next() {
                redirects.push(Redirect { fd, mode, target });
            }
        } else {
            args.push(arg);
        }
    }
    (args, redirects)
}

fn parse_command(input: &str, executables: &OnceLock<HashMap<String, String>>) -> Command {
    let all_args = parse_args(input);
    let name = all_args.get(0).cloned().unwrap_or_default();
    let command_type = get_command_type(&name, executables);
    let (args, redirects) = parse_redirects(all_args);
    Command { name, args, redirects, command_type }
}

fn resolve_fd(redirects: &[Redirect], fd: Fd) -> Option<std::fs::File> {
    let mut result = None;
    for redirect in redirects {
        if redirect.fd == fd {
            let append = redirect.mode == WriteMode::Append;
            result = std::fs::OpenOptions::new()
                .write(true).create(true).append(append).truncate(!append)
                .open(&redirect.target)
                .map_err(|e| eprintln!("shell: {}: {}", redirect.target, e))
                .ok();
        }
    }
    result
}

fn execute(command: Command, stdout_file: Option<std::fs::File>, stderr_file: Option<std::fs::File>, executables: &OnceLock<HashMap<String, String>>) -> bool {
    let Command { name, args, command_type, .. } = command;
    match command_type {
        CommandKind::Builtin => {
            let mut out: Box<dyn Write> = match stdout_file {
                Some(f) => Box::new(f),
                None => Box::new(io::stdout()),
            };
            let mut err: Box<dyn Write> = match stderr_file {
                Some(f) => Box::new(f),
                None => Box::new(io::stderr()),
            };
            match name.as_str() {
                "exit" => return true,
                "echo" => writeln!(out, "{}", args.join(" ")).unwrap(),
                "type" => {
                    for arg in &args {
                        match get_command_type(arg, executables) {
                            CommandKind::Builtin => writeln!(out, "{} is a shell builtin", arg).unwrap(),
                            CommandKind::External(path) => writeln!(out, "{} is {}", arg, path).unwrap(),
                            CommandKind::NotFound => writeln!(err, "{}: not found", arg).unwrap(),
                        }
                    }
                }
                "pwd" => {
                    if let Ok(path) = std::env::current_dir() {
                        writeln!(out, "{}", path.display()).unwrap();
                    } else {
                        writeln!(err, "pwd: error getting current directory").unwrap();
                    }
                }
                "cd" => {
                    if args.is_empty() || args[0] == "~" {
                        if let Ok(path) = std::env::var("HOME") {
                            if let Err(_) = std::env::set_current_dir(&path) {
                                writeln!(err, "cd: {}: No such file or directory", path).unwrap();
                            }
                        }
                    } else {
                        let path = &args[0];
                        if let Err(_) = std::env::set_current_dir(path) {
                            writeln!(err, "cd: {}: No such file or directory", path).unwrap();
                        }
                    }
                }
                _ => writeln!(err, "panic: unknown builtin command '{}'", name).unwrap(),
            }
        }
        CommandKind::External(path) => {
            let mut cmd = std::process::Command::new(&path);
            cmd.arg0(&name).args(&args);
            if let Some(file) = stdout_file {
                cmd.stdout(std::process::Stdio::from(file));
            }
            if let Some(file) = stderr_file {
                cmd.stderr(std::process::Stdio::from(file));
            }
            let _ = cmd.status();
        }
        CommandKind::NotFound => {
            eprintln!("{}: command not found", name);
        }
    }
    false
}

#[derive(Helper, Hinter, Highlighter, Validator)]
struct ShellHelper {
    executables: Arc<OnceLock<HashMap<String, String>>>,
    file_completer: FilenameCompleter,
}

impl ShellHelper {
    fn new() -> Self {
        let executables = Arc::new(OnceLock::new());
        let bg = Arc::clone(&executables);
        std::thread::spawn(move || { bg.get_or_init(build_executables); });
        Self { executables, file_completer: FilenameCompleter::new() }
    }

    fn executables(&self) -> &HashMap<String, String> {
        self.executables.get_or_init(build_executables)
    }
}

impl Completer for ShellHelper {
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
            .chain(
                self.executables().keys()
                    .filter(|name| name.starts_with(word))
                    .map(|name| Pair { display: name.to_string(), replacement: format!("{} ", name) })
            )
            .collect();
        candidates.sort_by(|a, b| a.display.cmp(&b.display));
        candidates.dedup_by(|a, b| a.display == b.display);
        Ok((0, candidates))
    }
}

fn main() {
    let config = Config::builder().completion_type(CompletionType::List).build();
    let mut editor: Editor<ShellHelper, DefaultHistory> = Editor::with_config(config).expect("Failed to create line editor");
    editor.set_helper(Some(ShellHelper::new()));
    loop {
        let input = match editor.readline("$ ") {
            Ok(line) => { editor.add_history_entry(&line).ok(); line }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(e) => { eprintln!("shell: {}", e); break; }
        };

        let executables = &*editor.helper().unwrap().executables;
        let command = parse_command(&input, executables);
        if command.name.is_empty() {
            continue;
        }

        let stdout_file = resolve_fd(&command.redirects, Fd::Stdout);
        let stderr_file = resolve_fd(&command.redirects, Fd::Stderr);

        if execute(command, stdout_file, stderr_file, executables) {
            break;
        }
    }
}
