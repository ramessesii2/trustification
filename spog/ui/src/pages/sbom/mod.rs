//! The SBOM details page

use crate::{common::clean_ext, model, pages::sbom_report::SbomReport};
use patternfly_yew::prelude::*;
use reqwest::Body;
use spog_ui_backend::{use_backend, AnalyzeService};
use spog_ui_common::error::components::ApiError;
use spog_ui_common::{config::use_config, error::components::Error};
use spog_ui_components::{
    common::{NotFound, PageHeading},
    content::{SourceCode, Technical},
    download::LocalDownloadButton,
    sbom::Report,
    spdx::*,
};
use std::rc::Rc;
use yew::prelude::*;
use yew_more_hooks::prelude::*;
use yew_oauth2::prelude::*;

#[derive(Clone, Debug, PartialEq, Properties)]
pub struct SBOMProperties {
    pub id: String,
}

#[function_component(SBOM)]
pub fn sbom(props: &SBOMProperties) -> Html {
    let backend = use_backend();
    let access_token = use_latest_access_token();

    let info = use_async_with_cloned_deps(
        |(id, backend)| async move {
            spog_ui_backend::SBOMService::new(backend.clone(), access_token)
                .get(id)
                .await
                .map(|result| result.map(model::SBOM::parse).map(Rc::new))
        },
        (props.id.clone(), backend),
    );

    let title = html!(<SBOMTitle id={props.id.clone()}/>);

    let (heading, content) = match &*info {
        UseAsyncState::Pending | UseAsyncState::Processing => (
            html!(<PageHeading>{title}</PageHeading>),
            html!(<PageSection fill={PageSectionFill::Fill}><Spinner/></PageSection>),
        ),
        UseAsyncState::Ready(Ok(None)) => (
            html!(<PageHeading sticky=false>{title} {" "} </PageHeading>),
            html!(<NotFound/>),
        ),
        UseAsyncState::Ready(Ok(Some(data))) => (
            html!(
                <PageHeading
                    sticky=false
                    action={html!(
                        <LocalDownloadButton data={data.get_source()} r#type="sbom" filename={clean_ext(&props.id)} />
                    )}
                >
                    {title} {" "} <Label label={data.type_name()} color={Color::Blue} />
                </PageHeading>
            ),
            html!(<Details id={props.id.clone()} sbom={data.clone()}/> ),
        ),
        UseAsyncState::Ready(Err(err)) => (
            html!(<PageHeading>{title}</PageHeading>),
            html!(<PageSection fill={PageSectionFill::Fill}><Error err={err.to_string()} /></PageSection>),
        ),
    };

    html!(
        <>
            { heading }
            { content }
        </>
    )
}

#[derive(Clone, Debug, PartialEq, Properties)]
pub struct SBOMTitleProperties {
    pub id: String,
}

#[function_component(SBOMTitle)]
pub fn sbom_title(props: &SBOMTitleProperties) -> Html {
    let backend = use_backend();
    let access_token = use_latest_access_token();

    let sbom = use_async_with_cloned_deps(
        |(id, backend)| async move {
            spog_ui_backend::SBOMService::new(backend.clone(), access_token)
                .get_from_index(&id)
                .await
                .map(|search_result| {
                    if search_result.result.len() == 1 {
                        let data = &search_result.result[0];
                        Some(data.clone())
                    } else {
                        None
                    }
                })
        },
        (props.id.clone(), backend),
    );

    let title = match &*sbom {
        UseAsyncState::Ready(Ok(Some(data))) => Some(data.name.clone()),
        _ => None,
    };

    html!(
        <>
          {title.unwrap_or(props.id.clone())}
        </>
    )
}

#[derive(Clone, PartialEq, Properties)]
struct DetailsProps {
    id: String,
    sbom: Rc<model::SBOM>,
}

