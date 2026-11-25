//! Generates the typed `ValenceStorageSettings` model from `schemas/`.
//!
//! The output is only included under the `ssr` feature (`src/server/storage_settings.rs`);
//! hydrate builds generate it and leave it unused.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    valence_codegen::build()?;
    Ok(())
}
