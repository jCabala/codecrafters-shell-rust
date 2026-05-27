#[allow(unused_imports)]
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;


fn builtin_commands() -> Vec<&'static str> {
    vec!["exit", "echo", "type", "pwd", "cd", "ls"]
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

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        // Read user input
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed to read line");
        input = input.trim().to_string();

        // Split command into name and arguments
        let mut parts = input.split_whitespace();
        let command = parts.next().unwrap_or("");
        
        if command.is_empty() {
            continue; // Skip empty input
        }
        
        let args: Vec<&str> = parts.collect();
        let command_type = get_command_type(command);

        if command_type == TypeResult::Builtin {
            // Handle builtin commands
            match command {
                "exit" => break,
                "echo" => println!("{}", args.join(" ")),
                "type" => {
                    for arg in &args {
                        match get_command_type(arg) {
                            TypeResult::Builtin => println!("{} is a shell builtin", arg),
                            TypeResult::External(path) => println!("{} is {}", arg, path),
                            TypeResult::NotFound => eprintln!("{}: not found", arg),
                        }
                    }
                }
                "pwd" => {
                    if let Ok(path) = std::env::current_dir() {
                        println!("{}", path.display());
                    } else {
                        eprintln!("pwd: error getting current directory");
                    }
                }
                "cd" => {
                    if args.is_empty() || args[0] == "~" {
                        if let Ok(path) = std::env::var("HOME") {
                            if let Err(_) = std::env::set_current_dir(&path) {
                                eprintln!("cd: {}: No such file or directory", path);
                            }
                        }
                    }  else {
                        let path = args[0];
                        if let Err(_) = std::env::set_current_dir(path) {
                            eprintln!("cd: {}: No such file or directory", path);
                        }
                    }
                }
                "ls" => {
                    // Just list files in current directory
                    if let Ok(entries) = std::fs::read_dir(".") {
                        for entry in entries {
                            if let Ok(entry) = entry {
                                print!("{}  ", entry.file_name().to_string_lossy());
                            }
                        }
                        println!();
                    } else {
                        eprintln!("ls: error reading current directory");
                    }
                }
                _ => eprintln!("panic: unknown builtin command '{}'", command),
            }
        } else if let TypeResult::External(path) = command_type {
            let _ = std::process::Command::new(&path).arg0(command).args(&args).status();
        } else {
            eprintln!("{}: command not found", command);
        }

    }
}
