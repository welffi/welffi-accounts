#![windows_subsystem = "windows"]

mod app;
mod client;
mod data;
mod fast_flags;
mod grid;
mod keys;
mod links;
mod login;
mod macro_import;
mod portraits;
mod roblox_api;
mod seal;
mod side;
mod switcher;
mod theme;
mod view;
mod widgets;
mod win;

use eframe::egui;

fn main() -> eframe::Result {
    if !win::claim_single_instance() {
        win::focus_running_copy();
        return Ok(());
    }
    let root = data::data_root();
    let _ = data::copy_old_data(&data::old_hub_root(), &root);
    let store = data::Store::load(root);
    client::hold_lock(0);
    client::release_client_storage();
    let hotkey_ok = switcher::start(store.settings.switch_key.clone());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(win::TITLE)
            .with_inner_size([1060.0, 700.0])
            .with_min_inner_size([760.0, 520.0])
            .with_icon(theme::window_icon()),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let result = eframe::run_native(win::TITLE, options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, store, hotkey_ok)))));
    client::release_client_storage();
    win::release_single_instance();
    result
}
