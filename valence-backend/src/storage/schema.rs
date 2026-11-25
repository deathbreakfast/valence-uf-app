//! Spectra event schemas for the daily storage snapshot tables.
//!
//! The macro generates a `*Logger` with `log` / `log_at` for each table. Hosts write rows
//! with `log_at(.., measured_at)` so the event timestamp matches the row's `measured_at`.
//! The row contract is in the [module docs](super).

// Fields and generated helpers are documented by the DSL below; the generated `log` helpers
// take one argument per column.
#![allow(missing_docs, clippy::too_many_arguments)]

use spectra::spectra_schema;

spectra_schema! {
    ValenceStorageSchemaDaily {
        store: "valence",
        table: "valence_storage_schema_daily",
        version: "0.1.0",
        description: "Daily Valence storage snapshot per schema, with a total row",
        fields: [
            ds: { r#type: String, classification: { pii: false, safe_for_console: true } },
            measured_at: { r#type: String, classification: { pii: false, safe_for_console: true } },
            run_id: { r#type: String, classification: { pii: false, safe_for_console: true } },
            schema: { r#type: String, classification: { pii: false, safe_for_console: true } },
            backend: { r#type: String, classification: { pii: false, safe_for_console: true } },
            rows: { r#type: i64, classification: { pii: false, safe_for_console: true } },
            table_bytes: { r#type: i64, classification: { pii: false, safe_for_console: true } },
            index_bytes: { r#type: i64, classification: { pii: false, safe_for_console: true } },
            how: { r#type: String, classification: { pii: false, safe_for_console: true } },
            avg_row_bytes: { r#type: i64, classification: { pii: false, safe_for_console: true } },
            copies_reported: { r#type: i64, classification: { pii: false, safe_for_console: true } },
        ],
    }
}

spectra_schema! {
    ValenceStorageOwnerDaily {
        store: "valence",
        table: "valence_storage_owner_daily",
        version: "0.1.0",
        description: "Daily Valence storage per owner and schema, estimated from rows times average row size",
        fields: [
            ds: { r#type: String, classification: { pii: false, safe_for_console: true } },
            measured_at: { r#type: String, classification: { pii: false, safe_for_console: true } },
            run_id: { r#type: String, classification: { pii: false, safe_for_console: true } },
            owner_kind: { r#type: String, classification: { pii: false, safe_for_console: true } },
            owner_id: { r#type: String, classification: { pii: true, safe_for_console: false } },
            schema: { r#type: String, classification: { pii: false, safe_for_console: true } },
            rows: { r#type: i64, classification: { pii: false, safe_for_console: true } },
            est_bytes: { r#type: i64, classification: { pii: false, safe_for_console: true } },
        ],
    }
}
