//! Submit command for data-fabrication harness.
//!
//! Collects harness files (.py, requirements.txt), signs with sr25519 keypair,
//! and submits to the validator RPC endpoint.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use sp_core::{sr25519, Pair};
use walkdir::WalkDir;

/// Directories to skip when collecting harness files.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "__pycache__",
    "node_modules",
    ".venv",
    "venv",
    ".mypy_cache",
    ".pytest_cache",
    "images",
    "docs",
    ".githooks",
];

/// Allowed file extensions for harness.
const ALLOWED_EXTS: &[&str] = &[".py", ".txt", ".cfg", ".yaml", ".yml", ".json", ".sh"];

/// File extensions to always skip.
const SKIP_EXTS: &[&str] = &[
    ".pyc", ".pyo", ".so", ".png", ".jpg", ".gif", ".ico", ".md", ".wasm",
];

/// A file entry in the submission payload.
#[derive(serde::Serialize)]
struct FileEntry {
    path: String,
    content: String,
    size: usize,
}

/// Code payload containing collected files.
#[derive(serde::Serialize)]
struct CodePayload {
    files: Vec<FileEntry>,
}

/// Submit request body.
#[derive(serde::Serialize)]
struct SubmitBody {
    name: String,
    code: String,
}

/// Submit response from validator.
#[derive(serde::Deserialize)]
struct SubmitResponse {
    agent_hash: Option<String>,
    epoch: Option<u64>,
    #[allow(dead_code)]
    name: Option<String>,
    version: Option<u64>,
    #[allow(dead_code)]
    error: Option<String>,
}

/// Collect files from harness directory.
fn collect_files(dir: &Path) -> Result<Vec<FileEntry>> {
    let mut files = Vec::new();

    for entry in WalkDir::new(dir).sort_by_file_name() {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy();

        if entry.file_type().is_dir() {
            if SKIP_DIRS.iter().any(|d| *d == name.as_ref()) {
                continue;
            }
            continue;
        }

        if SKIP_EXTS.iter().any(|e| name.ends_with(e)) {
            continue;
        }
        if !ALLOWED_EXTS.iter().any(|e| name.ends_with(e)) {
            continue;
        }

        let rel = entry
            .path()
            .strip_prefix(dir)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();

        match std::fs::read_to_string(entry.path()) {
            Ok(content) => {
                let size = content.len();
                files.push(FileEntry {
                    path: rel,
                    content,
                    size,
                });
            }
            Err(_) => continue,
        }
    }

    Ok(files)
}

/// Canonicalize JSON value for deterministic hashing.
fn canonicalize_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut pairs: Vec<_> = map.iter().collect();
            pairs.sort_by_key(|(k, _)| *k);
            let inner: Vec<String> = pairs
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap(),
                        canonicalize_json(v)
                    )
                })
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        serde_json::Value::Array(arr) => {
            let inner: Vec<String> = arr.iter().map(canonicalize_json).collect();
            format!("[{}]", inner.join(","))
        }
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

/// Sign a submit request for authentication.
fn sign_submit(
    keypair: &sr25519::Pair,
    challenge_id: &str,
    body: &serde_json::Value,
) -> (String, String, String) {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let nonce = format!("{}:{}", timestamp, &uuid::Uuid::new_v4().to_string()[..8]);

    let canonical = canonicalize_json(body);
    let body_hash = hex::encode(Sha256::digest(canonical.as_bytes()));

    let message = format!(
        "challenge:{}:POST:/submit:{}:{}",
        challenge_id, body_hash, nonce
    );
    let signature = keypair.sign(message.as_bytes());

    (
        hex::encode(signature.0),
        nonce,
        hex::encode(keypair.public().0),
    )
}

/// Parse a secret key (mnemonic or hex seed) into an sr25519 keypair.
pub fn parse_keypair(secret: &str) -> Result<sr25519::Pair> {
    if secret.starts_with("0x") || secret.starts_with("//") {
        sr25519::Pair::from_string(secret, None)
            .map_err(|e| anyhow::anyhow!("Invalid secret seed: {:?}", e))
    } else {
        sr25519::Pair::from_phrase(secret, None)
            .map_err(|e| anyhow::anyhow!("Invalid mnemonic: {:?}", e))
            .map(|(pair, _)| pair)
    }
}

