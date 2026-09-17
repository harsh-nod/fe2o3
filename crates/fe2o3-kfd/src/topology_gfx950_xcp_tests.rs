use super::*;
include!("topology_gfx950_asrock_tests.rs");

fn fixture() -> RenderFixture {
    let mut f = RenderFixture::valid();
    f.gpu.target = GfxTarget::Gfx950;
    f.gpu.pci_device_id = 0x75a0;
    f.gpu.fw_version = 41;
    f.gpu.sdma_fw_version = 12;
    f.gpu.capacity.simd_count = 1024;
    f.gpu.capacity.lds_size_in_kb = 160;
    fs::write(&f.os_release, "5.18.2-mi300-build-140423-ubuntu-22.04+\n").unwrap();
    fs::write(
        f.module_root.join("srcversion"),
        "975C4B2AA8AD01E2EA472C0\n",
    )
    .unwrap();
    fs::write(f.pci_path.join("device"), "0x75a0\n").unwrap();
    fs::write(f.pci_path.join("unique_id"), "5678\n").unwrap();
    fs::create_dir(f.pci_path.join("xcp")).unwrap();
    fs::write(f.pci_path.join("xcp/xcp_metrics"), []).unwrap();
    f
}

fn correlate(f: &RenderFixture) -> Result<RenderNodeObservation, TopologyError> {
    let policy = select_render_identity_correlation(
        f.gpu.target,
        &read_kernel_release(&f.os_release)?,
        &observe_amdgpu_module(&f.module_root)?,
    );
    correlate_render_node(&f.gpu, &f.paths(), &canonicalize(&f.devices_root)?, policy)
}

#[test]
fn xcp0_profile_retains_both_uid_domains_and_explicit_contract() {
    let f = fixture();
    let observed = correlate(&f).unwrap();
    assert_eq!(observed.unique_id(), 0x5678);
    assert_eq!(observed.kfd_unique_id(), 0x1234);
    assert_eq!(
        observed.identity_correlation(),
        RenderIdentityCorrelationV1::Gfx950EngineeringXcp0ViaParentRenderV1
    );
    assert_eq!(correlate(&f).unwrap(), observed);
    assert!(f.correlate().is_err());
}

#[test]
fn xcp0_route_is_selected_by_profile_not_mismatch_fallback() {
    let mut f = fixture();
    fs::write(f.pci_path.join("unique_id"), "1234\n").unwrap();
    assert_eq!(
        correlate(&f).unwrap().identity_correlation(),
        RenderIdentityCorrelationV1::Gfx950EngineeringXcp0ViaParentRenderV1
    );
    fs::remove_file(f.pci_path.join("xcp/xcp_metrics")).unwrap();
    assert!(correlate(&f).is_err());
    f.gpu.target = GfxTarget::Gfx942;
    assert_eq!(
        correlate(&f).unwrap().identity_correlation(),
        RenderIdentityCorrelationV1::SameDeviceUid
    );
}

#[test]
fn xcp0_requires_exact_platform_and_target() {
    for (field, value) in [
        ("kernel", "6.8.0-124-generic\n"),
        ("version", "6.16.12\n"),
        ("srcversion", "703B1127E578BC5D4BD6615\n"),
    ] {
        let f = fixture();
        let path = if field == "kernel" {
            f.os_release.clone()
        } else {
            f.module_root.join(field)
        };
        fs::write(path, value).unwrap();
        assert!(correlate(&f).is_err(), "accepted {field}");
    }
    let mut f = fixture();
    f.gpu.target = GfxTarget::Gfx942;
    assert!(correlate(&f).is_err());
}

#[test]
fn xcp0_mutation_of_either_uid_changes_retained_observation() {
    let mut f = fixture();
    let before = correlate(&f).unwrap();
    fs::write(f.pci_path.join("unique_id"), "5679\n").unwrap();
    assert_ne!(correlate(&f).unwrap(), before);
    fs::write(f.pci_path.join("unique_id"), "5678\n").unwrap();
    f.gpu.unique_id += 1;
    assert_ne!(correlate(&f).unwrap(), before);
    f.gpu.unique_id = 0;
    assert!(correlate(&f).is_err());
    f.gpu.unique_id = 0x1234;
    fs::write(f.pci_path.join("unique_id"), "0\n").unwrap();
    assert!(correlate(&f).is_err());
}

