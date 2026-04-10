mod livelink;
mod mapping;
mod osc;
mod state;

use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use clap::Parser;
use log::{debug, error, info, warn};

use crate::mapping::map_blendshapes;
use crate::osc::OscSender;
use crate::state::{TrackingState, CONNECTED};

/// Global shutdown flag, set by Ctrl+C handler.
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

fn should_run() -> bool {
    !SHUTDOWN.load(Ordering::Relaxed)
}

#[derive(Parser)]
#[command(name = "livelink-vrcft", about = "LiveLink Face → VRChat OSC bridge")]
struct Args {
    /// UDP port to listen for LiveLink Face packets
    #[arg(long, default_value_t = 11111)]
    listen_port: u16,

    /// VRChat OSC target address
    #[arg(long, default_value = "127.0.0.1:9000")]
    osc_target: String,

    /// OSC send rate in Hz (VRChat processes at its frame rate; >60 provides no benefit)
    #[arg(long, default_value_t = 60)]
    send_rate: u32,

    /// Connection timeout in seconds (mark disconnected after no packets for this long)
    #[arg(long, default_value_t = 2.0)]
    timeout: f64,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    let osc_target: std::net::SocketAddr = args
        .osc_target
        .parse()
        .expect("invalid OSC target address");
    let send_interval = Duration::from_micros(1_000_000 / args.send_rate as u64);
    let timeout = Duration::from_secs_f64(args.timeout);

    // Register Ctrl+C handler
    ctrlc::set_handler(|| {
        info!("shutting down...");
        SHUTDOWN.store(true, Ordering::Relaxed);
    })
    .expect("failed to set Ctrl+C handler");

    let state = Arc::new(RwLock::new(TrackingState::new()));

    // Spawn UDP receiver thread
    let recv_state = Arc::clone(&state);
    let listen_port = args.listen_port;
    let recv_thread = std::thread::Builder::new()
        .name("udp-receiver".into())
        .spawn(move || run_receiver(listen_port, recv_state))
        .expect("failed to spawn receiver thread");

    // Spawn OSC sender thread
    let send_state = Arc::clone(&state);
    let send_thread = std::thread::Builder::new()
        .name("osc-sender".into())
        .spawn(move || run_sender(osc_target, send_interval, timeout, send_state))
        .expect("failed to spawn sender thread");

    info!(
        "listening for LiveLink on UDP :{listen_port}, sending OSC to {osc_target} at {}Hz",
        args.send_rate
    );

    // Wait for threads to finish (they exit on SHUTDOWN)
    let _ = recv_thread.join();
    let _ = send_thread.join();
    info!("goodbye");
}

fn run_receiver(port: u16, state: Arc<RwLock<TrackingState>>) {
    let addr = format!("0.0.0.0:{port}");
    let socket = UdpSocket::bind(&addr).unwrap_or_else(|e| {
        panic!("failed to bind UDP socket on {addr}: {e}");
    });
    // Short timeout so we can check SHUTDOWN periodically
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok();

    let mut buf = [0u8; 2048];

    while should_run() {
        let (len, src) = match socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue; // timeout, check shutdown and loop
            }
            Err(e) => {
                error!("UDP recv error: {e}");
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };

        let data = &buf[..len];
        match livelink::parse_packet(data) {
            Ok(packet) => {
                debug!(
                    "frame {} from {} ({src})",
                    packet.frame_number, packet.device_id,
                );
                if let Ok(mut st) = state.write() {
                    // Only update strings if they changed (avoids allocation)
                    if st.device_id != packet.device_id {
                        st.device_id = packet.device_id;
                    }
                    if st.subject_name != packet.subject_name {
                        st.subject_name = packet.subject_name;
                    }
                    st.blendshapes = packet.blendshapes;
                    st.frame_number = packet.frame_number;
                    st.mark_connected();
                }
            }
            Err(e) => {
                // Try tail fallback for non-standard header versions
                match livelink::parse_blendshapes_from_tail(data) {
                    Ok(shapes) => {
                        debug!("parsed {len} bytes via tail fallback from {src}");
                        if let Ok(mut st) = state.write() {
                            st.blendshapes = shapes;
                            st.mark_connected();
                        }
                    }
                    Err(_) => {
                        warn!("unparseable {len}-byte packet from {src}: {e}");
                    }
                }
            }
        }
    }
}

fn run_sender(
    target: std::net::SocketAddr,
    interval: Duration,
    timeout: Duration,
    state: Arc<RwLock<TrackingState>>,
) {
    let mut sender = OscSender::new(target).expect("failed to create OSC sender");

    let mut last_log = Instant::now();
    let mut send_count: u64 = 0;
    let mut was_connected = false;

    while should_run() {
        let start = Instant::now();

        // Read state (RwLock read - doesn't block receiver)
        let (blendshapes, connected, device_id, packets_received) = {
            let st = match state.read() {
                Ok(st) => st,
                Err(poisoned) => poisoned.into_inner(), // recover from poison
            };
            let changed = st.check_timeout(timeout);
            let connected = CONNECTED.load(Ordering::Relaxed);

            // Log connection state changes
            if changed {
                if connected {
                    info!(
                        "connected: receiving from \"{}\" ({})",
                        st.subject_name, st.device_id
                    );
                } else {
                    warn!("disconnected: no packets for {:.1}s", timeout.as_secs_f64());
                }
            }

            (st.blendshapes, connected, st.device_id.clone(), st.packets_received)
        };

        // Track connection transitions for startup message
        if connected && !was_connected {
            was_connected = true;
        } else if !connected && was_connected {
            was_connected = false;
        }

        let params = map_blendshapes(&blendshapes, connected);
        if let Err(e) = sender.send_params(&params) {
            warn!("OSC send error: {e}");
        }
        send_count += 1;

        // Periodic stats every 30 seconds
        if last_log.elapsed() >= Duration::from_secs(30) {
            if connected {
                info!(
                    "stats: {} OSC bundles sent, {} LiveLink packets received, device=\"{device_id}\"",
                    send_count, packets_received,
                );
            } else {
                info!(
                    "stats: {} OSC bundles sent, waiting for LiveLink packets on UDP :{}",
                    send_count,
                    // We don't have the port here, but the startup log has it
                    "11111"
                );
            }
            last_log = Instant::now();
        }

        let elapsed = start.elapsed();
        if elapsed < interval {
            std::thread::sleep(interval - elapsed);
        }
    }
}
