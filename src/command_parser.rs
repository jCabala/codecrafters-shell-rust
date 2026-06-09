use crate::command::{Command, CommandKind, Fd, Redirect, WriteMode};
use crate::executables::ExecutableMap;

pub fn builtin_commands() -> Vec<&'static str> {
    vec!["exit", "echo", "type", "pwd", "cd", "jobs", "history", "declare"]
}

pub fn get_command_type(command: &str, executables: &ExecutableMap) -> Option<CommandKind> {
    if builtin_commands().contains(&command) {
        Some(CommandKind::Builtin)
    } else if let Some(path) = executables.get(command) {
        Some(CommandKind::External(path.clone()))
    } else {
        None
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

fn split_pipeline(input: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0;
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = input.char_indices();

    while let Some((i, c)) = chars.next() {
        match c {
            '\\' if !in_single => { chars.next(); }
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '|' if !in_single && !in_double => {
                segments.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    segments.push(&input[start..]);
    segments
}

pub struct Pipeline {
    pub commands: Vec<Command>,
    pub is_background: bool,
}

pub fn parse_pipeline(input: &str, executables: &ExecutableMap) -> Option<Pipeline> {
    let segments: Vec<&str> = split_pipeline(input)
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if segments.is_empty() {
        return None;
    }

    let mut commands = Vec::new();
    for s in segments {
        match parse_command(s, executables) {
            Some(cmd) => commands.push(cmd),
            None => return None,
        }
    }

    let n = commands.len();
    for (i, cmd) in commands.iter().enumerate() {
        for (j, arg) in cmd.args.iter().enumerate() {
            if arg == "&" {
                let is_last_position = i == n - 1 && j == cmd.args.len() - 1;
                if !is_last_position {
                    eprintln!("syntax error near unexpected token '&'");
                    return None;
                }
            }
        }
    }

    let is_background = commands.last_mut()
        .map(|cmd| {
            if cmd.args.last().map(|a| a == "&").unwrap_or(false) {
                cmd.args.pop();
                true
            } else {
                false
            }
        })
        .unwrap_or(false);

    Some(Pipeline { commands, is_background })
}

fn parse_command(input: &str, executables: &ExecutableMap) -> Option<Command> {
    let all_args = parse_args(input);
    let name = all_args.get(0).cloned().unwrap_or_default();
    let command_type = match get_command_type(&name, executables) {
        Some(ct) => ct,
        None => { eprintln!("{}: command not found", name); return None; }
    };
    let (args, redirects) = parse_redirects(all_args);
    Some(Command { name, args, redirects, command_type })
}
