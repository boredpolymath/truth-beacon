//! Asynchronous WebSocket Client for Discord Gateway v10 (Phase 14.1)
//!
//! Provides the persistent WebSocket event stream connection to `wss://gateway.discord.gg/?v=10&encoding=json`,
//! with full support for Discord Gateway Opcodes:
//! - Dispatch (0)
//! - Heartbeat (1)
//! - Identify (2)
//! - Resume (6)
//! - Reconnect (7)
//! - Invalid Session (9)
//! - Hello (10)
//! - Heartbeat ACK (11)

use crate::gateway::payload::{
    GatewayHelloData, GatewayOpcode, GatewayPayload, GatewayReadyData, DEFAULT_GATEWAY_URL,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;

/// Privileged Intent constants
pub const INTENT_GUILDS: u64 = 1 << 0;
pub const INTENT_GUILD_MEMBERS: u64 = 1 << 1;

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("WebSocket transport error: {0}")]
    Transport(String),

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Gateway connection closed (code: {code:?}, reason: {reason})")]
    ConnectionClosed { code: Option<u16>, reason: String },

    #[error("Zombied gateway connection: Heartbeat ACK was not received before next heartbeat")]
    ZombiedConnection,

    #[error("Invalid session received from gateway (resumable: {resumable})")]
    InvalidSession { resumable: bool },

    #[error("Missing or invalid bot authentication token")]
    Auth(String),

    #[error("Gateway client cancelled or shut down")]
    Shutdown,
}

/// Action decided by the opcode handler in response to an incoming gateway payload
#[derive(Debug, Clone, PartialEq)]
pub enum OpcodeAction {
    /// No outgoing frame needed
    None,
    /// Send an outgoing payload immediately (e.g. Identify, Resume, Heartbeat)
    Send(GatewayPayload),
    /// Reconnect the connection (true = attempt resume, false = fresh identify)
    Reconnect { resume: bool },
    /// Dispatch an event payload to subscribers
    DispatchEvent {
        event_name: String,
        data: serde_json::Value,
    },
}

pub const DEFAULT_EVENT_RING_BUFFER_CAPACITY: usize = 256;

/// Fixed-capacity ring buffer tracking recent Discord Gateway event IDs and sequences.
/// Enables seamless session resumption via Opcode 6 (RESUME) while preventing duplicate
/// event processing and eliminating the need for an expensive full GUILD_MEMBERS_CHUNK re-sync.
#[derive(Debug, Clone)]
pub struct EventRingBuffer {
    capacity: usize,
    events: VecDeque<RecentEventRecord>,
    seen_ids: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentEventRecord {
    pub sequence: u64,
    pub event_id: String,
    pub event_name: String,
    pub received_at: std::time::Instant,
}

impl Default for EventRingBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_EVENT_RING_BUFFER_CAPACITY)
    }
}

impl EventRingBuffer {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(16);
        Self {
            capacity: cap,
            events: VecDeque::with_capacity(cap.min(512)),
            seen_ids: HashSet::with_capacity(cap.min(512)),
        }
    }

    /// Records an event into the ring buffer.
    /// Returns `true` if the event is newly seen, or `false` if it was already recorded.
    pub fn push(&mut self, sequence: u64, event_id: String, event_name: String) -> bool {
        let is_new = self.seen_ids.insert(event_id.clone());
        if self.events.len() >= self.capacity {
            if let Some(evicted) = self.events.pop_front() {
                self.seen_ids.remove(&evicted.event_id);
            }
        }
        self.events.push_back(RecentEventRecord {
            sequence,
            event_id,
            event_name,
            received_at: std::time::Instant::now(),
        });
        is_new
    }

    pub fn contains(&self, event_id: &str) -> bool {
        self.seen_ids.contains(event_id)
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn clear(&mut self) {
        self.events.clear();
        self.seen_ids.clear();
    }

    pub fn recent_event_ids(&self) -> Vec<String> {
        self.events.iter().map(|e| e.event_id.clone()).collect()
    }
}

