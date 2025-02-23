#![allow(clippy::doc_markdown)]

mod conf;
mod helpers;
mod self_up;

use conf::Config;
use helpers::{find_matching_executables, format_list, p_cont, p_cont_ext, reorder_candidates};

use colored::Colorize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// Supported package managers:
// Linux: pacman, yay, apt, apt-get, dnf, zypper, snap, flatpak, xbps-install, apk, emerge, guix, nix, yum, eopkg, cave, sbopkg, scratch
// Windows: choco, scoop, winget, Windows itself (via PowerShell)
// General: rustup, brew, port (MacPorts), pkg (FreeBSD), cargo, npm, pip, composer, gem, conda, poetry, nuget, asdf, vcpkg, conan, stack, opam, mix, sdkman,
// gvm, pnpm, yarn, maven, and go
const PM: [&str; 46] = [
    "pacman",
    "yay",
    "apt",
    "apt-get",
    "dnf",
    "zypper",
    "snap",
    "flatpak",
    "xbps-install",
    "choco",
    "scoop",
    "winget",
    "rustup",
    "brew",
    "apk",
    "nix",
    "emerge",
    "guix",
    "yum",
    "port",
    "pkg",
    "eopkg",
    "cargo",
    "npm",
    "pip",
    "composer",
    "gem",
    "conda",
    "poetry",
    "nuget",
    "asdf",
    "vcpkg",
    "conan",
    "stack",
    "opam",
    "mix",
    "sdkman",
    "gvm",
    "pnpm",
    "yarn",
    "maven",
    "go",
    "cave",
    "sbopkg",
    "scratch",
    "windowsupdate",
];

