# LiteLink - Design

Lightweight Rust program that receives ARKit face tracking data from the
Unreal Engine LiveLink Face iOS app and forwards it to VRChat via OSC.

## Architecture

```
┌─────────────────┐     UDP :11111      ┌──────────────┐     OSC :9000      ┌────────┐
│ LiveLink Face   │ ──────────────────→  │ litelink     │ ──────────────────→ │ VRChat │
│ (iOS app)       │   big-endian f32s    │              │   /avatar/params   │        │
└─────────────────┘                      │              │                    └────────┘
                                         │  egui UI     │
                                         │  (optional)  │
                                         └──────────────┘
```

Three threads:
1. **UDP receiver** - listens on port 11111, parses LiveLink packets
2. **OSC sender** - sends parameter updates to VRChat at ~100Hz
3. **UI thread** - egui window for monitoring/overriding parameters

Shared state between threads via `Arc<Mutex<TrackingState>>` (or atomics for hot path).

## LiveLink UDP Packet Format

Port: **11111** (UDP). All multi-byte values are **big-endian**.

```
Byte 0:              u8    packet version (expect 6)
Bytes 1..5:          u32   device_id string length
Bytes 5..5+N:        [u8]  device_id (UTF-8)
Next 4 bytes:        u32   subject_name string length
Next M bytes:        [u8]  subject_name (UTF-8)
Next 4 bytes:        u32   frame number
Next 4 bytes:        f32   sub-frame
Next 4 bytes:        u32   fps numerator
Next 4 bytes:        u32   fps denominator
Next 1 byte:         u8    blendshape count (expect 61)
Next 244 bytes:      [f32; 61]  blendshapes (big-endian IEEE 754)
```

**Shortcut used by existing implementations:** just read the last 244 bytes as 61
big-endian f32s. The header is variable-length but the blendshapes are always the
trailing 244 bytes. We should parse the full header anyway for device identification
in the UI, but the shortcut is a valid fallback.

## ARKit Blendshape Indices (61 values)

### Eyes (0-13)
| Idx | Name              | Idx | Name               |
|-----|-------------------|-----|--------------------|
| 0   | EyeBlinkLeft      | 7   | EyeBlinkRight      |
| 1   | EyeLookDownLeft   | 8   | EyeLookDownRight   |
| 2   | EyeLookInLeft     | 9   | EyeLookInRight     |
| 3   | EyeLookOutLeft    | 10  | EyeLookOutRight    |
| 4   | EyeLookUpLeft     | 11  | EyeLookUpRight     |
| 5   | EyeSquintLeft     | 12  | EyeSquintRight     |
| 6   | EyeWideLeft       | 13  | EyeWideRight       |

### Jaw & Mouth (14-40)
| Idx | Name                | Idx | Name                 |
|-----|---------------------|-----|----------------------|
| 14  | JawForward          | 28  | MouthDimpleRight     |
| 15  | JawLeft             | 29  | MouthStretchLeft     |
| 16  | JawRight            | 30  | MouthStretchRight    |
| 17  | JawOpen             | 31  | MouthRollLower       |
| 18  | MouthClose          | 32  | MouthRollUpper       |
| 19  | MouthFunnel         | 33  | MouthShrugLower      |
| 20  | MouthPucker         | 34  | MouthShrugUpper      |
| 21  | MouthLeft           | 35  | MouthPressLeft       |
| 22  | MouthRight          | 36  | MouthPressRight      |
| 23  | MouthSmileLeft      | 37  | MouthLowerDownLeft   |
| 24  | MouthSmileRight     | 38  | MouthLowerDownRight  |
| 25  | MouthFrownLeft      | 39  | MouthUpperUpLeft     |
| 26  | MouthFrownRight     | 40  | MouthUpperUpRight    |
| 27  | MouthDimpleLeft     |     |                      |

### Brow (41-45)
| Idx | Name            |
|-----|-----------------|
| 41  | BrowDownLeft    |
| 42  | BrowDownRight   |
| 43  | BrowInnerUp     |
| 44  | BrowOuterUpLeft |
| 45  | BrowOuterUpRight|

### Cheek, Nose, Tongue (46-51)
| Idx | Name            |
|-----|-----------------|
| 46  | CheekPuff       |
| 47  | CheekSquintLeft |
| 48  | CheekSquintRight|
| 49  | NoseSneerLeft   |
| 50  | NoseSneerRight  |
| 51  | TongueOut       |

