use super::*;
use crate::currentness_diagnostic::Disabled;

fn set_gfx950_node(fixture: &Fixture, node: u32) {
    for (old, new) in [
        ("gfx_target_version 90402", "gfx_target_version 90500"),
        ("simd_count 1216", "simd_count 1024"),
        ("lds_size_in_kb 64", "lds_size_in_kb 160"),
        ("device_id 29857", "device_id 30112"),
        ("fw_version 192", "fw_version 41"),
        ("sdma_fw_version 25", "sdma_fw_version 12"),
    ] {
        fixture.replace_property(node, old, new);
    }
}

#[test]
fn exact_target_names_and_encodings_remain_distinct() {
    assert_eq!(GfxTarget::Gfx942.name(), "gfx942");
    assert_eq!(GfxTarget::Gfx942.encoded_version(), 90_402);
    assert_eq!(GfxTarget::Gfx950.name(), "gfx950");
    assert_eq!(GfxTarget::Gfx950.encoded_version(), 90_500);
}

#[test]
fn explicit_gfx950_discovery_retains_observed_target_and_geometry() {
    let fixture = Fixture::valid(2);
    for node in 1..=2 {
        set_gfx950_node(&fixture, node);
    }
    let observed = discover_topology_at(&fixture.root, GfxTarget::Gfx950).unwrap();
    assert_eq!(observed.observed_node_count(), 3);
    assert_eq!(observed.gpu_nodes().len(), 2);
    assert_eq!(observed.provenance().generation(), 7);
    for gpu in observed.gpu_nodes() {
        assert_eq!(gpu.target(), GfxTarget::Gfx950);
        assert_eq!(gpu.pci_device_id(), 0x75a0);
        assert_eq!(gpu.fw_version(), 41);
        assert_eq!(gpu.sdma_fw_version(), 12);
        assert_eq!(gpu.capacity().simd_count(), 1024);
        assert_eq!(gpu.capacity().lds_size_in_kb(), 160);
        assert_eq!(gpu.capacity().xcc_count(), 8);
        assert_eq!(gpu.capacity().wavefront_size(), 64);
        assert_eq!(gpu.io_links().len(), 2);
    }
    assert!(matches!(
        fixture.discover(),
        Err(TopologyError::UnsupportedTarget {
            node_id: 1,
            encoded: 90_500
        })
    ));
}

#[test]
fn explicit_gfx942_discovery_matches_default_and_gfx950_rejects_it() {
    let fixture = Fixture::valid(2);
    assert_eq!(
        fixture.discover().unwrap(),
        discover_topology_at(&fixture.root, GfxTarget::Gfx942).unwrap()
    );
    assert!(matches!(
        discover_topology_at(&fixture.root, GfxTarget::Gfx950),
        Err(TopologyError::UnsupportedTarget {
            node_id: 1,
            encoded: 90_402
        })
    ));
}

#[test]
fn either_selection_rejects_a_mixed_inventory_without_filtering_nodes() {
    let fixture = Fixture::valid(2);
    set_gfx950_node(&fixture, 2);
    for (target, expected_node, expected_encoding) in [
        (GfxTarget::Gfx942, 2, 90_500),
        (GfxTarget::Gfx950, 1, 90_402),
    ] {
        assert!(matches!(
            discover_topology_at(&fixture.root, target),
            Err(TopologyError::UnsupportedTarget { node_id, encoded })
                if node_id == expected_node && encoded == expected_encoding
        ));
    }
}

#[test]
fn either_selection_rejects_unknown_target_encoding() {
    let fixture = Fixture::valid(1);
    for encoded in [0, 90_501, 110_000] {
        fixture.replace_property(
            1,
            "gfx_target_version 90402",
            &format!("gfx_target_version {encoded}"),
        );
        for target in [GfxTarget::Gfx942, GfxTarget::Gfx950] {
            assert!(matches!(
                discover_topology_at(&fixture.root, target),
                Err(TopologyError::UnsupportedTarget { node_id: 1, encoded: actual })
                    if actual == encoded
            ));
        }
        fixture.replace_property(
            1,
            &format!("gfx_target_version {encoded}"),
            "gfx_target_version 90402",
        );
    }
}

