use std::env;

const PATH_TO_LINKER_SCRIPT: &str = "src/bin/kernel/linker.ld";

fn main() -> Result<(), String> {
    // "The build script’s current directory is the source directory of the build script’s
    // package."
    let path = env::current_dir()
        .map_err(|err| format!("Unable to access project directory: {err}"))?
        .into_os_string()
        .into_string()
        .map_err(|err| {
            format!(
                "Project directory path is not valid unicode (approximately {})",
                err.to_string_lossy()
            )
        })?;

    // Link with the custom linker script for only the kernel
    println!(
        "cargo:rustc-link-arg-bin=kernel=--script={}/{}",
        path, PATH_TO_LINKER_SCRIPT
    );
    // Disable section alignment
    println!("cargo:rustc-link-arg-bin=kernel=-n");
    // Produce a raw, stripped binary instead of an ELF, only for non-debugmode
    // In debug mode, we need the ELF to contain symbols
    // Later steps will then produce the appropriate binary while still having debug info
    // available for GDB
    // Unfortunately producing an output binary conflicts with producing debug symbols in the same
    // step
    match env::var("DEBUG")
        .expect("Cargo should specify the `DEBUG` environment variable")
        .as_str()
    {
        "false" => {
            println!("cargo:rustc-link-arg-bin=kernel=--oformat=binary");
            println!("cargo:rustc-link-arg=--strip-all");
        }
        "true" => {}
        _ => unreachable!(),
    }

    Ok(())
}
