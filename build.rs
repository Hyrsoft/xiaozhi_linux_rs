use std::env;
use std::fs;
use std::path::Path;
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    application: Application,
    board: Board,
}

#[derive(Deserialize)]
struct Application {
    name: String,
    version: String,
}

#[derive(Deserialize)]
struct Board {
    #[serde(rename = "type")]
    type_: String,
    name: String,
}

fn main() {
    println!("cargo:rerun-if-changed=config.toml");

    let config_path = Path::new("config.toml");
    if !config_path.exists() {
        // Fallback or error? Let's error to ensure user provides it as requested.
        panic!("config.toml not found!");
    }

    let config_str = fs::read_to_string(config_path).expect("Failed to read config.toml");
    let config: Config = toml::from_str(&config_str).expect("Failed to parse config.toml");

    println!("cargo:rustc-env=APP_NAME={}", config.application.name);
    println!("cargo:rustc-env=APP_VERSION={}", config.application.version);
    println!("cargo:rustc-env=BOARD_TYPE={}", config.board.type_);
    println!("cargo:rustc-env=BOARD_NAME={}", config.board.name);
}
