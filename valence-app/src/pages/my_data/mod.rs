//! `/valence/me`: what Valence holds about the signed-in member, by schema.

use leptos::prelude::*;
use leptos_router::components::A;
use orbital::components::{
    AutoGrid, Badge, BadgeAppearance, BadgeColor, Body1, Button, ButtonAppearance, Caption1, Card,
    CardContent, CardHeader, CardHeaderAction, ContentContainer, EmptyState, Flex, FlexAlign,
    FlexGap, FlexJustify, FlexWrap, List, ListItem, MessageBar, MessageBarActions, MessageBarBody,
    MessageBarIntent, MessageBarLayout, MessageBarTitle, Skeleton, SkeletonItem, SpacingSize,
    StatCard, StatCardVariant, Subtitle1, Table, TableBody, TableCell, TableHeader,
    TableHeaderCell, TableRow, Title3,
};
use orbital::primitives::{use_breakpoint_down, Breakpoint};

use crate::components::download::download_text_file;
use crate::server::{get_my_data, DownloadMyData, MyData, MyDataRecord, MyDataSchema};

/// Lepton's account settings page, where a member closes their account.
const ACCOUNT_SETTINGS_PATH: &str = "/user/account-settings";

/// File name for Download everything.
const DOWNLOAD_FILE_NAME: &str = "valence-your-data.json";

/// Created time as a short UTC date ("Apr 28, 2026"), or a dash when unknown.
fn created_label(created_at: Option<&str>) -> String {
    created_at
        .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
        .map(|t| {
            t.with_timezone(&chrono::Utc)
                .format("%b %-d, %Y")
                .to_string()
        })
        .unwrap_or_else(|| "—".to_string())
}

#[component]
pub fn ValenceMyDataPage() -> impl IntoView {
    let res = Resource::new(|| (), |_| async move { get_my_data().await });
    let download = ServerAction::<DownloadMyData>::new();
    let downloading = download.pending();

    Effect::new(move |_| {
        if let Some(Ok(doc)) = download.value().get() {
            download_text_file(DOWNLOAD_FILE_NAME, &doc, "application/json");
        }
    });

    view! {
        <ContentContainer max_width="1200px" data_testid="valence-my-data-page">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                <Flex justify=FlexJustify::SpaceBetween align=FlexAlign::Center wrap=FlexWrap::Wrap gap=FlexGap::Medium>
                    <Flex vertical=true gap=FlexGap::Small>
                        <Title3>"Your data"</Title3>
                        <Body1>"Everything Valence stores that you own, and why the site keeps it."</Body1>
                    </Flex>
                    <span id="valence-my-data-download" data-testid="my-data-download">
                        <Button
                            appearance=ButtonAppearance::Primary
                            loading=downloading
                            disabled=downloading
                            on_click=Callback::new(move |_| {
                                download.dispatch(DownloadMyData {});
                            })
                        >
                            "Download everything"
                        </Button>
                    </span>
                </Flex>

                {move || download.value().get().and_then(Result::err).map(|_| view! {
                    <div data-testid="my-data-download-error">
                        <MessageBar intent=MessageBarIntent::Error>
                            <MessageBarBody>"The download didn't finish. Nothing changed in your data; try again."</MessageBarBody>
                        </MessageBar>
                    </div>
                })}

                <Transition fallback=move || view! { <MyDataSkeleton /> }>
                    {move || res.get().map(|r| match r {
                        Ok(data) if !data.viewer_is_user => view! {
                            <div data-testid="my-data-not-user">
                                <EmptyState message="Sign in as a member to see your data." />
                            </div>
                        }.into_any(),
                        Ok(data) if data.schemas.is_empty() => view! {
                            <div data-testid="my-data-empty">
                                <EmptyState message="Valence holds no records that you own" />
                            </div>
                        }.into_any(),
                        Ok(data) => view! { <MyDataBody data=data /> }.into_any(),
                        Err(_) => view! {
                            <div data-testid="my-data-error">
                                <MessageBar intent=MessageBarIntent::Error layout=MessageBarLayout::Multiline>
                                    <MessageBarBody>
                                        "Valence didn't answer. Nothing changed in your data; this page just can't read it right now."
                                    </MessageBarBody>
                                    <MessageBarActions>
                                        <Button on_click=Callback::new(move |_| res.refetch())>"Try again"</Button>
                                    </MessageBarActions>
                                </MessageBar>
                            </div>
                        }.into_any(),
                    })}
                </Transition>
            </Flex>
        </ContentContainer>
    }
}

