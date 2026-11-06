use crate::data::SwitchKey;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, MapVirtualKeyW, RegisterHotKey, UnregisterHotKey, keybd_event};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, EnumWindows, GetClassNameW, GetForegroundWindow, GetMessageW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, PeekMessageW, PM_NOREMOVE, PostThreadMessageW, SW_RESTORE, SetForegroundWindow,
    SetWindowsHookExW, ShowWindow, WH_KEYBOARD_LL, WM_APP, WM_HOTKEY, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

const HOTKEY_ID: i32 = 0x5748;
const MOD_NOREPEAT: u32 = 0x4000;
const CARRY_KEYS: [u32; 6] = [0x57, 0x41, 0x53, 0x44, 0x20, 0xA0];

static PHYSICALLY_DOWN: [AtomicBool; 256] = [const { AtomicBool::new(false) }; 256];
static HOTKEY_VK: AtomicU32 = AtomicU32::new(0);
static THREAD: AtomicU32 = AtomicU32::new(0);
static REQUEST: Mutex<Option<(Option<SwitchKey>, mpsc::Sender<bool>)>> = Mutex::new(None);

pub fn start(initial: Option<SwitchKey>) -> bool {
    let (reply, answer) = mpsc::channel();
    thread::spawn(move || {
        let mut message: MSG = unsafe { std::mem::zeroed() };
        unsafe {
            PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(on_keyboard), GetModuleHandleW(std::ptr::null()), 0);
        }
        THREAD.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
        let _ = reply.send(register(initial.as_ref()));
        while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
            if message.message == WM_HOTKEY && message.wParam == HOTKEY_ID as usize {
                let key = HOTKEY_VK.load(Ordering::SeqCst);
                thread::spawn(move || activate_next(key));
            } else if message.message == WM_APP {
                if let Some((key, reply)) = REQUEST.lock().unwrap().take() {
                    let _ = reply.send(register(key.as_ref()));
                }
            }
        }
    });
    answer.recv_timeout(Duration::from_secs(2)).unwrap_or(false)
}

pub fn bind(key: Option<SwitchKey>) -> bool {
    let (reply, answer) = mpsc::channel();
    *REQUEST.lock().unwrap() = Some((key, reply));
    if unsafe { PostThreadMessageW(THREAD.load(Ordering::SeqCst), WM_APP, 0, 0) } == 0 {
        return false;
    }
    answer.recv_timeout(Duration::from_secs(2)).unwrap_or(false)
}

fn register(key: Option<&SwitchKey>) -> bool {
    unsafe { UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID) };
    HOTKEY_VK.store(0, Ordering::SeqCst);
    let Some(key) = key else {
        return true;
    };
    if unsafe { RegisterHotKey(std::ptr::null_mut(), HOTKEY_ID, key.modifiers | MOD_NOREPEAT, key.virtual_key) } == 0 {
        return false;
    }
    HOTKEY_VK.store(key.virtual_key, Ordering::SeqCst);
    true
}

unsafe extern "system" fn on_keyboard(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0 {
        let event = unsafe { &*(data as *const KBDLLHOOKSTRUCT) };
        if event.flags & LLKHF_INJECTED == 0 && event.vkCode < 256 {
            let down = matches!(message as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            if down || matches!(message as u32, WM_KEYUP | WM_SYSKEYUP) {
                PHYSICALLY_DOWN[event.vkCode as usize].store(down, Ordering::SeqCst);
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, message, data) }
}

fn roblox_pids() -> HashSet<u32> {
    let mut pids = HashSet::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() || snapshot as isize == -1 {
            return pids;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let end = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..end]).eq_ignore_ascii_case("RobloxPlayerBeta.exe") {
                pids.insert(entry.th32ProcessID);
            }
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    pids
}

fn text_of(window: HWND, read: unsafe extern "system" fn(HWND, *mut u16, i32) -> i32) -> String {
    let mut buffer = [0u16; 256];
    let length = unsafe { read(window, buffer.as_mut_ptr(), buffer.len() as i32) }.max(0) as usize;
    String::from_utf16_lossy(&buffer[..length])
}

fn is_game_sized(window: HWND) -> bool {
    if unsafe { IsIconic(window) } != 0 {
        return true;
    }
    let mut rect: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetWindowRect(window, &mut rect) != 0 && rect.right - rect.left >= 300 && rect.bottom - rect.top >= 200 }
}

fn is_store_roblox(window: HWND) -> bool {
    text_of(window, GetClassNameW) == "WINDOWSCLIENT" && text_of(window, GetWindowTextW).to_ascii_lowercase().starts_with("roblox")
}

struct Search {
    pids: HashSet<u32>,
    by_process: Vec<usize>,
    by_title: Vec<usize>,
}

unsafe extern "system" fn visit(window: HWND, param: LPARAM) -> i32 {
    let search = unsafe { &mut *(param as *mut Search) };
    if unsafe { IsWindowVisible(window) } != 0 && is_game_sized(window) {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(window, &mut pid) };
        if search.pids.contains(&pid) {
            search.by_process.push(window as usize);
        } else if is_store_roblox(window) {
            search.by_title.push(window as usize);
        }
    }
    1
}

fn find_all() -> Vec<usize> {
    let mut search = Search { pids: roblox_pids(), by_process: Vec::new(), by_title: Vec::new() };
    unsafe { EnumWindows(Some(visit), &mut search as *mut Search as LPARAM) };
    let mut found = if search.by_process.is_empty() { search.by_title } else { search.by_process };
    found.sort();
    found
}

pub fn next_index(active: Option<usize>, count: usize) -> usize {
    active.map_or(0, |index| (index + 1) % count)
}

fn send_key(key: u32, up: bool) {
    unsafe { keybd_event(key as u8, MapVirtualKeyW(key, MAPVK_VK_TO_VSC) as u8, if up { KEYEVENTF_KEYUP } else { 0 }, 0) };
}

fn bring_to_front(target: HWND, foreground: HWND) {
    unsafe {
        let mine = GetCurrentThreadId();
        let theirs = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
        let attached = theirs != 0 && theirs != mine && AttachThreadInput(mine, theirs, 1) != 0;
        SetForegroundWindow(target);
        if attached {
            AttachThreadInput(mine, theirs, 0);
        }
    }
}

fn activate_next(hotkey: u32) {
    let windows = find_all();
    if windows.is_empty() {
        return;
    }
    let foreground = unsafe { GetForegroundWindow() };
    let target = windows[next_index(windows.iter().position(|&window| window == foreground as usize), windows.len())] as HWND;
    if target == foreground {
        return;
    }
    let held: Vec<u32> = CARRY_KEYS.into_iter().filter(|&key| key != hotkey && PHYSICALLY_DOWN[key as usize].load(Ordering::SeqCst)).collect();
    for &key in &held {
        send_key(key, true);
    }
    if unsafe { IsIconic(target) } != 0 {
        unsafe { ShowWindow(target, SW_RESTORE) };
    }
    bring_to_front(target, foreground);
    for _ in 0..20 {
        if unsafe { GetForegroundWindow() } == target {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    thread::sleep(Duration::from_millis(50));
    for key in held.into_iter().filter(|&key| PHYSICALLY_DOWN[key as usize].load(Ordering::SeqCst)) {
        send_key(key, false);
    }
}

#[cfg(test)]
mod tests {
    use super::next_index;

    #[test]
    fn next_window_wraps_around() {
        assert_eq!(next_index(None, 3), 0);
        assert_eq!(next_index(Some(0), 3), 1);
        assert_eq!(next_index(Some(2), 3), 0);
    }
}
