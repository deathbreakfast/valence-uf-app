//! Contract for ValenceAdmin-gated mutating server functions (VU-08).
//!
//! Browse/read server fns stay ungated so entity pages load without admin;
//! every mutating admin op must appear here and use the shared permission name.

#![allow(missing_docs)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use valence_backend::{
    is_valence_admin_server_fn, VALENCE_ADMIN_PERMISSION, VALENCE_ADMIN_READ_SERVER_FNS,
    VALENCE_ADMIN_SERVER_FNS,
};

fn valence_app_server_rs() -> String {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = manifest.join("../valence-app/src/server");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("rs"))
        .collect();
    files.sort();
    let mut out = String::new();
    for path in files {
        out.push_str(
            &std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display())),
        );
        out.push('\n');
    }
    out
}

#[test]
fn valence_admin_permission_name_happy_path() {
    assert_eq!(VALENCE_ADMIN_PERMISSION, "ValenceAdmin");
}

#[test]
fn valence_admin_server_fns_lists_all_mutators_happy_path() {
    assert_eq!(VALENCE_ADMIN_SERVER_FNS.len(), 6);
    for name in [
        "start_iter_run",
        "run_iter_on_entity",
        "cancel_iter_run",
        "delete_entity_queue",
        "cancel_deletion_run",
        "save_storage_settings",
    ] {
        assert!(
            is_valence_admin_server_fn(name),
            "expected {name} in VALENCE_ADMIN_SERVER_FNS"
        );
    }
}

/// The `#[uf_product_macros::server(...)]` line nearest above `fn {name}(`.
fn server_macro_line(src: &str, name: &str) -> String {
    let needle = format!("fn {name}(");
    let idx = src
        .find(&needle)
        .unwrap_or_else(|| panic!("missing server fn {name}"));
    src[..idx]
        .lines()
        .rev()
        .find(|line| line.contains("#[uf_product_macros::server"))
        .unwrap_or_else(|| panic!("no server macro above {name}"))
        .to_string()
}

#[test]
fn server_rs_mutators_carry_valence_admin_macro_happy_path() {
    let src = valence_app_server_rs();
    for name in VALENCE_ADMIN_SERVER_FNS {
        let macro_line = server_macro_line(&src, name);
        assert!(
            macro_line.contains(r#"permission = "ValenceAdmin""#),
            "{name} must use ValenceAdmin macro; got: {macro_line}"
        );
    }
}

/// The macro's permission check is what answers a non-admin with a `permission` error, so
/// every site-wide storage fn must carry it.
#[test]
fn server_rs_site_storage_reads_carry_valence_admin_macro_happy_path() {
    assert_eq!(VALENCE_ADMIN_READ_SERVER_FNS, ["get_site_storage"]);
    let src = valence_app_server_rs();
    for name in VALENCE_ADMIN_READ_SERVER_FNS {
        let macro_line = server_macro_line(&src, name);
        assert!(
            macro_line.contains(r#"permission = "ValenceAdmin""#),
            "{name} must use ValenceAdmin macro so non-admins get a permission error; got: {macro_line}"
        );
    }
}

#[test]
fn server_rs_entity_view_stays_ungated_happy_path() {
    let src = valence_app_server_rs();
    let macro_line = server_macro_line(&src, "get_entity_view");
    assert!(
        !macro_line.contains("permission"),
        "entity view must stay loadable without ValenceAdmin: {macro_line}"
    );
}

#[test]
fn server_rs_member_storage_reads_stay_ungated_happy_path() {
    let src = valence_app_server_rs();
    for name in ["get_my_storage", "get_storage_settings"] {
        let macro_line = server_macro_line(&src, name);
        assert!(
            !macro_line.contains("permission"),
            "{name} must stay open to signed-in members: {macro_line}"
        );
        assert!(!is_valence_admin_server_fn(name));
    }
}

#[test]
fn is_valence_admin_server_fn_rejects_browse_reads_sad() {
    for name in [
        "get_entity_view",
        "get_schema",
        "get_schemas",
        "search_schema_or_id",
        "evaluate_iter_for_entity",
        "get_iter_run",
        "list_deletion_runs",
    ] {
        assert!(
            !is_valence_admin_server_fn(name),
            "{name} must stay browse/read without ValenceAdmin"
        );
    }
}

#[test]
fn is_valence_admin_server_fn_rejects_blank_sad() {
    assert!(!is_valence_admin_server_fn(""));
    assert!(!is_valence_admin_server_fn("   "));
}
