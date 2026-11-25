//! Storage server functions for Your data, the dashboard and the Storage page.
//!
//! Sizes come from the newest daily run in the Spectra storage tables
//! ([`valence_backend::storage`]); the cost settings come from the `valence_storage_settings`
//! Valence record. Each function returns a view whose `size` names its fallback, so a host
//! with no run yet, no Spectra router, or a failing Spectra still answers:
//!
//! - [`get_my_storage`] — the signed-in member's records. With no run, rows are counted live
//!   from Valence ownership and sizes are `None`.
//! - [`get_site_storage`] — the whole database by schema, 30-day history and the biggest
//!   owners. ValenceAdmin only.
//! - [`get_storage_settings`] / [`save_storage_settings`] — copies kept and price per GB each
//!   month. Saving is ValenceAdmin only.

use leptos::prelude::*;
use valence_backend::storage::{MyStorageView, SiteStorage, StorageSettings};

/// Days of history on the Storage page chart.
#[cfg(feature = "ssr")]
const HISTORY_DAYS: u32 = 30;

/// Owners listed under "biggest owners" on the Storage page.
#[cfg(feature = "ssr")]
const TOP_OWNERS: usize = 10;

/// The host's Spectra router: the `Arc<Spectra>` in Leptos context, else the process global.
/// `None` when the host runs without Spectra; the views then show live counts only.
#[cfg(feature = "ssr")]
fn spectra_router() -> Option<std::sync::Arc<spectra::SpectraRouter>> {
    use_context::<std::sync::Arc<spectra::Spectra>>()
        .map(|s| s.router())
        .or_else(spectra::SpectraRouter::try_global)
}

#[cfg(feature = "ssr")]
fn session_user_id(v: &valence::Valence) -> Result<String, ServerFnError> {
    match v.actor() {
        valence::Actor::User { user_id } => Ok(user_id.clone()),
        _ => Err(super::helpers::auth_error("Storage needs a signed-in user")),
    }
}

#[cfg(feature = "ssr")]
fn log_read_error<T>(what: &str, result: &Result<T, valence_backend::storage::StorageReadError>) {
    if let Err(e) = result {
        tracing::warn!(target: "valence_app::storage", read = what, error = %e, "storage snapshot read failed");
    }
}

/// What the signed-in member's records take up, estimated from their rows in each schema
/// times that schema's average row size.
///
/// With no run yet (or no Spectra), `size` is `NotMeasured` and each schema carries the
/// live row count with sizes `None`.
#[uf_product_macros::server]
pub async fn get_my_storage() -> Result<MyStorageView, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use valence::owner_ref::OwnerKind;
        use valence::ownership::OwnershipService;
        use valence_backend::storage::{
            build_owner_view, latest_owner_snapshot, latest_schema_snapshot, LiveSchemaCount,
            OwnerKey,
        };

        let v = super::helpers::viewer_valence().await?;
        let user_id = session_user_id(&v)?;
        let settings = super::storage_settings::load_storage_settings(&v).await?;

        let (owner_snapshot, schema_snapshot) = match spectra_router() {
            Some(router) => {
                let owner = latest_owner_snapshot(&router, &OwnerKey::user(&user_id)).await;
                log_read_error("owner", &owner);
                let schemas = latest_schema_snapshot(&router).await;
                log_read_error("schema", &schemas);
                (owner, schemas.ok().flatten())
            }
            None => (Ok(None), None),
        };

        let live = if matches!(owner_snapshot, Ok(Some(_))) {
            Vec::new()
        } else {
            OwnershipService::owner_data_summary(&user_id, OwnerKind::User.as_str(), &v)
                .await
                .map_err(|e| super::helpers::io_error(e.to_string()))?
                .rows_by_schema
                .into_iter()
                .map(|r| LiveSchemaCount {
                    schema: r.valence_model,
                    rows: Some(i64::try_from(r.active_rows).unwrap_or(i64::MAX)),
                })
                .collect()
        };

        Ok(build_owner_view(
            owner_snapshot,
            schema_snapshot.as_ref(),
            &settings,
            &live,
            chrono::Utc::now(),
        ))
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!("SSR only")
    }
}

