use crate::project::Project;
use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub compiler: String,
    pub compiler_opts: Option<Vec<String>>,
    pub linker: String,
    pub linker_opts: Option<Vec<String>>,
    pub packer: String,
    pub packer_opts: Option<Vec<String>>,
    pub bin: String,
    pub obj: String,
}

#[derive(Debug, Deserialize)]
pub struct BuildConfig {
    pub config: Config,
    #[serde(rename = "project")]
    pub projects: Vec<Project>,
}
