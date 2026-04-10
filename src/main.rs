#[cfg(feature = "gui")]
mod gui;
mod livelink;
mod mapping;
mod osc;
mod state;

use std::net::UdpSocket;
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use clap::Parser;
use log::{debug, error, info, warn};

use crate::mapping::map_blendshapes;
use crate::osc::OscSender;
use crate::state::{TrackingState, BUNDLES_SENT, CONNECTED, SHUTDOWN};

fn should_run() -> bool {
    !SHUTDOWN.load(Ordering::Relaxed)
}

#[derive(Parser)]
#[command(name = "litelink", about = "Fast & lightweight LiveLink Face → VRChat OSC bridge")]
struct Args {
    /// UDP port to listen for LiveLink Face packets
    #[arg(long, default_value_t = 11111)]
    listen_port: u16,

    /// VRChat OSC target address
    #[arg(long, default_value = "127.0.0.1:9000")]
    osc_target: String,

    /// OSC parameter prefix (must match your avatar's parameter naming)
    #[arg(long, default_value = "/avatar/parameters/FT/v2")]
    prefix: String,

    /// OSC send rate in Hz
    #[arg(long, default_value_t = 60)]
    send_rate: u32,

    /// Connection timeout in seconds
    #[arg(long, default_value_t = 2.0)]
    timeout: f64,

    /// Run without the status window (only relevant when compiled with gui feature)
    #[cfg(feature = "gui")]
    #[arg(long)]
    headless: bool,

    /// Run a benchmark for N seconds, then print a performance summary and exit
    #[arg(long, value_name = "SECONDS")]
    benchmark: Option<u64>,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    let osc_target: std::net::SocketAddr = args
        .osc_target
        .parse()
        .expect("invalid --osc-target address");
    let send_interval = Duration::from_micros(1_000_000 / args.send_rate as u64);
    let timeout = Duration::from_secs_f64(args.timeout);

    ctrlc::set_handler(|| {
        SHUTDOWN.store(true, Ordering::Relaxed);
    })
    .expect("failed to set Ctrl+C handler");

    let state = Arc::new(RwLock::new(TrackingState::new()));

    let recv_state = Arc::clone(&state);
    let listen_port = args.listen_port;
    let recv_thread = std::thread::Builder::new()
        .name("udp-receiver".into())
        .spawn(move || run_receiver(listen_port, recv_state))
        .expect("failed to spawn receiver thread");

    let send_state = Arc::clone(&state);
    let prefix = args.prefix.clone();
    let send_thread = std::thread::Builder::new()
        .name("osc-sender".into())
        .spawn(move || run_sender(osc_target, send_interval, timeout, send_state, &prefix))
        .expect("failed to spawn sender thread");

    info!(
        "LiveLink :{listen_port} → OSC {osc_target} @ {}Hz (prefix={})",
        args.send_rate, args.prefix
    );

    // Benchmark mode
    if let Some(duration_secs) = args.benchmark {
        run_benchmark(duration_secs, &state);
        SHUTDOWN.store(true, Ordering::Relaxed);
        let _ = recv_thread.join();
        let _ = send_thread.join();
        return;
    }

    // GUI path: eframe takes over the main thread (default when compiled with gui feature)
    #[cfg(feature = "gui")]
    if !args.headless {
        let gui_config = gui::GuiConfig {
            listen_port,
            osc_target: args.osc_target.clone(),
            prefix: args.prefix.clone(),
            send_rate: args.send_rate,
        };
        if let Err(e) = gui::run(Arc::clone(&state), gui_config) {
            error!("GUI error: {e}");
        }
        SHUTDOWN.store(true, Ordering::Relaxed);
        let _ = recv_thread.join();
        let _ = send_thread.join();
        return;
    }

    // Headless path
    let _ = recv_thread.join();
    let _ = send_thread.join();
    info!("goodbye");
}

fn run_receiver(port: u16, state: Arc<RwLock<TrackingState>>) {
    let addr = format!("0.0.0.0:{port}");
    let socket = UdpSocket::bind(&addr).unwrap_or_else(|e| {
        panic!("failed to bind UDP socket on {addr}: {e}");
    });
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok();

    let mut buf = [0u8; 2048];

    while should_run() {
        let (len, src) = match socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
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
                debug!("frame {} from {} ({src})", packet.frame_number, packet.device_id);
                if let Ok(mut st) = state.write() {
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
            Err(e) => match livelink::parse_blendshapes_from_tail(data) {
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
            },
        }
    }
}

