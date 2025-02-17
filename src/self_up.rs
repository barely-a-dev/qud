use std::io::Write;
use std::{error::Error, fs, path::PathBuf, process::Command};

pub fn self_update(noconfirm: bool) -> Result<(), Box<dyn Error>> {
    let repo_url = "https://github.com/barely-a-dev/qud.git";

    if !noconfirm {
        println!("Are you sure you want to update qud v1.5.1? (Y/n)");
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

    let temp_dir = std::env::temp_dir().join("qud_temp");
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)?;
    }
    fs::create_dir(&temp_dir)?;
    let clone_path = temp_dir.join("qud");
    println!("Cloning repository into {clone_path:?}");

    let status = Command::new("git")
        .args(["clone", repo_url, clone_path.to_str().unwrap()])
        .status()?;
    if !status.success() {
        return Err("Git clone failed".into());
    }

    println!("Building project (release mode)...");
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&clone_path)
        .status()?;
    if !status.success() {
        return Err("Cargo build failed. Please report this error.".into());
    }

    // Determine the binary name based on the OS.
    #[cfg(target_family = "windows")]
    let binary_name = "qud.exe";
    #[cfg(not(target_family = "windows"))]
    let binary_name = "qud";

    let binary_path = clone_path.join("target").join("release").join(binary_name);
    if !binary_path.exists() {
        return Err(format!("Built binary not found at {binary_path:?}").into());
    }

    #[cfg(not(target_family = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&binary_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary_path, perms)?;
    }

    // Define the installation path.
    #[cfg(not(target_family = "windows"))]
    let install_path = PathBuf::from("/usr/bin/qud");

    #[cfg(target_family = "windows")]
    let install_path = {
        let target_dir = PathBuf::from("C:\\Program Files\\qud");
        fs::create_dir_all(&target_dir)?;
        target_dir.join("qud.exe")
    };

    #[cfg(not(target_family = "windows"))]
    {
        let temp_install_path = install_path.with_extension("new");
        println!(
            "Copying built binary from {binary_path:?} to temporary location {temp_install_path:?}"
        );
        fs::copy(&binary_path, &temp_install_path)?;
        
        println!("Spawning updater process to replace the binary after exit.");
        Command::new("sh")
            .arg("-c")
            .arg(format!(
                "sleep 10 && mv {} {}",
                temp_install_path.display(),
                install_path.display()
            ))
            .spawn()?;

        println!("Self-update scheduled.");
    }

    #[cfg(target_family = "windows")]
    {
        println!("Copying built binary from {binary_path:?} to {install_path:?}");
        fs::copy(&binary_path, &install_path)?;
        println!("Self-update successful!");
    }

    // Clean up temporary directory.
    fs::remove_dir_all(&temp_dir)?;

    Ok(())
}

pub mod perm {
    #[cfg(not(target_family = "windows"))]
    mod platform {
        use std::os::raw::c_uint;

        extern "C" {
            fn geteuid() -> c_uint;
        }

        pub fn is_elevated() -> bool {
            unsafe { geteuid() == 0 }
        }
    }

    #[cfg(target_family = "windows")]
    mod platform {
        // Link to shell32.dll where IsUserAnAdmin is defined.
        #[link(name = "shell32")]
        extern "system" {
            fn IsUserAnAdmin() -> i32;
        }

        pub fn is_elevated() -> bool {
            // IsUserAnAdmin returns a nonzero value if the user is an administrator.
            unsafe { IsUserAnAdmin() != 0 }
        }
    }

    pub fn is_elevated() -> bool {
        platform::is_elevated()
    }
}
