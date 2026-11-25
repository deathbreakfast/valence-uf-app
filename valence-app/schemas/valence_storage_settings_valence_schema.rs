// ValenceStorageSettings — the site's storage cost settings (one row, id "site").
//
// Codegen input only (valence-app/build.rs); the generated model is included from
// `src/server/storage_settings.rs`. Including this file as a module as well would register
// the schema twice.

use ::valence::prelude::*;

valence_schema! {
    ValenceStorageSettings {
        repository: "https://github.com/unified-field-dev/valence-uf-app",
        table: "valence_storage_settings",
        version: "0.1.0",
        database: valence_platform::DEFAULT_PLATFORM_STORAGE,
        description: "Storage cost settings for the Valence database: copies kept and price per GB each month",

        // A site setting, not personal data: it never counts toward an owner's records.
        ownership: { system: true },

        // Signed-in members read the settings (Your data shows cost with them). Only
        // ValenceAdmin holders create or update the row, on every write path; System may also
        // update it so the deletion runner can clear `updated_by`.
        policies: {
            read: {
                always_allow: [],
                allow: [
                    crate::server::storage_settings::SIGNED_IN_MEMBER,
                    ::valence::privacy_policies::common::SYSTEM_ONLY,
                ],
                block: [],
                always_block: [],
            },
            create: {
                always_allow: [],
                allow: [crate::server::storage_settings::VALENCE_ADMIN_GATE],
                block: [],
                always_block: [],
            },
            update: {
                always_allow: [],
                allow: [
                    crate::server::storage_settings::VALENCE_ADMIN_GATE,
                    ::valence::privacy_policies::common::SYSTEM_ONLY,
                ],
                block: [],
                always_block: [],
            },
            delete: {
                always_allow: [],
                allow: [::valence::privacy_policies::common::SYSTEM_ONLY],
                block: [],
                always_block: [],
            },
        },

        fields: [
            id: {
                r#type: FieldType::String,
                primary_key: true,
                required: true,
            },
            copies_override: {
                r#type: FieldType::Integer,
                required: false,
            },
            price_per_gb_month: {
                r#type: FieldType::Decimal,
                required: false,
            },
            currency: {
                r#type: FieldType::String,
                required: true,
            },
            updated_by: {
                r#type: FieldType::Record("user"),
                required: false,
            },
            updated_at: {
                r#type: FieldType::DateTime,
                required: true,
            },
        ],

        connections: [
            // Deleting the user who last saved clears the field; the settings stay.
            updated_by: {
                table: "user",
                cardinality: HasOne,
                required: false,
                on_delete: SetNull,
                model: "lepton::generated::User",
            },
        ],
    }
}
