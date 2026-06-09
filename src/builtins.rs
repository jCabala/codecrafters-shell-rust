use std::io::Write;
use std::sync::{Arc, Mutex};
use crate::bg_jobs::BackgroundJobRegistry;
use crate::command::CommandKind;
use crate::command_parser::get_command_type;
use crate::executables::ExecutableMap;
use crate::history::History;

pub fn run_builtin(
    name: &str,
    args: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
    executables: &ExecutableMap,
    bg_jobs: &Arc<Mutex<BackgroundJobRegistry>>,
    history: &Arc<Mutex<History>>,
) -> bool {
    match name {
        "exit" => return true,
        "echo" => writeln!(out, "{}", args.join(" ")).unwrap(),
        "type" => {
            for arg in args {
                match get_command_type(arg, executables) {
                    Some(CommandKind::Builtin) => writeln!(out, "{} is a shell builtin", arg).unwrap(),
                    Some(CommandKind::External(path)) => writeln!(out, "{} is {}", arg, path).unwrap(),
                    None => writeln!(err, "{}: not found", arg).unwrap(),
                }
            }
        }
        "pwd" => {
            if let Ok(path) = std::env::current_dir() {
                writeln!(out, "{}", path.display()).unwrap();
            } else {
                writeln!(err, "pwd: error getting current directory").unwrap();
            }
        }
        "cd" => {
            if args.is_empty() || args[0] == "~" {
                if let Ok(path) = std::env::var("HOME") {
                    if let Err(_) = std::env::set_current_dir(&path) {
                        writeln!(err, "cd: {}: No such file or directory", path).unwrap();
                    }
                }
            } else {
                let path = &args[0];
                if let Err(_) = std::env::set_current_dir(path) {
                    writeln!(err, "cd: {}: No such file or directory", path).unwrap();
                }
            }
        }
        "jobs" => bg_jobs.lock().unwrap().list_jobs(out),
        "history" => {
            if args.first().map(|a| a == "-r").unwrap_or(false) {
                match args.get(1) {
                    None => writeln!(err, "history: -r: missing filename").unwrap(),
                    Some(path) => if let Err(e) = history.lock().unwrap().read_from_file(path) {
                        writeln!(err, "history: {}: {}", path, e).unwrap();
                    }
                }
            } else if args.first().map(|a| a == "-w").unwrap_or(false) {
                match args.get(1) {
                    None => writeln!(err, "history: -w: missing filename").unwrap(),
                    Some(path) => if let Err(e) = history.lock().unwrap().write_to_file(path) {
                        writeln!(err, "history: {}: {}", path, e).unwrap();
                    }
                }
            } else {
                let limit = args.first().and_then(|a| a.parse::<usize>().ok());
                history.lock().unwrap().write_to(out, limit).unwrap();
            }
        }
        _ => writeln!(err, "panic: unknown builtin command '{}'", name).unwrap(),
    }
    false
}
