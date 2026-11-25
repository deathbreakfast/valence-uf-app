use leptos::prelude::*;
use leptos_router::components::A;
use orbital::components::{
    Body1, Button, ButtonAppearance, Card, CardContent, EmptyState, Flex, FlexAlign, FlexGap,
    FlexWrap, MessageBar, MessageBarIntent,
};

use crate::components::help::ValenceHelpCardHeader;
use crate::server::get_dashboard_my_data_stats;

use super::skeletons::DashboardKpiSkeleton;

/// "You own N records in K kinds. M are being deleted."
fn my_data_sentence(records: u64, kinds: u64, being_deleted: u64) -> String {
    let records_word = if records == 1 { "record" } else { "records" };
    let kinds_word = if kinds == 1 { "kind" } else { "kinds" };
    let mut sentence = format!("You own {records} {records_word} in {kinds} {kinds_word}.");
    if being_deleted > 0 {
        let verb = if being_deleted == 1 { "is" } else { "are" };
        sentence.push_str(&format!(" {being_deleted} {verb} being deleted."));
    }
    sentence
}

#[component]
pub fn DashboardMyDataCard() -> impl IntoView {
    let res = Resource::new(
        || (),
        |_| async move { get_dashboard_my_data_stats().await },
    );

    view! {
        <Card>
            <ValenceHelpCardHeader
                title="Your data"
                description="What Valence holds that you own."
            />
            <CardContent>
                <div id="valence-dashboard-my-data" data-testid="dashboard-my-data-card">
                    <Transition fallback=move || view! { <DashboardKpiSkeleton /> }>
                        {move || res.get().map(|r| match r {
                            Ok(stats) if !stats.viewer_is_user => view! {
                                <EmptyState message="Sign in as a user to see your data." />
                            }.into_any(),
                            Ok(stats) => {
                                let sentence = my_data_sentence(
                                    stats.owned_rows + stats.pending_deletion_rows,
                                    stats.tables_with_data,
                                    stats.pending_deletion_rows,
                                );
                                view! {
                                    <Flex gap=FlexGap::Medium wrap=FlexWrap::Wrap align=FlexAlign::Center>
                                        <span data-testid="dashboard-my-data-summary">
                                            <Body1>{sentence}</Body1>
                                        </span>
                                        <A href=crate::paths::ME>
                                            <Button appearance=ButtonAppearance::Secondary>"Open your data"</Button>
                                        </A>
                                    </Flex>
                                }.into_any()
                            }
                            Err(e) => view! {
                                <MessageBar intent=MessageBarIntent::Error>{e.to_string()}</MessageBar>
                            }.into_any(),
                        })}
                    </Transition>
                </div>
            </CardContent>
        </Card>
    }
}

#[cfg(test)]
mod tests {
    use super::my_data_sentence;

    #[test]
    fn sentence_matches_counts() {
        assert_eq!(
            my_data_sentence(30, 4, 2),
            "You own 30 records in 4 kinds. 2 are being deleted."
        );
        assert_eq!(my_data_sentence(1, 1, 0), "You own 1 record in 1 kind.");
        assert_eq!(
            my_data_sentence(3, 2, 1),
            "You own 3 records in 2 kinds. 1 is being deleted."
        );
    }
}
