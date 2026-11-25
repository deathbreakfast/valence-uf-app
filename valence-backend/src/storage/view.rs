//! Page state for Your data and the Storage page, with a named fallback for every input.
//!
//! [`build_site_view`] and [`build_owner_view`] are pure: the server functions read the
//! snapshot, settings and live counts, then hand them here with the current time.
//!
//! - **Size:** [`SizeState::Measured`] from a run under [`STALE_AFTER_HOURS`] old,
//!   [`SizeState::Stale`] from an older one, [`SizeState::NotMeasured`] when no run exists
//!   (live row counts, sizes `None`), and [`SizeState::Unavailable`] when Spectra failed.
//! - **Copies:** the settings override, else the replica count the run reported, else 1
//!   ([`CopiesSource::Assumed`]).
//! - **Cost:** `bytes / 1e9 × copies × price`, only when a price is set.
//! - A schema whose measurement `failed` keeps its row count, is left out of the totals and
//!   is listed in [`ValenceStorageView::failed_schemas`].

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::rows::{HistoryRow, OwnerSnapshot, SchemaSnapshot, SchemaSnapshotRow, StorageReadError};

/// A run older than this many hours is shown as [`SizeState::Stale`].
pub const STALE_AFTER_HOURS: i64 = 36;

/// Bytes in the gigabyte that prices are quoted per (decimal GB).
pub const BYTES_PER_GB: f64 = 1e9;

/// Fewest copies an owner can set.
pub const COPIES_MIN: i32 = 1;

/// Most copies an owner can set.
pub const COPIES_MAX: i32 = 10;

/// Currency used until the settings name another.
pub const DEFAULT_CURRENCY: &str = "USD";

/// How fresh the sizes are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SizeState {
    /// From a run under [`STALE_AFTER_HOURS`] old.
    Measured {
        /// UTC day of the run.
        ds: String,
        /// When the run started.
        measured_at: DateTime<Utc>,
    },
    /// From the newest run, which is older than [`STALE_AFTER_HOURS`].
    Stale {
        /// UTC day of the run.
        ds: String,
        /// When the run started.
        measured_at: DateTime<Utc>,
    },
    /// No run yet: rows are counted live and sizes are `None`.
    NotMeasured,
    /// Spectra couldn't be read: rows are counted live and sizes are `None`.
    Unavailable,
}

/// Where the copy count in the cost came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CopiesSource {
    /// Replicas the database reported in the newest run.
    Reported(i64),
    /// Set by a `ValenceAdmin` in the storage settings.
    Set(i64),
    /// Nothing reported or set; counts as one copy, so cost may be higher.
    Assumed,
}

impl CopiesSource {
    /// Copies the cost multiplies by.
    pub const fn count(self) -> i64 {
        match self {
            Self::Reported(n) | Self::Set(n) => n,
            Self::Assumed => 1,
        }
    }
}

/// Whether a storage price is set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PriceState {
    /// Price per GB each month.
    Set {
        /// Price of one GB for one month.
        per_gb_month: f64,
        /// ISO currency code.
        currency: String,
    },
    /// No price: sizes only, no cost.
    Unset,
}

/// Points for the 30-day chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HistoryState {
    /// One point per measured day, oldest first.
    Points(Vec<HistoryPoint>),
    /// No run in the window yet.
    NoneYet,
    /// Spectra couldn't be read.
    Down,
}

/// One day on the history chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryPoint {
    /// UTC day.
    pub ds: String,
    /// When that day's run started.
    pub measured_at: DateTime<Utc>,
    /// Database rows.
    pub rows: i64,
    /// Database bytes, when reported.
    pub bytes: Option<i64>,
    /// Monthly cost at today's copies and price, when both bytes and a price are known.
    pub cost: Option<f64>,
}

/// How a schema's size was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeasureHow {
    /// The backend reported bytes for the table.
    Measured,
    /// Rows times a sampled (or, for owners, the schema's) average row size.
    Estimated,
    /// The measurement timed out or errored; left out of the total.
    Failed,
    /// No run yet; only the live row count is known.
    NotMeasured,
}

