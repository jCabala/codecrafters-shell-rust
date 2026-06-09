use rayon::prelude::*;
use rustc_hash::FxHashMap;
use std::os::unix::fs::PermissionsExt;

pub type ExecutableMap = FxHashMap<String, String>;

fn scan_dir(dir: &str) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries.flatten()
        .filter(|e| !e.file_type().map(|t| t.is_dir()).unwrap_or(false)) // skip dirs without stat
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            if meta.permissions().mode() & 0o111 == 0 { return None; }
            let file_name = entry.file_name();
            let name = file_name.to_str()?.to_string();
            Some((name.clone(), format!("{}/{}", dir, name)))
        })
        .collect()
}

pub fn build_executables() -> ExecutableMap {
    let paths = std::env::var("PATH").unwrap_or_default();
    let dirs: Vec<&str> = paths.split(':').collect();

    // scan all PATH dirs in parallel
    let dir_results: Vec<Vec<(String, String)>> = dirs.par_iter()
        .map(|&dir| scan_dir(dir))
        .collect();

    // pre-allocate exact capacity before inserting
    let total: usize = dir_results.iter().map(|v| v.len()).sum();
    let mut map = ExecutableMap::default();
    map.reserve(total);

    // merge in PATH order so first occurrence wins
    for entries in dir_results {
        for (name, path) in entries {
            map.entry(name).or_insert(path);
        }
    }
    map
}
