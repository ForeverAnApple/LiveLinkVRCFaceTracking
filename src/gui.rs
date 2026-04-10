use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use eframe::egui;

use crate::state::{TrackingState, CONNECTED, SHUTDOWN};

// Dark theme colors
const BG: egui::Color32 = egui::Color32::from_rgb(24, 24, 28);
const SURFACE: egui::Color32 = egui::Color32::from_rgb(34, 34, 40);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(230, 230, 235);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(140, 140, 150);
const GREEN: egui::Color32 = egui::Color32::from_rgb(60, 200, 120);
const RED: egui::Color32 = egui::Color32::from_rgb(220, 60, 70);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(100, 120, 220);

/// Config passed from CLI into the GUI for display.
pub struct GuiConfig {
    pub listen_port: u16,
    pub osc_target: String,
    pub prefix: String,
    pub send_rate: u32,
}

struct DisplayState {
    connected: bool,
    device_id: String,
    subject_name: String,
    frame_number: u32,
    packets_received: u64,
    last_packet_age_ms: Option<u64>,
}

pub struct BridgeApp {
    state: Arc<RwLock<TrackingState>>,
    config: GuiConfig,
    enabled: bool,
    show_config: bool,
}

impl BridgeApp {
    pub fn new(state: Arc<RwLock<TrackingState>>, config: GuiConfig) -> Self {
        Self {
            state,
            config,
            enabled: true,
            show_config: false,
        }
    }

    fn snapshot(&self) -> DisplayState {
        let connected = CONNECTED.load(Ordering::Relaxed);
        let st = self.state.read().unwrap_or_else(|p| p.into_inner());
        DisplayState {
            connected,
            device_id: st.device_id.clone(),
            subject_name: st.subject_name.clone(),
            frame_number: st.frame_number,
            packets_received: st.packets_received,
            last_packet_age_ms: st.last_packet_time.map(|t| t.elapsed().as_millis() as u64),
        }
    }

    fn apply_theme(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.visuals.dark_mode = true;
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = SURFACE;
        style.visuals.override_text_color = Some(TEXT_PRIMARY);
        style.visuals.widgets.noninteractive.bg_fill = SURFACE;
        style.visuals.widgets.inactive.bg_fill = SURFACE;
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(44, 44, 52);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(54, 54, 64);
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.visuals.window_corner_radius = egui::CornerRadius::same(8);
        style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);
        ctx.set_style(style);
    }
}

