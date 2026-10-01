//! Discord Gateway v10 Payload & Opcode Architecture (Phase 14.1)
//!
//! Discord Gateway v10 communicates via WebSocket text frames containing JSON payloads.
//! Standard schema:
//! ```json
//! {
//!   "op": <integer opcode>,
//!   "d": <event or command data>,
//!   "s": <sequence number or null>,
//!   "t": <event name or null>
//! }
//! ```

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Default Discord Gateway v10 WebSocket URL
pub const DEFAULT_GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";

/// Discord Gateway Opcodes (v10)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GatewayOpcode {
    /// Opcode 0: An event was dispatched by Discord (READY, GUILD_MEMBER_ADD, etc.)
    Dispatch,
    /// Opcode 1: Heartbeat sent by client or requested by server
    Heartbeat,
    /// Opcode 2: Starts a new session during initial handshake
    Identify,
    /// Opcode 3: Update client status/presence
    PresenceUpdate,
    /// Opcode 4: Join/move/leave voice channels
    VoiceStateUpdate,
    /// Opcode 6: Resume a dropped session
    Resume,
    /// Opcode 7: Discord requires the client to reconnect and resume
    Reconnect,
    /// Opcode 8: Request guild members (chunking)
    RequestGuildMembers,
    /// Opcode 9: Session has been invalidated
    InvalidSession,
    /// Opcode 10: Sent immediately after connecting; contains heartbeat_interval
    Hello,
    /// Opcode 11: Heartbeat ACK returned in response to client heartbeat
    HeartbeatAck,
    /// Unknown opcode fallback
    Unknown(u8),
}

impl GatewayOpcode {
    pub fn as_u8(self) -> u8 {
        match self {
            GatewayOpcode::Dispatch => 0,
            GatewayOpcode::Heartbeat => 1,
            GatewayOpcode::Identify => 2,
            GatewayOpcode::PresenceUpdate => 3,
            GatewayOpcode::VoiceStateUpdate => 4,
            GatewayOpcode::Resume => 6,
            GatewayOpcode::Reconnect => 7,
            GatewayOpcode::RequestGuildMembers => 8,
            GatewayOpcode::InvalidSession => 9,
            GatewayOpcode::Hello => 10,
            GatewayOpcode::HeartbeatAck => 11,
            GatewayOpcode::Unknown(u) => u,
        }
    }
}

impl From<u8> for GatewayOpcode {
    fn from(val: u8) -> Self {
        match val {
            0 => GatewayOpcode::Dispatch,
            1 => GatewayOpcode::Heartbeat,
            2 => GatewayOpcode::Identify,
            3 => GatewayOpcode::PresenceUpdate,
            4 => GatewayOpcode::VoiceStateUpdate,
            6 => GatewayOpcode::Resume,
            7 => GatewayOpcode::Reconnect,
            8 => GatewayOpcode::RequestGuildMembers,
            9 => GatewayOpcode::InvalidSession,
            10 => GatewayOpcode::Hello,
            11 => GatewayOpcode::HeartbeatAck,
            other => GatewayOpcode::Unknown(other),
        }
    }
}

impl From<GatewayOpcode> for u8 {
    fn from(op: GatewayOpcode) -> Self {
        op.as_u8()
    }
}

impl Serialize for GatewayOpcode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(self.as_u8())
    }
}

impl<'de> Deserialize<'de> for GatewayOpcode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        Ok(GatewayOpcode::from(val))
    }
}

/// Generic Gateway v10 JSON frame
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GatewayPayload {
    /// Opcode identifying frame type
    pub op: GatewayOpcode,

    /// Event data or command payload
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d: Option<serde_json::Value>,

    /// Sequence number (only present for Opcode 0 Dispatch)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub s: Option<u64>,

    /// Event name (only present for Opcode 0 Dispatch)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<String>,
}

impl GatewayPayload {
    /// Construct a Heartbeat payload (Opcode 1)
    pub fn heartbeat(last_sequence: Option<u64>) -> Self {
        Self {
            op: GatewayOpcode::Heartbeat,
            d: last_sequence.map(|seq| serde_json::Value::Number(seq.into())),
            s: None,
            t: None,
        }
    }

    /// Construct an Identify payload (Opcode 2)
    pub fn identify(token: &str, intents: u64) -> Self {
        let properties = serde_json::json!({
            "os": std::env::consts::OS,
            "browser": "truth-beacon",
            "device": "truth-beacon"
        });

        Self {
            op: GatewayOpcode::Identify,
            d: Some(serde_json::json!({
                "token": token,
                "intents": intents,
                "properties": properties,
                "compress": false
            })),
            s: None,
            t: None,
        }
    }

    /// Construct a Resume payload (Opcode 6)
    pub fn resume(token: &str, session_id: &str, last_sequence: u64) -> Self {
        Self {
            op: GatewayOpcode::Resume,
            d: Some(serde_json::json!({
                "token": token,
                "session_id": session_id,
                "seq": last_sequence
            })),
            s: None,
            t: None,
        }
    }

    /// Parse a JSON string into a GatewayPayload
    pub fn from_json_str(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }

