use crate::links::{self, PrivateServer};
use crate::win;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

const SESSION_EXPIRED: &str = "Session expired. Remove the account and add it again.";
const REFERER: &str = "https://www.roblox.com/";

#[derive(Default)]
struct Call<'a> {
    post: bool,
    url: &'a str,
    cookie: Option<&'a str>,
    body: Option<String>,
    csrf: Option<String>,
    referer: bool,
}

struct Reply {
    status: u16,
    csrf: Option<String>,
    ticket: Option<String>,
    body: String,
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
            .build()
            .into()
    })
}

pub fn content_type(body: Option<&str>) -> &'static str {
    if body.is_some() { "application/json; charset=utf-8" } else { "text/plain; charset=utf-8" }
}

fn headers<B>(mut request: ureq::RequestBuilder<B>, call: &Call) -> ureq::RequestBuilder<B> {
    if let Some(cookie) = call.cookie {
        request = request.header("Cookie", format!(".ROBLOSECURITY={cookie}"));
    }
    if let Some(csrf) = &call.csrf {
        request = request.header("X-CSRF-TOKEN", csrf);
    }
    if call.referer {
        request = request.header("Referer", REFERER);
    }
    request
}

fn send(call: &Call) -> Result<Reply, String> {
    let response = if call.post {
        let body = call.body.as_deref();
        headers(agent().post(call.url), call).content_type(content_type(body)).send(body.unwrap_or(""))
    } else {
        headers(agent().get(call.url), call).call()
    };
    let mut response = response.map_err(|error| format!("Couldn't reach Roblox ({error})"))?;
    let header = |name: &str| response.headers().get(name).and_then(|value| value.to_str().ok()).map(str::to_string);
    let (csrf, ticket) = (header("x-csrf-token"), header("rbx-authentication-ticket"));
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().unwrap_or_default();
    Ok(Reply { status, csrf, ticket, body })
}

fn post_with_csrf(url: &str, cookie: &str, body: Value) -> Result<Reply, String> {
    let mut call = Call { post: true, url, cookie: Some(cookie), body: Some(body.to_string()), referer: true, ..Call::default() };
    let first = send(&call)?;
    if first.status != 403 {
        return Ok(first);
    }
    call.csrf = first.csrf;
    send(&call)
}

fn find<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map.get(key).or_else(|| map.values().find_map(|inner| find(inner, key))),
        Value::Array(items) => items.iter().find_map(|inner| find(inner, key)),
        _ => None,
    }
}

fn number_text(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) if !text.is_empty() && text.chars().all(|c| c.is_ascii_digit()) => Some(text.clone()),
        _ => None,
    }
}

fn parse(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or(Value::Null)
}

pub fn who_am_i(cookie: &str) -> Result<(String, String), String> {
    let reply = send(&Call { url: "https://users.roblox.com/v1/users/authenticated", cookie: Some(cookie), ..Call::default() })?;
    if reply.status != 200 {
        return Err(format!("Roblox rejected the login ({})", reply.status));
    }
    let value = parse(&reply.body);
    let id = value.get("id").and_then(number_text);
    let name = value.get("name").and_then(Value::as_str);
    match (id, name) {
        (Some(id), Some(name)) => Ok((id, name.to_string())),
        _ => Err("Unexpected reply from Roblox".to_string()),
    }
}

pub fn auth_ticket(cookie: &str) -> Result<String, String> {
    let url = "https://auth.roblox.com/v1/authentication-ticket/";
    let mut call = Call { post: true, url, cookie: Some(cookie), referer: true, ..Call::default() };
    let first = send(&call)?;
    if first.status == 401 {
        return Err(SESSION_EXPIRED.to_string());
    }
    call.csrf = Some(first.csrf.filter(|token| !token.is_empty()).ok_or(format!("No CSRF token from Roblox ({})", first.status))?);
    let second = send(&call)?;
    second.ticket.filter(|ticket| !ticket.is_empty()).ok_or(format!("No auth ticket from Roblox ({})", second.status))
}

pub fn user_id_of(username: &str) -> Result<String, String> {
    if !links::valid_username(username) {
        return Err("That isn't a valid Roblox username".to_string());
    }
    let body = json!({ "usernames": [username], "excludeBannedUsers": true }).to_string();
    let reply = send(&Call { post: true, url: "https://users.roblox.com/v1/usernames/users", body: Some(body), ..Call::default() })?;
    if reply.status != 200 {
        return Err(format!("Username lookup failed ({})", reply.status));
    }
    find(&parse(&reply.body), "id").and_then(number_text).ok_or(format!("No Roblox user named {username}"))
}

pub fn find_recent_game(cookie: &str) -> Result<(String, String), String> {
    let body = json!({ "pageType": "Home", "sessionId": win::new_session_id() });
    let reply = post_with_csrf("https://apis.roblox.com/discovery-api/omni-recommendation", cookie, body)?;
    match reply.status {
        200 => parse_recent_game(&reply.body).ok_or("Roblox didn't list a recent game for this account".to_string()),
        401 => Err(SESSION_EXPIRED.to_string()),
        status => Err(format!("Recent games lookup failed ({status})")),
    }
}

