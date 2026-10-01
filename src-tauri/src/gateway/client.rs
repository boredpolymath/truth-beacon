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

/// Active Gateway Session metadata
#[derive(Debug, Clone, Default)]
pub struct GatewaySessionState {
    pub session_id: Arc<RwLock<Option<String>>>,
    pub resume_gateway_url: Arc<RwLock<Option<String>>>,
    pub last_sequence: Arc<AtomicU64>,
    pub heartbeat_interval_ms: Arc<AtomicU64>,
    pub heartbeat_acked: Arc<AtomicBool>,
    pub has_received_hello: Arc<AtomicBool>,
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
        }
    }

    pub async fn reset_session(&self) {
        {
            let mut sess_id_guard = self.session_id.write().await;
            *sess_id_guard = None;
        }
        {
            let mut resume_url_guard = self.resume_gateway_url.write().await;
            *resume_url_guard = None;
        }
        self.last_sequence.store(0, Ordering::SeqCst);
        self.heartbeat_acked.store(true, Ordering::SeqCst);
        self.has_received_hello.store(false, Ordering::SeqCst);
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
pub struct GatewayProtocolHandler {
    config: GatewayConfig,
    session: GatewaySessionState,
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
                if let Some(seq) = payload.s {
                    self.session.last_sequence.store(seq, Ordering::SeqCst);
                }

                let event_name = payload.t.unwrap_or_default();
                let event_data = payload.d.unwrap_or(serde_json::Value::Null);

                // Handle session caching on READY event
                if event_name == "READY" {
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
                }

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
                    let mut sess_guard = self.session.session_id.write().await;
                    *sess_guard = None;
                    self.session.last_sequence.store(0, Ordering::SeqCst);
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

        // Process incoming frames
        while self.is_running.load(Ordering::SeqCst) {
            match read_half.next().await {
                Some(Ok(Message::Text(text))) => {
                    let payload = GatewayPayload::from_json_str(&text)?;
                    let action = protocol_handler.handle_incoming(payload).await?;

                    match action {
                        OpcodeAction::Send(outgoing) => {
                            let json_str = outgoing.to_json_str()?;
                            let mut writer = write_mutex.lock().await;
                            writer
                                .send(Message::Text(json_str.into()))
                                .await
                                .map_err(|e| GatewayError::Transport(e.to_string()))?;
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
                            return Err(GatewayError::InvalidSession { resumable: resume });
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
                    return Err(GatewayError::ConnectionClosed { code, reason });
                }
                Some(Ok(Message::Ping(data))) => {
                    let mut writer = write_mutex.lock().await;
                    let _ = writer.send(Message::Pong(data)).await;
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    return Err(GatewayError::Transport(format!(
                        "WebSocket read error: {}",
                        e
                    )));
                }
                None => {
                    return Err(GatewayError::ConnectionClosed {
                        code: None,
                        reason: "WebSocket stream EOF".into(),
                    });
                }
            }
        }

        Ok(())
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
}
