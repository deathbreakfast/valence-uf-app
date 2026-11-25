//! Reads the newest storage run from Spectra.
//!
//! Each reader finds the newest run (sort by `ds` descending, limit 1; the newest event
//! timestamp breaks a tie between two runs on one day), then reads every row of that run
//! [`READ_PAGE_SIZE`] rows at a time. No table or no rows gives `Ok(None)` (or an empty list):
//! Spectra answers an unregistered table with no rows, not an error. A Spectra error gives
//! `Err`.

use std::collections::BTreeMap;

use chrono::{Duration, Utc};
use serde_json::Value;
use spectra_core::{
    EventRow, EventsQueryFilter, GridFilterItem, GridFilterModel, GridFilterOperator, SpectraRouter,
};

use super::rows::{
    str_field, HistoryRow, OwnerKey, OwnerSnapshot, OwnerSnapshotRow, SchemaSnapshot,
    SchemaSnapshotRow, StorageReadError, ALL_SCHEMAS,
};
use super::{VALENCE_STORAGE_OWNER_DAILY, VALENCE_STORAGE_SCHEMA_DAILY};

/// Rows per Spectra query. The mem and SQLite backends cap a query at 1,000 rows.
pub const READ_PAGE_SIZE: u32 = 1_000;

/// Highest offset Spectra accepts (`MAX_EVENT_QUERY_OFFSET`); paging stops there.
const MAX_OFFSET: u32 = 1_000_000;

/// `ds` and `run_id` of the newest run in a table.
struct RunKey {
    ds: String,
    run_id: String,
}

/// Every schema row of the newest run, or `None` when the table has no rows.
///
/// ```rust,ignore
/// let snapshot = valence_backend::latest_schema_snapshot(&router).await?;
/// ```
pub async fn latest_schema_snapshot(
    router: &SpectraRouter,
) -> Result<Option<SchemaSnapshot>, StorageReadError> {
    let Some(run) = latest_run(router, VALENCE_STORAGE_SCHEMA_DAILY).await? else {
        return Ok(None);
    };
    let rows = read_all(
        router,
        VALENCE_STORAGE_SCHEMA_DAILY,
        run_filter(&run, Vec::new()),
    )
    .await?;
    let mut parsed: Vec<SchemaSnapshotRow> = rows
        .iter()
        .filter_map(|r| SchemaSnapshotRow::from_fields(&r.fields, r.ts))
        .collect();
    parsed.sort_by(|a, b| a.schema.cmp(&b.schema));
    let measured_at = parsed.first().map_or_else(
        || rows.first().map_or_else(Utc::now, |r| r.ts),
        |r| r.measured_at,
    );
    let (totals, schemas): (Vec<_>, Vec<_>) =
        parsed.into_iter().partition(|r| r.schema == ALL_SCHEMAS);
    Ok(Some(SchemaSnapshot {
        ds: run.ds,
        run_id: run.run_id,
        measured_at,
        total: totals.into_iter().next(),
        schemas,
    }))
}

/// One owner's rows from the newest owner run, or `None` when the table has no rows.
///
/// The newest run is found across all owners, so an owner with nothing measured that day
/// gets `Some` with no rows rather than `None`.
pub async fn latest_owner_snapshot(
    router: &SpectraRouter,
    owner: &OwnerKey,
) -> Result<Option<OwnerSnapshot>, StorageReadError> {
    let Some(run) = latest_run(router, VALENCE_STORAGE_OWNER_DAILY).await? else {
        return Ok(None);
    };
    let filter = run_filter(
        &run,
        vec![
            equals("owner_kind", &owner.kind),
            equals("owner_id", &owner.id),
        ],
    );
    let rows = read_all(router, VALENCE_STORAGE_OWNER_DAILY, filter).await?;
    let mut parsed: Vec<OwnerSnapshotRow> = rows
        .iter()
        .filter_map(|r| OwnerSnapshotRow::from_fields(&r.fields, r.ts))
        .collect();
    parsed.sort_by(|a, b| a.schema.cmp(&b.schema));
    let measured_at = match parsed.first() {
        Some(row) => row.measured_at,
        None => run_measured_at(router, &run).await?,
    };
    let (totals, schemas): (Vec<_>, Vec<_>) =
        parsed.into_iter().partition(|r| r.schema == ALL_SCHEMAS);
    Ok(Some(OwnerSnapshot {
        ds: run.ds,
        run_id: run.run_id,
        measured_at,
        total: totals.into_iter().next(),
        schemas,
    }))
}

