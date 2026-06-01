use std::io::{self, Write};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use rustyline::{Editor, Helper, Hinter, Highlighter, Validator, Context, Config, CompletionType, error::ReadlineError};
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::history::DefaultHistory;

#[derive(Debug, Clone, Eq, PartialEq)]
enum CommandKind {
    Builtin,
    External(String),
    NotFound,
}

#[derive(PartialEq)]
enum Fd { Stdout, Stderr }

#[derive(PartialEq)]
enum WriteMode { Overwrite, Append }

struct Redirect {
    fd: Fd,
    mode: WriteMode,
    target: String,
}

struct Command {
    name: String,
    args: Vec<String>,
    redirects: Vec<Redirect>,
    command_type: CommandKind,
    is_background: bool,
}

fn builtin_commands() -> Vec<&'static str> {
    vec!["exit", "echo", "type", "pwd", "cd", "jobs"]
}

fn build_executables() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let paths = std::env::var("PATH").unwrap_or_default();
    for dir in paths.split(':') {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.permissions().mode() & 0o111 == 0 { continue; }
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else { continue };
            map.entry(name.to_string()).or_insert_with(|| format!("{}/{}", dir, name));
        }
    }
    map
}

fn get_command_type(command: &str, executables: &HashMap<String, String>) -> CommandKind {
    if builtin_commands().contains(&command) {
        CommandKind::Builtin
    } else if let Some(path) = executables.get(command) {
        CommandKind::External(path.clone())
    } else {
        CommandKind::NotFound
    }
}

