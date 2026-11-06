use crate::app::{App, Confirm};
use crate::theme::{self, LINE};
use crate::widgets::{self, Kind};
use eframe::egui::{self, Align, FontId, Layout, Sense, Vec2};

impl App {
    pub fn side(&mut self, ui: &mut egui::Ui) {
        let enabled = !self.busy;
        widgets::card(ui, |ui| {
            widgets::title(ui, "Launch");
            widgets::hint(ui, "Runs for every ticked account.");
            ui.add_space(12.0);
            widgets::text_box(ui, &mut self.follow, "Player to follow (optional)", 20, ui.available_width());
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let half = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;
                if widgets::button(ui, "Open / follow", Kind::Accent, enabled, half).clicked() {
                    self.open();
                }
                if widgets::button(ui, "Join most recent", Kind::Plain, enabled, half).clicked() {
                    self.join_recent();
                }
            });
        });
        ui.add_space(14.0);
        widgets::card(ui, |ui| {
            widgets::title(ui, "Private servers");
            ui.add_space(12.0);
            widgets::text_box(ui, &mut self.link, "Paste a private server link", usize::MAX, ui.available_width());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let name_width = ui.available_width() - 60.0 - 66.0 - ui.spacing().item_spacing.x * 2.0;
                widgets::text_box(ui, &mut self.server_name, "Name to save it as", 40, name_width);
                if widgets::button(ui, "Save", Kind::Plain, true, 60.0).clicked() {
                    self.save_server();
                }
                if widgets::button(ui, "Join", Kind::Accent, enabled, 66.0).clicked() {
                    self.join_link();
                }
            });
            self.server_list(ui, enabled);
        });
        ui.add_space(14.0);
        widgets::card(ui, |ui| {
            widgets::title(ui, "Windows");
            ui.add_space(12.0);
            widgets::setting_row(ui, "Switch key", "Jumps to the next Roblox window", |ui| {
                let (label, kind) = match (&self.store.settings.switch_key, self.capturing) {
                    (_, true) => ("Press a key".to_string(), Kind::Accent),
                    (Some(key), false) => (key.label.clone(), Kind::Plain),
                    (None, false) => ("Not set".to_string(), Kind::Plain),
                };
                if widgets::button(ui, &label, kind, !self.capturing, 92.0).clicked() {
                    self.start_capture(ui.ctx());
                }
            });
            ui.add_space(12.0);
            widgets::setting_row(ui, "Performance FastFlags", "Used on the next Roblox launch", |ui| {
                if widgets::switch(ui, self.store.settings.fast_flags).clicked() {
                    self.toggle_fast_flags();
                }
            });
        });
    }

    fn server_list(&mut self, ui: &mut egui::Ui, enabled: bool) {
        if self.store.settings.servers.is_empty() {
            return;
        }
        ui.add_space(12.0);
        let mut join = None;
        for server in &self.store.settings.servers {
            let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().hline(line.x_range(), line.center().y, egui::Stroke::new(1.0, LINE));
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, "\u{E74D}", "Delete", true).clicked() {
                        self.confirm = Some(Confirm::DeleteServer(server.name.clone()));
                    }
                    if widgets::icon_button(ui, "\u{E768}", "Join", enabled).clicked() {
                        join = Some(server.link.clone());
                    }
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
                    theme::truncated_text(ui.painter(), rect, Align::Min, &server.name, FontId::proportional(14.0), theme::TEXT);
                });
            });
        }
        if let Some(link) = join {
            self.join_server(link);
        }
    }
}