/// The `limit` owners with the most estimated bytes in the newest owner run.
///
/// Reads every owner's `*` row (paging past 1,000 owners) and sorts by `est_bytes` here,
/// since Spectra sorts JSON fields as strings.
pub async fn top_owners(
    router: &SpectraRouter,
    limit: usize,
) -> Result<Vec<OwnerSnapshotRow>, StorageReadError> {
    let Some(run) = latest_run(router, VALENCE_STORAGE_OWNER_DAILY).await? else {
        return Ok(Vec::new());
    };
    let filter = run_filter(&run, vec![equals("schema", ALL_SCHEMAS)]);
    let rows = read_all(router, VALENCE_STORAGE_OWNER_DAILY, filter).await?;
    let mut owners: Vec<OwnerSnapshotRow> = rows
        .iter()
        .filter_map(|r| OwnerSnapshotRow::from_fields(&r.fields, r.ts))
        .collect();
    owners.sort_by(|a, b| {
        b.est_bytes
            .cmp(&a.est_bytes)
            .then_with(|| a.owner_id.cmp(&b.owner_id))
    });
    owners.truncate(limit);
    Ok(owners)
}

/// The database `*` total of each of the last `days` days, oldest first.
///
/// When a day has several runs, the newest one counts.
pub async fn history(
    router: &SpectraRouter,
    days: u32,
) -> Result<Vec<HistoryRow>, StorageReadError> {
    let filter = EventsQueryFilter {
        table: VALENCE_STORAGE_SCHEMA_DAILY.to_string(),
        start: Some(Utc::now() - Duration::days(i64::from(days))),
        filter: grid(vec![equals("schema", ALL_SCHEMAS)]),
        ..EventsQueryFilter::default()
    };
    let rows = read_all(router, VALENCE_STORAGE_SCHEMA_DAILY, filter).await?;
    let mut by_day: BTreeMap<String, SchemaSnapshotRow> = BTreeMap::new();
    for row in rows
        .iter()
        .filter_map(|r| SchemaSnapshotRow::from_fields(&r.fields, r.ts))
    {
        match by_day.get(&row.ds) {
            Some(kept) if kept.measured_at >= row.measured_at => {}
            _ => {
                by_day.insert(row.ds.clone(), row);
            }
        }
    }
    Ok(by_day
        .into_values()
        .map(|r| HistoryRow {
            bytes: r.table_bytes.map(|t| t + r.index_bytes.unwrap_or(0)),
            ds: r.ds,
            measured_at: r.measured_at,
            rows: r.rows,
        })
        .collect())
}

