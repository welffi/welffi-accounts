use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, IsIconic, SW_RESTORE, SW_SHOWNORMAL, SetForegroundWindow, ShowWindow};

pub const TITLE: &str = "welffi accounts";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

static INSTANCE: AtomicUsize = AtomicUsize::new(0);

pub fn claim_single_instance() -> bool {
    let name = wide("welffi_accounts_single_instance");
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 1, name.as_ptr());
        INSTANCE.store(handle as usize, Ordering::SeqCst);
        GetLastError() != ERROR_ALREADY_EXISTS
    }
}

pub fn release_single_instance() {
    let handle = INSTANCE.swap(0, Ordering::SeqCst);
    if handle != 0 {
        unsafe { CloseHandle(handle as HANDLE) };
    }
}

pub fn focus_running_copy() {
    let title = wide(TITLE);
    unsafe {
        let window = FindWindowW(std::ptr::null(), title.as_ptr());
        if window.is_null() {
            return;
        }
        if IsIconic(window) != 0 {
            ShowWindow(window, SW_RESTORE);
        }
        SetForegroundWindow(window);
    }
}

pub fn open_uri(uri: &str) -> Result<(), String> {
    let (verb, target) = (wide("open"), wide(uri));
    let result = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), target.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) };
    if result as usize > 32 { Ok(()) } else { Err("Windows couldn't start Roblox. Is it installed?".to_string()) }
}

pub fn run_hidden_powershell(script: &str) {
    let _ = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", script]).creation_flags(CREATE_NO_WINDOW).status();
}

pub fn random() -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
    hasher.finish()
}

pub fn new_tracker() -> String {
    format!("{}{}", 100_000 + random() % 75_001, 100_000 + random() % 800_001)
}

pub fn new_session_id() -> String {
    let (high, low) = (random(), random());
    format!("{:08x}-{:04x}-4{:03x}-a{:03x}-{:012x}", high >> 32, (high >> 16) & 0xFFFF, high & 0xFFF, low >> 52, low & 0xFFFF_FFFF_FFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_matches_the_old_range() {
        for _ in 0..50 {
            let tracker = new_tracker();
            let (first, second) = tracker.split_at(6);
            assert!((100_000..=175_000).contains(&first.parse::<u32>().unwrap()));
            assert!((100_000..=900_000).contains(&second.parse::<u32>().unwrap()));
        }
    }

    #[test]
    fn session_id_looks_like_a_guid() {
        let id = new_session_id();
        assert_eq!(id.len(), 36);
        assert_eq!(id.split('-').map(str::len).collect::<Vec<_>>(), [8, 4, 4, 4, 12]);
    }
}
