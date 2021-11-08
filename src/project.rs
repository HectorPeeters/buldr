use crate::compile_command::CompileCommand;
use crate::config::Config;
use crate::Cache;
use indicatif::{ProgressBar, ProgressStyle};
use serde_derive::Deserialize;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use termion::color;
use walkdir::DirEntry;
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Executable,
    Library,
}

#[derive(Debug, Deserialize)]
pub struct Project {
    pub name: String,
    pub kind: ProjectKind,
    pub src: Vec<PathBuf>,
    pub extensions: Option<Vec<String>>,
    pub include: Option<Vec<String>>,
    pub links: Option<Vec<String>>,
    pub defines: Option<Vec<String>>,
    pub depends: Option<Vec<String>>,
    pub default: Option<bool>,
}

impl Project {
    fn get_output_file(&self, path: &Path, config: &Config) -> PathBuf {
        let mut output_file = Path::new(&config.obj).join(&self.name).join(path);
        output_file.set_extension("o");
        output_file
    }

    fn is_valid_file(file_name: &OsStr, supported_types: &Option<Vec<String>>) -> bool {
        match supported_types {
            Some(supported_types) => {
                let string_name = file_name.to_str().unwrap();
                for supported in supported_types {
                    if string_name.ends_with(supported) {
                        return true;
                    }
                }

                false
            }
            None => true,
        }
    }

    pub fn get_source_files(&self) -> Vec<DirEntry> {
        let mut source_files: Vec<DirEntry> = vec![];

        for dir in &self.src {
            let mut dir_source_iter: Vec<DirEntry> = WalkDir::new(dir)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.metadata().unwrap().is_file())
                .filter(|e| Self::is_valid_file(e.file_name(), &self.extensions))
                .collect();

            source_files.append(&mut dir_source_iter);
        }

        source_files
    }

    pub fn get_compile_commands(
        &self,
        source_files: &[DirEntry],
        config: &Config,
    ) -> Vec<CompileCommand> {
        let commands: Vec<_> = source_files
            .iter()
            .map(|source| {
                let output_file = self.get_output_file(source.path(), config);

                let mut command =
                    CompileCommand::new(std::env::current_dir().unwrap(), &config.compiler, source);

                // Add the primary compile commands arguments
                command.push_args(&[
                    "-c",
                    source.path().to_str().unwrap(),
                    "-o",
                    output_file.to_str().unwrap(),
                ]);

                // Add the include arguments
                if let Some(include_dirs) = &self.include {
                    command.push_args(
                        &include_dirs
                            .iter()
                            .map(|x| format!("-I{}", x))
                            .collect::<Vec<_>>()[..],
                    );
                }

                // Add the defines
                if let Some(defines) = &self.defines {
                    command.push_args(
                        &defines
                            .iter()
                            .map(|x| format!("-D{}", x))
                            .collect::<Vec<_>>()[..],
                    );
                }

                if let Some(args) = &config.compiler_opts {
                    command.push_args(args);
                }

                command
            })
            .collect();

        commands
    }

    pub fn link(&self, source_files: Vec<DirEntry>, config: &Config) -> Result<(), std::io::Error> {
        // Determine the output directory
        let output_dir = Path::new(&config.bin);

        // Create the output directory if it doesn't exist
        std::fs::create_dir_all(&output_dir)?;

        // Get a list of all the object files
        let object_files: Vec<_> = source_files
            .iter()
            .map(|x| self.get_output_file(x.path(), config))
            .collect();

        // Get the link command based on what kind of project this is
        let mut link_command = match self.kind {
            ProjectKind::Executable => {
                let mut link_command = Command::new(&config.linker);

                // Add all the object files that have to be linked
                link_command.args(object_files);

                // Set up the output file main search directory
                link_command
                    .arg("-o")
                    .arg(&output_dir.join(&self.name))
                    .arg("-L")
                    .arg(&config.bin);

                // Add all the other project dependencies it has to link to
                if let Some(deps) = &self.depends {
                    link_command.args(deps.iter().map(|x| format!("-l{}", x)).collect::<Vec<_>>());
                }

                // Add any other user-specified linker options
                if let Some(args) = &config.linker_opts {
                    link_command.args(args);
                }

                link_command
            }
            ProjectKind::Library => {
                let mut link_command = Command::new(&config.packer);

                // Setup the output file name and object files
                link_command
                    .arg("rcs")
                    .arg(output_dir.join(format!("lib{}.a", self.name)))
                    .args(object_files);

                link_command
            }
        };

        // Add all the linked libraries
        if let Some(links) = &self.links {
            link_command.args(links.iter().map(|x| format!("-l{}", x)).collect::<Vec<_>>());
        }

        // Execute the command and get the output
        let output = link_command.output().expect("failed to link command");

        // If the link command didn't exit succesfully, print the error and exit
        if !output.status.success() {
            eprintln!(
                "{}{}{}",
                color::Fg(color::Red),
                String::from_utf8(output.stderr).unwrap(),
                color::Fg(color::Reset),
            );
            std::process::exit(-1);
        }

        Ok(())
    }

    pub fn build(
        &self,
        force_link: bool,
        cache: &mut Cache,
        config: &Config,
    ) -> Result<bool, std::io::Error> {
        // Gathering source files
        let source_files = self.get_source_files();

        // Check which source files we actually have to recompile
        let source_files_to_recompile: Vec<_> = source_files
            .iter()
            .filter(|x| {
                let time = x.metadata().unwrap().modified().unwrap();
                cache.has_changed(&self.get_output_file(x.path(), config), &time)
            })
            .collect();

        // If there is nothing to do return
        if source_files_to_recompile.is_empty() {
            if force_link {
                self.link(source_files, config)?;
            }
            return Ok(force_link);
        }

        // Set up the progress bar
        let progress_bar = ProgressBar::new(source_files_to_recompile.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar().template("{prefix:10} {bar:80} {pos:>5}/{len:5} {msg}"),
        );
        progress_bar.set_prefix(self.name.clone());

        // Fetch all the compile commands
        let compile_commands = self.get_compile_commands(
            &source_files_to_recompile
                .iter()
                .map(|x| (*x).clone())
                .collect::<Vec<_>>()[..],
            config,
        );

        // Execute all compile commands
        for mut compile_command in compile_commands {
            // Set the current file we are compiling
            progress_bar.set_message(
                compile_command
                    .source_file
                    .file_name()
                    .to_str()
                    .unwrap()
                    .to_string(),
            );

            // Get the output file and create it's parent directory if it doesn't exist
            let output_file = self.get_output_file(compile_command.source_file.path(), config);
            std::fs::create_dir_all(&output_file.parent().unwrap())?;

            match compile_command.execute() {
                Ok(_) => {
                    // The command executed succesfully so we can update the build cache
                    cache.update(&output_file);
                    cache.write()?;
                    // Increment the progress bar
                    progress_bar.inc(1);
                }
                Err(e) => {
                    // The command failed so lets print an error message
                    eprintln!("{}{}{}", color::Fg(color::Red), e, color::Fg(color::Reset),);
                    // Since compilation stops here, we can stop the progress bar
                    progress_bar.finish_and_clear();
                    // And exit the program
                    std::process::exit(-1);
                }
            }
        }
        // Compilation succesful
        progress_bar.finish_with_message("done");

        // Link all compiled object files
        self.link(source_files, config)?;
        Ok(true)
    }
}
