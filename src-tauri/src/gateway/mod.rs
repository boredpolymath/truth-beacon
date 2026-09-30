use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub mod rate_limiter;
pub use rate_limiter::DiscordRateLimiter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GatewayState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    RateLimited,
}

pub struct GatewayListener {
    state: Arc<std::sync::Mutex<GatewayState>>,
    is_running: Arc<AtomicBool>,
}

impl GatewayListener {
    pub fn new() -> Self {
        Self {
            state: Arc::new(std::sync::Mutex::new(GatewayState::Disconnected)),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&self) {
        self.is_running.store(true, Ordering::SeqCst);
        let mut state = self.state.lock().unwrap();
        *state = GatewayState::Connected;
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        let mut state = self.state.lock().unwrap();
        *state = GatewayState::Disconnected;
    }

    pub fn get_state(&self) -> GatewayState {
        let state = self.state.lock().unwrap();
        state.clone()
    }
}
