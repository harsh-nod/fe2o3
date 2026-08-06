use fe2o3_artifact_transaction::PublishedLinkArtifactV1;
use fe2o3_artifacts::{
    ArtifactContainerV1, DirectLinkPublicationBridgeV1, SelectedNativeKernel,
    ValidatedDirectLinkBundleEvidenceV1,
};
use fe2o3_host::{ObservedContext, ValidatedPublishedDirectLinkSelectionV1};

fn admit_legacy(
    validated: &ValidatedDirectLinkBundleEvidenceV1<'_>,
    legacy: &DirectLinkPublicationBridgeV1,
    published: PublishedLinkArtifactV1,
    container: &ArtifactContainerV1,
    selected: SelectedNativeKernel<'_>,
    observed: &ObservedContext,
) {
    let _ = ValidatedPublishedDirectLinkSelectionV1::validate(
        validated, legacy, published, container, selected, observed,
    );
}

fn main() {}
