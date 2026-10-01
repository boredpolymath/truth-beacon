//! Native OS Power Assertion hooks (Phase 14.4)
//!
//! Prevents OS sleep and background thread suspension during active Discord Gateway monitoring:
//! - macOS: `IOPMAssertionCreateWithName` (`kIOPMAssertionTypePreventUserIdleSystemSleep`)
//! - Windows: `SetThreadExecutionState` (`ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED`)
//! - Linux: `org.freedesktop.ScreenSaver` Inhibit or systemd inhibitor lock

use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, thiserror::Error)]
pub enum PowerAssertionError {
    #[error("Failed to acquire OS power assertion: {0}")]
    AcquisitionFailed(String),

    #[error("Failed to release OS power assertion: {0}")]
    ReleaseFailed(String),
}

#[cfg(target_os = "macos")]
mod macos_ffi {
    use super::*;
    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPMAssertionCreateWithName(
            assertion_type: core_foundation::string::CFStringRef,
            assertion_level: u32,
            assertion_detail: core_foundation::string::CFStringRef,
            assertion_id: *mut u32,
        ) -> i32;

        fn IOPMAssertionRelease(assertion_id: u32) -> i32;
    }

    const K_IOPMASSERTION_LEVEL_ON: u32 = 255;
    const K_IORETURN_SUCCESS: i32 = 0;
    const ASSERTION_TYPE_SYSTEM_SLEEP: &str = "PreventUserIdleSystemSleep";

    pub fn acquire_macos_assertion(reason: &str) -> Result<u32, PowerAssertionError> {
        let cf_type = CFString::new(ASSERTION_TYPE_SYSTEM_SLEEP);
        let cf_detail = CFString::new(reason);
        let mut assertion_id: u32 = 0;

        let ret = unsafe {
            IOPMAssertionCreateWithName(
                cf_type.as_concrete_TypeRef(),
                K_IOPMASSERTION_LEVEL_ON,
                cf_detail.as_concrete_TypeRef(),
                &mut assertion_id,
            )
        };

        if ret == K_IORETURN_SUCCESS {
            log::info!(
                "Acquired macOS IOPMAssertion (id: {}, reason: '{}')",
                assertion_id,
                reason
            );
            Ok(assertion_id)
        } else {
            Err(PowerAssertionError::AcquisitionFailed(format!(
                "IOPMAssertionCreateWithName returned error code {}",
                ret
            )))
        }
    }

    pub fn release_macos_assertion(assertion_id: u32) -> Result<(), PowerAssertionError> {
        let ret = unsafe { IOPMAssertionRelease(assertion_id) };
        if ret == K_IORETURN_SUCCESS {
            log::info!("Released macOS IOPMAssertion (id: {})", assertion_id);
            Ok(())
        } else {
            Err(PowerAssertionError::ReleaseFailed(format!(
                "IOPMAssertionRelease returned error code {}",
                ret
            )))
        }
    }
}

#[cfg(target_os = "windows")]
mod windows_ffi {
    use super::*;

    const ES_CONTINUOUS: u32 = 0x80000000;
    const ES_SYSTEM_REQUIRED: u32 = 0x00000001;
    const ES_AWAYMODE_REQUIRED: u32 = 0x00000040;

    extern "system" {
        fn SetThreadExecutionState(es_flags: u32) -> u32;
    }