#[function_component(Details)]
fn details(props: &DetailsProps) -> Html {
    #[derive(Copy, Clone, Eq, PartialEq)]
    enum TabIndex {
        Overview,
        Info,
        Packages,
        Source,
        Report,
    }

    let config = use_config();

    let tab = use_state_eq(|| TabIndex::Info);
    let onselect = use_callback(tab.clone(), |index, tab| tab.set(index));

    match &*props.sbom {
        model::SBOM::SPDX { bom, source } => {
            html!(
                <>
                    <PageSection r#type={PageSectionType::Tabs} variant={PageSectionVariant::Light} sticky={[PageSectionSticky::Top]}>
                        <Tabs<TabIndex> inset={TabInset::Page} detached=true selected={*tab} {onselect}>
                            <Tab<TabIndex> index={TabIndex::Info} title="Info" />
                            <Tab<TabIndex> index={TabIndex::Packages} title="Packages" />
                            <Tab<TabIndex> index={TabIndex::Overview} title="Related advisories" />
                            { for config.features.show_report.then(|| html_nested!(
                                <Tab<TabIndex> index={TabIndex::Report} title="Report" />
                            )) }
                            { for config.features.show_source.then(|| html_nested!(
                                <Tab<TabIndex> index={TabIndex::Source} title="Source" />
                            )) }
                        </Tabs<TabIndex>>
                    </PageSection>

                    <PageSection hidden={*tab != TabIndex::Overview} fill={PageSectionFill::Fill}>
                        <SbomReport id={props.id.clone()} />
                    </PageSection>

                    <PageSection hidden={*tab != TabIndex::Info} fill={PageSectionFill::Fill}>
                        <Stack gutter=true>
                            <StackItem>
                                <Grid gutter=true>
                                    <GridItem cols={[6]}>{spdx_meta(bom)}</GridItem>
                                    <GridItem cols={[3]}>{spdx_creator(bom)}</GridItem>
                                    <GridItem cols={[3]}>{spdx_stats(source.as_bytes().len(), bom)}</GridItem>
                                </Grid>
                            </StackItem>
                            <StackItem>
                                <Grid gutter=true>
                                    <GridItem cols={[12]}>{spdx_main(bom)}</GridItem>
                                </Grid>
                            </StackItem>
                        </Stack>
                    </PageSection>

                    <PageSection hidden={*tab != TabIndex::Packages} fill={PageSectionFill::Fill}>
                        <SpdxPackages bom={bom.clone()} />
                    </PageSection>

                    <PageSection hidden={*tab != TabIndex::Source} variant={PageSectionVariant::Light} fill={PageSectionFill::Fill}>
                        <SourceCode source={source.clone()} />
                    </PageSection>
                    <PageSection hidden={*tab != TabIndex::Report} variant={PageSectionVariant::Light} fill={PageSectionFill::Fill}>
                        <ReportViewwer raw={source.clone()}/>
                    </PageSection>
                </>
            )
        }
        model::SBOM::CycloneDX { bom: _, source } => {
            html!(
                <>
                    <PageSection r#type={PageSectionType::Tabs} variant={PageSectionVariant::Light} sticky={[PageSectionSticky::Top]}>
                        <Tabs<TabIndex> inset={TabInset::Page} detached=true selected={*tab} {onselect}>
                            <Tab<TabIndex> index={TabIndex::Info} title="Info" />
                            <Tab<TabIndex> index={TabIndex::Overview} title="Related advisories" />
                            { for config.features.show_report.then(|| html_nested!(
                                <Tab<TabIndex> index={TabIndex::Report} title="Report" />
                            )) }
                            { for config.features.show_source.then(|| html_nested!(
                                <Tab<TabIndex> index={TabIndex::Source} title="Source" />
                            )) }
                        </Tabs<TabIndex>>
                    </PageSection>

                    <PageSection hidden={*tab != TabIndex::Overview} fill={PageSectionFill::Fill}>
                        <SbomReport id={props.id.clone()} />
                    </PageSection>

                    <PageSection hidden={*tab != TabIndex::Info} fill={PageSectionFill::Fill}>
                        <Grid gutter=true>
                            <GridItem cols={[2]}><Technical size={source.as_bytes().len()}/></GridItem>
                        </Grid>
                    </PageSection>

                    <PageSection hidden={*tab != TabIndex::Source} fill={PageSectionFill::Fill}>
                        <SourceCode source={source.clone()} />
                    </PageSection>
                    <PageSection hidden={*tab != TabIndex::Report} fill={PageSectionFill::Fill}>
                        <ReportViewwer raw={source.clone()}/>
                    </PageSection>
                </>
            )
        }
        model::SBOM::Unknown(source) => {
            html!(
                <>
                    <PageSection r#type={PageSectionType::Tabs} variant={PageSectionVariant::Light} sticky={[PageSectionSticky::Top]}>
                        <Tabs<TabIndex> inset={TabInset::Page} detached=true selected={*tab} {onselect}>
                            <Tab<TabIndex> index={TabIndex::Overview} title="Overview" />
                            <Tab<TabIndex> index={TabIndex::Info} title="Info" />
                            <Tab<TabIndex> index={TabIndex::Source} title="Source" />
                        </Tabs<TabIndex>>
                    </PageSection>

                    <PageSection hidden={*tab != TabIndex::Overview} fill={PageSectionFill::Fill}>
                        <SbomReport id={props.id.clone()} />
                    </PageSection>

                    <PageSection hidden={*tab != TabIndex::Info} fill={PageSectionFill::Fill}>
                        <Grid gutter=true>
                            <GridItem cols={[2]}><Technical size={source.as_bytes().len()}/></GridItem>
                        </Grid>
                    </PageSection>

                    <PageSection hidden={*tab != TabIndex::Source} fill={PageSectionFill::Fill}>
                        <SourceCode source={source.clone()} />
                    </PageSection>
                    <PageSection hidden={*tab != TabIndex::Report} fill={PageSectionFill::Fill}>
                        <ReportViewwer raw={source.clone()}/>
                    </PageSection>
                </>
            )
        }
    }
}

#[derive(Clone, PartialEq, Properties)]
pub struct ReportViewwerProperties {
    pub raw: Rc<String>,
}

#[function_component(ReportViewwer)]
pub fn report_viewer(props: &ReportViewwerProperties) -> Html {
    let backend = use_backend();
    let access_token = use_latest_access_token();

    let fetch = {
        use_async_with_cloned_deps(
            move |raw| async move {
                let service = AnalyzeService::new(backend, access_token);
                service.report(Body::from((*raw).clone())).await.map(Rc::new)
            },
            props.raw.clone(),
        )
    };

    html!(
        <>
            {
                match &*fetch {
                    UseAsyncState::Pending | UseAsyncState::Processing => html!(
                        <PageSection fill={PageSectionFill::Fill}>
                            <Spinner />
                        </PageSection>
                    ),
                    UseAsyncState::Ready(Ok(data)) => html!(
                        <Report data={data.clone()} />
                    ),
                    UseAsyncState::Ready(Err(err)) => html!(
                        <ApiError error={err.clone()} />
                    )
                }
            }
        </>
    )
}
