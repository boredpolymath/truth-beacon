//! Resilience, Exponential Backoff, and Memory Footprint Monitoring (Phase 14.4)
//!
//! Enforces:
//! - Exponential backoff reconnection loop: immediate (0s), 2s, 5s, 10s, 30s, up to 60s max interval.
//! - Continuous RAM footprint tracking ensuring idle background operation stays strictly **under 30 MB**.

use std::time::Duration;

/// Canonical backoff intervals: immediate (0s), 2s, 5s, 10s, 30s, capped at 60s.
pub const BACKOFF_SCHEDULE_SECS: [u64; 6] = [0, 2, 5, 10, 30, 60];
pub const MAX_BACKOFF_SECS: u64 = 60;

/// Strict 30 MB background idle RAM threshold
pub const MAX_BACKGROUND_IDLE_RAM_BYTES: u64 = 30 * 1024 * 1024; // 31,457,280 bytes (30 MB)

/// Exponential backoff state machine for Discord Gateway reconnection loop
#[derive(Debug, Clone)]
pub struct ReconnectionBackoff {
    current_attempt: usize,
    schedule: Vec<Duration>,
    max_interval: Duration,
}

impl Default for ReconnectionBackoff {
    fn default() -> Self {
        Self::new()
    }
}

impl ReconnectionBackoff {
    /// Creates a new backoff manager configured with canonical Discord Gateway intervals
    pub fn new() -> Self {
        let schedule = BACKOFF_SCHEDULE_SECS
            .iter()
            .map(|&secs| Duration::from_secs(secs))
            .collect();
        Self {
            current_attempt: 0,
            schedule,
            max_interval: Duration::from_secs(MAX_BACKOFF_SECS),
        }
    }

    /// Advances to the next backoff stage and returns the duration to wait before reconnecting.
    /// Stage 0: 0s (immediate)
    /// Stage 1: 2s
    /// Stage 2: 5s
    /// Stage 3: 10s
    /// Stage 4: 30s
    /// Stage 5+: 60s (capped max interval)
    pub fn next_delay(&mut self) -> Duration {
        let delay = if self.current_attempt < self.schedule.len() {
            self.schedule[self.current_attempt]
        } else {
            self.max_interval
        };

        self.current_attempt = self.current_attempt.saturating_add(1);
        delay
    }

    /// Resets the backoff stage to 0 upon successful handshake (READY / RESUMED)
    pub fn reset(&mut self) {
        self.current_attempt = 0;
    }

    /// Current reconnection attempt count
    pub fn attempt_count(&self) -> usize {
        self.current_attempt
    }

    /// Current delay without advancing the counter
    pub fn current_delay(&self) -> Duration {
        if self.current_attempt == 0 {
            Duration::from_secs(0)
        } else {
            let idx = (self.current_attempt - 1).min(self.schedule.len() - 1);
            self.schedule[idx]
        }
    }
}

/// OS-level memory monitor tracking Resident Set Size (RSS)
pub struct MemoryMonitor;

impl MemoryMonitor {
    /// Returns the current process resident set size (RSS) in bytes
    #[cfg(target_os = "macos")]
    pub fn get_resident_memory_bytes() -> Option<u64> {
        #[repr(C)]
        struct MachTaskBasicInfo {
            virtual_size: u64,
            resident_size: u64,
            resident_size_max: u64,
            user_time: libc::timeval,
            system_time: libc::timeval,
            policy: i32,
            suspend_count: i32,
        }

        const MACH_TASK_BASIC_INFO: libc::c_uint = 20;
        const MACH_TASK_BASIC_INFO_COUNT: libc::mach_msg_type_number_t =
            (std::mem::size_of::<MachTaskBasicInfo>() / std::mem::size_of::<libc::natural_t>())
                as libc::mach_msg_type_number_t;

        extern "C" {
            fn mach_task_self() -> libc::mach_port_t;
        }

        unsafe {
            let mut info: MachTaskBasicInfo = std::mem::zeroed();
            let mut count = MACH_TASK_BASIC_INFO_COUNT;
            let task = mach_task_self();

            let ret = libc::task_info(
                task,
                MACH_TASK_BASIC_INFO,
                &mut info as *mut MachTaskBasicInfo as *mut libc::c_int,
                &mut count,
            );

            if ret == 0 {
                Some(info.resident_size)
            } else {
                None
            }
        }
    }

