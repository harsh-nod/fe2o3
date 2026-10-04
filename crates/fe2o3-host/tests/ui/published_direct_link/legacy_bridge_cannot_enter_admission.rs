use fe2o3_artifacts::{
    ArtifactContainerV1, DirectLinkPublicationBridgeV1,
    ManifestClaimDirectLinkCurrentPublicationLeaseV1, SelectedNativeKernel,
    ValidatedDirectLinkBundleEvidenceV1,
};
use fe2o3_host::{ObservedContext, ValidatedPublishedDirectLinkSelectionV1};

fn admit_legacy(
    validated: &ValidatedDirectLinkBundleEvidenceV1<'_>,
    legacy: &DirectLinkPublicationBridgeV1,
    current: ManifestClaimDirectLinkCurrentPublicationLeaseV1,
    container: &ArtifactContainerV1,
    selected: SelectedNativeKernel<'_>,
    observed: &ObservedContext,
) {
    let _ = ValidatedPublishedDirectLinkSelectionV1::validate(
        validated, legacy, current, container, selected, observed,
    );
}

fn main() {}
