use crate::roblox_api;
use crate::win;
use serde_json::Value;
use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use std::os::windows::io::AsRawHandle;

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

const BROWSERS: [&str; 4] = ["brave.exe", "msedge.exe", "chrome.exe", "opera.exe"];

pub struct Login {
    pub id: String,
    pub name: String,
    pub sealed: String,
}

struct Job(HANDLE);

impl Job {
    fn new() -> Option<Job> {
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return None;
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let size = size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
            SetInformationJobObject(handle, JobObjectExtendedLimitInformation, &limits as *const _ as *const _, size);
            Some(Job(handle))
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

fn registry_path(root: windows_sys::Win32::System::Registry::HKEY, exe: &str) -> Option<PathBuf> {
    let key = win::wide(&format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe}"));
    let mut buffer = [0u16; 1024];
    let mut size = (buffer.len() * 2) as u32;
    let status = unsafe { RegGetValueW(root, key.as_ptr(), std::ptr::null(), RRF_RT_REG_SZ, std::ptr::null_mut(), buffer.as_mut_ptr() as _, &mut size) };
    if status != 0 {
        return None;
    }
    let text = String::from_utf16_lossy(&buffer[..(size as usize / 2).saturating_sub(1)]);
    let path = PathBuf::from(text.trim().trim_matches('"'));
    path.is_file().then_some(path)
}

pub fn find_browser() -> Option<PathBuf> {
    BROWSERS.iter().find_map(|exe| [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER].into_iter().find_map(|root| registry_path(root, exe)))
}

pub fn add_account() -> Result<Login, String> {
    let browser = find_browser().ok_or("No Chromium browser found for the login window")?;
    let cookie = capture_cookie(&browser).ok_or("Login cancelled")?;
    let (id, name) = roblox_api::who_am_i(&cookie)?;
    let sealed = crate::seal::seal(&cookie)?;
    Ok(Login { id, name, sealed })
}

fn capture_cookie(browser: &Path) -> Option<String> {
    let profile = std::env::temp_dir().join(format!("waccounts_login_{}", win::new_session_id()));
    let job = Job::new()?;
    let child = Command::new(browser)
        .arg(format!("--user-data-dir={}", profile.display()))
        .args(["--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check", "https://www.roblox.com/login"])
        .spawn()
        .ok()?;
    unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle() as HANDLE) };
    let cookie = connect(&profile.join("DevToolsActivePort")).and_then(|mut socket| {
        let cookie = wait_for_cookie(&mut socket);
        let _ = socket.send(Message::text(r#"{"id":2,"method":"Browser.close"}"#));
        thread::sleep(Duration::from_millis(500));
        cookie
    });
    drop(job);
    remove_profile(&profile);
    cookie
}

fn connect(port_file: &Path) -> Option<Socket> {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !port_file.exists() {
        if Instant::now() > deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(200));
    }
    thread::sleep(Duration::from_millis(200));
    let text = fs::read_to_string(port_file).ok()?;
    let mut lines = text.lines();
    let (port, path) = (lines.next()?.trim(), lines.next()?.trim());
    tungstenite::connect(format!("ws://127.0.0.1:{port}{path}")).ok().map(|(socket, _)| socket)
}

fn wait_for_cookie(socket: &mut Socket) -> Option<String> {
    let deadline = Instant::now() + Duration::from_secs(15 * 60);
    while Instant::now() < deadline {
        socket.send(Message::text(r#"{"id":1,"method":"Storage.getCookies"}"#)).ok()?;
        loop {
            let reply = match socket.read().ok()? {
                Message::Text(text) => text.to_string(),
                Message::Close(_) => return None,
                _ => continue,
            };
            let value: Value = serde_json::from_str(&reply).unwrap_or(Value::Null);
            if value.get("id").and_then(Value::as_u64) != Some(1) {
                continue;
            }
            if let Some(cookie) = parse_session_cookie(&value) {
                return Some(cookie);
            }
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }
    None
}

pub fn parse_session_cookie(reply: &Value) -> Option<String> {
    reply.get("result")?.get("cookies")?.as_array()?.iter().find_map(|cookie| {
        let roblox = cookie.get("domain").and_then(Value::as_str).is_some_and(|domain| domain.to_ascii_lowercase().ends_with("roblox.com"));
        let session = cookie.get("name").and_then(Value::as_str) == Some(".ROBLOSECURITY");
        (roblox && session).then(|| cookie.get("value").and_then(Value::as_str).map(str::to_string)).flatten()
    })
}

fn remove_profile(profile: &Path) {
    for _ in 0..25 {
        if !profile.exists() || fs::remove_dir_all(profile).is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(400));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_cookie_comes_from_roblox_only() {
        let reply: Value = serde_json::from_str(
            r#"{"id":1,"result":{"cookies":[
                {"name":".ROBLOSECURITY","domain":"evil.test","value":"no"},
                {"name":"other","domain":".roblox.com","value":"no"},
                {"name":".ROBLOSECURITY","domain":".ROBLOX.com","value":"yes"}]}}"#,
        )
        .unwrap();
        assert_eq!(parse_session_cookie(&reply).as_deref(), Some("yes"));
        assert_eq!(parse_session_cookie(&serde_json::from_str(r#"{"id":1,"result":{"cookies":[]}}"#).unwrap()), None);
        assert_eq!(parse_session_cookie(&serde_json::from_str(r#"{"id":1,"error":{}}"#).unwrap()), None);
    }
}
