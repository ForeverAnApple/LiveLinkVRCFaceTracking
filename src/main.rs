mod livelink;
mod mapping;
mod osc;
mod state;

use std::fs::File;
use std::io::Write as IoWrite;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use log::{debug, error, info, warn};

use crate::mapping::{map_blendshapes, OscValue};
use crate::osc::OscSender;
use crate::state::{TrackingState, ARKIT_BLENDSHAPE_NAMES, BLENDSHAPE_COUNT, CONNECTED};

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

    /// OSC parameter prefix (e.g. "/avatar/parameters/FT/v2" for VRCFT avatars)
    #[arg(long, default_value = "/avatar/parameters/FT/v2")]
    prefix: String,

    /// Enable diagnostic logging to files in ./logs/
    #[arg(long)]
    log: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Proxy OSC messages: listen on a port, log them, forward to VRChat.
    /// Configure VRCFT to send to --listen-port instead of 9000,
    /// and this will log + forward everything to --forward-to.
    Sniff {
        /// Port to listen on (tell VRCFT to send here instead of 9000)
        #[arg(long, default_value_t = 9001)]
        listen_port: u16,

        /// Forward captured messages to this address (VRChat)
        #[arg(long, default_value = "127.0.0.1:9000")]
        forward_to: String,

        /// Enable diagnostic logging to logs/vrcft_sniff.log
        #[arg(long)]
        log: bool,
    },
}

/// Thread-safe log file writer.
struct LogFile {
    file: Mutex<File>,
}

impl LogFile {
    fn create(path: &str) -> std::io::Result<Self> {
        std::fs::create_dir_all("logs")?;
        let file = File::create(path)?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }

    fn write_line(&self, line: &str) {
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{}", line);
        }
    }
}

fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let millis = now.subsec_millis();
    format!("{secs}.{millis:03}")
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    // Register Ctrl+C handler
    ctrlc::set_handler(|| {
        info!("shutting down...");
        SHUTDOWN.store(true, Ordering::Relaxed);
    })
    .expect("failed to set Ctrl+C handler");

    // Handle subcommands
    if let Some(Command::Sniff {
        listen_port: sniff_port,
        forward_to,
        log: sniff_log,
    }) = &args.command
    {
        let forward_addr: std::net::SocketAddr =
            forward_to.parse().expect("invalid --forward-to address");
        run_sniff(*sniff_port, forward_addr, *sniff_log);
        return;
    }

    let osc_target: std::net::SocketAddr = args
        .osc_target
        .parse()
        .expect("invalid OSC target address");
    let send_interval = Duration::from_micros(1_000_000 / args.send_rate as u64);
    let timeout = Duration::from_secs_f64(args.timeout);

    // Set up log files if --log is enabled
    let input_log = if args.log {
        Some(Arc::new(
            LogFile::create("logs/livelink_input.log")
                .expect("failed to create logs/livelink_input.log"),
        ))
    } else {
        None
    };
    let output_log = if args.log {
        Some(Arc::new(
            LogFile::create("logs/osc_output.log").expect("failed to create logs/osc_output.log"),
        ))
    } else {
        None
    };

    if args.log {
        info!("--log enabled: writing to logs/livelink_input.log and logs/osc_output.log");
        // Write headers
        if let Some(ref log) = input_log {
            let header = std::iter::once("timestamp".to_string())
                .chain(ARKIT_BLENDSHAPE_NAMES.iter().map(|s| s.to_string()))
                .collect::<Vec<_>>()
                .join("\t");
            log.write_line(&header);
        }
        if let Some(ref log) = output_log {
            log.write_line("timestamp\taddress\tvalue");
        }
    }

    let state = Arc::new(RwLock::new(TrackingState::new()));

    // Spawn UDP receiver thread
    let recv_state = Arc::clone(&state);
    let listen_port = args.listen_port;
    let recv_log = input_log.clone();
    let recv_thread = std::thread::Builder::new()
        .name("udp-receiver".into())
        .spawn(move || run_receiver(listen_port, recv_state, recv_log))
        .expect("failed to spawn receiver thread");

    // Spawn OSC sender thread
    let send_state = Arc::clone(&state);
    let prefix = args.prefix.clone();
    let send_thread = std::thread::Builder::new()
        .name("osc-sender".into())
        .spawn(move || run_sender(osc_target, send_interval, timeout, send_state, output_log, &prefix))
        .expect("failed to spawn sender thread");

    info!(
        "listening for LiveLink on UDP :{listen_port}, sending OSC to {osc_target} at {}Hz, prefix={}",
        args.send_rate, args.prefix
    );

    let _ = recv_thread.join();
    let _ = send_thread.join();
    info!("goodbye");
}

