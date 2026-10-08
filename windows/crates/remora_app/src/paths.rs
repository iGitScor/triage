use std::path::PathBuf;

/// `%LOCALAPPDATA%\Remora` on Windows. Elsewhere (developing on a Mac or Linux) a folder of its own,
/// so a dev build never touches the macOS app's data.
pub fn data_dir() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir);
    if cfg!(windows) {
        base.join("Remora")
    } else {
        base.join("Remora for Windows (dev)")
    }
}

/// The credential store entry holding every account's secrets, as one JSON value.
pub const VAULT_SERVICE: &str = if cfg!(windows) { "fr.igitscor.remora" } else { "fr.igitscor.remora.windows-dev" };
pub const VAULT_ACCOUNT: &str = "secrets";
