use crate::core::PersistentData;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Storage {
    path: PathBuf,
}

impl Storage {
    pub fn new(directory: PathBuf) -> Self {
        Self { path: directory.join("state.json") }
    }

    pub fn load(&self) -> PersistentData {
        match fs::read_to_string(&self.path) {
            Ok(input) => serde_json::from_str(&input).unwrap_or_else(|error| {
                eprintln!("Не удалось прочитать state.json: {error}");
                PersistentData::default()
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => PersistentData::default(),
            Err(error) => {
                eprintln!("Не удалось открыть state.json: {error}");
                PersistentData::default()
            }
        }
    }

    pub fn save(&self, data: &PersistentData) -> Result<(), String> {
        let directory = self.path.parent().ok_or("Нет папки данных приложения")?;
        fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let input = serde_json::to_vec_pretty(data).map_err(|error| error.to_string())?;
        atomic_write(&self.path, &input).map_err(|error| error.to_string())
    }
}

#[cfg(not(windows))]
fn atomic_write(path: &Path, input: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, input)?;
    fs::rename(temporary, path)
}

#[cfg(windows)]
fn atomic_write(path: &Path, input: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, input)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temporary, path)
}
