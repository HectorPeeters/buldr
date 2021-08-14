use crate::config::BuildConfig;
use crate::config::Config;
use crate::project::Project;
use cache::Cache;
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
    use std::fs;

    fs::create_dir_all(&config.config.bin)?;
    fs::create_dir_all(&config.config.obj)?;

    Ok(())
}

fn get_dependencies<'a>(projects: &'a [Project], name: &str) -> Vec<&'a Project> {
    let mut depgraph: DepGraph<String> = DepGraph::new();

    depgraph.register_node(name.to_string());

    for project in projects {
        match &project.depends {
            Some(x) => depgraph.register_dependencies(project.name.clone(), x.clone()),
            None => {}
        }
    }

    let dependency_names = depgraph.dependencies_of(&name.to_string()).unwrap();

    let mut dependencies: Vec<&'a Project> = vec![];
    for project_name in dependency_names {
        let project_name = project_name.unwrap();
        match projects.iter().find(|x| &x.name == project_name) {
            Some(x) => dependencies.push(x),
            None => {
                eprintln!("No dependency found with name '{}'", project_name);
                return vec![];
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
    let dependencies = get_dependencies(all_projects, &project.name);

    for dependency in dependencies {
        dependency.build(cache, config)?;
    }

    project.build(cache, config)
}

fn load_config() -> Result<BuildConfig, std::io::Error> {
    Ok(toml::from_str::<BuildConfig>(&std::fs::read_to_string(
        "build.toml",
    )?)?)
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

    let build_file_path = PathBuf::from("build.toml");

    if matches.subcommand_matches("create").is_some() {
        if build_file_path.exists() {
            println!("build.toml already exists");
            return Ok(());
        }

        File::create(&build_file_path)?;

        let template = include_str!("template.toml");
        std::fs::write(&build_file_path, template)?;
    } else if matches.subcommand_matches("clean").is_some() {
        if build_file_path.exists() {
            Cache::new()?.clean();
        }

        let config = load_config()?;
        if PathBuf::from(&config.config.bin).exists() {
            std::fs::remove_dir_all(config.config.bin)?;
        }
        if PathBuf::from(&config.config.obj).exists() {
            std::fs::remove_dir_all(config.config.obj)?;
        }
    } else if matches.subcommand_matches("compile_commands").is_some() {
        let config = load_config()?;
        let mut all_compile_commands = vec![];
        for project in config.projects {
            all_compile_commands.append(
                &mut project.get_compile_commands(&project.get_source_files()[..], &config.config),
            );
        }
        std::fs::write(
            Path::new("compile_commands.json"),
            serde_json::to_string(&all_compile_commands).unwrap(),
        )
        .unwrap();
    } else {
        if !build_file_path.exists() {
            eprintln!("No build.toml file found!");
            return Ok(());
        }

        let config = load_config()?;

        create_directories(&config)?;

        let mut cache = Cache::new()?;

        if config.projects.is_empty() {
            eprintln!("No projects defined");
            return Ok(());
        }

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

        build_project_with_dependencies(project, &config.projects, &config.config, &mut cache)
            .unwrap();
    }
    Ok(())
}
