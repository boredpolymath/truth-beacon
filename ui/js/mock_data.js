/**
 * TruthBeacon: Scaffolded Initial State & Mock Ground-Truth Data
 * Designed according to Orange Heart Industries SRS specifications.
 */

export const INITIAL_BENCHMARKS = [
  {
    id: "bm_pastor_dan",
    guild_id: "guild_crossroads_9921",
    user_id: "291039401928374829",
    canonical_username: "DanWard",
    server_nickname: "Pastor Dan (Lead)",
    community_role: "Server Owner / Executive Pastor",
    avatar_url: "https://images.unsplash.com/photo-1534528741775-53994a69daeb?w=150&auto=format&fit=crop&q=80",
    avatar_perceptual_hash: "a8f3b4c9e1d2f071",
    is_active: true,
    tags: ["Core Staff", "Public Speaker", "Verified VIP"],
    created_at: 1718000000,
    updated_at: 1718000000
  },
  {
    id: "bm_sarah_mod",
    guild_id: "guild_crossroads_9921",
    user_id: "192830491820491820",
    canonical_username: "SarahChen",
    server_nickname: "Sarah | Community Care",
    community_role: "Lead Moderator",
    avatar_url: "https://images.unsplash.com/photo-1494790108377-be9c29b29330?w=150&auto=format&fit=crop&q=80",
    avatar_perceptual_hash: "3c89f1a702b8d4e9",
    is_active: true,
    tags: ["Moderation Team", "Direct Message Escrow"],
    created_at: 1719000000,
    updated_at: 1719000000
  },
  {
    id: "bm_support_bot",
    guild_id: "guild_crossroads_9921",
    user_id: "991827364510293847",
    canonical_username: "CrossroadsHelpDesk",
    server_nickname: "Official Support Desk",
    community_role: "System Bot",
    avatar_url: "https://images.unsplash.com/photo-1618005182384-a83a8bd57fbe?w=150&auto=format&fit=crop&q=80",
    avatar_perceptual_hash: "ff00cc3399aa5511",
    is_active: true,
    tags: ["Official Service", "Automated Desk"],
    created_at: 1717000000,
    updated_at: 1717000000
  }
];

export const INITIAL_INCIDENTS = [
  {
    id: "inc_901",
    guild_id: "guild_crossroads_9921",
    timestamp: Date.now() - 1000 * 60 * 14, // 14 mins ago
    discrepancy: {
      matched_benchmark_id: "bm_pastor_dan",
      matched_benchmark_name: "DanWard",
      suspect_user_id: "128492019482710492",
      suspect_username: "Dan\u0428ard", // Cyrillic homoglyph for W
      suspect_nickname: "Pastor Dan (Lead)",
      suspect_avatar_url: "https://images.unsplash.com/photo-1534528741775-53994a69daeb?w=150&auto=format&fit=crop&q=80",
      suspect_account_age_hours: 2,
      string_similarity_score: 0.98,
      homoglyph_detected: true,
      normalized_diff: "Cyrillic Sha (Ш) substituted for Latin W; identical server nickname",
      avatar_hamming_distance: 1,
      risk_tier: "critical"
    },
    status: "pending",
    resolution_notes: null,
    operator_id: null,
    resolved_at: null
  },
  {
    id: "inc_902",
    guild_id: "guild_crossroads_9921",
    timestamp: Date.now() - 1000 * 60 * 55, // 55 mins ago
    discrepancy: {
      matched_benchmark_id: "bm_support_bot",
      matched_benchmark_name: "CrossroadsHelpDesk",
      suspect_user_id: "128492019482799999",
      suspect_username: "CrossroadsHelpDesk_",
      suspect_nickname: "Official Support Desk [VERIFIED]",
      suspect_avatar_url: "https://images.unsplash.com/photo-1618005182384-a83a8bd57fbe?w=150&auto=format&fit=crop&q=80",
      suspect_account_age_hours: 18,
      string_similarity_score: 0.94,
      homoglyph_detected: false,
      normalized_diff: "Trailing underscore typo-squat; unauthorized badge tag appended",
      avatar_hamming_distance: 3,
      risk_tier: "elevated"
    },
    status: "pending",
    resolution_notes: null,
    operator_id: null,
    resolved_at: null
  }
];

export const INITIAL_AUDIT_LOGS = [
  {
    id: "aud_01",
    timestamp: Date.now() - 1000 * 60 * 180,
    action: "ban_and_purge",
    guild_id: "guild_crossroads_9921",
    operator_id: "Staff_Lead",
    target_user_id: "128491000291029384",
    incident_id: "inc_899",
    reason: "Impersonation of Lead Moderator with direct message scam payload",
    metadata: { similarity: 0.99, age_hours: 1 }
  },
  {
    id: "aud_02",
    timestamp: Date.now() - 1000 * 60 * 320,
    action: "whitelist_alternate",
    guild_id: "guild_crossroads_9921",
    operator_id: "Pastor_Dan",
    target_user_id: "192837465019283746",
    incident_id: "inc_898",
    reason: "Authorized iPad testing account for media projection team",
    metadata: { tag: "Approved Alternate" }
  }
];