#[test]
fn gfx950_observation_cannot_mint_a_gfx942_xgmi_route() {
    let fixture = Fixture::valid(2);
    for node in 1..=2 {
        set_gfx950_node(&fixture, node);
    }
    let observed = discover_topology_at(&fixture.root, GfxTarget::Gfx950).unwrap();
    for (source, destination) in [(1001, 1002), (1002, 1001)] {
        assert_eq!(
            observed.admit_gfx942_xgmi_route(source, destination),
            Err(Gfx942XgmiRouteErrorV1::UnsupportedTarget)
        );
    }
    // Check both endpoints independently even though public discovery rejects mixed targets.
    for foreign_node in 0..2 {
        let mut observed = observed.clone();
        observed.gpu_nodes[0].target = GfxTarget::Gfx942;
        observed.gpu_nodes[1].target = GfxTarget::Gfx942;
        observed.gpu_nodes[foreign_node].target = GfxTarget::Gfx950;
        assert_eq!(
            observed.admit_gfx942_xgmi_route(1001, 1002),
            Err(Gfx942XgmiRouteErrorV1::UnsupportedTarget)
        );
    }
}

#[test]
fn gfx950_host_observation_preserves_render_correlation() {
    let fixture = Fixture::valid(1);
    let render = RenderFixture::valid();
    set_gfx950_node(&fixture, 1);
    fixture.replace_property(1, "location_id 4096", "location_id 1280");
    fixture.replace_property(1, "unique_id 2001", "unique_id 4660");
    fs::write(render.pci_path.join("device"), "0x75a0\n").unwrap();
    let mut paths = render.paths();
    paths.topology_root = &fixture.root;
    let (observed, ()) =
        discover_host_topology_with::<Disabled>(&paths, GfxTarget::Gfx950).unwrap();
    assert_eq!(
        observed.topology().gpu_nodes()[0].target(),
        GfxTarget::Gfx950
    );
    assert_eq!(observed.render_nodes().len(), 1);
    assert_eq!(observed.render_nodes()[0].unique_id(), 4660);
    fs::write(render.pci_path.join("device"), "0x74a1\n").unwrap();
    assert!(matches!(
        discover_host_topology_with::<Disabled>(&paths, GfxTarget::Gfx950),
        Err(TopologyError::RenderCorrelationMismatch {
            field: "PCI device",
            ..
        })
    ));
}

#[test]
fn gfx950_selection_keeps_closed_property_and_wave_width_checks() {
    let fixture = Fixture::valid(1);
    set_gfx950_node(&fixture, 1);
    fixture.replace_property(1, "wave_front_size 64", "wave_front_size 32");
    assert!(matches!(
        discover_topology_at(&fixture.root, GfxTarget::Gfx950),
        Err(TopologyError::PropertyOutOfRange { key, .. }) if key == "wave_front_size"
    ));
    fixture.replace_property(
        1,
        "wave_front_size 32",
        "wave_front_size 64\nunknown_profile 1",
    );
    assert!(matches!(
        discover_topology_at(&fixture.root, GfxTarget::Gfx950),
        Err(TopologyError::UnknownProperty { key, .. }) if key == "unknown_profile"
    ));
}

#[test]
#[ignore = "requires an explicitly selected live gfx950 host; read-only sysfs and procfs discovery"]
fn live_gfx950_topology_is_observed_without_gfx942_route_authority() {
    let observed = discover_default_topology_for_target(GfxTarget::Gfx950).unwrap();
    let nodes = observed.topology().gpu_nodes();
    assert!(
        nodes.len() >= 2,
        "multi-GPU topology observation requires two GPUs"
    );
    assert_eq!(nodes.len(), observed.render_nodes().len());
    for gpu in nodes {
        assert_eq!(gpu.target(), GfxTarget::Gfx950);
        let render = observed
            .render_nodes()
            .iter()
            .find(|render| render.node_id() == gpu.node_id())
            .unwrap();
        assert_eq!(gpu.unique_id(), render.unique_id());
        assert_eq!(gpu.drm_render_minor(), render.drm_render_minor());
        println!(
            "fe2o3.observed-topology.v1 target={} node={} gpu_id={} uid={} render_minor={} simds={} lds_kib={} xccs={}",
            gpu.target().name(),
            gpu.node_id(),
            gpu.gpu_id(),
            gpu.unique_id(),
            gpu.drm_render_minor(),
            gpu.capacity().simd_count(),
            gpu.capacity().lds_size_in_kb(),
            gpu.capacity().xcc_count(),
        );
    }
    let source = u32::try_from(nodes[0].gpu_id()).unwrap();
    let destination = u32::try_from(nodes[1].gpu_id()).unwrap();
    for (source, destination) in [(source, destination), (destination, source)] {
        assert_eq!(
            observed
                .topology()
                .admit_gfx942_xgmi_route(source, destination),
            Err(Gfx942XgmiRouteErrorV1::UnsupportedTarget)
        );
    }
    assert!(matches!(
        discover_default_topology(),
        Err(TopologyError::UnsupportedTarget {
            encoded: 90_500,
            ..
        })
    ));
}
