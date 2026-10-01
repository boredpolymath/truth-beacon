use serde::{Deserialize, Serialize};

pub mod client;
pub mod daemon;
pub mod events;
pub mod payload;
pub mod power_assertion;
pub mod rate_limiter;
pub mod resilience;

pub use client::{
    DiscordGatewayClient, GatewayConfig, GatewayDispatchEvent, GatewayError,
    GatewayProtocolHandler, GatewaySessionState, OpcodeAction, INTENT_GUILDS, INTENT_GUILD_MEMBERS,
};
pub use daemon::{DaemonMemoryReport, DiscordGatewayDaemon};
pub use events::{
    format_avatar_url, DiscordUserPayload, EventRoutingError, GatewayEventRouter,
    GuildMemberAddPayload, GuildMemberUpdatePayload, UserUpdatePayload, GUILD_MEMBERS_INTENT,
};
pub use payload::{
    GatewayHelloData, GatewayOpcode, GatewayPayload, GatewayReadyData, DEFAULT_GATEWAY_URL,
};
pub use power_assertion::{PowerAssertion, PowerAssertionError};
pub use rate_limiter::DiscordRateLimiter;
pub use resilience::{
    MemoryMonitor, ReconnectionBackoff, BACKOFF_SCHEDULE_SECS, MAX_BACKGROUND_IDLE_RAM_BYTES,
    MAX_BACKOFF_SECS,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GatewayState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    RateLimited,
}
