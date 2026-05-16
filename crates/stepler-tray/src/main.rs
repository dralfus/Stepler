#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn main() {
    #[cfg(windows)]
    {
        if std::env::args().any(|arg| arg == "--stop") {
            run_tray_host("--stop");
            return;
        }

        run_tray_host("");
    }

    #[cfg(not(windows))]
    {
        eprintln!("stepler-tray is Windows-only");
    }
}

#[cfg(windows)]
fn find_tray_host() -> Option<PathBuf> {
    let repo_root = find_repo_root()?;
    let host_path = repo_root
        .join("apps")
        .join("Stepler.Tray")
        .join("bin")
        .join("Debug")
        .join("net9.0-windows")
        .join("Stepler.Tray.exe");
    host_path.exists().then_some(host_path)
}

#[cfg(windows)]
fn run_tray_host(arg: &str) {
    if let Some(host_path) = find_tray_host() {
        let mut command = std::process::Command::new(host_path);
        if !arg.is_empty() {
            command.arg(arg);
        }
        let _ = command.creation_flags(CREATE_NO_WINDOW).spawn();
    }
}

#[cfg(windows)]
fn find_repo_root() -> Option<PathBuf> {
    let mut current = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));

    while let Some(path) = current {
        if path.join("Cargo.toml").exists() && path.join("crates").is_dir() {
            return Some(path);
        }
        current = path.parent().map(Path::to_path_buf);
    }

    let fallback = PathBuf::from(r"F:\distr\system\Stepler");
    (fallback.join("Cargo.toml").exists()).then_some(fallback)
}
