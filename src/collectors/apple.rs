//! Apple Silicon collector. Implemented Day 8-9.

#[cfg(target_os = "macos")]
pub fn poll() -> Option<crate::collectors::HardwareSnapshot> {
    None
}
