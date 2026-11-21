mod anchor;
mod index;
mod linker;
mod platform;
mod resolver;
mod scan;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(
    name = "tp",
    version,
    about = "EZRA permanent links and local corpus index"
)]
struct Cli {
    /// Configuration file. Defaults to config.yaml beside tp.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    Scan {
        roots: Vec<PathBuf>,
        #[arg(long)]
        full_hash: bool,
        #[arg(long)]
        write_ids: bool,
    },
    Open {
        uri: String,
    },
    Link {
        path: PathBuf,
        #[arg(long)]
        no_clipboard: bool,
    },
    Alias {
        #[command(subcommand)]
        command: Alias,
    },
    Linkify {
        file: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Register {
        #[arg(long)]
        context_menu: bool,
    },
    Unregister,
}

#[derive(Subcommand)]
enum Alias {
    Add { id: String, name: String },
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Settings {
    database_path: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    scan_roots: Vec<PathBuf>,
    skip_directories: Vec<String>,
}

fn default_data_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("LOCALAPPDATA").filter(|_| cfg!(windows)) {
        return PathBuf::from(path).join("EZRA");
    }
    if let Some(path) =
        std::env::var_os("XDG_DATA_HOME").filter(|path| Path::new(path).is_absolute())
    {
        return PathBuf::from(path).join("ezra");
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(".local/share/ezra")
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Action::Register { context_menu } => return platform::registration(true, context_menu),
        Action::Unregister => return platform::registration(false, false),
        _ => {}
    }
    let explicit = cli.config.is_some();
    let config = if let Some(path) = cli.config {
        std::path::absolute(path)?
    } else {
        std::env::current_exe()?
            .parent()
            .context("No executable parent")?
            .join("config.yaml")
    };
    let config_dir = config
        .parent()
        .context("No configuration parent directory")?;
    let settings: Settings = match fs::read_to_string(&config) {
        Ok(text) => {
            serde_yaml::from_str(&text).with_context(|| format!("Read {}", config.display()))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && !explicit => Settings::default(),
        Err(e) => return Err(e).with_context(|| format!("Read {}", config.display())),
    };
    let database = settings
        .database_path
        .map(|path| config_dir.join(path))
        .unwrap_or_else(|| default_data_dir().join("tp_index.sqlite"));
    let database = platform::local_path(&database)?;
    let output = settings
        .output_dir
        .map(|path| config_dir.join(path))
        .unwrap_or_else(|| database.parent().unwrap().join("tp-output"));
    let output = platform::local_path(&output)?;
    let mut db = index::open(&database)?;
    match cli.command {
        Action::Scan {
            mut roots,
            full_hash,
            write_ids,
        } => {
            if roots.is_empty() {
                roots = settings
                    .scan_roots
                    .into_iter()
                    .map(|path| config_dir.join(path))
                    .collect();
            }
            if roots.is_empty() {
                bail!("Specify scan roots or set scan_roots in config.yaml");
            }
            scan::run(
                &mut db,
                &roots,
                &settings.skip_directories,
                full_hash,
                write_ids,
                &database,
                &output,
            )
        }
        Action::Open { uri } => {
            resolver::dispatch(resolver::resolve(&db, &uri)?, &output, platform::open)
        }
        Action::Link { path, no_clipboard } => {
            let link = resolver::link(&resolver::path_id(&db, &path)?);
            println!("{link}");
            if !no_clipboard && let Err(error) = platform::clipboard(&link) {
                eprintln!("Clipboard unavailable: {error:#}");
            }
            Ok(())
        }
        Action::Alias {
            command: Alias::Add { id, name },
        } => {
            let id = uuid::Uuid::parse_str(&id)
                .context("Alias requires a record UUID")?
                .to_string();
            index::alias(&db, &id, &name, "manual")
        }
        Action::Linkify { file, out } => {
            println!("{}", linker::run(&db, &file, &out)?.display());
            Ok(())
        }
        Action::Register { .. } | Action::Unregister => unreachable!(),
    }
}

fn main() -> std::process::ExitCode {
    match run(Cli::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tp: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
