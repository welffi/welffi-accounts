use crate::client::{self, Launch};
use crate::data::{Account, Server, Store};
use crate::fast_flags;
use crate::keys::{self, Capture};
use crate::links;
use crate::login::{self, Login};
use crate::macro_import;
use crate::portraits::Portraits;
use crate::roblox_api;
use crate::switcher;
use crate::win;
use eframe::egui;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

pub enum Event {
    Note(String, String),
    Status(String, Option<bool>),
    Added(Result<Login, String>),
    PortraitsReady,
    Finished,
}

pub enum Confirm {
    RemoveAccount(String),
    DeleteServer(String),
}

#[derive(Clone)]
pub struct Outbox {
    sender: Sender<Event>,
    ctx: egui::Context,
}

impl Outbox {
    pub fn send(&self, event: Event) {
        let _ = self.sender.send(event);
        self.ctx.request_repaint();
    }
}

pub struct App {
    pub store: Store,
    pub portraits: Portraits,
    pub ticked: HashSet<String>,
    pub notes: HashMap<String, String>,
    pub status: (String, Option<bool>),
    pub busy: bool,
    pub follow: String,
    pub link: String,
    pub server_name: String,
    pub capturing: bool,
    pub confirm: Option<Confirm>,
    pub dragging: Option<String>,
    outbox: Outbox,
    receiver: Receiver<Event>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, store: Store, hotkey_ok: bool) -> App {
        crate::theme::apply(&cc.egui_ctx);
        let (sender, receiver) = channel();
        let ticked = store.accounts.first().map(|account| account.id.clone()).into_iter().collect();
        let status = match (&store.settings.switch_key, hotkey_ok) {
            (Some(key), false) => (format!("{} is already used by another program", key.label), Some(false)),
            _ => ("Ready".to_string(), None),
        };
        let app = App {
            portraits: Portraits::new(store.portraits()),
            link: store.settings.last_private_link.clone(),
            store,
            ticked,
            notes: HashMap::new(),
            status,
            busy: false,
            follow: String::new(),
            server_name: String::new(),
            capturing: false,
            confirm: None,
            dragging: None,
            outbox: Outbox { sender, ctx: cc.egui_ctx.clone() },
            receiver,
        };
        app.refresh_portraits();
        app
    }

    pub fn set_status(&mut self, message: impl Into<String>, ok: Option<bool>) {
        self.status = (message.into(), ok);
    }

    fn report(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.set_status(error, Some(false));
        }
    }

    pub fn note(&self, id: &str) -> &str {
        self.notes.get(id).map_or("Ready", String::as_str)
    }

    pub fn poll(&mut self) {
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                Event::Note(id, note) => {
                    self.notes.insert(id, note);
                }
                Event::Status(message, ok) => self.set_status(message, ok),
                Event::Added(Ok(login)) => self.added(login),
                Event::Added(Err(error)) => self.set_status(error, Some(false)),
                Event::PortraitsReady => self.portraits.reload(),
                Event::Finished => self.busy = false,
            }
        }
    }

    fn refresh_portraits(&self) {
        let ids: Vec<String> = self.store.accounts.iter().map(|account| account.id.clone()).collect();
        let (folder, outbox) = (self.store.portraits(), self.outbox.clone());
        thread::spawn(move || {
            if roblox_api::fetch_portraits(&ids, &folder).is_ok() {
                outbox.send(Event::PortraitsReady);
            }
        });
    }

    fn run_for_ticked(&mut self, launch: Launch) {
        if self.busy {
            return;
        }
        let accounts: Vec<Account> = self.store.accounts.iter().filter(|account| self.ticked.contains(&account.id)).cloned().collect();
        if accounts.is_empty() {
            return self.set_status("Tick an account first", Some(false));
        }
        if let Err(error) = client::ensure_lock() {
            return self.set_status(error, Some(false));
        }
        if let Err(error) = fast_flags::apply(self.store.settings.fast_flags) {
            return self.set_status(format!("Couldn't change FastFlags: {error}"), Some(false));
        }
        self.busy = true;
        let (gap, outbox) = (self.store.settings.launch_gap_ms, self.outbox.clone());
        thread::spawn(move || {
            let (mut opened, mut last_failure) = (0, String::new());
            for (index, account) in accounts.iter().enumerate() {
                if index > 0 {
                    thread::sleep(Duration::from_millis(gap));
                }
                outbox.send(Event::Note(account.id.clone(), "Launching...".to_string()));
                outbox.send(Event::Status(format!("Launching {}...", account.name), None));
                let note = match client::launch(account, &launch) {
                    Ok(message) => {
                        opened += 1;
                        message
                    }
                    Err(message) => {
                        last_failure = message.clone();
                        message
                    }
                };
                outbox.send(Event::Note(account.id.clone(), note));
            }
            let summary = format!("Launched {opened} of {} accounts", accounts.len());
            let message = if last_failure.is_empty() { summary } else { format!("{summary}. {last_failure}") };
            outbox.send(Event::Status(message, Some(opened == accounts.len())));
            outbox.send(Event::Finished);
        });
    }

    pub fn open(&mut self) {
        let follow = self.follow.trim().to_string();
        self.run_for_ticked(Launch::Open { follow });
    }

    pub fn join_recent(&mut self) {
        self.run_for_ticked(Launch::JoinRecent);
    }

    pub fn join_link(&mut self) {
        let link = self.link.trim().to_string();
        if link.is_empty() {
            return self.set_status("Paste a private server link first", Some(false));
        }
        self.store.settings.last_private_link = link.clone();
        let saved = self.store.save_settings();
        self.report(saved);
        self.run_for_ticked(Launch::JoinLink(link));
    }

    pub fn join_server(&mut self, link: String) {
        self.run_for_ticked(Launch::JoinLink(link));
    }

    pub fn save_server(&mut self) {
        let (name, link) = (self.server_name.trim().to_string(), self.link.trim().to_string());
        if !links::is_private_link(&link) {
            return self.set_status("That isn't a private server link", Some(false));
        }
        if name.is_empty() {
            return self.set_status("Give the server a name first", Some(false));
        }
        let server = Server { name: name.clone(), link };
        match self.store.settings.servers.iter_mut().find(|existing| existing.name == name) {
            Some(existing) => *existing = server,
            None => self.store.settings.servers.push(server),
        }
        self.server_name.clear();
        match self.store.save_settings() {
            Ok(()) => self.set_status(format!("Saved {name}"), Some(true)),
            Err(error) => self.set_status(error, Some(false)),
        }
    }

    pub fn delete_server(&mut self, name: &str) {
        self.store.settings.servers.retain(|server| server.name != name);
        match self.store.save_settings() {
            Ok(()) => self.set_status(format!("Deleted {name}"), None),
            Err(error) => self.set_status(error, Some(false)),
        }
    }

    pub fn add_account(&mut self) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.set_status("Log in using the browser window. It closes by itself.", None);
        let outbox = self.outbox.clone();
        thread::spawn(move || {
            outbox.send(Event::Added(login::add_account()));
            outbox.send(Event::Finished);
        });
    }

    fn added(&mut self, login: Login) {
        match self.store.accounts.iter_mut().find(|account| account.id == login.id) {
            Some(account) => {
                account.name = login.name.clone();
                account.sealed_cookie = login.sealed;
            }
            None => self.store.accounts.push(Account { id: login.id, name: login.name.clone(), tracker: win::new_tracker(), sealed_cookie: login.sealed }),
        }
        match self.store.save_accounts() {
            Ok(()) => self.set_status(format!("Added {}", login.name), Some(true)),
            Err(error) => self.set_status(error, Some(false)),
        }
        self.refresh_portraits();
    }

    pub fn remove_account(&mut self, id: &str) {
        let Some(index) = self.store.accounts.iter().position(|account| account.id == id) else {
            return;
        };
        let account = self.store.accounts.remove(index);
        self.ticked.remove(id);
        self.notes.remove(id);
        self.portraits.delete(id);
        match self.store.save_accounts() {
            Ok(()) => self.set_status(format!("Removed {}", account.name), None),
            Err(error) => self.set_status(error, Some(false)),
        }
    }

    pub fn save_order(&mut self) {
        let saved = self.store.save_accounts();
        self.report(saved);
    }

    pub fn import(&mut self) {
        let mut folder = macro_import::default_folder();
        if !folder.join("accounts.ini").exists() {
            match rfd::FileDialog::new().set_title("Select the welffi's macros folder").pick_folder() {
                Some(picked) => folder = picked,
                None => return,
            }
        }
        let (accounts_ini, config_ini) = match macro_import::read_folder(&folder) {
            Ok(files) => files,
            Err(error) => return self.set_status(error, Some(false)),
        };
        let (accounts, servers) = macro_import::merge(&accounts_ini, &config_ini, &mut self.store.accounts, &mut self.store.settings);
        if self.ticked.is_empty() {
            self.ticked.extend(self.store.accounts.first().map(|account| account.id.clone()));
        }
        self.link = self.store.settings.last_private_link.clone();
        match self.store.save_accounts().and_then(|()| self.store.save_settings()) {
            Ok(()) => self.set_status(format!("Imported {accounts} accounts and {servers} servers"), Some(true)),
            Err(error) => self.set_status(error, Some(false)),
        }
        self.refresh_portraits();
    }

    pub fn toggle_all(&mut self) {
        if self.store.accounts.iter().any(|account| !self.ticked.contains(&account.id)) {
            self.ticked = self.store.accounts.iter().map(|account| account.id.clone()).collect();
        } else {
            self.ticked.clear();
        }
    }

    pub fn toggle_fast_flags(&mut self) {
        let on = !self.store.settings.fast_flags;
        if let Err(error) = fast_flags::apply(on) {
            return self.set_status(format!("Couldn't change FastFlags: {error}"), Some(false));
        }
        self.store.settings.fast_flags = on;
        match self.store.save_settings() {
            Ok(()) if on => self.set_status("FastFlags on. Restart Roblox to apply.", Some(true)),
            Ok(()) => self.set_status("FastFlags off. Defaults restored.", Some(true)),
            Err(error) => self.set_status(error, Some(false)),
        }
    }

    pub fn start_capture(&mut self, ctx: &egui::Context) {
        switcher::bind(None);
        self.capturing = true;
        ctx.memory_mut(|memory| memory.stop_text_input());
        self.set_status("Press the new switch key. Esc cancels, Backspace clears.", None);
    }

    pub fn capture_keys(&mut self, ctx: &egui::Context) {
        if !self.capturing {
            return;
        }
        let pressed = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Key { key, pressed: true, modifiers, .. } => keys::capture(*key, *modifiers),
                _ => None,
            })
        });
        let Some(capture) = pressed else {
            return;
        };
        self.capturing = false;
        let current = self.store.settings.switch_key.clone();
        match capture {
            Capture::Cancel => {
                switcher::bind(current);
                self.set_status("Ready", None);
            }
            Capture::Refused(message) => {
                switcher::bind(current);
                self.set_status(message, Some(false));
            }
            Capture::Clear => self.apply_switch_key(None),
            Capture::Set(key) => self.apply_switch_key(Some(key)),
        }
    }

    fn apply_switch_key(&mut self, key: Option<crate::data::SwitchKey>) {
        if !switcher::bind(key.clone()) {
            switcher::bind(self.store.settings.switch_key.clone());
            let label = key.map(|key| key.label).unwrap_or_default();
            return self.set_status(format!("{label} is already used by another program"), Some(false));
        }
        let message = key.as_ref().map_or("Switch key cleared".to_string(), |key| format!("Switch key set to {}", key.label));
        self.store.settings.switch_key = key;
        match self.store.save_settings() {
            Ok(()) => self.set_status(message, Some(true)),
            Err(error) => self.set_status(error, Some(false)),
        }
    }
}
