use std::net::UdpSocket;
use std::time::Duration;

use rosc::decoder;
use rosc::{OscPacket, OscType};

const BLENDSHAPE_COUNT: usize = 61;

/// Build a synthetic LiveLink packet.
fn build_livelink_packet(
    device_id: &str,
    subject_name: &str,
    frame: u32,
    blendshapes: &[f32; BLENDSHAPE_COUNT],
) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(6); // version
    buf.extend_from_slice(&(device_id.len() as u32).to_be_bytes());
    buf.extend_from_slice(device_id.as_bytes());
    buf.extend_from_slice(&(subject_name.len() as u32).to_be_bytes());
    buf.extend_from_slice(subject_name.as_bytes());
    buf.extend_from_slice(&frame.to_be_bytes());
    buf.extend_from_slice(&0.0f32.to_be_bytes());
    buf.extend_from_slice(&30u32.to_be_bytes());
    buf.extend_from_slice(&1u32.to_be_bytes());
    buf.push(BLENDSHAPE_COUNT as u8);
    for &val in blendshapes {
        buf.extend_from_slice(&val.to_be_bytes());
    }
    buf
}

/// End-to-end test: send a LiveLink packet to the receiver, capture the OSC output.
///
/// This test starts the full binary with custom ports and verifies OSC output.
#[test]
fn end_to_end_packet_flow() {
    // Use high ports to avoid conflicts
    let livelink_port = 19111;
    let osc_port = 19222;

    // Bind an OSC receiver socket before starting the binary
    let osc_receiver = UdpSocket::bind(format!("127.0.0.1:{osc_port}")).unwrap();
    osc_receiver
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    // Start the binary as a subprocess
    let exe = env!("CARGO_BIN_EXE_litelink");
    let mut child = std::process::Command::new(exe)
        .args([
            "--listen-port",
            &livelink_port.to_string(),
            "--osc-target",
            &format!("127.0.0.1:{osc_port}"),
            "--send-rate",
            "50",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to start litelink");

    // Give it a moment to bind
    std::thread::sleep(Duration::from_millis(200));

    // Send a LiveLink packet
    let sender = UdpSocket::bind("0.0.0.0:0").unwrap();
    let mut shapes = [0.0f32; BLENDSHAPE_COUNT];
    shapes[17] = 0.85; // JawOpen

    let packet = build_livelink_packet("TestDevice", "TestSubject", 1, &shapes);
    sender
        .send_to(&packet, format!("127.0.0.1:{livelink_port}"))
        .unwrap();

    // Read OSC bundles until we get one with our JawOpen value.
    // The sender may emit a few zero-state bundles before our LiveLink packet is processed.
    let mut buf = [0u8; 8192];
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut found_jaw_open = false;
    let mut found_tracking = false;

    while std::time::Instant::now() < deadline {
        let (len, _) = match osc_receiver.recv_from(&mut buf) {
            Ok(v) => v,
            Err(_) => break,
        };
        let osc_data = &buf[..len];
        let (_, osc_packet) = decoder::decode_udp(osc_data).expect("failed to decode OSC");

        let messages = match osc_packet {
            OscPacket::Bundle(bundle) => {
                let mut msgs = Vec::new();
                for pkt in bundle.content {
                    if let OscPacket::Message(msg) = pkt {
                        msgs.push(msg);
                    }
                }
                msgs
            }
            OscPacket::Message(msg) => vec![msg],
        };

        // Check JawOpen
        if let Some(jaw_open) = messages.iter().find(|m| m.addr.ends_with("JawOpen")) {
            if let OscType::Float(v) = &jaw_open.args[0] {
                if (*v - 0.85).abs() < 0.01 {
                    found_jaw_open = true;
                }
            }
        }

        // Check tracking status
        if let Some(tracking) = messages
            .iter()
            .find(|m| m.addr.contains("ExpressionTrackingActive"))
        {
            if let OscType::Bool(true) = &tracking.args[0] {
                found_tracking = true;
            }
        }

        if found_jaw_open && found_tracking {
            break;
        }
    }

    assert!(found_jaw_open, "never received JawOpen ~0.85 in OSC output");
    assert!(
        found_tracking,
        "never received ExpressionTrackingActive=true"
    );

    // Clean up
    child.kill().ok();
    child.wait().ok();
}
