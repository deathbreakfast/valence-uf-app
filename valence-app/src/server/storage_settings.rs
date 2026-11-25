//! The `valence_storage_settings` Valence schema: the site's storage cost settings.
//!
//! One row (id [`SETTINGS_ID`]) holds the copies kept and the price per GB each month. The
//! schema lives in `schemas/valence_storage_settings_valence_schema.rs`; `build.rs` generates
//! the typed model included below. Its privacy policy enforces who may touch it: signed-in
//! members can read it, and only actors holding `ValenceAdmin` ([`VALENCE_ADMIN_GATE`]) can
//! create or update it, whichever write path they reach it through. System may also update
//! it so the deletion runner can clear `updated_by`. `save_storage_settings` checks
//! `ValenceAdmin` too and writes under the admin's own session. It is separate from Meson's settings: a database disk and a file bucket are priced and
//! copied differently.

use std::any::Any;

use async_trait::async_trait;
use chrono::Utc;
use leptos::prelude::ServerFnError;
use valence::privacy::PrivacyRule;
use valence::{
    Actor, ActorContext, Error, Model, PolicyEvaluator, PrivacyOperation, RecordId, Valence,
};
use valence_backend::storage::{StorageSettings, DEFAULT_CURRENCY};
use valence_backend::VALENCE_ADMIN_PERMISSION;

/// Allows signed-in human users (not anonymous visitors or service users).
pub(crate) const SIGNED_IN_MEMBER: PrivacyRule = PrivacyRule {
    name: "signed_in_member",
    description: Some("Allow signed-in users"),
    check: |_record, viewer| viewer.is_user(),
};

/// Policy rule for [`ValenceAdminGate`].
pub(crate) const VALENCE_ADMIN_GATE: ValenceAdminGate = ValenceAdminGate;

/// Allows System and actors holding the `ValenceAdmin` Gauge permission.
///
/// Uses Gauge's raw walks, which are safe inside a policy evaluator; the typed `actor_can`
/// re-enters ORM privacy and would recurse.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ValenceAdminGate;

#[async_trait]
impl PolicyEvaluator for ValenceAdminGate {
    fn name(&self) -> &'static str {
        "valence::VALENCE_ADMIN"
    }

    fn description(&self) -> Option<&'static str> {
        Some("ValenceAdmin Gauge permission (raw walks)")
    }

    async fn evaluate(
        &self,
        _op: PrivacyOperation,
        _record: &serde_json::Value,
        actor: &dyn ActorContext,
        v: &Valence,
    ) -> valence::Result<bool> {
        let viewer: Actor = serde_json::from_value(actor.actor_json().clone())
            .map_err(|e| Error::Internal(format!("invalid actor context: {e}")))?;
        if viewer.is_system() {
            return Ok(true);
        }
        gauge::actor_can_raw::actor_can_raw(v, VALENCE_ADMIN_PERMISSION)
            .await
            .map_err(|e| Error::Privacy(format!("ValenceAdmin raw check failed: {e}")))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

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
        valence::use_!(r#"On the Valence **Storage** page, its cost settings form and the **Your data** storage card, we **load the site's storage cost settings**: how many copies of the database are kept and the price per GB each month. The page multiplies measured sizes by them to show a monthly cost to signed-in members, and the form shows a **ValenceAdmin** who last changed them."#),
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
