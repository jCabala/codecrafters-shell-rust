use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use rustyline::{Editor, Helper, Hinter, Highlighter, Validator, Context, Config, CompletionType, error::ReadlineError};
use rustyline::completion::{Completer, Pair};
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

fn get_command_path(command: &str) -> Option<String> {
    if let Ok(paths) = std::env::var("PATH") {
        for path in paths.split(':') {
            let full_path = format!("{}/{}", path, command);
            if let Ok(metadata) = std::fs::metadata(&full_path) {
                if metadata.permissions().mode() & 0o111 != 0 {
                    return Some(full_path);
                }
            }
        }
    }
    None
}

fn get_command_type(command: &str) -> CommandKind {
    if builtin_commands().contains(&command) {
        CommandKind::Builtin
    } else if let Some(path) = get_command_path(command) {
        CommandKind::External(path)
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

fn parse_command(input: &str) -> Command {
    let all_args = parse_args(input);
    let name = all_args.get(0).cloned().unwrap_or_default();
    let command_type = get_command_type(&name);
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

fn execute(command: Command, stdout_file: Option<std::fs::File>, stderr_file: Option<std::fs::File>) -> bool {
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
                "exit" => return true, // That signals the main loop to exit
                "echo" => writeln!(out, "{}", args.join(" ")).unwrap(),
                "type" => {
                    for arg in &args {
                        match get_command_type(arg) {
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
struct ShellHelper;

impl Completer for ShellHelper {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        let word = &line[..pos];
        if word.contains(' ') {
            return Ok((pos, vec![]));
        }
        let candidates = builtin_commands()
            .iter()
            .filter(|cmd| cmd.starts_with(word))
            .map(|cmd| Pair { display: cmd.to_string(), replacement: format!("{} ", cmd) })
            .collect();
        Ok((0, candidates))
    }
}

fn main() {
    let config = Config::builder().completion_type(CompletionType::List).build();
    let mut editor: Editor<ShellHelper, DefaultHistory> = Editor::with_config(config).expect("Failed to create line editor");
    editor.set_helper(Some(ShellHelper));
    loop {
        let input = match editor.readline("$ ") {
            Ok(line) => { editor.add_history_entry(&line).ok(); line }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(e) => { eprintln!("shell: {}", e); break; }
        };

        let command = parse_command(&input);
        if command.name.is_empty() {
            continue;
        }

        let stdout_file = resolve_fd(&command.redirects, Fd::Stdout);
        let stderr_file = resolve_fd(&command.redirects, Fd::Stderr);

        if execute(command, stdout_file, stderr_file) {
            break;
        }
    }
}
