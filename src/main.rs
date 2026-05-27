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

        // Route the command
        match input.as_str() {
            "exit" => break,
            _ => invalid_command_error(&input),
        }
    }
}