/// The whole database by schema, its 30-day history, the biggest owners and the newest run.
///
/// With no run yet (or no Spectra), `view.size` is `NotMeasured` and every registered
/// schema is listed with sizes `None`. Valence has no site-wide row count, so `rows` is
/// `None` too until the first run.
#[uf_product_macros::server(permission = "ValenceAdmin")]
pub async fn get_site_storage() -> Result<SiteStorage, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use valence::SchemaRegistry;
        use valence_backend::storage::{
            build_site_view, history, latest_schema_snapshot, top_owners, LiveSchemaCount,
            OwnerTotal,
        };

        let v = super::helpers::viewer_valence().await?;
        let settings = super::storage_settings::load_storage_settings(&v).await?;

        let (snapshot, history_rows, owners) = match spectra_router() {
            Some(router) => {
                let snapshot = latest_schema_snapshot(&router).await;
                log_read_error("schema", &snapshot);
                let history_rows = history(&router, HISTORY_DAYS).await;
                log_read_error("history", &history_rows);
                let owners = top_owners(&router, TOP_OWNERS).await;
                log_read_error("top_owners", &owners);
                (snapshot, history_rows, owners.unwrap_or_default())
            }
            None => (Ok(None), Ok(Vec::new()), Vec::new()),
        };

        let (run_id, measured_at) = match &snapshot {
            Ok(Some(s)) => (Some(s.run_id.clone()), Some(s.measured_at)),
            _ => (None, None),
        };
        let live: Vec<LiveSchemaCount> = if matches!(snapshot, Ok(Some(_))) {
            Vec::new()
        } else {
            let mut names: Vec<String> = SchemaRegistry::global()
                .list_schemas()
                .into_iter()
                .map(str::to_string)
                .collect();
            names.sort();
            names
                .into_iter()
                .map(|schema| LiveSchemaCount { schema, rows: None })
                .collect()
        };

        let view = build_site_view(snapshot, history_rows, &settings, &live, chrono::Utc::now());
        let top_owners = owners
            .iter()
            .map(|o| OwnerTotal::from_row(o, view.copies, &view.price))
            .collect();
        Ok(SiteStorage {
            view,
            top_owners,
            run_id,
            measured_at,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!("SSR only")
    }
}

/// The site's storage cost settings, or the defaults when none are saved.
#[uf_product_macros::server]
pub async fn get_storage_settings() -> Result<StorageSettings, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let v = super::helpers::viewer_valence().await?;
        super::storage_settings::load_storage_settings(&v).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!("SSR only")
    }
}

/// Saves copies kept (`1..=10`, or `None` to use what the database reports) and the price
/// per GB each month (`>= 0`, or `None` for sizes only). ValenceAdmin only.
#[uf_product_macros::server(permission = "ValenceAdmin")]
pub async fn save_storage_settings(
    copies_override: Option<i32>,
    price_per_gb_month: Option<f64>,
) -> Result<StorageSettings, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use valence_backend::storage::validate_storage_settings;

        validate_storage_settings(copies_override, price_per_gb_month)
            .map_err(|e| super::helpers::validation_message(e.to_string()))?;
        let v = super::helpers::viewer_valence().await?;
        let user_id = session_user_id(&v)?;
        let current = super::storage_settings::load_storage_settings(&v).await?;
        super::storage_settings::save_storage_settings_row(
            &v,
            &user_id,
            copies_override,
            price_per_gb_month,
            current.currency,
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (copies_override, price_per_gb_month);
        unreachable!("SSR only")
    }
}

#[cfg(all(test, feature = "ssr"))]
mod purpose_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use valence_data_use_scan::{lint_purpose, PurposeTier};

    /// Every raw-string purpose passed to `valence::use_!` in a source file. The needle is
    /// split so this test's own source doesn't match it.
    fn purposes(src: &str) -> Vec<String> {
        let open = concat!("valence::use", "_!(r#\"");
        let close = concat!("\"#", ")");
        src.split(open)
            .skip(1)
            .map(|rest| rest.split(close).next().expect("closing").to_string())
            .collect()
    }

    #[test]
    fn storage_purposes_name_storage_and_pass_the_purpose_lint() {
        let sources = [
            include_str!("storage.rs"),
            include_str!("storage_settings.rs"),
        ];
        let found: Vec<String> = sources.iter().flat_map(|s| purposes(s)).collect();
        // The two settings calls (load and save); this file's reads go through Spectra.
        assert_eq!(found.len(), 2, "{found:?}");
        for purpose in found {
            assert!(
                purpose.contains("Storage"),
                "purpose must name Storage: {purpose}"
            );
            let gaps = lint_purpose(&purpose, PurposeTier::S3);
            assert!(gaps.is_empty(), "purpose lint {gaps:?}: {purpose}");
        }
    }
}