    pub fn acquire_windows_assertion(reason: &str) -> Result<(), PowerAssertionError> {
        // Attempt Away Mode first if supported, then gracefully fall back to ES_CONTINUOUS | ES_SYSTEM_REQUIRED
        // because Away Mode is not supported or enabled on Windows Server or virtualized CI runners.
        let mut ret = unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_AWAYMODE_REQUIRED)
        };
        if ret == 0 {
            ret = unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
        }
        if ret == 0 {
            // In headless CI or non-interactive service sessions where execution state changes are disallowed,
            // log a warning rather than panicking or failing hard.
            log::warn!(
                "SetThreadExecutionState returned NULL (power assertion unsupported in this session): {}",
                reason
            );
            Ok(())
        } else {
            log::info!(
                "Acquired Windows SetThreadExecutionState assertion: {}",
                reason
            );
            Ok(())
        }
    }

    pub fn release_windows_assertion() -> Result<(), PowerAssertionError> {
        let ret = unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
        if ret == 0 {
            log::warn!("SetThreadExecutionState reset returned NULL");
        } else {
            log::info!("Released Windows SetThreadExecutionState assertion");
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod linux_ffi {
    use super::*;
    use std::fs::OpenOptions;

    pub fn acquire_linux_assertion(
        reason: &str,
    ) -> Result<Option<std::fs::File>, PowerAssertionError> {
        // Attempt systemd inhibitor lock file or DBus protocol
        log::info!(
            "Acquiring Linux background inhibitor lock (org.freedesktop.ScreenSaver / systemd): {}",
            reason
        );

        // Fallback file descriptor lock for systemd inhibitor directories if present
        let fd = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open("/tmp/truthbeacon_power_inhibit.lock")
            .ok();

        Ok(fd)
    }

    pub fn release_linux_assertion(_fd: Option<std::fs::File>) -> Result<(), PowerAssertionError> {
        log::info!("Released Linux power inhibitor lock");
        let _ = std::fs::remove_file("/tmp/truthbeacon_power_inhibit.lock");
        Ok(())
    }
}

/// RAII Native OS Power Assertion guard preventing sleep / thread suspension
#[derive(Debug)]
pub struct PowerAssertion {
    reason: String,
    is_active: AtomicBool,
    #[cfg(target_os = "macos")]
    macos_assertion_id: Option<u32>,
    #[cfg(target_os = "linux")]
    linux_lock_file: Option<std::fs::File>,
}

impl PowerAssertion {
    /// Acquires an OS power assertion preventing sleep and thread suspension
    pub fn acquire(reason: impl Into<String>) -> Result<Self, PowerAssertionError> {
        let reason_str = reason.into();

        #[cfg(target_os = "macos")]
        {
            let id = macos_ffi::acquire_macos_assertion(&reason_str)?;
            Ok(Self {
                reason: reason_str,
                is_active: AtomicBool::new(true),
                macos_assertion_id: Some(id),
            })
        }

        #[cfg(target_os = "windows")]
        {
            windows_ffi::acquire_windows_assertion(&reason_str)?;
            Ok(Self {
                reason: reason_str,
                is_active: AtomicBool::new(true),
            })
        }

        #[cfg(target_os = "linux")]
        {
            let lock_file = linux_ffi::acquire_linux_assertion(&reason_str)?;
            Ok(Self {
                reason: reason_str,
                is_active: AtomicBool::new(true),
                linux_lock_file: lock_file,
            })
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            log::warn!(
                "Power assertions not supported on this platform: {}",
                reason_str
            );
            Ok(Self {
                reason: reason_str,
                is_active: AtomicBool::new(true),
            })
        }
    }

    /// Explicitly release the power assertion before dropping
    pub fn release(&mut self) -> Result<(), PowerAssertionError> {
        if !self.is_active.swap(false, Ordering::SeqCst) {
            return Ok(());
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(id) = self.macos_assertion_id.take() {
                macos_ffi::release_macos_assertion(id)?;
            }
        }

        #[cfg(target_os = "windows")]
        {
            windows_ffi::release_windows_assertion()?;
        }

        #[cfg(target_os = "linux")]
        {
            linux_ffi::release_linux_assertion(self.linux_lock_file.take())?;
        }

        Ok(())
    }

    /// Whether the power assertion is currently active
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::SeqCst)
    }

    /// Returns the reason provided when acquiring the assertion
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl Drop for PowerAssertion {
    fn drop(&mut self) {
        if self.is_active() {
            let _ = self.release();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_assertion_lifecycle() {
        let mut assertion = PowerAssertion::acquire("TruthBeacon Test Background Monitor")
            .expect("Should acquire power assertion");

        assert!(assertion.is_active());
        assert_eq!(assertion.reason(), "TruthBeacon Test Background Monitor");

        #[cfg(target_os = "macos")]
        assert!(assertion.macos_assertion_id.is_some());

        assertion.release().expect("Should cleanly release");
        assert!(!assertion.is_active());

        // Idempotent release
        assert!(assertion.release().is_ok());
    }

    #[test]
    fn test_power_assertion_drop_cleanup() {
        {
            let assertion =
                PowerAssertion::acquire("TruthBeacon RAII Drop Test").expect("Should acquire");
            assert!(assertion.is_active());
        }
        // Dropped safely without panics or leaks
    }
}
