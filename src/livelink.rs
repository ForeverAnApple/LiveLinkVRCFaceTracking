use crate::state::BLENDSHAPE_COUNT;

/// Parsed LiveLink Face packet.
#[derive(Debug, Clone)]
pub struct LiveLinkPacket {
    pub device_id: String,
    pub subject_name: String,
    pub frame_number: u32,
    pub sub_frame: f32,
    pub fps_numerator: u32,
    pub fps_denominator: u32,
    pub blendshapes: [f32; BLENDSHAPE_COUNT],
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("packet too short: need at least {need} bytes, got {got}")]
    TooShort { need: usize, got: usize },
    #[error("unexpected packet version: {0} (expected 6)")]
    BadVersion(u8),
    #[error("unexpected blendshape count: {0} (expected {BLENDSHAPE_COUNT})")]
    BadBlendshapeCount(u8),
    #[error("invalid UTF-8 in {field}: {source}")]
    InvalidUtf8 {
        field: &'static str,
        source: std::string::FromUtf8Error,
    },
}

/// Read a big-endian u32 from a byte slice at the given offset.
fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

/// Read a big-endian f32 from a byte slice at the given offset.
fn read_f32(data: &[u8], offset: usize) -> f32 {
    f32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

/// Parse a full LiveLink Face packet with header fields.
pub fn parse_packet(data: &[u8]) -> Result<LiveLinkPacket, ParseError> {
    // Minimum: 1 (version) + 4 (device_id len) + 0 (device_id) + 4 (subject len) + 0 (subject)
    //          + 4 (frame) + 4 (subframe) + 4 (fps_num) + 4 (fps_den) + 1 (count) + 244 (shapes)
    //        = 270 minimum with empty strings
    if data.len() < 270 {
        return Err(ParseError::TooShort {
            need: 270,
            got: data.len(),
        });
    }

    let mut pos = 0;

    // Packet version
    let version = data[pos];
    pos += 1;
    if version != 6 {
        return Err(ParseError::BadVersion(version));
    }

    // Device ID
    let device_id_len = read_u32(data, pos) as usize;
    pos += 4;
    if pos + device_id_len > data.len() {
        return Err(ParseError::TooShort {
            need: pos + device_id_len,
            got: data.len(),
        });
    }
    let device_id = String::from_utf8(data[pos..pos + device_id_len].to_vec()).map_err(|e| {
        ParseError::InvalidUtf8 {
            field: "device_id",
            source: e,
        }
    })?;
    pos += device_id_len;

    // Subject name
    if pos + 4 > data.len() {
        return Err(ParseError::TooShort {
            need: pos + 4,
            got: data.len(),
        });
    }
    let subject_name_len = read_u32(data, pos) as usize;
    pos += 4;
    if pos + subject_name_len > data.len() {
        return Err(ParseError::TooShort {
            need: pos + subject_name_len,
            got: data.len(),
        });
    }
    let subject_name =
        String::from_utf8(data[pos..pos + subject_name_len].to_vec()).map_err(|e| {
            ParseError::InvalidUtf8 {
                field: "subject_name",
                source: e,
            }
        })?;
    pos += subject_name_len;

    // Frame info: frame_number (u32) + sub_frame (f32) + fps_num (u32) + fps_den (u32) = 16 bytes
    // Plus blendshape_count (u8) + 61*4 bytes = 245
    let remaining_needed = 16 + 1 + BLENDSHAPE_COUNT * 4;
    if pos + remaining_needed > data.len() {
        return Err(ParseError::TooShort {
            need: pos + remaining_needed,
            got: data.len(),
        });
    }

    let frame_number = read_u32(data, pos);
    pos += 4;
    let sub_frame = read_f32(data, pos);
    pos += 4;
    let fps_numerator = read_u32(data, pos);
    pos += 4;
    let fps_denominator = read_u32(data, pos);
    pos += 4;

    let blendshape_count = data[pos];
    pos += 1;
    if blendshape_count != BLENDSHAPE_COUNT as u8 {
        return Err(ParseError::BadBlendshapeCount(blendshape_count));
    }

    let mut blendshapes = [0.0f32; BLENDSHAPE_COUNT];
    for (i, shape) in blendshapes.iter_mut().enumerate() {
        *shape = read_f32(data, pos + i * 4);
    }

    Ok(LiveLinkPacket {
        device_id,
        subject_name,
        frame_number,
        sub_frame,
        fps_numerator,
        fps_denominator,
        blendshapes,
    })
}

/// Fallback parser: just read the trailing 244 bytes as 61 big-endian f32s.
/// Useful when the header format changes but blendshapes remain at the tail.
pub fn parse_blendshapes_from_tail(data: &[u8]) -> Result<[f32; BLENDSHAPE_COUNT], ParseError> {
    let needed = BLENDSHAPE_COUNT * 4;
    if data.len() < needed {
        return Err(ParseError::TooShort {
            need: needed,
            got: data.len(),
        });
    }

    let start = data.len() - needed;
    let mut blendshapes = [0.0f32; BLENDSHAPE_COUNT];
    for (i, shape) in blendshapes.iter_mut().enumerate() {
        *shape = read_f32(data, start + i * 4);
    }
    Ok(blendshapes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a valid test packet with known values.
    fn build_test_packet(
        device_id: &str,
        subject_name: &str,
        frame: u32,
        blendshapes: &[f32; BLENDSHAPE_COUNT],
    ) -> Vec<u8> {
        let mut buf = Vec::new();

        // Version
        buf.push(6);

        // Device ID
        buf.extend_from_slice(&(device_id.len() as u32).to_be_bytes());
        buf.extend_from_slice(device_id.as_bytes());

        // Subject name
        buf.extend_from_slice(&(subject_name.len() as u32).to_be_bytes());
        buf.extend_from_slice(subject_name.as_bytes());

        // Frame info
        buf.extend_from_slice(&frame.to_be_bytes());
        buf.extend_from_slice(&0.0f32.to_be_bytes()); // sub_frame
        buf.extend_from_slice(&30u32.to_be_bytes()); // fps numerator
        buf.extend_from_slice(&1u32.to_be_bytes()); // fps denominator

        // Blendshape count
        buf.push(BLENDSHAPE_COUNT as u8);

        // Blendshapes
        for &val in blendshapes {
            buf.extend_from_slice(&val.to_be_bytes());
        }

        buf
    }

    #[test]
    fn parse_valid_packet() {
        let mut shapes = [0.0f32; BLENDSHAPE_COUNT];
        shapes[0] = 0.5; // EyeBlinkLeft
        shapes[17] = 0.8; // JawOpen
        shapes[51] = 1.0; // TongueOut

        let data = build_test_packet("iPhone12,1", "iPhoneFace", 42, &shapes);
        let packet = parse_packet(&data).unwrap();

        assert_eq!(packet.device_id, "iPhone12,1");
        assert_eq!(packet.subject_name, "iPhoneFace");
        assert_eq!(packet.frame_number, 42);
        assert!((packet.blendshapes[0] - 0.5).abs() < f32::EPSILON);
        assert!((packet.blendshapes[17] - 0.8).abs() < f32::EPSILON);
        assert!((packet.blendshapes[51] - 1.0).abs() < f32::EPSILON);
        assert_eq!(packet.fps_numerator, 30);
        assert_eq!(packet.fps_denominator, 1);
    }

    #[test]
    fn parse_bad_version() {
        let shapes = [0.0f32; BLENDSHAPE_COUNT];
        let mut data = build_test_packet("dev", "sub", 1, &shapes);
        data[0] = 5; // wrong version
        assert!(matches!(
            parse_packet(&data),
            Err(ParseError::BadVersion(5))
        ));
    }

    #[test]
    fn parse_truncated_packet() {
        assert!(matches!(
            parse_packet(&[6, 0, 0]),
            Err(ParseError::TooShort { .. })
        ));
    }

    #[test]
    fn parse_tail_fallback() {
        let mut shapes = [0.0f32; BLENDSHAPE_COUNT];
        shapes[0] = 0.25;
        shapes[60] = 0.75;

        let data = build_test_packet("dev", "sub", 1, &shapes);
        let parsed = parse_blendshapes_from_tail(&data).unwrap();

        assert!((parsed[0] - 0.25).abs() < f32::EPSILON);
        assert!((parsed[60] - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_empty_strings() {
        let shapes = [0.0f32; BLENDSHAPE_COUNT];
        let data = build_test_packet("", "", 0, &shapes);
        let packet = parse_packet(&data).unwrap();
        assert_eq!(packet.device_id, "");
        assert_eq!(packet.subject_name, "");
    }

    #[test]
    fn all_blendshapes_roundtrip() {
        let mut shapes = [0.0f32; BLENDSHAPE_COUNT];
        for (i, s) in shapes.iter_mut().enumerate() {
            *s = i as f32 / BLENDSHAPE_COUNT as f32;
        }
        let data = build_test_packet("test", "face", 100, &shapes);
        let packet = parse_packet(&data).unwrap();
        for i in 0..BLENDSHAPE_COUNT {
            assert!(
                (packet.blendshapes[i] - shapes[i]).abs() < f32::EPSILON,
                "mismatch at index {i}"
            );
        }
    }
}