fn run_receiver(
    port: u16,
    state: Arc<RwLock<TrackingState>>,
    log_file: Option<Arc<LogFile>>,
) {
    let addr = format!("0.0.0.0:{port}");
    let socket = UdpSocket::bind(&addr).unwrap_or_else(|e| {
        panic!("failed to bind UDP socket on {addr}: {e}");
    });
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok();

    let mut buf = [0u8; 2048];
    let mut last_log_time = Instant::now();

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
                debug!(
                    "frame {} from {} ({src})",
                    packet.frame_number, packet.device_id,
                );

                // Log raw input blendshapes (throttled to ~10Hz to avoid huge files)
                if let Some(ref log) = log_file {
                    if last_log_time.elapsed() >= Duration::from_millis(100) {
                        let ts = timestamp();
                        let values: Vec<String> =
                            packet.blendshapes.iter().map(|v| format!("{v:.6}")).collect();
                        log.write_line(&format!("{ts}\t{}", values.join("\t")));
                        last_log_time = Instant::now();
                    }
                }

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

                    if let Some(ref log) = log_file {
                        if last_log_time.elapsed() >= Duration::from_millis(100) {
                            let ts = timestamp();
                            let values: Vec<String> =
                                shapes.iter().map(|v| format!("{v:.6}")).collect();
                            log.write_line(&format!("{ts}\t{}", values.join("\t")));
                            last_log_time = Instant::now();
                        }
                    }

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
    log_file: Option<Arc<LogFile>>,
    prefix: &str,
) {
    let mut sender = OscSender::new(target).expect("failed to create OSC sender");

    let mut last_stats = Instant::now();
    let mut last_log_time = Instant::now();
    let mut send_count: u64 = 0;

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
                    info!(
                        "connected: receiving from \"{}\" ({})",
                        st.subject_name, st.device_id
                    );
                } else {
                    warn!("disconnected: no packets for {:.1}s", timeout.as_secs_f64());
                }
            }

            (
                st.blendshapes,
                connected,
                st.device_id.clone(),
                st.packets_received,
            )
        };

        let params = map_blendshapes(&blendshapes, connected, prefix);

        // Log output params (throttled to ~10Hz)
        if let Some(ref log) = log_file {
            if last_log_time.elapsed() >= Duration::from_millis(100) {
                let ts = timestamp();
                for p in &params {
                    let val = match &p.value {
                        OscValue::Float(v) => format!("{v:.6}"),
                        OscValue::Bool(v) => format!("{v}"),
                    };
                    log.write_line(&format!("{ts}\t{}\t{val}", p.address));
                }
                last_log_time = Instant::now();
            }
        }

        if let Err(e) = sender.send_params(&params) {
            warn!("OSC send error: {e}");
        }
        send_count += 1;

        if last_stats.elapsed() >= Duration::from_secs(30) {
            if connected {
                info!(
                    "stats: {} OSC bundles sent, {} LiveLink packets received, device=\"{device_id}\"",
                    send_count, packets_received,
                );
            } else {
                info!(
                    "stats: {} OSC bundles sent, waiting for LiveLink packets",
                    send_count
                );
            }
            last_stats = Instant::now();
        }

        let elapsed = start.elapsed();
        if elapsed < interval {
            std::thread::sleep(interval - elapsed);
        }
    }
}

