/**
 * TruthBeacon: Scaffolded Initial State & Mock Ground-Truth Data
 * Designed according to Orange Heart Industries Human-Centered Stewardship ethos.
 */

export const INITIAL_BENCHMARKS = [
  {
    id: "bm_pastor_dan",
    guild_id: "guild_crossroads_9921",
    user_id: "291039401928374829",
    canonical_username: "DanWard",
    server_nickname: "Pastor Dan (Lead)",
    community_role: "Server Owner / Executive Pastor",
    avatar_url: "assets/avatars/pastor_dan.svg",
    avatar_perceptual_hash: "a8f3b4c9e1d2f071",
    is_active: true,
    tags: ["Core Staff", "Public Speaker", "Verified VIP"],
    authorized_alts: [
      { user_id: "192837465019283746", label: "Media Team iPad", note: "Approved sanctuary projection device" }
    ],
    created_at: 1718000000,
    updated_at: 1718000000
  },
  {
    id: "bm_sarah_mod",
    guild_id: "guild_crossroads_9921",
    user_id: "192830491820491820",
    canonical_username: "SarahChen",
    server_nickname: "Sarah | Community Care",
    community_role: "Lead Moderator & Stewardship Elder",
    avatar_url: "assets/avatars/sarah_mod.svg",
    avatar_perceptual_hash: "3c89f1a702b8d4e9",
    is_active: true,
    tags: ["Moderation Team", "Direct Message Escrow"],
    authorized_alts: [],
    created_at: 1719000000,
    updated_at: 1719000000
  },
  {
    id: "bm_support_bot",
    guild_id: "guild_crossroads_9921",
    user_id: "991827364510293847",
    canonical_username: "CrossroadsHelpDesk",
    server_nickname: "Official Support Desk",
    community_role: "Verified System Service",
    avatar_url: "assets/avatars/support_bot.svg",
    avatar_perceptual_hash: "ff00cc3399aa5511",
    is_active: true,
    tags: ["Official Service", "Automated Desk"],
    authorized_alts: [],
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
      suspect_raw_display: "DanШard",
      suspect_nickname: "Pastor Dan (Lead)",
      suspect_avatar_url: "assets/avatars/pastor_dan_alt.svg",
      suspect_account_age_hours: 2,
      string_similarity_score: 0.98,
      homoglyph_detected: true,
      homoglyph_char: "\u0428",
      homoglyph_target: "W",
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
    timestamp: Date.now() - 1000 * 60 * 52, // 52 mins ago
    discrepancy: {
      matched_benchmark_id: "bm_support_bot",
      matched_benchmark_name: "CrossroadsHelpDesk",
      suspect_user_id: "128492019482799999",
      suspect_username: "CrossroadsHelpDesk_",
      suspect_raw_display: "CrossroadsHelpDesk_",
      suspect_nickname: "Official Support Desk [VERIFIED]",
      suspect_avatar_url: "assets/avatars/support_bot.svg",
      suspect_account_age_hours: 18,
      string_similarity_score: 0.94,
      homoglyph_detected: false,
      homoglyph_char: null,
      homoglyph_target: null,
      normalized_diff: "Trailing underscore typo-squat; unauthorized badge tag appended",
      avatar_hamming_distance: 3,
      risk_tier: "elevated"
    },
    status: "pending",
    resolution_notes: null,
    operator_id: null,
    resolved_at: null
  },
  {
    id: "inc_903",
    guild_id: "guild_crossroads_9921",
    timestamp: Date.now() - 1000 * 60 * 110, // ~2 hours ago
    discrepancy: {
      matched_benchmark_id: "bm_sarah_mod",
      matched_benchmark_name: "SarahChen",
      suspect_user_id: "128492019482788888",
      suspect_username: "Sarah_Chen_Care",
      suspect_raw_display: "Sarah_Chen_Care",
      suspect_nickname: "Sarah | Community Care",
      suspect_avatar_url: "assets/avatars/sarah_mod.svg",
      suspect_account_age_hours: 1,
      string_similarity_score: 0.96,
      homoglyph_detected: false,
      homoglyph_char: null,
      homoglyph_target: null,
      normalized_diff: "Display name copy with exact official photo match",
      avatar_hamming_distance: 1,
      risk_tier: "critical"
    },
    status: "pending",
    resolution_notes: null,
    operator_id: null,
    resolved_at: null
  },
  {
    id: "inc_904",
    guild_id: "guild_crossroads_9921",
    timestamp: Date.now() - 1000 * 60 * 240, // 4 hours ago
    discrepancy: {
      matched_benchmark_id: "bm_pastor_dan",
      matched_benchmark_name: "DanWard",
      suspect_user_id: "128492019482777777",
      suspect_username: "DanWard_Official",
      suspect_raw_display: "DanWard_Official",
      suspect_nickname: "Pastor Dan (Lead)",
      suspect_avatar_url: "assets/avatars/pastor_dan_alt.svg",
      suspect_account_age_hours: 5,
      string_similarity_score: 0.88,
      homoglyph_detected: false,
      homoglyph_char: null,
      homoglyph_target: null,
      normalized_diff: "Appended official suffix and identical nickname",
      avatar_hamming_distance: 12,
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
    action: "adjudicate_impersonation",
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
    action: "authorize_alternate",
    guild_id: "guild_crossroads_9921",
    operator_id: "Pastor_Dan",
    target_user_id: "192837465019283746",
    incident_id: "inc_898",
    reason: "Authorized iPad testing account for media projection team",
    metadata: { tag: "Approved Alternate", benchmark: "DanWard" }
  }
];
