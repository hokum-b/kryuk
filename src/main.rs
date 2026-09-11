use std::env;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("");

    match command {
        "run" => {
            let extra_args = &args[2..];
            if let Err(err) = run_sober(extra_args) {
                eprintln!("error: {err}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        "edit" => {
            if let Err(err) = edit_fflags() {
                eprintln!("error: {err}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        "status" | "list" => {
            if let Err(err) = show_status() {
                eprintln!("error: {err}");
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        "help" | "--help" | "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        "version" | "--version" | "-V" => {
            println!("kryuk {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "" => {
            print_help();
            ExitCode::SUCCESS
        }
        unknown => {
            eprintln!("error: unknown command '{unknown}'\n");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "usage: kryuk <command> [args...]\n\n\
         commands:\n  \
           run [args...]   apply fastflags and run sober.\n  \
           edit            edit fastflags.\n  \
           status          show fastflags.\n  \
           help            show help."
    );
}

fn kryuk_config_dir() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        if !xdg.trim().is_empty() {
            return PathBuf::from(xdg).join("kryuk");
        }
    }
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("kryuk")
}

fn kryuk_fflags_path() -> PathBuf {
    kryuk_config_dir().join("fflags.json")
}

fn sober_base_dir() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".var")
        .join("app")
        .join("org.vinegarhq.Sober")
}

fn sober_config_path() -> PathBuf {
    sober_base_dir().join("config").join("sober").join("config.json")
}

fn sober_client_app_settings_path() -> PathBuf {
    sober_base_dir()
        .join("data")
        .join("sober")
        .join("appData")
        .join("ClientSettings")
        .join("ClientAppSettings.json")
}

fn default_fflags_template() -> &'static str {
    "{\n  \"FFlagDebugGraphicsPreferOpenGL\": true\n}\n"
}

fn initial_fflags_content() -> String {
    if let Ok(existing_flags) = read_sober_existing_fflags() {
        let is_only_placeholder = existing_flags
            .as_object()
            .map(|obj| obj.len() == 1 && obj.contains_key("FFlagExample"))
            .unwrap_or(false);

        if !is_only_placeholder {
            if let Ok(pretty) = serde_json::to_string_pretty(&existing_flags) {
                return pretty + "\n";
            }
        }
    }
    default_fflags_template().to_string()
}

fn edit_fflags() -> Result<(), Box<dyn std::error::Error>> {
    let fflags_file = kryuk_fflags_path();
    let config_dir = kryuk_config_dir();

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
    }

    if !fflags_file.exists() {
        let initial_content = initial_fflags_content();
        fs::write(&fflags_file, initial_content)?;
    }

    open_in_editor(&fflags_file)
}

fn open_in_editor(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path_str = path.to_string_lossy();

    if let Ok(mut child) = Command::new("xdg-open").arg(path).spawn() {
        match child.wait() {
            Ok(status) if status.success() => {
                return Ok(());
            }
            _ => {}
        }
    }

    if let Ok(editor) = env::var("VISUAL").or_else(|_| env::var("EDITOR")) {
        let trimmed = editor.trim();
        if !trimmed.is_empty() {
            let mut parts = trimmed.split_whitespace();
            if let Some(cmd) = parts.next() {
                let mut command = Command::new(cmd);
                for arg in parts {
                    command.arg(arg);
                }
                command.arg(path);
                let status = command.status()?;
                if status.success() {
                    return Ok(());
                }
            }
        }
    }

    for fallback in &["gnome-text-editor", "gedit", "kate", "nano", "vim", "vi"] {
        if let Ok(status) = Command::new(fallback).arg(path).status() {
            if status.success() {
                return Ok(());
            }
        }
    }

    Err(format!("failed to open editor for {path_str}").into())
}

fn read_sober_existing_fflags() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let sober_cfg = sober_config_path();
    if !sober_cfg.exists() {
        return Err("sober config does not exist".into());
    }

    let raw = fs::read_to_string(&sober_cfg)?;
    let clean = strip_json_comments(&raw);
    let parsed: serde_json::Value = serde_json::from_str(&clean)?;

    if let Some(fflags) = parsed.get("fflags") {
        if fflags.is_object() && !fflags.as_object().unwrap().is_empty() {
            return Ok(fflags.clone());
        }
    }

    Err("no existing fastflags found".into())
}

fn load_kryuk_fflags() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let fflags_file = kryuk_fflags_path();

    if !fflags_file.exists() {
        let config_dir = kryuk_config_dir();
        fs::create_dir_all(&config_dir)?;
        fs::write(&fflags_file, initial_fflags_content())?;
    }

    let raw = fs::read_to_string(&fflags_file)
        .map_err(|e| format!("failed to read {}: {e}", fflags_file.display()))?;

    let clean = strip_json_comments(&raw);
    let parsed: serde_json::Value = serde_json::from_str(&clean)
        .map_err(|e| format!("invalid json in {}: {e}", fflags_file.display()))?;

    if !parsed.is_object() {
        return Err(format!("{} must contain a json object", fflags_file.display()).into());
    }

    Ok(parsed)
}

