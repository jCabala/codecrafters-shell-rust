#[allow(unused_imports)]
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;


fn builtin_commands() -> Vec<&'static str> {
    vec!["exit", "echo", "type"]
}

fn get_command_path(command: &str) -> Option<String> {
    if let Ok(paths) = std::env::var("PATH") {
        for path in paths.split(':') {
            let full_path = format!("{}/{}", path, command);
            if std::fs::metadata(&full_path).is_ok() {
                return Some(full_path);
            }
        }
    }
    None
}

fn has_executable_permission(path: &str) -> bool {
    if let Ok(metadata) = std::fs::metadata(path) {
        let permissions = metadata.permissions();
        return permissions.mode() & 0o111 != 0;
    }
    false
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
        let args: Vec<&str> = parts.collect();

        // Route the command
        match command {
            "exit" => break,
            "echo" => println!("{}", args.join(" ")),
            "type" => {
                if args.is_empty() {
                    eprintln!("type: missing operand");
                } else if builtin_commands().contains(&args[0]) {
                    for arg in args {
                        println!("{} is a shell builtin", arg);
                    }
                } else if let Some(path) = get_command_path(&args[0]) && has_executable_permission(&path) {
                    eprintln!("{} is {}", args[0], path);
                } else {
                    eprintln!("{}: not found", args[0]);
                }
            }
            _ => eprintln!("{}: command not found", command),
        }
    }
}
