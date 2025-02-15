use std::{
    error::Error,
    fs,
    path::PathBuf,
    process::Command,
};

pub fn self_update() -> Result<(), Box<dyn Error>> {
    let repo_url = "https://github.com/barely-a-dev/qud.git";

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
        return Err("Cargo build failed".into());
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

    #[cfg(not(target_family = "windows"))]
    let install_path = PathBuf::from("/usr/bin/qud");

    #[cfg(target_family = "windows")]
    let install_path = {
        let target_dir = PathBuf::from("C:\\Program Files\\qud");
        fs::create_dir_all(&target_dir)?;
        target_dir.join("qud.exe")
    };

    println!(
        "Copying built binary from {binary_path:?} to {install_path:?}",
    );
    fs::copy(&binary_path, &install_path)?;

    println!("Self-update successful!");

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