/// Sniff/proxy mode: listen for OSC messages, log them, and forward to VRChat.
fn run_sniff(listen_port: u16, forward_to: std::net::SocketAddr, log_enabled: bool) {
    info!("OSC proxy: listening on :{listen_port}, forwarding to {forward_to}");
    info!(
        "configure VRCFT to send OSC to port {listen_port} instead of {}",
        forward_to.port()
    );

    let log_file = if log_enabled {
        let log = LogFile::create("logs/vrcft_sniff.log")
            .expect("failed to create logs/vrcft_sniff.log");
        log.write_line("timestamp\taddress\tvalue");
        info!("--log enabled: writing sniffed params to logs/vrcft_sniff.log");
        Some(log)
    } else {
        None
    };

    let socket = UdpSocket::bind(format!("0.0.0.0:{listen_port}")).unwrap_or_else(|e| {
        panic!("failed to bind to port {listen_port}: {e}");
    });
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok();

    let forward_socket = UdpSocket::bind("0.0.0.0:0").expect("failed to bind forward socket");

    let mut buf = [0u8; 8192];
    let mut msg_count: u64 = 0;
    let mut last_summary = Instant::now();
    let mut last_log_time = Instant::now();
    let mut param_tracker: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    while should_run() {
        let (len, src) = match socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                if !param_tracker.is_empty() && last_summary.elapsed() >= Duration::from_secs(2) {
                    print_sniff_summary(&param_tracker, msg_count);
                    last_summary = Instant::now();
                }
                continue;
            }
            Err(e) => {
                error!("recv error: {e}");
                continue;
            }
        };

        let data = &buf[..len];

        // Forward raw packet to VRChat immediately
        if let Err(e) = forward_socket.send_to(data, forward_to) {
            warn!("forward error: {e}");
        }

        // Decode and log
        match rosc::decoder::decode_udp(data) {
            Ok((_, packet)) => {
                let messages = extract_messages(packet);
                let should_log_this_tick = last_log_time.elapsed() >= Duration::from_millis(100);

                for msg in &messages {
                    let val = format_osc_args(&msg.args);
                    param_tracker.insert(msg.addr.clone(), val.clone());
                    msg_count += 1;

                    // Write to log file (throttled)
                    if should_log_this_tick {
                        if let Some(ref log) = log_file {
                            let ts = timestamp();
                            log.write_line(&format!("{ts}\t{}\t{val}", msg.addr));
                        }
                    }
                }
                if should_log_this_tick {
                    last_log_time = Instant::now();
                }

                debug!("{} messages from {src} ({msg_count} total)", messages.len());
            }
            Err(e) => {
                warn!("failed to decode {len}-byte OSC packet from {src}: {e}");
            }
        }

        if last_summary.elapsed() >= Duration::from_secs(2) {
            print_sniff_summary(&param_tracker, msg_count);
            last_summary = Instant::now();
        }
    }

    if !param_tracker.is_empty() {
        info!("=== FINAL SNAPSHOT ===");
        print_sniff_summary(&param_tracker, msg_count);
    }
}

fn extract_messages(packet: rosc::OscPacket) -> Vec<rosc::OscMessage> {
    match packet {
        rosc::OscPacket::Message(msg) => vec![msg],
        rosc::OscPacket::Bundle(bundle) => bundle
            .content
            .into_iter()
            .flat_map(extract_messages)
            .collect(),
    }
}

fn format_osc_args(args: &[rosc::OscType]) -> String {
    args.iter()
        .map(|a| match a {
            rosc::OscType::Float(v) => format!("{v:.6}"),
            rosc::OscType::Bool(v) => format!("{v}"),
            rosc::OscType::Int(v) => format!("{v}"),
            rosc::OscType::String(v) => format!("\"{v}\""),
            other => format!("{other:?}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn print_sniff_summary(
    params: &std::collections::HashMap<String, String>,
    total: u64,
) {
    let mut sorted: Vec<_> = params.iter().collect();
    sorted.sort_by_key(|(k, _)| k.as_str());
    let mut lines = Vec::new();
    for (addr, val) in &sorted {
        lines.push(format!("  {addr} = {val}"));
    }
    info!(
        "--- sniff: {} unique params, {total} msgs total ---\n{}",
        sorted.len(),
        lines.join("\n")
    );
}
