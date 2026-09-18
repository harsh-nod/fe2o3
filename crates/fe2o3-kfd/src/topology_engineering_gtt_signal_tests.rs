fn gtt_signal_fixture() -> (Fixture, TopologySnapshot) {
    let fixture = Fixture::valid(2);
    fs::write(
        fixture.node(0).join("properties"),
        "cpu_cores_count 128\nsimd_count 0\n",
    )
    .unwrap();
    for gpu in 1..=2 {
        fixture.replace_property(gpu, "gfx_target_version 90402", "gfx_target_version 90500");
        let path = fixture.node(gpu).join("io_links/0/properties");
        let text = fs::read_to_string(&path)
            .unwrap()
            .replace("flags 3", "flags 1");
        fs::write(path, text).unwrap();
    }
    let topology = discover_topology_at_for_target(&fixture.root, GfxTarget::Gfx950).unwrap();
    (fixture, topology)
}

#[test]
fn gtt_signal_sidecar_retains_cpu_properties_and_identity() {
    let (fixture, topology) = gtt_signal_fixture();
    let before = EngineeringGttSignalRoutesV1::observe_topology(&topology, [1001, 1002]).unwrap();
    let same = EngineeringGttSignalRoutesV1::observe_topology(&topology, [1001, 1002]).unwrap();
    assert_eq!(before, same);
    fixture.replace_property(0, "cpu_cores_count 128", "cpu_cores_count 64");
    let changed = EngineeringGttSignalRoutesV1::observe_topology(&topology, [1001, 1002]).unwrap();
    assert_ne!(before, changed);
    fs::rename(fixture.node(0), fixture.root.join("old-cpu")).unwrap();
    Fixture::write_node(&fixture.root, 0, 0, 0, 2);
    fs::write(
        fixture.node(0).join("properties"),
        "cpu_cores_count 128\nsimd_count 0\n",
    )
    .unwrap();
    fs::remove_dir_all(fixture.root.join("old-cpu")).unwrap();
    let replaced = EngineeringGttSignalRoutesV1::observe_topology(&topology, [1001, 1002]).unwrap();
    assert_ne!(before, replaced);
}

#[test]
fn gtt_signal_sidecar_rejects_unclassified_cpu_and_generation_or_roster_drift() {
    for replacement in [
        "cpu_cores_count 0\nsimd_count 0\n",
        "cpu_cores_count 128\n",
        "cpu_cores_count 128\nsimd_count 1\n",
    ] {
        let (fixture, topology) = gtt_signal_fixture();
        fs::write(fixture.node(0).join("properties"), replacement).unwrap();
        assert!(EngineeringGttSignalRoutesV1::observe_topology(&topology, [1001, 1002]).is_err());
    }
    let (fixture, topology) = gtt_signal_fixture();
    assert!(EngineeringGttSignalRoutesV1::observe_topology(&topology, [1001, 9999]).is_err());
    assert!(EngineeringGttSignalRoutesV1::observe_topology(&topology, [1001, 1001]).is_err());
    fs::write(fixture.root.join("generation_id"), "8\n").unwrap();
    assert!(EngineeringGttSignalRoutesV1::observe_topology(&topology, [1001, 1002]).is_err());
    fs::write(fixture.root.join("generation_id"), "7\n").unwrap();
    fs::remove_dir_all(fixture.node(0)).unwrap();
    assert!(EngineeringGttSignalRoutesV1::observe_topology(&topology, [1001, 1002]).is_err());
}

#[test]
fn gtt_signal_sidecar_requires_new_cpu_endpoint_route_and_exact_link_flags() {
    let (fixture, topology) = gtt_signal_fixture();
    Fixture::write_node(&fixture.root, 3, 0, 0, 2);
    fs::write(
        fixture.node(3).join("properties"),
        "cpu_cores_count 128\nsimd_count 0\n",
    )
    .unwrap();
    assert!(EngineeringGttSignalRoutesV1::observe_topology(&topology, [1001, 1002]).is_err());
    let fresh = discover_topology_at_for_target(&fixture.root, GfxTarget::Gfx950).unwrap();
    assert!(EngineeringGttSignalRoutesV1::observe_topology(&fresh, [1001, 1002]).is_err());
    fs::remove_dir_all(fixture.node(3)).unwrap();
    let path = fixture.node(2).join("io_links/0/properties");
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("flags 1", "flags 5");
    fs::write(path, text).unwrap();
    let fresh = discover_topology_at_for_target(&fixture.root, GfxTarget::Gfx950).unwrap();
    assert!(EngineeringGttSignalRoutesV1::observe_topology(&fresh, [1001, 1002]).is_err());
}
