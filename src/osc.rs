use std::collections::HashMap;
use std::net::UdpSocket;

use rosc::encoder;
use rosc::{OscBundle, OscMessage, OscPacket, OscTime, OscType};

use crate::mapping::{OscParam, OscValue};

/// Epsilon for float change detection. Values changing less than this are skipped.
const FLOAT_EPSILON: f32 = 0.001;

pub struct OscSender {
    socket: UdpSocket,
    target: std::net::SocketAddr,
    /// Cache of last-sent values for change detection.
    last_sent: HashMap<&'static str, CachedValue>,
}

#[derive(Clone, Copy, PartialEq)]
enum CachedValue {
    Float(f32),
    Bool(bool),
}

impl OscSender {
    pub fn new(target: std::net::SocketAddr) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        Ok(Self {
            socket,
            target,
            last_sent: HashMap::with_capacity(128),
        })
    }

    /// Send only the parameters that have changed since last send.
    /// Returns the number of params actually sent.
    pub fn send_params(&mut self, params: &[OscParam]) -> std::io::Result<usize> {
        let mut content: Vec<OscPacket> = Vec::new();

        for p in params {
            let new_val = match &p.value {
                OscValue::Float(v) => CachedValue::Float(*v),
                OscValue::Bool(v) => CachedValue::Bool(*v),
            };

            let changed = match self.last_sent.get(p.address) {
                Some(CachedValue::Float(old)) => match new_val {
                    CachedValue::Float(new) => (new - old).abs() > FLOAT_EPSILON,
                    _ => true,
                },
                Some(CachedValue::Bool(old)) => match new_val {
                    CachedValue::Bool(new) => new != *old,
                    _ => true,
                },
                None => true, // first time, always send
            };

            if changed {
                self.last_sent.insert(p.address, new_val);
                let args = match &p.value {
                    OscValue::Float(v) => vec![OscType::Float(*v)],
                    OscValue::Bool(v) => vec![OscType::Bool(*v)],
                };
                content.push(OscPacket::Message(OscMessage {
                    addr: p.address.to_string(),
                    args,
                }));
            }
        }

        if content.is_empty() {
            return Ok(0);
        }

        let count = content.len();
        let bundle = OscPacket::Bundle(OscBundle {
            timetag: OscTime {
                seconds: 0,
                fractional: 1,
            },
            content,
        });

        let encoded = encoder::encode(&bundle)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        self.socket.send_to(&encoded, self.target)?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::map_blendshapes;
    use crate::state::BLENDSHAPE_COUNT;

    #[test]
    fn osc_messages_have_valid_addresses() {
        let bs = [0.5f32; BLENDSHAPE_COUNT];
        let params = map_blendshapes(&bs, true);

        for p in &params {
            assert!(p.address.starts_with('/'), "bad addr: {}", p.address);
            assert!(!p.address.contains(' '), "space in addr: {}", p.address);
        }
    }

    #[test]
    fn osc_bundle_encodes_successfully() {
        let bs = [0.0f32; BLENDSHAPE_COUNT];
        let params = map_blendshapes(&bs, true);

        let content: Vec<OscPacket> = params
            .iter()
            .map(|p| {
                let args = match &p.value {
                    OscValue::Float(v) => vec![OscType::Float(*v)],
                    OscValue::Bool(v) => vec![OscType::Bool(*v)],
                };
                OscPacket::Message(OscMessage {
                    addr: p.address.to_string(),
                    args,
                })
            })
            .collect();

        let bundle = OscPacket::Bundle(OscBundle {
            timetag: OscTime {
                seconds: 0,
                fractional: 1,
            },
            content,
        });

        let encoded = encoder::encode(&bundle);
        assert!(encoded.is_ok(), "encoding failed: {:?}", encoded.err());
        let bytes = encoded.unwrap();
        assert!(
            bytes.len() > 100,
            "bundle suspiciously small: {} bytes",
            bytes.len()
        );
    }

    #[test]
    fn sender_binds_successfully() {
        let target: std::net::SocketAddr = "127.0.0.1:9000".parse().unwrap();
        let sender = OscSender::new(target);
        assert!(sender.is_ok());
    }

    #[test]
    fn change_detection_skips_identical_values() {
        // Bind a real receiver so send_to has a valid target
        let recv = UdpSocket::bind("127.0.0.1:0").unwrap();
        let target = recv.local_addr().unwrap();
        let mut sender = OscSender::new(target).unwrap();

        let bs = [0.5f32; BLENDSHAPE_COUNT];
        let params = map_blendshapes(&bs, true);

        // First send: all params are new, all should be sent
        let count1 = sender.send_params(&params).unwrap();
        assert!(count1 > 90, "first send should send all params, got {count1}");

        // Second send with identical values: nothing should be sent
        let params2 = map_blendshapes(&bs, true);
        let count2 = sender.send_params(&params2).unwrap();
        assert_eq!(count2, 0, "identical values should send nothing");
    }

    #[test]
    fn change_detection_sends_changed_values() {
        // Bind a real receiver so send_to has a valid target
        let recv = UdpSocket::bind("127.0.0.1:0").unwrap();
        let target = recv.local_addr().unwrap();
        let mut sender = OscSender::new(target).unwrap();

        let bs = [0.0f32; BLENDSHAPE_COUNT];
        let params = map_blendshapes(&bs, true);
        sender.send_params(&params).unwrap();

        // Change one blendshape significantly
        let mut bs2 = [0.0f32; BLENDSHAPE_COUNT];
        bs2[17] = 0.8; // JawOpen
        let params2 = map_blendshapes(&bs2, true);
        let count = sender.send_params(&params2).unwrap();
        // Should send JawOpen + derived params that depend on it, but not all
        assert!(count > 0, "changed values should be sent");
        assert!(count < 90, "only changed params should be sent, got {count}");
    }

    #[test]
    fn change_detection_ignores_tiny_changes() {
        // Bind a real receiver so send_to has a valid target
        let recv = UdpSocket::bind("127.0.0.1:0").unwrap();
        let target = recv.local_addr().unwrap();
        let mut sender = OscSender::new(target).unwrap();

        let bs = [0.5f32; BLENDSHAPE_COUNT];
        let params = map_blendshapes(&bs, true);
        sender.send_params(&params).unwrap();

        // Tiny change below epsilon
        let mut bs2 = [0.5f32; BLENDSHAPE_COUNT];
        bs2[17] = 0.5005; // below FLOAT_EPSILON of 0.001
        let params2 = map_blendshapes(&bs2, true);
        let count = sender.send_params(&params2).unwrap();
        assert_eq!(count, 0, "sub-epsilon changes should be skipped");
    }
}
