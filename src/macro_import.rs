use crate::data::{Account, Server, Settings};
use crate::links;
use crate::win;
use std::fs;
use std::path::{Path, PathBuf};

type Section = Vec<(String, String)>;
type Ini = Vec<(String, Section)>;

pub fn default_folder() -> PathBuf {
    std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default().join("Downloads").join("AutoHotKey").join("welffis macros")
}

pub fn parse_ini(text: &str) -> Ini {
    let mut sections: Ini = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            sections.retain(|(existing, _)| !existing.eq_ignore_ascii_case(name));
            sections.push((name.to_string(), Vec::new()));
        } else if let (Some((_, values)), Some((key, value))) = (sections.last_mut(), line.split_once('=')) {
            let key = key.trim();
            if !key.is_empty() {
                values.retain(|(existing, _)| !existing.eq_ignore_ascii_case(key));
                values.push((key.to_string(), value.trim().to_string()));
            }
        }
    }
    sections
}

fn section<'a>(ini: &'a Ini, name: &str) -> Option<&'a Section> {
    ini.iter().find(|(existing, _)| existing.eq_ignore_ascii_case(name)).map(|(_, values)| values)
}

fn value<'a>(values: Option<&'a Section>, key: &str) -> Option<&'a str> {
    values?.iter().find(|(existing, _)| existing.eq_ignore_ascii_case(key)).map(|(_, value)| value.as_str())
}

fn digits(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

pub fn merge(accounts_ini: &str, config_ini: &str, accounts: &mut Vec<Account>, settings: &mut Settings) -> (usize, usize) {
    let config = parse_ini(config_ini);
    let mut incoming: Vec<Account> = Vec::new();
    for (id, values) in parse_ini(accounts_ini) {
        let cookie = value(Some(&values), "cookie").unwrap_or("");
        if !digits(&id) || cookie.is_empty() || accounts.iter().any(|account| account.id == id) {
            continue;
        }
        let tracker = value(Some(&values), "tracker").filter(|tracker| digits(tracker)).map_or_else(win::new_tracker, str::to_string);
        let name = value(Some(&values), "name").unwrap_or(&id).to_string();
        incoming.push(Account { name, tracker, sealed_cookie: cookie.to_string(), id });
    }

    let order: Vec<&str> = value(section(&config, "accounts"), "order").unwrap_or("").split(',').collect();
    incoming.sort_by_key(|account| order.iter().position(|id| *id == account.id).unwrap_or(usize::MAX));
    let added = incoming.len();
    accounts.extend(incoming);

    let mut servers = 0;
    for (name, link) in section(&config, "private_servers").into_iter().flatten() {
        if settings.servers.iter().any(|server| &server.name == name) || !links::is_private_link(link) {
            continue;
        }
        settings.servers.push(Server { name: name.clone(), link: link.clone() });
        servers += 1;
    }

    if settings.last_private_link.is_empty() {
        if let Some(last) = value(section(&config, "accounts"), "private_link") {
            settings.last_private_link = last.to_string();
        }
    }
    if matches!(value(section(&config, "fastflags"), "enabled"), Some("1" | "true")) {
        settings.fast_flags = true;
    }
    (added, servers)
}

pub fn read_folder(folder: &Path) -> Result<(String, String), String> {
    let accounts = fs::read_to_string(folder.join("accounts.ini")).map_err(|_| "That folder has no accounts.ini".to_string())?;
    let config = fs::read_to_string(folder.join("config.ini")).unwrap_or_default();
    Ok((accounts, config))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNTS: &str = "[2]\nname=Two\ncookie=S2\ntracker=123\n[1]\nname=One\ncookie=S1\n[x]\ncookie=S\n[3]\nname=Empty\ncookie=\n[9]\nname=Known\ncookie=S9\n";
    const CONFIG: &str = "[accounts]\norder=1,2\nprivate_link=https://www.roblox.com/share?code=last&type=Server\n[private_servers]\nGood=https://www.roblox.com/share?code=abc&type=Server\nBad=https://example.com\nKept=https://www.roblox.com/share?code=new&type=Server\n[fastflags]\nenabled=1\n";

    #[test]
    fn macro_merge_adds_new_accounts_in_order() {
        let mut accounts = vec![Account { id: "9".into(), ..Account::default() }];
        let mut settings = Settings::default();
        settings.servers.push(Server { name: "Kept".into(), link: "old".into() });
        assert_eq!(merge(ACCOUNTS, CONFIG, &mut accounts, &mut settings), (2, 1));
        let ids: Vec<&str> = accounts.iter().map(|account| account.id.as_str()).collect();
        assert_eq!(ids, ["9", "1", "2"]);
        assert_eq!((accounts[1].sealed_cookie.as_str(), accounts[2].tracker.as_str()), ("S1", "123"));
        assert_eq!(accounts[1].tracker.len(), 12);
        assert_eq!(settings.servers.iter().map(|server| server.link.as_str()).collect::<Vec<_>>(), ["old", "https://www.roblox.com/share?code=abc&type=Server"]);
        assert!(settings.last_private_link.contains("code=last") && settings.fast_flags);
    }

    #[test]
    fn existing_private_link_is_kept() {
        let mut settings = Settings { last_private_link: "mine".into(), ..Settings::default() };
        merge("", CONFIG, &mut Vec::new(), &mut settings);
        assert_eq!(settings.last_private_link, "mine");
    }

    #[test]
    fn ini_keys_ignore_case_and_spaces() {
        let ini = parse_ini("[Accounts]\r\n Order = 1,2 \r\n");
        assert_eq!(value(section(&ini, "accounts"), "order"), Some("1,2"));
    }
}
