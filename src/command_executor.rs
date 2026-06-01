use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::os::unix::process::CommandExt;
use crate::bg_jobs::BackgroundJobRegistry;
use crate::builtins::run_builtin;
use crate::command::{Command, CommandKind, Streams};

fn spawn_background<F>(bg_jobs: Arc<Mutex<BackgroundJobRegistry>>, pid: u32, cmd_str: String, f: F)
where
    F: FnOnce() + Send + 'static,
{
    let job_id = bg_jobs.lock().unwrap().register_job(cmd_str);
    println!("[{}] {}", job_id, pid);
    std::thread::spawn(move || {
        f();
        bg_jobs.lock().unwrap().mark_done(job_id);
    });
}

pub fn execute(
    command: Command,
    streams: Streams,
    executables: Arc<HashMap<String, String>>,
    bg_jobs: Arc<Mutex<BackgroundJobRegistry>>,
) -> bool {
    let Command { name, args, command_type, is_background, .. } = command;

    if matches!(command_type, CommandKind::NotFound) {
        eprintln!("{}: command not found", name);
        return false;
    }

    let cmd_str = if args.is_empty() { name.clone() } else { format!("{} {}", name, args.join(" ")) };

    match command_type {
        CommandKind::External(path) => {
            let mut cmd = std::process::Command::new(&path);
            cmd.arg0(&name).args(&args);
            streams.apply_to_cmd(&mut cmd);
            match cmd.spawn() {
                Ok(mut child) => {
                    if is_background {
                        let pid = child.id();
                        spawn_background(Arc::clone(&bg_jobs), pid, cmd_str, move || { child.wait().ok(); });
                    } else {
                        child.wait().ok();
                    }
                }
                Err(e) => eprintln!("{}: {}", name, e),
            }
            false
        }
        CommandKind::Builtin => {
            let (mut out, mut err) = streams.into_writers();
            if is_background {
                let executables_clone = Arc::clone(&executables);
                let bg_for_builtin = Arc::clone(&bg_jobs);
                spawn_background(Arc::clone(&bg_jobs), 0, cmd_str, move || {
                    run_builtin(&name, &args, &mut *out, &mut *err, &executables_clone, &bg_for_builtin);
                });
                false
            } else {
                run_builtin(&name, &args, &mut *out, &mut *err, &executables, &bg_jobs)
            }
        }
        CommandKind::NotFound => unreachable!(),
    }
}
