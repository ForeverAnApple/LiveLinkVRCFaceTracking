use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const BLENDSHAPE_COUNT: usize = 61;

/// Shared tracking state between UDP receiver and OSC sender.
///
/// Protected by `RwLock` - receiver writes, sender reads.
pub struct TrackingState {
    /// The 61 ARKit blendshape values (0.0-1.0 for expressions, radians for head/eye pose).
    pub blendshapes: [f32; BLENDSHAPE_COUNT],
    /// Device identifier from the LiveLink packet header.
    pub device_id: String,
    /// Subject name from the LiveLink packet header.
    pub subject_name: String,
    /// Frame number from the most recent packet.
    pub frame_number: u32,
    /// Timestamp of the last received packet.
    pub last_packet_time: Option<Instant>,
    /// Total packets received (for stats).
    pub packets_received: u64,
}

/// Connection status as an atomic - no lock needed to read/write.
pub static CONNECTED: AtomicBool = AtomicBool::new(false);

/// Global shutdown flag, set by Ctrl+C handler or GUI close.
pub static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Total OSC bundles sent (atomic so sender can increment without write lock).
pub static BUNDLES_SENT: AtomicU64 = AtomicU64::new(0);

impl TrackingState {
    pub fn new() -> Self {
        Self {
            blendshapes: [0.0; BLENDSHAPE_COUNT],
            device_id: String::new(),
            subject_name: String::new(),
            frame_number: 0,
            last_packet_time: None,
            packets_received: 0,
        }
    }

    /// Check timeout and update CONNECTED atomic. Returns true if state changed.
    pub fn check_timeout(&self, timeout: Duration) -> bool {
        let was_connected = CONNECTED.load(Ordering::Relaxed);
        let is_connected = self
            .last_packet_time
            .is_some_and(|t| t.elapsed() <= timeout);
        CONNECTED.store(is_connected, Ordering::Relaxed);
        was_connected != is_connected
    }

    /// Mark as connected (called by receiver on successful parse).
    pub fn mark_connected(&mut self) {
        self.last_packet_time = Some(Instant::now());
        self.packets_received += 1;
        CONNECTED.store(true, Ordering::Relaxed);
    }
}
