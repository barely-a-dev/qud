#![allow(clippy::doc_markdown)]

mod self_up;

use pico_args::Arguments;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::WalkDir;

// Supported package managers:
// Linux: pacman, yay, apt, apt-get, dnf, zypper, snap, flatpak, xbps-install, apk, emerge, guix, nix, yum, eopkg, cave, sbopkg, scratch
// Windows: choco, scoop, winget
// General: rustup, brew, port (MacPorts), pkg (FreeBSD), cargo, npm, pip, composer, gem, conda, poetry, nuget, asdf, vcpkg, conan, stack, opam, mix, sdkman
// gvm, pnpm, yarn, maven, go
// and qud itself.
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
    "qud",
];

/// Determines how to order package manager updates.
#[derive(Debug)]
enum OrdMode {
    /// The user did not provide an explicit order – ask interactively.
    Interactive,
    /// The user provided a comma-separated list of package manager names.
    Specified(Vec<String>),
}

/// Holds runtime configuration derived from command-line arguments.
#[allow(clippy::struct_excessive_bools)]
struct Config {
    exclusions: HashMap<String, Vec<String>>,
    /// If provided, update only these package managers (by name).
    only: Option<Vec<String>>,
    /// Specifications to override the detected executable for a package manager.
    /// Format: "pm::/path/to/executable"
    specs: HashMap<String, PathBuf>,
    /// Auto mode uses non-interactive flags (if available) for each package manager.
    auto: bool,
    noconfirm: bool,
    /// Verbose mode prints extra logging information.
    verbose: bool,
    /// List mode prints found package managers without performing any updates.
    list: bool,
    dry_run: bool,
    /// Extra flags to pass to package managers. Format: "pm::<flags>"
    exts: HashMap<String, Vec<String>>,
    /// Optional ordering of updates.
    ord: Option<OrdMode>,
}

impl Config {
    #[allow(clippy::too_many_lines)]
    fn parse_args() -> Config {
        let mut pargs = Arguments::from_env();

        // Help, version, and self updating.
        if pargs.contains(["-h", "--help"]) {
            Self::print_help();
            std::process::exit(0);
        }
        if pargs.contains(["-V", "--version"]) {
            println!("qud v1.3.8");
            std::process::exit(0);
        }
        if pargs.contains(["-S", "--self-update"]) {
            if !self_up::perm::is_elevated() {
                eprintln!("Program must be run as root to update.");
                std::process::exit(2);
            }
            println!("Updating qud...");
            match self_up::self_update() {
                Ok(()) => println!("qud updated successfully, exiting."),
                Err(e) => eprintln!("qud failed to update: {e}, exiting."),
            }
            std::process::exit(-1);
        }

        let dry_run = pargs.contains(["-d", "--dry"]);
        let excl_values: Vec<String> = pargs
            .values_from_str(["-e", "--excl"])
            .unwrap_or_else(|_| Vec::new());
        let mut exclusions = HashMap::new();
        for excl in excl_values {
            Self::add_exclusion(&mut exclusions, &excl);
        }
        let auto = pargs.contains(["-a", "--auto"]);
        let noconfirm = pargs.contains(["-n", "--noconfirm"]);
        let verbose = pargs.contains(["-v", "--verbose"]);
        let list = pargs.contains(["-l", "--list"]);
        let only_values: Vec<String> = pargs
            .values_from_str(["-o", "--only"])
            .unwrap_or_else(|_| Vec::new());
        let only = if only_values.is_empty() {
            None
        } else {
            Some(only_values)
        };

        let spec_values: Vec<String> = pargs
            .values_from_str(["-s", "--spec"])
            .unwrap_or_else(|_| Vec::new());
        let mut specs = HashMap::new();
        for spec in spec_values {
            if spec.contains("::") {
                let sects: Vec<&str> = spec.split("::").collect();
                if sects.len() == 2 {
                    let pm = sects[0].to_string();
                    let path = PathBuf::from(sects[1]);
                    specs.insert(pm, path);
                } else {
                    eprintln!("\x1b[91mERR:\x1b[0m Invalid spec format: {spec}");
                }
            } else {
                eprintln!("\x1b[91mERR:\x1b[0m Invalid spec format: {spec}");
            }
        }

        let ext_values: Vec<String> = pargs
            .values_from_str(["-E", "--ext"])
            .unwrap_or_else(|_| Vec::new());
        let mut exts: HashMap<String, Vec<String>> = HashMap::new();
        for ext in ext_values {
            if let Some((pm, flags)) = ext.split_once("::") {
                let flags_vec: Vec<String> = flags
                    .split_whitespace()
                    .map(std::string::ToString::to_string)
                    .collect();
                exts.entry(pm.to_string()).or_default().extend(flags_vec);
            } else {
                eprintln!("\x1b[91mERR:\x1b[0m Invalid ext format: {ext}");
            }
        }

        let ord: Option<OrdMode> = if pargs.clone().contains(["-O", "--ord"]) {
            let ord_value: Option<String> =
                pargs.opt_value_from_str(["-O", "--ord"]).unwrap_or(None);

            match ord_value {
                Some(val) if !val.is_empty() => {
                    let order: Vec<String> = val
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    Some(OrdMode::Specified(order))
                }
                _ => Some(OrdMode::Interactive),
            }
        } else {
            None
        };

        Config {
            exclusions,
            only,
            specs,
            auto,
            noconfirm,
            verbose,
            list,
            dry_run,
            exts,
            ord,
        }
    }

