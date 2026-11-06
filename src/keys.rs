use crate::data::SwitchKey;
use eframe::egui::{Key, Modifiers};

const NAMED: [(Key, u32, &str); 22] = [
    (Key::Space, 0x20, "Space"),
    (Key::Tab, 0x09, "Tab"),
    (Key::Enter, 0x0D, "Enter"),
    (Key::Insert, 0x2D, "Insert"),
    (Key::Delete, 0x2E, "Delete"),
    (Key::Home, 0x24, "Home"),
    (Key::End, 0x23, "End"),
    (Key::PageUp, 0x21, "PageUp"),
    (Key::PageDown, 0x22, "PageDown"),
    (Key::ArrowLeft, 0x25, "Left"),
    (Key::ArrowUp, 0x26, "Up"),
    (Key::ArrowRight, 0x27, "Right"),
    (Key::ArrowDown, 0x28, "Down"),
    (Key::Minus, 0xBD, "-"),
    (Key::Equals, 0xBB, "="),
    (Key::Comma, 0xBC, ","),
    (Key::Period, 0xBE, "."),
    (Key::Semicolon, 0xBA, ";"),
    (Key::Slash, 0xBF, "/"),
    (Key::OpenBracket, 0xDB, "["),
    (Key::CloseBracket, 0xDD, "]"),
    (Key::Backtick, 0xC0, "`"),
];

const LETTERS: [Key; 26] = [
    Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::G, Key::H, Key::I, Key::J, Key::K, Key::L, Key::M,
    Key::N, Key::O, Key::P, Key::Q, Key::R, Key::S, Key::T, Key::U, Key::V, Key::W, Key::X, Key::Y, Key::Z,
];

const DIGITS: [Key; 10] = [Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];

const FUNCTIONS: [Key; 24] = [
    Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12,
    Key::F13, Key::F14, Key::F15, Key::F16, Key::F17, Key::F18, Key::F19, Key::F20, Key::F21, Key::F22, Key::F23, Key::F24,
];

pub enum Capture {
    Cancel,
    Clear,
    Refused(String),
    Set(SwitchKey),
}

fn virtual_key(key: Key) -> Option<(u32, String)> {
    if let Some(index) = LETTERS.iter().position(|&letter| letter == key) {
        return Some((0x41 + index as u32, char::from(b'A' + index as u8).to_string()));
    }
    if let Some(index) = DIGITS.iter().position(|&digit| digit == key) {
        return Some((0x30 + index as u32, index.to_string()));
    }
    if let Some(index) = FUNCTIONS.iter().position(|&function| function == key) {
        return Some((0x70 + index as u32, format!("F{}", index + 1)));
    }
    NAMED.iter().find(|(named, _, _)| *named == key).map(|(_, code, label)| (*code, label.to_string()))
}

pub fn capture(key: Key, modifiers: Modifiers) -> Option<Capture> {
    match key {
        Key::Escape => return Some(Capture::Cancel),
        Key::Backspace => return Some(Capture::Clear),
        _ => {}
    }
    let (code, name) = virtual_key(key)?;
    let plain = !modifiers.ctrl && !modifiers.alt && !modifiers.shift;
    if plain && (name == "F2" || name == "F3") {
        return Some(Capture::Refused(format!("{name} is kept for starting and stopping macros")));
    }
    let mut parts = Vec::new();
    let mut flags = 0;
    for (held, flag, label) in [(modifiers.ctrl, 2, "Ctrl"), (modifiers.alt, 1, "Alt"), (modifiers.shift, 4, "Shift")] {
        if held {
            parts.push(label.to_string());
            flags |= flag;
        }
    }
    parts.push(name);
    Some(Capture::Set(SwitchKey { modifiers: flags, virtual_key: code, label: parts.join("+") }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(key: Key, modifiers: Modifiers) -> SwitchKey {
        match capture(key, modifiers) {
            Some(Capture::Set(binding)) => binding,
            _ => panic!("expected a binding"),
        }
    }

    #[test]
    fn keys_map_to_windows_codes() {
        assert_eq!(set(Key::F4, Modifiers::NONE), SwitchKey { modifiers: 0, virtual_key: 0x73, label: "F4".into() });
        assert_eq!(set(Key::Q, Modifiers::CTRL | Modifiers::SHIFT), SwitchKey { modifiers: 6, virtual_key: 0x51, label: "Ctrl+Shift+Q".into() });
        assert_eq!(set(Key::Num7, Modifiers::ALT).virtual_key, 0x37);
    }

    #[test]
    fn macro_keys_are_refused_without_modifiers() {
        assert!(matches!(capture(Key::F2, Modifiers::NONE), Some(Capture::Refused(_))));
        assert_eq!(set(Key::F3, Modifiers::CTRL).label, "Ctrl+F3");
        assert!(matches!(capture(Key::Escape, Modifiers::NONE), Some(Capture::Cancel)));
        assert!(matches!(capture(Key::Backspace, Modifiers::NONE), Some(Capture::Clear)));
    }
}
