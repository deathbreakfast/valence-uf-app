//! Valence storage: the daily Spectra snapshot tables, their reader, and the page state
//! built from them.
//!
//! A daily Chronon job measures the database and writes two Spectra tables:
//! [`VALENCE_STORAGE_SCHEMA_DAILY`] (one row per schema) and [`VALENCE_STORAGE_OWNER_DAILY`]
//! (one row per owner and schema). Embedded hosts and the remote fleet each run their own
//! job; both write the rows described below. The `valence-app` storage server functions read
//! the newest run with the functions in `snapshot` and turn it into a
//! [`ValenceStorageView`] or [`MyStorageView`] with [`build_site_view`] and
//! [`build_owner_view`]. When the tables are empty or missing, or the host has no Spectra
//! router, the views fall back to live row counts with sizes left blank.
//!
//! ## Row contract
//!
//! Every row of one run carries the same `ds`, `run_id` and `measured_at`:
//!
//! - `ds` is the UTC day of the run, `YYYY-MM-DD`. It is the partition key the reader sorts on.
//! - `run_id` is unique per run. A second run on the same day (a manual "Run now") writes a new
//!   `run_id`; the reader takes the newest run of the newest day and ignores the older one.
//! - `measured_at` is the run's start time in RFC 3339 UTC (`2026-11-24T11:20:00Z`). Log each
//!   row with the Spectra event timestamp set to the same instant (`*Logger::log_at`).
//!
//! `valence_storage_schema_daily` holds, per run:
//!
//! - one row per Valence schema, with `schema` set to the table name (`valence_model`), the
//!   live `rows`, and `table_bytes` / `index_bytes` / `avg_row_bytes`;
//! - exactly one `*` row for the whole database, whose `copies_reported` is the replica count
//!   the database reports (or `-1` when the connection can't see replicas).
//!
//! `how` says where a schema's bytes came from: `measured` (the backend reports bytes per
//! table, as Postgres does), `estimated` (rows times the average encoded size of a row
//! sample, as on SQLite without `dbstat`) or `failed` (the measurement timed out or errored;
//! the page leaves that schema out of the total and lists it as not measured).
//!
//! `valence_storage_owner_daily` holds, per run:
//!
//! - one row per owner and schema the owner has rows in, with `est_bytes` = `rows` times that
//!   schema's `avg_row_bytes` from the same run;
//! - one `*` row per owner with the owner's totals across schemas.
//!
//! `owner_kind` is the Valence owner kind (`user`, `service_user`, `team`, ...) and `owner_id`
//! the bare owner id as Valence ownership stores it (`u42`, not `user:u42`).
//!
//! Spectra event fields have no optional type, so the byte columns that may be unknown
//! (`table_bytes`, `index_bytes`, `avg_row_bytes`, `copies_reported`) use `-1` for "not
//! reported". The reader also treats a null or missing value as unknown. Write `-1` for
//! `copies_reported` on every per-schema row; only the `*` row's value is read.
//!
//! The reader pages through a run 1,000 rows at a time (the mem and SQLite backends cap a
//! query there), so a run may hold any number of owner rows.

mod rows;
#[cfg(feature = "spectra")]
mod schema;
#[cfg(feature = "spectra")]
mod snapshot;
mod view;

pub use rows::{
    HistoryRow, OwnerKey, OwnerSnapshot, OwnerSnapshotRow, SchemaSnapshot, SchemaSnapshotRow,
    StorageReadError, ALL_SCHEMAS, NOT_REPORTED,
};
#[cfg(feature = "spectra")]
pub use schema::{
    ValenceStorageOwnerDaily, ValenceStorageOwnerDailyLogger, ValenceStorageOwnerDailyPayload,
    ValenceStorageSchemaDaily, ValenceStorageSchemaDailyLogger, ValenceStorageSchemaDailyPayload,
};
#[cfg(feature = "spectra")]
pub use snapshot::{
    history, latest_owner_snapshot, latest_schema_snapshot, top_owners, READ_PAGE_SIZE,
};
pub use view::{
    build_owner_view, build_site_view, validate_storage_settings, CopiesSource, HistoryPoint,
    HistoryState, LiveSchemaCount, MeasureHow, MyStorageView, OwnerTotal, PriceState, SchemaRow,
    SiteStorage, SizeState, StorageSettings, StorageSettingsError, StorageTotals,
    ValenceStorageView, BYTES_PER_GB, COPIES_MAX, COPIES_MIN, DEFAULT_CURRENCY, STALE_AFTER_HOURS,
};

/// Spectra table with one row per schema per daily run, plus a `*` database total.
pub const VALENCE_STORAGE_SCHEMA_DAILY: &str = "valence_storage_schema_daily";

/// Spectra table with one row per owner and schema per daily run, plus each owner's `*` total.
pub const VALENCE_STORAGE_OWNER_DAILY: &str = "valence_storage_owner_daily";
