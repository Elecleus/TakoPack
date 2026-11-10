use std::path::Path;

use clap::Parser;
use nu_ansi_term::Color::Red;

use takopack_core::errors::Result;
use takopack_python::cli::PythonSubcommands;
use takopack_python::pypi::PypiFetcher;
use takopack_rust::cargo::source::{crates_io::FromCratesIoBuilder, remote::FromRemoteBuilder};
use takopack_rust::{
    RustSubcommands,
    cli::{inspect_crate, package_crate},
};

use super::options::{Cli, Opt};
use super::source::SourceCommand;
use super::{output, spec};

pub fn run() {
    env_logger::init();
    match real_main() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("{}", Red.bold().paint(format!("takopack failed: {:?}", e)));
            std::process::exit(1);
        }
    }
}

fn real_main() -> Result<i32> {
    let m = Cli::parse();
    use Opt::*;
    match m.command {
        Cargo(cargo_opt) => match cargo_opt {
            RustSubcommands::Package {
                init,
                extract,
                finish,
                range_capability_policy,
            } => {
                package_crate(init, extract, finish, range_capability_policy).unwrap();
                Ok(0)
            }
            RustSubcommands::LocalPackage {
                path,
                output,
                finish,
                range_capability_policy,
            } => {
                log::info!("packaging from local directory: {:?}", path);
                takopack_rust::local::process_local_package(
                    &path,
                    output,
                    finish,
                    range_capability_policy,
                )?;
                Ok(0)
            }
            RustSubcommands::RegistrySync { dry_run, jobs } => {
                log::info!("starting registry sync");
                takopack_rust::registry_sync::run_registry_sync(dry_run, jobs)
            }
            RustSubcommands::ResolveCheck { path, registry } => {
                log::info!("starting resolve check");
                takopack_rust::resolve_check::run_resolve_check(&path, registry.as_deref())
            }
            RustSubcommands::BuildReqs { path, registry } => {
                log::info!("generating dynamic BuildRequires");
                takopack_rust::buildreqs::run_buildreqs(&path, registry.as_deref())
            }
            RustSubcommands::Inspect {
                target,
                registry,
                version,
            } => {
                inspect_crate(&target, registry, version).unwrap();
                Ok(0)
            }
        },
        Spec { source } => {
            run_spec(source)?;
            Ok(0)
        }
        Output { source } => {
            run_output(source)?;
            Ok(0)
        }
        Py(py_opt) => match py_opt {
            PythonSubcommands::Package { name, version, .. } => {
                let package = PypiFetcher::new(&name, version.as_deref()).fetch().unwrap();

                package.render(Path::new(".")).unwrap();

                Ok(0)
            }
        },
    }
}

fn run_spec(source: SourceCommand) -> Result<()> {
    match source {
        SourceCommand::CratesIo { name, version } => {
            let mut builder = FromCratesIoBuilder::new(name);
            if let Some(version) = version {
                builder.version(version);
            }
            spec::run(builder.resolve()?)?;
        }
        SourceCommand::Remote { url, checksum } => {
            let mut builder = FromRemoteBuilder::new(url);
            if let Some(checksum) = checksum {
                builder = builder.checksum(checksum);
            }
            spec::run(builder.build())?;
        }
    }

    Ok(())
}

fn run_output(source: SourceCommand) -> Result<()> {
    match source {
        SourceCommand::CratesIo { name, version } => {
            let mut builder = FromCratesIoBuilder::new(name);
            if let Some(version) = version {
                builder.version(version);
            }
            output::run(builder.resolve()?)?;
        }
        SourceCommand::Remote { url, checksum } => {
            let mut builder = FromRemoteBuilder::new(url);
            if let Some(checksum) = checksum {
                builder = builder.checksum(checksum);
            }
            output::run(builder.build())?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn verify_app() {
        Cli::command().debug_assert()
    }
}
