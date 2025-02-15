# qud: Universal Package Manager Updater

`qud` is a command-line tool designed to **automatically detect and update** various package managers on your system. It
supports both system-level and language-specific package managers on Linux, Windows, macOS, and more.

> **Note:** Current version: **v1.0.7**

---

## Table of Contents

- [Features](#features)
- [Supported Package Managers](#supported-package-managers)
- [Installation](#installation)
- [Usage](#usage)
- [Command-Line Options](#command-line-options)
- [Examples](#examples)
- [How It Works](#how-it-works)
- [Customization & Internals](#customization--internals)
- [Contributing](#contributing)
- [License](#license)

---

## Features

- **Multi-Package Manager Support:** Automatically detects and updates a variety of package managers.
- **Automated Updates:** Supports non-interactive, auto mode for unattended updates.
- **Interactive Ordering:** Optionally reorder update operations interactively.
- **Exclusions:** Exclude specific packages or entire package managers from updates.
- **Custom Executable Overrides:** Override the detected executable path for any package manager.
- **Extra Flags:** Pass custom flags to package manager commands.
- **Dry Run Mode:** Preview the update commands without executing them.
- **Verbose Logging:** Detailed output for debugging or monitoring operations.

---

## Supported Package Managers

`qud` supports a broad range of package managers, including but not limited to:

- **Linux:**  
  `pacman`, `yay`, `apt`, `apt-get`, `dnf`, `zypper`, `snap`, `flatpak`, `xbps-install`, `apk`, `emerge`, `guix`, `nix`,
  `yum`, `eopkg`
- **Windows:**  
  `choco`, `scoop`, `winget`
- **General/Other:**  
  `rustup`, `brew`, `port` (MacPorts), `pkg` (FreeBSD), `cargo`, `npm`, `pip`, `composer`, `gem`, `conda`, `poetry`,
  `nuget`, `asdf`, `vcpkg`, `conan`, `stack`, `opam`, `mix`, `sdkman`, `gvm`, `pnpm`, `yarn`, `maven`, `go`

---

## Installation

Build from source using [Rust](https://www.rust-lang.org/):

```bash
# Clone the repository
git clone https://github.com/barely-a-dev/qud.git
cd qud

# Build in release mode
cargo build --release
```

After building, add the resulting binary (found in `target/release/qud`) to your system `PATH` for easy access.

---

## Usage

Run `qud` from your terminal:

```bash
qud [options]
```

When executed, `qud` will:

1. Search your `PATH` for known package manager executables.
2. Apply configuration based on your provided command-line options.
3. Build and execute (or print, in dry-run mode) update commands for each detected package manager.

---

## Command-Line Options

| Option         | Alias     | Description                                                                                                                                                                                                        |
|----------------|-----------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `--dry <flag>` | `-d`      | **Dry Run:** Print update commands without executing them.                                                                                                                                                         |
| `--excl <s>`   | `-e <s>`  | **Exclusions:** Exclude a specific package (format: `pm::pkg`) or an entire package manager (format: `pm`). Can be used multiple times.                                                                            |
| `--auto`       | `-a`      | **Auto Mode:** Use non-interactive flags (where available) for automated updates.                                                                                                                                  |
| `--verbose`    | `-v`      | **Verbose Logging:** Enable detailed logging output.                                                                                                                                                               |
| `--list`       | `-l`      | **List Mode:** List all detected package managers and exit without updating.                                                                                                                                       |
| `--only <pm>`  | `-o <pm>` | **Selective Update:** Update only the specified package manager(s). Can be repeated.                                                                                                                               |
| `--spec <s>`   | `-s <s>`  | **Executable Override:** Specify an alternative executable for a package manager. Format: `pm::/path/to/executable`.                                                                                               |
| `--ext <s>`    | `-E <s>`  | **Extra Flags:** Pass additional flags to a package manager's command. Format: `pm::"<flags>"`.                                                                                                                    |
| `--ord [s]`    | `-O [s]`  | **Custom Order:** Specify the update order. If a value is provided (e.g. `pm1,pm2,pm3`), those package managers are updated in that order. Without a value, interactive mode prompts you to sort the update order. |
| `--help`       | `-h`      | **Help:** Display the help message.                                                                                                                                                                                |
| `--version`    | `-V`      | **Version:** Show version information (e.g., `v1.0.7`).                                                                                                                                                            |

---

## Examples

### Dry Run

Preview update commands without making any changes:

```bash
qud --dry
```

### Excluding a Package or Manager

Exclude a specific package (e.g., `vim` from `apt`):

```bash
qud --excl apt::vim
```

Exclude an entire package manager (e.g., `pacman`):

```bash
qud --excl pacman
```

### Automatic Non-Interactive Updates

Enable auto mode to use non-interactive flags:

```bash
qud --auto
```

### Updating Only Specific Package Managers

Update only `apt` and `yum`:

```bash
qud --only apt --only yum
```

### Overriding the Executable Path

Override the detected path for `pacman`:

```bash
qud --spec pacman::/custom/path/to/pacman
```

### Passing Extra Flags

Pass additional flags to `apt` (for example, to fix missing dependencies):

```bash
qud --ext 'apt::"--fix-missing"'
```

### Custom Update Order

Specify the update order for selected package managers:

```bash
qud --ord apt,yum,brew
```

Or, use interactive mode to reorder them manually:

```bash
qud --ord
```

After running with `--ord` and no value, you'll see a prompt like:

```
Detected package managers:
  0: apt
  1: yum
  2: brew
Enter the desired update order as comma-separated indices (e.g. 2,0,1) or press Enter to keep the current order:
```

Type your desired order (e.g., `2,0,1`) and press Enter.

---

## How It Works

1. **Detection:**  
   `qud` scans the directories in your `PATH` for executables that match a pre-defined list of package manager names
   using a recursive search (powered by the [`walkdir`](https://crates.io/crates/walkdir) crate). If multiple instances
   are found, the first one is chosen while issuing a warning if verbose mode is active.

2. **Configuration:**  
   The tool processes command-line arguments to determine which package managers to update, any custom flags or
   executable overrides, and the ordering of updates. Options like `--excl`, `--spec`, and `--ext` allow granular
   control.

3. **Processing & Execution:**  
   For each package manager:
    - **Exclusions & Overrides:** Checks if the manager or specific packages should be skipped.
    - **Command Assembly:** Combines base update commands with extra exclusion or extension arguments.
    - **Sudo Usage:** Some updates require administrative privileges and are prefixed with `sudo` when needed.
    - **Dry Run vs. Execution:** In dry run mode, the command is printed; otherwise, it is executed with real-time I/O
      streams.

4. **Reordering:**  
   If `--ord` is used, `qud` reorders the detected package managers:
    - **Specified Order:** If you provide a comma-separated list (e.g., `apt,yum,brew`), those managers are updated in
      that order, with any remaining ones appended in their original sequence.
    - **Interactive Mode:** Without a value, you’re prompted to input your desired order interactively.

---

## Customization & Internals

- **Executable Detection:**  
  The tool leverages environment variables and file system traversal to locate package manager executables. It even
  checks for duplicate installations and warns when multiple instances are detected.

- **Per-Package Manager Logic:**  
  Each package manager has its own update routine defined in `process_pm`. This includes both the base command and any
  extra flags needed (which vary based on whether auto mode is enabled).

- **Exclusion & Extra Arguments:**  
  Custom handling is provided for exclusions. For example:
    - `pacman` and `xbps-install` use the `--ignore` flag.
    - `yay` uses multiple `--excludepkg` flags.
    - Other package managers may warn if exclusions are not supported.

- **Command Generation:**  
  The update commands are constructed using Rust’s `Command` API, allowing you to see the full command line that would
  be executed (especially useful in dry run mode).

For more details or to contribute enhancements, please review the source code and join the discussion on our GitHub
repository.

---

## Contributing

Contributions, bug reports, and feature requests are welcome!  
Please open an issue or submit a pull request on the [GitHub repository](https://github.com/barely-a-dev/qud).

---

## License

`qud` is licensed under the [GPL 3.0](LICENSE).

---

## Version

**v1.0.7**

For further questions or assistance, please refer to the [GitHub repository](https://github.com/barely-a-dev/qud) or
contact the maintainer. Happy updating!