/// Stat cards, the account note and one card per schema.
#[component]
fn MyDataBody(data: MyData) -> impl IntoView {
    let MyData {
        records,
        kinds,
        being_deleted,
        schemas,
        ..
    } = data;

    view! {
        <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
            <div id="valence-my-data-stats" data-testid="my-data-stats">
                <AutoGrid min="220px" gap=SpacingSize::Size160>
                    <StatCard label="Records you own" value=Signal::derive(move || records.to_string()) />
                    <StatCard label="Kinds of record" value=Signal::derive(move || kinds.to_string()) />
                    <StatCard
                        label="Being deleted"
                        value=Signal::derive(move || being_deleted.to_string())
                        variant=StatCardVariant::Warning
                    />
                </AutoGrid>
            </div>

            <div id="valence-my-data-account" data-testid="my-data-account">
                <MessageBar intent=MessageBarIntent::Info layout=MessageBarLayout::Multiline>
                    <MessageBarTitle>"Deleting your account"</MessageBarTitle>
                    <MessageBarBody>
                        "Close your account in Account Settings. Valence then removes everything on this page within 30 days and shows the progress here."
                    </MessageBarBody>
                    <MessageBarActions>
                        <A href=ACCOUNT_SETTINGS_PATH>
                            <Button appearance=ButtonAppearance::Secondary>"Open Account Settings"</Button>
                        </A>
                    </MessageBarActions>
                </MessageBar>
            </div>

            <div id="valence-my-data-schemas">
                <Flex vertical=true gap=SpacingSize::Size160.flex_gap()>
                    {schemas
                        .into_iter()
                        .map(|schema| view! { <MyDataSchemaCard schema=schema /> })
                        .collect_view()}
                </Flex>
            </div>
        </Flex>
    }
}

/// One kind of record: why it's kept, retention and the newest few.
#[component]
fn MyDataSchemaCard(schema: MyDataSchema) -> impl IntoView {
    let MyDataSchema {
        name,
        count,
        being_deleted,
        why_kept,
        retention,
        newest,
        more,
    } = schema;
    let see_all = valence_backend::valence_schema_path(&name);
    let testid = format!("my-data-schema-{name}");
    let records_schema = name.clone();
    let title = format!("{name} · {count}");

    view! {
        <div data-testid=testid data-schema=name>
            <Card>
                <CardHeader>
                    <Subtitle1>{title}</Subtitle1>
                    <CardHeaderAction slot>
                        <Flex gap=FlexGap::Small align=FlexAlign::Center>
                            {(being_deleted > 0).then(|| view! {
                                <span data-testid="my-data-being-deleted">
                                    <Badge color=BadgeColor::Warning appearance=BadgeAppearance::Tint>
                                        {format!("{being_deleted} being deleted")}
                                    </Badge>
                                </span>
                            })}
                            <A href=see_all>
                                <Button appearance=ButtonAppearance::Subtle>"See all"</Button>
                            </A>
                        </Flex>
                    </CardHeaderAction>
                </CardHeader>
                <CardContent>
                    <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                        <div data-testid="my-data-why-kept">
                            <Flex vertical=true gap=FlexGap::Small>
                                <Caption1>"Why it's kept"</Caption1>
                                {if why_kept.is_empty() {
                                    view! { <Body1>"This site hasn't declared why it reads these records."</Body1> }.into_any()
                                } else {
                                    why_kept
                                        .into_iter()
                                        .map(|line| view! { <Body1>{line}</Body1> })
                                        .collect_view()
                                        .into_any()
                                }}
                                <span data-testid="my-data-retention">
                                    <Caption1>{retention}</Caption1>
                                </span>
                            </Flex>
                        </div>
                        <MyDataRecords schema=records_schema records=newest />
                        {(more > 0).then(|| view! {
                            <span data-testid="my-data-more">
                                <Caption1>{format!("and {more} more")}</Caption1>
                            </span>
                        })}
                    </Flex>
                </CardContent>
            </Card>
        </div>
    }
}

