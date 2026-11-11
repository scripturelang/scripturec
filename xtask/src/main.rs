use std::{
    env, fs,
    path::Path,
    process::{Command, ExitCode},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cargo web: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args == ["web", "--help"] || args == ["web", "-h"] {
        println!(
            "Usage: cargo web [--offline]\nBuild the WebAssembly library and browser bindings in pkg/."
        );
        return Ok(());
    }
    let offline = args == ["web", "--offline"];
    if !offline && args != ["web"] {
        return Err("usage: cargo web [--offline]".into());
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    env::set_current_dir(root)?;
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let target = root.join("target");
    let package = root.join("pkg");

    // Read the exact binding version from Cargo rather than maintaining a second pin.
    let metadata = Command::new(&cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--locked",
            "--offline",
        ])
        .output()?;
    if !metadata.status.success() {
        return Err(String::from_utf8_lossy(&metadata.stderr)
            .into_owned()
            .into());
    }
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout)?;
    let version = metadata["packages"]
        .as_array()
        .and_then(|packages| packages.iter().find(|p| p["name"] == "scripturec"))
        .and_then(|p| p["dependencies"].as_array())
        .and_then(|deps| deps.iter().find(|d| d["name"] == "wasm-bindgen"))
        .and_then(|d| d["req"].as_str())
        .and_then(|req| req.strip_prefix('='))
        .ok_or("wasm-bindgen must have an exact version in Cargo.toml")?;

    let mut build = Command::new(&cargo);
    build
        .args([
            "build",
            "--package",
            "scripturec",
            "--lib",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--locked",
        ])
        .arg("--target-dir")
        .arg(&target);
    if offline {
        build.arg("--offline");
    }
    execute(&mut build)?;

    let local_tool = target
        .join("tools/bin")
        .join(format!("wasm-bindgen{}", env::consts::EXE_SUFFIX));
    let tool = if matches_version(Path::new("wasm-bindgen"), version) {
        "wasm-bindgen".into()
    } else {
        if !matches_version(&local_tool, version) {
            let mut install = Command::new(&cargo);
            install
                .args([
                    "install",
                    "wasm-bindgen-cli",
                    "--version",
                    version,
                    "--locked",
                    "--force",
                ])
                .arg("--root")
                .arg(target.join("tools"))
                .arg("--target-dir")
                .arg(target.join("wasm-bindgen-build"));
            if offline {
                install.arg("--offline");
            }
            execute(&mut install)?;
        }
        local_tool
    };
    fs::create_dir_all(&package)?;
    execute(
        Command::new(tool)
            .arg(target.join("wasm32-unknown-unknown/release/scripturec.wasm"))
            .args(["--target", "web", "--out-name", "scripturec", "--out-dir"])
            .arg(&package),
    )?;
    fs::write(package.join("package.json"), "{\"type\":\"module\"}\n")?;
    println!("Built pkg/. Copy this directory into the website repository when ready.");
    Ok(())
}

fn matches_version(tool: &Path, version: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).trim()
                    == format!("wasm-bindgen {version}")
        })
}

fn execute(command: &mut Command) -> Result<()> {
    let status = command.status()?;
    if !status.success() {
        return Err(format!("{command:?} failed ({status})").into());
    }
    Ok(())
}
