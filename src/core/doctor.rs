use std::path::Path;
use std::process::{Command, Stdio};

use crate::core::config;
use crate::error::{Result, VcdError};

const ORBSTACK_APP: &str = "/Applications/OrbStack.app";

pub fn run() -> Result<()> {
    let mut failures = 0;

    if !check_orbstack() {
        failures += 1;
    }
    if !check_docker_cli() {
        failures += 1;
    }

    if !check_config() {
        failures += 1;
    }

    if failures == 0 {
        Ok(())
    } else {
        Err(
            VcdError::new("环境检查失败", format!("{failures} check(s) failed"))
                .with_hint("请按上方 fail 项提示修复后重新运行 vcd doctor"),
        )
    }
}

fn check_orbstack() -> bool {
    if Path::new(ORBSTACK_APP).is_dir() {
        ok("OrbStack installed");
        true
    } else {
        fail(
            "OrbStack not installed",
            "install OrbStack with: brew install --cask orbstack",
        );
        false
    }
}

fn check_docker_cli() -> bool {
    match command_ok("docker", &["--version"]) {
        Ok(true) => {
            ok("Docker CLI available");
            true
        }
        Ok(false) => {
            fail(
                "Docker CLI not available",
                "install and start OrbStack, then reopen your terminal",
            );
            false
        }
        Err(err) => {
            fail(
                "Docker CLI not found",
                format!("install and start OrbStack, then reopen your terminal ({err})"),
            );
            false
        }
    }
}

fn check_config() -> bool {
    let config_path = match config::default_config_path() {
        Ok(path) => path,
        Err(err) => {
            fail(
                "vcd config unreadable",
                format!("run vcd init <user> ({err})"),
            );
            return false;
        }
    };

    match config::read_config(&config_path) {
        Ok(_) => {
            ok("vcd config readable");
            true
        }
        Err(err) => {
            fail(
                "vcd config unreadable",
                format!("run vcd init <user> ({err})"),
            );
            false
        }
    }
}

fn command_ok(program: &str, args: &[&str]) -> std::io::Result<bool> {
    Ok(Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success())
}

fn ok(message: &str) {
    println!("[ok] {message}");
}

fn fail(message: &str, hint: impl AsRef<str>) {
    println!("[fail] {message}");
    println!("Hint: {}", hint.as_ref());
}