/// Submit a harness to the validator.
///
/// # Arguments
/// * `harness_dir` - Path to the harness directory containing .py files
/// * `rpc_url` - Validator RPC endpoint URL
/// * `hotkey` - sr25519 keypair for signing
///
/// # Returns
/// * `Ok(String)` - Submission ID (agent_hash) on success
/// * `Err` - On failure
pub async fn submit_command(
    harness_dir: &Path,
    rpc_url: &str,
    hotkey: &sr25519::Pair,
) -> Result<String> {
    const CHALLENGE_ID: &str = "data-fabrication";

    if !harness_dir.is_dir() {
        bail!("Not a directory: {}", harness_dir.display());
    }

    // Collect files from harness directory
    let files = collect_files(harness_dir)?;
    if files.is_empty() {
        bail!("No harness files found in {}", harness_dir.display());
    }

    let total_size: usize = files.iter().map(|f| f.size).sum();
    tracing::info!(
        "Collected {} files ({} bytes total) from {}",
        files.len(),
        total_size,
        harness_dir.display()
    );

    for f in &files {
        tracing::debug!("  {} ({} bytes)", f.path, f.size);
    }

    // Build submission payload
    let name = harness_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "harness".to_string());

    let code_payload = CodePayload { files };
    let code_json = serde_json::to_string(&code_payload)?;
    let body_obj = SubmitBody {
        name: name.clone(),
        code: code_json,
    };
    let body_value = serde_json::to_value(&body_obj)?;
    let body_str = serde_json::to_string(&body_obj)?;

    // Sign the request
    let (signature, nonce, hotkey_hex) = sign_submit(hotkey, CHALLENGE_ID, &body_value);

    tracing::info!("Submitting harness '{}' to {}", name, rpc_url);
    tracing::debug!("Hotkey: {}", hotkey_hex);

    // Send POST request to validator
    let client = reqwest::Client::new();
    let url = format!("{}/challenge/{}/submit", rpc_url, CHALLENGE_ID);
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-hotkey", &hotkey_hex)
        .header("x-signature", &signature)
        .header("x-nonce", &nonce)
        .body(body_str)
        .send()
        .await
        .context("Failed to send submit request")?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);

    if status.is_success() {
        let resp: SubmitResponse = serde_json::from_value(body.clone()).unwrap_or(SubmitResponse {
            agent_hash: None,
            epoch: None,
            name: None,
            version: None,
            error: None,
        });

        if let Some(hash) = &resp.agent_hash {
            tracing::info!("Submission successful! Agent hash: {}", hash);
        }
        if let Some(v) = resp.version {
            tracing::info!("Version: {}", v);
        }
        if let Some(e) = resp.epoch {
            tracing::info!("Epoch: {}", e);
        }

        resp.agent_hash.ok_or_else(|| {
            anyhow::anyhow!("Submit succeeded but no agent_hash returned")
        })
    } else {
        let msg = body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown error");
        bail!("Submit failed ({}): {}", status.as_u16(), msg)
    }
}

