//! Owner-readable schemas so the Your data E2E suite has records a member can read.
//!
//! Valence denies reads on schemas with no policies, so these declare an owner read
//! rule keyed on a bare `user` id (fixture-only; real user-owned data uses a `user`
//! connection with `on_delete`).

#![allow(dead_code)]

use valence::{valence_schema, Database, DatabaseFromEngine, MEM_ENGINE_ID};

const MY_DATA_DB: DatabaseFromEngine = Database::from_engine("default", MEM_ENGINE_ID);

valence_schema! {
    E2eNote {
        repository: "https://github.com/unified-field-dev/valence-uf-app",
        table: "e2e_note",
        version: "0.1.0",
        description: "E2E notes a member owns, for Your data",
        database: MY_DATA_DB,
        policies: {
            read: { allow: [valence::privacy_policies::owner::OWNER_BY_USER_FIELD] },
        },
        fields: [
            id: { r#type: FieldType::String, primary_key: true, required: true },
            user: { r#type: FieldType::String, required: true },
            title: { r#type: FieldType::String, required: false },
        ],
    }
}

valence_schema! {
    E2eBookmark {
        repository: "https://github.com/unified-field-dev/valence-uf-app",
        table: "e2e_bookmark",
        version: "0.1.0",
        description: "E2E bookmarks a member owns, for Your data",
        database: MY_DATA_DB,
        policies: {
            read: { allow: [valence::privacy_policies::owner::OWNER_BY_USER_FIELD] },
        },
        fields: [
            id: { r#type: FieldType::String, primary_key: true, required: true },
            user: { r#type: FieldType::String, required: true },
            title: { r#type: FieldType::String, required: false },
        ],
    }
}