/// Active Gateway Session metadata
#[derive(Debug, Clone, Default)]
pub struct GatewaySessionState {
    pub session_id: Arc<RwLock<Option<String>>>,
    pub resume_gateway_url: Arc<RwLock<Option<String>>>,
    pub last_sequence: Arc<AtomicU64>,
    pub heartbeat_interval_ms: Arc<AtomicU64>,
    pub heartbeat_acked: Arc<AtomicBool>,
    pub has_received_hello: Arc<AtomicBool>,
    pub recent_events: Arc<RwLock<EventRingBuffer>>,
    pub is_resumed: Arc<AtomicBool>,
}

impl GatewaySessionState {
    pub fn new() -> Self {
        Self {
            session_id: Arc::new(RwLock::new(None)),
            resume_gateway_url: Arc::new(RwLock::new(None)),
            last_sequence: Arc::new(AtomicU64::new(0)),
            heartbeat_interval_ms: Arc::new(AtomicU64::new(41250)),
            heartbeat_acked: Arc::new(AtomicBool::new(true)),
            has_received_hello: Arc::new(AtomicBool::new(false)),
            recent_events: Arc::new(RwLock::new(EventRingBuffer::new(
                DEFAULT_EVENT_RING_BUFFER_CAPACITY,
            ))),
            is_resumed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Records an event in the session's internal ring buffer.
    pub async fn record_event(&self, sequence: u64, event_id: String, event_name: String) -> bool {
        let mut buf = self.recent_events.write().await;
        buf.push(sequence, event_id, event_name)
    }

    /// Checks if a recent event ID exists in the ring buffer.
    pub async fn has_seen_event(&self, event_id: &str) -> bool {
        let buf = self.recent_events.read().await;
        buf.contains(event_id)
    }

    /// Returns the number of events in the ring buffer.
    pub async fn ring_buffer_len(&self) -> usize {
        let buf = self.recent_events.read().await;
        buf.len()
    }

    /// Returns `true` if the session can be resumed via Opcode 6 (RESUME).
    pub async fn can_resume(&self) -> bool {
        let sess_guard = self.session_id.read().await;
        sess_guard.is_some() && self.last_sequence.load(Ordering::SeqCst) > 0
    }

    /// Marks the session as successfully resumed via Opcode 6 (RESUME).
    pub fn mark_resumed(&self) {
        self.is_resumed.store(true, Ordering::SeqCst);
    }

    /// Checks if the active session was successfully resumed without requiring a full re-sync.
    pub fn is_session_resumed(&self) -> bool {
        self.is_resumed.load(Ordering::SeqCst)
    }

    /// Resets the session state on unrecoverable disconnects (e.g. non-resumable Opcode 9).
    pub async fn reset_session(&self) {
        {
            let mut sess_id_guard = self.session_id.write().await;
            *sess_id_guard = None;
        }
        {
            let mut resume_url_guard = self.resume_gateway_url.write().await;
            *resume_url_guard = None;
        }
        {
            let mut buf = self.recent_events.write().await;
            buf.clear();
        }
        self.last_sequence.store(0, Ordering::SeqCst);
        self.heartbeat_acked.store(true, Ordering::SeqCst);
        self.has_received_hello.store(false, Ordering::SeqCst);
        self.is_resumed.store(false, Ordering::SeqCst);
    }
}

/// Configuration for Discord Gateway v10 connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub gateway_url: String,
    pub bot_token: String,
    pub intents: u64,
    pub auto_reconnect: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            gateway_url: DEFAULT_GATEWAY_URL.to_string(),
            bot_token: String::new(),
            intents: INTENT_GUILDS | INTENT_GUILD_MEMBERS, // GUILD_MEMBERS (1 << 1)
            auto_reconnect: true,
        }
    }
}

