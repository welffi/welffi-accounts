use crate::data::{Account, local_app_data};
use crate::links;
use crate::roblox_api;
use crate::seal;
use crate::win;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use windows_sys::Win32::Foundation::{HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};

const LOCKED: &str = "A Roblox or the macro is holding the Roblox lock. Close them and try again.";
const CLOSE_TRAY_COPIES: &str = "Get-CimInstance Win32_Process -Filter \"Name='RobloxPlayerBeta.exe'\" | Where-Object { $_.CommandLine -like '*--launch-to-tray*' } | ForEach-Object { $p = Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue; if ($p -and $p.MainWindowHandle -eq 0) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue } }";

static LOCK: AtomicUsize = AtomicUsize::new(0);

pub enum Launch {
    Open { follow: String },
    JoinRecent,
    JoinLink(String),
}

pub fn hold_lock(wait_ms: u32) -> bool {
    let mut handle = LOCK.load(Ordering::SeqCst);
    if handle == 0 {
        let name = win::wide("ROBLOX_singletonMutex");
        handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) } as usize;
        if handle == 0 {
            return false;
        }
        LOCK.store(handle, Ordering::SeqCst);
    }
    let result = unsafe { WaitForSingleObject(handle as HANDLE, wait_ms) };
    result == WAIT_OBJECT_0 || result == WAIT_ABANDONED
}

pub fn ensure_lock() -> Result<(), String> {
    if hold_lock(0) {
        return Ok(());
    }
    win::run_hidden_powershell(CLOSE_TRAY_COPIES);
    if hold_lock(2000) { Ok(()) } else { Err(LOCKED.to_string()) }
}

fn client_storage() -> PathBuf {
    local_app_data().join("Roblox").join("LocalStorage").join("appStorage.json")
}

pub fn clear_saved_accounts(text: &str) -> Option<String> {
    let mut root: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let map = root.as_object_mut()?;
    let empty = Value::String("{}".to_string());
    let previous_set = map.get("PreviousAccountsList").is_some_and(|value| *value != empty);
    if previous_set {
        map.insert("PreviousAccountsList".to_string(), empty);
    }
    let blob_removed = map.shift_remove("AccountBlob").is_some();
    (previous_set || blob_removed).then(|| root.to_string())
}

fn set_read_only(path: &PathBuf, read_only: bool) -> std::io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    if permissions.readonly() != read_only {
        permissions.set_readonly(read_only);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn forget_other_accounts() -> Result<(), String> {
    let path = client_storage();
    if !path.exists() {
        return Ok(());
    }
    let mut failure = String::new();
    for _ in 0..5 {
        let attempt = fs::read_to_string(&path).and_then(|text| {
            if let Some(cleared) = clear_saved_accounts(&text) {
                set_read_only(&path, false)?;
                fs::write(&path, cleared)?;
            }
            set_read_only(&path, true)
        });
        match attempt {
            Ok(()) => return Ok(()),
            Err(error) => failure = error.to_string(),
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(format!("Couldn't prepare Roblox's account storage: {failure}"))
}

pub fn release_client_storage() {
    let _ = set_read_only(&client_storage(), false);
}

fn start(account: &Account, ticket: &str, query: &str) -> Result<(), String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    win::open_uri(&links::launch_uri(&account.tracker, ticket, query, now))
}

pub fn launch(account: &Account, launch: &Launch) -> Result<String, String> {
    forget_other_accounts()?;
    let cookie = seal::unseal(&account.sealed_cookie)?;
    match launch {
        Launch::Open { follow } if follow.is_empty() => {
            start(account, &roblox_api::auth_ticket(&cookie)?, "")?;
            Ok(format!("Opened Roblox on {}", account.name))
        }
        Launch::Open { follow } => {
            let query = links::follow_query(&roblox_api::user_id_of(follow)?);
            start(account, &roblox_api::auth_ticket(&cookie)?, &query)?;
            Ok(format!("{} is following {follow}", account.name))
        }
        Launch::JoinRecent => {
            let (place_id, game) = roblox_api::find_recent_game(&cookie)?;
            start(account, &roblox_api::auth_ticket(&cookie)?, &links::join_query(&account.tracker, &place_id))?;
            Ok(format!("{} is joining {game}", account.name))
        }
        Launch::JoinLink(link) => {
            let server = roblox_api::resolve_private_server(&cookie, link)?;
            start(account, &roblox_api::auth_ticket(&cookie)?, &links::private_query(&account.tracker, &server))?;
            Ok(format!("{} is joining the private server", account.name))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_accounts_are_cleared_and_the_rest_kept() {
        let text = r#"{"A":1,"PreviousAccountsList":"{\"1\":\"x\"}","AccountBlob":"abc","Z":"keep"}"#;
        assert_eq!(clear_saved_accounts(text).unwrap(), r#"{"A":1,"PreviousAccountsList":"{}","Z":"keep"}"#);
    }

    #[test]
    fn already_clear_storage_is_left_alone() {
        assert_eq!(clear_saved_accounts(r#"{"A":1,"PreviousAccountsList":"{}"}"#), None);
        assert_eq!(clear_saved_accounts(r#"{"A":1}"#), None);
        assert_eq!(clear_saved_accounts("not json"), None);
    }
}
