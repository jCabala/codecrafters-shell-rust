#[allow(unused_imports)]
use std::io::{self, Write};


fn invalid_command_error(command: &str) {
    eprintln!("{command}: command not found");
}

fn main() {
    print!("$ ");
    io::stdout().flush().unwrap();

    // Read user input
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed to read line");
    input = input.trim().to_string();

    // Throw an invalid command error
    invalid_command_error(&input);
}
