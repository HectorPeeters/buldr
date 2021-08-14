use crate::compile_command::CompileCommand;
use crate::config::Config;
use crate::Cache;
use indicatif::{ProgressBar, ProgressStyle};
use serde_derive::Deserialize;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
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

                let mut command = CompileCommand::new(
                    std::env::current_dir().unwrap(),
                    &config.compiler,
                    std::fs::canonicalize(source.path())
                        .unwrap()
                        .to_str()
                        .unwrap(),
                    source,
                );

                command.push_args(&[
                    "-c",
                    source.path().to_str().unwrap(),
                    "-o",
                    output_file.to_str().unwrap(),
                ]);

                if let Some(include_dirs) = &self.include {
                    for include_dir in include_dirs {
                        command.push_arg(&format!("-I{}", include_dir));
                    }
                }

                if let Some(defines) = &self.defines {
                    for define in defines {
                        command.push_arg(&format!("-D{}", define));
                    }
                }

                if let Some(args) = &config.compiler_opts {
                    for arg in args {
                        command.push_arg(arg);
                    }
                }

                command
            })
            .collect();

        commands
    }

    pub fn build(&self, cache: &mut Cache, config: &Config) -> Result<(), std::io::Error> {
        // Gathering source files
        let source_files = self.get_source_files();

        let object_files: Vec<_> = source_files
            .iter()
            .map(|x| self.get_output_file(x.path(), config))
            .collect();

        let source_files_to_recompile: Vec<_> = source_files
            .iter()
            .filter(|x| {
                let time = x
                    .metadata()
                    .unwrap()
                    .modified()
                    .unwrap()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                cache.has_changed(&self.get_output_file(x.path(), config), time)
            })
            .collect();

        // Compiling source files

        if source_files_to_recompile.is_empty() {
            return Ok(());
        }

        let progress_bar = ProgressBar::new(source_files_to_recompile.len() as u64);
        progress_bar.set_style(
            ProgressStyle::default_bar().template("{prefix:10} {bar:80} {pos:>5}/{len:5} {msg}"),
        );
        progress_bar.set_prefix(self.name.clone());

        let compile_commands = self.get_compile_commands(
            &source_files_to_recompile
                .iter()
                .map(|x| (**x).clone())
                .collect::<Vec<_>>()[..],
            config,
        );

        for mut compile_command in compile_commands {
            progress_bar.set_message(compile_command.file.to_string());

            let output_file = self.get_output_file(compile_command.source_file.path(), config);
            std::fs::create_dir_all(&output_file.parent().unwrap())?;

            match compile_command.execute() {
                Ok(_) => {
                    cache.update(&output_file);
                    cache.write()?;
                    progress_bar.inc(1);
                }
                Err(e) => {
                    eprintln!("{}{}{}", color::Fg(color::Red), e, color::Fg(color::Reset),);
                    progress_bar.finish_and_clear();
                    std::process::exit(-1);
                }
            }
        }
        progress_bar.finish_with_message("done");

        // Linking compiled source files

        let output_file = Path::new(&config.bin);

        std::fs::create_dir_all(&output_file)?;

        match self.kind {
            ProjectKind::Executable => {
                let mut link_command = Command::new(&config.linker);
                link_command.args(object_files);
                if let Some(args) = &config.linker_opts {
                    link_command.args(args);
                }
                link_command
                    .arg("-o")
                    .arg(&output_file.join(&self.name))
                    .arg("-L")
                    .arg(&config.bin);

                if let Some(links) = &self.links {
                    for link in links {
                        link_command.arg(format!("-l{}", link));
                    }
                }
                if let Some(deps) = &self.depends {
                    for dep in deps {
                        link_command.arg(format!("-l{}", dep));
                    }
                }

                let output = link_command.output().expect("failed to link command");

                if !output.status.success() {
                    eprintln!(
                        "{}{}{}",
                        color::Fg(color::Red),
                        String::from_utf8(output.stderr).unwrap(),
                        color::Fg(color::Reset),
                    );
                    std::process::exit(-1);
                }
            }
            ProjectKind::Library => {
                let mut link_command = Command::new(&config.packer);
                link_command
                    .arg("rcs")
                    .arg(output_file.join(format!("lib{}.a", self.name)))
                    .args(object_files);

                if let Some(links) = &self.links {
                    for link in links {
                        link_command.arg(format!("-l{}", link));
                    }
                }

                let output = link_command.output().expect("failed to link command");

                if !output.status.success() {
                    eprintln!(
                        "{}{}{}",
                        color::Fg(color::Red),
                        String::from_utf8(output.stderr).unwrap(),
                        color::Fg(color::Reset),
                    );
                    std::process::exit(-1);
                }
            }
        }

        Ok(())
    }
}
