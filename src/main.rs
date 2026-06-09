mod autocomplete;
mod builtins;
mod command;
mod shell;
mod shell_state;

fn main() {
    shell::Shell::new().run();
}