    /// Serialize payload to a JSON string
    pub fn to_json_str(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Payload sent in Opcode 10 Hello
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayHelloData {
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval: u64,
}

/// Session data parsed from Dispatch `READY`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayReadyData {
    pub v: u8,
    pub session_id: String,
    pub resume_gateway_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opcode_conversions() {
        assert_eq!(GatewayOpcode::from(0), GatewayOpcode::Dispatch);
        assert_eq!(GatewayOpcode::from(1), GatewayOpcode::Heartbeat);
        assert_eq!(GatewayOpcode::from(2), GatewayOpcode::Identify);
        assert_eq!(GatewayOpcode::from(6), GatewayOpcode::Resume);
        assert_eq!(GatewayOpcode::from(7), GatewayOpcode::Reconnect);
        assert_eq!(GatewayOpcode::from(9), GatewayOpcode::InvalidSession);
        assert_eq!(GatewayOpcode::from(10), GatewayOpcode::Hello);
        assert_eq!(GatewayOpcode::from(11), GatewayOpcode::HeartbeatAck);
        assert_eq!(GatewayOpcode::from(99), GatewayOpcode::Unknown(99));

        assert_eq!(GatewayOpcode::Dispatch.as_u8(), 0);
        assert_eq!(GatewayOpcode::Heartbeat.as_u8(), 1);
        assert_eq!(GatewayOpcode::Identify.as_u8(), 2);
        assert_eq!(GatewayOpcode::Resume.as_u8(), 6);
        assert_eq!(GatewayOpcode::Reconnect.as_u8(), 7);
        assert_eq!(GatewayOpcode::InvalidSession.as_u8(), 9);
        assert_eq!(GatewayOpcode::Hello.as_u8(), 10);
        assert_eq!(GatewayOpcode::HeartbeatAck.as_u8(), 11);
        assert_eq!(GatewayOpcode::Unknown(42).as_u8(), 42);
    }

    #[test]
    fn test_serialize_deserialize_hello_payload() {
        let raw_json = r#"{"op":10,"d":{"heartbeat_interval":41250}}"#;
        let payload = GatewayPayload::from_json_str(raw_json).unwrap();
        assert_eq!(payload.op, GatewayOpcode::Hello);

        let hello: GatewayHelloData = serde_json::from_value(payload.d.unwrap()).unwrap();
        assert_eq!(hello.heartbeat_interval, 41250);
    }

    #[test]
    fn test_serialize_deserialize_heartbeat() {
        // Heartbeat with sequence
        let hb_with_seq = GatewayPayload::heartbeat(Some(42));
        assert_eq!(hb_with_seq.op, GatewayOpcode::Heartbeat);
        assert_eq!(hb_with_seq.d, Some(serde_json::json!(42)));
        let json_str = hb_with_seq.to_json_str().unwrap();
        assert!(json_str.contains(r#""op":1"#));
        assert!(json_str.contains(r#""d":42"#));

        // Heartbeat without sequence (first heartbeat before dispatch)
        let hb_null = GatewayPayload::heartbeat(None);
        assert_eq!(hb_null.d, None);
    }

    #[test]
    fn test_serialize_identify_payload() {
        let payload = GatewayPayload::identify("Bot mock_secret_token", 513);
        assert_eq!(payload.op, GatewayOpcode::Identify);
        let d = payload.d.unwrap();
        assert_eq!(d["token"], "Bot mock_secret_token");
        assert_eq!(d["intents"], 513);
        assert_eq!(d["properties"]["browser"], "truth-beacon");
    }

    #[test]
    fn test_serialize_resume_payload() {
        let payload = GatewayPayload::resume("Bot token_123", "sess_abc_789", 105);
        assert_eq!(payload.op, GatewayOpcode::Resume);
        let d = payload.d.unwrap();
        assert_eq!(d["session_id"], "sess_abc_789");
        assert_eq!(d["seq"], 105);
    }

    #[test]
    fn test_deserialize_dispatch_ready_event() {
        let raw = r#"{
            "op": 0,
            "s": 1,
            "t": "READY",
            "d": {
                "v": 10,
                "session_id": "session_xyz_123",
                "resume_gateway_url": "wss://gateway-us-east1.discord.gg"
            }
        }"#;

        let payload = GatewayPayload::from_json_str(raw).unwrap();
        assert_eq!(payload.op, GatewayOpcode::Dispatch);
        assert_eq!(payload.s, Some(1));
        assert_eq!(payload.t.as_deref(), Some("READY"));

        let ready: GatewayReadyData = serde_json::from_value(payload.d.unwrap()).unwrap();
        assert_eq!(ready.v, 10);
        assert_eq!(ready.session_id, "session_xyz_123");
        assert_eq!(
            ready.resume_gateway_url.as_deref(),
            Some("wss://gateway-us-east1.discord.gg")
        );
    }

    #[test]
    fn test_deserialize_reconnect_and_invalid_session() {
        let reconnect_json = r#"{"op":7,"d":null}"#;
        let reconnect_payload = GatewayPayload::from_json_str(reconnect_json).unwrap();
        assert_eq!(reconnect_payload.op, GatewayOpcode::Reconnect);

        let invalid_session_resumable = r#"{"op":9,"d":true}"#;
        let p_resumable = GatewayPayload::from_json_str(invalid_session_resumable).unwrap();
        assert_eq!(p_resumable.op, GatewayOpcode::InvalidSession);
        assert_eq!(p_resumable.d, Some(serde_json::Value::Bool(true)));

        let invalid_session_fresh = r#"{"op":9,"d":false}"#;
        let p_fresh = GatewayPayload::from_json_str(invalid_session_fresh).unwrap();
        assert_eq!(p_fresh.op, GatewayOpcode::InvalidSession);
        assert_eq!(p_fresh.d, Some(serde_json::Value::Bool(false)));
    }
}
