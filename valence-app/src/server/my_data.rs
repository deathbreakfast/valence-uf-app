//! Your data server functions: what Valence holds about the signed-in member.
//!
//! Everything here runs as the Higgs session actor. Valence has no "list my record
//! ids" query yet, so [`scan_owned`] pages a schema's rows the member can read and keeps
//! the ones whose ownership row names them, stopping once the owner summary's count is
//! reached. Swap it for an `OwnershipService` listing once Valence ships one.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
use valence::actor::Actor;
#[cfg(feature = "ssr")]
use valence::owner_ref::OwnerKind;
#[cfg(feature = "ssr")]
use valence::ownership::OwnershipService;

/// Newest records shown per schema card on the Your data page.
pub(crate) const MY_DATA_NEWEST: usize = 3;

/// Caption shown when a schema declares no retention.
pub(crate) const DEFAULT_RETENTION: &str = "Kept until you delete them or close your account";

/// Most readable rows one scan reads per schema before it stops and reports the
/// result as cut short.
///
/// The owner summary also counts records the member can't read, so a scan that
/// never reaches that count would otherwise walk the whole table.
pub(crate) const MAX_SCAN_ROWS: usize = 5_000;

/// Fields tried, in order, for a record's display label before falling back to its id.
const LABEL_FIELDS: &[&str] = &["title", "name", "label", "subject", "email"];

/// Fields tried, in order, for a record's created time.
const CREATED_FIELDS: &[&str] = &["created_at", "createdAt", "created"];

/// What Valence holds about the signed-in member.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MyData {
    /// `false` when the session actor isn't a user (service or system callers).
    pub viewer_is_user: bool,
    /// Records the member owns, including those being deleted.
    pub records: u64,
    /// Schemas holding at least one of those records.
    pub kinds: u64,
    /// Records queued for deletion.
    pub being_deleted: u64,
    /// One entry per schema, largest first.
    pub schemas: Vec<MyDataSchema>,
}

/// One kind of record the member owns.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MyDataSchema {
    pub name: String,
    /// Records of this kind the member owns, including those being deleted.
    pub count: u64,
    pub being_deleted: u64,
    /// Read purposes declared for this schema, as plain sentences.
    pub why_kept: Vec<String>,
    /// `true` when the data-use catalog lookup failed, so `why_kept` is unknown
    /// rather than empty.
    pub why_kept_failed: bool,
    /// Retention caption.
    pub retention: String,
    /// Newest few records the member can read.
    pub newest: Vec<MyDataRecord>,
    /// How many owned records aren't listed in `newest`.
    pub more: u64,
}

/// One record row on a Your data card.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MyDataRecord {
    pub id: String,
    pub label: String,
    /// RFC 3339 created time, when the record carries one.
    pub created_at: Option<String>,
}

