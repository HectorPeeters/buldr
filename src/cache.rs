use serde_derive::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Serialize, Deserialize)]
struct CacheData {
    // Map which contains the last compiled time (secs since epoch) of each file in the project
    files: HashMap<String, u64>,
}

pub struct Cache {
    path: PathBuf,
    data: CacheData,
}

impl Cache {
    pub fn new(build_file: &str) -> Result<Self, std::io::Error> {
        // Create a new hasher
        let mut hasher = DefaultHasher::new();
        // Hash the full path of the build.toml file. This will be used as a unique identifier for
        // the cache file.
        std::fs::canonicalize(build_file)
            .unwrap()
            .hash(&mut hasher);

        // Create the cache file the temp directory
        let cache_file = std::env::temp_dir().join(format!("buldr_{}", hasher.finish()));

        let data = if cache_file.exists() {
            // If the cache file exist load the data from there
            toml::from_str::<CacheData>(&std::fs::read_to_string(&cache_file)?)?
        } else {
            // If the cache file doesn't exist, create ana empty one
            CacheData {
                files: HashMap::new(),
            }
        };

        Ok(Cache {
            path: cache_file,
            data,
        })
    }

    pub fn has_changed(&mut self, path: &Path, time: &SystemTime) -> bool {
        // If the file doesn't exist we have to recompile anyway
        if !path.exists() {
            return true;
        }

        let seconds = time
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        match self.data.files.get(path.to_str().unwrap()) {
            // It's stored in the cache so lets see if its up to date
            Some(last_write_time) => *last_write_time < seconds,
            // It's not even in the cache so lets recompile
            None => true,
        }
    }

    pub fn update(&mut self, path: &Path) {
        // Get the current time
        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Store this in the cache
        self.data
            .files
            .insert(String::from(path.to_str().unwrap()), time);
    }

    pub fn write(&mut self) -> Result<(), std::io::Error> {
        // Convert the cache date to a string
        let string_data = toml::to_string(&self.data).unwrap();

        // And write it to the cache file
        std::fs::write(&self.path, string_data)
    }

    pub fn clean(&mut self) {
        // If the file exists, remove it!
        if self.path.exists() {
            std::fs::remove_file(&self.path).unwrap();
        }
    }
}