#[allow(clippy::too_many_lines)]
fn main() {
    let config = Config::parse_args();

    let mut seen = HashMap::new();
    let mut duplicates: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut candidates = Vec::new();
    for candidate in find_matching_executables(&PM)
        .into_iter()
        .map(PathBuf::from)
    {
        if let Some(pm_name) = candidate.file_name().and_then(|s| s.to_str()) {
            if seen.contains_key(pm_name) {
                duplicates
                    .entry(pm_name.to_string())
                    .or_default()
                    .push(candidate.clone());
            } else {
                seen.insert(pm_name.to_string(), candidate.clone());
                candidates.push(candidate);
            }
        }
    }

    if config.verbose {
        for candidate in &candidates {
            if let Some(pm_name) = candidate.file_name().and_then(|s| s.to_str()) {
                if !config.specs.contains_key(pm_name) {
                    if let Some(dups) = duplicates.get(pm_name) {
                        let mut all_paths = vec![candidate.display().to_string()];
                        all_paths.extend(dups.iter().map(|p| p.display().to_string()));
                        eprintln!(
                            "{} Multiple installations of {} found: {}. Using {}. Use --spec {}::/path/to/executable to override this.",
                            "Warning:".yellow(),
                            pm_name,
                            format_list(&all_paths),
                            candidate.display(),
                            pm_name
                        );
                    }
                }
            }
        }
    }

    if config.verbose {
        println!(
            "{} Found {} candidate package manager executable(s).",
            "INFO:".blue(),
            candidates.len()
        );
    }

    if config.list {
        println!("Detected package managers:");
        for path in &candidates {
            if let Some(pm_name) = path.file_name().and_then(|s| s.to_str()) {
                println!("  {} ({})", pm_name, path.display());
            }
        }
        return;
    }

    let mut final_candidates = Vec::new();
    let mut used_pm_names = HashSet::new();
    for candidate in candidates {
        if let Some(pm_name) = candidate.file_name().and_then(|s| s.to_str()) {
            used_pm_names.insert(pm_name.to_string());
            if let Some(spec_path) = config.specs.get(pm_name) {
                if config.verbose {
                    println!(
                        "{} Overriding {} with specified executable: {}",
                        "INFO:".blue(),
                        pm_name,
                        spec_path.display()
                    );
                }
                final_candidates.push(spec_path.clone());
            } else {
                final_candidates.push(candidate);
            }
        }
    }
    for (pm, spec_path) in &config.specs {
        if !used_pm_names.contains(pm) {
            if config.verbose {
                println!(
                    "{} Adding specified executable for {} not found in PATH: {}",
                    "INFO:".blue(),
                    pm,
                    spec_path.display()
                );
            }
            final_candidates.push(spec_path.clone());
        }
    }

    #[allow(unused_mut)]
    let mut final_candidates = if let Some(ref ord_mode) = config.ord {
        reorder_candidates(final_candidates, ord_mode, config.verbose)
    } else {
        final_candidates
    };

    #[cfg(target_os = "windows")]
    {
        let include_windowsupdate = match &config.only {
            Some(only_list) => only_list.iter().any(|s| s == "windowsupdate"),
            None => true,
        };
        if include_windowsupdate {
            if !final_candidates.iter().any(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "windowsupdate")
                    .unwrap_or(false)
            }) {
                if config.verbose {
                    println!("{} Adding Windows update.", "INFO:".blue());
                }
                final_candidates.push(PathBuf::from("windowsupdate"));
            }
        }
    }

    let planned_updates: Vec<String> = final_candidates
        .iter()
        .filter_map(|candidate| {
            if let Some(pm_name) = candidate.file_name().and_then(|s| s.to_str()) {
                if let Some(ref only_list) = config.only {
                    if !only_list.iter().any(|s| s == pm_name) {
                        return None;
                    }
                }
                if let Some(exclusions) = config.exclusions.get(pm_name) {
                    if exclusions.is_empty() {
                        return None;
                    }
                }
                Some(format!("{} ({})", pm_name, candidate.display()))
            } else {
                None
            }
        })
        .collect();

    println!("Updating with:");
    for item in &planned_updates {
        println!("  {item}");
    }

    if !config.auto && !config.noconfirm {
        let updatable: Vec<(PathBuf, String)> = final_candidates
            .iter()
            .filter_map(|candidate| {
                if let Some(pm_name) = candidate.file_name().and_then(|s| s.to_str()) {
                    if let Some(ref only_list) = config.only {
                        if !only_list.iter().any(|s| s == pm_name) {
                            return None;
                        }
                    }
                    if let Some(exclusions) = config.exclusions.get(pm_name) {
                        if exclusions.is_empty() {
                            return None;
                        }
                    }
                    Some((candidate.clone(), pm_name.to_string()))
                } else {
                    None
                }
            })
            .collect();

        if !updatable.is_empty() {
            println!("{} Detected package managers to update:", "INFO:".blue());
            for (i, (_candidate, pm_name)) in updatable.iter().enumerate() {
                println!("  {}. {}", i + 1, pm_name);
            }
            println!(
                "{} Enter numbers of package managers to skip (space separated), or press Enter to proceed:",
                "INFO:".blue()
            );
            std::io::stdout().flush().unwrap();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            let skip_numbers: Vec<usize> = input
                .split_whitespace()
                .filter_map(|s| s.parse::<usize>().ok())
                .collect();
            let skip_set: HashSet<usize> = skip_numbers.into_iter().map(|n| n - 1).collect();
            let skip_pm_names: HashSet<String> = updatable
                .iter()
                .enumerate()
                .filter(|(i, _)| skip_set.contains(i))
                .map(|(_, (_, pm_name))| pm_name.clone())
                .collect();
            final_candidates.retain(|candidate| {
                if let Some(pm_name) = candidate.file_name().and_then(|s| s.to_str()) {
                    !skip_pm_names.contains(pm_name)
                } else {
                    true
                }
            });
            println!("{} Proceeding with updates for:", "INFO:".blue());
            for candidate in &final_candidates {
                if let Some(pm_name) = candidate.file_name().and_then(|s| s.to_str()) {
                    println!("  {} ({})", pm_name, candidate.display());
                }
            }
        }
    }

    for package_manager in final_candidates {
        let Some(pm_name) = package_manager.file_name().and_then(|s| s.to_str()) else {
            continue;
        };

        if let Some(ref only_list) = config.only {
            if !only_list.iter().any(|s| s == pm_name) {
                if config.verbose {
                    println!(
                        "{} Skipping {} because it is not in the --only list.",
                        "INFO:".blue(),
                        pm_name
                    );
                }
                continue;
            }
        }

        if let Some(exclusions) = config.exclusions.get(pm_name) {
            if exclusions.is_empty() {
                if config.verbose {
                    println!(
                        "{} Skipping {} because it is fully excluded via --excl",
                        "INFO:".blue(),
                        pm_name
                    );
                }
                continue;
            }
        }

        let mut extra_args = config.get_exclusion_args(pm_name);
        extra_args.extend(config.get_ext_args(pm_name));

        process_pm(
            pm_name,
            config.auto,
            #[cfg(not(target_os = "windows"))]
            &env::current_dir().unwrap_or_else(|_| "/".into()),
            #[cfg(target_os = "windows")]
            &env::current_dir().unwrap_or_else(|_| "C:\\".into()),
            &extra_args,
            config.dry_run,
        );
    }
}

