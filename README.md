# qud: Universal Package Manager Updater

`qud` is a command-line tool that **automatically detects and updates** various package managers across Linux, Windows,
macOS, and more.

> **Latest Version:** **v1.2.8**

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

## Features

- **Multi-Package Manager Support** – Detects and updates various package managers.
- **Automated Updates** – Runs in non-interactive mode for unattended updates.
- **Interactive Ordering** – Reorder updates interactively.
- **Exclusions** – Exclude specific packages or entire package managers.
- **Custom Executable Overrides** – Specify alternative package manager paths.
- **Extra Flags** – Pass custom flags to update commands.
- **Dry Run Mode** – Preview update commands before execution.
- **Verbose Logging** – Detailed output for debugging.

## Supported Package Managers

**Linux:** `pacman`, `yay`, `apt`, `apt-get`, `dnf`, `zypper`, `snap`, `flatpak`, `xbps-install`, `apk`, `emerge`,
`guix`, `nix`, `yum`, `eopkg`, `cave`, `sbopkg`, `scratch`
**Windows:** `choco`, `scoop`, `winget`  
**General/Other:** `rustup`, `brew`, `port`, `pkg`, `cargo`, `npm`, `pip`, `composer`, `gem`, `conda`, `poetry`,
`nuget`, `asdf`, `vcpkg`, `conan`, `stack`, `opam`, `mix`, `sdkman`, `gvm`, `pnpm`, `yarn`, `maven`, `go`

## Installation

### From Source (Rust Required)

```bash
git clone https://github.com/barely-a-dev/qud.git
cd qud
cargo build --release
cp ./target/release/qud /usr/bin/qud
```

### Prebuilt Packages

#### Arch Linux:

```bash
sudo pacman -U qud-<version>-x86_64.pkg.tar.zst
```

#### Debian-based:

```bash
sudo apt install ./qud_v1.2.8_amd64.deb
```

## Usage

Run `qud`:

```bash
qud [options]
```

It will:

1. Detect available package managers.
2. Apply configuration based on options.
3. Execute or print (in dry-run mode) update commands.

## Command-Line Options

| Option        | Alias | Description                                                                    |
|---------------|-------|--------------------------------------------------------------------------------|
| `--dry`       | `-d`  | Print update commands without executing.                                       |
| `--excl <s>`  | `-e`  | Exclude a package (`pm::pkg`) or a package manager (`pm`). Repeatable.         |
| `--auto`      | `-a`  | Run updates in non-interactive mode.                                           |
| `--verbose`   | `-v`  | Enable detailed logging.                                                       |
| `--list`      | `-l`  | List detected package managers without updating.                               |
| `--only <pm>` | `-o`  | Update only the specified package manager(s). Repeatable.                      |
| `--spec <s>`  | `-s`  | Override package manager executable (`pm::/path/to/executable`).               |
| `--ext <s>`   | `-E`  | Add extra flags (`pm::"<flags>"`).                                             |
| `--ord [s]`   | `-O`  | Set update order (e.g., `pm1,pm2,pm3`). Interactive mode if no value provided. |
| `--help`      | `-h`  | Display help.                                                                  |
| `--version`   | `-V`  | Show version.                                                                  |

## Examples

### Dry Run

```bash
qud --dry
```

### Exclude a Package or Manager

```bash
qud --excl apt::vim  # Exclude vim from apt
qud --excl pacman    # Exclude pacman entirely
```

### Auto Mode (Non-Interactive)

```bash
qud --auto
```

### Update Only Specific Package Managers

```bash
qud --only apt --only yum
```

### Override Executable Path

```bash
qud --spec pacman::/custom/path/to/pacman
```

### Add Extra Flags

```bash
qud --ext 'apt::"--fix-missing"'
```

### Set Custom Update Order

```bash
qud --ord apt,yum,brew
```

For interactive ordering:

```bash
qud --ord
```

## How It Works

1. **Detection:** Scans `PATH` for package manager executables using [`walkdir`](https://crates.io/crates/walkdir).
2. **Configuration:** Processes command-line arguments for exclusions, overrides, and order.
3. **Execution:**
    - Skips excluded package managers or packages.
    - Constructs appropriate update commands.
    - Uses `sudo` if required.
    - Prints commands in dry-run mode or executes them.
4. **Reordering:**
    - Uses provided order if specified (`--ord pm1,pm2`).
    - Defaults to interactive sorting if no order is specified.

## Customization & Internals

- **Executable Detection:** Uses `PATH` and file traversal.
- **Per-Package Manager Logic:** Defines update routines per package manager.
- **Exclusions & Overrides:** Supports package-specific and manager-wide exclusions.
- **Command Generation:** Uses Rust’s `Command` API for structured execution.

## Contributing

Contributions, bug reports, and feature requests are welcome!  
Submit issues or pull requests on [GitHub](https://github.com/barely-a-dev/qud).

## License

`qud` is licensed under the [GPL 3.0](LICENSE).

## Version

**v1.2.8**

For assistance, visit [GitHub](https://github.com/barely-a-dev/qud). Happy updating!

