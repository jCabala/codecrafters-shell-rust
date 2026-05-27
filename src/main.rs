#[allow(unused_imports)]
use std::io::{self, Write};


fn invalid_command_error(command: &str) {
    eprintln!("{command}: command not found");
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
            _ => invalid_command_error(command),
        }
    }
}
