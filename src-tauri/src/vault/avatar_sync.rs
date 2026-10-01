//! Official Avatar Hash Synchronization & Periodic Refresh (Phase 12.4).
//!
//! Provides:
//! - **Background Avatar Fetcher**: Automatically downloads canonical member avatars on creation or update.
//! - **64-bit DCT Perceptual Hashing**: Computes and persists resilient visual fingerprints for clone detection.
//! - **Periodic Refresh Routine**: Queries Discord REST API periodically to detect and synchronize official avatar updates.

use crate::detection::perceptual_hash::compute_perceptual_hash;
use crate::models::{CanonicalBenchmark, UpdateBenchmarkInput};
use crate::vault::ingestion::{
    is_secure_endpoint, resolve_discord_avatar_url_with_cdn, IngestionClient, IngestionError,
};
use crate::vault::{VaultError, VaultManager};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::broadcast;

/// Domain errors encountered during avatar synchronization routines.
#[derive(Debug, thiserror::Error)]
pub enum AvatarSyncError {
    #[error("Vault error: {0}")]
    Vault(#[from] VaultError),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Ingestion error: {0}")]
    Ingestion(#[from] IngestionError),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Benchmark '{0}' not found")]
    NotFound(String),
}

/// Detailed audit report returned after synchronizing avatar hashes for a guild.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvatarSyncReport {
    pub guild_id: String,
    pub total_benchmarks_scanned: usize,
    pub updated_count: usize,
    pub unchanged_count: usize,
    pub failed_count: usize,
    pub updated_benchmarks: Vec<CanonicalBenchmark>,
}

/// Single benchmark avatar synchronization result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingleAvatarSyncResult {
    pub benchmark_id: String,
    pub avatar_url: Option<String>,
    pub avatar_perceptual_hash: Option<String>,
    pub changed: bool,
}

/// Computes the 64-bit DCT perceptual hash from image bytes directly.
pub fn compute_hash_from_bytes(bytes: &[u8]) -> Option<String> {
    compute_perceptual_hash(bytes).ok()
}

/// Downloads image bytes from the given URL and computes its 64-bit DCT perceptual hash.
pub async fn fetch_and_hash_avatar(client: &reqwest::Client, avatar_url: &str) -> Option<String> {
    if !is_secure_endpoint(avatar_url) {
        log::warn!("Rejected insecure cleartext avatar URL: {}", avatar_url);
        return None;
    }
    let res = client.get(avatar_url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    let bytes = res.bytes().await.ok()?;
    compute_hash_from_bytes(&bytes)
}

/// Synchronizes the avatar and DCT perceptual hash for a single benchmark profile.
///
/// Fetches the image at `avatar_url`, computes its 64-bit DCT perceptual hash, and updates
/// SQLite and the in-memory cache if the hash has changed or was previously missing.
pub async fn sync_single_benchmark_avatar(
    vault_mgr: &VaultManager,
    client: &reqwest::Client,
    benchmark_id: &str,
) -> Result<SingleAvatarSyncResult, AvatarSyncError> {
    let benchmark = vault_mgr
        .get_benchmark(benchmark_id)?
        .ok_or_else(|| AvatarSyncError::NotFound(benchmark_id.to_string()))?;

    let avatar_url = match benchmark.avatar_url {
        Some(ref url) if !url.trim().is_empty() => url.clone(),
        _ => {
            return Ok(SingleAvatarSyncResult {
                benchmark_id: benchmark.id,
                avatar_url: None,
                avatar_perceptual_hash: None,
                changed: false,
            });
        }
    };

    let new_hash = fetch_and_hash_avatar(client, &avatar_url).await;

    let changed = match (&benchmark.avatar_perceptual_hash, &new_hash) {
        (Some(old_h), Some(new_h)) => old_h != new_h,
        (None, Some(_)) => true,
        _ => false,
    };

    if changed {
        vault_mgr.update_benchmark(
            &benchmark.id,
            UpdateBenchmarkInput {
                avatar_perceptual_hash: new_hash.clone(),
                ..Default::default()
            },
        )?;
    }

    Ok(SingleAvatarSyncResult {
        benchmark_id: benchmark.id,
        avatar_url: Some(avatar_url),
        avatar_perceptual_hash: new_hash,
        changed,
    })
}

/// Spawns an asynchronous background task to fetch and hash a benchmark avatar (fire-and-forget).
pub fn spawn_background_avatar_fetch(
    vault_mgr: VaultManager,
    benchmark_id: String,
    avatar_url: String,
) {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        if let Some(hash) = fetch_and_hash_avatar(&client, &avatar_url).await {
            let _ = vault_mgr.update_benchmark(
                &benchmark_id,
                UpdateBenchmarkInput {
                    avatar_perceptual_hash: Some(hash),
                    ..Default::default()
                },
            );
        }
    });
}

