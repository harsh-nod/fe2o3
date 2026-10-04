fn asrock_fixture() -> RenderFixture {
    let mut fixture = RenderFixture::valid();
    fixture.gpu.target = GfxTarget::Gfx950;
    fixture.gpu.pci_device_id = 0x75a0;
    fs::write(fixture.pci_path.join("device"), "0x75a0\n").unwrap();
    fs::write(&fixture.os_release, "5.15.160+\n").unwrap();
    fs::write(fixture.module_root.join("version"), "6.16.15\n").unwrap();
    fs::write(
        fixture.module_root.join("srcversion"),
        "9462451703604FCD7EC2365\n",
    )
    .unwrap();
    fixture
}

#[test]
fn asrock_retains_strict_same_device_uid_without_xcp_evidence() {
    let fixture = asrock_fixture();
    let observed = fixture.correlate().unwrap();
    assert_eq!(observed.unique_id(), fixture.gpu.unique_id());
    assert_eq!(fixture.correlate().unwrap(), observed);
}

#[test]
fn asrock_rejects_a_mismatch_of_either_uid_observation() {
    for change_kfd in [false, true] {
        let mut fixture = asrock_fixture();
        if change_kfd {
            fixture.gpu.unique_id += 1;
        } else {
            fs::write(fixture.pci_path.join("unique_id"), "1235\n").unwrap();
        }
        fs::create_dir(fixture.pci_path.join("xcp")).unwrap();
        fs::write(fixture.pci_path.join("xcp/xcp_metrics"), []).unwrap();
        assert!(matches!(
            fixture.correlate(),
            Err(TopologyError::RenderCorrelationMismatch {
                field: "unique_id",
                ..
            })
        ));
    }
}

#[test]
fn asrock_platform_fields_cannot_enable_a_uid_fallback() {
    for (field, value) in [
        ("kernel", "5.18.2-mi300-build-140423-ubuntu-22.04+\n"),
        ("version", "6.16.13\n"),
        ("srcversion", "975C4B2AA8AD01E2EA472C0\n"),
    ] {
        let fixture = asrock_fixture();
        let path = if field == "kernel" {
            fixture.os_release.clone()
        } else {
            fixture.module_root.join(field)
        };
        fs::write(path, value).unwrap();
        assert_eq!(fixture.correlate().unwrap().unique_id(), 0x1234);
        fs::write(fixture.pci_path.join("unique_id"), "5678\n").unwrap();
        assert!(matches!(
            fixture.correlate(),
            Err(TopologyError::RenderCorrelationMismatch {
                field: "unique_id",
                ..
            })
        ));
    }
}