async fn latest_run(
    router: &SpectraRouter,
    table: &str,
) -> Result<Option<RunKey>, StorageReadError> {
    let rows = query(
        router,
        EventsQueryFilter {
            table: table.to_string(),
            sort_field: Some("ds".to_string()),
            sort_desc: true,
            limit: Some(1),
            ..EventsQueryFilter::default()
        },
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    match (
        str_field(&row.fields, "ds"),
        str_field(&row.fields, "run_id"),
    ) {
        (Some(ds), Some(run_id)) => Ok(Some(RunKey { ds, run_id })),
        _ => Err(StorageReadError::new(format!(
            "newest row in `{table}` has no `ds` or `run_id`"
        ))),
    }
}

/// Start time of a run, from any one of its rows.
async fn run_measured_at(
    router: &SpectraRouter,
    run: &RunKey,
) -> Result<chrono::DateTime<Utc>, StorageReadError> {
    let mut filter = run_filter(run, Vec::new());
    filter.table = VALENCE_STORAGE_OWNER_DAILY.to_string();
    filter.limit = Some(1);
    let rows = query(router, filter).await?;
    Ok(rows.first().map_or_else(Utc::now, |r| {
        OwnerSnapshotRow::from_fields(&r.fields, r.ts).map_or(r.ts, |o| o.measured_at)
    }))
}

/// Reads every row matching `filter`, one page at a time.
async fn read_all(
    router: &SpectraRouter,
    table: &str,
    mut filter: EventsQueryFilter,
) -> Result<Vec<EventRow>, StorageReadError> {
    filter.table = table.to_string();
    filter.limit = Some(READ_PAGE_SIZE);
    let mut all = Vec::new();
    let mut offset = 0_u32;
    loop {
        filter.offset = Some(offset);
        let page = query(router, filter.clone()).await?;
        let len = u32::try_from(page.len()).unwrap_or(u32::MAX);
        all.extend(page);
        if len < READ_PAGE_SIZE || offset >= MAX_OFFSET - READ_PAGE_SIZE {
            return Ok(all);
        }
        offset += READ_PAGE_SIZE;
    }
}

async fn query(
    router: &SpectraRouter,
    filter: EventsQueryFilter,
) -> Result<Vec<EventRow>, StorageReadError> {
    let table = filter.table.clone();
    router
        .query_events(filter)
        .await
        .map_err(|e| StorageReadError::new(format!("query `{table}`: {e}")))
}

fn run_filter(run: &RunKey, mut extra: Vec<GridFilterItem>) -> EventsQueryFilter {
    let mut items = vec![equals("ds", &run.ds), equals("run_id", &run.run_id)];
    items.append(&mut extra);
    EventsQueryFilter {
        filter: grid(items),
        ..EventsQueryFilter::default()
    }
}

fn grid(items: Vec<GridFilterItem>) -> GridFilterModel {
    GridFilterModel {
        items,
        ..GridFilterModel::default()
    }
}

fn equals(field: &str, value: &str) -> GridFilterItem {
    GridFilterItem {
        field: field.to_string(),
        operator: GridFilterOperator::Equals,
        value: Value::String(value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::sync::Arc;

    use chrono::{DateTime, Duration, Utc};
    use serde_json::json;
    use spectra::{MemEventsBackend, MemMetricsBackend};
    use spectra_core::EventStorageBackend;

    use super::*;

    fn mem_router() -> (SpectraRouter, Arc<MemEventsBackend>) {
        let events = Arc::new(MemEventsBackend::new());
        let router =
            SpectraRouter::with_defaults(Arc::new(MemMetricsBackend::new()), events.clone());
        (router, events)
    }

    async fn schema_row(
        events: &MemEventsBackend,
        ds: &str,
        run_id: &str,
        at: DateTime<Utc>,
        schema: &str,
        rows: i64,
        table_bytes: i64,
    ) {
        events
            .append_row(
                VALENCE_STORAGE_SCHEMA_DAILY,
                &json!({
                    "ds": ds, "measured_at": at.to_rfc3339(), "run_id": run_id,
                    "schema": schema, "backend": "postgres", "rows": rows,
                    "table_bytes": table_bytes, "index_bytes": 10, "how": "measured",
                    "avg_row_bytes": 100, "copies_reported": if schema == "*" { 2 } else { -1 },
                }),
                at,
                None,
            )
            .await
            .unwrap();
    }

    async fn owner_row(
        events: &MemEventsBackend,
        ds: &str,
        run_id: &str,
        at: DateTime<Utc>,
        owner_id: &str,
        schema: &str,
        est_bytes: i64,
    ) {
        events
            .append_row(
                VALENCE_STORAGE_OWNER_DAILY,
                &json!({
                    "ds": ds, "measured_at": at.to_rfc3339(), "run_id": run_id,
                    "owner_kind": "user", "owner_id": owner_id, "schema": schema,
                    "rows": 1, "est_bytes": est_bytes,
                }),
                at,
                None,
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn missing_tables_read_as_none() {
        let (router, _) = mem_router();
        assert_eq!(latest_schema_snapshot(&router).await.unwrap(), None);
        assert_eq!(
            latest_owner_snapshot(&router, &OwnerKey::user("u1"))
                .await
                .unwrap(),
            None
        );
        assert_eq!(top_owners(&router, 5).await.unwrap(), Vec::new());
        assert_eq!(history(&router, 30).await.unwrap(), Vec::new());

        // A router with no backends at all (Spectra's no-op default) reads the same way.
        assert_eq!(
            latest_schema_snapshot(&SpectraRouter::new()).await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn two_partitions_return_only_the_latest() {
        let (router, events) = mem_router();
        let day1 = Utc::now() - Duration::days(1);
        let day2 = Utc::now();
        schema_row(&events, "2026-11-23", "run-a", day1, "*", 30, 3000).await;
        schema_row(&events, "2026-11-23", "run-a", day1, "todo", 30, 3000).await;
        schema_row(&events, "2026-11-24", "run-b", day2, "*", 50, 5000).await;
        schema_row(&events, "2026-11-24", "run-b", day2, "todo", 40, 4000).await;
        schema_row(&events, "2026-11-24", "run-b", day2, "tag", 10, 1000).await;

        let snap = latest_schema_snapshot(&router)
            .await
            .unwrap()
            .expect("snapshot");
        assert_eq!(snap.ds, "2026-11-24");
        assert_eq!(snap.run_id, "run-b");
        assert_eq!(
            snap.schemas
                .iter()
                .map(|r| r.schema.as_str())
                .collect::<Vec<_>>(),
            ["tag", "todo"]
        );
        assert_eq!(snap.total.expect("total").copies_reported, Some(2));

        let points = history(&router, 30).await.unwrap();
        assert_eq!(
            points.iter().map(|p| p.ds.as_str()).collect::<Vec<_>>(),
            ["2026-11-23", "2026-11-24"]
        );
        assert_eq!(points[1].bytes, Some(5010));
    }

    #[tokio::test]
    async fn a_rerun_on_the_same_day_replaces_the_earlier_run() {
        let (router, events) = mem_router();
        let first = Utc::now() - Duration::hours(2);
        let rerun = Utc::now();
        schema_row(&events, "2026-11-24", "run-1", first, "todo", 1, 100).await;
        schema_row(&events, "2026-11-24", "run-2", rerun, "todo", 2, 200).await;

        let snap = latest_schema_snapshot(&router)
            .await
            .unwrap()
            .expect("snapshot");
        assert_eq!(snap.run_id, "run-2");
        assert_eq!(snap.schemas.len(), 1);
        assert_eq!(snap.schemas[0].rows, 2);
    }

    #[tokio::test]
    async fn owner_rows_page_past_one_thousand() {
        let (router, events) = mem_router();
        let at = Utc::now();
        for i in 0..2_500 {
            owner_row(
                &events,
                "2026-11-24",
                "run-1",
                at,
                &format!("u{i:04}"),
                "*",
                i,
            )
            .await;
        }
        owner_row(&events, "2026-11-24", "run-1", at, "u0007", "todo", 7).await;

        let top = top_owners(&router, 3).await.unwrap();
        assert_eq!(
            top.iter().map(|o| o.owner_id.as_str()).collect::<Vec<_>>(),
            ["u2499", "u2498", "u2497"]
        );
        let all = top_owners(&router, usize::MAX).await.unwrap();
        assert_eq!(all.len(), 2_500);

        let mine = latest_owner_snapshot(&router, &OwnerKey::user("u0007"))
            .await
            .unwrap()
            .expect("snapshot");
        assert_eq!(mine.total.expect("total").est_bytes, 7);
        assert_eq!(mine.schemas.len(), 1);
    }

    #[tokio::test]
    async fn owner_missing_from_the_latest_run_gets_an_empty_snapshot() {
        let (router, events) = mem_router();
        let at = Utc::now();
        owner_row(&events, "2026-11-24", "run-1", at, "u1", "*", 10).await;

        let other = latest_owner_snapshot(&router, &OwnerKey::user("u2"))
            .await
            .unwrap()
            .expect("snapshot");
        assert!(other.total.is_none());
        assert_eq!(other.schemas, Vec::new());
        assert_eq!(other.run_id, "run-1");
    }

    #[tokio::test]
    async fn invalid_query_is_an_error_not_none() {
        // A table name Spectra rejects stands in for a backend failure.
        let (router, _) = mem_router();
        let err = latest_run(&router, "bad table").await.err().expect("error");
        assert!(err.message.contains("bad table"), "{err}");
    }
}
