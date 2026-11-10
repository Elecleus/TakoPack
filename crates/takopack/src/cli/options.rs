use clap::{Parser, Subcommand, builder::Styles, builder::styling::AnsiColor};

use takopack_python::cli::PythonSubcommands;
use takopack_rust::RustSubcommands;

use super::source::SourceCommand;

const CLI_STYLE: Styles = Styles::styled()
    .header(AnsiColor::Yellow.on_default())
    .usage(AnsiColor::Green.on_default())
    .literal(AnsiColor::Green.on_default())
    .placeholder(AnsiColor::Green.on_default());

#[derive(Debug, Clone, Parser)]
#[command(name = "takopack", about = "Package Rust crates for takopack")]
#[command(version)]
#[command(styles = CLI_STYLE)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Opt,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Opt {
    /// Rust/Cargo package operations
    #[command(subcommand)]
    Cargo(RustSubcommands),
    /// Python package operations
    #[command(subcommand)]
    Py(PythonSubcommands),
    /// Render RPM specs for a Rust crate or workspace to stdout
    Spec {
        #[command(subcommand)]
        source: SourceCommand,
    },
    /// Print the curated Output for a Rust crate or workspace
    Output {
        #[command(subcommand)]
        source: SourceCommand,
    },
}
