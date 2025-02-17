use std::{env, fs};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use crate::conf::OrdMode;

#[cfg(target_family = "windows")]
fn is_executable(path: &Path) -> bool {
    path.extension()
        .map(|ext| ext == "exe" || ext == "bat" || ext == "cmd" || ext == "com")
        .unwrap_or(false)
}

#[cfg(not(target_family = "windows"))]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Recursively searches the directories in PATH for executables matching any name in `target_filenames`.
#[must_use]
pub fn find_matching_executables(target_filenames: &[&str]) -> Vec<String> {
    let mut executables = Vec::new();

    if let Ok(paths) = env::var("PATH") {
        for path in env::split_paths(&paths) {
            if path.exists() {
                for entry in WalkDir::new(path).into_iter().flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_file() && is_executable(entry_path) {
                        if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                            if target_filenames.contains(&file_name)
                                && !executables.contains(&entry_path.display().to_string())
                            {
                                executables.push(entry_path.display().to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    executables
}

/// Reorders the list of candidate package managers based on the provided ordering mode.
pub fn reorder_candidates(candidates: Vec<PathBuf>, ord_mode: &OrdMode, verbose: bool) -> Vec<PathBuf> {
    match ord_mode {
        OrdMode::Specified(order_vec) => {
            if verbose {
                println!("Reordering package managers using specified order: {order_vec:?}");
            }
            // Map each package manager name to its order index.
            let order_map: HashMap<String, usize> = order_vec
                .iter()
                .enumerate()
                .map(|(i, pm)| (pm.to_string(), i))
                .collect();
            let mut enumerated: Vec<(usize, PathBuf)> =
                candidates.into_iter().enumerate().collect();
            enumerated.sort_by_key(|(orig_index, candidate)| {
                let pm_name = candidate.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if let Some(&order_index) = order_map.get(pm_name) {
                    (0, order_index)
                } else {
                    (1, *orig_index)
                }
            });
            enumerated
                .into_iter()
                .map(|(_, candidate)| candidate)
                .collect()
        }
        OrdMode::Interactive => {
            println!("Interactive ordering mode enabled.");
            println!("Detected package managers:");
            for (i, candidate) in candidates.iter().enumerate() {
                if let Some(pm_name) = candidate.file_name().and_then(|s| s.to_str()) {
                    println!("  {i}: {pm_name}");
                }
            }
            println!("Enter the desired update order as comma-separated indices (e.g. 2,0,1) or press Enter to keep the current order:");
            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .expect("Failed to read line");
            let input = input.trim();
            if input.is_empty() {
                if verbose {
                    println!("No input provided, keeping original order.");
                }
                return candidates;
            }
            let indices: Vec<usize> = input
                .split(',')
                .filter_map(|s| s.trim().parse::<usize>().ok())
                .collect();
            if verbose {
                println!("Specified indices: {indices:?}");
            }
            let mut ordered = Vec::new();
            let mut selected_indices = HashSet::new();
            for idx in indices {
                if idx < candidates.len() {
                    ordered.push(candidates[idx].clone());
                    selected_indices.insert(idx);
                }
            }
            // Append any candidates not specified, preserving original order.
            for (i, candidate) in candidates.into_iter().enumerate() {
                if !selected_indices.contains(&i) {
                    ordered.push(candidate);
                }
            }
            ordered
        }
    }
}

/// Checks if a directory contains a file with the given name.
pub fn p_cont<P: AsRef<Path>>(dir: P, file_name: &str) -> std::io::Result<bool> {
    let dir = dir.as_ref();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                if name == file_name {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

pub fn p_cont_ext<P: AsRef<Path>>(dir: P, extension: &str) -> Option<std::io::Result<String>> {
    let dir = dir.as_ref();
    if dir.is_dir() {
        for entry in fs::read_dir(dir).ok()? {
            let entry = entry.ok()?;
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(extension) {
                    return Some(Ok(name.to_string()));
                }
            }
        }
    }
    None
}

#[must_use]
pub fn format_list(pkgs: &[String]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let last = pkgs.len() - 1;
    for (i, pkg) in pkgs.iter().enumerate() {
        if i == last && last != 0 {
            write!(out, "and \"{pkg}\"").unwrap();
        } else if i == last {
            write!(out, "\"{pkg}\"").unwrap();
        } else {
            write!(out, "\"{pkg}\", ").unwrap();
        }
    }
    out
}
