use fe2o3_artifact_transaction::DurableCurrentLinkPublicationLeaseV1;
use fe2o3_artifacts::{
    ArtifactContainerV1, ManifestClaimDirectLinkPublicationBridgeV1, SelectedNativeKernel,
    ValidatedDirectLinkBundleEvidenceV1,
};
use fe2o3_host::{ObservedContext, ValidatedPublishedDirectLinkSelectionV1};

fn admit_generic(
    validated: &ValidatedDirectLinkBundleEvidenceV1<'_>,
    bridge: &ManifestClaimDirectLinkPublicationBridgeV1,
    current: DurableCurrentLinkPublicationLeaseV1,
    container: &ArtifactContainerV1,
    selected: SelectedNativeKernel<'_>,
    observed: &ObservedContext,
) {
    let _ = ValidatedPublishedDirectLinkSelectionV1::validate(
        validated, bridge, current, container, selected, observed,
    );
}

fn main() {}
