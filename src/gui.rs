use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use eframe::egui;

use crate::state::{TrackingState, CONNECTED, SHUTDOWN};

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
}

impl BridgeApp {
    pub fn new(state: Arc<RwLock<TrackingState>>) -> Self {
        Self { state }
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
}

impl eframe::App for BridgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Auto-close on Ctrl+C
        if SHUTDOWN.load(Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        ctx.request_repaint_after(Duration::from_millis(250));
        let snap = self.snapshot();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (color, label) = if snap.connected {
                    (egui::Color32::from_rgb(80, 200, 80), "Connected")
                } else {
                    (egui::Color32::from_rgb(200, 60, 60), "Disconnected")
                };
                let dot_size = 8.0;
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(dot_size, dot_size),
                    egui::Sense::hover(),
                );
                ui.painter().circle_filled(rect.center(), dot_size / 2.0, color);
                ui.strong(label);
            });

            ui.separator();

            if snap.connected {
                ui.label(format!("{} ({})", snap.subject_name, snap.device_id));
                ui.label(format!(
                    "frame {} | {} packets",
                    snap.frame_number, snap.packets_received
                ));
                if let Some(age) = snap.last_packet_age_ms {
                    ui.label(format!("last packet: {}ms ago", age));
                }
            } else {
                ui.label("waiting for LiveLink...");
            }
        });
    }
}

pub fn run(state: Arc<RwLock<TrackingState>>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("livelink-vrcft")
            .with_inner_size([280.0, 120.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "livelink-vrcft",
        options,
        Box::new(move |_cc| Ok(Box::new(BridgeApp::new(state)))),
    )
}
