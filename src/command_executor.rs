use crate::executables::ExecutableMap;
use std::fs::File;
use std::os::unix::io::FromRawFd;
use std::sync::{Arc, Mutex};
use std::os::unix::process::CommandExt;
use crate::bg_jobs::BackgroundJobRegistry;
use crate::builtins::run_builtin;
use crate::command::{Command, CommandKind, Streams};
use crate::command_parser::Pipeline;
use crate::history::History;


pub fn execute_pipeline(
    pipeline: Pipeline,
    executables: Arc<ExecutableMap>,
    bg_jobs: Arc<Mutex<BackgroundJobRegistry>>,
    history: Arc<Mutex<History>>,
) -> bool {
    let Pipeline { commands, is_background } = pipeline;
    let n = commands.len();

    let cmd_str = commands.iter()
        .map(|c| if c.args.is_empty() { c.name.clone() } else { format!("{} {}", c.name, c.args.join(" ")) })
        .collect::<Vec<_>>()
        .join(" | ");

    let mut read_ends:  Vec<Option<File>> = (0..n).map(|_| None).collect();
    let mut write_ends: Vec<Option<File>> = (0..n).map(|_| None).collect();

    for i in 0..n - 1 {
        let mut fds = [0i32; 2];
         /* O_CLOEXEC: close inherited write ends in child on exec, preventing self-deadlock
            Why? EOF only when all write ends are closed. If a child inherits a write end and execs an external command, it may never close that write end, causing the parent to wait indefinitely for EOF. */
        unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) };
        read_ends[i + 1] = Some(unsafe { File::from_raw_fd(fds[0]) });
        write_ends[i]    = Some(unsafe { File::from_raw_fd(fds[1]) });
    }

    let mut children: Vec<std::process::Child>          = Vec::new();
    let mut threads:  Vec<std::thread::JoinHandle<bool>> = Vec::new();

    for (i, command) in commands.into_iter().enumerate() {
        let streams = Streams::for_pipeline(&command.redirects, read_ends[i].take(), write_ends[i].take());
        let Command { name, args, command_type, .. } = command;

        match command_type {
            CommandKind::External(path) => {
                let mut cmd = std::process::Command::new(&path);
                cmd.arg0(&name).args(&args);
                streams.apply_to_cmd(&mut cmd);
                match cmd.spawn() {
                    Ok(child) => children.push(child),
                    Err(e)    => eprintln!("{}: {}", name, e),
                }
            }
            CommandKind::Builtin => {
                let (mut out, mut err) = streams.into_writers();
                let executables_clone = Arc::clone(&executables);
                let bg_jobs_clone = Arc::clone(&bg_jobs);
                let history_clone = Arc::clone(&history);
                let handle = std::thread::spawn(move || {
                    run_builtin(&name, &args, &mut *out, &mut *err, &executables_clone, &bg_jobs_clone, &history_clone)
                });
                threads.push(handle);
            }
        }
    }

    let last_pid = children.last().map(|c| c.id()).unwrap_or(0);

    if is_background {
        let job_id = bg_jobs.lock().unwrap().register_job(cmd_str);
        println!("[{}] {}", job_id, last_pid);
        std::thread::spawn(move || {
            for mut child in children { child.wait().ok(); }
            for handle in threads { handle.join().ok(); }
            bg_jobs.lock().unwrap().mark_done(job_id);
        });
        false
    } else {
        for mut child in children { child.wait().ok(); }
        threads.into_iter().any(|h| h.join().unwrap_or(false))
    }
}
