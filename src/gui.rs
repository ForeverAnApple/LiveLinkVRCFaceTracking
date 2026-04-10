use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use eframe::egui;

use crate::state::{TrackingState, BUNDLES_SENT, CONNECTED, SHUTDOWN};

// Dark theme colors
const BG: egui::Color32 = egui::Color32::from_rgb(24, 24, 28);
const SURFACE: egui::Color32 = egui::Color32::from_rgb(34, 34, 40);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(230, 230, 235);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(140, 140, 150);
const GREEN: egui::Color32 = egui::Color32::from_rgb(60, 200, 120);
const RED: egui::Color32 = egui::Color32::from_rgb(220, 60, 70);

// Gear icon Unicode
const GEAR: &str = "\u{2699}";

/// Config passed from CLI into the GUI for display.
pub struct GuiConfig {
    pub listen_port: u16,
    pub osc_target: String,
    pub prefix: String,
    pub send_rate: u32,
}

/// Which view is currently shown.
#[derive(PartialEq)]
enum View {
    Home,
    Config,
}

/// Rate tracker: computes packets/sec from periodic samples.
struct RateTracker {
    last_count: u64,
    last_sample: Instant,
    rate: f32,
}

impl RateTracker {
    fn new() -> Self {
        Self {
            last_count: 0,
            last_sample: Instant::now(),
            rate: 0.0,
        }
    }

    fn update(&mut self, current_count: u64) {
        let elapsed = self.last_sample.elapsed().as_secs_f32();
        if elapsed >= 1.0 {
            let delta = current_count.saturating_sub(self.last_count);
            self.rate = delta as f32 / elapsed;
            self.last_count = current_count;
            self.last_sample = Instant::now();
        }
    }
}

pub struct BridgeApp {
    state: Arc<RwLock<TrackingState>>,
    config: GuiConfig,
    enabled: bool,
    view: View,
    recv_rate: RateTracker,
    send_rate: RateTracker,
}

impl BridgeApp {
    pub fn new(state: Arc<RwLock<TrackingState>>, config: GuiConfig) -> Self {
        Self {
            state,
            config,
            enabled: true,
            view: View::Home,
            recv_rate: RateTracker::new(),
            send_rate: RateTracker::new(),
        }
    }