### Head & Eye Pose (52-60) - values in RADIANS, not 0-1
| Idx | Name          | Idx | Name           |
|-----|---------------|-----|----------------|
| 52  | HeadYaw       | 55  | EyeYawLeft     |
| 53  | HeadPitch     | 56  | EyePitchLeft   |
| 54  | HeadRoll      | 57  | EyeRollLeft    |
|     |               | 58  | EyeYawRight    |
|     |               | 59  | EyePitchRight  |
|     |               | 60  | EyeRollRight   |

## ARKit → VRChat OSC Mapping

### Direct Expression Parameters

Each maps to `/avatar/parameters/v2/{Name}` as a float (0.0-1.0):

| ARKit Index | ARKit Name       | OSC Parameter(s)                              |
|-------------|------------------|-----------------------------------------------|
| 0           | EyeBlinkLeft     | (eye openness: 1.0 - value)                   |
| 5           | EyeSquintLeft    | `EyeSquintLeft`                               |
| 6           | EyeWideLeft      | `EyeWideLeft`                                 |
| 7           | EyeBlinkRight    | (eye openness: 1.0 - value)                   |
| 12          | EyeSquintRight   | `EyeSquintRight`                              |
| 13          | EyeWideRight     | `EyeWideRight`                                |
| 14          | JawForward       | `JawForward`                                  |
| 15          | JawLeft          | `JawLeft`                                     |
| 16          | JawRight         | `JawRight`                                    |
| 17          | JawOpen          | `JawOpen`                                     |
| 18          | MouthClose       | `MouthClosed`                                 |
| 19          | MouthFunnel      | `LipFunnelUpperLeft/Right`, `LipFunnelLowerLeft/Right` |
| 20          | MouthPucker      | `LipPuckerUpperLeft/Right`, `LipPuckerLowerLeft/Right` |
| 21          | MouthLeft        | `MouthUpperLeft`, `MouthLowerLeft`            |
| 22          | MouthRight       | `MouthUpperRight`, `MouthLowerRight`          |
| 23          | MouthSmileLeft   | `MouthCornerPullLeft`, `MouthCornerSlantLeft` |
| 24          | MouthSmileRight  | `MouthCornerPullRight`, `MouthCornerSlantRight`|
| 25          | MouthFrownLeft   | `MouthFrownLeft`                              |
| 26          | MouthFrownRight  | `MouthFrownRight`                             |
| 27          | MouthDimpleLeft  | `MouthDimpleLeft`                             |
| 28          | MouthDimpleRight | `MouthDimpleRight`                            |
| 29          | MouthStretchLeft | `MouthStretchLeft`                            |
| 30          | MouthStretchRight| `MouthStretchRight`                           |
| 31          | MouthRollLower   | `LipSuckLowerLeft`, `LipSuckLowerRight`       |
| 32          | MouthRollUpper   | `LipSuckUpperLeft`, `LipSuckUpperRight`       |
| 33          | MouthShrugLower  | `MouthRaiserLower`                            |
| 34          | MouthShrugUpper  | `MouthRaiserUpper`                            |
| 35          | MouthPressLeft   | `MouthPressLeft`                              |
| 36          | MouthPressRight  | `MouthPressRight`                             |
| 37          | MouthLowerDownLeft  | `MouthLowerDownLeft`                       |
| 38          | MouthLowerDownRight | `MouthLowerDownRight`                      |
| 39          | MouthUpperUpLeft | `MouthUpperUpLeft`                            |
| 40          | MouthUpperUpRight| `MouthUpperUpRight`                           |
| 41          | BrowDownLeft     | `BrowLowererLeft`, `BrowPinchLeft`            |
| 42          | BrowDownRight    | `BrowLowererRight`, `BrowPinchRight`          |
| 43          | BrowInnerUp      | `BrowInnerUpLeft`, `BrowInnerUpRight`         |
| 44          | BrowOuterUpLeft  | `BrowOuterUpLeft`                             |
| 45          | BrowOuterUpRight | `BrowOuterUpRight`                            |
| 46          | CheekPuff        | `CheekPuffLeft`, `CheekPuffRight`             |
| 47          | CheekSquintLeft  | `CheekSquintLeft`                             |
| 48          | CheekSquintRight | `CheekSquintRight`                            |
| 49          | NoseSneerLeft    | `NoseSneerLeft`                               |
| 50          | NoseSneerRight   | `NoseSneerRight`                              |
| 51          | TongueOut        | `TongueOut`                                   |

