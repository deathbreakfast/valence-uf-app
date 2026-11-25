//! Typed rows read back from the storage snapshot tables.
//!
//! These carry no Spectra types, so the view code and its tests build without the `spectra`
//! feature. Parsing follows the [row contract](super): `-1`, null or a missing value in a byte
//! column means "not reported".

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `schema` value of a total row: the database total, or an owner's total.
pub const ALL_SCHEMAS: &str = "*";

/// Sentinel written to an `i64` byte column the job couldn't measure.
pub const NOT_REPORTED: i64 = -1;

/// A Spectra query failed (backend error, bad query), as opposed to "no rows yet".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageReadError {
    /// Operator-facing summary of the failure.
    pub message: String,
}

impl StorageReadError {
    /// Wraps a failure message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for StorageReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "storage snapshot read failed: {}", self.message)
    }
}

impl std::error::Error for StorageReadError {}

/// Owner whose rows [`latest_owner_snapshot`](super::latest_owner_snapshot) reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerKey {
    /// Valence owner kind (`user`, `service_user`, `team`, ...).
    pub kind: String,
    /// Bare owner id (`u42`).
    pub id: String,
}

impl OwnerKey {
    /// Key for a signed-in user.
    pub fn user(user_id: impl Into<String>) -> Self {
        Self {
            kind: "user".to_string(),
            id: user_id.into(),
        }
    }
}

/// One `valence_storage_schema_daily` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaSnapshotRow {
    /// UTC day of the run, `YYYY-MM-DD`.
    pub ds: String,
    /// When the run started.
    pub measured_at: DateTime<Utc>,
    /// Run that wrote the row.
    pub run_id: String,
    /// Valence table name, or [`ALL_SCHEMAS`] for the database total.
    pub schema: String,
    /// Storage backend (`postgres`, `sqlite`, ...).
    pub backend: String,
    /// Row count at measurement time.
    pub rows: i64,
    /// Table bytes without indexes, when reported.
    pub table_bytes: Option<i64>,
    /// Index bytes, when reported.
    pub index_bytes: Option<i64>,
    /// `measured`, `estimated` or `failed`.
    pub how: String,
    /// Average encoded row size, when reported.
    pub avg_row_bytes: Option<i64>,
    /// Replica count the database reports (on the `*` row only).
    pub copies_reported: Option<i64>,
}

impl SchemaSnapshotRow {
    /// Parses one event row's fields; `None` when a required column is missing.
    ///
    /// `ts` stands in for `measured_at` when that column is missing or unparsable.
    pub fn from_fields(fields: &Value, ts: DateTime<Utc>) -> Option<Self> {
        Some(Self {
            ds: str_field(fields, "ds")?,
            measured_at: measured_at(fields, ts),
            run_id: str_field(fields, "run_id")?,
            schema: str_field(fields, "schema")?,
            backend: str_field(fields, "backend").unwrap_or_default(),
            rows: int_field(fields, "rows").unwrap_or(0),
            table_bytes: reported(fields, "table_bytes"),
            index_bytes: reported(fields, "index_bytes"),
            how: str_field(fields, "how").unwrap_or_default(),
            avg_row_bytes: reported(fields, "avg_row_bytes"),
            copies_reported: reported(fields, "copies_reported"),
        })
    }
}

/// One `valence_storage_owner_daily` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerSnapshotRow {
    /// UTC day of the run, `YYYY-MM-DD`.
    pub ds: String,
    /// When the run started.
    pub measured_at: DateTime<Utc>,
    /// Run that wrote the row.
    pub run_id: String,
    /// Valence owner kind.
    pub owner_kind: String,
    /// Bare owner id.
    pub owner_id: String,
    /// Valence table name, or [`ALL_SCHEMAS`] for the owner's total.
    pub schema: String,
    /// Rows the owner has in the schema.
    pub rows: i64,
    /// `rows` times the schema's average row size.
    pub est_bytes: i64,
}

