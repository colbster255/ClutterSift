//! Explicit, fixed local-AI setup. Never accepts executable names, arguments, or endpoints from IPC.
use reqwest::blocking::{Client, Response};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
pub const MODEL: &str = "qwen2.5:1.5b";
const ENDPOINT: &str = "http://127.0.0.1:11434";
const HIDDEN: u32 = 0x08000000;
const SETUP_LINKS: &[&str] = &[
    "https://github.com/ollama/ollama/blob/main/LICENSE",
    "https://ollama.com/library/qwen2.5:1.5b",
    "https://ollama.com/download/windows",
    "https://docs.ollama.com/windows",
    "https://openrouter.ai",
    "https://openrouter.ai/settings/keys",
    "https://openrouter.ai/docs/guides/features/structured-outputs",
    "https://openrouter.ai/models?fmt=cards&supported_parameters=structured_outputs",
];
pub fn open_link(url: &str) -> Result<(), String> {
    if !SETUP_LINKS.contains(&url) {
        return Err("Unknown setup link.".into());
    }
    let target: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    // SAFETY: fixed allowlisted HTTPS URL and live NUL-terminated UTF-16 buffers.
    // Windows opens the user's default browser; no arbitrary command or path is accepted.
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as isize <= 32 {
        Err("Could not open the browser. Copy the setup link into your browser.".into())
    } else {
        Ok(())
    }
}
#[derive(Serialize)]
pub struct LocalStatus {
    pub running: bool,
    pub models: Vec<String>,
}
fn client(seconds: u64) -> Result<Client, String> {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(seconds))
        .build()
        .map_err(|_| "Could not initialize local setup transport.".into())
}
fn bounded_json(response: Response) -> Result<Value, String> {
    if !response.status().is_success() {
        return Err(format!("Local Ollama returned HTTP {}.", response.status()));
    }
    let mut bytes = Vec::new();
    response
        .take(131073)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read local Ollama response.")?;
    if bytes.len() > 131072 {
        return Err("Local Ollama response was too large.".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Local Ollama returned invalid JSON.".into())
}
pub fn status() -> Result<LocalStatus, String> {
    let response = match client(5)?.get(format!("{ENDPOINT}/api/tags")).send() {
        Ok(response) => response,
        Err(error) if error.is_connect() || error.is_timeout() => {
            return Ok(LocalStatus {
                running: false,
                models: vec![],
            })
        }
        Err(_) => return Err("Could not check local Ollama.".into()),
    };
    let value = bounded_json(response)?;
    let models = value
        .get("models")
        .and_then(Value::as_array)
        .ok_or("Local service did not return an Ollama model list.")?
        .iter()
        .filter_map(|m| m.get("name").and_then(Value::as_str))
        .filter(|name| !name.to_lowercase().contains("cloud"))
        .map(str::to_owned)
        .collect();
    Ok(LocalStatus {
        running: true,
        models,
    })
}
fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Local setup cancelled. Installed software and downloaded model data are retained; setup can be run again.".into())
    } else {
        Ok(())
    }
}
fn ollama() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Programs/Ollama/ollama.exe");
    path.is_file().then_some(path)
}
fn install(cancel: &AtomicBool) -> Result<(), String> {
    let mut child = Command::new("winget.exe")
        .args(["install", "--id", "Ollama.Ollama", "--exact", "--source", "winget", "--scope", "user", "--silent", "--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity"])
        .creation_flags(HIDDEN).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map_err(|_| "Windows Package Manager is unavailable. Install App Installer from Microsoft Store, or install Ollama from ollama.com/download/windows, then retry setup.")?;
    let start = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) || start.elapsed() > Duration::from_secs(900) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Stopped waiting for the Ollama installer. A separate installer may still finish; installed files are retained. Check Windows Installed apps before retrying.".into());
        }
        if let Some(exit) = child
            .try_wait()
            .map_err(|_| "Could not monitor the Ollama installer.")?
        {
            // An existing installation can produce a non-zero winget exit.
            if exit.success() || ollama().is_some() {
                return Ok(());
            }
            return Err(format!("Ollama installation failed (exit {exit}). Install Ollama from ollama.com/download/windows, open it, then retry setup."));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}
