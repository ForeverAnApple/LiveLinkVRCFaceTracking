# LiteLink

Fast & lightweight LiveLink Face → VRChat OSC bridge. Single binary, cross-platform, no SteamVR required.

![Demo](assets/demo.gif)

## Install

Grab a binary from [Releases](https://github.com/ForeverAnApple/LiveLinkVRCFaceTracking/releases):

- `litelink-linux-x86_64` / `litelink-windows-x86_64.exe`
- `litelink-gui-*` variants include a status window

Or build from source (Rust 1.80+):

```bash
cargo build --release                  # with GUI (default)
cargo build --release --no-default-features  # CLI only
```

## Usage

1. Install [LiveLink Face](https://apps.apple.com/us/app/live-link-face/id1495370836) on your iPhone
2. Set target IP to your PC's IP, port `11111`
3. Enable OSC in VRChat: **Action Menu > Options > OSC > Enabled**
4. Run the bridge:

```bash
./litelink
```

Listens on UDP `:11111`, sends OSC to `127.0.0.1:9000`.

### Options

```
--listen-port <PORT>    LiveLink UDP port (default: 11111)
--osc-target <ADDR>     VRChat OSC target (default: 127.0.0.1:9000)
--prefix <PREFIX>       OSC parameter prefix (default: /avatar/parameters/FT/v2)
--send-rate <HZ>        OSC send rate in Hz (default: 60)
--timeout <SECS>        Connection timeout in seconds (default: 2.0)
--headless              Run without GUI (GUI build only)
--benchmark <SECS>      Performance benchmark for N seconds
```

### Avatar compatibility

Default prefix (`/avatar/parameters/FT/v2`) works with [VRCFaceTracking](https://github.com/benaclejames/VRCFaceTracking) Unified Expressions avatars. Change `--prefix` if your avatar uses different parameter names.

