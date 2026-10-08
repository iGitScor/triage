use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::PathBuf;
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

    /// Deletes every file Remora wrote.
    pub fn erase(&self) -> io::Result<()> {
        match fs::remove_dir_all(&self.dir) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }
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
}
