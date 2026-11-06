#[derive(Clone, PartialEq, Debug)]
pub struct PrivateServer {
    pub place_id: String,
    pub code: String,
}

fn after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    text.find(marker).map(|index| &text[index + marker.len()..])
}

fn leading(text: &str, keep: impl Fn(char) -> bool) -> Option<&str> {
    let end = text.find(|c: char| !keep(c)).unwrap_or(text.len());
    (end > 0).then(|| &text[..end])
}

fn word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

pub fn parse_direct(link: &str) -> Option<PrivateServer> {
    let place_id = leading(after(link, "/games/")?, |c| c.is_ascii_digit())?;
    let query = &link[link.find('?')?..];
    let code = ["?privateServerLinkCode=", "&privateServerLinkCode="].iter().find_map(|marker| leading(after(query, marker)?, word))?;
    Some(PrivateServer { place_id: place_id.to_string(), code: code.to_string() })
}

pub fn share_code(link: &str) -> Option<&str> {
    let lower = link.to_ascii_lowercase();
    let start = lower.find("roblox.com/share?")? + "roblox.com/share".len();
    let query = &link[start..];
    let lower_query = &lower[start..];
    ["?code=", "&code="].iter().find_map(|marker| {
        let index = lower_query.find(marker)? + marker.len();
        leading(&query[index..], word)
    })
}

pub fn is_private_link(link: &str) -> bool {
    (link.to_ascii_lowercase().contains("roblox.com/games/") && parse_direct(link).is_some()) || share_code(link).is_some()
}

pub fn valid_username(name: &str) -> bool {
    (3..=20).contains(&name.len()) && name.chars().all(word)
}

pub fn is_portrait_host(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("").to_ascii_lowercase();
    !host.contains(['@', ':']) && host.ends_with(".rbxcdn.com")
}

pub fn launch_uri(tracker: &str, ticket: &str, place_query: &str, time_ms: u128) -> String {
    let tail = format!("+launchtime:{time_ms}+browsertrackerid:{tracker}+robloxLocale:en_us+gameLocale:en_us+channel:+LaunchExp:InApp");
    if place_query.is_empty() {
        return format!("roblox-player:1+launchmode:app+gameinfo:{ticket}{tail}");
    }
    format!("roblox-player:1+launchmode:play+gameinfo:{ticket}+placelauncherurl:https%3A%2F%2Fassetgame.roblox.com%2Fgame%2FPlaceLauncher.ashx%3F{place_query}{tail}")
}

pub fn follow_query(user_id: &str) -> String {
    format!("request%3DRequestFollowUser%26userId%3D{user_id}")
}

pub fn join_query(tracker: &str, place_id: &str) -> String {
    format!("request%3DRequestGame%26browserTrackerId%3D{tracker}%26placeId%3D{place_id}%26isPlayTogetherGame%3Dfalse")
}

pub fn private_query(tracker: &str, server: &PrivateServer) -> String {
    format!("request%3DRequestPrivateGame%26browserTrackerId%3D{tracker}%26placeId%3D{}%26linkCode%3D{}", server.place_id, server.code)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRECT: &str = "https://www.roblox.com/games/123/Some-Game?privateServerLinkCode=abc123";
    const SHARE: &str = "https://www.roblox.com/share?code=abc123&type=Server";

    #[test]
    fn private_links_are_recognised() {
        assert!(is_private_link(DIRECT));
        assert!(is_private_link(SHARE));
        assert!(is_private_link("https://www.roblox.com/share?type=Server&code=x1"));
        assert!(!is_private_link("https://www.roblox.com/games/123/Some-Game"));
        assert!(!is_private_link("https://example.com/share?code=abc"));
        assert!(!is_private_link("https://www.roblox.com/share?linkcode=abc"));
    }

    #[test]
    fn direct_link_parses() {
        assert_eq!(parse_direct(DIRECT), Some(PrivateServer { place_id: "123".into(), code: "abc123".into() }));
        assert_eq!(parse_direct(SHARE), None);
        assert_eq!(share_code(SHARE), Some("abc123"));
    }

    #[test]
    fn usernames() {
        assert!(valid_username("welffi_1"));
        assert!(!valid_username("ab"));
        assert!(!valid_username("has space"));
        assert!(!valid_username("abcdefghijklmnopqrstu"));
    }

    #[test]
    fn portrait_hosts() {
        assert!(is_portrait_host("https://tr.rbxcdn.com/x.png"));
        assert!(!is_portrait_host("http://tr.rbxcdn.com/x.png"));
        assert!(!is_portrait_host("https://rbxcdn.com.evil.test/x.png"));
        assert!(!is_portrait_host("https://tr.rbxcdn.com@evil.test/x.png"));
    }

    #[test]
    fn launch_uris_match_the_old_hub() {
        let app = launch_uri("123456", "TICKET", "", 5);
        assert_eq!(app, "roblox-player:1+launchmode:app+gameinfo:TICKET+launchtime:5+browsertrackerid:123456+robloxLocale:en_us+gameLocale:en_us+channel:+LaunchExp:InApp");
        let play = launch_uri("123456", "TICKET", &join_query("123456", "90"), 5);
        assert!(play.starts_with("roblox-player:1+launchmode:play+gameinfo:TICKET+placelauncherurl:"));
        assert!(play.contains("PlaceLauncher.ashx%3Frequest%3DRequestGame%26browserTrackerId%3D123456%26placeId%3D90%26isPlayTogetherGame%3Dfalse+launchtime:5"));
        let server = PrivateServer { place_id: "90".into(), code: "abc".into() };
        assert_eq!(private_query("123456", &server), "request%3DRequestPrivateGame%26browserTrackerId%3D123456%26placeId%3D90%26linkCode%3Dabc");
        assert_eq!(follow_query("7"), "request%3DRequestFollowUser%26userId%3D7");
    }
}