impl MeasureHow {
    /// Parses the `how` column. Anything unrecognised counts as `Failed`, so an unknown value
    /// never reaches the total.
    pub fn from_column(how: &str) -> Self {
        match how {
            "measured" => Self::Measured,
            "estimated" => Self::Estimated,
            _ => Self::Failed,
        }
    }
}

/// One schema on the page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaRow {
    /// Valence table name.
    pub schema: String,
    /// Row count (an owner's rows on Your data), when known.
    pub rows: Option<i64>,
    /// Table plus index bytes (an owner's estimated bytes on Your data), when known.
    pub bytes: Option<i64>,
    /// Index bytes, when reported.
    pub index_bytes: Option<i64>,
    /// Average row size the estimate came from, when reported.
    pub avg_row_bytes: Option<i64>,
    /// How the size was found.
    pub how: MeasureHow,
    /// Fraction of the total bytes, `0.0..=1.0`, when both are known.
    pub share: Option<f64>,
    /// Monthly cost, when bytes and a price are known.
    pub cost: Option<f64>,
}

/// Sums over the schemas that were measured (failed schemas excluded).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StorageTotals {
    /// Rows, when any count is known.
    pub rows: Option<i64>,
    /// Bytes, when any size is known.
    pub bytes: Option<i64>,
    /// Index bytes, when any are reported.
    pub index_bytes: Option<i64>,
    /// Monthly cost, when bytes and a price are known.
    pub cost: Option<f64>,
}

/// The Storage page: the whole database by schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValenceStorageView {
    /// Freshness of the sizes.
    pub size: SizeState,
    /// Copy count behind the cost.
    pub copies: CopiesSource,
    /// Storage price.
    pub price: PriceState,
    /// 30-day history.
    pub history: HistoryState,
    /// Schemas, biggest first.
    pub schemas: Vec<SchemaRow>,
    /// Schemas whose measurement failed in the newest run.
    pub failed_schemas: Vec<String>,
    /// Totals over the measured schemas.
    pub totals: StorageTotals,
}

/// Your data: what the signed-in member's records take up.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MyStorageView {
    /// Freshness of the sizes.
    pub size: SizeState,
    /// Copy count behind the cost.
    pub copies: CopiesSource,
    /// Storage price.
    pub price: PriceState,
    /// The member's schemas, biggest first. Sizes are always estimates.
    pub schemas: Vec<SchemaRow>,
    /// The member's totals.
    pub totals: StorageTotals,
}

/// One of the biggest owners on the Storage page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OwnerTotal {
    /// Valence owner kind.
    pub owner_kind: String,
    /// Bare owner id.
    pub owner_id: String,
    /// Rows across schemas.
    pub rows: i64,
    /// Estimated bytes across schemas.
    pub est_bytes: i64,
    /// Monthly cost, when a price is set.
    pub cost: Option<f64>,
}

/// Everything `get_site_storage` returns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteStorage {
    /// The database by schema.
    pub view: ValenceStorageView,
    /// Biggest owners in the newest owner run.
    pub top_owners: Vec<OwnerTotal>,
    /// The newest run's id, when there is one.
    pub run_id: Option<String>,
    /// When the newest run started, when there is one.
    pub measured_at: Option<DateTime<Utc>>,
}

/// A live row count, used when no run can be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveSchemaCount {
    /// Valence table name.
    pub schema: String,
    /// Live row count, when the host can count it.
    pub rows: Option<i64>,
}

/// The `valence_storage_settings` row as the storage pages use it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageSettings {
    /// Copies kept, overriding what the database reports.
    pub copies_override: Option<i32>,
    /// Price of one GB for one month.
    pub price_per_gb_month: Option<f64>,
    /// ISO currency code of the price.
    pub currency: String,
    /// User who last saved the settings.
    pub updated_by: Option<String>,
    /// When the settings were last saved.
    pub updated_at: Option<DateTime<Utc>>,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            copies_override: None,
            price_per_gb_month: None,
            currency: DEFAULT_CURRENCY.to_string(),
            updated_by: None,
            updated_at: None,
        }
    }
}

