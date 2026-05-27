#[allow(unused_imports)]
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;


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

#[derive(Debug, Clone, Eq, PartialEq)]
enum TypeResult {
    Builtin,
    External(String),
    NotFound,
}
fn get_command_type(command: &str) -> TypeResult {
    if builtin_commands().contains(&command) {
        TypeResult::Builtin
    } else if let Some(path) = get_command_path(command) {
        TypeResult::External(path)
    } else {
        TypeResult::NotFound
    }
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
    command_type: TypeResult,
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

fn parse_redirects(all_args: Vec<String>) -> (Vec<String>, Vec<Redirect>) {
    // Split args and redirects
    let mut args = Vec::new();
    let mut redirects = Vec::new();
    let mut iter = all_args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == ">" || arg == "1>" {
            if let Some(target) = iter.next() {
                redirects.push(Redirect { fd: Fd::Stdout, mode: WriteMode::Overwrite, target });
            }
        } else if arg == "2>" {
            if let Some(target) = iter.next() {
                redirects.push(Redirect { fd: Fd::Stderr, mode: WriteMode::Overwrite, target });
            }
        } else if arg == ">>" || arg == "1>>" {
            if let Some(target) = iter.next() {
                redirects.push(Redirect { fd: Fd::Stdout, mode: WriteMode::Append, target });
            }
        } else if arg == "2>>" {
            if let Some(target) = iter.next() {
                redirects.push(Redirect { fd: Fd::Stderr, mode: WriteMode::Append, target });
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

fn resolve_stdout(redirects: &[Redirect]) -> Option<std::fs::File> {
    resolve_fd(redirects, Fd::Stdout)
}

fn resolve_stderr(redirects: &[Redirect]) -> Option<std::fs::File> {
    resolve_fd(redirects, Fd::Stderr)
}

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");
        let input = input.trim_end_matches('\n').trim_end_matches('\r').to_string();

        let Command { name: command_name, args, redirects, command_type } = parse_command(&input);

        if command_name.is_empty() {
            continue;
        }

        let stdout_file = resolve_stdout(&redirects);
        let stderr_file = resolve_stderr(&redirects);

        if command_type == TypeResult::Builtin {
            let mut out: Box<dyn Write> = match stdout_file {
                Some(f) => Box::new(f),
                None => Box::new(io::stdout()),
            };
            let mut err: Box<dyn Write> = match stderr_file {
                Some(f) => Box::new(f),
                None => Box::new(io::stderr()),
            };
            match command_name.as_str() {
                "exit" => break,
                "echo" => writeln!(out, "{}", args.join(" ")).unwrap(),
                "type" => {
                    for arg in &args {
                        match get_command_type(arg) {
                            TypeResult::Builtin => writeln!(out, "{} is a shell builtin", arg).unwrap(),
                            TypeResult::External(path) => writeln!(out, "{} is {}", arg, path).unwrap(),
                            TypeResult::NotFound => writeln!(err, "{}: not found", arg).unwrap(),
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
                _ => writeln!(err, "panic: unknown builtin command '{}'", command_name).unwrap(),
            }
        } else if let TypeResult::External(path) = command_type {
            let mut cmd = std::process::Command::new(&path);
            cmd.arg0(&command_name).args(&args);
            if let Some(file) = stdout_file {
                cmd.stdout(std::process::Stdio::from(file));
            }
            if let Some(file) = stderr_file {
                cmd.stderr(std::process::Stdio::from(file));
            }
            let _ = cmd.status();
        } else {
            eprintln!("{}: command not found", command_name);
        }
    }
}
