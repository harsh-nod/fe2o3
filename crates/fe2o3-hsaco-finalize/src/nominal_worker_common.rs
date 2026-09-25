//! Shared strict Worker policy checks; descriptor codecs retain their own schemas.
use crate::worker_v3_hsaco_admission::{WorkerV3HsacoInspectionError, WorkerV3LaunchContractV1};

pub(crate) fn common_launch<F: From<WorkerV3HsacoInspectionError>>(
    count: usize,
    mut launch_at: impl FnMut(usize) -> Result<WorkerV3LaunchContractV1, F>,
) -> Result<WorkerV3LaunchContractV1, F> {
    let mut launch = None;
    for index in 0..count {
        let actual = launch_at(index)?;
        if launch.is_some_and(|expected| expected != actual) {
            return Err(
                WorkerV3HsacoInspectionError::StrictV3DescriptorLaunchContract(
                    "heterogeneous per-kernel launch policy",
                )
                .into(),
            );
        }
        launch = Some(actual);
    }
    launch.ok_or_else(|| {
        WorkerV3HsacoInspectionError::StrictV3DescriptorLaunchContract("kernel set").into()
    })
}

pub(crate) fn export_manifest_matches<E>(
    receipt: &[u8],
    manifest: &[u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    charge(receipt.len().max(manifest.len()).saturating_add(1))?;
    Ok(receipt == manifest)
}
