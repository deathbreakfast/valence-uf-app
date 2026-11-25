//! The `valence_storage_settings` Valence schema: the site's storage cost settings.
//!
//! One row (id [`SETTINGS_ID`]) holds the copies kept and the price per GB each month. The
//! schema lives in `schemas/valence_storage_settings_valence_schema.rs`; `build.rs` generates
//! the typed model included below. Signed-in members can read it; the only write path is
//! `save_storage_settings`, which requires ValenceAdmin and runs under the admin's session.
//! It is separate from Meson's settings: a database disk and a file bucket are priced and
//! copied differently.

use chrono::Utc;
use leptos::prelude::ServerFnError;
use valence::privacy::PrivacyRule;
use valence::{Model, RecordId, Valence};
use valence_backend::storage::{StorageSettings, DEFAULT_CURRENCY};

/// Allows signed-in human users (not anonymous visitors or service users).
pub(crate) const SIGNED_IN_MEMBER: PrivacyRule = PrivacyRule {
    name: "signed_in_member",
    description: Some("Allow signed-in users"),
    check: |_record, viewer| viewer.is_user(),
};

mod generated {
    #![allow(
        dead_code,
        unused_imports,
        missing_docs,
        clippy::all,
        clippy::pedantic,
        clippy::nursery
    )]

    include!(concat!(env!("OUT_DIR"), "/generated_models.rs"));
}

pub(crate) use generated::ValenceStorageSettings;

/// Primary key of the one settings row.
pub(crate) const SETTINGS_ID: &str = "site";

/// The site's storage settings, or the defaults (no override, no price) when none are saved.
pub(crate) async fn load_storage_settings(v: &Valence) -> Result<StorageSettings, ServerFnError> {
    let row = ValenceStorageSettings::get(
        SETTINGS_ID,
        v,
        valence::use_!(r#"On the Valence **Storage** page, its cost settings form and the **Your data** storage card, we **load the site's storage cost settings**: how many copies of the database are kept and the price per GB each month. The page multiplies measured sizes by them to show a monthly cost to signed-in members, and the form shows who last changed them."#),
    )
    .await
    .map_err(|e| super::helpers::io_error(format!("load storage settings: {e}")))?;
    Ok(row
        .as_ref()
        .map_or_else(StorageSettings::default, settings_from_row))
}

/// Saves the settings as `user_id`, keeping the saved currency. Callers validate first.
pub(crate) async fn save_storage_settings_row(
    v: &Valence,
    user_id: &str,
    copies_override: Option<i32>,
    price_per_gb_month: Option<f64>,
    currency: String,
) -> Result<StorageSettings, ServerFnError> {
    let row = ValenceStorageSettings::new(
        copies_override.map(i64::from),
        price_per_gb_month,
        currency,
        Some(RecordId::new("user", user_id)),
        Utc::now(),
    )
    .map_err(|e| super::helpers::io_error(format!("build storage settings: {e}")))?;
    let saved = ValenceStorageSettings::upsert(
        SETTINGS_ID,
        row,
        v,
        valence::use_!(r#"When a **ValenceAdmin** saves the **Storage** cost settings, we **write the copies kept and the price per GB each month** to the site's single settings record, with **who saved it and when**. The Storage page and every member's Your data card use these values to work out the monthly cost of the database."#),
    )
    .await
    .map_err(|e| super::helpers::io_error(format!("save storage settings: {e}")))?;
    Ok(settings_from_row(&saved))
}

fn settings_from_row(row: &ValenceStorageSettings) -> StorageSettings {
    let currency = row.currency().trim();
    StorageSettings {
        copies_override: row.copies_override().and_then(|n| i32::try_from(*n).ok()),
        price_per_gb_month: row.price_per_gb_month().copied(),
        currency: if currency.is_empty() {
            DEFAULT_CURRENCY.to_string()
        } else {
            currency.to_string()
        },
        updated_by: row.updated_by().map(|r| r.id().to_string()),
        updated_at: Some(*row.updated_at()),
    }
}