    fn snapshot(&mut self) -> Snapshot {
        let connected = CONNECTED.load(Ordering::Relaxed);
        let st = self.state.read().unwrap_or_else(|p| p.into_inner());
        self.recv_rate.update(st.packets_received);
        self.send_rate.update(BUNDLES_SENT.load(Ordering::Relaxed));
        Snapshot {
            connected,
            device_id: st.device_id.clone(),
            subject_name: st.subject_name.clone(),
            last_packet_age_ms: st.last_packet_time.map(|t| t.elapsed().as_millis() as u64),
            recv_hz: self.recv_rate.rate,
            send_hz: self.send_rate.rate,
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

    fn draw_home(&mut self, ui: &mut egui::Ui, snap: &Snapshot) {
        let margin = 16.0;

        // Top-right gear button
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() - 28.0);
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(GEAR).size(18.0).color(TEXT_DIM))
                        .frame(false),
                )
                .clicked()
            {
                self.view = View::Config;
            }
        });

        // Power button centered
        ui.vertical_centered(|ui| {
            ui.add_space(4.0);

            let (btn_color, btn_hover) = if self.enabled {
                (GREEN, egui::Color32::from_rgb(80, 220, 140))
            } else {
                (RED, egui::Color32::from_rgb(240, 80, 90))
            };

            let btn_size = egui::vec2(72.0, 72.0);
            let (rect, response) = ui.allocate_exact_size(btn_size, egui::Sense::click());

            if response.clicked() {
                self.enabled = !self.enabled;
            }

            let color = if response.hovered() { btn_hover } else { btn_color };
            let painter = ui.painter();
            painter.circle_filled(rect.center(), 34.0, color);

            // Power icon
            let c = rect.center();
            painter.circle_stroke(c, 20.0, egui::Stroke::new(2.5, BG));
            painter.line_segment(
                [c + egui::vec2(0.0, -24.0), c + egui::vec2(0.0, -11.0)],
                egui::Stroke::new(3.0, BG),
            );

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(if self.enabled { "ON" } else { "OFF" })
                    .size(11.0)
                    .color(TEXT_DIM),
            );
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        // Connection status — padded
        ui.horizontal(|ui| {
            ui.add_space(margin);
            status_dot(ui, snap.connected);
            let text = if snap.connected {
                format!("LiveLink  {}", snap.subject_name)
            } else {
                "LiveLink  waiting...".to_string()
            };
            ui.label(egui::RichText::new(text).size(13.0));
        });

        if snap.connected {
            ui.horizontal(|ui| {
                ui.add_space(margin + 16.0);
                ui.label(
                    egui::RichText::new(&snap.device_id)
                        .size(10.0)
                        .color(TEXT_DIM),
                );
            });
        }

        ui.add_space(2.0);

        ui.horizontal(|ui| {
            ui.add_space(margin);
            status_dot(ui, self.enabled);
            ui.label(
                egui::RichText::new(format!("OSC → {}", self.config.osc_target)).size(13.0),
            );
        });

        ui.horizontal(|ui| {
            ui.add_space(margin + 16.0);
            ui.label(
                egui::RichText::new(format!("listening :{}", self.config.listen_port))
                    .size(10.0)
                    .color(TEXT_DIM),
            );
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);

        // Stats row — centered
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                stat_label(ui, "msg/s in", &format!("{:.0}", snap.recv_hz));
                ui.add_space(16.0);
                stat_label(ui, "msg/s out", &format!("{:.0}", snap.send_hz));
                if let Some(age) = snap.last_packet_age_ms {
                    ui.add_space(16.0);
                    stat_label(ui, "latency", &format!("{}ms", age));
                }
            });
        });
    }

    fn draw_config(&mut self, ui: &mut egui::Ui) {
        // Back button (gear toggled)
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("\u{2190} back").size(13.0).color(TEXT_DIM))
                        .frame(false),
                )
                .clicked()
            {
                self.view = View::Home;
            }
        });

        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("Configuration").size(16.0).strong());
        });
        ui.add_space(8.0);

        let margin = 16.0;
        egui::Frame::new()
            .fill(SURFACE)
            .corner_radius(6)
            .inner_margin(12.0)
            .outer_margin(egui::Margin::symmetric(margin as i8, 0))
            .show(ui, |ui| {
                egui::Grid::new("config_grid")
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        config_row(ui, "listen port", &format!("{}", self.config.listen_port));
                        config_row(ui, "osc target", &self.config.osc_target);
                        config_row(ui, "prefix", &self.config.prefix);
                        config_row(ui, "send rate", &format!("{}Hz", self.config.send_rate));
                    });
            });
    }
}

struct Snapshot {
    connected: bool,
    device_id: String,
    subject_name: String,
    last_packet_age_ms: Option<u64>,
    recv_hz: f32,
    send_hz: f32,
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

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(8.0))
            .show(ctx, |ui| match self.view {
                View::Home => self.draw_home(ui, &snap),
                View::Config => self.draw_config(ui),
            });
    }
}

fn status_dot(ui: &mut egui::Ui, active: bool) {
    let color = if active { GREEN } else { RED };
    let r = 5.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(r * 2.0, r * 2.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), r, color);
}

fn stat_label(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(egui::RichText::new(value).size(15.0).strong());
        ui.label(egui::RichText::new(label).size(10.0).color(TEXT_DIM));
    });
}

fn config_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(egui::RichText::new(label).size(12.0).color(TEXT_DIM));
    ui.label(egui::RichText::new(value).size(12.0));
    ui.end_row();
}

pub fn run(state: Arc<RwLock<TrackingState>>, config: GuiConfig) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("LiteLink")
            .with_inner_size([260.0, 310.0])
            .with_min_inner_size([240.0, 260.0])
            .with_always_on_top(),
        ..Default::default()
    };
    eframe::run_native(
        "litelink",
        options,
        Box::new(move |_cc| Ok(Box::new(BridgeApp::new(state, config)))),
    )
}
