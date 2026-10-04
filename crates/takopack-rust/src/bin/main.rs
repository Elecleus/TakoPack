use std::path::PathBuf;

use anyhow::Result;
use cargo_metadata::CargoOpt;
use clap::{Parser, Subcommand};
use takopack_rust::cargo::{
    cargo_dir::StructuredCargoDir,
    source::{CratesIoFetcher, local_dir::LocalCargoDir, manifest::SingleManifestFile},
};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Local {
        path: PathBuf,
    },
    CratesIo {
        name: String,
        version: Option<String>,
    },
    Manifest {
        path: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let dir: StructuredCargoDir = match cli.command {
        Commands::Local { path } => LocalCargoDir::new(path).into(),
        Commands::CratesIo { name, version } => CratesIoFetcher::new(&name, version.as_deref())
            .fetch()?
            .extract()?
            .into(),
        Commands::Manifest { path } => SingleManifestFile::new(&path).materialize().into(),
    };

    println!("{:#?}", dir.get_metadata(CargoOpt::AllFeatures));

    Ok(())
}
