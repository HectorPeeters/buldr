use serde_derive::Serialize;
use std::path::PathBuf;
use std::process::Command;
use walkdir::DirEntry;

#[derive(Debug, Serialize)]
pub struct CompileCommand {
    pub directory: PathBuf,
    pub command: String,
    pub arguments: Vec<String>,
    pub file: String,
    #[serde(skip_serializing)]
    pub source_file: DirEntry,
}

impl CompileCommand {
    pub fn new(directory: PathBuf, command: &str, file: &str, source_file: &DirEntry) -> Self {
        Self {
            directory,
            command: command.to_string(),
            arguments: vec![],
            file: file.to_string(),
            source_file: source_file.clone(),
        }
    }

    pub fn push_arg(&mut self, arg: &str) {
        self.arguments.push(arg.to_string());
    }

    pub fn push_args(&mut self, args: &[&str]) {
        for arg in args {
            self.arguments.push(arg.to_string());
        }
    }

    pub fn execute(&mut self) -> Result<(), String> {
        let mut command = Command::new(&self.command);
        command.args(&self.arguments);

        let output = command.output().expect("Failed to execute compile command");
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8(output.stderr).unwrap())
        }
    }
}
