use std::path::{Path, PathBuf};
use std::{fs, io};

/// `%LOCALAPPDATA%\fr.igitscor.remora` on Windows: Tauri's app data folder, which the uninstaller's
/// "delete app data" option removes. Not `%LOCALAPPDATA%\Remora`: that is where the per-user installer puts
/// the app itself. Elsewhere (developing on a Mac or Linux) a folder of its own, so a dev build never touches
/// the macOS app's data.
pub fn data_dir() -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir);
    if cfg!(windows) {
        base.join("fr.igitscor.remora")
    } else {
        base.join("Remora for Windows (dev)")
    }
}

/// Where 0.3.0 saved its data, next to the installed app.
pub fn legacy_data_dir() -> Option<PathBuf> {
    cfg!(windows).then(|| dirs::data_local_dir().map(|d| d.join("Remora"))).flatten()
}

/// Moves Remora's JSON files from `from` to `to`, once: only when `to` has none yet. Nothing else in `from`
/// (the installed app) is touched.
pub fn migrate(from: &Path, to: &Path) -> io::Result<usize> {
    if crate::store::json_files(to)?.next().is_some() {
        return Ok(0);
    }
    let files: Vec<PathBuf> = crate::store::json_files(from)?.collect();
    if files.is_empty() {
        return Ok(0);
    }
    fs::create_dir_all(to)?;
    for file in &files {
        fs::rename(file, to.join(file.file_name().unwrap()))?;
    }
    Ok(files.len())
}

/// The credential store entry holding every account's secrets, as one JSON value.
pub const VAULT_SERVICE: &str = if cfg!(windows) { "fr.igitscor.remora" } else { "fr.igitscor.remora.windows-dev" };
pub const VAULT_ACCOUNT: &str = "secrets";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_only_json_files_and_only_once() {
        let dir = tempfile::tempdir().unwrap();
        let (old, new) = (dir.path().join("Remora"), dir.path().join("fr.igitscor.remora"));
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("accounts.json"), "[]").unwrap();
        fs::write(old.join("Remora.exe"), "app").unwrap();
        assert_eq!(migrate(&old, &new).unwrap(), 1);
        assert!(new.join("accounts.json").exists() && old.join("Remora.exe").exists());

        fs::write(old.join("states.json"), "{}").unwrap();
        assert_eq!(migrate(&old, &new).unwrap(), 0, "the new folder already has data");
        assert_eq!(migrate(&dir.path().join("nowhere"), &dir.path().join("other")).unwrap(), 0);
    }
}