fn is_continue_sort(sort: &Value) -> bool {
    if sort.get("topicId").and_then(number_text).as_deref() == Some("100000003") {
        return true;
    }
    sort.get("topic").and_then(Value::as_str).is_some_and(|topic| topic.to_ascii_lowercase().starts_with("continue"))
}

pub fn parse_recent_game(json: &str) -> Option<(String, String)> {
    let root = parse(json);
    let games = root.get("contentMetadata")?.get("Game")?;
    root.get("sorts")?.as_array()?.iter().filter(|sort| is_continue_sort(sort)).find_map(|sort| {
        sort.get("recommendationList")?.as_array()?.iter().filter(|item| item.get("contentType").and_then(Value::as_str) == Some("Game")).find_map(|item| {
            let game = games.get(number_text(item.get("contentId")?)?)?;
            let place = game.get("rootPlaceId").filter(|place| place.is_number())?;
            Some((place.to_string(), game.get("name").and_then(Value::as_str).unwrap_or("the game").to_string()))
        })
    })
}

pub fn resolve_private_server(cookie: &str, link: &str) -> Result<PrivateServer, String> {
    if let Some(direct) = links::parse_direct(link) {
        return Ok(direct);
    }
    let code = links::share_code(link).ok_or("That isn't a private server link")?;
    let reply = post_with_csrf("https://apis.roblox.com/sharelinks/v1/resolve-link", cookie, json!({ "linkId": code, "linkType": "Server" }))?;
    match reply.status {
        200 => {}
        401 => return Err(SESSION_EXPIRED.to_string()),
        status => return Err(format!("Private server link lookup failed ({status})")),
    }
    let value = parse(&reply.body);
    let place_id = find(&value, "placeId").and_then(number_text);
    let code = find(&value, "linkCode").and_then(Value::as_str).filter(|code| !code.is_empty());
    match (place_id, code) {
        (Some(place_id), Some(code)) => Ok(PrivateServer { place_id, code: code.to_string() }),
        _ => Err("That private server link is invalid or expired".to_string()),
    }
}

pub fn fetch_portraits(ids: &[String], folder: &Path) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let url = format!("https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={}&size=150x150&format=Png&isCircular=false", ids.join(","));
    let reply = send(&Call { url: &url, ..Call::default() })?;
    if reply.status != 200 {
        return Ok(());
    }
    for item in parse(&reply.body).get("data").and_then(Value::as_array).into_iter().flatten() {
        let ready = item.get("state").and_then(Value::as_str) == Some("Completed");
        let image = item.get("imageUrl").and_then(Value::as_str).filter(|url| links::is_portrait_host(url));
        let (Some(image), Some(id), true) = (image, item.get("targetId").and_then(number_text), ready) else {
            continue;
        };
        let mut response = agent().get(image).call().map_err(|error| error.to_string())?;
        if response.status() != 200 {
            continue;
        }
        let bytes = response.body_mut().read_to_vec().map_err(|error| error.to_string())?;
        let target = folder.join(format!("{id}.png"));
        let temp = folder.join(format!("{id}.png.tmp"));
        fs::write(&temp, bytes).and_then(|()| fs::rename(&temp, &target)).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_game_comes_from_the_continue_sort() {
        let json = r#"{"sorts":[
            {"topicId":1,"recommendationList":[{"contentType":"Game","contentId":7}]},
            {"topicId":100000003,"recommendationList":[{"contentType":"Ad","contentId":8},{"contentType":"Game","contentId":9}]}
        ],"contentMetadata":{"Game":{"7":{"rootPlaceId":70,"name":"Wrong"},"9":{"rootPlaceId":90,"name":"Right"}}}}"#;
        assert_eq!(parse_recent_game(json), Some(("90".into(), "Right".into())));
    }

    #[test]
    fn recent_game_falls_back_to_the_topic_name() {
        let json = r#"{"sorts":[{"topicId":5,"topic":"Continue Playing","recommendationList":[{"contentType":"Game","contentId":1}]}],
            "contentMetadata":{"Game":{"1":{"rootPlaceId":11,"name":"A"}}}}"#;
        assert_eq!(parse_recent_game(json).map(|game| game.0), Some("11".into()));
        assert_eq!(parse_recent_game(r#"{"sorts":[],"contentMetadata":{"Game":{}}}"#), None);
    }

    #[test]
    fn empty_post_still_has_a_content_type() {
        assert!(content_type(None).starts_with("text/plain"));
        assert!(content_type(Some("{}")).starts_with("application/json"));
    }

    #[test]
    fn nested_fields_are_found() {
        let value = parse(r#"{"privateServerInviteData":{"placeId":123,"linkCode":"abc"}}"#);
        assert_eq!(find(&value, "placeId").and_then(number_text), Some("123".into()));
        assert_eq!(find(&value, "linkCode").and_then(Value::as_str), Some("abc"));
    }
}