/// Why `save_storage_settings` refused the input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StorageSettingsError {
    /// Copies outside the range [`COPIES_MIN`] to [`COPIES_MAX`].
    CopiesOutOfRange(i32),
    /// A price below zero.
    NegativePrice(f64),
    /// A NaN or infinite price.
    PriceNotFinite,
}

impl std::fmt::Display for StorageSettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CopiesOutOfRange(n) => {
                write!(
                    f,
                    "Copies kept must be {COPIES_MIN} to {COPIES_MAX}, not {n}"
                )
            }
            Self::NegativePrice(p) => write!(f, "Price per GB-month can't be negative ({p})"),
            Self::PriceNotFinite => f.write_str("Price per GB-month must be a number"),
        }
    }
}

impl std::error::Error for StorageSettingsError {}

/// Checks the inputs of `save_storage_settings`: copies `1..=10`, price finite and `>= 0`.
pub fn validate_storage_settings(
    copies_override: Option<i32>,
    price_per_gb_month: Option<f64>,
) -> Result<(), StorageSettingsError> {
    if let Some(n) = copies_override {
        if !(COPIES_MIN..=COPIES_MAX).contains(&n) {
            return Err(StorageSettingsError::CopiesOutOfRange(n));
        }
    }
    if let Some(p) = price_per_gb_month {
        if !p.is_finite() {
            return Err(StorageSettingsError::PriceNotFinite);
        }
        if p < 0.0 {
            return Err(StorageSettingsError::NegativePrice(p));
        }
    }
    Ok(())
}

/// Builds the Storage page state.
pub fn build_site_view(
    snapshot: Result<Option<SchemaSnapshot>, StorageReadError>,
    history: Result<Vec<HistoryRow>, StorageReadError>,
    settings: &StorageSettings,
    live: &[LiveSchemaCount],
    now: DateTime<Utc>,
) -> ValenceStorageView {
    let reported = snapshot
        .as_ref()
        .ok()
        .and_then(Option::as_ref)
        .and_then(|s| s.total.as_ref())
        .and_then(|t| t.copies_reported);
    let copies = copies_for(settings, reported);
    let price = price_for(settings);

    let (size, mut schemas, failed_schemas) = match snapshot {
        Ok(Some(snap)) => {
            let mut failed = Vec::new();
            let rows = snap
                .schemas
                .iter()
                .map(|r| {
                    let row = measured_row(r);
                    if row.how == MeasureHow::Failed {
                        failed.push(row.schema.clone());
                    }
                    row
                })
                .collect();
            (size_for(snap.ds, snap.measured_at, now), rows, failed)
        }
        Ok(None) => (SizeState::NotMeasured, live_rows(live), Vec::new()),
        Err(_) => (SizeState::Unavailable, live_rows(live), Vec::new()),
    };
    let totals = finish_rows(&mut schemas, copies, &price);

    let history = match history {
        Ok(rows) if rows.is_empty() => HistoryState::NoneYet,
        Ok(rows) => HistoryState::Points(
            rows.into_iter()
                .map(|r| HistoryPoint {
                    cost: cost_of(r.bytes, copies, &price),
                    ds: r.ds,
                    measured_at: r.measured_at,
                    rows: r.rows,
                    bytes: r.bytes,
                })
                .collect(),
        ),
        Err(_) => HistoryState::Down,
    };

    ValenceStorageView {
        size,
        copies,
        price,
        history,
        schemas,
        failed_schemas,
        totals,
    }
}