fn run_sender(
    target: std::net::SocketAddr,
    interval: Duration,
    timeout: Duration,
    state: Arc<RwLock<TrackingState>>,
    prefix: &str,
) {
    let mut sender = OscSender::new(target).expect("failed to create OSC sender");
    let mut last_stats = Instant::now();

    while should_run() {
        let start = Instant::now();

        let (blendshapes, connected, device_id, packets_received) = {
            let st = match state.read() {
                Ok(st) => st,
                Err(poisoned) => poisoned.into_inner(),
            };
            let changed = st.check_timeout(timeout);
            let connected = CONNECTED.load(Ordering::Relaxed);

            if changed {
                if connected {
                    info!("connected: \"{}\" ({})", st.subject_name, st.device_id);
                } else {
                    warn!("disconnected: no packets for {:.1}s", timeout.as_secs_f64());
                }
            }

            (st.blendshapes, connected, st.device_id.clone(), st.packets_received)
        };

        let params = map_blendshapes(&blendshapes, connected, prefix);
        if let Err(e) = sender.send_params(&params) {
            warn!("OSC send error: {e}");
        }
        BUNDLES_SENT.fetch_add(1, Ordering::Relaxed);

        if last_stats.elapsed() >= Duration::from_secs(30) {
            let sent = BUNDLES_SENT.load(Ordering::Relaxed);
            if connected {
                info!("stats: {sent} bundles sent, {packets_received} packets recv, device=\"{device_id}\"");
            } else {
                info!("stats: {sent} bundles sent, waiting for LiveLink packets");
            }
            last_stats = Instant::now();
        }

        let elapsed = start.elapsed();
        if elapsed < interval {
            std::thread::sleep(interval - elapsed);
        }
    }
}

fn run_benchmark(duration_secs: u64, state: &Arc<RwLock<TrackingState>>) {
    let duration = Duration::from_secs(duration_secs);
    eprintln!("benchmark: running for {duration_secs}s...");
    eprintln!("benchmark: send LiveLink data from your iPhone now\n");

    // Wait for first packet or timeout
    let wait_start = Instant::now();
    loop {
        if CONNECTED.load(Ordering::Relaxed) {
            break;
        }
        if wait_start.elapsed() > Duration::from_secs(10) {
            eprintln!("benchmark: no LiveLink packets received after 10s, running anyway\n");
            break;
        }
        if !should_run() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Snapshot counters at start
    let start = Instant::now();
    let recv_start = state.read().map(|s| s.packets_received).unwrap_or(0);
    let send_start = BUNDLES_SENT.load(Ordering::Relaxed);
    let rss_start = read_rss_kb();

    // Sample latency periodically
    let mut latency_samples: Vec<u64> = Vec::new();
    let mut peak_rss = rss_start;

    while start.elapsed() < duration && should_run() {
        std::thread::sleep(Duration::from_millis(100));

        if let Ok(st) = state.read() {
            if let Some(t) = st.last_packet_time {
                latency_samples.push(t.elapsed().as_millis() as u64);
            }
        }

        let rss = read_rss_kb();
        if rss > peak_rss {
            peak_rss = rss;
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let recv_end = state.read().map(|s| s.packets_received).unwrap_or(0);
    let send_end = BUNDLES_SENT.load(Ordering::Relaxed);
    let rss_end = read_rss_kb();

    let recv_total = recv_end - recv_start;
    let send_total = send_end - send_start;
    let recv_rate = recv_total as f64 / elapsed;
    let send_rate = send_total as f64 / elapsed;

    let (lat_avg, lat_p50, lat_p99) = if !latency_samples.is_empty() {
        let mut sorted = latency_samples.clone();
        sorted.sort();
        let avg = sorted.iter().sum::<u64>() as f64 / sorted.len() as f64;
        let p50 = sorted[sorted.len() / 2];
        let p99 = sorted[(sorted.len() as f64 * 0.99) as usize];
        (avg, p50, p99)
    } else {
        (0.0, 0, 0)
    };

    println!();
    println!("═══════════════════════════════════════════");
    println!("  litelink benchmark ({elapsed:.1}s)");
    println!("═══════════════════════════════════════════");
    println!();
    println!("  Throughput");
    println!("    recv:   {recv_rate:>8.1} msg/s  ({recv_total} total)");
    println!("    send:   {send_rate:>8.1} msg/s  ({send_total} total)");
    println!();
    println!("  Latency (recv → process)");
    println!("    avg:    {lat_avg:>8.1} ms");
    println!("    p50:    {lat_p50:>8} ms");
    println!("    p99:    {lat_p99:>8} ms");
    println!();
    println!("  Memory (RSS)");
    println!("    start:  {rss_start:>8} KB");
    println!("    end:    {rss_end:>8} KB");
    println!("    peak:   {peak_rss:>8} KB");
    println!();
    println!("═══════════════════════════════════════════");
}

/// Read current RSS (Resident Set Size) in KB from /proc/self/status.
fn read_rss_kb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| {
                    l.split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse::<u64>().ok())
                })
        })
        .unwrap_or(0)
}
