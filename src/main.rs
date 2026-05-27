#[allow(unused_imports)]
use std::io::{self, Write};


fn known_commands() -> Vec<&'static str> {
    vec!["exit", "echo", "type"]
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
                } else if known_commands().contains(&args[0]) {
                    for arg in args {
                        println!("{} is a shell builtin", arg);
                    }
                } else {
                    eprintln!("{}: not found", args[0]);
                }
            }
            _ => eprintln!("{}: command not found", command),
        }
    }
}
