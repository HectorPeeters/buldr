use crate::config::BuildConfig;
use crate::config::Config;
use crate::project::Project;
use cache::Cache;
use clap::ArgMatches;
use clap::{App, Arg, SubCommand};
use solvent::DepGraph;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;

mod cache;
mod compile_command;
mod config;
mod project;

fn create_directories(config: &BuildConfig) -> Result<(), std::io::Error> {
    // Create the bin directory
    std::fs::create_dir_all(&config.config.bin)?;

    // Create the obj directory
    std::fs::create_dir_all(&config.config.obj)?;

    Ok(())
}

fn get_dependencies<'a>(projects: &'a [Project], name: &str) -> Vec<&'a Project> {
    let mut depgraph: DepGraph<&str> = DepGraph::new();

    // Register the final node
    depgraph.register_node(name);

    // For each project, register its dependencies
    for project in projects {
        match &project.depends {
            Some(x) => {
                for dep in x {
                    depgraph.register_dependency(&project.name, dep);
                }
            }
            None => {}
        }
    }

    // Get all the names of the dependencies
    let dependency_names = depgraph.dependencies_of(&name).unwrap();

    // Convert this list of names to a list of projects
    let mut dependencies: Vec<&'a Project> = vec![];
    for project_name in dependency_names {
        let project_name = project_name.unwrap();
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

fn build_project_with_dependencies(
    project: &Project,
    all_projects: &[Project],
    config: &Config,
    cache: &mut Cache,
) -> Result<(), std::io::Error> {
    // Get all the dependencies
    let dependencies = get_dependencies(all_projects, &project.name);

    // Compile them in the correct order
    for dependency in dependencies {
        dependency.build(cache, config)?;
    }

    // Finally buld the resulting project
    project.build(cache, config)
}

fn load_config() -> Result<BuildConfig, std::io::Error> {
    Ok(toml::from_str::<BuildConfig>(&std::fs::read_to_string(
        "build.toml",
    )?)?)
}

fn create(build_file_path: &PathBuf) -> Result<(), std::io::Error> {
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

fn clean(build_file_path: &PathBuf) -> Result<(), std::io::Error> {
    // If the build file exists, clear the cache
    if build_file_path.exists() {
        Cache::new()?.clean();
    }

    // Load the config
    let config = load_config()?;

    // Remove the bin dir if it exists
    if PathBuf::from(&config.config.bin).exists() {
        std::fs::remove_dir_all(config.config.bin)?;
    }

    // Remove the obj dir if it exists
    if PathBuf::from(&config.config.obj).exists() {
        std::fs::remove_dir_all(config.config.obj)?;
    }

    Ok(())
}

fn compile_commands() -> Result<(), std::io::Error> {
    // Load the config
    let config = load_config()?;

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
        Path::new("compile_commands.json"),
        serde_json::to_string(&all_compile_commands).unwrap(),
    )
}

fn build(build_file_path: &PathBuf, matches: &ArgMatches) -> Result<(), std::io::Error> {
    // Make sure the build file exists
    if !build_file_path.exists() {
        eprintln!("No build.toml file found!");
        return Ok(());
    }

    // Load the config
    let config = load_config()?;

    // Create the bin and obj directories
    create_directories(&config)?;

    // Load or create the cache
    let mut cache = Cache::new()?;

    // Make sure there are some projects defined
    if config.projects.is_empty() {
        eprintln!("No projects defined");
        return Ok(());
    }

    // Find which project to compile
    let project = match matches.value_of("project") {
        Some(name) => match config.projects.iter().find(|x| x.name == name) {
            Some(project) => project,
            None => {
                eprintln!("No project found with name '{}'", name);
                return Ok(());
            }
        },
        None => match config.projects.iter().find(|x| x.default == Some(true)) {
            Some(project) => project,
            None => {
                eprintln!("No default project");
                return Ok(());
            }
        },
    };

    // Build that project and its dependencies
    build_project_with_dependencies(project, &config.projects, &config.config, &mut cache)
}

fn main() -> Result<(), std::io::Error> {
    let matches = App::new("Buldr")
        .version("0.0.1")
        .author("Hector Peeters <hector.peeters@gmail.com>")
        .arg(Arg::with_name("project").index(1))
        .subcommand(SubCommand::with_name("create").about("generate a template build.toml file"))
        .subcommand(SubCommand::with_name("clean").about("Clean all build files"))
        .subcommand(
            SubCommand::with_name("compile_commands").about("Generate compile_commands.json"),
        )
        .get_matches();

    // Get the path to the build.toml file
    let build_file_path = PathBuf::from("build.toml");

    match matches.subcommand_name() {
        Some("create") => create(&build_file_path),
        Some("clean") => clean(&build_file_path),
        Some("compile_commands") => compile_commands(),
        Some(_) | None => build(&build_file_path, &matches),
    }
}
