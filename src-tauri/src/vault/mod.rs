use crate::models::{CanonicalBenchmark, CreateBenchmarkInput};
use std::sync::{Arc, Mutex};

pub struct BenchmarkVault {
    benchmarks: Arc<Mutex<Vec<CanonicalBenchmark>>>,
}

impl Default for BenchmarkVault {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchmarkVault {
    pub fn new() -> Self {
        Self {
            benchmarks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn list(&self, guild_id: &str) -> Vec<CanonicalBenchmark> {
        let list = self.benchmarks.lock().unwrap();
        list.iter()
            .filter(|b| b.guild_id == guild_id && b.is_active)
            .cloned()
            .collect()
    }

    pub fn add(
        &self,
        input: CreateBenchmarkInput,
        avatar_hash: Option<String>,
    ) -> CanonicalBenchmark {
        let now = chrono::Utc::now().timestamp();
        let benchmark = CanonicalBenchmark {
            id: format!("bm_{}", now),
            guild_id: input.guild_id,
            user_id: input.user_id,
            canonical_username: input.canonical_username,
            server_nickname: input.server_nickname,
            community_role: input.community_role,
            avatar_url: input.avatar_url,
            avatar_perceptual_hash: avatar_hash,
            is_active: true,
            tags: input.tags,
            created_at: now,
            updated_at: now,
        };

        let mut list = self.benchmarks.lock().unwrap();
        list.push(benchmark.clone());
        benchmark
    }

    pub fn remove(&self, id: &str) -> bool {
        let mut list = self.benchmarks.lock().unwrap();
        if let Some(pos) = list.iter().position(|b| b.id == id) {
            list.remove(pos);
            true
        } else {
            false
        }
    }
}
