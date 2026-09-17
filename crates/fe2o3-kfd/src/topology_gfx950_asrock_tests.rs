fn asrock_fixture() -> RenderFixture {
    let f = fixture();
    fs::write(&f.os_release, "5.15.160+\n").unwrap();
    fs::write(f.module_root.join("version"), "6.16.15\n").unwrap();
    fs::write(
        f.module_root.join("srcversion"),
        "9462451703604FCD7EC2365\n",
    )
    .unwrap();
    fs::write(f.pci_path.join("unique_id"), "1234\n").unwrap();
    // This profile uses exact board-UID equality, not the old XCP exception.
    fs::remove_file(f.pci_path.join("xcp/xcp_metrics")).unwrap();
    fs::remove_dir(f.pci_path.join("xcp")).unwrap();
    f
}

#[test]
fn asrock_retains_strict_same_device_uid_without_xcp_evidence() {
    let f = asrock_fixture();
    let observed = correlate(&f).unwrap();
    assert_eq!(observed.unique_id(), 0x1234);
    assert_eq!(observed.kfd_unique_id(), 0x1234);
    assert_eq!(
        observed.identity_correlation(),
        RenderIdentityCorrelationV1::SameDeviceUid
    );
    assert_eq!(correlate(&f).unwrap(), observed);
}

#[test]
fn asrock_rejects_a_mismatch_of_either_uid_observation() {
    for change_kfd in [false, true] {
        let mut f = asrock_fixture();
        if change_kfd {
            f.gpu.unique_id += 1;
        } else {
            fs::write(f.pci_path.join("unique_id"), "1235\n").unwrap();
        }
        // Supplying XCP evidence cannot turn a UID mismatch into a fallback.
        fs::create_dir(f.pci_path.join("xcp")).unwrap();
        fs::write(f.pci_path.join("xcp/xcp_metrics"), []).unwrap();
        assert!(correlate(&f).is_err());
    }
}

#[test]
fn asrock_platform_fields_do_not_borrow_the_old_xcp_exception() {
    for (field, value) in [
        ("kernel", "5.18.2-mi300-build-140423-ubuntu-22.04+\n"),
        ("version", "6.16.13\n"),
        ("srcversion", "975C4B2AA8AD01E2EA472C0\n"),
    ] {
        let f = asrock_fixture();
        let path = if field == "kernel" {
            f.os_release.clone()
        } else {
            f.module_root.join(field)
        };
        fs::write(path, value).unwrap();
        let observed = correlate(&f).unwrap();
        assert_eq!(
            observed.identity_correlation(),
            RenderIdentityCorrelationV1::SameDeviceUid
        );
        fs::write(f.pci_path.join("unique_id"), "5678\n").unwrap();
        assert!(correlate(&f).is_err());
    }
}
