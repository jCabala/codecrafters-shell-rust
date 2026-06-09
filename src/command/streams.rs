use std::io::{self, Write};
use super::{Fd, Redirect, WriteMode};

fn resolve_fd(redirects: &[Redirect], fd: Fd) -> Option<std::fs::File> {
    let mut result = None;
    for redirect in redirects {
        if redirect.fd == fd {
            let append = redirect.mode == WriteMode::Append;
            result = std::fs::OpenOptions::new()
                .write(true).create(true).append(append).truncate(!append)
                .open(&redirect.target)
                .map_err(|e| eprintln!("shell: {}: {}", redirect.target, e))
                .ok();
        }
    }
    result
}

pub struct Streams {
    pub stdin:  Option<std::fs::File>,
    pub stdout: Option<std::fs::File>,
    pub stderr: Option<std::fs::File>,
}

impl Streams {
    pub fn for_pipeline(redirects: &[Redirect], stdin: Option<std::fs::File>, pipe_stdout: Option<std::fs::File>) -> Self {
        Self {
            stdin,
            stdout: resolve_fd(redirects, Fd::Stdout).or(pipe_stdout),
            stderr: resolve_fd(redirects, Fd::Stderr),
        }
    }

    pub fn into_writers(self) -> (Box<dyn Write + Send>, Box<dyn Write + Send>) {
        let out: Box<dyn Write + Send> = match self.stdout { Some(f) => Box::new(f), None => Box::new(io::stdout()) };
        let err: Box<dyn Write + Send> = match self.stderr { Some(f) => Box::new(f), None => Box::new(io::stderr()) };
        (out, err)
    }

    pub fn apply_to_cmd(self, cmd: &mut std::process::Command) {
        if let Some(f) = self.stdin  { cmd.stdin(std::process::Stdio::from(f)); }
        if let Some(f) = self.stdout { cmd.stdout(std::process::Stdio::from(f)); }
        if let Some(f) = self.stderr { cmd.stderr(std::process::Stdio::from(f)); }
    }
}