#[test]
fn xcp0_rejects_other_partitions_and_every_geometry_mutation() {
    for (field, value) in [
        ("current_compute_partition", "CPX\n"),
        ("current_memory_partition", "NPS4\n"),
    ] {
        let f = fixture();
        fs::write(f.pci_path.join(field), value).unwrap();
        assert!(correlate(&f).is_err());
    }
    let mutations: &[fn(&mut GpuTopologyNode)] = &[
        |g| g.capacity.simd_count += 1,
        |g| g.capacity.simd_per_cu += 1,
        |g| g.capacity.xcc_count += 1,
        |g| g.capacity.array_count += 1,
        |g| g.capacity.simd_arrays_per_engine += 1,
        |g| g.capacity.lds_size_in_kb += 1,
        |g| g.capacity.max_waves_per_simd += 1,
        |g| g.capacity.compute_queue_count += 1,
        |g| g.capacity.wavefront_size = 32,
        |g| g.fw_version += 1,
        |g| g.sdma_fw_version += 1,
    ];
    for mutate in mutations {
        let mut f = fixture();
        mutate(&mut f.gpu);
        assert!(correlate(&f).is_err());
    }
}

#[test]
fn xcp0_rejects_wrong_pci_location_and_render_minor() {
    let mut f = fixture();
    f.gpu.location_id += 1;
    assert!(correlate(&f).is_err());
    f.gpu.location_id -= 1;
    f.gpu.drm_render_minor += 1;
    assert!(correlate(&f).is_err());
    f.gpu.drm_render_minor -= 1;
    fs::write(f.pci_path.join("drm/renderD128/dev"), "226:129\n").unwrap();
    assert!(correlate(&f).is_err());
}

#[test]
fn xcp0_rejects_virtual_render_with_same_device_symlink() {
    use std::os::unix::fs::symlink;
    let f = fixture();
    let render = f.devices_root.join("platform/synthetic-xcp/drm/renderD128");
    fs::create_dir_all(&render).unwrap();
    fs::write(render.join("dev"), "226:128\n").unwrap();
    symlink(&f.pci_path, render.join("device")).unwrap();
    let link = f.device_character_root.join("226:128");
    fs::remove_file(&link).unwrap();
    symlink(&render, link).unwrap();
    assert!(matches!(
        correlate(&f),
        Err(TopologyError::RenderCorrelationMismatch {
            field: "XCP0 PCI ancestry",
            ..
        })
    ));
}

#[test]
fn xcp0_rejects_missing_symlinked_or_nonregular_evidence() {
    use std::os::unix::fs::symlink;
    for variant in 0..4 {
        let f = fixture();
        let xcp = f.pci_path.join("xcp");
        let metrics = xcp.join("xcp_metrics");
        fs::remove_file(&metrics).unwrap();
        match variant {
            0 => {}
            1 => {
                symlink(f.pci_path.join("unique_id"), &metrics).unwrap();
            }
            2 => {
                fs::create_dir(&metrics).unwrap();
            }
            _ => {
                fs::remove_dir(&xcp).unwrap();
                symlink(&f.pci_path, &xcp).unwrap();
            }
        }
        assert!(correlate(&f).is_err());
    }
}

#[test]
fn xcp0_replaced_evidence_and_duplicate_parent_change_or_reject_identity() {
    let f = fixture();
    let before = correlate(&f).unwrap();
    let metrics = f.pci_path.join("xcp/xcp_metrics");
    fs::rename(&metrics, f.pci_path.join("xcp/retained-old-metrics")).unwrap();
    fs::write(&metrics, []).unwrap();
    assert_ne!(correlate(&f).unwrap(), before);
    assert!(gfx950_xcp::validate_inventory(&[before.clone(), before]).is_err());
}

#[test]
fn xcp0_directory_replacement_changes_retained_identity() {
    let f = fixture();
    let before = correlate(&f).unwrap();
    let path = f.pci_path.join("xcp");
    let retained = f.pci_path.join("retained-old-xcp");
    fs::rename(&path, &retained).unwrap();
    fs::create_dir(&path).unwrap();
    fs::rename(retained.join("xcp_metrics"), path.join("xcp_metrics")).unwrap();
    let after = correlate(&f).unwrap();
    assert_eq!(before.unique_id(), after.unique_id());
    assert_eq!(before.kfd_unique_id(), after.kfd_unique_id());
    assert_ne!(before, after);
}

#[test]
fn missing_module_observations_cannot_select_xcp0_contract() {
    for field in ["version", "srcversion"] {
        let f = fixture();
        fs::remove_file(f.module_root.join(field)).unwrap();
        let policy = select_render_identity_correlation(
            GfxTarget::Gfx950,
            &read_kernel_release(&f.os_release).unwrap(),
            &observe_amdgpu_module(&f.module_root).unwrap(),
        );
        assert_eq!(policy, RenderIdentityCorrelationV1::SameDeviceUid);
        assert!(correlate(&f).is_err());
    }
}
