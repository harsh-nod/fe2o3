//! Contracted SPX/XCP0 correlation for one exact engineering platform.
//! KFD's XCD UID is never compared or converted to the parent board UID.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct XcpSysfsIdentity {
    directory: FileIdentity,
    metrics: FileIdentity,
}

fn observe_xcp(path: &Path) -> Result<XcpSysfsIdentity, TopologyError> {
    let directory = ensure_directory(path)?;
    let metrics_path = path.join("xcp_metrics");
    let metadata = inspect(&metrics_path)?;
    if metadata.file_type().is_symlink() {
        return Err(TopologyError::Symlink(metrics_path));
    }
    if !metadata.is_file() {
        return Err(TopologyError::UnexpectedFileType {
            path: metrics_path,
            expected: "regular file",
        });
    }
    Ok(XcpSysfsIdentity {
        directory,
        metrics: FileIdentity::from_metadata(&metadata),
    })
}

pub(super) fn observe_parent(
    gpu: &GpuTopologyNode,
    render: &Path,
    pci: &Path,
    board_uid: u64,
    partition: PartitionProfile,
) -> Result<XcpSysfsIdentity, TopologyError> {
    // Reviewed kfd_topology.c binds the render minor and BDF to the same adev;
    // amdgpu_xcp.c maps XCP0 to that adev's parent DRM node. Other XCPs use
    // platform render devices and are deliberately outside this profile.
    if render.parent().and_then(Path::parent) != Some(pci) {
        return Err(mismatch(
            gpu.node_id,
            "XCP0 PCI ancestry",
            "parent render",
            "other parent",
        ));
    }
    let c = &gpu.capacity;
    if gpu.target != GfxTarget::Gfx950
        || gpu.pci_device_id != 0x75a0
        || gpu.fw_version != 41
        || gpu.sdma_fw_version != 12
        || [
            c.simd_count,
            c.simd_per_cu,
            c.xcc_count,
            c.array_count,
            c.simd_arrays_per_engine,
            c.lds_size_in_kb,
            c.max_waves_per_simd,
            c.compute_queue_count,
            c.wavefront_size,
        ] != [1024, 4, 8, 32, 1, 160, 8, 24, 64]
        || partition != V1_PARTITION_PROFILE
        || board_uid == 0
        || gpu.unique_id == 0
    {
        return Err(mismatch(
            gpu.node_id,
            "XCP0 full-device profile",
            "gfx950 SPX/NPS1",
            "unsupported",
        ));
    }
    // Only valid XCPs expose this attribute. Inspect its identity, not changing
    // telemetry contents; retain both records for subsequent snapshot checks.
    let path = pci.join("xcp");
    let before = observe_xcp(&path)?;
    if observe_xcp(&path)? != before {
        return Err(TopologyError::ChangedDuringRead(path));
    }
    Ok(before)
}

pub(super) fn validate_inventory(renders: &[RenderNodeObservation]) -> Result<(), TopologyError> {
    for (index, render) in renders.iter().enumerate() {
        if renders[..index]
            .iter()
            .any(|prior| prior.pci_address == render.pci_address)
        {
            return Err(mismatch(
                render.node_id,
                "XCP0 inventory",
                "one node per PCI endpoint",
                "duplicate endpoint",
            ));
        }
    }
    Ok(())
}
