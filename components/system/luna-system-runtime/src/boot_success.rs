//! Final boot-success confirmation for `luna-system-runtime`.
//!
//! `luna-boot.efi` creates the persistent `LunaBootAttempt` marker in UEFI
//! NVRAM before `ExitBootServices`. After the runtime has successfully
//! initialized, userspace clears that marker through Linux `efivarfs`.

use std::fs;
use std::path::{Path, PathBuf};

const EFIVARS_DIR: &str = "/sys/firmware/efi/efivars";
const VARIABLE_NAME: &str = "LunaBootAttempt";
const LUNA_VARIABLE_VENDOR_GUID: &str = "9f6c5d8a-5f3b-4e24-8a3c-1d3f6e2b7c91";

/// Returns the Linux `efivarfs` path used by the bootloader's NVRAM marker.
pub fn marker_path() -> PathBuf {
    Path::new(EFIVARS_DIR).join(format!("{VARIABLE_NAME}-{LUNA_VARIABLE_VENDOR_GUID}"))
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
    fn marker_path_uses_luna_variable_namespace() {
        assert_eq!(
            marker_path().to_str(),
            Some("/sys/firmware/efi/efivars/LunaBootAttempt-9f6c5d8a-5f3b-4e24-8a3c-1d3f6e2b7c91")
        );
    }
}
