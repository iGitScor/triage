use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::{fs, io, thread};

/// JSON files in one folder: `accounts.json`, `states.json`… Written to a temporary file then renamed,
/// so a crash never leaves half a file.
///
/// `save` only encodes: one background thread writes the files, in the order they were saved, so callers holding
/// the inbox lock never wait on the disk (PERF-07). A file whose content didn't change isn't written again.
#[derive(Clone, Debug)]
pub struct JsonStore {
    dir: PathBuf,
    writer: Arc<Writer>,
}

#[derive(Debug, Default)]
struct Writer {
    /// Started on the first save; `None` again once the thread is gone.
    queue: Mutex<Option<Sender<Job>>>,
    /// What each file holds, or will once the queue is written: an equal save is skipped.
    last: Mutex<HashMap<String, Vec<u8>>>,
}

enum Job {
    Write {
        name: String,
        bytes: Vec<u8>,
    },
    /// Answered once every write queued before it is done.
    Flush(Sender<()>),
}

impl JsonStore {
    pub fn new(dir: PathBuf) -> Self {
        JsonStore { dir, writer: Arc::default() }
    }

    /// Waits for the saves queued so far, so it reads what was last saved.
    pub fn load<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        self.flush();
        let bytes = fs::read(self.path(name)).ok()?;
        let value = serde_json::from_slice(&bytes).ok()?;
        self.writer.last.lock().unwrap().insert(name.to_string(), bytes);
        Some(value)
    }

    /// Encodes `value` as compact JSON (keys sorted, so the same data gives the same bytes) and queues the write,
    /// unless the file already holds exactly that. Disk errors are reported by the writer.
    pub fn save<T: Serialize>(&self, name: &str, value: &T) -> io::Result<()> {
        let bytes = serde_json::to_vec(&serde_json::to_value(value)?)?;
        {
            let mut last = self.writer.last.lock().unwrap();
            if last.get(name) == Some(&bytes) {
                return Ok(());
            }
            last.insert(name.to_string(), bytes.clone());
        }
        self.send(Job::Write { name: name.to_string(), bytes });
        Ok(())
    }

    /// Returns once every save queued before it is on disk: before erasing, before quitting, and in tests.
    pub fn flush(&self) {
        let (done, wait) = mpsc::channel();
        if self.send(Job::Flush(done)) {
            let _ = wait.recv();
        }
    }

    /// Deletes the files Remora wrote (its JSON files), then the folder if nothing else is in it.
    /// Never a whole folder: whatever else is there stays.
    pub fn erase(&self) -> io::Result<()> {
        self.flush();
        self.writer.last.lock().unwrap().clear();
        for file in json_files(&self.dir)? {
            fs::remove_file(file)?;
        }
        let _ = fs::remove_dir(&self.dir);
        Ok(())
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }

    /// False when there is no writer to send to (nothing saved yet, for a flush).
    fn send(&self, job: Job) -> bool {
        let mut queue = self.writer.queue.lock().unwrap();
        if queue.is_none() {
            if matches!(job, Job::Flush(_)) {
                return false;
            }
            *queue = Some(spawn_writer(self.dir.clone()));
        }
        queue.as_ref().is_some_and(|sender| sender.send(job).is_ok())
    }
}

/// The one thread that writes, so a later save of a file is never overwritten by an earlier one.
fn spawn_writer(dir: PathBuf) -> Sender<Job> {
    let (sender, jobs) = mpsc::channel::<Job>();
    thread::Builder::new()
        .name("remora-store".into())
        .spawn(move || {
            for job in jobs {
                match job {
                    Job::Write { name, bytes } => {
                        if let Err(error) = write(&dir, &name, &bytes) {
                            eprintln!("Remora: could not save {name}: {error}");
                        }
                    }
                    Job::Flush(done) => {
                        let _ = done.send(());
                    }
                }
            }
        })
        .expect("the store's writer thread starts");
    sender
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let temp = dir.join(format!(".{name}.json.tmp"));
    fs::write(&temp, bytes)?;
    fs::rename(temp, dir.join(format!("{name}.json")))
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
        store.flush();
        fs::write(folder.join("Remora.exe"), "the installed app").unwrap();
        store.erase().unwrap();
        assert!(folder.join("Remora.exe").exists());
        assert!(!folder.join("accounts.json").exists());
    }

    #[test]
    fn writes_compact_json_and_reads_the_pretty_files_of_earlier_versions() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Remora");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("states.json"), serde_json::to_vec_pretty(&HashMap::from([("a", 1)])).unwrap()).unwrap();
        let store = JsonStore::new(folder.clone());
        assert_eq!(store.load::<HashMap<String, u8>>("states"), Some(HashMap::from([("a".into(), 1)])));

        store.save("states", &HashMap::from([("b", 2), ("a", 1)])).unwrap();
        store.flush();
        assert_eq!(fs::read_to_string(folder.join("states.json")).unwrap(), r#"{"a":1,"b":2}"#);
        assert_eq!(
            store.load::<HashMap<String, u8>>("states"),
            Some(HashMap::from([("a".into(), 1), ("b".into(), 2)]))
        );
    }

    #[test]
    fn unchanged_data_is_not_written_again() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Remora");
        let store = JsonStore::new(folder.clone());
        store.save("states", &vec![1u8, 2]).unwrap();
        store.flush();
        // Changed behind the store's back: an equal save must leave it alone.
        fs::write(folder.join("states.json"), "[9]").unwrap();
        store.save("states", &vec![1u8, 2]).unwrap();
        store.flush();
        assert_eq!(fs::read_to_string(folder.join("states.json")).unwrap(), "[9]");

        store.save("states", &vec![3u8]).unwrap();
        store.flush();
        assert_eq!(fs::read_to_string(folder.join("states.json")).unwrap(), "[3]");
    }

    #[test]
    fn a_file_read_at_launch_is_not_rewritten_by_an_equal_save() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Remora");
        let first = JsonStore::new(folder.clone());
        first.save("accounts", &vec![1u8]).unwrap();
        first.flush();
        let store = JsonStore::new(folder.clone());
        assert_eq!(store.load::<Vec<u8>>("accounts"), Some(vec![1]));
        fs::write(folder.join("accounts.json"), "[ 1 ]").unwrap();
        store.save("accounts", &vec![1u8]).unwrap();
        store.flush();
        assert_eq!(fs::read_to_string(folder.join("accounts.json")).unwrap(), "[ 1 ]");
    }

    #[test]
    fn saves_land_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonStore::new(dir.path().join("Remora"));
        for n in 0..200u32 {
            store.save("states", &n).unwrap();
        }
        assert_eq!(store.load::<u32>("states"), Some(199));
    }

    #[test]
    fn saving_after_erase_writes_again() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonStore::new(dir.path().join("Remora"));
        store.save("states", &vec![1u8]).unwrap();
        store.erase().unwrap();
        store.save("states", &vec![1u8]).unwrap();
        assert_eq!(store.load::<Vec<u8>>("states"), Some(vec![1]));
    }
}