impl eframe::App for BridgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if SHUTDOWN.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        ctx.request_repaint_after(Duration::from_millis(250));
        Self::apply_theme(ctx);
        let snap = self.snapshot();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(4.0);

            // --- Power button ---
            ui.vertical_centered(|ui| {
                let (btn_color, btn_hover) = if self.enabled {
                    (GREEN, egui::Color32::from_rgb(80, 220, 140))
                } else {
                    (RED, egui::Color32::from_rgb(240, 80, 90))
                };

                let btn_size = egui::vec2(64.0, 64.0);
                let (rect, response) = ui.allocate_exact_size(btn_size, egui::Sense::click());

                if response.clicked() {
                    self.enabled = !self.enabled;
                    // TODO: actually pause/resume the sender thread
                }

                let color = if response.hovered() { btn_hover } else { btn_color };
                let painter = ui.painter();
                painter.circle_filled(rect.center(), 30.0, color);

                // Power icon (circle with line)
                let center = rect.center();
                painter.circle_stroke(
                    center,
                    18.0,
                    egui::Stroke::new(2.5, BG),
                );
                painter.line_segment(
                    [
                        center + egui::vec2(0.0, -22.0),
                        center + egui::vec2(0.0, -10.0),
                    ],
                    egui::Stroke::new(3.0, BG),
                );

                let label = if self.enabled { "ON" } else { "OFF" };
                ui.add_space(4.0);
                ui.label(egui::RichText::new(label).size(11.0).color(TEXT_DIM));
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(2.0);

            // --- Connection status ---
            ui.horizontal(|ui| {
                let (dot_color, status_text) = if snap.connected {
                    (GREEN, "LiveLink connected")
                } else {
                    (RED, "LiveLink waiting...")
                };

                let dot_radius = 5.0;
                let (dot_rect, _) = ui.allocate_exact_size(
                    egui::vec2(dot_radius * 2.0, dot_radius * 2.0),
                    egui::Sense::hover(),
                );
                ui.painter().circle_filled(dot_rect.center(), dot_radius, dot_color);
                ui.label(egui::RichText::new(status_text).size(13.0));
            });

            if snap.connected {
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new(format!("{} ({})", snap.subject_name, snap.device_id))
                            .size(11.0)
                            .color(TEXT_DIM),
                    );
                });
            }

            ui.add_space(2.0);

            // VRChat OSC status
            ui.horizontal(|ui| {
                let dot_radius = 5.0;
                let (dot_rect, _) = ui.allocate_exact_size(
                    egui::vec2(dot_radius * 2.0, dot_radius * 2.0),
                    egui::Sense::hover(),
                );
                // OSC is always "sending" when enabled
                let osc_color = if self.enabled { GREEN } else { TEXT_DIM };
                ui.painter().circle_filled(dot_rect.center(), dot_radius, osc_color);
                ui.label(egui::RichText::new(format!("OSC → {}", self.config.osc_target)).size(13.0));
            });

            ui.horizontal(|ui| {
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new(format!("listening on :{}", self.config.listen_port))
                        .size(11.0)
                        .color(TEXT_DIM),
                );
            });

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(2.0);

            // --- Stats ---
            if snap.connected {
                ui.horizontal(|ui| {
                    stat_label(ui, "packets", &format!("{}", snap.packets_received));
                    ui.add_space(12.0);
                    stat_label(ui, "frame", &format!("{}", snap.frame_number));
                    if let Some(age) = snap.last_packet_age_ms {
                        ui.add_space(12.0);
                        stat_label(ui, "latency", &format!("{}ms", age));
                    }
                });
            } else {
                ui.label(
                    egui::RichText::new("no data yet")
                        .size(11.0)
                        .color(TEXT_DIM),
                );
            }

            ui.add_space(4.0);

            // --- Config toggle ---
            let config_label = if self.show_config { "hide config" } else { "config" };
            if ui
                .add(egui::Button::new(
                    egui::RichText::new(config_label).size(11.0).color(ACCENT),
                ).frame(false))
                .clicked()
            {
                self.show_config = !self.show_config;
            }

            if self.show_config {
                ui.add_space(2.0);
                egui::Frame::new()
                    .fill(SURFACE)
                    .corner_radius(6.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        egui::Grid::new("config_grid")
                            .num_columns(2)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("listen port").size(11.0).color(TEXT_DIM));
                                ui.label(egui::RichText::new(format!("{}", self.config.listen_port)).size(11.0));
                                ui.end_row();

                                ui.label(egui::RichText::new("osc target").size(11.0).color(TEXT_DIM));
                                ui.label(egui::RichText::new(&self.config.osc_target).size(11.0));
                                ui.end_row();

                                ui.label(egui::RichText::new("prefix").size(11.0).color(TEXT_DIM));
                                ui.label(egui::RichText::new(&self.config.prefix).size(11.0));
                                ui.end_row();

                                ui.label(egui::RichText::new("send rate").size(11.0).color(TEXT_DIM));
                                ui.label(egui::RichText::new(format!("{}Hz", self.config.send_rate)).size(11.0));
                                ui.end_row();
                            });
                    });
            }
        });
    }
}

fn stat_label(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(egui::RichText::new(value).size(14.0).strong());
        ui.label(egui::RichText::new(label).size(10.0).color(TEXT_DIM));
    });
}

pub fn run(state: Arc<RwLock<TrackingState>>, config: GuiConfig) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("livelink-vrcft")
            .with_inner_size([300.0, 340.0])
            .with_min_inner_size([280.0, 280.0]),
        ..Default::default()
    };
    eframe::run_native(
        "livelink-vrcft",
        options,
        Box::new(move |_cc| Ok(Box::new(BridgeApp::new(state, config)))),
    )
}