/// Run interactive submit flow (prompts for inputs).
pub async fn run_interactive_submit(rpc_url: &str) -> Result<()> {
    use dialoguer::{Input, Select};

    const CHALLENGE_ID: &str = "data-fabrication";

    println!("\n  Data Fabrication - Harness Submit\n");

    // Get keypair from user
    let secret: String = Input::new()
        .with_prompt("  Mnemonic or secret seed (0x...)")
        .interact_text()?;

    let keypair = parse_keypair(&secret)?;
    let hotkey_hex = hex::encode(keypair.public().0);
    println!("  Hotkey: {}", hotkey_hex);

    // Get harness directory
    let dir_str: String = Input::new()
        .with_prompt("  Harness directory path")
        .interact_text()?;

    let harness_dir = PathBuf::from(shellexpand::tilde(&dir_str).to_string());
    if !harness_dir.is_dir() {
        bail!("Not a directory: {}", harness_dir.display());
    }

    // Collect and display files
    println!("\n  Collecting files from {}...", harness_dir.display());
    let files = collect_files(&harness_dir)?;
    if files.is_empty() {
        bail!("No source files found in {}", harness_dir.display());
    }

    let total_size: usize = files.iter().map(|f| f.size).sum();
    println!(
        "  Found {} files ({} bytes total)\n",
        files.len(),
        total_size
    );
    for f in &files {
        println!("    {} ({} bytes)", f.path, f.size);
    }

    // Confirm submission
    println!();
    let choices = &["Yes, submit", "No, cancel"];
    let selection = Select::new()
        .with_prompt("  Submit this harness?")
        .items(choices)
        .default(0)
        .interact()?;

    if selection != 0 {
        println!("  Cancelled.");
        return Ok(());
    }

    // Build payload
    let name = harness_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "harness".to_string());

    let code_payload = CodePayload { files };
    let code_json = serde_json::to_string(&code_payload)?;
    let body_obj = SubmitBody {
        name: name.clone(),
        code: code_json,
    };
    let body_value = serde_json::to_value(&body_obj)?;
    let body_str = serde_json::to_string(&body_obj)?;

    // Sign request
    let (signature, nonce, hotkey) = sign_submit(&keypair, CHALLENGE_ID, &body_value);

    println!("\n  Submitting...");
    let client = reqwest::Client::new();
    let url = format!("{}/challenge/{}/submit", rpc_url, CHALLENGE_ID);
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-hotkey", &hotkey)
        .header("x-signature", &signature)
        .header("x-nonce", &nonce)
        .body(body_str)
        .send()
        .await
        .context("Failed to send submit request")?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);

    if status.is_success() {
        let resp: SubmitResponse = serde_json::from_value(body.clone()).unwrap_or(SubmitResponse {
            agent_hash: None,
            epoch: None,
            name: None,
            version: None,
            error: None,
        });
        println!("\n  Submitted successfully!");
        if let Some(hash) = &resp.agent_hash {
            println!("  Agent hash: {}", hash);
        }
        if let Some(v) = resp.version {
            println!("  Version:    {}", v);
        }
        if let Some(e) = resp.epoch {
            println!("  Epoch:      {}", e);
        }
        println!();
    } else {
        let msg = body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown error");
        println!("\n  Submit failed ({}): {}\n", status.as_u16(), msg);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_collect_files_empty() {
        let dir = TempDir::new().unwrap();
        let files = collect_files(dir.path()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_collect_files_py() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("harness.py"), "print('hello')").unwrap();

        let files = collect_files(dir.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "harness.py");
        assert_eq!(files[0].content, "print('hello')");
    }

    #[test]
    fn test_collect_files_requirements() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("requirements.txt"), "requests\n").unwrap();

        let files = collect_files(dir.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "requirements.txt");
    }

    #[test]
    fn test_collect_files_skip_pyc() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("harness.py"), "pass").unwrap();
        fs::create_dir(dir.path().join("__pycache__")).unwrap();
        fs::write(dir.path().join("__pycache__/harness.pyc"), b"\x00").unwrap();

        let files = collect_files(dir.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "harness.py");
    }

    #[test]
    fn test_canonicalize_json() {
        let value = serde_json::json!({"b": 2, "a": 1});
        let canonical = canonicalize_json(&value);
        assert_eq!(canonical, r#"{"a":1,"b":2}"#);
    }

    #[test]
    fn test_canonicalize_json_nested() {
        let value = serde_json::json!({"outer": {"b": 2, "a": 1}});
        let canonical = canonicalize_json(&value);
        assert_eq!(canonical, r#"{"outer":{"a":1,"b":2}}"#);
    }

    #[test]
    fn test_parse_keypair_mnemonic() {
        // A valid test mnemonic (not real)
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let result = parse_keypair(mnemonic);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_keypair_hex() {
        let hex = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let result = parse_keypair(hex);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sign_submit_produces_valid_format() {
        let keypair = sr25519::Pair::from_string("//Alice", None).unwrap();
        let body = serde_json::json!({"name": "test", "code": "{}"});

        let (sig, nonce, hotkey) = sign_submit(&keypair, "data-fabrication", &body);

        // Signature should be 128 hex chars (64 bytes)
        assert_eq!(sig.len(), 128);
        // Nonce should match format timestamp:uuid
        assert!(nonce.contains(':'));
        // Hotkey should be 64 hex chars (32 bytes)
        assert_eq!(hotkey.len(), 64);
    }
}