/// Periodic refresh routine to detect official avatar updates across all active benchmarks in a guild.
///
/// 1. Queries Discord REST API for the current live member or user profile.
/// 2. Resolves current avatar URL from Discord CDN.
/// 3. Detects if the official account has updated their avatar image.
/// 4. Re-computes 64-bit DCT perceptual hash.
/// 5. Writes changes to SQLite and updates the in-memory cache.
/// 6. Records an audit log entry in `audit_logs` table (`action = "AVATAR_HASH_SYNCHRONIZED"`).
pub async fn sync_guild_avatar_hashes(
    vault_mgr: &VaultManager,
    ingestion_client: &IngestionClient,
    bot_token: &str,
    guild_id: &str,
) -> Result<AvatarSyncReport, AvatarSyncError> {
    let benchmarks = vault_mgr.list_benchmarks(guild_id, true)?;
    let mut updated_benchmarks = Vec::new();
    let mut updated_count = 0;
    let mut unchanged_count = 0;
    let mut failed_count = 0;

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    for bm in benchmarks {
        let snowflake_num: u64 = bm.user_id.trim().parse().unwrap_or(0);
        let target_snowflake = if snowflake_num > 0 {
            snowflake_num.to_string()
        } else {
            bm.user_id.clone()
        };

        // Query live Discord member profile
        let live_member_res = ingestion_client
            .fetch_guild_member(bot_token, guild_id, &target_snowflake)
            .await;

        let live_avatar_url = match live_member_res {
            Ok(member) => resolve_discord_avatar_url_with_cdn(
                ingestion_client.cdn_base_url(),
                &member.user,
                Some(guild_id),
                member.avatar.as_deref(),
            ),
            Err(_) => {
                // Fallback to global user query
                match ingestion_client.fetch_user(bot_token, &target_snowflake).await {
                    Ok(user) => resolve_discord_avatar_url_with_cdn(
                        ingestion_client.cdn_base_url(),
                        &user,
                        None,
                        None,
                    ),
                    Err(_) => {
                        failed_count += 1;
                        continue;
                    }
                }
            }
        };

        let new_url = match live_avatar_url {
            Some(url) if !url.trim().is_empty() => url,
            _ => {
                unchanged_count += 1;
                continue;
            }
        };

        let url_changed = bm.avatar_url.as_deref() != Some(&new_url);
        let hash_missing = bm.avatar_perceptual_hash.is_none();

        if url_changed || hash_missing {
            // Fetch live avatar bytes and calculate 64-bit DCT perceptual hash
            if let Some(new_hash) = fetch_and_hash_avatar(&http_client, &new_url).await {
                let updated = vault_mgr.update_benchmark(
                    &bm.id,
                    UpdateBenchmarkInput {
                        avatar_url: Some(new_url.clone()),
                        avatar_perceptual_hash: Some(new_hash.clone()),
                        ..Default::default()
                    },
                )?;

                // Record audit log entry in SQLite
                let conn = vault_mgr.storage().get_connection();
                let conn = conn.lock().unwrap();
                let now = chrono::Utc::now().timestamp();
                let audit_id = format!("audit_avsync_{}_{}", now, bm.id);
                let meta = serde_json::json!({
                    "action": "AVATAR_HASH_SYNCHRONIZED",
                    "benchmark_id": bm.id,
                    "user_id": bm.user_id,
                    "old_avatar_url": bm.avatar_url,
                    "new_avatar_url": new_url,
                    "old_hash": bm.avatar_perceptual_hash,
                    "new_hash": new_hash,
                })
                .to_string();

                let _ = conn.execute(
                    "INSERT INTO audit_logs (
                        id, timestamp, action, guild_id, operator_id, target_user_id,
                        reason, metadata_json
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);",
                    params![
                        audit_id,
                        now,
                        "AVATAR_HASH_SYNCHRONIZED",
                        guild_id,
                        "SYSTEM_AVATAR_SYNCHRONIZER",
                        bm.user_id,
                        "Automatic periodic synchronization detected official avatar asset update",
                        meta,
                    ],
                );

                updated_benchmarks.push(updated);
                updated_count += 1;
            } else {
                failed_count += 1;
            }
        } else {
            unchanged_count += 1;
        }
    }

    Ok(AvatarSyncReport {
        guild_id: guild_id.to_string(),
        total_benchmarks_scanned: updated_count + unchanged_count + failed_count,
        updated_count,
        unchanged_count,
        failed_count,
        updated_benchmarks,
    })
}

