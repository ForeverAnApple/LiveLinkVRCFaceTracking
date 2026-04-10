use crate::state::BLENDSHAPE_COUNT;

/// An OSC parameter with its address and computed value.
pub struct OscParam {
    /// Full OSC address, e.g. "/avatar/parameters/v2/JawOpen"
    pub address: &'static str,
    /// Parameter value.
    pub value: OscValue,
}

pub enum OscValue {
    Float(f32),
    Bool(bool),
}

// ARKit blendshape indices for readability.
mod idx {
    pub const EYE_BLINK_LEFT: usize = 0;
    pub const EYE_LOOK_DOWN_LEFT: usize = 1;
    pub const EYE_LOOK_IN_LEFT: usize = 2;
    pub const EYE_LOOK_OUT_LEFT: usize = 3;
    pub const EYE_LOOK_UP_LEFT: usize = 4;
    pub const EYE_SQUINT_LEFT: usize = 5;
    pub const EYE_WIDE_LEFT: usize = 6;
    pub const EYE_BLINK_RIGHT: usize = 7;
    pub const EYE_LOOK_DOWN_RIGHT: usize = 8;
    pub const EYE_LOOK_IN_RIGHT: usize = 9;
    pub const EYE_LOOK_OUT_RIGHT: usize = 10;
    pub const EYE_LOOK_UP_RIGHT: usize = 11;
    pub const EYE_SQUINT_RIGHT: usize = 12;
    pub const EYE_WIDE_RIGHT: usize = 13;
    pub const JAW_FORWARD: usize = 14;
    pub const JAW_LEFT: usize = 15;
    pub const JAW_RIGHT: usize = 16;
    pub const JAW_OPEN: usize = 17;
    pub const MOUTH_CLOSE: usize = 18;
    pub const MOUTH_FUNNEL: usize = 19;
    pub const MOUTH_PUCKER: usize = 20;
    pub const MOUTH_LEFT: usize = 21;
    pub const MOUTH_RIGHT: usize = 22;
    pub const MOUTH_SMILE_LEFT: usize = 23;
    pub const MOUTH_SMILE_RIGHT: usize = 24;
    pub const MOUTH_FROWN_LEFT: usize = 25;
    pub const MOUTH_FROWN_RIGHT: usize = 26;
    pub const MOUTH_DIMPLE_LEFT: usize = 27;
    pub const MOUTH_DIMPLE_RIGHT: usize = 28;
    pub const MOUTH_STRETCH_LEFT: usize = 29;
    pub const MOUTH_STRETCH_RIGHT: usize = 30;
    pub const MOUTH_ROLL_LOWER: usize = 31;
    pub const MOUTH_ROLL_UPPER: usize = 32;
    pub const MOUTH_SHRUG_LOWER: usize = 33;
    pub const MOUTH_SHRUG_UPPER: usize = 34;
    pub const MOUTH_PRESS_LEFT: usize = 35;
    pub const MOUTH_PRESS_RIGHT: usize = 36;
    pub const MOUTH_LOWER_DOWN_LEFT: usize = 37;
    pub const MOUTH_LOWER_DOWN_RIGHT: usize = 38;
    pub const MOUTH_UPPER_UP_LEFT: usize = 39;
    pub const MOUTH_UPPER_UP_RIGHT: usize = 40;
    pub const BROW_DOWN_LEFT: usize = 41;
    pub const BROW_DOWN_RIGHT: usize = 42;
    pub const BROW_INNER_UP: usize = 43;
    pub const BROW_OUTER_UP_LEFT: usize = 44;
    pub const BROW_OUTER_UP_RIGHT: usize = 45;
    pub const CHEEK_PUFF: usize = 46;
    pub const CHEEK_SQUINT_LEFT: usize = 47;
    pub const CHEEK_SQUINT_RIGHT: usize = 48;
    pub const NOSE_SNEER_LEFT: usize = 49;
    pub const NOSE_SNEER_RIGHT: usize = 50;
    pub const TONGUE_OUT: usize = 51;
    // Head pose (radians)
    pub const HEAD_YAW: usize = 52;
    pub const HEAD_PITCH: usize = 53;
    pub const HEAD_ROLL: usize = 54;
    // Eye gaze (radians)
    pub const EYE_YAW_LEFT: usize = 55;
    pub const EYE_PITCH_LEFT: usize = 56;
    pub const EYE_YAW_RIGHT: usize = 58;
    pub const EYE_PITCH_RIGHT: usize = 59;
}

