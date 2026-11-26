use leptos::prelude::*;
use orbital::components::{
    MessageBar, MessageBarBody, MessageBarIntent, MessageBarLayout, MessageBarTitle,
};

/// Shown in place of platform metrics when Spectra denies the query.
#[component]
pub fn DashboardPermissionDeniedState() -> impl IntoView {
    view! {
        <div data-testid="dashboard-permission-denied">
            <MessageBar intent=MessageBarIntent::Info layout=MessageBarLayout::Multiline>
                <MessageBarTitle>"Platform metrics need a Spectra permission"</MessageBarTitle>
                <MessageBarBody>
                    "Reading these charts needs spectra.query on the platform tables. Everything else on this page works without it."
                </MessageBarBody>
            </MessageBar>
        </div>
    }
}