/// Spawns a recurring background periodic refresh loop for avatar hash synchronization.
///
/// Returns a broadcast sender to send a cancellation signal when shutting down.
pub fn start_periodic_avatar_refresh(
    vault_mgr: VaultManager,
    ingestion_client: IngestionClient,
    bot_token: String,
    guild_id: String,
    interval: Duration,
) -> broadcast::Sender<()> {
    let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        // Tick once immediately to advance ticker past startup
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break;
                }
                _ = ticker.tick() => {
                    let _ = sync_guild_avatar_hashes(
                        &vault_mgr,
                        &ingestion_client,
                        &bot_token,
                        &guild_id,
                    ).await;
                }
            }
        }
    });

    shutdown_tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::CreateBenchmarkInput;

    /// Helper creating a minimal valid PNG image buffer with distinct spatial patterns.
    fn create_mock_pattern_png(pattern_type: u8) -> Vec<u8> {
        let mut img = image::RgbImage::new(16, 16);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            let val = match pattern_type {
                0 => {
                    if (x + y) % 2 == 0 {
                        255
                    } else {
                        0
                    }
                } // high-freq checkerboard
                1 => {
                    if x < 8 {
                        255
                    } else {
                        0
                    }
                } // vertical step edge
                2 => {
                    if y < 8 {
                        255
                    } else {
                        0
                    }
                } // horizontal step edge
                _ => ((x * 16) ^ (y * 16)) as u8, // diagonal pattern
            };
            *pixel = image::Rgb([val, val, val]);
        }
        let mut cursor = std::io::Cursor::new(Vec::new());
        img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
        cursor.into_inner()
    }

    #[test]
    fn test_compute_hash_from_mock_image_bytes() {
        let png_a = create_mock_pattern_png(0);
        let png_b = create_mock_pattern_png(1);

        let hash_a = compute_hash_from_bytes(&png_a);
        let hash_b = compute_hash_from_bytes(&png_b);

        assert!(hash_a.is_some());
        assert!(hash_b.is_some());
        assert_ne!(hash_a, hash_b);

        // Verify hex formatting (64-bit hex is 16 chars)
        let a_str = hash_a.unwrap();
        assert_eq!(a_str.len(), 16);
        assert!(u64::from_str_radix(&a_str, 16).is_ok());
    }

    #[tokio::test]
    async fn test_sync_single_benchmark_avatar_offline_mock() {
        let vault_mgr = VaultManager::in_memory().unwrap();
        let png_bytes = create_mock_pattern_png(0);

        // Spawn mock HTTP server serving mock PNG bytes
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            tokio::select! {
                _ = &mut shutdown_rx => {}
                res = listener.accept() => {
                    if let Ok((mut socket, _)) = res {
                        use tokio::io::AsyncWriteExt;
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            png_bytes.len()
                        );
                        let mut full = header.into_bytes();
                        full.extend_from_slice(&png_bytes);
                        let _ = socket.write_all(&full).await;
                    }
                }
            }
        });

        let avatar_url = format!("http://{}/avatar.png", addr);

        // Create benchmark without hash
        let bm = vault_mgr
            .create_benchmark(
                CreateBenchmarkInput {
                    guild_id: "guild_av_test".into(),
                    user_id: "12345".into(),
                    canonical_username: "Alice".into(),
                    server_nickname: None,
                    community_role: "Staff".into(),
                    avatar_url: Some(avatar_url),
                    tags: vec![],
                    sensitivity_override: None,
                },
                None,
            )
            .unwrap();

        assert!(bm.avatar_perceptual_hash.is_none());

        let client = reqwest::Client::new();
        let sync_result = sync_single_benchmark_avatar(&vault_mgr, &client, &bm.id)
            .await
            .unwrap();

        let _ = shutdown_tx.send(());

        assert!(sync_result.changed);
        assert!(sync_result.avatar_perceptual_hash.is_some());

        // Verify updated in vault cache and SQLite
        let updated_bm = vault_mgr.get_benchmark(&bm.id).unwrap().unwrap();
        assert_eq!(
            updated_bm.avatar_perceptual_hash,
            sync_result.avatar_perceptual_hash
        );
    }

    #[tokio::test]
    async fn test_sync_guild_avatar_hashes_detects_update_and_records_audit() {
        let vault_mgr = VaultManager::in_memory().unwrap();
        let guild_id = "guild_sync_refresh";
        let user_id = "777000111";

        // Seed initial benchmark with old avatar and old hash
        let initial_bm = vault_mgr
            .create_benchmark(
                CreateBenchmarkInput {
                    guild_id: guild_id.into(),
                    user_id: user_id.into(),
                    canonical_username: "Elder John".into(),
                    server_nickname: None,
                    community_role: "Leadership".into(),
                    avatar_url: Some("https://old.discord.com/avatar_old.png".into()),
                    tags: vec!["Core Staff".into()],
                    sensitivity_override: None,
                },
                Some("old_hash_0000000".into()),
            )
            .unwrap();

        let new_png_bytes = create_mock_pattern_png(1);

        // Setup mock server serving both Discord API and the new avatar image
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    res = listener.accept() => {
                        if let Ok((mut socket, _)) = res {
                            use tokio::io::{AsyncReadExt, AsyncWriteExt};
                            let mut buf = [0u8; 1024];
                            let n = socket.read(&mut buf).await.unwrap_or(0);
                            let req = String::from_utf8_lossy(&buf[..n]);

                            if req.contains("GET /guilds/") {
                                // Return member with new avatar hash 'new_avatar_asset'
                                let body = serde_json::json!({
                                    "user": {
                                        "id": user_id,
                                        "username": "elder_john",
                                        "discriminator": "0",
                                        "global_name": "Elder John",
                                        "avatar": "new_avatar_asset",
                                        "bot": false
                                    },
                                    "nick": null,
                                    "roles": []
                                }).to_string();

                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                    body.len(),
                                    body
                                );
                                let _ = socket.write_all(resp.as_bytes()).await;
                            } else {
                                // Return mock image
                                let header = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    new_png_bytes.len()
                                );
                                let mut full = header.into_bytes();
                                full.extend_from_slice(&new_png_bytes);
                                let _ = socket.write_all(&full).await;
                            }
                        }
                    }
                }
            }
        });

        let base_url = format!("http://{}", addr);
        let ingestion_client = IngestionClient::with_base_url(&base_url);

        let report =
            sync_guild_avatar_hashes(&vault_mgr, &ingestion_client, "Bot mock_token", guild_id)
                .await
                .unwrap();

        let _ = shutdown_tx.send(());

        assert_eq!(report.total_benchmarks_scanned, 1);
        assert_eq!(report.updated_count, 1);
        assert_eq!(report.unchanged_count, 0);

        // Verify benchmark updated in SQLite
        let updated = vault_mgr.get_benchmark(&initial_bm.id).unwrap().unwrap();
        assert_ne!(
            updated.avatar_perceptual_hash.as_deref(),
            Some("old_hash_0000000")
        );
        assert!(updated.avatar_url.unwrap().contains("new_avatar_asset"));

        // Verify audit log record
        let conn = vault_mgr.storage().get_connection();
        let conn = conn.lock().unwrap();
        let (action, reason): (String, String) = conn
            .query_row(
                "SELECT action, reason FROM audit_logs WHERE target_user_id = ?1 ORDER BY timestamp DESC LIMIT 1",
                [user_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();

        assert_eq!(action, "AVATAR_HASH_SYNCHRONIZED");
        assert!(reason.contains("avatar"));
    }
}
