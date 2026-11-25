use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;
use orbital::components::{
    Button, ButtonAppearance, Icon, Input, InputAppearance, InputPrefix, MessageBar,
    MessageBarActions, MessageBarBody, MessageBarIntent, MessageBarLayout, MessageBarTitle, Title3,
};
use turf::inline_style_sheet_values;

use crate::server::{SearchOutcome, SearchSchemaOrId};

/// `true` when the search failed for lack of a session: the server writes those
/// with the stable `auth:` kind prefix.
fn is_auth_error(err: &ServerFnError) -> bool {
    matches!(err, ServerFnError::ServerError(m) if m.trim_start().starts_with("auth:"))
}

/// What to tell the operator when the search itself failed.
fn search_error_message(err: &ServerFnError) -> &'static str {
    if is_auth_error(err) {
        "Your session has ended. Sign in again, then search."
    } else {
        "Valence didn't answer the search. Try again in a moment."
    }
}

#[component]
pub fn DashboardSearchHeader() -> impl IntoView {
    let query = RwSignal::new(String::new());
    let searched = RwSignal::new(String::new());
    let search_action = ServerAction::<SearchSchemaOrId>::new();
    let navigate = use_navigate();

    Effect::new(move |_| {
        if let Some(Ok(SearchOutcome::Redirect(path) | SearchOutcome::Hidden(path))) =
            search_action.value().get()
        {
            navigate(&path, Default::default());
        }
    });

    let (style_sheet, class_names) = inline_style_sheet_values! {
        .Header {
            display: flex;
            flex-direction: column;
            gap: 12px;
        }

        .SearchRow {
            width: min(560px, 100%);
        }
    };

    view! {
        <style>{style_sheet}</style>
        <div id="valence-dashboard-search" class=class_names.header data-testid="valence-dashboard-search-header">
            <Title3>"Valence"</Title3>
            <form
                on:submit=move |ev| {
                    ev.prevent_default();
                    let query_val = query.get();
                    if !query_val.trim().is_empty() {
                        searched.set(query_val.trim().to_string());
                        search_action.dispatch(SearchSchemaOrId { query: query_val });
                    }
                }
            >
                <div data-testid="valence-dashboard-search" class=class_names.search_row>
                    <Input
                        bind=query
                        appearance=InputAppearance::with_placeholder("Search for schema or ID.")
                    >
                        <InputPrefix slot>
                            <Icon icon=icondata::AiSearchOutlined />
                        </InputPrefix>
                    </Input>
                </div>
            </form>
            {move || match search_action.value().get() {
                Some(Ok(SearchOutcome::NoMatch)) => view! {
                    <div data-testid="dashboard-search-no-match">
                        <MessageBar intent=MessageBarIntent::Warning layout=MessageBarLayout::Multiline>
                            <MessageBarTitle>{format!("Nothing matches {}", searched.get_untracked())}</MessageBarTitle>
                            <MessageBarBody>
                                "No schema has that name and no record has that id. Check the id, or search Deletions if it may have been removed."
                            </MessageBarBody>
                            <MessageBarActions>
                                <A href=crate::paths::DELETIONS>
                                    <Button appearance=ButtonAppearance::Secondary>"Search Deletions"</Button>
                                </A>
                            </MessageBarActions>
                        </MessageBar>
                    </div>
                }.into_any(),
                Some(Err(e)) => view! {
                    <div data-testid="dashboard-search-error">
                        <MessageBar intent=MessageBarIntent::Error layout=MessageBarLayout::Multiline>
                            <MessageBarTitle>"Search didn't run"</MessageBarTitle>
                            <MessageBarBody>{search_error_message(&e)}</MessageBarBody>
                        </MessageBar>
                    </div>
                }.into_any(),
                _ => ().into_any(),
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::is_auth_error;
    use leptos::prelude::ServerFnError;

    #[test]
    fn auth_prefix_only_counts_at_the_start() {
        assert!(is_auth_error(&ServerFnError::new(
            "auth: Authentication required"
        )));
        assert!(!is_auth_error(&ServerFnError::new(
            "io: upstream said auth: token rejected"
        )));
    }
}
