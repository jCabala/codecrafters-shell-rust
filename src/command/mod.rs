pub mod executor;
pub mod parser;
mod streams;

pub use streams::Streams;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CommandKind {
    Builtin,
    External(String),
}

#[derive(PartialEq)]
pub enum Fd { Stdout, Stderr }

#[derive(PartialEq)]
pub enum WriteMode { Overwrite, Append }

pub struct Redirect {
    pub fd: Fd,
    pub mode: WriteMode,
    pub target: String,
}

pub struct Command {
    pub name: String,
    pub args: Vec<String>,
    pub redirects: Vec<Redirect>,
    pub command_type: CommandKind,
}