/// Builds Your data's storage state for one member.
///
/// `schema_snapshot` supplies each schema's average row size and the reported copies; pass
/// `None` when it couldn't be read.
pub fn build_owner_view(
    snapshot: Result<Option<OwnerSnapshot>, StorageReadError>,
    schema_snapshot: Option<&SchemaSnapshot>,
    settings: &StorageSettings,
    live: &[LiveSchemaCount],
    now: DateTime<Utc>,
) -> MyStorageView {
    let reported = schema_snapshot
        .and_then(|s| s.total.as_ref())
        .and_then(|t| t.copies_reported);
    let copies = copies_for(settings, reported);
    let price = price_for(settings);
    let avg_row_bytes = |schema: &str| {
        schema_snapshot
            .and_then(|s| s.schemas.iter().find(|r| r.schema == schema))
            .and_then(|r| r.avg_row_bytes)
    };

    let (size, mut schemas) = match snapshot {
        Ok(Some(snap)) => {
            let rows = snap
                .schemas
                .iter()
                .map(|r| SchemaRow {
                    schema: r.schema.clone(),
                    rows: Some(r.rows),
                    bytes: Some(r.est_bytes),
                    index_bytes: None,
                    avg_row_bytes: avg_row_bytes(&r.schema),
                    how: MeasureHow::Estimated,
                    share: None,
                    cost: None,
                })
                .collect();
            (size_for(snap.ds, snap.measured_at, now), rows)
        }
        Ok(None) => (SizeState::NotMeasured, live_rows(live)),
        Err(_) => (SizeState::Unavailable, live_rows(live)),
    };
    let totals = finish_rows(&mut schemas, copies, &price);

    MyStorageView {
        size,
        copies,
        price,
        schemas,
        totals,
    }
}

impl OwnerTotal {
    /// An owner's `*` row with its cost at the given copies and price.
    pub fn from_row(
        row: &super::rows::OwnerSnapshotRow,
        copies: CopiesSource,
        price: &PriceState,
    ) -> Self {
        Self {
            owner_kind: row.owner_kind.clone(),
            owner_id: row.owner_id.clone(),
            rows: row.rows,
            est_bytes: row.est_bytes,
            cost: cost_of(Some(row.est_bytes), copies, price),
        }
    }
}

fn copies_for(settings: &StorageSettings, reported: Option<i64>) -> CopiesSource {
    if let Some(n) = settings.copies_override {
        return CopiesSource::Set(i64::from(n));
    }
    match reported {
        Some(n) if n >= 1 => CopiesSource::Reported(n),
        _ => CopiesSource::Assumed,
    }
}

fn price_for(settings: &StorageSettings) -> PriceState {
    match settings.price_per_gb_month {
        Some(p) if p.is_finite() && p >= 0.0 => PriceState::Set {
            per_gb_month: p,
            currency: settings.currency.clone(),
        },
        _ => PriceState::Unset,
    }
}

fn size_for(ds: String, measured_at: DateTime<Utc>, now: DateTime<Utc>) -> SizeState {
    if now - measured_at > Duration::hours(STALE_AFTER_HOURS) {
        SizeState::Stale { ds, measured_at }
    } else {
        SizeState::Measured { ds, measured_at }
    }
}

#[allow(clippy::cast_precision_loss)] // byte counts stay far below 2^53
fn cost_of(bytes: Option<i64>, copies: CopiesSource, price: &PriceState) -> Option<f64> {
    match price {
        PriceState::Set { per_gb_month, .. } => {
            bytes.map(|b| b as f64 / BYTES_PER_GB * copies.count() as f64 * per_gb_month)
        }
        PriceState::Unset => None,
    }
}

fn measured_row(r: &SchemaSnapshotRow) -> SchemaRow {
    let how = MeasureHow::from_column(&r.how);
    let failed = how == MeasureHow::Failed;
    SchemaRow {
        schema: r.schema.clone(),
        rows: Some(r.rows),
        bytes: if failed {
            None
        } else {
            r.table_bytes.map(|t| t + r.index_bytes.unwrap_or(0))
        },
        index_bytes: if failed { None } else { r.index_bytes },
        avg_row_bytes: r.avg_row_bytes,
        how,
        share: None,
        cost: None,
    }
}

fn live_rows(live: &[LiveSchemaCount]) -> Vec<SchemaRow> {
    live.iter()
        .map(|l| SchemaRow {
            schema: l.schema.clone(),
            rows: l.rows,
            bytes: None,
            index_bytes: None,
            avg_row_bytes: None,
            how: MeasureHow::NotMeasured,
            share: None,
            cost: None,
        })
        .collect()
}