fn pull<R: BufRead>(
    mut reader: R,
    cancel: &AtomicBool,
    progress: &impl Fn(&str),
) -> Result<(), String> {
    let started = Instant::now();
    let mut success = false;
    let mut last = String::new();
    loop {
        cancelled(cancel)?;
        if started.elapsed() > Duration::from_secs(1800) {
            return Err("Model download timed out. Retry setup to resume.".into());
        }
        let mut line = Vec::new();
        let count = reader
            .by_ref()
            .take(8193)
            .read_until(b'\n', &mut line)
            .map_err(|_| "Model download interrupted. Retry setup to resume.")?;
        if count == 0 {
            break;
        }
        if line.len() > 8192 {
            return Err("Ollama download status was too large.".into());
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let value: Value =
            serde_json::from_slice(&line).map_err(|_| "Invalid Ollama download status.")?;
        if value.get("error").is_some() {
            return Err("Ollama could not download the model. Check your connection and available disk space, then retry setup.".into());
        }
        let status = value
            .get("status")
            .and_then(Value::as_str)
            .ok_or("Missing Ollama download status.")?;
        success = status == "success";
        let message = match (
            value.get("completed").and_then(Value::as_u64),
            value.get("total").and_then(Value::as_u64),
        ) {
            (Some(done), Some(total)) if total > 0 => format!(
                "Downloading model: {}%",
                done.saturating_mul(100)
                    .checked_div(total)
                    .unwrap_or(0)
                    .min(100)
            ),
            _ => status.chars().take(120).collect(),
        };
        if message != last {
            progress(&message);
            last = message;
        }
    }
    cancelled(cancel)?;
    if success {
        Ok(())
    } else {
        Err("Model download ended before completion. Retry setup to resume.".into())
    }
}
struct CancelRead<'a, R>(&'a AtomicBool, R);
impl<R: Read> Read for CancelRead<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.0.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("Setup cancelled"));
        }
        self.1.read(buffer)
    }
}
pub fn setup(
    approved: bool,
    cancel: &AtomicBool,
    progress: impl Fn(&str),
) -> Result<LocalStatus, String> {
    if !approved {
        return Err("Choose Install Ollama & download model to authorize setup.".into());
    }
    cancelled(cancel)?;
    progress("Checking local Ollama…");
    if !status()?.running {
        if ollama().is_none() {
            progress("Installing Ollama for your Windows account…");
            install(cancel)?;
        }
        cancelled(cancel)?;
        if !status()?.running {
            progress("Starting local Ollama…");
            let executable = ollama().ok_or("Ollama was not found in its default installation folder. Open Ollama from the Start menu, then retry setup.")?;
            // The server intentionally remains available after setup; no shell, elevation or startup entry is added.
            Command::new(executable)
                .arg("serve")
                .env("OLLAMA_HOST", "127.0.0.1:11434")
                .env("OLLAMA_NO_CLOUD", "1")
                .creation_flags(HIDDEN)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|_| {
                    "Could not start Ollama. Open it from the Start menu, then retry setup."
                })?;
            let mut running = false;
            for _ in 0..30 {
                cancelled(cancel)?;
                if status()?.running {
                    running = true;
                    break;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            if !running {
                return Err(
                    "Ollama did not become ready. Open it from the Start menu and retry setup."
                        .into(),
                );
            }
        }
    }
    let installed = status()?;
    if !installed.models.iter().any(|m| m == MODEL) {
        progress("Downloading qwen2.5:1.5b (about 1 GB)…");
        // Blocking reqwest applies this deadline to each read, so a stalled
        // local service cannot indefinitely postpone cancellation.
        let response = client(20)?
            .post(format!("{ENDPOINT}/api/pull"))
            .json(&json!({"model":MODEL,"stream":true}))
            .send()
            .map_err(|_| "Could not start the model download. Check Ollama and retry setup.")?;
        if !response.status().is_success() {
            return Err(format!(
                "Model download returned HTTP {}.",
                response.status()
            ));
        }
        pull(
            BufReader::new(CancelRead(cancel, response)),
            cancel,
            &progress,
        )?;
    }
    cancelled(cancel)?;
    let ready = status()?;
    if !ready.running || !ready.models.iter().any(|m| m == MODEL) {
        return Err("Model download was not confirmed by Ollama. Retry setup.".into());
    }
    progress("Local model installed and Ollama is reachable.");
    Ok(ready)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_setup_links_reject_untrusted_destinations_without_launching() {
        for url in [
            "file:///C:/Windows/System32/cmd.exe",
            "https://example.org",
            "https://openrouter.ai.evil.invalid",
            "https://openrouter.ai/settings/keys?redirect=other",
        ] {
            assert!(open_link(url).is_err());
        }
    }
    #[test]
    fn truncated_error_and_oversized_download_streams_never_report_ready() {
        let cancel = AtomicBool::new(false);
        for input in [
            b"{\"status\":\"pulling\"}\n".to_vec(),
            b"{\"error\":\"disk full\"}\n".to_vec(),
            vec![b'x'; 8193],
            b"invalid\n".to_vec(),
        ] {
            assert!(pull(std::io::Cursor::new(input), &cancel, &|_| {}).is_err());
        }
        assert!(pull(
            std::io::Cursor::new(b"{\"status\":\"success\"}\n"),
            &cancel,
            &|_| {}
        )
        .is_ok());
        cancel.store(true, Ordering::Relaxed);
        assert!(pull(
            std::io::Cursor::new(b"{\"status\":\"success\"}\n"),
            &cancel,
            &|_| {}
        )
        .is_err());
        assert!(setup(false, &cancel, |_| {}).is_err());
        let mut reader = CancelRead(&cancel, std::io::Cursor::new(b"data"));
        let mut data = Vec::new();
        assert!(reader.read_to_end(&mut data).is_err());
        assert!(data.is_empty());
    }
}
