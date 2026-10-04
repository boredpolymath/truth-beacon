use crate::models::incident::RiskTier;
use crate::models::IncidentStatus;
use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use std::process::Command;
use tauri::Emitter;
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum NotificationError {
    #[error("OS notification error: {0}")]
    OsError(String),
    #[error("Invalid risk tier: {0}")]
    InvalidRiskTier(String),
    #[error("Unknown action: {0}")]
    UnknownAction(String),
    #[error("Command execution error: {0}")]
    CommandError(String),
}

/// Structured payload for native desktop OS notifications (Phase 17.2)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotificationPayload {
    pub incident_id: String,
    pub risk_tier: RiskTier,
    pub suspect_username: String,
    pub suspect_user_id: String,
    pub matched_benchmark_name: String,
    pub similarity_score: f64,
    pub reason: String,
    pub actions: Vec<String>,
}

impl NotificationPayload {
    /// Creates a standard notification payload with the required action buttons:
    /// "Inspect", "Dismiss", and "Ban & Purge"
    pub fn new(
        incident_id: impl Into<String>,
        risk_tier: RiskTier,
        suspect_username: impl Into<String>,
        suspect_user_id: impl Into<String>,
        matched_benchmark_name: impl Into<String>,
        similarity_score: f64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            incident_id: incident_id.into(),
            risk_tier,
            suspect_username: suspect_username.into(),
            suspect_user_id: suspect_user_id.into(),
            matched_benchmark_name: matched_benchmark_name.into(),
            similarity_score,
            reason: reason.into(),
            actions: vec![
                "Inspect".to_string(),
                "Dismiss".to_string(),
                "Ban & Purge".to_string(),
            ],
        }
    }

    /// Determines if this risk tier qualifies for high-priority desktop notification dispatch.
    /// Per Phase 17.2: Only `Elevated` and `Critical` discrepancies trigger OS notifications.
    pub fn should_dispatch(&self) -> bool {
        matches!(self.risk_tier, RiskTier::Elevated | RiskTier::Critical)
    }
}

/// Dispatches a native OS desktop notification toast.
/// Automatically verifies that the incident has `Elevated` or `Critical` risk tier.
pub fn dispatch_native_notification(
    app: Option<&tauri::AppHandle>,
    payload: &NotificationPayload,
) -> Result<bool, NotificationError> {
    if !payload.should_dispatch() {
        log::debug!(
            "Skipping notification for non-qualifying risk tier: {:?}",
            payload.risk_tier
        );
        return Ok(false);
    }

    // Always emit internal Tauri event so running webviews and toast overlays receive it
    if let Some(handle) = app {
        let _ = handle.emit("truthbeacon://native-notification", payload);
    }

    // Dispatch Native OS Desktop Notification Toast
    #[cfg(target_os = "macos")]
    {
        dispatch_macos_notification(payload)?;
    }

    #[cfg(target_os = "windows")]
    {
        dispatch_windows_notification(payload)?;
    }

    #[cfg(target_os = "linux")]
    {
        dispatch_linux_notification(payload)?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        log::info!(
            "[TruthBeacon Desktop Notification] [{:?}] Suspect @{} flagged against @{}",
            payload.risk_tier,
            payload.suspect_username,
            payload.matched_benchmark_name
        );
    }

    Ok(true)
}