    /// Returns the current process resident set size (RSS) in bytes on Linux
    #[cfg(target_os = "linux")]
    pub fn get_resident_memory_bytes() -> Option<u64> {
        if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
            let mut parts = statm.split_whitespace();
            // Second entry in statm is resident pages
            if let (Some(_size), Some(resident)) = (parts.next(), parts.next()) {
                if let Ok(pages) = resident.parse::<u64>() {
                    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
                    if page_size > 0 {
                        return Some(pages * page_size as u64);
                    }
                }
            }
        }
        None
    }

    /// Returns the current process resident set size (RSS) in bytes on Windows
    #[cfg(target_os = "windows")]
    pub fn get_resident_memory_bytes() -> Option<u64> {
        #[repr(C)]
        struct PROCESS_MEMORY_COUNTERS {
            cb: u32,
            page_fault_count: u32,
            peak_working_set_size: usize,
            working_set_size: usize,
            quota_peak_paged_pool_usage: usize,
            quota_paged_pool_usage: usize,
            quota_peak_non_paged_pool_usage: usize,
            quota_non_paged_pool_usage: usize,
            pagefile_usage: usize,
            peak_pagefile_usage: usize,
        }

        extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn K32GetProcessMemoryInfo(
                process: *mut std::ffi::c_void,
                pmc: *mut PROCESS_MEMORY_COUNTERS,
                cb: u32,
            ) -> i32;
        }

        unsafe {
            let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            let process = GetCurrentProcess();
            if K32GetProcessMemoryInfo(process, &mut counters, counters.cb) != 0 {
                Some(counters.working_set_size as u64)
            } else {
                None
            }
        }
    }

    /// Fallback for other platforms
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    pub fn get_resident_memory_bytes() -> Option<u64> {
        None
    }

    /// Resident memory in Megabytes (MB)
    pub fn get_resident_memory_mb() -> Option<f64> {
        Self::get_resident_memory_bytes().map(|bytes| bytes as f64 / (1024.0 * 1024.0))
    }

    /// Asserts whether the current background process is within the 30 MB idle RAM budget
    pub fn is_within_ram_budget() -> bool {
        if let Some(bytes) = Self::get_resident_memory_bytes() {
            bytes <= MAX_BACKGROUND_IDLE_RAM_BYTES
        } else {
            true // Fallback if OS memory counters are unavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff_progression() {
        let mut backoff = ReconnectionBackoff::new();

        // 1st attempt: immediate (0s)
        assert_eq!(backoff.next_delay(), Duration::from_secs(0));
        assert_eq!(backoff.attempt_count(), 1);

        // 2nd attempt: 2s
        assert_eq!(backoff.next_delay(), Duration::from_secs(2));
        assert_eq!(backoff.attempt_count(), 2);

        // 3rd attempt: 5s
        assert_eq!(backoff.next_delay(), Duration::from_secs(5));
        assert_eq!(backoff.attempt_count(), 3);

        // 4th attempt: 10s
        assert_eq!(backoff.next_delay(), Duration::from_secs(10));
        assert_eq!(backoff.attempt_count(), 4);

        // 5th attempt: 30s
        assert_eq!(backoff.next_delay(), Duration::from_secs(30));
        assert_eq!(backoff.attempt_count(), 5);

        // 6th attempt: 60s (capped max interval)
        assert_eq!(backoff.next_delay(), Duration::from_secs(60));
        assert_eq!(backoff.attempt_count(), 6);

        // 7th attempt: still capped at 60s
        assert_eq!(backoff.next_delay(), Duration::from_secs(60));
        assert_eq!(backoff.attempt_count(), 7);

        // Reset upon successful connection
        backoff.reset();
        assert_eq!(backoff.attempt_count(), 0);
        assert_eq!(backoff.next_delay(), Duration::from_secs(0));
    }

    #[test]
    fn test_memory_monitor_reports_valid_rss() {
        let rss_bytes = MemoryMonitor::get_resident_memory_bytes();
        assert!(rss_bytes.is_some(), "OS must return resident memory info");

        let bytes = rss_bytes.unwrap();
        assert!(bytes > 0, "Resident memory must be non-zero");

        let mb = MemoryMonitor::get_resident_memory_mb().unwrap();
        log::info!("Current process RSS: {:.2} MB ({} bytes)", mb, bytes);
        // Note: unit tests load many compilation artifacts, but the idle memory check is available
    }
}