fn run_sober(extra_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if Command::new("flatpak").arg("--version").output().is_err() {
        return Err("flatpak not found".into());
    }

    let fflags = load_kryuk_fflags()?;
    bootstrap_sober_config(&fflags)?;
    bootstrap_client_app_settings(&fflags)?;

    let mut cmd = Command::new("flatpak");
    cmd.arg("run").arg("org.vinegarhq.Sober");
    for arg in extra_args {
        cmd.arg(arg);
    }

    let err = cmd.exec();
    Err(format!("failed to execute flatpak run: {err}").into())
}

fn bootstrap_sober_config(fflags: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    let cfg_path = sober_config_path();
    let parent = cfg_path.parent().unwrap();
    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }

    let mut config_obj = if cfg_path.exists() {
        let raw = fs::read_to_string(&cfg_path)?;
        let clean = strip_json_comments(&raw);
        match serde_json::from_str::<serde_json::Value>(&clean) {
            Ok(v) if v.is_object() => v,
            _ => serde_json::json!({}),
        }
    } else {
        serde_json::json!({})
    };

    if let Some(map) = config_obj.as_object_mut() {
        map.insert("fflags".to_string(), fflags.clone());
    }

    let formatted_json = serde_json::to_string_pretty(&config_obj)?;
    fs::write(&cfg_path, formatted_json + "\n")?;
    Ok(())
}

fn bootstrap_client_app_settings(
    fflags: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings_path = sober_client_app_settings_path();
    let parent = settings_path.parent().unwrap();
    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }

    let formatted = serde_json::to_string_pretty(fflags)?;
    fs::write(&settings_path, formatted + "\n")?;
    Ok(())
}

fn show_status() -> Result<(), Box<dyn std::error::Error>> {
    let fflags_file = kryuk_fflags_path();
    let sober_cfg = sober_config_path();

    println!("fastflags file: {}", fflags_file.display());
    println!("sober config:   {}", sober_cfg.display());

    if fflags_file.exists() {
        let fflags = load_kryuk_fflags()?;
        if let Some(obj) = fflags.as_object() {
            println!("\nfastflags ({}):", obj.len());
            for (key, val) in obj {
                println!("  {key}: {val}");
            }
        }
    } else {
        println!("\nno fastflags file found. run 'kryuk edit' to create one.");
    }

    Ok(())
}

fn strip_json_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;

    while i < len {
        let c = chars[i];

        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }

        if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                if chars[i] == '\n' {
                    out.push('\n');
                }
                i += 1;
            }
            if i + 1 < len {
                i += 2;
            } else {
                i = len;
            }
            continue;
        }

        out.push(c);
        i += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_comments() {
        let raw = "{\n// test line\n\"key\": \"value // not comment\",\n/* block */\n\"num\": 1\n}";
        let stripped = strip_json_comments(raw);
        let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(parsed["key"], "value // not comment");
        assert_eq!(parsed["num"], 1);
    }

    #[test]
    fn test_bootstrap_logic() {
        let dir = std::env::temp_dir().join("kryuk_test_bootstrap_clean");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let cfg_path = dir.join("config.json");
        let initial_cfg = "{\n  \"discord_rpc_enabled\": true,\n  \"fflags\": {\n    \"OldFlag\": false\n  }\n}\n";
        fs::write(&cfg_path, initial_cfg).unwrap();

        let new_flags = serde_json::json!({
            "FFlagDebugGraphicsPreferOpenGL": true,
            "DFIntTextureQualityOverride": 3
        });

        let raw = fs::read_to_string(&cfg_path).unwrap();
        let clean = strip_json_comments(&raw);
        let mut parsed: serde_json::Value = serde_json::from_str(&clean).unwrap();
        parsed.as_object_mut().unwrap().insert("fflags".to_string(), new_flags.clone());

        let formatted_json = serde_json::to_string_pretty(&parsed).unwrap();
        fs::write(&cfg_path, formatted_json + "\n").unwrap();

        let read_back = fs::read_to_string(&cfg_path).unwrap();
        let clean_back = strip_json_comments(&read_back);
        let parsed_back: serde_json::Value = serde_json::from_str(&clean_back).unwrap();
        assert_eq!(parsed_back["discord_rpc_enabled"], true);
        assert_eq!(parsed_back["fflags"]["FFlagDebugGraphicsPreferOpenGL"], true);
        assert_eq!(parsed_back["fflags"]["DFIntTextureQualityOverride"], 3);
        assert!(parsed_back["fflags"].get("OldFlag").is_none());

        let _ = fs::remove_dir_all(&dir);
    }
}
