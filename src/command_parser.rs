use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use crate::command::{Command, CommandKind, Fd, Redirect, WriteMode};

pub fn builtin_commands() -> Vec<&'static str> {
    vec!["exit", "echo", "type", "pwd", "cd", "jobs"]
}

pub fn build_executables() -> HashMap<String, String> {
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

pub fn get_command_type(command: &str, executables: &HashMap<String, String>) -> CommandKind {
    if builtin_commands().contains(&command) {
        CommandKind::Builtin
    } else if let Some(path) = executables.get(command) {
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

pub fn parse_command(input: &str, executables: &HashMap<String, String>) -> Command {
    let all_args = parse_args(input);
    let name = all_args.get(0).cloned().unwrap_or_default();
    let command_type = get_command_type(&name, executables);
    let (mut args, redirects) = parse_redirects(all_args);

    let is_background = args.last() == Some(&"&".into());
    if is_background {
        args.pop();
    }
    Command { name, args, redirects, command_type, is_background }
}
