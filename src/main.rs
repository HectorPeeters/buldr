use crate::config::BuildConfig;
use crate::config::Config;
use crate::project::Project;
use cache::Cache;
use clap::ArgMatches;
use clap::{App, Arg, SubCommand};
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

mod cache;
mod compile_command;
mod config;
mod project;

const COMPILE_COMMANDS_PATH: &str = "compile_commands.json";

fn create_directories(config: &BuildConfig) -> Result<(), std::io::Error> {
    // Create the bin directory
    std::fs::create_dir_all(&config.config.bin)?;

    // Create the obj directory
    std::fs::create_dir_all(&config.config.obj)?;

    Ok(())
}

fn get_dependencies<'a>(projects: &'a [Project], project: &Project) -> Vec<&'a Project> {
    match &project.depends {
        Some(dependency_names) => {
            let mut dependencies: Vec<&'a Project> = vec![];

            for project_name in dependency_names {
                match projects.iter().find(|x| &x.name == project_name) {
                    Some(x) => dependencies.push(x),
                    None => {
                        eprintln!("No dependency found with name '{}'", project_name);
                        std::process::exit(1);
                    }
                }
            }

            dependencies
        }
        None => vec![],
    }
}

fn build_project_with_dependencies(
    project: &Project,
    all_projects: &[Project],
    config: &Config,
    cache: &mut Cache,
) -> Result<bool, std::io::Error> {
    // Get all the dependencies
    let dependencies = get_dependencies(all_projects, project);

    let mut needs_rebuild = false;

    // Compile them in the correct order
    for dependency in dependencies {
        needs_rebuild |= build_project_with_dependencies(dependency, all_projects, config, cache)?;
    }

    // Finally build the resulting project
    project.build(needs_rebuild, cache, config)
}

fn load_config(build_file: &str) -> Result<BuildConfig, std::io::Error> {
    Ok(toml::from_str::<BuildConfig>(&std::fs::read_to_string(
        build_file,
    )?)?)
}

fn create(build_file_path: &Path) -> Result<(), std::io::Error> {
    // If there is already a build.toml file, don't overwrite it!
    if build_file_path.exists() {
        println!("build.toml already exists");
        return Ok(());
    }

    // Create a new build.toml file
    File::create(&build_file_path)?;

    // Write the template file to build.toml
    let template = include_str!("template.toml");
    std::fs::write(&build_file_path, template)
}

fn clean(build_file: &str) -> Result<(), std::io::Error> {
    // If the build file exists, clear the cache
    if PathBuf::from(build_file).exists() {
        Cache::new(build_file)?.clean();
    }

    // Load the config
    let config = load_config(build_file)?;

    // Remove the bin dir if it exists
    if PathBuf::from(&config.config.bin).exists() {
        std::fs::remove_dir_all(config.config.bin)?;
    }

    // Remove the obj dir if it exists
    if PathBuf::from(&config.config.obj).exists() {
        std::fs::remove_dir_all(config.config.obj)?;
    }

    if PathBuf::from(COMPILE_COMMANDS_PATH).exists() {
        std::fs::remove_file(COMPILE_COMMANDS_PATH)?;
    }

    Ok(())
}

fn compile_commands(build_file: &str) -> Result<(), std::io::Error> {
    // Load the config
    let config = load_config(build_file)?;

    // List to store all compile commands of all projects
    let mut all_compile_commands = vec![];

    for project in config.projects {
        // Get the compile commands of each project and add it to the list
        all_compile_commands.append(
            &mut project.get_compile_commands(&project.get_source_files()[..], &config.config),
        );
    }

    // Write the result to compile_commands.json
    std::fs::write(
        Path::new(COMPILE_COMMANDS_PATH),
        serde_json::to_string(&all_compile_commands).unwrap(),
    )
}

fn build(build_file: &str, matches: &ArgMatches) -> Result<Option<PathBuf>, std::io::Error> {
    // Make sure the build file exists
    if !PathBuf::from(build_file).exists() {
        eprintln!("No build.toml file found!");
        return Ok(None);
    }

    // Load the config
    let config = load_config(build_file)?;

    // Create the bin and obj directories
    create_directories(&config)?;

    // Load or create the cache
    let mut cache = Cache::new(build_file)?;

    // Make sure there are some projects defined
    if config.projects.is_empty() {
        eprintln!("No projects defined");
        return Ok(None);
    }

    // Find which project to compile
    let project = match matches.value_of("project") {
        Some(name) => match config.projects.iter().find(|x| x.name == name) {
            Some(project) => project,
            None => {
                eprintln!("No project found with name '{}'", name);
                return Ok(None);
            }
        },
        None => match config.projects.iter().find(|x| x.default == Some(true)) {
            Some(project) => project,
            None => {
                eprintln!("No default project");
                return Ok(None);
            }
        },
    };

    // Build that project and its dependencies
    build_project_with_dependencies(project, &config.projects, &config.config, &mut cache)?;
    let output = Path::new(&config.config.bin).join(&project.name);

    Ok(Some(output))
}

fn run(build_file: &str, matches: &ArgMatches) -> Result<(), std::io::Error> {
    match build(build_file, matches)? {
        Some(output) => {
            Command::new(output)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .unwrap();
        }
        _ => {}
    }

    Ok(())
}

fn main() -> Result<(), std::io::Error> {
    let matches = App::new("Buldr")
        .version("0.0.1")
        .author("Hector Peeters <hector.peeters@gmail.com>")
        .arg(Arg::with_name("project").index(1))
        .arg(
            Arg::with_name("build-file")
                .short("b")
                .long("build-file")
                .takes_value(true),
        )
        .subcommand(SubCommand::with_name("create").about("generate a template build.toml file"))
        .subcommand(SubCommand::with_name("clean").about("Clean all build files"))
        .subcommand(
            SubCommand::with_name("compile_commands").about("Generate compile_commands.json"),
        )
        .subcommand(
            SubCommand::with_name("run").about("Build and run the default compiled executable"),
        )
        .get_matches();

    // Get the path to the build.toml file
    let build_file = matches.value_of("build-file").unwrap_or("build.toml");
    let build_file_path = PathBuf::from(build_file);

    if matches.is_present("build-file") {
        println!("Using custom config: {}", build_file);
    }

    match matches.subcommand_name() {
        Some("create") => create(&build_file_path),
        Some("clean") => clean(&build_file),
        Some("compile_commands") => compile_commands(build_file),
        Some("run") => run(&build_file, &matches),
        Some(_) | None => build(&build_file, &matches).map(|_| ()),
    }
}
