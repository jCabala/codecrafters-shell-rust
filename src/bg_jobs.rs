use std::io::Write;
use std::collections::HashMap;

pub struct BackgroundJob {
    name: String,
    pub cmd: String,
    pub done: bool,
}

pub struct BackgroundJobRegistry {
    pub jobs: HashMap<usize, BackgroundJob>,
    next_id: usize,
    last_id: usize,
    prev_id: usize,
}

impl BackgroundJobRegistry {
    pub fn new() -> Self {
        Self { jobs: HashMap::new(), next_id: 1, last_id: 0, prev_id: 0 }
    }

    pub fn register_job(&mut self, name: String, cmd: String) -> usize {
        let job_id = self.next_id;
        self.next_id += 1;
        self.prev_id = self.last_id;
        self.last_id = job_id;
        self.jobs.insert(job_id, BackgroundJob { name, cmd, done: false });
        job_id
    }

    pub fn mark_done(&mut self, job_id: usize) {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.done = true;
        }
    }

    pub fn drain_completed(&mut self) -> Vec<(usize, String)> {
        let mut done_ids: Vec<usize> = self.jobs.iter()
            .filter_map(|(&id, j)| if j.done { Some(id) } else { None })
            .collect();
        done_ids.sort();
        done_ids.into_iter().filter_map(|id| {
            self.jobs.remove(&id).map(|job| {
                self.update_markers_after_removal(id);
                (id, job.name)
            })
        }).collect()
    }

    pub fn list_jobs(&mut self, out: &mut dyn Write) {
        let mut ids: Vec<usize> = self.jobs.keys().copied().collect();
        ids.sort();

        let mut done_ids = Vec::new();
        for &id in &ids {
            let job = &self.jobs[&id];
            let marker = if id == self.last_id { '+' } else if id == self.prev_id { '-' } else { ' ' };
            if job.done {
                writeln!(out, "[{}]{}  {:<24}{}", id, marker, "Done", job.cmd).unwrap();
                done_ids.push(id);
            } else {
                writeln!(out, "[{}]{}  {:<24}{} &", id, marker, "Running", job.cmd).unwrap();
            }
        }

        for id in done_ids {
            self.jobs.remove(&id);
            self.update_markers_after_removal(id);
        }
    }

    fn update_markers_after_removal(&mut self, removed_id: usize) {
        if removed_id == self.last_id {
            self.last_id = self.prev_id;
            self.prev_id = self.jobs.keys().copied().filter(|&id| id != self.last_id).max().unwrap_or(0);
        } else if removed_id == self.prev_id {
            self.prev_id = 0;
        }
    }
}