/// Core Discord Gateway Opcode Protocol Handler
#[derive(Clone)]
pub struct GatewayProtocolHandler {
    config: GatewayConfig,
    session: GatewaySessionState,
}

/// Extracts a unique event identifier from an incoming Gateway Dispatch payload.
pub fn extract_event_id(event_name: &str, seq: u64, data: &serde_json::Value) -> String {
    if let Some(id) = data.get("id").and_then(|v| v.as_str()) {
        id.to_string()
    } else if let Some(user_id) = data
        .get("user")
        .and_then(|u| u.get("id"))
        .and_then(|v| v.as_str())
    {
        if let Some(guild_id) = data.get("guild_id").and_then(|v| v.as_str()) {
            format!("{}:{}:{}", event_name, guild_id, user_id)
        } else {
            format!("{}:{}", event_name, user_id)
        }
    } else if let Some(guild_id) = data.get("guild_id").and_then(|v| v.as_str()) {
        format!("{}:{}:{}", event_name, guild_id, seq)
    } else {
        format!("{}:{}", event_name, seq)
    }
}

impl GatewayProtocolHandler {
    pub fn new(config: GatewayConfig, session: GatewaySessionState) -> Self {
        Self { config, session }
    }

    /// Process an incoming Gateway payload and determine the appropriate Opcode action
    pub async fn handle_incoming(
        &self,
        payload: GatewayPayload,
    ) -> Result<OpcodeAction, GatewayError> {
        match payload.op {
            // Opcode 10: Hello
            GatewayOpcode::Hello => {
                let data = payload.d.ok_or_else(|| {
                    GatewayError::Transport("Hello frame missing payload data".into())
                })?;
                let hello: GatewayHelloData = serde_json::from_value(data)?;
                self.session
                    .heartbeat_interval_ms
                    .store(hello.heartbeat_interval, Ordering::SeqCst);
                self.session
                    .has_received_hello
                    .store(true, Ordering::SeqCst);
                self.session.heartbeat_acked.store(true, Ordering::SeqCst);

                // If we have an existing session and sequence, attempt Resume; otherwise Identify
                let session_id_guard = self.session.session_id.read().await;
                let last_seq = self.session.last_sequence.load(Ordering::SeqCst);

                if let Some(ref session_id) = *session_id_guard {
                    if last_seq > 0 {
                        let resume_payload =
                            GatewayPayload::resume(&self.config.bot_token, session_id, last_seq);
                        return Ok(OpcodeAction::Send(resume_payload));
                    }
                }

                // Fresh connection: Send Identify (Opcode 2)
                let identify_payload =
                    GatewayPayload::identify(&self.config.bot_token, self.config.intents);
                Ok(OpcodeAction::Send(identify_payload))
            }

            // Opcode 11: Heartbeat ACK
            GatewayOpcode::HeartbeatAck => {
                self.session.heartbeat_acked.store(true, Ordering::SeqCst);
                Ok(OpcodeAction::None)
            }

            // Opcode 1: Heartbeat (Server requested immediate heartbeat)
            GatewayOpcode::Heartbeat => {
                let seq = match self.session.last_sequence.load(Ordering::SeqCst) {
                    0 => None,
                    n => Some(n),
                };
                let hb = GatewayPayload::heartbeat(seq);
                Ok(OpcodeAction::Send(hb))
            }

            // Opcode 0: Dispatch
            GatewayOpcode::Dispatch => {
                let seq = payload.s.unwrap_or(0);
                if seq > 0 {
                    self.session.last_sequence.store(seq, Ordering::SeqCst);
                }

                let event_name = payload.t.unwrap_or_default();
                let event_data = payload.d.unwrap_or(serde_json::Value::Null);

                // Handle session caching on READY event
                if event_name == "READY" {
                    self.session.is_resumed.store(false, Ordering::SeqCst);
                    if let Ok(ready) =
                        serde_json::from_value::<GatewayReadyData>(event_data.clone())
                    {
                        let mut sess_guard = self.session.session_id.write().await;
                        *sess_guard = Some(ready.session_id);
                        if let Some(resume_url) = ready.resume_gateway_url {
                            let mut url_guard = self.session.resume_gateway_url.write().await;
                            *url_guard = Some(resume_url);
                        }
                    }
                } else if event_name == "RESUMED" {
                    self.session.mark_resumed();
                    log::info!(
                        "Gateway session successfully resumed via Opcode 6 (RESUME); internal ring buffer preserved and skipping full GUILD_MEMBERS_CHUNK re-sync."
                    );
                }

                // Extract event identifier and store in internal ring buffer
                let event_id = extract_event_id(&event_name, seq, &event_data);
                if self.session.has_seen_event(&event_id).await {
                    log::debug!("Skipping duplicate gateway event: {}", event_name);
                    return Ok(OpcodeAction::None);
                }
                self.session
                    .record_event(seq, event_id, event_name.clone())
                    .await;

                Ok(OpcodeAction::DispatchEvent {
                    event_name,
                    data: event_data,
                })
            }

            // Opcode 7: Reconnect
            GatewayOpcode::Reconnect => {
                // Discord requests reconnection with Resume
                Ok(OpcodeAction::Reconnect { resume: true })
            }

            // Opcode 9: Invalid Session
            GatewayOpcode::InvalidSession => {
                let is_resumable = payload.d.and_then(|v| v.as_bool()).unwrap_or(false);
                if !is_resumable {
                    self.session.reset_session().await;
                }
                Ok(OpcodeAction::Reconnect {
                    resume: is_resumable,
                })
            }

            // Client-to-server opcodes or unrecognized
            GatewayOpcode::Identify
            | GatewayOpcode::Resume
            | GatewayOpcode::PresenceUpdate
            | GatewayOpcode::VoiceStateUpdate
            | GatewayOpcode::RequestGuildMembers
            | GatewayOpcode::Unknown(_) => Ok(OpcodeAction::None),
        }
    }

