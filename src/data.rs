use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub tracker: String,
    pub sealed_cookie: String,
}

#[derive(Clone, Default, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Server {
    pub name: String,
    pub link: String,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SwitchKey {
    pub modifiers: u32,
    pub virtual_key: u32,
    pub label: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Settings {
    pub servers: Vec<Server>,
    pub last_private_link: String,
    pub launch_gap_ms: u64,
    pub fast_flags: bool,
    pub switch_key: Option<SwitchKey>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            servers: Vec::new(),
            last_private_link: String::new(),
            launch_gap_ms: 2500,
            fast_flags: false,
            switch_key: Some(SwitchKey { modifiers: 0, virtual_key: 0x73, label: "F4".to_string() }),
        }
    }
}

pub fn data_root() -> PathBuf {
    if let Some(root) = env::var_os("WACCOUNTS_DATA") {
        return PathBuf::from(root);
    }
    local_app_data().join("welffi accounts")
}

pub fn local_app_data() -> PathBuf {
    env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default()
}

pub fn old_hub_root() -> PathBuf {
    local_app_data().join("welffi hub")
}

pub fn copy_old_data(old: &Path, new: &Path) -> io::Result<bool> {
    if new.join("accounts.json").exists() || !old.join("accounts.json").exists() {
        return Ok(false);
    }
    fs::create_dir_all(new.join("portraits"))?;
    if old.join("settings.json").exists() {
        fs::copy(old.join("settings.json"), new.join("settings.json"))?;
    }
    if let Ok(entries) = fs::read_dir(old.join("portraits")) {
        for entry in entries.flatten().filter(|entry| entry.path().extension().is_some_and(|ext| ext == "png")) {
            fs::copy(entry.path(), new.join("portraits").join(entry.file_name()))?;
        }
    }
    fs::copy(old.join("accounts.json"), new.join("accounts.json"))?;
    Ok(true)
}

pub struct Store {
    pub root: PathBuf,
    pub accounts: Vec<Account>,
    pub settings: Settings,
}

impl Store {
    pub fn load(root: PathBuf) -> Store {
        let _ = fs::create_dir_all(&root);
        let accounts = read(&root.join("accounts.json"));
        let settings = read(&root.join("settings.json"));
        Store { root, accounts, settings }
    }

    pub fn portraits(&self) -> PathBuf {
        self.root.join("portraits")
    }

    pub fn save_accounts(&self) -> Result<(), String> {
        write(&self.root.join("accounts.json"), &self.accounts).map_err(|error| format!("Couldn't save accounts: {error}"))
    }

    pub fn save_settings(&self) -> Result<(), String> {
        write(&self.root.join("settings.json"), &self.settings).map_err(|error| format!("Couldn't save settings: {error}"))
    }
}

fn read<T: DeserializeOwned + Default>(path: &Path) -> T {
    let Ok(text) = fs::read_to_string(path) else {
        return T::default();
    };
    match serde_json::from_str(text.trim_start_matches('\u{feff}')) {
        Ok(value) => value,
        Err(_) => {
            let _ = fs::rename(path, path.with_extension("json.unreadable"));
            T::default()
        }
    }
}

fn write<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, serde_json::to_string_pretty(value)?)?;
    fs::rename(temp, path)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn scratch(name: &str) -> PathBuf {
        let folder = env::temp_dir().join(format!("waccounts_test_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn reads_the_old_hub_shape() {
        let accounts: Vec<Account> = serde_json::from_str(r#"[{"Id":"1","Name":"a","Tracker":"2","SealedCookie":"S"}]"#).unwrap();
        assert_eq!((accounts[0].id.as_str(), accounts[0].sealed_cookie.as_str()), ("1", "S"));
        let settings: Settings = serde_json::from_str(r#"{"Servers":[{"Name":"n","Link":"l"}],"LaunchGapMs":100,"SwitchKey":null}"#).unwrap();
        assert_eq!((settings.servers.len(), settings.launch_gap_ms, settings.switch_key), (1, 100, None));
        let fresh: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(fresh.switch_key.map(|key| key.virtual_key), Some(0x73));
    }

    #[test]
    fn writes_the_old_hub_shape() {
        let account = Account { id: "1".into(), name: "a".into(), tracker: "2".into(), sealed_cookie: "S".into() };
        let text = serde_json::to_string(&account).unwrap();
        assert_eq!(text, r#"{"Id":"1","Name":"a","Tracker":"2","SealedCookie":"S"}"#);
    }

    #[test]
    fn save_and_load_round_trip() {
        let root = scratch("store");
        let mut store = Store::load(root.clone());
        store.accounts.push(Account { id: "7".into(), ..Account::default() });
        store.settings.fast_flags = true;
        store.save_accounts().unwrap();
        store.save_settings().unwrap();
        let again = Store::load(root.clone());
        assert_eq!((again.accounts.len(), again.settings.fast_flags), (1, true));
        assert!(!root.join("accounts.json.tmp").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unreadable_file_is_kept_aside() {
        let root = scratch("unreadable");
        fs::write(root.join("accounts.json"), "not json").unwrap();
        assert!(Store::load(root.clone()).accounts.is_empty());
        assert!(root.join("accounts.json.unreadable").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn old_data_is_copied_once_and_kept() {
        let root = scratch("copy");
        let (old, new) = (root.join("old"), root.join("new"));
        fs::create_dir_all(old.join("portraits")).unwrap();
        fs::write(old.join("accounts.json"), "[]").unwrap();
        fs::write(old.join("settings.json"), "{}").unwrap();
        fs::write(old.join("portraits").join("1.png"), "png").unwrap();
        assert!(copy_old_data(&old, &new).unwrap());
        assert!(new.join("accounts.json").exists() && new.join("settings.json").exists() && new.join("portraits").join("1.png").exists());
        assert!(old.join("accounts.json").exists());
        fs::write(new.join("accounts.json"), "[{}]").unwrap();
        assert!(!copy_old_data(&old, &new).unwrap());
        assert_eq!(fs::read_to_string(new.join("accounts.json")).unwrap(), "[{}]");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn nothing_to_copy_without_old_accounts() {
        let root = scratch("nocopy");
        assert!(!copy_old_data(&root.join("old"), &root.join("new")).unwrap());
        assert!(!root.join("new").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