#[allow(clippy::too_many_lines)]
fn process_pm(pm_name: &str, auto: bool, current_dir: &Path, extra_args: &[String], dry_run: bool) {
    println!(
        "{} Processing package manager: {} in directory: {}",
        "INFO:".blue(),
        pm_name,
        current_dir.display()
    );
    match pm_name {
        "pacman" => {
            let args: &[&str] = if auto {
                &["-Syu", "--noconfirm"]
            } else {
                &["-Syu"]
            };
            upd("pacman", args, true, extra_args, dry_run);
        }
        "yay" => {
            let args: &[&str] = if auto {
                &[
                    "-Syu",
                    "--noconfirm",
                    "--answerdiff",
                    "None",
                    "--answerclean",
                    "None",
                ]
            } else {
                &["-Syu"]
            };
            upd("yay", args, false, extra_args, dry_run);
        }
        "apt" | "apt-get" => {
            upd(pm_name, &["update"], true, extra_args, dry_run);
            let upgrade_args: &[&str] = if auto {
                &["upgrade", "-y"]
            } else {
                &["upgrade"]
            };
            upd(pm_name, upgrade_args, true, extra_args, dry_run);
        }
        "dnf" => {
            let args: &[&str] = if auto {
                &["upgrade", "--refresh", "-y"]
            } else {
                &["upgrade", "--refresh"]
            };
            upd("dnf", args, true, extra_args, dry_run);
        }
        "zypper" => {
            let args: &[&str] = if auto {
                &["--non-interactive", "update"]
            } else {
                &["update"]
            };
            upd("zypper", args, true, extra_args, dry_run);
        }
        "snap" => {
            upd("snap", &["refresh"], true, extra_args, dry_run);
        }
        "flatpak" => {
            let args: &[&str] = if auto { &["update", "-y"] } else { &["update"] };
            upd("flatpak", args, false, extra_args, dry_run);
        }
        "xbps-install" => {
            let args: &[&str] = if auto { &["-Syu", "--yes"] } else { &["-Syu"] };
            upd("xbps-install", args, true, extra_args, dry_run);
        }
        "choco" => {
            let args: &[&str] = if auto {
                &["upgrade", "all", "-y"]
            } else {
                &["upgrade", "all"]
            };
            upd("choco", args, false, extra_args, dry_run);
        }
        "scoop" => {
            upd("scoop", &["update", "*"], false, extra_args, dry_run);
        }
        "winget" => {
            let args: &[&str] = if auto {
                &[
                    "upgrade",
                    "--all",
                    "--accept-source-agreements",
                    "--accept-package-agreements",
                ]
            } else {
                &["upgrade", "--all"]
            };
            upd("winget", args, false, extra_args, dry_run);
        }
        #[cfg(target_os = "windows")]
        "windowsupdate" => {
            let setup_commands = [
                "Set-ExecutionPolicy -Scope CurrentUser RemoteSigned -Force",
                "if (!(Get-Module -ListAvailable -Name PSWindowsUpdate)) { Install-Module -Name PSWindowsUpdate -Force -Scope CurrentUser }",
                "Import-Module PSWindowsUpdate",
                if auto {
                    "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile -Command &{Install-WindowsUpdate -AcceptAll -AutoReboot}'"
                } else {
                    "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile -Command &{Install-WindowsUpdate}'"
                }
            ].join("; ");

            upd(
                "powershell",
                &["-Command", &setup_commands],
                false,
                extra_args,
                dry_run,
            );
        }
        "rustup" => {
            upd("rustup", &["update"], false, extra_args, dry_run);
        }
        "brew" => {
            upd("brew", &["update"], false, extra_args, dry_run);
            upd("brew", &["upgrade"], false, extra_args, dry_run);
        }
        "apk" => {
            upd("apk", &["update"], true, extra_args, dry_run);
            upd("apk", &["upgrade"], true, extra_args, dry_run);
        }
        "nix" => {
            upd("nix-channel", &["--update"], false, extra_args, dry_run);
            upd("nix-env", &["-u", "*"], false, extra_args, dry_run);
        }
        "emerge" => {
            upd("emerge", &["--sync"], true, extra_args, dry_run);
            let args: &[&str] = if auto {
                &["-uDN", "@world"]
            } else {
                &["-avuDN", "@world"]
            };
            upd("emerge", args, true, extra_args, dry_run);
        }
        "guix" => {
            upd("guix", &["pull"], false, extra_args, dry_run);
            upd(
                "guix",
                &["package", "--upgrade"],
                false,
                extra_args,
                dry_run,
            );
        }
        "yum" => {
            let args: &[&str] = if auto { &["update", "-y"] } else { &["update"] };
            upd("yum", args, true, extra_args, dry_run);
        }
        "port" => {
            upd("port", &["selfupdate"], true, extra_args, dry_run);
            upd("port", &["upgrade", "outdated"], true, extra_args, dry_run);
        }
        "pkg" => {
            upd("pkg", &["update"], true, extra_args, dry_run);
            let args: &[&str] = if auto {
                &["upgrade", "-y"]
            } else {
                &["upgrade"]
            };
            upd("pkg", args, true, extra_args, dry_run);
        }
        "eopkg" => {
            upd("eopkg", &["update-repo"], true, extra_args, dry_run);
            let args: &[&str] = if auto {
                &["upgrade", "-y"]
            } else {
                &["upgrade"]
            };
            upd("eopkg", args, true, extra_args, dry_run);
        }
        "cargo" => {
            if p_cont(current_dir, "Cargo.toml").unwrap_or(false) {
                upd("cargo", &["update"], false, extra_args, dry_run);
            }
        }
        "npm" => {
            if p_cont(current_dir, "package.json").unwrap_or(false) {
                upd("npm", &["update"], false, extra_args, dry_run);
            }
        }
        "pip" => {
            if p_cont(current_dir, "requirements.txt").unwrap_or(false) {
                upd(
                    "pip",
                    &["install", "--upgrade", "-r", "requirements.txt"],
                    false,
                    extra_args,
                    dry_run,
                );
            }
        }
        "composer" => {
            if p_cont(current_dir, "composer.json").unwrap_or(false) {
                upd("composer", &["update"], false, extra_args, dry_run);
            }
        }
        "gem" => {
            upd(
                "gem",
                &["update", "--no-document"],
                false,
                extra_args,
                dry_run,
            );
        }
        "conda" => {
            upd(
                "conda",
                &["update", "--all", "-y"],
                true,
                extra_args,
                dry_run,
            );
        }
        "poetry" => {
            upd("poetry", &["update"], false, extra_args, dry_run);
        }
        "nuget" => {
            if p_cont(current_dir, "packages.config").unwrap_or(false) {
                upd(
                    "nuget",
                    &["update", "packages.config"],
                    false,
                    extra_args,
                    dry_run,
                );
            } else if let Some(Ok(f)) = p_cont_ext(current_dir, ".sln") {
                upd("nuget", &["update", &f], false, extra_args, dry_run);
            }
        }
        "asdf" => {
            upd("asdf", &["update"], false, extra_args, dry_run);
            upd(
                "asdf",
                &["plugin-update", "--all"],
                false,
                extra_args,
                dry_run,
            );
        }
        "vcpkg" => {
            let args: &[&str] = if auto { &["upgrade"] } else { &["update"] };
            upd("vcpkg", args, false, extra_args, dry_run);
        }
        "conan" => {
            if p_cont(current_dir, "conanfile.txt").unwrap_or(false)
                || p_cont(current_dir, "conanfile.py").unwrap_or(false)
            {
                upd(
                    "conan",
                    &["install", ".", "--update"],
                    false,
                    extra_args,
                    dry_run,
                );
            }
        }
        "stack" => {
            if p_cont(current_dir, "stack.yaml").unwrap_or(false) {
                upd("stack", &["update"], false, extra_args, dry_run);
                upd("stack", &["upgrade"], false, extra_args, dry_run);
            }
        }
        "opam" => {
            upd("opam", &["update"], false, extra_args, dry_run);
            let args: &[&str] = if auto {
                &["upgrade", "-y"]
            } else {
                &["upgrade"]
            };
            upd("opam", args, false, extra_args, dry_run);
        }
        "mix" => {
            if p_cont(current_dir, "mix.exs").unwrap_or(false) {
                upd("mix", &["deps.update", "--all"], false, extra_args, dry_run);
            }
        }
        "sdkman" => {
            upd("sdkman", &["update"], false, extra_args, dry_run);
        }
        "gvm" => {
            upd("gvm", &["update"], false, extra_args, dry_run);
        }
        "pnpm" => {
            if p_cont(current_dir, "package.json").unwrap_or(false) {
                upd("pnpm", &["update"], false, extra_args, dry_run);
            }
        }
        "yarn" => {
            if p_cont(current_dir, "yarn.lock").unwrap_or(false) {
                upd("yarn", &["upgrade"], false, extra_args, dry_run);
            }
        }
        "maven" => {
            if p_cont(current_dir, "pom.xml").unwrap_or(false) {
                let args: &[&str] = if auto {
                    &["versions:use-latest-releases"]
                } else {
                    &["versions:display-dependency-updates"]
                };
                upd("mvn", args, false, extra_args, dry_run);
            }
        }
        "go" => {
            if p_cont(current_dir, "go.mod").unwrap_or(false) {
                upd("go", &["get", "-u", "./..."], false, extra_args, dry_run);
            }
        }
        "cave" => {
            upd("cave", &["sync"], true, extra_args, dry_run);
            let args: &[&str] = if auto {
                &["upgrade", "--non-interactive"]
            } else {
                &["upgrade"]
            };
            upd("cave", args, true, extra_args, dry_run);
        }
        "sbopkg" => {
            upd("sbopkg", &["-r"], true, extra_args, dry_run);
            let args: &[&str] = if auto {
                &["-i", "--non-interactive"]
            } else {
                &["-i"]
            };
            upd("sbopkg", args, true, extra_args, dry_run);
        }
        "scratch" => {
            let args: &[&str] = if auto {
                &["update", "--non-interactive"]
            } else {
                &["update"]
            };
            upd("scratch", args, true, extra_args, dry_run);
        }
        _ => eprintln!(
            "{} Unknown package manager: {}",
            "Warning:".yellow(),
            pm_name
        ),
    }
}

