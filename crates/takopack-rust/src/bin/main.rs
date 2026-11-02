use anyhow::Result;
use clap::{Parser, Subcommand};
use takopack_rust::cargo::{Output, Process, source::crates_io::FromCratesIoBuilder};
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

    let tempdir = TempDir::with_prefix("takopack-")?;

    let source = match cli.command {
        Commands::CratesIo { name, version } => {
            let mut builder = FromCratesIoBuilder::new(name);
            if let Some(version) = version {
                builder.version(version);
            }

            builder.resolve()?
        }
    };

    let mut process = Process::from(source, &tempdir);
    let output = process.load()?;
    print_output(output);

    Ok(())
}

fn print_output(output: &Output) {
    println!("workspace: {}", output.workspace_root.display());
    println!("archive:   {}", output.source.id);
    println!("sha256:    {}", output.source.checksum);
    println!("url:       {}", output.source.url);

    for package in &output.packages {
        println!();
        println!("package: {} {}", package.name, package.version);
        if let Some(rust_version) = &package.rust_version {
            println!("  rust-version: {rust_version}");
        }
        if let Some(license) = &package.license {
            println!("  license: {license}");
        }
        if let Some(description) = &package.description {
            println!("  description: {description}");
        }
        println!(
            "  lib: {}  proc-macro: {}  bins: {:?}",
            package.is_lib, package.is_proc_macro, package.bins
        );

        println!("  base dependencies:");
        for dependency in &package.base_dependencies {
            println!(
                "    {} kind={:?} source={:?} default-features={} features={:?} range={:?}",
                dependency.package_name,
                dependency.kind,
                dependency.source,
                dependency.default_features,
                dependency.features,
                dependency.rpm_range,
            );
        }

        for feature in &package.features {
            println!("  feature {:?}: requires {:?}", feature.name, feature.features);
            for dependency in &feature.dependencies {
                println!(
                    "    {} kind={:?} features={:?} range={:?}",
                    dependency.package_name,
                    dependency.kind,
                    dependency.features,
                    dependency.rpm_range,
                );
            }
        }
    }
}