/// Newest records as a table, or a list below the Md breakpoint.
#[component]
fn MyDataRecords(schema: String, records: Vec<MyDataRecord>) -> impl IntoView {
    let compact = use_breakpoint_down(Breakpoint::Md);
    let rows: Vec<(String, String, String)> = records
        .into_iter()
        .map(|r| {
            let href = valence_backend::valence_entity_path(&schema, &r.id);
            (href, r.label, created_label(r.created_at.as_deref()))
        })
        .collect();

    move || {
        let rows = rows.clone();
        if compact.get() {
            view! {
                <div data-testid="my-data-records-list">
                    <List>
                        {rows.into_iter().map(|(href, label, created)| view! {
                            <ListItem>
                                <Flex vertical=true gap=FlexGap::Small>
                                    <A href=href>
                                        <Body1>{label}</Body1>
                                    </A>
                                    <Caption1>{created}</Caption1>
                                </Flex>
                            </ListItem>
                        }).collect_view()}
                    </List>
                </div>
            }
            .into_any()
        } else {
            view! {
                <div data-testid="my-data-records-table">
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHeaderCell>"Record"</TableHeaderCell>
                                <TableHeaderCell>"Created"</TableHeaderCell>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {rows.into_iter().map(|(href, label, created)| view! {
                                <TableRow>
                                    <TableCell>
                                        <A href=href>
                                            <Body1>{label}</Body1>
                                        </A>
                                    </TableCell>
                                    <TableCell><Body1>{created}</Body1></TableCell>
                                </TableRow>
                            }).collect_view()}
                        </TableBody>
                    </Table>
                </div>
            }
            .into_any()
        }
    }
}

/// Loading: stat and table placeholders under the page header.
#[component]
fn MyDataSkeleton() -> impl IntoView {
    view! {
        <div data-testid="my-data-loading">
            <Flex vertical=true gap=SpacingSize::Size240.flex_gap()>
                <AutoGrid min="220px" gap=SpacingSize::Size160>
                    {(0..3).map(|_| view! {
                        <Card>
                            <CardContent>
                                <Skeleton><SkeletonItem /></Skeleton>
                            </CardContent>
                        </Card>
                    }).collect_view()}
                </AutoGrid>
                <Card>
                    <CardContent>
                        <Table>
                            <TableHeader>
                                <TableRow>
                                    <TableHeaderCell>"Record"</TableHeaderCell>
                                    <TableHeaderCell>"Created"</TableHeaderCell>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {(0..3).map(|_| view! {
                                    <TableRow>
                                        <TableCell><Skeleton><SkeletonItem /></Skeleton></TableCell>
                                        <TableCell><Skeleton><SkeletonItem /></Skeleton></TableCell>
                                    </TableRow>
                                }).collect_view()}
                            </TableBody>
                        </Table>
                    </CardContent>
                </Card>
            </Flex>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::created_label;

    #[test]
    fn created_label_formats_utc_date() {
        assert_eq!(
            created_label(Some("2026-04-28T23:30:00-02:00")),
            "Apr 29, 2026"
        );
        assert_eq!(created_label(None), "—");
        assert_eq!(created_label(Some("soon")), "—");
    }
}
