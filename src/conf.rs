use crate::helpers::format_list;
use crate::self_up;
use colored::Colorize;
use pico_args::Arguments;
use std::collections::HashMap;
use std::path::PathBuf;

/// Determines how to order package manager updates.
#[derive(Debug)]
pub enum OrdMode {
    /// The user did not provide an explicit order – ask interactively.
    Interactive,
    /// The user provided a comma-separated list of package manager names.
    Specified(Vec<String>),
}

/// Holds runtime configuration derived from command-line arguments.
#[allow(clippy::struct_excessive_bools)]
pub struct Config {
    pub(crate) exclusions: HashMap<String, Vec<String>>,
    /// If provided, update only these package managers (by name).
    pub(crate) only: Option<Vec<String>>,
    /// Specifications to override the detected executable for a package manager.
    /// Format: "pm::/path/to/executable"
    pub(crate) specs: HashMap<String, PathBuf>,
    /// Auto mode uses non-interactive flags (if available) for each package manager.
    pub(crate) auto: bool,
    pub(crate) noconfirm: bool,
    /// Verbose mode prints extra logging information.
    pub(crate) verbose: bool,
    /// List mode prints found package managers without performing any updates.
    pub(crate) list: bool,
    pub(crate) dry_run: bool,
    /// Extra flags to pass to package managers. Format: "pm::<flags>"
    pub(crate) exts: HashMap<String, Vec<String>>,
    /// Optional ordering of updates.
    pub(crate) ord: Option<OrdMode>,
    //install_mode: bool,
}

impl Config {
    #[allow(clippy::too_many_lines)]
    pub fn parse_args() -> Config {
        let mut pargs = Arguments::from_env();

        // Help, version, and self updating.
        if pargs.contains(["-h", "--help"]) {
            Self::print_help();
            std::process::exit(0);
        }
        if pargs.contains(["-V", "--version"]) {
            println!("qud v1.5.1");
            std::process::exit(0);
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
        if pargs.contains(["-S", "--self-update"]) {
            if !self_up::perm::is_elevated() {
                eprintln!("Program must be run as root to update.");
                std::process::exit(2);
            }
            println!("Updating qud...");
            match self_up::self_update(noconfirm) {
                Ok(()) => println!("qud updated successfully, exiting."),
                Err(e) => eprintln!("qud failed to update: {e}, exiting."),
            }
            std::process::exit(-1);
        }
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
                    eprintln!("{} Invalid spec format: {spec}", "ERR:".red());
                }
            } else {
                eprintln!("{} Invalid spec format: {spec}", "ERR:".red());
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
                eprintln!("{} Invalid ext format: {ext}", "ERR:".red());
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

        // Print error and exit for unrecognized arguments.
        let remaining = pargs.finish();
        if !remaining.is_empty() {
            eprintln!("{} Unrecognized arguments: {:?}", "ERR:".red(), remaining);
            std::process::exit(1);
        }

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
            //install_mode: false,
        }
    }

    fn print_help() {
        println!(
            r#"qud v1.5.1

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
                eprintln!("{} Invalid exclusion format: {excl}", "ERR:".red());
            }
        } else {
            // Full exclusion: mark the package manager as entirely excluded by storing an empty Vec.
            map.insert(excl.to_string(), Vec::new());
        }
    }

    /// Returns extra arguments for the given package manager based on the exclusions map.
    pub fn get_exclusion_args(&self, pm: &str) -> Vec<String> {
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
                        "{} {} does not support exclusions (or not yet implemented). The following packages ({}) will still be updated.",
                        "WARN:".yellow(),
                        p,
                        format_list(pkgs)
                    );
                }
            }
        }
        args
    }

    /// Returns extra flags for the given package manager passed via --ext.
    pub fn get_ext_args(&self, pm: &str) -> Vec<String> {
        self.exts.get(pm).cloned().unwrap_or_default()
    }
}