    /// Construct a Heartbeat payload (Opcode 1) for the periodic heartbeat loop,
    /// failing if the previous heartbeat was never acknowledged (Zombied connection).
    pub fn create_periodic_heartbeat(&self) -> Result<GatewayPayload, GatewayError> {
        let acked = self.session.heartbeat_acked.swap(false, Ordering::SeqCst);
        if !acked {
            return Err(GatewayError::ZombiedConnection);
        }

        let seq = match self.session.last_sequence.load(Ordering::SeqCst) {
            0 => None,
            n => Some(n),
        };
        Ok(GatewayPayload::heartbeat(seq))
    }
}

/// Real-time event dispatched from Gateway
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayDispatchEvent {
    pub event_name: String,
    pub data: serde_json::Value,
    pub sequence: u64,
}

/// Asynchronous Discord Gateway WebSocket Client
pub struct DiscordGatewayClient {
    config: GatewayConfig,
    session: GatewaySessionState,
    is_running: Arc<AtomicBool>,
    event_sender: mpsc::Sender<GatewayDispatchEvent>,
}

impl DiscordGatewayClient {
    pub fn new(
        config: GatewayConfig,
        event_sender: mpsc::Sender<GatewayDispatchEvent>,
    ) -> (Self, GatewaySessionState) {
        let session = GatewaySessionState::new();
        let client = Self {
            config,
            session: session.clone(),
            is_running: Arc::new(AtomicBool::new(false)),
            event_sender,
        };
        (client, session)
    }

    pub fn from_session(
        config: GatewayConfig,
        session: GatewaySessionState,
        event_sender: mpsc::Sender<GatewayDispatchEvent>,
    ) -> Self {
        Self {
            config,
            session,
            is_running: Arc::new(AtomicBool::new(false)),
            event_sender,
        }
    }