#[cfg(target_os = "macos")]
fn dispatch_macos_notification(payload: &NotificationPayload) -> Result<(), NotificationError> {
    let title = match payload.risk_tier {
        RiskTier::Critical => "TruthBeacon Alert: Critical Imposter Flagged",
        RiskTier::Elevated => "TruthBeacon Alert: Elevated Discrepancy",
        _ => "TruthBeacon Notification",
    };

    let subtitle = format!(
        "{:.0}% Match with @{}",
        payload.similarity_score * 100.0,
        payload.matched_benchmark_name
    );

    let clean_suspect = payload.suspect_username.replace('"', "\\\"");
    let clean_reason = payload.reason.replace('"', "\\\"");
    let body = format!("@{} - {}", clean_suspect, clean_reason);

    // Escape backslashes and double quotes for AppleScript string literals
    let safe_title = title.replace('\\', "\\\\").replace('"', "\\\"");
    let safe_subtitle = subtitle.replace('\\', "\\\\").replace('"', "\\\"");
    let safe_body = body.replace('\\', "\\\\").replace('"', "\\\"");

    let script = format!(
        "display notification \"{}\" with title \"{}\" subtitle \"{}\" sound name \"default\"",
        safe_body, safe_title, safe_subtitle
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| NotificationError::OsError(e.to_string()))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        log::warn!(
            "osascript notification returned non-zero status: {}",
            err_msg
        );
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn dispatch_windows_notification(payload: &NotificationPayload) -> Result<(), NotificationError> {
    let title = match payload.risk_tier {
        RiskTier::Critical => "TruthBeacon: Critical Imposter Detected",
        RiskTier::Elevated => "TruthBeacon: Elevated Imposter Alert",
        _ => "TruthBeacon Alert",
    };

    let clean_title = payload.suspect_username.replace('"', "`\"");
    let clean_body = format!(
        "Suspect @{} matches @{} ({:.0}%). {}",
        clean_title,
        payload.matched_benchmark_name,
        payload.similarity_score * 100.0,
        payload.reason
    );

    let ps_script = format!(
        "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; \
         $template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
         $textNodes = $template.GetElementsByTagName('text'); \
         $textNodes.Item(0).AppendChild($template.CreateTextNode('{}')) > $null; \
         $textNodes.Item(1).AppendChild($template.CreateTextNode('{}')) > $null; \
         $notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('TruthBeacon'); \
         $notifier.Show([Windows.UI.Notifications.ToastNotification]::new($template));",
        title, clean_body
    );

    let _ = Command::new("powershell")
        .arg("-Command")
        .arg(&ps_script)
        .spawn();

    Ok(())
}

#[cfg(target_os = "linux")]
fn dispatch_linux_notification(payload: &NotificationPayload) -> Result<(), NotificationError> {
    let title = match payload.risk_tier {
        RiskTier::Critical => "TruthBeacon Alert: Critical Imposter Flagged",
        RiskTier::Elevated => "TruthBeacon Alert: Elevated Discrepancy",
        _ => "TruthBeacon Notification",
    };

    let urgency = match payload.risk_tier {
        RiskTier::Critical => "critical",
        _ => "normal",
    };

    let body = format!(
        "Suspect @{} matches @{} ({:.0}%): {}",
        payload.suspect_username,
        payload.matched_benchmark_name,
        payload.similarity_score * 100.0,
        payload.reason
    );

    let _ = Command::new("notify-send")
        .arg("--urgency")
        .arg(urgency)
        .arg("--app-name")
        .arg("TruthBeacon")
        .arg(title)
        .arg(&body)
        .spawn();

    Ok(())
}

/// Executes one of the three standardized action buttons from a notification:
/// 1. "Inspect" - Focuses and opens main console on the incident
/// 2. "Dismiss" - Marks incident as benign/resolved with immutable audit log entry
/// 3. "Ban & Purge" - Bans suspect from server with message pruning and audit logging
pub fn execute_notification_action(
    app: &tauri::AppHandle,
    incident_id: &str,
    action: &str,
) -> Result<bool, NotificationError> {
    match action {
        "Inspect" => {
            // Unminimize, show, and focus main window
            crate::tray::show_main_window(app);
            // Notify frontend to switch to triage tab and focus card
            let _ = app.emit(
                "truthbeacon://inspect-incident",
                serde_json::json!({
                    "incident_id": incident_id
                }),
            );
            Ok(true)
        }
        "Dismiss" => {
            crate::commands::resolve_incident(
                incident_id.to_string(),
                IncidentStatus::Dismissed,
                Some("Dismissed via desktop notification toast action button".to_string()),
                Some("NotificationToast".to_string()),
                None,
            )
            .map_err(|e| NotificationError::CommandError(e.to_string()))?;

            let _ = app.emit(
                "truthbeacon://incident-resolved",
                serde_json::json!({
                    "incident_id": incident_id,
                    "action": "dismiss"
                }),
            );
            Ok(true)
        }
        "Ban & Purge" => {
            crate::commands::resolve_incident(
                incident_id.to_string(),
                IncidentStatus::Banned,
                Some("Banned and purged via desktop notification toast action button".to_string()),
                Some("NotificationToast".to_string()),
                None,
            )
            .map_err(|e| NotificationError::CommandError(e.to_string()))?;

            let _ = app.emit(
                "truthbeacon://incident-resolved",
                serde_json::json!({
                    "incident_id": incident_id,
                    "action": "ban"
                }),
            );
            Ok(true)
        }
        unknown => Err(NotificationError::UnknownAction(unknown.to_string())),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_notification_payload_creation_and_action_buttons() {
        let payload = NotificationPayload::new(
            "inc_test_101",
            RiskTier::Critical,
            "Pastor_Dan",
            "998877665544332211",
            "PastorDan",
            0.98,
            "Lookalike homoglyph substitution detected",
        );

        assert_eq!(payload.incident_id, "inc_test_101");
        assert_eq!(payload.risk_tier, RiskTier::Critical);
        assert_eq!(payload.suspect_username, "Pastor_Dan");
        assert_eq!(payload.matched_benchmark_name, "PastorDan");
        assert_eq!(payload.similarity_score, 0.98);

        // Verify the 3 required action buttons (Phase 17.2)
        assert_eq!(payload.actions.len(), 3);
        assert_eq!(payload.actions[0], "Inspect");
        assert_eq!(payload.actions[1], "Dismiss");
        assert_eq!(payload.actions[2], "Ban & Purge");
    }

    #[test]
    fn test_elevated_and_critical_dispatch_policy() {
        let critical_payload = NotificationPayload::new(
            "inc_crit",
            RiskTier::Critical,
            "AttackerA",
            "123",
            "Leader",
            0.95,
            "Clone",
        );
        let elevated_payload = NotificationPayload::new(
            "inc_elev",
            RiskTier::Elevated,
            "AttackerB",
            "456",
            "Leader",
            0.88,
            "High similarity",
        );
        let notable_payload = NotificationPayload::new(
            "inc_notable",
            RiskTier::Notable,
            "AttackerC",
            "789",
            "Leader",
            0.78,
            "Notable overlap",
        );
        let standard_payload = NotificationPayload::new(
            "inc_std",
            RiskTier::Standard,
            "AttackerD",
            "000",
            "Leader",
            0.40,
            "Benign",
        );

        // Critical and Elevated MUST dispatch desktop notifications
        assert!(critical_payload.should_dispatch());
        assert!(elevated_payload.should_dispatch());

        // Standard and Notable must NOT trigger noisy desktop notifications
        assert!(!notable_payload.should_dispatch());
        assert!(!standard_payload.should_dispatch());
    }

    #[test]
    fn test_notification_payload_json_serialization() {
        let payload = NotificationPayload::new(
            "inc_json_test",
            RiskTier::Critical,
            "PastorDan_Alt",
            "111222333",
            "PastorDan",
            0.96,
            "Homoglyph detected",
        );

        let json_str = serde_json::to_string(&payload).unwrap();
        assert!(json_str.contains("\"actions\":[\"Inspect\",\"Dismiss\",\"Ban & Purge\"]"));
        assert!(json_str.contains("\"risk_tier\":\"critical\""));

        let deserialized: NotificationPayload = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized, payload);
    }
}