### Derived/Combined Parameters

These are computed from the base expressions and sent as additional OSC params:

| OSC Parameter         | Computation                                           |
|-----------------------|-------------------------------------------------------|
| `EyeOpenLeft`         | 1.0 - EyeBlinkLeft                                   |
| `EyeOpenRight`        | 1.0 - EyeBlinkRight                                  |
| `EyeOpen`             | avg(EyeOpenLeft, EyeOpenRight)                        |
| `EyeClosedLeft`       | EyeBlinkLeft                                          |
| `EyeClosedRight`      | EyeBlinkRight                                         |
| `EyeClosed`           | avg(EyeBlinkLeft, EyeBlinkRight)                      |
| `EyeLeftX`            | EyeYawLeft (radians, from idx 55)                     |
| `EyeLeftY`            | -EyePitchLeft (negated, from idx 56)                  |
| `EyeRightX`           | EyeYawRight (radians, from idx 58)                    |
| `EyeRightY`           | -EyePitchRight (negated, from idx 59)                 |
| `EyeSquint`           | max(EyeSquintLeft, EyeSquintRight)                    |
| `EyeWide`             | max(EyeWideLeft, EyeWideRight)                        |
| `JawX`                | JawRight - JawLeft                                    |
| `JawZ`                | JawForward                                            |
| `MouthX`              | (MouthRight - MouthLeft)                              |
| `SmileFrown`          | avg(SmileL, SmileR) - avg(FrownL, FrownR)             |
| `SmileFrownLeft`      | MouthSmileLeft - MouthFrownLeft                       |
| `SmileFrownRight`     | MouthSmileRight - MouthFrownRight                     |
| `LipFunnel`           | MouthFunnel                                           |
| `LipPucker`           | MouthPucker                                           |
| `LipSuck`             | avg(MouthRollLower, MouthRollUpper)                   |
| `CheekPuffSuck`       | CheekPuff (no suck from ARKit)                        |
| `NoseSneer`           | avg(NoseSneerLeft, NoseSneerRight)                    |
| `BrowInnerUp`         | BrowInnerUp                                           |
| `BrowOuterUp`         | avg(BrowOuterUpLeft, BrowOuterUpRight)                |
| `BrowDown`            | avg(BrowDownLeft, BrowDownRight)                      |
| `MouthOpen`           | avg(MouthLowerDown*, MouthUpperUp*) * 0.25            |

### Status Parameters (no v2/ prefix)
| OSC Parameter                | Type | Value                        |
|------------------------------|------|------------------------------|
| `EyeTrackingActive`         | bool | true while receiving packets |
| `ExpressionTrackingActive`  | bool | true while receiving packets |
| `LipTrackingActive`         | bool | true while receiving packets |

## OSC Transport

- **Destination**: `127.0.0.1:9000` (VRChat default OSC input)
- **Protocol**: UDP
- **Message format**: OSC 1.0 (use `rosc` crate)
- **Address format**: `/avatar/parameters/v2/{ParameterName}` for face params
- **Update rate**: ~100Hz (10ms between sends)
- **Bundle**: Multiple parameters per OSC bundle for efficiency

## UI (egui)

Minimal window showing:
- Connection status (receiving packets? from which device?)
- Live blendshape values as horizontal bars
- Per-parameter override sliders (click to override, click again to release)
- OSC target IP:port configuration
- LiveLink listen port configuration

## Module Structure

```
src/
  main.rs          - entry point, spawns threads, runs egui
  livelink.rs      - UDP packet parsing
  osc.rs           - OSC message construction and sending
  mapping.rs       - ARKit index → OSC parameter mapping tables
  state.rs         - shared tracking state
```

## MVP Scope

Phase 1 (today):
- [x] Project setup (Cargo, flake.nix)
- [ ] LiveLink UDP parser (full header + 61 blendshapes)
- [ ] ARKit → OSC mapping table
- [ ] OSC sender to VRChat
- [ ] Headless mode (no UI, just bridge)

Phase 2:
- [ ] egui monitoring window
- [ ] Parameter override sliders
- [ ] Config persistence

Phase 3:
- [ ] Nix package build
- [ ] Binary releases (GitHub Actions)
