use std::fs::File;
use std::io;
use std::io::Write;
use std::{env, path::Path, process::Command};

type DynError = Box<dyn std::error::Error>;

fn main() -> Result<(), DynError> {
    let mut args = env::args();
    match args.nth(1).as_deref() {
        Some("qemu") => {
            let is_debug = matches!(args.next().as_deref(), Some("debug"));
            build(is_debug, project_root().join("target/dist"))?;
            let mut qemu = Command::new("qemu-system-aarch64");
            qemu.args([
                "-serial",
                "stdio",
                "-M",
                "raspi3b",
                "-display",
                "none",
                "-semihosting",
                "-kernel",
                "target/dist/kernel",
            ]);
            if is_debug {
                qemu.args(["-s", "-S"]);
            }
            if !qemu.status()?.success() {
                Err("qemu failed")?;
            }
            Ok(())
        }
        Some("build") => {
            let is_debug = matches!(args.next().as_deref(), Some("debug"));
            build(is_debug, project_root().join("target/dist"))?;
            Ok(())
        }
        Some(unknown) => Err(format!("Unknown command: {}", unknown))?,
        None => {
            println!(
                "Available commands
qemu             compiles kernel and runs in QEMU"
            );
            Ok(())
        }
    }
}

fn project_root<'a>() -> &'a Path {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Project root should be an existing directory")
}

fn build(is_debug: bool, output_dir: impl AsRef<Path>) -> Result<(), DynError> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(cargo);
    command
        .current_dir(project_root().join("os"))
        .args(["build", "--bins", "-Z=unstable-options"])
        .arg(format!("--out-dir={}", output_dir.as_ref().display()));

    if !is_debug {
        command.arg("--release");
    }

    if !command.status()?.success() {
        Err("cargo build failed")?;
    }

    if is_debug {
        // We need to manually objcopy into a binary for debug mode, as well as get symbols
        if !Command::new("rust-objcopy")
            .args(["-Obinary"])
            .arg(output_dir.as_ref().join("kernel"))
            .status()?
            .success()
        {
            Err("objcopy failed")?
        }
    }

    println!("Built object");

    let mut kernel = File::options()
        .append(true)
        .open(output_dir.as_ref().join("kernel"))?;
    let mut init = File::open(output_dir.as_ref().join("init"))?;
    let init_length = u16::try_from(init.metadata()?.len())?;
    assert!(init_length <= u16::MAX - 0x4000);
    kernel.write(&(init_length.to_le_bytes()))?;
    io::copy(&mut init, &mut kernel)?;

    println!("Concatenated objects");

    Ok(())
}
