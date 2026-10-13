use std::path::PathBuf;

use anyhow::Result;
use cargo_metadata::CargoOpt;
use clap::{Parser, Subcommand};
use takopack_rust::cargo::{
    cargo_dir::CargoDir,
    process::Process,
    source::{CargoSource, crates_io::FromCratesIo},
};
use tempfile::TempDir;

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    CratesIo {
        name: String,
        version: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let tempdir = TempDir::with_prefix("takopack")?;

    let source = match cli.command {
        Commands::CratesIo { name, version } => {
            let mut s = FromCratesIo::new(name);
            if let Some(version) = version {
                s.version(version);
            } else {
                s.latest()?;
            };

            Box::new(s.prepare(&tempdir)?)
        }
    };

    let process = Process::from(source, tempdir).load().unwrap();

    Ok(())
}