    /// Connect to Discord Gateway WebSocket and maintain the event and heartbeat stream
    pub async fn run_connection(&self) -> Result<(), GatewayError> {
        self.is_running.store(true, Ordering::SeqCst);

        let target_url = {
            let resume_url_guard = self.session.resume_gateway_url.read().await;
            if let Some(ref r_url) = *resume_url_guard {
                format!("{}/?v=10&encoding=json", r_url.trim_end_matches('/'))
            } else {
                self.config.gateway_url.clone()
            }
        };

        log::info!("Connecting to Discord Gateway v10 at {}", target_url);

        let (ws_stream, _) = tokio_tungstenite::connect_async(&target_url)
            .await
            .map_err(|e| GatewayError::Transport(format!("WebSocket handshake failed: {}", e)))?;

        let (write_half, mut read_half) = ws_stream.split();
        let write_mutex = Arc::new(Mutex::new(write_half));

        let protocol_handler =
            GatewayProtocolHandler::new(self.config.clone(), self.session.clone());

        // Spawn periodic background heartbeat sender to maintain connection with Discord Gateway
        let hb_writer = write_mutex.clone();
        let hb_handler = protocol_handler.clone();
        let hb_running = self.is_running.clone();
        let hb_interval = self.session.heartbeat_interval_ms.clone();

        let hb_handle = tokio::spawn(async move {
            // Wait until Opcode 10 Hello defines the server heartbeat interval
            while hb_running.load(Ordering::SeqCst) {
                let interval = hb_interval.load(Ordering::SeqCst);
                if interval > 0 {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }

            while hb_running.load(Ordering::SeqCst) {
                let interval = hb_interval.load(Ordering::SeqCst);
                let delay = if interval > 0 { interval } else { 41250 };
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                if !hb_running.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(hb) = hb_handler.create_periodic_heartbeat() {
                    if let Ok(json_str) = hb.to_json_str() {
                        let mut writer = hb_writer.lock().await;
                        let _ = writer.send(Message::Text(json_str.into())).await;
                    }
                }
            }
        });

        let mut loop_res: Result<(), GatewayError> = Ok(());

        // Process incoming frames
        while self.is_running.load(Ordering::SeqCst) {
            match read_half.next().await {
                Some(Ok(Message::Text(text))) => {
                    let payload = match GatewayPayload::from_json_str(&text) {
                        Ok(p) => p,
                        Err(e) => {
                            loop_res = Err(GatewayError::from(e));
                            break;
                        }
                    };
                    let action = match protocol_handler.handle_incoming(payload).await {
                        Ok(a) => a,
                        Err(e) => {
                            loop_res = Err(e);
                            break;
                        }
                    };

                    match action {
                        OpcodeAction::Send(outgoing) => {
                            let json_str = match outgoing.to_json_str() {
                                Ok(s) => s,
                                Err(e) => {
                                    loop_res = Err(GatewayError::from(e));
                                    break;
                                }
                            };
                            let mut writer = write_mutex.lock().await;
                            if let Err(e) = writer.send(Message::Text(json_str.into())).await {
                                loop_res = Err(GatewayError::Transport(e.to_string()));
                                break;
                            }
                        }
                        OpcodeAction::DispatchEvent { event_name, data } => {
                            let seq = self.session.last_sequence.load(Ordering::SeqCst);
                            let _ = self
                                .event_sender
                                .send(GatewayDispatchEvent {
                                    event_name,
                                    data,
                                    sequence: seq,
                                })
                                .await;
                        }
                        OpcodeAction::Reconnect { resume } => {
                            loop_res = Err(GatewayError::InvalidSession { resumable: resume });
                            break;
                        }
                        OpcodeAction::None => {}
                    }
                }
                Some(Ok(Message::Close(frame))) => {
                    let (code, reason) = match frame {
                        Some(CloseFrame { code, reason }) => {
                            (Some(code.into()), reason.to_string())
                        }
                        None => (None, "Normal closure".to_string()),
                    };
                    loop_res = Err(GatewayError::ConnectionClosed { code, reason });
                    break;
                }
                Some(Ok(Message::Ping(data))) => {
                    let mut writer = write_mutex.lock().await;
                    let _ = writer.send(Message::Pong(data)).await;
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    loop_res = Err(GatewayError::Transport(format!(
                        "WebSocket read error: {}",
                        e
                    )));
                    break;
                }
                None => {
                    loop_res = Err(GatewayError::ConnectionClosed {
                        code: None,
                        reason: "WebSocket stream EOF".into(),
                    });
                    break;
                }
            }
        }

        hb_handle.abort();
        loop_res
    }

    /// Stops the gateway client
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_protocol_handler_hello_triggers_identify() {
        let config = GatewayConfig {
            gateway_url: "wss://gateway.discord.gg/?v=10&encoding=json".into(),
            bot_token: "Bot test_token".into(),
            intents: INTENT_GUILDS | INTENT_GUILD_MEMBERS,
            auto_reconnect: true,
        };
        let session = GatewaySessionState::new();
        let handler = GatewayProtocolHandler::new(config, session.clone());

        let hello_payload = GatewayPayload {
            op: GatewayOpcode::Hello,
            d: Some(serde_json::json!({
                "heartbeat_interval": 41250
            })),
            s: None,
            t: None,
        };

        let action = handler.handle_incoming(hello_payload).await.unwrap();
        assert_eq!(session.heartbeat_interval_ms.load(Ordering::SeqCst), 41250);
        assert!(session.has_received_hello.load(Ordering::SeqCst));

        // When session is fresh, action must be Send(Identify)
        match action {
            OpcodeAction::Send(p) => {
                assert_eq!(p.op, GatewayOpcode::Identify);
                let d = p.d.unwrap();
                assert_eq!(d["token"], "Bot test_token");
                assert_eq!(d["intents"], INTENT_GUILDS | INTENT_GUILD_MEMBERS);
            }
            other => panic!("Expected Send(Identify), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_protocol_handler_hello_triggers_resume_when_session_cached() {
        let config = GatewayConfig {
            gateway_url: "wss://gateway.discord.gg/?v=10&encoding=json".into(),
            bot_token: "Bot test_token".into(),
            intents: INTENT_GUILD_MEMBERS,
            auto_reconnect: true,
        };
        let session = GatewaySessionState::new();

        // Populate existing session and sequence
        {
            let mut s = session.session_id.write().await;
            *s = Some("cached_session_12345".into());
        }
        session.last_sequence.store(108, Ordering::SeqCst);

        let handler = GatewayProtocolHandler::new(config, session.clone());

        let hello_payload = GatewayPayload {
            op: GatewayOpcode::Hello,
            d: Some(serde_json::json!({ "heartbeat_interval": 41250 })),
            s: None,
            t: None,
        };

        let action = handler.handle_incoming(hello_payload).await.unwrap();

        // When session exists and seq > 0, action must be Send(Resume)
        match action {
            OpcodeAction::Send(p) => {
                assert_eq!(p.op, GatewayOpcode::Resume);
                let d = p.d.unwrap();
                assert_eq!(d["token"], "Bot test_token");
                assert_eq!(d["session_id"], "cached_session_12345");
                assert_eq!(d["seq"], 108);
            }
            other => panic!("Expected Send(Resume), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_protocol_handler_heartbeat_ack_lifecycle() {
        let config = GatewayConfig::default();
        let session = GatewaySessionState::new();
        let handler = GatewayProtocolHandler::new(config, session.clone());

        // First heartbeat sent
        let hb = handler.create_periodic_heartbeat().unwrap();
        assert_eq!(hb.op, GatewayOpcode::Heartbeat);
        assert!(!session.heartbeat_acked.load(Ordering::SeqCst));

        // If another heartbeat is attempted before ACK, it must return ZombiedConnection error
        let zombied_err = handler.create_periodic_heartbeat();
        assert!(matches!(zombied_err, Err(GatewayError::ZombiedConnection)));

        // Receive Opcode 11 Heartbeat ACK
        let ack_payload = GatewayPayload {
            op: GatewayOpcode::HeartbeatAck,
            d: None,
            s: None,
            t: None,
        };
        let action = handler.handle_incoming(ack_payload).await.unwrap();
        assert_eq!(action, OpcodeAction::None);
        assert!(session.heartbeat_acked.load(Ordering::SeqCst));

        // Now next heartbeat can be sent safely
        let hb2 = handler.create_periodic_heartbeat().unwrap();
        assert_eq!(hb2.op, GatewayOpcode::Heartbeat);
    }

    #[tokio::test]
    async fn test_protocol_handler_dispatch_ready_and_reconnect() {
        let config = GatewayConfig::default();
        let session = GatewaySessionState::new();
        let handler = GatewayProtocolHandler::new(config, session.clone());

        let ready_json = serde_json::json!({
            "v": 10,
            "session_id": "sess_999888",
            "resume_gateway_url": "wss://gateway-res.discord.gg"
        });

        let ready_payload = GatewayPayload {
            op: GatewayOpcode::Dispatch,
            d: Some(ready_json),
            s: Some(1),
            t: Some("READY".into()),
        };

        let action = handler.handle_incoming(ready_payload).await.unwrap();
        assert!(matches!(action, OpcodeAction::DispatchEvent { .. }));

        assert_eq!(session.last_sequence.load(Ordering::SeqCst), 1);
        assert_eq!(
            session.session_id.read().await.as_deref(),
            Some("sess_999888")
        );
        assert_eq!(
            session.resume_gateway_url.read().await.as_deref(),
            Some("wss://gateway-res.discord.gg")
        );

        // Opcode 7: Reconnect
        let reconnect_payload = GatewayPayload {
            op: GatewayOpcode::Reconnect,
            d: None,
            s: None,
            t: None,
        };
        let reconnect_action = handler.handle_incoming(reconnect_payload).await.unwrap();
        assert_eq!(reconnect_action, OpcodeAction::Reconnect { resume: true });

        // Opcode 9: Invalid Session (resumable)
        let invalid_resumable = GatewayPayload {
            op: GatewayOpcode::InvalidSession,
            d: Some(serde_json::Value::Bool(true)),
            s: None,
            t: None,
        };
        let inv_res_action = handler.handle_incoming(invalid_resumable).await.unwrap();
        assert_eq!(inv_res_action, OpcodeAction::Reconnect { resume: true });

        // Opcode 9: Invalid Session (non-resumable)
        let invalid_fresh = GatewayPayload {
            op: GatewayOpcode::InvalidSession,
            d: Some(serde_json::Value::Bool(false)),
            s: None,
            t: None,
        };
        let inv_fresh_action = handler.handle_incoming(invalid_fresh).await.unwrap();
        assert_eq!(inv_fresh_action, OpcodeAction::Reconnect { resume: false });
        assert!(session.session_id.read().await.is_none());
        assert_eq!(session.last_sequence.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_event_ring_buffer_fifo_eviction_and_deduplication() {
        let mut ring = EventRingBuffer::new(16);

        // Insert 16 unique events
        for seq in 1..=16 {
            let id = format!("event_{}", seq);
            let is_new = ring.push(seq, id.clone(), "GUILD_MEMBER_ADD".into());
            assert!(is_new, "Event {} must be newly recorded", seq);
            assert!(ring.contains(&id));
        }

        assert_eq!(ring.len(), 16);

        // Re-inserting an already seen event returns false (duplicate detected)
        let dup = ring.push(16, "event_16".into(), "GUILD_MEMBER_ADD".into());
        assert!(!dup, "Duplicate event must return false");

        // Push 17th event: should evict oldest (event_1)
        let is_new = ring.push(17, "event_17".into(), "GUILD_MEMBER_UPDATE".into());
        assert!(is_new);
        assert_eq!(ring.len(), 16);
        assert!(
            !ring.contains("event_1"),
            "event_1 must be evicted via FIFO"
        );
        assert!(
            ring.contains("event_17"),
            "event_17 must be present in ring buffer"
        );
    }

    #[tokio::test]
    async fn test_session_resumed_opcode_6_preserves_ring_buffer_without_full_resync() {
        let config = GatewayConfig::default();
        let session = GatewaySessionState::new();

        // 1. Establish session
        {
            let mut s = session.session_id.write().await;
            *s = Some("active_sess_555".into());
        }
        session.last_sequence.store(42, Ordering::SeqCst);

        // 2. Pre-populate ring buffer with dispatched events
        session
            .record_event(40, "evt_40".into(), "GUILD_MEMBER_ADD".into())
            .await;
        session
            .record_event(41, "evt_41".into(), "GUILD_MEMBER_UPDATE".into())
            .await;
        session
            .record_event(42, "evt_42".into(), "USER_UPDATE".into())
            .await;
        assert_eq!(session.ring_buffer_len().await, 3);
        assert!(session.can_resume().await);

        let handler = GatewayProtocolHandler::new(config.clone(), session.clone());

        // 3. Receive Opcode 10 Hello -> handler issues Opcode 6 Resume
        let hello = GatewayPayload {
            op: GatewayOpcode::Hello,
            d: Some(serde_json::json!({ "heartbeat_interval": 30000 })),
            s: None,
            t: None,
        };
        let action = handler.handle_incoming(hello).await.unwrap();
        match action {
            OpcodeAction::Send(payload) => {
                assert_eq!(payload.op, GatewayOpcode::Resume);
                let d = payload.d.unwrap();
                assert_eq!(d["session_id"], "active_sess_555");
                assert_eq!(d["seq"], 42);
            }
            other => panic!("Expected OpcodeAction::Send(Resume), got {:?}", other),
        }

        // 4. Discord responds with RESUMED (Dispatch with t: "RESUMED")
        let resumed_payload = GatewayPayload {
            op: GatewayOpcode::Dispatch,
            d: Some(serde_json::json!({})),
            s: Some(43),
            t: Some("RESUMED".into()),
        };
        let resumed_action = handler.handle_incoming(resumed_payload).await.unwrap();
        assert!(matches!(resumed_action, OpcodeAction::DispatchEvent { .. }));

        // 5. Verify session is marked resumed, ring buffer is preserved, no full re-sync needed
        assert!(
            session.is_session_resumed(),
            "Session must be marked resumed"
        );
        assert_eq!(
            session.ring_buffer_len().await,
            4,
            "Ring buffer must preserve existing events and append RESUMED event"
        );
        assert!(session.has_seen_event("evt_40").await);
        assert!(session.has_seen_event("evt_41").await);
        assert!(session.has_seen_event("evt_42").await);
        assert_eq!(session.last_sequence.load(Ordering::SeqCst), 43);

        // 6. If an invalid non-resumable session arrives later, ring buffer must be wiped
        let invalid_payload = GatewayPayload {
            op: GatewayOpcode::InvalidSession,
            d: Some(serde_json::Value::Bool(false)),
            s: None,
            t: None,
        };
        let inv_action = handler.handle_incoming(invalid_payload).await.unwrap();
        assert_eq!(inv_action, OpcodeAction::Reconnect { resume: false });
        assert_eq!(
            session.ring_buffer_len().await,
            0,
            "Ring buffer must be wiped on fresh re-sync"
        );
        assert!(!session.is_session_resumed());
        assert!(!session.can_resume().await);
    }
}