fn parse_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut chars = input.chars();

    while let Some(c) = chars.next() {
        match c {
            '\\' if !in_single_quote && !in_double_quote => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            '\\' if in_double_quote => {
                match chars.next() {
                    Some(next @ '"') | Some(next @ '\\') => current.push(next),
                    Some(next) => { current.push('\\'); current.push(next); }
                    None => current.push('\\'),
                }
            }
            '\'' if !in_single_quote && !in_double_quote => in_single_quote = true,
            '\'' if in_single_quote => in_single_quote = false,
            '"' if !in_single_quote && !in_double_quote => in_double_quote = true,
            '"' if in_double_quote => in_double_quote = false,
            ' ' | '\t' if !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn parse_redirect_op(s: &str) -> Option<(Fd, WriteMode)> {
    match s {
        ">" | "1>"   => Some((Fd::Stdout, WriteMode::Overwrite)),
        "2>"         => Some((Fd::Stderr, WriteMode::Overwrite)),
        ">>" | "1>>" => Some((Fd::Stdout, WriteMode::Append)),
        "2>>"        => Some((Fd::Stderr, WriteMode::Append)),
        _            => None,
    }
}

fn parse_redirects(all_args: Vec<String>) -> (Vec<String>, Vec<Redirect>) {
    let mut args = Vec::new();
    let mut redirects = Vec::new();
    let mut iter = all_args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        if let Some((fd, mode)) = parse_redirect_op(&arg) {
            if let Some(target) = iter.next() {
                redirects.push(Redirect { fd, mode, target });
            }
        } else {
            args.push(arg);
        }
    }
    (args, redirects)
}

fn parse_command(input: &str, executables: &HashMap<String, String>) -> Command {
    let all_args = parse_args(input);
    let name = all_args.get(0).cloned().unwrap_or_default();
    let command_type = get_command_type(&name, executables);
    let (mut args, redirects) = parse_redirects(all_args);

    let is_background = args.last() == Some(&"&".into());
    if is_background {
        args.pop();
    }
    Command { name, args, redirects, command_type, is_background }
}

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

struct Streams {
    stdout: Option<std::fs::File>,
    stderr: Option<std::fs::File>,
}

impl Streams {
    fn from_redirects(redirects: &[Redirect]) -> Self {
        Self {
            stdout: resolve_fd(redirects, Fd::Stdout),
            stderr: resolve_fd(redirects, Fd::Stderr),
        }
    }

    fn into_writers(self) -> (Box<dyn Write + Send>, Box<dyn Write + Send>) {
        let out: Box<dyn Write + Send> = match self.stdout { Some(f) => Box::new(f), None => Box::new(io::stdout()) };
        let err: Box<dyn Write + Send> = match self.stderr { Some(f) => Box::new(f), None => Box::new(io::stderr()) };
        (out, err)
    }

    fn apply_to_cmd(self, cmd: &mut std::process::Command) {
        if let Some(f) = self.stdout { cmd.stdout(std::process::Stdio::from(f)); }
        if let Some(f) = self.stderr { cmd.stderr(std::process::Stdio::from(f)); }
    }
}

struct BackgroundJob {
    child: Option<std::process::Child>,
    name: String,
    cmd: String,
}

struct BackgroundJobHandler {
    jobs: HashMap<usize, BackgroundJob>,
    next_id: usize,
    last_id: usize,
    prev_id: usize,
    completed: Vec<(usize, String)>,
}

impl BackgroundJobHandler {
    fn new() -> Self {
        Self { jobs: HashMap::new(), next_id: 1, last_id: 0, prev_id: 0, completed: Vec::new() }
    }

    fn next_job_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn register_job(&mut self, job_id: usize, child: Option<std::process::Child>, name: String, cmd: String) {
        self.prev_id = self.last_id;
        self.last_id = job_id;
        self.jobs.insert(job_id, BackgroundJob { child, name, cmd });
    }

    fn update_markers_after_removal(&mut self, removed_id: usize) {
        if removed_id == self.last_id {
            self.last_id = self.prev_id;
            self.prev_id = self.jobs.keys().copied().filter(|&id| id != self.last_id).max().unwrap_or(0);
        } else if removed_id == self.prev_id {
            self.prev_id = self.jobs.keys().copied().filter(|&id| id != self.last_id).max().unwrap_or(0);
        }
    }

    fn mark_done(&mut self, job_id: usize) {
        if let Some(job) = self.jobs.remove(&job_id) {
            self.update_markers_after_removal(job_id);
            self.completed.push((job_id, job.name));
        }
    }

    fn drain_completed(&mut self) -> Vec<(usize, String)> {
        let ids: Vec<usize> = self.jobs.keys().copied().collect();
        for id in ids {
            if let Some(job) = self.jobs.get_mut(&id) {
                if let Some(ref mut child) = job.child {
                    if matches!(child.try_wait(), Ok(Some(_))) {
                        if let Some(job) = self.jobs.remove(&id) {
                            self.update_markers_after_removal(id);
                            self.completed.push((id, job.name));
                        }
                    }
                }
            }
        }
        std::mem::take(&mut self.completed)
    }

    fn list_jobs(&mut self, out: &mut dyn Write) {
        let mut ids: Vec<usize> = self.jobs.keys().copied().collect();
        ids.sort();

        let mut done_ids = Vec::new();
        for &id in &ids {
            if let Some(job) = self.jobs.get_mut(&id) {
                if let Some(ref mut child) = job.child {
                    if matches!(child.try_wait(), Ok(Some(_))) {
                        done_ids.push(id);
                    }
                }
            }
        }

        for &id in &ids {
            let job = &self.jobs[&id];
            let marker = if id == self.last_id { '+' } else if id == self.prev_id { '-' } else { ' ' };
            if done_ids.contains(&id) {
                writeln!(out, "[{}]{}  {:<24}{}", id, marker, "Done", job.cmd).unwrap();
            } else {
                writeln!(out, "[{}]{}  {:<24}{} &", id, marker, "Running", job.cmd).unwrap();
            }
        }

        for id in done_ids {
            self.jobs.remove(&id);
            self.update_markers_after_removal(id);
        }
    }
}

fn run_builtin(
    name: &str,
    args: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
    executables: &HashMap<String, String>,
    bg_jobs: &Arc<Mutex<BackgroundJobHandler>>,
) -> bool {
    match name {
        "exit" => return true,
        "echo" => writeln!(out, "{}", args.join(" ")).unwrap(),
        "type" => {
            for arg in args {
                match get_command_type(arg, executables) {
                    CommandKind::Builtin => writeln!(out, "{} is a shell builtin", arg).unwrap(),
                    CommandKind::External(path) => writeln!(out, "{} is {}", arg, path).unwrap(),
                    CommandKind::NotFound => writeln!(err, "{}: not found", arg).unwrap(),
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
        _ => writeln!(err, "panic: unknown builtin command '{}'", name).unwrap(),
    }
    false
}

fn execute(
    command: Command,
    streams: Streams,
    executables: Arc<HashMap<String, String>>,
    bg_jobs: Arc<Mutex<BackgroundJobHandler>>,
) -> bool {
    let Command { name, args, command_type, is_background, .. } = command;

    if matches!(command_type, CommandKind::NotFound) {
        eprintln!("{}: command not found", name);
        return false;
    }

    let cmd_str = if args.is_empty() { name.clone() } else { format!("{} {}", name, args.join(" ")) };

    match (command_type, is_background) {
        (CommandKind::External(path), false) => {
            let mut cmd = std::process::Command::new(&path);
            cmd.arg0(&name).args(&args);
            streams.apply_to_cmd(&mut cmd);
            match cmd.spawn() {
                Ok(mut child) => { child.wait().ok(); }
                Err(e) => eprintln!("{}: {}", name, e),
            }
            false
        }
        (CommandKind::External(path), true) => {
            let mut cmd = std::process::Command::new(&path);
            cmd.arg0(&name).args(&args);
            streams.apply_to_cmd(&mut cmd);
            match cmd.spawn() {
                Ok(child) => {
                    let pid = child.id();
                    let job_id = bg_jobs.lock().unwrap().next_job_id();
                    println!("[{}] {}", job_id, pid);
                    bg_jobs.lock().unwrap().register_job(job_id, Some(child), name, cmd_str);
                }
                Err(e) => eprintln!("{}: {}", name, e),
            }
            false
        }
        (CommandKind::Builtin, false) => {
            let (mut out, mut err) = streams.into_writers();
            run_builtin(&name, &args, &mut *out, &mut *err, &executables, &bg_jobs)
        }
        (CommandKind::Builtin, true) => {
            let job_id = bg_jobs.lock().unwrap().next_job_id();
            let name_for_reg = name.clone();
            let bg_jobs_clone = Arc::clone(&bg_jobs);
            let bg_for_builtin = Arc::clone(&bg_jobs);
            let executables_clone = Arc::clone(&executables);
            let (mut out, mut err) = streams.into_writers();
            std::thread::spawn(move || {
                run_builtin(&name, &args, &mut *out, &mut *err, &executables_clone, &bg_for_builtin);
                bg_jobs_clone.lock().unwrap().mark_done(job_id);
            });
            println!("[{}] 0", job_id);
            bg_jobs.lock().unwrap().register_job(job_id, None, name_for_reg, cmd_str);
            false
        }
        (CommandKind::NotFound, _) => unreachable!(),
    }
}

#[derive(Helper, Hinter, Highlighter, Validator)]
struct ShellHelper {
    executables: Arc<HashMap<String, String>>,
    file_completer: FilenameCompleter,
}

impl ShellHelper {
    fn new(executables: Arc<HashMap<String, String>>) -> Self {
        Self { executables, file_completer: FilenameCompleter::new() }
    }
}

impl Completer for ShellHelper {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        let word = &line[..pos];
        if word.contains(' ') {
            let (start, candidates) = self.file_completer.complete(line, pos, _ctx)?;
            let candidates = candidates.into_iter().map(|p| {
                if p.replacement.ends_with('/') {
                    Pair { display: format!("{}/", p.display), replacement: p.replacement }
                } else {
                    Pair { display: p.display, replacement: format!("{} ", p.replacement) }
                }
            }).collect();
            return Ok((start, candidates));
        }
        let mut candidates: Vec<Pair> = builtin_commands()
            .iter()
            .filter(|cmd| cmd.starts_with(word))
            .map(|cmd| Pair { display: cmd.to_string(), replacement: format!("{} ", cmd) })
            .collect();
        candidates.extend(
            self.executables.keys()
                .filter(|name| name.starts_with(word))
                .map(|name| Pair { display: name.to_string(), replacement: format!("{} ", name) })
        );
        candidates.sort_by(|a, b| a.display.cmp(&b.display));
        candidates.dedup_by(|a, b| a.display == b.display);
        Ok((0, candidates))
    }
}

fn main() {
    let executables = Arc::new(build_executables());
    let bg_jobs = Arc::new(Mutex::new(BackgroundJobHandler::new()));
    let config = Config::builder().completion_type(CompletionType::List).build();
    let mut editor: Editor<ShellHelper, DefaultHistory> = Editor::with_config(config).expect("Failed to create line editor");
    editor.set_helper(Some(ShellHelper::new(Arc::clone(&executables))));
    loop {
        for (id, name) in bg_jobs.lock().unwrap().drain_completed() {
            eprintln!("[{}]+  Done    {}", id, name);
        }

        let input = match editor.readline("$ ") {
            Ok(line) => { editor.add_history_entry(&line).ok(); line }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break,
            Err(e) => { eprintln!("shell: {}", e); break; }
        };

        let command = parse_command(&input, &executables);
        if command.name.is_empty() {
            continue;
        }

        let streams = Streams::from_redirects(&command.redirects);
        if execute(command, streams, Arc::clone(&executables), Arc::clone(&bg_jobs)) {
            break;
        }
    }
}
