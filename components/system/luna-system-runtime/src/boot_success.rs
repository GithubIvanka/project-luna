//! Final boot-success confirmation for `luna-system-runtime`.
//!
//! `luna-boot.efi` creates the persistent `LunaBootAttempt` marker in UEFI
//! NVRAM before `ExitBootServices`. After the runtime has successfully
//! initialized, userspace clears that marker through Linux `efivarfs`.

use std::fs;
use std::path::{Path, PathBuf};

const EFIVARS_DIR: &str = "/sys/firmware/efi/efivars";
const VARIABLE_NAME: &str = "LunaBootAttempt";
const GLOBAL_VARIABLE_VENDOR_GUID: &str = "8be4df61-93ca-11d2-aa0d-00e098032b8c";

/// Returns the Linux `efivarfs` path used by the bootloader's NVRAM marker.
pub fn marker_path() -> PathBuf {
    Path::new(EFIVARS_DIR).join(format!(
        "{VARIABLE_NAME}-{GLOBAL_VARIABLE_VENDOR_GUID}"
    ))
}

/// Confirms that the system runtime reached its success boundary.
///
/// A missing marker is already the desired steady state, so this function is
/// idempotent. A mounted `efivarfs` filesystem is required when the marker is
/// still present; failure is returned rather than silently claiming success.
pub fn confirm_boot_success() -> Result<(), String> {
    let path = marker_path();
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to clear UEFI LunaBootAttempt marker at {}: {error}",
            path.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::marker_path;

    #[test]
    fn marker_path_uses_global_variable_namespace() {
        assert_eq!(
            marker_path().to_str(),
            Some("/sys/firmware/efi/efivars/LunaBootAttempt-8be4df61-93ca-11d2-aa0d-00e098032b8c")
        );
    }
}
