# welffi accounts

welffi's Roblox account manager. Rust and egui, like welffi hub. It saves Roblox logins and launches many accounts at once. The hub downloads and opens it.

It was ported from the C# account manager in welffi hub (`src\whub`). That copy gets deleted once this one is confirmed working.

## Commands

`cargo` may not be on PATH. If so, use `%USERPROFILE%\.cargo\bin\cargo.exe`.

- Build: `cargo build`
- Checks: `cargo test`
- Release: `build.bat`, which makes `dist\welffi accounts.exe`. Close the app first, since a running exe can't be replaced.
- Publish: raise `version` in `Cargo.toml`, run `build.bat`, then make a GitHub release on welffi/welffi-accounts with a tag like `v0.1.0`. Upload `dist\welffi accounts.exe` named `welffi-accounts.exe`. The hub looks for that asset name.
- The exe icon is `assets\app.ico`, added by `build.rs` with `rc.exe` from the Windows SDK. The window icon is `assets\icon64.rgba`.

## Where code goes

- `data.rs`: accounts and settings on disk, and the one-time copy from the old hub.
- `seal.rs`: DPAPI sealing of cookies.
- `roblox_api.rs`: every Roblox web call. `links.rs`: link parsing and launch URIs, no network.
- `client.rs`: the Roblox lock, Roblox's account storage, and launching.
- `login.rs`: the browser login. `macro_import.rs`, `fast_flags.rs`, `switcher.rs` (switch key and window switching), `keys.rs`.
- `app.rs`: app state and actions. `view.rs`, `grid.rs` and `side.rs` draw it. `widgets.rs` has the shared controls and `theme.rs` every color.

Drawing code shows things and calls actions. Logic with real branches goes in a plain function with a check, so it can be tested without a window.

## Testing

welffi does the feel testing: how clicks, drags and layout feel. Don't spend time on it. For UI changes, build, run the checks, take one screenshot if the look changed, then run `build.bat` and hand it over.

Test fully before handing over anything that could lose data or break logins: saving accounts or settings, cookies, import, launching Roblox, the Roblox lock and FastFlags.

When running the app for a test:

- Set `WACCOUNTS_DATA` to a scratch folder so real accounts aren't touched.
- Starting the app still releases the real Roblox `appStorage.json` and holds the Roblox lock, whatever `WACCOUNTS_DATA` says.
- Only one copy can run at a time, so a test copy exits straight away if welffi has it open.
- Never launch Roblox, or use a saved account, without a reason tied to the current task.

## Data

- `%LOCALAPPDATA%\welffi accounts\`: `accounts.json`, `settings.json` and `portraits\`. The JSON keeps the old hub's PascalCase shape.
- On first run, if there's no `accounts.json` yet, it copies the old hub's files from `%LOCALAPPDATA%\welffi hub\` and leaves the old ones as a backup.

## Safety

- Never print, log or send saved cookies or auth tickets. When checking them, print counts or yes/no only.
- Cookies stay sealed with DPAPI on disk. Only send them to Roblox.
- Nothing in here changes the macro's files in `Downloads\AutoHotKey\welffis macros`. Import only reads them. Keep import able to read them until welffi retires the macro.

## Things that have caught us out

- Roblox needs a Content-Type on an empty POST and answers 415 without it. `roblox_api::content_type` handles this.
- The Roblox lock is a mutex, which belongs to the thread that took it. Take it on the UI thread only. A worker thread that exits leaves it abandoned.
- Roblox open before the app starts keeps the lock, and this app and the macro can't both hold it.
- The switch key is unbound while a new key is being picked, or the current key can't be pressed.

## Style for welffi

- Plain voice in docs and notes: simple sentences, no em dashes. No comments in code.
- UI ideas shouldn't come from Tandem.
- Commit or push only when asked. Commits use the noreply email, so no personal address goes public.