/// Fills share and cost, sorts biggest first, and sums the rows that weren't `Failed`.
#[allow(clippy::cast_precision_loss)]
fn finish_rows(rows: &mut [SchemaRow], copies: CopiesSource, price: &PriceState) -> StorageTotals {
    let counted = || rows.iter().filter(|r| r.how != MeasureHow::Failed);
    let sum = |values: Vec<Option<i64>>| -> Option<i64> {
        values
            .iter()
            .any(Option::is_some)
            .then(|| values.iter().flatten().sum())
    };
    let total_rows = sum(counted().map(|r| r.rows).collect());
    let total_bytes = sum(counted().map(|r| r.bytes).collect());
    let total_index = sum(counted().map(|r| r.index_bytes).collect());

    for row in rows.iter_mut() {
        row.share = match (row.bytes, total_bytes) {
            (Some(b), Some(t)) if t > 0 => Some(b as f64 / t as f64),
            _ => None,
        };
        row.cost = cost_of(row.bytes, copies, price);
    }
    rows.sort_by(|a, b| {
        b.bytes
            .cmp(&a.bytes)
            .then_with(|| b.rows.cmp(&a.rows))
            .then_with(|| a.schema.cmp(&b.schema))
    });

    StorageTotals {
        rows: total_rows,
        bytes: total_bytes,
        index_bytes: total_index,
        cost: cost_of(total_bytes, copies, price),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

    use super::*;
    use crate::storage::rows::{OwnerSnapshotRow, ALL_SCHEMAS};

    fn schema_row(schema: &str, rows: i64, bytes: Option<i64>, how: &str) -> SchemaSnapshotRow {
        SchemaSnapshotRow {
            ds: "2026-11-24".into(),
            measured_at: now() - Duration::hours(2),
            run_id: "run-1".into(),
            schema: schema.into(),
            backend: "postgres".into(),
            rows,
            table_bytes: bytes,
            index_bytes: bytes.map(|_| 0),
            how: how.into(),
            avg_row_bytes: Some(100),
            copies_reported: None,
        }
    }

    fn snapshot(
        measured_at: DateTime<Utc>,
        copies_reported: Option<i64>,
        schemas: Vec<SchemaSnapshotRow>,
    ) -> SchemaSnapshot {
        let mut total = schema_row(ALL_SCHEMAS, 0, Some(0), "measured");
        total.copies_reported = copies_reported;
        SchemaSnapshot {
            ds: "2026-11-24".into(),
            run_id: "run-1".into(),
            measured_at,
            total: Some(total),
            schemas,
        }
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-24T18:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn priced(copies: Option<i32>) -> StorageSettings {
        StorageSettings {
            copies_override: copies,
            price_per_gb_month: Some(0.10),
            ..StorageSettings::default()
        }
    }

    fn live() -> Vec<LiveSchemaCount> {
        vec![
            LiveSchemaCount {
                schema: "todo".into(),
                rows: Some(14),
            },
            LiveSchemaCount {
                schema: "tag".into(),
                rows: Some(6),
            },
        ]
    }

    #[test]
    fn no_snapshot_is_not_measured_with_live_row_counts() {
        let view = build_site_view(Ok(None), Ok(Vec::new()), &priced(None), &live(), now());
        assert_eq!(view.size, SizeState::NotMeasured);
        assert_eq!(view.history, HistoryState::NoneYet);
        assert_eq!(view.copies, CopiesSource::Assumed);
        assert_eq!(view.schemas[0].schema, "todo");
        assert_eq!(view.schemas[0].rows, Some(14));
        assert!(view
            .schemas
            .iter()
            .all(|r| r.bytes.is_none() && r.cost.is_none()));
        assert!(view
            .schemas
            .iter()
            .all(|r| r.how == MeasureHow::NotMeasured));
        assert_eq!(view.totals.rows, Some(20));
        assert_eq!(view.totals.bytes, None);
        assert_eq!(view.totals.cost, None);

        let mine = build_owner_view(Ok(None), None, &priced(None), &live(), now());
        assert_eq!(mine.size, SizeState::NotMeasured);
        assert_eq!(mine.totals.rows, Some(20));
        assert_eq!(mine.totals.bytes, None);
    }

    #[test]
    fn spectra_error_is_unavailable_and_history_down() {
        let err = || StorageReadError::new("backend down");
        let view = build_site_view(Err(err()), Err(err()), &priced(None), &live(), now());
        assert_eq!(view.size, SizeState::Unavailable);
        assert_eq!(view.history, HistoryState::Down);
        assert_eq!(view.schemas.len(), 2);

        let mine = build_owner_view(Err(err()), None, &priced(None), &live(), now());
        assert_eq!(mine.size, SizeState::Unavailable);
    }

    #[test]
    fn a_forty_hour_old_run_is_stale() {
        let at = now() - Duration::hours(40);
        let view = build_site_view(
            Ok(Some(snapshot(
                at,
                None,
                vec![schema_row("todo", 1, Some(10), "measured")],
            ))),
            Ok(Vec::new()),
            &priced(None),
            &[],
            now(),
        );
        assert_eq!(
            view.size,
            SizeState::Stale {
                ds: "2026-11-24".into(),
                measured_at: at
            }
        );

        let fresh = now() - Duration::hours(35);
        let view = build_site_view(
            Ok(Some(snapshot(fresh, None, Vec::new()))),
            Ok(Vec::new()),
            &priced(None),
            &[],
            now(),
        );
        assert!(matches!(view.size, SizeState::Measured { .. }));
    }

    #[test]
    fn failed_schema_is_left_out_of_the_total_and_listed() {
        let view = build_site_view(
            Ok(Some(snapshot(
                now(),
                None,
                vec![
                    schema_row("todo", 10, Some(3_000), "measured"),
                    schema_row("tag", 5, Some(1_000), "estimated"),
                    schema_row("notification", 900, Some(9_000_000), "failed"),
                ],
            ))),
            Ok(Vec::new()),
            &priced(None),
            &[],
            now(),
        );
        assert_eq!(view.failed_schemas, vec!["notification".to_string()]);
        assert_eq!(view.totals.bytes, Some(4_000));
        assert_eq!(view.totals.rows, Some(15));
        let failed = view
            .schemas
            .iter()
            .find(|r| r.schema == "notification")
            .expect("failed row kept");
        assert_eq!(failed.how, MeasureHow::Failed);
        assert_eq!(failed.bytes, None);
        assert_eq!(failed.rows, Some(900));
        let todo = &view.schemas[0];
        assert_eq!(todo.schema, "todo");
        assert_eq!(todo.share, Some(0.75));
    }

    #[test]
    fn copies_override_beats_reported_beats_assumed() {
        let snap = || Ok(Some(snapshot(now(), Some(3), Vec::new())));
        let set = build_site_view(snap(), Ok(Vec::new()), &priced(Some(2)), &[], now());
        assert_eq!(set.copies, CopiesSource::Set(2));

        let reported = build_site_view(snap(), Ok(Vec::new()), &priced(None), &[], now());
        assert_eq!(reported.copies, CopiesSource::Reported(3));

        let assumed = build_site_view(
            Ok(Some(snapshot(now(), None, Vec::new()))),
            Ok(Vec::new()),
            &priced(None),
            &[],
            now(),
        );
        assert_eq!(assumed.copies, CopiesSource::Assumed);
        assert_eq!(assumed.copies.count(), 1);
    }

    #[test]
    fn cost_is_gigabytes_times_copies_times_price() {
        let view = build_site_view(
            Ok(Some(snapshot(
                now(),
                Some(2),
                vec![schema_row("todo", 1, Some(5_000_000_000), "measured")],
            ))),
            Ok(Vec::new()),
            &priced(None),
            &[],
            now(),
        );
        // 5 GB × 2 copies × 0.10 a GB-month.
        assert!((view.totals.cost.unwrap() - 1.0).abs() < 1e-9);
        assert!((view.schemas[0].cost.unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn no_price_gives_no_cost() {
        let settings = StorageSettings::default();
        let view = build_site_view(
            Ok(Some(snapshot(
                now(),
                Some(2),
                vec![schema_row("todo", 1, Some(5_000_000_000), "measured")],
            ))),
            Ok(vec![HistoryRow {
                ds: "2026-11-24".into(),
                measured_at: now(),
                rows: 1,
                bytes: Some(5_000_000_000),
            }]),
            &settings,
            &[],
            now(),
        );
        assert_eq!(view.price, PriceState::Unset);
        assert_eq!(view.totals.bytes, Some(5_000_000_000));
        assert_eq!(view.totals.cost, None);
        assert!(view.schemas.iter().all(|r| r.cost.is_none()));
        let HistoryState::Points(points) = view.history else {
            panic!("points expected");
        };
        assert_eq!(points[0].cost, None);
    }

    #[test]
    fn owner_view_estimates_from_owner_rows() {
        let owner_row = |schema: &str, rows: i64, est_bytes: i64| OwnerSnapshotRow {
            ds: "2026-11-24".into(),
            measured_at: now(),
            run_id: "run-1".into(),
            owner_kind: "user".into(),
            owner_id: "u1".into(),
            schema: schema.into(),
            rows,
            est_bytes,
        };
        let schemas = snapshot(
            now(),
            Some(2),
            vec![schema_row("todo", 100, Some(10), "measured")],
        );
        let mine = build_owner_view(
            Ok(Some(OwnerSnapshot {
                ds: "2026-11-24".into(),
                run_id: "run-1".into(),
                measured_at: now(),
                total: Some(owner_row(ALL_SCHEMAS, 20, 2_000_000_000)),
                schemas: vec![
                    owner_row("todo", 14, 1_500_000_000),
                    owner_row("tag", 6, 500_000_000),
                ],
            })),
            Some(&schemas),
            &priced(None),
            &live(),
            now(),
        );
        assert!(matches!(mine.size, SizeState::Measured { .. }));
        assert_eq!(mine.copies, CopiesSource::Reported(2));
        assert_eq!(mine.schemas[0].schema, "todo");
        assert_eq!(mine.schemas[0].how, MeasureHow::Estimated);
        assert_eq!(mine.schemas[0].avg_row_bytes, Some(100));
        assert_eq!(mine.totals.bytes, Some(2_000_000_000));
        // 2 GB × 2 copies × 0.10.
        assert!((mine.totals.cost.unwrap() - 0.4).abs() < 1e-9);
    }

    #[test]
    fn settings_validation_bounds() {
        assert_eq!(
            validate_storage_settings(Some(0), None),
            Err(StorageSettingsError::CopiesOutOfRange(0))
        );
        assert!(validate_storage_settings(Some(1), None).is_ok());
        assert!(validate_storage_settings(Some(10), None).is_ok());
        assert_eq!(
            validate_storage_settings(Some(11), None),
            Err(StorageSettingsError::CopiesOutOfRange(11))
        );
        assert!(validate_storage_settings(None, Some(0.0)).is_ok());
        assert!(matches!(
            validate_storage_settings(None, Some(-0.01)),
            Err(StorageSettingsError::NegativePrice(_))
        ));
        assert_eq!(
            validate_storage_settings(None, Some(f64::NAN)),
            Err(StorageSettingsError::PriceNotFinite)
        );
        assert!(validate_storage_settings(None, None).is_ok());
    }

    #[test]
    fn unknown_how_counts_as_failed() {
        assert_eq!(MeasureHow::from_column("measured"), MeasureHow::Measured);
        assert_eq!(MeasureHow::from_column("estimated"), MeasureHow::Estimated);
        assert_eq!(MeasureHow::from_column("failed"), MeasureHow::Failed);
        assert_eq!(MeasureHow::from_column("guessed"), MeasureHow::Failed);
    }
}
