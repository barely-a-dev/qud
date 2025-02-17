#![allow(clippy::doc_markdown)]

mod conf;
mod helpers;
mod self_up;

use conf::Config;
use helpers::{find_matching_executables, format_list, p_cont, p_cont_ext, reorder_candidates};

use std::collections::{HashMap, HashSet};
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// Supported package managers:
// Linux: pacman, yay, apt, apt-get, dnf, zypper, snap, flatpak, xbps-install, apk, emerge, guix, nix, yum, eopkg, cave, sbopkg, scratch
// Windows: choco, scoop, winget, Windows itself (via PowerShell)
// General: rustup, brew, port (MacPorts), pkg (FreeBSD), cargo, npm, pip, composer, gem, conda, poetry, nuget, asdf, vcpkg, conan, stack, opam, mix, sdkman,
// gvm, pnpm, yarn, maven, go,
// and qud itself.
const PM: [&str; 47] = [
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
    "qud",
    "windowsupdate",
];

#[allow(clippy::too_many_lines)]
fn main() {
    let config = Config::parse_args();

    // Find matching executables in PATH (as raw candidates, possibly with duplicates).
    let raw_candidates: Vec<PathBuf> = find_matching_executables(&PM)
        .into_iter()
        .map(PathBuf::from)
        .collect();

    // Group candidates by their file name (usually package manager name)
    let mut grouped: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for candidate in &raw_candidates {
        if let Some(pm_name) = candidate.file_name().and_then(|s| s.to_str()) {
            grouped
                .entry(pm_name.to_string())
                .or_default()
                .push(candidate.clone());
        }
    }

    // Now, choose one candidate per package manager.
    // We preserve the order in which they were found in PATH.
    let mut unique_candidates = Vec::new();
    let mut seen = HashSet::new();
    for candidate in raw_candidates {
        if let Some(pm_name) = candidate.file_name().and_then(|s| s.to_str()) {
            if seen.contains(pm_name) {
                continue;
            }
            seen.insert(pm_name.to_string());
            if let Some(dups) = grouped.get(pm_name) {
                if dups.len() > 1 && config.verbose {
                    eprintln!(
                        "\x1b[93mWarning:\x1b[0m Multiple installations of {} found: {}. Using {}. Use --spec {}::/path/to/executable to override this. \x1b[93mIgnore if --spec was already specified.\x1b[0m",
                        pm_name,
                        format_list(&dups.iter()
                            .map(|p| p.display().to_string())
                            .collect::<Vec<_>>()),
                        candidate.display(),
                        pm_name
                    );
                }
            }
            unique_candidates.push(candidate);
        }
    }

    if config.verbose {
        println!(
            "Found {} candidate package manager executable(s).",
            unique_candidates.len()
        );
    }

    // If --list was passed, list found package managers and exit.
    if config.list {
        println!("Detected package managers:");
        for path in &unique_candidates {
            if let Some(pm_name) = path.file_name().and_then(|s| s.to_str()) {
                println!("  {} ({})", pm_name, path.display());
            }
        }
        return;
    }

    // Apply any overrides from --spec. If a spec is provided for a package manager,
    // use that instead of the detected candidate. Also add any --spec entries that were not detected.
    let mut final_candidates = Vec::new();
    let mut used_pm_names = HashSet::new();
    for candidate in unique_candidates {
        if let Some(pm_name) = candidate.file_name().and_then(|s| s.to_str()) {
            used_pm_names.insert(pm_name.to_string());
            if let Some(spec_path) = config.specs.get(pm_name) {
                if config.verbose {
                    println!(
                        "Overriding {} with specified executable: {}",
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
                    "Adding specified executable for {} not found in PATH: {}",
                    pm,
                    spec_path.display()
                );
            }
            final_candidates.push(spec_path.clone());
        }
    }

    // Reorder final candidates if --ord was provided.
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
                    println!("Adding Windows update.");
                }
                final_candidates.push(PathBuf::from("windowsupdate"));
            }
        }
    }

    let planned_updates: Vec<_> = final_candidates
        .iter()
        .filter_map(|candidate| {
            if let Some(pm_name) = candidate.file_name().and_then(|s| s.to_str()) {
                // Respect --only option.
                if let Some(ref only_list) = config.only {
                    if !only_list.iter().any(|s| s == pm_name) {
                        return None;
                    }
                }
                // Skip fully excluded package managers.
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
        println!("Proceed with these updates? (Y/n): ");
        std::io::stdout().flush().unwrap();
        let mut confirm = String::new();
        std::io::stdin()
            .read_line(&mut confirm)
            .expect("Failed to read line");
        let confirm = confirm.trim().to_lowercase();
        if confirm == "n" || confirm == "no" {
            println!("Update cancelled by user.");
            std::process::exit(0);
        }
    }

    let current_dir = env::current_dir().unwrap_or_else(|_| "/".into());

    // Process each final candidate package manager.
    for package_manager in final_candidates {
        let Some(pm_name) = package_manager.file_name().and_then(|s| s.to_str()) else {
            continue;
        };

        // If --only was used, skip those not specified.
        if let Some(ref only_list) = config.only {
            if !only_list.iter().any(|s| s == pm_name) {
                if config.verbose {
                    println!("Skipping {pm_name} because it is not in the --only list.");
                }
                continue;
            }
        }

        // Check for full exclusion (i.e. --excl used with just a package manager name)
        if let Some(exclusions) = config.exclusions.get(pm_name) {
            if exclusions.is_empty() {
                if config.verbose {
                    println!("Skipping {pm_name} because it is fully excluded via --excl");
                }
                continue;
            }
        }

        // Get any extra exclusion and extension arguments for this package manager.
        let mut extra_args = config.get_exclusion_args(pm_name);
        extra_args.extend(config.get_ext_args(pm_name));

        process_pm(
            pm_name,
            config.auto,
            &current_dir,
            &extra_args,
            config.dry_run,
        );
    }
}

#[allow(clippy::too_many_lines)]
fn process_pm(pm_name: &str, auto: bool, current_dir: &Path, extra_args: &[String], dry_run: bool) {
    println!(
        "\x1b[94mINFO: Processing package manager: {} in directory: {}\x1b[0m",
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
            // First set execution policy for current user
            let setup_commands = [
                // Set execution policy for current user
                "Set-ExecutionPolicy -Scope CurrentUser RemoteSigned -Force",
                // Check if PSWindowsUpdate is installed, install if not
                "if (!(Get-Module -ListAvailable -Name PSWindowsUpdate)) { Install-Module -Name PSWindowsUpdate -Force -Scope CurrentUser }",
                // Import it
                "Import-Module PSWindowsUpdate"
            ].join("; ");
            
            let setup_result = Command::new("powershell")
                .args(&[
                    "-Command",
                    "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile -Command &{",
                    &setup_commands,
                    "}'"
                ])
                .status();
            
            if let Err(e) = setup_result {
                eprintln!("Failed to setup Windows Update environment: {}", e);
                return;
            }

            let update_command = if auto {
                "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile -Command &{Install-WindowsUpdate -AcceptAll -AutoReboot}'"
            } else {
                "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile -Command &{Install-WindowsUpdate}'"
            };

            upd("powershell", &["-Command", update_command], false, extra_args, dry_run);
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
        "qud" => {
            upd("qud", &["--self-update"], true, extra_args, dry_run);
        }
        _ => eprintln!("\x1b[93mWarning:\x1b[0m Unknown package manager: {pm_name}"),
    }
}

/// Combines the base arguments and any extra (exclusion/extension) arguments, then
/// executes (or prints in dry-run mode) the update command.
///
/// `use_sudo` indicates whether to prefix the command with "sudo".
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
        format!("runas /user:Administrator \"{} {}\"", command, args.join(" "))
    } else {
        format!("{} {}", command, args.join(" "))
    };

    if dry_run {
        println!("Dry run: {cmd_str}");
        return;
    }

    println!("\x1b[94mINFO: Executing command: {cmd_str}\x1b[0m");
    match gen_upd_cmd(command, &args, use_sudo).status() {
        Ok(es) => {
            if es.success() {
                println!("\x1b[94mINFO: Successfully updated with {command}, exited with status {es}\x1b[0m");
            } else {
                println!(
                    "\x1b[91mERR:\x1b[0m Failed to update with {command}, exited with status: {es}"
                );
            }
        }
        Err(e) => eprintln!("\x1b[91mERR:\x1b[0m Failed to update with {command}, error: {e}"),
    }
}

/// Creates a Command configured with the given arguments and inherited I/O settings.
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