/// Clamp a blendshape value to 0.0-1.0 (ARKit can occasionally exceed this range).
fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

fn float(address: &'static str, value: f32) -> OscParam {
    OscParam {
        address,
        value: OscValue::Float(value),
    }
}

fn status(address: &'static str, active: bool) -> OscParam {
    OscParam {
        address,
        value: OscValue::Bool(active),
    }
}

/// Build the full list of OSC parameters from 61 ARKit blendshapes.
/// Uses static string addresses to avoid allocations in the hot path.
pub fn map_blendshapes(bs: &[f32; BLENDSHAPE_COUNT], connected: bool) -> Vec<OscParam> {
    let mut params = Vec::with_capacity(128);

    // --- Direct 1:1 expression parameters ---
    let direct_singles: &[(usize, &'static str)] = &[
        (idx::EYE_SQUINT_LEFT, "/avatar/parameters/v2/EyeSquintLeft"),
        (idx::EYE_WIDE_LEFT, "/avatar/parameters/v2/EyeWideLeft"),
        (idx::EYE_SQUINT_RIGHT, "/avatar/parameters/v2/EyeSquintRight"),
        (idx::EYE_WIDE_RIGHT, "/avatar/parameters/v2/EyeWideRight"),
        (idx::JAW_FORWARD, "/avatar/parameters/v2/JawForward"),
        (idx::JAW_LEFT, "/avatar/parameters/v2/JawLeft"),
        (idx::JAW_RIGHT, "/avatar/parameters/v2/JawRight"),
        (idx::JAW_OPEN, "/avatar/parameters/v2/JawOpen"),
        (idx::MOUTH_CLOSE, "/avatar/parameters/v2/MouthClosed"),
        (idx::MOUTH_FROWN_LEFT, "/avatar/parameters/v2/MouthFrownLeft"),
        (idx::MOUTH_FROWN_RIGHT, "/avatar/parameters/v2/MouthFrownRight"),
        (idx::MOUTH_DIMPLE_LEFT, "/avatar/parameters/v2/MouthDimpleLeft"),
        (idx::MOUTH_DIMPLE_RIGHT, "/avatar/parameters/v2/MouthDimpleRight"),
        (idx::MOUTH_STRETCH_LEFT, "/avatar/parameters/v2/MouthStretchLeft"),
        (idx::MOUTH_STRETCH_RIGHT, "/avatar/parameters/v2/MouthStretchRight"),
        (idx::MOUTH_SHRUG_LOWER, "/avatar/parameters/v2/MouthRaiserLower"),
        (idx::MOUTH_SHRUG_UPPER, "/avatar/parameters/v2/MouthRaiserUpper"),
        (idx::MOUTH_PRESS_LEFT, "/avatar/parameters/v2/MouthPressLeft"),
        (idx::MOUTH_PRESS_RIGHT, "/avatar/parameters/v2/MouthPressRight"),
        (idx::MOUTH_LOWER_DOWN_LEFT, "/avatar/parameters/v2/MouthLowerDownLeft"),
        (idx::MOUTH_LOWER_DOWN_RIGHT, "/avatar/parameters/v2/MouthLowerDownRight"),
        (idx::MOUTH_UPPER_UP_LEFT, "/avatar/parameters/v2/MouthUpperUpLeft"),
        (idx::MOUTH_UPPER_UP_RIGHT, "/avatar/parameters/v2/MouthUpperUpRight"),
        (idx::BROW_OUTER_UP_LEFT, "/avatar/parameters/v2/BrowOuterUpLeft"),
        (idx::BROW_OUTER_UP_RIGHT, "/avatar/parameters/v2/BrowOuterUpRight"),
        (idx::CHEEK_SQUINT_LEFT, "/avatar/parameters/v2/CheekSquintLeft"),
        (idx::CHEEK_SQUINT_RIGHT, "/avatar/parameters/v2/CheekSquintRight"),
        (idx::NOSE_SNEER_LEFT, "/avatar/parameters/v2/NoseSneerLeft"),
        (idx::NOSE_SNEER_RIGHT, "/avatar/parameters/v2/NoseSneerRight"),
        (idx::TONGUE_OUT, "/avatar/parameters/v2/TongueOut"),
    ];

    for &(i, addr) in direct_singles {
        params.push(float(addr, clamp01(bs[i])));
    }

    // --- Eye look direction (indices 1-4, 8-11) ---
    params.push(float("/avatar/parameters/v2/EyeLookDownLeft", clamp01(bs[idx::EYE_LOOK_DOWN_LEFT])));
    params.push(float("/avatar/parameters/v2/EyeLookInLeft", clamp01(bs[idx::EYE_LOOK_IN_LEFT])));
    params.push(float("/avatar/parameters/v2/EyeLookOutLeft", clamp01(bs[idx::EYE_LOOK_OUT_LEFT])));
    params.push(float("/avatar/parameters/v2/EyeLookUpLeft", clamp01(bs[idx::EYE_LOOK_UP_LEFT])));
    params.push(float("/avatar/parameters/v2/EyeLookDownRight", clamp01(bs[idx::EYE_LOOK_DOWN_RIGHT])));
    params.push(float("/avatar/parameters/v2/EyeLookInRight", clamp01(bs[idx::EYE_LOOK_IN_RIGHT])));
    params.push(float("/avatar/parameters/v2/EyeLookOutRight", clamp01(bs[idx::EYE_LOOK_OUT_RIGHT])));
    params.push(float("/avatar/parameters/v2/EyeLookUpRight", clamp01(bs[idx::EYE_LOOK_UP_RIGHT])));

    // --- One-to-many direct mappings ---

    // MouthFunnel → LipFunnelUpper/LowerLeft/Right
    let funnel = clamp01(bs[idx::MOUTH_FUNNEL]);
    params.push(float("/avatar/parameters/v2/LipFunnelUpperLeft", funnel));
    params.push(float("/avatar/parameters/v2/LipFunnelUpperRight", funnel));
    params.push(float("/avatar/parameters/v2/LipFunnelLowerLeft", funnel));
    params.push(float("/avatar/parameters/v2/LipFunnelLowerRight", funnel));

    // MouthPucker → LipPuckerUpper/LowerLeft/Right
    let pucker = clamp01(bs[idx::MOUTH_PUCKER]);
    params.push(float("/avatar/parameters/v2/LipPuckerUpperLeft", pucker));
    params.push(float("/avatar/parameters/v2/LipPuckerUpperRight", pucker));
    params.push(float("/avatar/parameters/v2/LipPuckerLowerLeft", pucker));
    params.push(float("/avatar/parameters/v2/LipPuckerLowerRight", pucker));

    // MouthLeft → MouthUpperLeft + MouthLowerLeft
    let mouth_left = clamp01(bs[idx::MOUTH_LEFT]);
    params.push(float("/avatar/parameters/v2/MouthUpperLeft", mouth_left));
    params.push(float("/avatar/parameters/v2/MouthLowerLeft", mouth_left));

    // MouthRight → MouthUpperRight + MouthLowerRight
    let mouth_right = clamp01(bs[idx::MOUTH_RIGHT]);
    params.push(float("/avatar/parameters/v2/MouthUpperRight", mouth_right));
    params.push(float("/avatar/parameters/v2/MouthLowerRight", mouth_right));

    // MouthSmileLeft → MouthCornerPullLeft + MouthCornerSlantLeft
    let smile_l = clamp01(bs[idx::MOUTH_SMILE_LEFT]);
    params.push(float("/avatar/parameters/v2/MouthCornerPullLeft", smile_l));
    params.push(float("/avatar/parameters/v2/MouthCornerSlantLeft", smile_l));

    // MouthSmileRight → MouthCornerPullRight + MouthCornerSlantRight
    let smile_r = clamp01(bs[idx::MOUTH_SMILE_RIGHT]);
    params.push(float("/avatar/parameters/v2/MouthCornerPullRight", smile_r));
    params.push(float("/avatar/parameters/v2/MouthCornerSlantRight", smile_r));

    // MouthRollLower → LipSuckLowerLeft + LipSuckLowerRight
    let roll_lower = clamp01(bs[idx::MOUTH_ROLL_LOWER]);
    params.push(float("/avatar/parameters/v2/LipSuckLowerLeft", roll_lower));
    params.push(float("/avatar/parameters/v2/LipSuckLowerRight", roll_lower));

    // MouthRollUpper → LipSuckUpperLeft + LipSuckUpperRight
    let roll_upper = clamp01(bs[idx::MOUTH_ROLL_UPPER]);
    params.push(float("/avatar/parameters/v2/LipSuckUpperLeft", roll_upper));
    params.push(float("/avatar/parameters/v2/LipSuckUpperRight", roll_upper));

    // BrowDownLeft → BrowLowererLeft + BrowPinchLeft
    let brow_down_l = clamp01(bs[idx::BROW_DOWN_LEFT]);
    params.push(float("/avatar/parameters/v2/BrowLowererLeft", brow_down_l));
    params.push(float("/avatar/parameters/v2/BrowPinchLeft", brow_down_l));

    // BrowDownRight → BrowLowererRight + BrowPinchRight
    let brow_down_r = clamp01(bs[idx::BROW_DOWN_RIGHT]);
    params.push(float("/avatar/parameters/v2/BrowLowererRight", brow_down_r));
    params.push(float("/avatar/parameters/v2/BrowPinchRight", brow_down_r));

    // BrowInnerUp → BrowInnerUpLeft + BrowInnerUpRight
    let brow_inner = clamp01(bs[idx::BROW_INNER_UP]);
    params.push(float("/avatar/parameters/v2/BrowInnerUpLeft", brow_inner));
    params.push(float("/avatar/parameters/v2/BrowInnerUpRight", brow_inner));

    // CheekPuff → CheekPuffLeft + CheekPuffRight
    let cheek_puff = clamp01(bs[idx::CHEEK_PUFF]);
    params.push(float("/avatar/parameters/v2/CheekPuffLeft", cheek_puff));
    params.push(float("/avatar/parameters/v2/CheekPuffRight", cheek_puff));

    // --- Derived/combined parameters ---
    let eye_blink_l = clamp01(bs[idx::EYE_BLINK_LEFT]);
    let eye_blink_r = clamp01(bs[idx::EYE_BLINK_RIGHT]);
    let eye_open_l = 1.0 - eye_blink_l;
    let eye_open_r = 1.0 - eye_blink_r;

    params.push(float("/avatar/parameters/v2/EyeOpenLeft", eye_open_l));
    params.push(float("/avatar/parameters/v2/EyeOpenRight", eye_open_r));
    params.push(float("/avatar/parameters/v2/EyeOpen", (eye_open_l + eye_open_r) * 0.5));
    params.push(float("/avatar/parameters/v2/EyeClosedLeft", eye_blink_l));
    params.push(float("/avatar/parameters/v2/EyeClosedRight", eye_blink_r));
    params.push(float("/avatar/parameters/v2/EyeClosed", (eye_blink_l + eye_blink_r) * 0.5));

    // Eye gaze (radians, not clamped to 0-1)
    params.push(float("/avatar/parameters/v2/EyeLeftX", bs[idx::EYE_YAW_LEFT]));
    params.push(float("/avatar/parameters/v2/EyeLeftY", -bs[idx::EYE_PITCH_LEFT]));
    params.push(float("/avatar/parameters/v2/EyeRightX", bs[idx::EYE_YAW_RIGHT]));
    params.push(float("/avatar/parameters/v2/EyeRightY", -bs[idx::EYE_PITCH_RIGHT]));

    // Head pose (radians, not clamped)
    params.push(float("/avatar/parameters/v2/HeadYaw", bs[idx::HEAD_YAW]));
    params.push(float("/avatar/parameters/v2/HeadPitch", bs[idx::HEAD_PITCH]));
    params.push(float("/avatar/parameters/v2/HeadRoll", bs[idx::HEAD_ROLL]));

    // Aggregates
    let squint_l = clamp01(bs[idx::EYE_SQUINT_LEFT]);
    let squint_r = clamp01(bs[idx::EYE_SQUINT_RIGHT]);
    params.push(float("/avatar/parameters/v2/EyeSquint", squint_l.max(squint_r)));

    let wide_l = clamp01(bs[idx::EYE_WIDE_LEFT]);
    let wide_r = clamp01(bs[idx::EYE_WIDE_RIGHT]);
    params.push(float("/avatar/parameters/v2/EyeWide", wide_l.max(wide_r)));

    // Jaw derived
    params.push(float("/avatar/parameters/v2/JawX", clamp01(bs[idx::JAW_RIGHT]) - clamp01(bs[idx::JAW_LEFT])));
    params.push(float("/avatar/parameters/v2/JawZ", clamp01(bs[idx::JAW_FORWARD])));

    // Mouth derived
    params.push(float("/avatar/parameters/v2/MouthX", mouth_right - mouth_left));

    let frown_l = clamp01(bs[idx::MOUTH_FROWN_LEFT]);
    let frown_r = clamp01(bs[idx::MOUTH_FROWN_RIGHT]);
    let smile_avg = (smile_l + smile_r) * 0.5;
    let frown_avg = (frown_l + frown_r) * 0.5;
    params.push(float("/avatar/parameters/v2/SmileFrown", smile_avg - frown_avg));
    params.push(float("/avatar/parameters/v2/SmileFrownLeft", smile_l - frown_l));
    params.push(float("/avatar/parameters/v2/SmileFrownRight", smile_r - frown_r));

    params.push(float("/avatar/parameters/v2/LipFunnel", funnel));
    params.push(float("/avatar/parameters/v2/LipPucker", pucker));
    params.push(float("/avatar/parameters/v2/LipSuck", (roll_lower + roll_upper) * 0.5));

    params.push(float("/avatar/parameters/v2/CheekPuffSuck", cheek_puff));

    let sneer_l = clamp01(bs[idx::NOSE_SNEER_LEFT]);
    let sneer_r = clamp01(bs[idx::NOSE_SNEER_RIGHT]);
    params.push(float("/avatar/parameters/v2/NoseSneer", (sneer_l + sneer_r) * 0.5));

    params.push(float("/avatar/parameters/v2/BrowInnerUp", brow_inner));
    params.push(float("/avatar/parameters/v2/BrowOuterUp", (clamp01(bs[idx::BROW_OUTER_UP_LEFT]) + clamp01(bs[idx::BROW_OUTER_UP_RIGHT])) * 0.5));
    params.push(float("/avatar/parameters/v2/BrowDown", (brow_down_l + brow_down_r) * 0.5));

    // MouthOpen: avg of lower-down and upper-up * 0.25
    let mouth_open = (clamp01(bs[idx::MOUTH_LOWER_DOWN_LEFT])
        + clamp01(bs[idx::MOUTH_LOWER_DOWN_RIGHT])
        + clamp01(bs[idx::MOUTH_UPPER_UP_LEFT])
        + clamp01(bs[idx::MOUTH_UPPER_UP_RIGHT]))
        * 0.25;
    params.push(float("/avatar/parameters/v2/MouthOpen", mouth_open));

    // --- Status parameters (no v2/ prefix) ---
    params.push(status("/avatar/parameters/EyeTrackingActive", connected));
    params.push(status("/avatar/parameters/ExpressionTrackingActive", connected));
    params.push(status("/avatar/parameters/LipTrackingActive", connected));

    params
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_float(params: &[OscParam], suffix: &str) -> f32 {
        params
            .iter()
            .find(|p| p.address.ends_with(suffix))
            .map(|p| match p.value {
                OscValue::Float(v) => v,
                _ => panic!("expected float for {suffix}"),
            })
            .unwrap_or_else(|| panic!("param {suffix} not found"))
    }

    #[test]
    fn mapping_produces_params() {
        let bs = [0.0f32; BLENDSHAPE_COUNT];
        let params = map_blendshapes(&bs, true);
        // Should have direct + eye look + one-to-many + derived + status
        assert!(params.len() > 90, "expected many params, got {}", params.len());
    }

    #[test]
    fn eye_open_inverse_of_blink() {
        let mut bs = [0.0f32; BLENDSHAPE_COUNT];
        bs[idx::EYE_BLINK_LEFT] = 0.7;
        bs[idx::EYE_BLINK_RIGHT] = 0.3;
        let params = map_blendshapes(&bs, true);

        assert!((find_float(&params, "EyeOpenLeft") - 0.3).abs() < f32::EPSILON);
        assert!((find_float(&params, "EyeOpenRight") - 0.7).abs() < f32::EPSILON);
        assert!((find_float(&params, "EyeClosedLeft") - 0.7).abs() < f32::EPSILON);
        assert!((find_float(&params, "EyeClosedRight") - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn jaw_x_is_right_minus_left() {
        let mut bs = [0.0f32; BLENDSHAPE_COUNT];
        bs[idx::JAW_RIGHT] = 0.8;
        bs[idx::JAW_LEFT] = 0.3;
        let params = map_blendshapes(&bs, true);
        assert!((find_float(&params, "JawX") - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn status_params_use_no_v2_prefix() {
        let bs = [0.0f32; BLENDSHAPE_COUNT];
        let params = map_blendshapes(&bs, true);
        let s = params.iter().find(|p| p.address.contains("EyeTrackingActive")).unwrap();
        assert_eq!(s.address, "/avatar/parameters/EyeTrackingActive");
        assert!(matches!(s.value, OscValue::Bool(true)));
    }

    #[test]
    fn status_params_false_when_disconnected() {
        let bs = [0.0f32; BLENDSHAPE_COUNT];
        let params = map_blendshapes(&bs, false);
        let s = params.iter().find(|p| p.address.contains("LipTrackingActive")).unwrap();
        assert!(matches!(s.value, OscValue::Bool(false)));
    }

    #[test]
    fn mouth_funnel_fans_out_to_four() {
        let mut bs = [0.0f32; BLENDSHAPE_COUNT];
        bs[idx::MOUTH_FUNNEL] = 0.6;
        let params = map_blendshapes(&bs, true);
        let funnel_params: Vec<_> = params
            .iter()
            .filter(|p| {
                let name = p.address.rsplit('/').next().unwrap_or("");
                name.starts_with("LipFunnel") && name != "LipFunnel"
            })
            .collect();
        assert_eq!(funnel_params.len(), 4);
        for p in &funnel_params {
            match p.value {
                OscValue::Float(v) => assert!((v - 0.6).abs() < f32::EPSILON),
                _ => panic!("expected float"),
            }
        }
    }

    #[test]
    fn smile_frown_derived() {
        let mut bs = [0.0f32; BLENDSHAPE_COUNT];
        bs[idx::MOUTH_SMILE_LEFT] = 0.8;
        bs[idx::MOUTH_SMILE_RIGHT] = 0.6;
        bs[idx::MOUTH_FROWN_LEFT] = 0.1;
        bs[idx::MOUTH_FROWN_RIGHT] = 0.2;
        let params = map_blendshapes(&bs, true);

        // SmileFrown = avg(0.8, 0.6) - avg(0.1, 0.2) = 0.7 - 0.15 = 0.55
        assert!((find_float(&params, "SmileFrown") - 0.55).abs() < 1e-6);
        assert!((find_float(&params, "SmileFrownLeft") - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn eye_gaze_negated_pitch() {
        let mut bs = [0.0f32; BLENDSHAPE_COUNT];
        bs[idx::EYE_PITCH_LEFT] = 0.3;
        bs[idx::EYE_PITCH_RIGHT] = -0.2;
        let params = map_blendshapes(&bs, true);

        assert!((find_float(&params, "EyeLeftY") - (-0.3)).abs() < f32::EPSILON);
        assert!((find_float(&params, "EyeRightY") - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn clamping_caps_above_one() {
        let mut bs = [0.0f32; BLENDSHAPE_COUNT];
        bs[idx::JAW_OPEN] = 1.05; // ARKit can exceed 1.0
        let params = map_blendshapes(&bs, true);
        assert!((find_float(&params, "/v2/JawOpen") - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn clamping_floors_below_zero() {
        let mut bs = [0.0f32; BLENDSHAPE_COUNT];
        bs[idx::JAW_OPEN] = -0.01;
        let params = map_blendshapes(&bs, true);
        assert!((find_float(&params, "/v2/JawOpen")).abs() < f32::EPSILON);
    }

    #[test]
    fn eye_look_params_present() {
        let mut bs = [0.0f32; BLENDSHAPE_COUNT];
        bs[idx::EYE_LOOK_DOWN_LEFT] = 0.4;
        bs[idx::EYE_LOOK_IN_RIGHT] = 0.6;
        let params = map_blendshapes(&bs, true);

        assert!((find_float(&params, "EyeLookDownLeft") - 0.4).abs() < f32::EPSILON);
        assert!((find_float(&params, "EyeLookInRight") - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn head_pose_params_present() {
        let mut bs = [0.0f32; BLENDSHAPE_COUNT];
        bs[idx::HEAD_YAW] = 0.3;
        bs[idx::HEAD_PITCH] = -0.2;
        bs[idx::HEAD_ROLL] = 0.1;
        let params = map_blendshapes(&bs, true);

        assert!((find_float(&params, "HeadYaw") - 0.3).abs() < f32::EPSILON);
        assert!((find_float(&params, "HeadPitch") - (-0.2)).abs() < f32::EPSILON);
        assert!((find_float(&params, "HeadRoll") - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn all_addresses_are_static_str() {
        let bs = [0.5f32; BLENDSHAPE_COUNT];
        let params = map_blendshapes(&bs, true);
        for p in &params {
            assert!(p.address.starts_with("/avatar/parameters/"));
        }
    }
}
