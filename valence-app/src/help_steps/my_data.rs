//! Spotlight steps for Your data (`/valence/me`).

use leptos::prelude::*;
use uf_help_macros::help_spotlight_step;

use super::help_stack;

/// Centered intro: what this page covers.
#[help_spotlight_step(
    route = "/valence/me",
    feature_highlight = "valence-my-data-intro",
    title = "Your data",
    order = 10
)]
#[component]
pub fn ValenceMyDataIntroHelp() -> impl IntoView {
    help_stack(
        "help-step-valence-my-data-intro",
        "This page lists only records you own and can read yourself. Nobody else sees it.",
        None,
        &[],
    )
}

/// Download everything button.
#[help_spotlight_step(
    route = "/valence/me",
    feature_highlight = "valence-my-data-download",
    title = "Take a copy",
    spotlight = "valence-my-data-download",
    position = "bottom",
    order = 20
)]
#[component]
pub fn ValenceMyDataDownloadHelp() -> impl IntoView {
    help_stack(
        "help-step-valence-my-data-download",
        "Downloads one JSON file with every record on this page, grouped by kind.",
        None,
        &[],
    )
}

/// Account deletion note.
#[help_spotlight_step(
    route = "/valence/me",
    feature_highlight = "valence-my-data-account",
    title = "Closing your account",
    spotlight = "valence-my-data-account",
    position = "bottom",
    order = 30
)]
#[component]
pub fn ValenceMyDataAccountHelp() -> impl IntoView {
    help_stack(
        "help-step-valence-my-data-account",
        "Closing your account in Account Settings removes everything here within 30 days.",
        Some("Records already queued show a being-deleted badge on their card."),
        &[],
    )
}

/// Per-schema cards.
#[help_spotlight_step(
    route = "/valence/me",
    feature_highlight = "valence-my-data-schemas",
    title = "Each kind of record",
    spotlight = "valence-my-data-schemas",
    position = "top",
    order = 40
)]
#[component]
pub fn ValenceMyDataSchemasHelp() -> impl IntoView {
    help_stack(
        "help-step-valence-my-data-schemas",
        "Why it's kept quotes the reasons the site declared for reading these records.",
        Some("Open a record to see its fields, or See all for the whole schema."),
        &[],
    )
}