impl OwnerSnapshotRow {
    /// Parses one event row's fields; `None` when a required column is missing.
    pub fn from_fields(fields: &Value, ts: DateTime<Utc>) -> Option<Self> {
        Some(Self {
            ds: str_field(fields, "ds")?,
            measured_at: measured_at(fields, ts),
            run_id: str_field(fields, "run_id")?,
            owner_kind: str_field(fields, "owner_kind")?,
            owner_id: str_field(fields, "owner_id")?,
            schema: str_field(fields, "schema")?,
            rows: int_field(fields, "rows").unwrap_or(0),
            est_bytes: int_field(fields, "est_bytes").unwrap_or(0).max(0),
        })
    }
}

/// Every schema row of the newest run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaSnapshot {
    /// UTC day of the run.
    pub ds: String,
    /// Run id.
    pub run_id: String,
    /// When the run started.
    pub measured_at: DateTime<Utc>,
    /// The `*` database total row, when the run wrote one.
    pub total: Option<SchemaSnapshotRow>,
    /// Per-schema rows, sorted by schema name.
    pub schemas: Vec<SchemaSnapshotRow>,
}

/// One owner's rows from the newest owner run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerSnapshot {
    /// UTC day of the run.
    pub ds: String,
    /// Run id.
    pub run_id: String,
    /// When the run started.
    pub measured_at: DateTime<Utc>,
    /// The owner's `*` total row, when the run wrote one.
    pub total: Option<OwnerSnapshotRow>,
    /// Per-schema rows, sorted by schema name. Empty when the owner had no rows that day.
    pub schemas: Vec<OwnerSnapshotRow>,
}

/// The database total of one day, for the history chart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryRow {
    /// UTC day.
    pub ds: String,
    /// When that day's newest run started.
    pub measured_at: DateTime<Utc>,
    /// Database rows.
    pub rows: i64,
    /// Table plus index bytes, when reported.
    pub bytes: Option<i64>,
}

/// A string column (a number is accepted and stringified).
pub fn str_field(fields: &Value, key: &str) -> Option<String> {
    match fields.get(key)? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[allow(clippy::cast_possible_truncation)] // a float byte count rounds to the nearest byte
fn int_field(fields: &Value, key: &str) -> Option<i64> {
    match fields.get(key)? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// A byte column: negative (the `-1` sentinel), null or missing all mean "not reported".
fn reported(fields: &Value, key: &str) -> Option<i64> {
    int_field(fields, key).filter(|v| *v >= 0)
}

fn measured_at(fields: &Value, ts: DateTime<Utc>) -> DateTime<Utc> {
    str_field(fields, "measured_at")
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map_or(ts, |d| d.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use serde_json::json;

    #[test]
    fn sentinel_null_and_missing_bytes_parse_as_unknown() {
        let ts = Utc::now();
        let row = SchemaSnapshotRow::from_fields(
            &json!({
                "ds": "2026-11-24", "run_id": "r1", "schema": "todo", "backend": "sqlite",
                "rows": 10, "table_bytes": -1, "index_bytes": null, "how": "estimated",
                "measured_at": "2026-11-24T11:20:00Z",
            }),
            ts,
        )
        .expect("row");
        assert_eq!(row.table_bytes, None);
        assert_eq!(row.index_bytes, None);
        assert_eq!(row.avg_row_bytes, None);
        assert_eq!(row.copies_reported, None);
        assert_eq!(row.measured_at.to_rfc3339(), "2026-11-24T11:20:00+00:00");
    }

    #[test]
    fn missing_measured_at_falls_back_to_event_ts() {
        let ts = Utc::now();
        let row = OwnerSnapshotRow::from_fields(
            &json!({
                "ds": "2026-11-24", "run_id": "r1", "owner_kind": "user", "owner_id": "u1",
                "schema": "*", "rows": 3, "est_bytes": 300,
            }),
            ts,
        )
        .expect("row");
        assert_eq!(row.measured_at, ts);
        assert_eq!(row.est_bytes, 300);
    }

    #[test]
    fn row_without_ds_is_skipped() {
        assert!(SchemaSnapshotRow::from_fields(&json!({ "run_id": "r1" }), Utc::now()).is_none());
    }
}
