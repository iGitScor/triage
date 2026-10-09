use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::{fs, io};

/// JSON files in one folder: `accounts.json`, `states.json`… Written to a temporary file then renamed,
/// so a crash never leaves half a file.
#[derive(Clone, Debug)]
pub struct JsonStore {
    dir: PathBuf,
}

impl JsonStore {
    pub fn new(dir: PathBuf) -> Self {
        JsonStore { dir }
    }

    pub fn load<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        let text = fs::read_to_string(self.path(name)).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save<T: Serialize>(&self, name: &str, value: &T) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let temp = self.dir.join(format!(".{name}.json.tmp"));
        fs::write(&temp, serde_json::to_vec_pretty(value)?)?;
        fs::rename(temp, self.path(name))
    }

    /// Deletes the files Remora wrote (its JSON files), then the folder if nothing else is in it.
    /// Never a whole folder: whatever else is there stays.
    pub fn erase(&self) -> io::Result<()> {
        for file in json_files(&self.dir)? {
            fs::remove_file(file)?;
        }
        let _ = fs::remove_dir(&self.dir);
        Ok(())
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }
}

/// Remora's own files in a folder: `name.json`, and `.name.json.tmp` left by an interrupted save.
/// A missing folder has none.
pub(crate) fn json_files(dir: &Path) -> io::Result<impl Iterator<Item = PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(e),
    };
    Ok(entries.into_iter().map(|e| e.path()).filter(|p| {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        p.is_file() && (name.ends_with(".json") || name.ends_with(".json.tmp"))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_loads_and_erases() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonStore::new(dir.path().join("Remora"));
        assert_eq!(store.load::<Vec<u8>>("states"), None);
        store.save("states", &vec![1u8, 2]).unwrap();
        assert_eq!(store.load::<Vec<u8>>("states"), Some(vec![1, 2]));
        store.erase().unwrap();
        assert_eq!(store.load::<Vec<u8>>("states"), None);
        store.erase().unwrap();
    }

    #[test]
    fn erasing_never_deletes_files_it_did_not_write() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Remora");
        let store = JsonStore::new(folder.clone());
        store.save("accounts", &Vec::<u8>::new()).unwrap();
        fs::write(folder.join("Remora.exe"), "the installed app").unwrap();
        store.erase().unwrap();
        assert!(folder.join("Remora.exe").exists());
        assert!(!folder.join("accounts.json").exists());
    }
}
