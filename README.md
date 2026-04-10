# livelink-vrcft

Lightweight LiveLink Face to VRChat OSC bridge in Rust. Receives ARKit face tracking data from the [LiveLink Face](https://apps.apple.com/us/app/live-link-face/id1495370836) iOS app over UDP and forwards it to VRChat as OSC parameters.

## Usage

```bash
cargo run
```

By default, listens for LiveLink on UDP port 11111 and sends OSC to `127.0.0.1:9000`.

### Options

```
--listen-port <PORT>    LiveLink UDP port (default: 11111)
--osc-target <ADDR>     VRChat OSC address (default: 127.0.0.1:9000)
--prefix <PREFIX>       OSC parameter prefix (default: /avatar/parameters/FT/v2)
--send-rate <HZ>        OSC send rate in Hz (default: 60)
--timeout <SECS>        Connection timeout in seconds (default: 2.0)
```

### iPhone Setup

1. Install [LiveLink Face](https://apps.apple.com/us/app/live-link-face/id1495370836) on your iPhone
2. In the app settings, set the target IP to your PC's local IP address and port to 11111
3. Run `cargo run` on your PC
4. Start streaming in the app

### Avatar Compatibility

The `--prefix` flag controls the OSC parameter path prefix. The default (`/avatar/parameters/FT/v2`) works with avatars set up for [VRCFaceTracking](https://github.com/benaclejames/VRCFaceTracking). If your avatar uses a different naming convention, adjust the prefix accordingly.

## Building

Requires Rust 1.80+. Uses a Nix flake for development:

```bash
# With nix + direnv (recommended)
direnv allow

# Or manually
nix develop

# Build
cargo build --release

# Test
cargo nextest run
```

## Architecture

- **livelink.rs** -- UDP packet parser for the LiveLink Face protocol (61 ARKit blendshapes, big-endian)
- **mapping.rs** -- ARKit blendshape to VRChat Unified Expressions mapping with clamping
- **osc.rs** -- OSC bundle construction with change detection (only sends changed values)
- **state.rs** -- Shared tracking state between receiver and sender threads
- **main.rs** -- CLI args, thread orchestration, graceful shutdown

## License

MIT