/// Turns a Markdown purpose into a plain sentence: drops `**` bold markers and
/// collapses whitespace.
pub(crate) fn plain_purpose(purpose: &str) -> String {
    purpose
        .replace("**", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Retention caption for a schema's declared retention text.
pub(crate) fn retention_caption(retention: &str) -> String {
    let retention = retention.trim();
    if retention.is_empty() {
        DEFAULT_RETENTION.to_string()
    } else {
        format!("Retention: {retention}")
    }
}

/// How many owned records a card doesn't list.
pub(crate) const fn more_count(count: u64, shown: usize) -> u64 {
    count.saturating_sub(shown as u64)
}

/// Bare record id from a row's `id`, which may be `table:id`, a bare id, or a
/// `{ tb, id }` record object.
pub(crate) fn row_id(row: &serde_json::Value) -> Option<String> {
    let raw = row.get("id")?;
    let id = match raw {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Object(o) => match o.get("id")? {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        },
        _ => return None,
    };
    let id = id.trim();
    (!id.is_empty()).then(|| id.to_string())
}

/// Display label: the first non-empty title-like string field, else the id.
pub(crate) fn record_label(row: &serde_json::Value, id: &str) -> String {
    LABEL_FIELDS
        .iter()
        .find_map(|f| {
            row.get(*f)
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| id.to_string())
}

/// Created time as RFC 3339 when the row carries a parseable one.
pub(crate) fn record_created_at(row: &serde_json::Value) -> Option<String> {
    CREATED_FIELDS.iter().find_map(|f| {
        let raw = row.get(*f)?.as_str()?;
        chrono::DateTime::parse_from_rfc3339(raw)
            .ok()
            .map(|t| t.with_timezone(&chrono::Utc).to_rfc3339())
    })
}

/// `true` when an ownership row names `user_id` as its user owner.
///
/// Ownership rows may store the id bare or as `user:<id>`, so both sides drop the
/// prefix before comparing.
pub(crate) fn ownership_names_user(ownership: &serde_json::Value, user_id: &str) -> bool {
    let bare = |s: &str| s.strip_prefix("user:").unwrap_or(s).to_string();
    let is_user = ownership
        .get("owner_type")
        .and_then(|v| v.as_str())
        .is_some_and(|t| t == "user");
    let owner = ownership.get("owner_id").and_then(|v| v.as_str());
    let live = ownership
        .get("status")
        .and_then(|v| v.as_str())
        .is_none_or(|s| s == "active" || s == "pending_deletion");
    is_user && live && owner.is_some_and(|o| bare(o) == bare(user_id))
}

/// Field to sort a schema's rows newest first: a created-time field when the schema
/// has one, else `id`.
#[cfg(feature = "ssr")]
fn newest_first_field(schema_name: &str) -> String {
    super::registry::get_schema_metadata_by_name(schema_name)
        .and_then(|s| {
            s.fields
                .iter()
                .find(|f| CREATED_FIELDS.contains(&f.name.as_str()))
                .map(|f| f.name.clone())
        })
        .unwrap_or_else(|| "id".to_string())
}

/// Owned rows a scan found, and whether it stopped at [`MAX_SCAN_ROWS`] first.
#[cfg(feature = "ssr")]
struct ScanResult {
    rows: Vec<serde_json::Value>,
    cut_short: bool,
}

/// Pages `table` newest first under the session `v` and keeps rows whose ownership
/// row names `user_id`, stopping after `want` matches, when the table ends, or after
/// [`MAX_SCAN_ROWS`] readable rows.
///
/// Each row costs one ownership lookup until Valence ships an owner listing query.
///
/// Rows the member can't read never come back from the query, so this can't reveal
/// anything their session couldn't already open.
#[cfg(feature = "ssr")]
async fn scan_owned(
    v: &valence::Valence,
    table: &str,
    user_id: &str,
    want: usize,
    purpose: valence::data_use::DataUsePurpose,
) -> Result<ScanResult, ServerFnError> {
    use valence::query::{QueryCore, SortDirection};

    const PAGE: u32 = 100;
    let mut out = Vec::new();
    if want == 0 {
        return Ok(ScanResult {
            rows: out,
            cut_short: false,
        });
    }
    let mut scanned = 0usize;
    let order_field = newest_first_field(table);
    let mut resume: Option<u32> = None;
    loop {
        let mut query = QueryCore::new(table.to_string())
            .order_by(order_field.clone(), SortDirection::Desc)
            .limit(PAGE);
        if let Some(position) = resume {
            query = query.resume_at(position);
        }
        let (rows, stats) = query
            .execute_with_stats::<serde_json::Value>(v, purpose)
            .await
            .map_err(|e| super::helpers::io_error(e.to_string()))?;
        for row in rows {
            if scanned >= MAX_SCAN_ROWS {
                return Ok(ScanResult {
                    rows: out,
                    cut_short: true,
                });
            }
            scanned += 1;
            let Some(id) = row_id(&row) else {
                continue;
            };
            let rid = valence::ownership::normalize_record_id_for_ownership(&id);
            let ownership = OwnershipService::get_ownership_json(table, &rid, v)
                .await
                .map_err(|e| super::helpers::io_error(e.to_string()))?;
            if ownership
                .as_ref()
                .is_some_and(|o| ownership_names_user(o, user_id))
            {
                out.push(row);
                if out.len() >= want {
                    return Ok(ScanResult {
                        rows: out,
                        cut_short: false,
                    });
                }
            }
        }
        match stats.resume {
            Some(position) if resume != Some(position) => resume = Some(position),
            _ => break,
        }
    }
    Ok(ScanResult {
        rows: out,
        cut_short: false,
    })
}

/// Read purposes declared for `schema_name`, as plain sentences.
///
/// # Errors
///
/// Returns the catalog lookup's error, including the not-installed error when the
/// host never installed a data-use catalog.
#[cfg(feature = "ssr")]
async fn why_kept(schema_name: &str) -> Result<Vec<String>, ServerFnError> {
    let rows = super::data_uses::get_schema_data_uses(schema_name.to_string()).await?;
    let mut out: Vec<String> = Vec::new();
    for row in rows {
        if row.op != "read" || row.source_schema.is_some() {
            continue;
        }
        let sentence = plain_purpose(&row.purpose);
        if !sentence.is_empty() && !out.contains(&sentence) {
            out.push(sentence);
        }
    }
    Ok(out)
}

/// The session's user id, or `None` for service and system actors.
#[cfg(feature = "ssr")]
fn session_user_id(v: &valence::Valence) -> Option<String> {
    match v.actor() {
        Actor::User { user_id } => Some(user_id.clone()),
        _ => None,
    }
}

/// What Valence holds about the signed-in member, grouped by schema.
///
/// Counts come from the ownership summary; the newest records come from
/// [`scan_owned`] under the member's own session.
#[uf_product_macros::server]
pub async fn get_my_data() -> Result<MyData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let v = super::helpers::viewer_valence().await?;
        let Some(user_id) = session_user_id(&v) else {
            return Ok(MyData::default());
        };
        let summary = OwnershipService::owner_data_summary(&user_id, OwnerKind::User.as_str(), &v)
            .await
            .map_err(|e| super::helpers::io_error(e.to_string()))?;

        let mut schemas = Vec::with_capacity(summary.rows_by_schema.len());
        for row in summary.rows_by_schema {
            let count = row.active_rows + row.pending_deletion_rows;
            let want = usize::try_from(count)
                .unwrap_or(usize::MAX)
                .min(MY_DATA_NEWEST);
            let rows = scan_owned(
                &v,
                &row.valence_model,
                &user_id,
                want,
                valence::use_!(r#"In the **Valence ops console** **Your data** page, we **list the newest records a member owns** in each schema to show a member what Valence holds about them. Only the signed-in member sees their own records."#),
            )
            .await?
            .rows;
            let newest: Vec<MyDataRecord> = rows
                .iter()
                .filter_map(|r| {
                    let id = row_id(r)?;
                    Some(MyDataRecord {
                        label: record_label(r, &id),
                        created_at: record_created_at(r),
                        id: valence::ownership::normalize_record_id_for_ownership(&id),
                    })
                })
                .collect();
            let retention = super::registry::get_schema_metadata_by_name(&row.valence_model)
                .map(|s| s.meta.retention)
                .unwrap_or_default();
            let (why_kept, why_kept_failed) = match why_kept(&row.valence_model).await {
                Ok(lines) => (lines, false),
                Err(e) => {
                    tracing::warn!(
                        target = "valence_app::my_data",
                        schema = %row.valence_model,
                        error = %e,
                        "data-use catalog lookup failed for Your data"
                    );
                    (Vec::new(), true)
                }
            };
            schemas.push(MyDataSchema {
                why_kept,
                why_kept_failed,
                retention: retention_caption(&retention),
                more: more_count(count, newest.len()),
                newest,
                count,
                being_deleted: row.pending_deletion_rows,
                name: row.valence_model,
            });
        }

        Ok(MyData {
            viewer_is_user: true,
            records: schemas.iter().map(|s| s.count).sum(),
            kinds: schemas.len() as u64,
            being_deleted: schemas.iter().map(|s| s.being_deleted).sum(),
            schemas,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!("SSR only")
    }
}

/// One JSON document of every record the signed-in member owns and can read,
/// grouped by schema.
#[uf_product_macros::server]
#[server(DownloadMyData)]
pub async fn download_my_data() -> Result<String, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let v = super::helpers::viewer_valence().await?;
        let Some(user_id) = session_user_id(&v) else {
            return Err(super::helpers::auth_error(
                "Sign in as a member to download your data",
            ));
        };
        let summary = OwnershipService::owner_data_summary(&user_id, OwnerKind::User.as_str(), &v)
            .await
            .map_err(|e| super::helpers::io_error(e.to_string()))?;

        let mut by_schema = serde_json::Map::new();
        let mut cut_short: Vec<String> = Vec::new();
        for row in summary.rows_by_schema {
            let count = row.active_rows + row.pending_deletion_rows;
            let scan = scan_owned(
                &v,
                &row.valence_model,
                &user_id,
                usize::try_from(count).unwrap_or(usize::MAX),
                valence::use_!(r#"In the **Valence ops console** **Your data** page, we **read every record a member owns** to give a member a copy of their records as one JSON download. Only the signed-in member gets their own records."#),
            )
            .await?;
            if scan.cut_short {
                cut_short.push(row.valence_model.clone());
            }
            by_schema.insert(row.valence_model, serde_json::Value::Array(scan.rows));
        }

        let mut doc = serde_json::json!({
            "user": user_id,
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "schemas": by_schema,
        });
        if !cut_short.is_empty() {
            doc["incomplete"] = serde_json::json!({
                "schemas": cut_short,
                "reason": format!(
                    "Stopped after reading {MAX_SCAN_ROWS} records in each of these schemas; some of your records there may be missing from this file."
                ),
            });
        }
        serde_json::to_string_pretty(&doc).map_err(|e| super::helpers::io_error(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!("SSR only")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_purpose_strips_bold_and_spacing() {
        assert_eq!(
            plain_purpose("We **load your todos**\n when  you open them."),
            "We load your todos when you open them."
        );
    }

    #[test]
    fn retention_caption_defaults_when_blank() {
        assert_eq!(retention_caption("  "), DEFAULT_RETENTION);
        assert_eq!(retention_caption("90 days"), "Retention: 90 days");
    }

    #[test]
    fn more_count_never_underflows() {
        assert_eq!(more_count(14, 3), 11);
        assert_eq!(more_count(2, 3), 0);
    }

    #[test]
    fn row_id_reads_string_and_record_shapes() {
        assert_eq!(
            row_id(&json!({"id": "todo:a1"})).as_deref(),
            Some("todo:a1")
        );
        assert_eq!(
            row_id(&json!({"id": {"tb": "todo", "id": "a1"}})).as_deref(),
            Some("a1")
        );
        assert_eq!(row_id(&json!({"id": ""})), None);
        assert_eq!(row_id(&json!({"name": "x"})), None);
    }

    #[test]
    fn record_label_prefers_title_then_id() {
        assert_eq!(
            record_label(&json!({"title": "Order badges", "name": "n"}), "a1"),
            "Order badges"
        );
        assert_eq!(record_label(&json!({"title": "  "}), "a1"), "a1");
    }

    #[test]
    fn record_created_at_parses_rfc3339_only() {
        assert_eq!(
            record_created_at(&json!({"created_at": "2026-04-28T10:00:00+02:00"})).as_deref(),
            Some("2026-04-28T08:00:00+00:00")
        );
        assert_eq!(record_created_at(&json!({"created_at": "yesterday"})), None);
    }

    #[test]
    fn ownership_names_user_matches_bare_and_prefixed_ids() {
        let row = json!({"owner_id": "user:admin", "owner_type": "user", "status": "active"});
        assert!(ownership_names_user(&row, "admin"));
        assert!(ownership_names_user(&row, "user:admin"));
        assert!(!ownership_names_user(&row, "outsider"));
    }

    #[test]
    fn ownership_names_user_rejects_deleted_and_non_user_rows() {
        let deleted = json!({"owner_id": "admin", "owner_type": "user", "status": "deleted"});
        assert!(!ownership_names_user(&deleted, "admin"));
        let service = json!({"owner_id": "admin", "owner_type": "service", "status": "active"});
        assert!(!ownership_names_user(&service, "admin"));
        let pending =
            json!({"owner_id": "admin", "owner_type": "user", "status": "pending_deletion"});
        assert!(ownership_names_user(&pending, "admin"));
    }
}