fn upd(command: &str, base_args: &[&str], use_sudo: bool, extra_args: &[String], dry_run: bool) {
    let mut args: Vec<String> = base_args.iter().map(ToString::to_string).collect();
    args.extend_from_slice(extra_args);

    #[cfg(not(target_os = "windows"))]
    let cmd_str = if use_sudo {
        format!("sudo {} {}", command, args.join(" "))
    } else {
        format!("{} {}", command, args.join(" "))
    };

    #[cfg(target_os = "windows")]
    let cmd_str = if use_sudo {
        format!(
            "runas /user:Administrator \"{} {}\"",
            command,
            args.join(" ")
        )
    } else {
        format!("{} {}", command, args.join(" "))
    };

    if dry_run {
        println!("Dry run: {cmd_str}");
        return;
    }

    println!("{} Executing command: {cmd_str}", "INFO:".blue());
    match gen_upd_cmd(command, &args, use_sudo).status() {
        Ok(es) => {
            if es.success() {
                println!(
                    "{} Successfully updated with {}, exited with status {}",
                    "INFO:".blue(),
                    command,
                    es
                );
            } else {
                println!(
                    "{} Failed to update with {}, exited with status: {}",
                    "ERR:".red(),
                    command,
                    es
                );
            }
        }
        Err(e) => eprintln!(
            "{} Failed to update with {}, error: {}",
            "ERR:".red(),
            command,
            e
        ),
    }
}

#[must_use]
pub fn gen_upd_cmd(command: &str, args: &[String], use_sudo: bool) -> Command {
    #[cfg(target_family = "windows")]
    {
        let mut cmd = if use_sudo {
            let mut c = Command::new("runas");
            c.arg("/user:Administrator")
                .arg(format!("{} {}", command, args.join(" ")));
            c
        } else {
            Command::new(command)
        };
        cmd.args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdin(Stdio::inherit());
        cmd
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut cmd = if use_sudo {
            let mut c = Command::new("sudo");
            c.arg(command);
            c
        } else {
            Command::new(command)
        };
        cmd.args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdin(Stdio::inherit());
        cmd
    }
}