    fn print_help() {
        println!(
            r#"qud v1.3.8

Usage:
  qud [options]

Options:
  --dry, -d           Dry run (print commands instead of executing).
  --excl, -e <s>      Exclude a package from a manager (format: pm::pkg) or a package manager entirely (format: pm). May be repeated.
  --auto, -a          Auto mode (use non-interactive flags where available).
  --verbose, -v       Enable verbose logging.
  --list, -l          List detected package managers without updating.
  --only, -o <pm>     Update only the specified package manager (may be repeated).
  --spec, -s <s>      Override the detected executable for a package manager (format: pm::/path/to/executable). May be repeated.
  --ext, -E <s>       Pass extra flags to a package manager (format: pm::"<flags>").
  --ord, -O [s]       Specify the update order. If provided a value (pm1,pm2,...), that order is used for those found; if no value is provided, you'll be prompted to sort.
  --help, -h          Show this help screen.
  --version, -V       Show version information.
  --self-update, -S   Update qud.
  --noconfirm, -n     Don't confirm when updating. Does not pass non-interactive flags to package managers.
"#
        );
    }

    /// Inserts an exclusion rule into the map.
    fn add_exclusion(map: &mut HashMap<String, Vec<String>>, excl: &str) {
        if excl.contains("::") {
            let parts: Vec<&str> = excl.split("::").collect();
            if parts.len() == 2 {
                let pm = parts[0].to_string();
                let pkg = parts[1].to_string();
                // If the package manager was already fully excluded (empty vec), keep it that way.
                map.entry(pm).or_default().push(pkg);
            } else {
                eprintln!("\x1b[91mERR:\x1b[0m Invalid exclusion format: {excl}");
            }
        } else {
            // Full exclusion: mark the package manager as entirely excluded by storing an empty Vec.
            map.insert(excl.to_string(), Vec::new());
        }
    }

    /// Returns extra arguments for the given package manager based on the exclusions map.
    fn get_exclusion_args(&self, pm: &str) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(pkgs) = self.exclusions.get(pm) {
            match pm {
                "pacman" | "xbps-install" => {
                    let joined = pkgs.join(", ");
                    args.push("--ignore".to_string());
                    args.push(joined);
                }
                "yay" => {
                    for pkg in pkgs {
                        args.push("--excludepkg".to_string());
                        args.push(pkg.clone());
                    }
                }
                "dnf" | "yum" | "zypper" => {
                    for pkg in pkgs {
                        args.push("--exclude".to_string());
                        args.push(pkg.clone());
                    }
                }
                p => {
                    eprintln!(
                        "\x1b[93mWARN:\x1b[0m {} does not support exclusions (or not yet implemented). The following packages ({}) will still be updated.",
                        p,
                        format_list(pkgs)
                    );
                }
            }
        }
        args
    }

    /// Returns extra flags for the given package manager passed via --ext.
    fn get_ext_args(&self, pm: &str) -> Vec<String> {
        self.exts.get(pm).cloned().unwrap_or_default()
    }
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
    let final_candidates = if let Some(ref ord_mode) = config.ord {
        reorder_candidates(final_candidates, ord_mode, config.verbose)
    } else {
        final_candidates
    };

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

/// Checks if a directory contains a file with the given name.
fn p_cont<P: AsRef<Path>>(dir: P, file_name: &str) -> std::io::Result<bool> {
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

fn p_cont_ext<P: AsRef<Path>>(dir: P, extension: &str) -> Option<std::io::Result<String>> {
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

/// Combines the base arguments and any extra (exclusion/extension) arguments, then
/// executes (or prints in dry-run mode) the update command.
///
/// `use_sudo` indicates whether to prefix the command with "sudo".
fn upd(command: &str, base_args: &[&str], use_sudo: bool, extra_args: &[String], dry_run: bool) {
    let mut args: Vec<String> = base_args.iter().map(ToString::to_string).collect();
    args.extend_from_slice(extra_args);

    let cmd_str = if use_sudo {
        format!("sudo {} {}", command, args.join(" "))
    } else {
        format!("{} {}", command, args.join(" "))
    };

    if dry_run {
        println!("Dry run: {cmd_str}");
        return;
    }
    println!("\x1b[94mINFO: Executing command: {cmd_str}\x1b[0m");
    match gen_upd_cmd(command, &args, use_sudo).status() {
        Ok(es) => println!(
            "\x1b[94mINFO: Successfully updated with {command}, exited with status {es}\x1b[0m"
        ),
        Err(e) => eprintln!("\x1b[91mERR:\x1b[0m Failed to update with {command}, ERR: {e}"),
    }
}

/// Creates a Command configured with the given arguments and inherited I/O settings.
#[must_use]
pub fn gen_upd_cmd(command: &str, args: &[String], use_sudo: bool) -> Command {
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
fn reorder_candidates(candidates: Vec<PathBuf>, ord_mode: &OrdMode, verbose: bool) -> Vec<PathBuf> {
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
