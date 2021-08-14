use serde_derive::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Serialize, Deserialize)]
struct CacheData {
    files: HashMap<String, u64>,
}

pub struct Cache {
    path: PathBuf,
    data: CacheData,
}

impl Cache {
    pub fn new() -> Result<Self, std::io::Error> {
        let mut hasher = DefaultHasher::new();
        std::fs::canonicalize("build.toml")
            .unwrap()
            .hash(&mut hasher);

        let cache_file = std::env::temp_dir().join(format!("buldr_{}", hasher.finish()));

        let data = if cache_file.exists() {
            toml::from_str::<CacheData>(&std::fs::read_to_string(&cache_file)?)?
        } else {
            CacheData {
                files: HashMap::new(),
            }
        };

        Ok(Cache {
            path: cache_file,
            data,
        })
    }

    pub fn has_changed(&mut self, path: &Path, seconds: u64) -> bool {
        if !path.exists() {
            return true;
        }
        match self.data.files.get(path.to_str().unwrap()) {
            Some(last_write_time) => *last_write_time < seconds,
            None => true,
        }
    }

    pub fn update(&mut self, path: &Path) {
        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.data
            .files
            .insert(String::from(path.to_str().unwrap()), time);
    }

    pub fn write(&mut self) -> Result<(), std::io::Error> {
        let string_data = toml::to_string(&self.data).unwrap();
        std::fs::write(&self.path, string_data)
    }

    pub fn clean(&mut self) {
        if self.path.exists() {
            std::fs::remove_file(&self.path).unwrap();
        }
    }
}
