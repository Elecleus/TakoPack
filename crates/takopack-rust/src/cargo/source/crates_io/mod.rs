//! Here is our **pipeline**:
//! 1. Get the arguements "name" and "version".
//! 2. Download the .crate file from crates.io.
//! 3. decompress the file.
//!
//! ## For Developer:
//!
//! You might want to read https://crates.io/data-access first.

mod extracted;
mod fetched;
pub mod fetcher;
