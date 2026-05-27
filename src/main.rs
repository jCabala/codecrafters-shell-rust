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
            if let Ok(metadata) = std::fs::metadata(&full_path) {
                if metadata.permissions().mode() & 0o111 != 0 {
                    return Some(full_path);
                }
            }
        }
    }
    None
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
                for arg in &args {
                    if builtin_commands().contains(arg) {
                        println!("{} is a built-in command", arg);
                    } else if let Some(path) = get_command_path(arg) {
                        println!("{} is {}", arg, path);
                    } else {
                        println!("{} not found", arg);
                    }
                }
            }
            _ => eprintln!("{}: command not found", command),
        }
    }
}
