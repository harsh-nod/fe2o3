use super::*;

#[test]
fn all_construction_paths_leave_zero_inactive_fields() {
    let descriptors: [_; PREEXEC_MAX_DESCRIPTORS] = std::array::from_fn(|index| {
        StaticPreexecDescriptorV1::for_index(
            index,
            index as i32,
            StaticPreexecObjectIdentityV1::new(1, index as u64 + 2, 1, 0o100600),
        )
        .unwrap()
    });
    let executable = StaticPreexecObjectIdentityV1::new(1, 1, 1, 0o100500);
    assert!(!std::mem::needs_drop::<StaticPreexecManifestV1>());

    for count in MIN_DESCRIPTOR_COUNT..=PREEXEC_MAX_DESCRIPTORS {
        let active = &descriptors[..count];
        let borrowed = StaticPreexecManifestV1::from_descriptors(2, 1, executable, active).unwrap();
        let owned = StaticPreexecManifestV1::new(2, 1, executable, active.to_vec()).unwrap();
        let decoded = StaticPreexecManifestV1::decode(&borrowed.encode()).unwrap();
        for manifest in [borrowed.clone(), owned, decoded] {
            assert_eq!(manifest, borrowed);
            assert_eq!(manifest.descriptor_count, count);
            assert_eq!(manifest.descriptors(), active);
            assert!(
                manifest.descriptors[count..]
                    .iter()
                    .all(|descriptor| *descriptor == StaticPreexecDescriptorV1::ZERO)
            );
            assert!(
                manifest.encode()[descriptor_offset(count)..]
                    .iter()
                    .all(|byte| *byte == 0)
            );
            // Inactive zero objects must not participate in alias validation.
            assert_eq!(
                manifest.validate_manifest_object(&StaticPreexecObjectIdentityV1::new(0, 0, 0, 0)),
                Ok(())
            );
        }
    }
}

#[test]
fn debug_preserves_the_active_table_view() {
    let descriptors = std::array::from_fn::<_, 3, _>(|index| {
        StaticPreexecDescriptorV1::for_index(
            index,
            index as i32,
            StaticPreexecObjectIdentityV1::new(1, index as u64 + 2, 1, 0o100600),
        )
        .unwrap()
    });
    let executable = StaticPreexecObjectIdentityV1::new(1, 1, 1, 0o100500);
    let manifest =
        StaticPreexecManifestV1::from_descriptors(2, 1, executable, &descriptors).unwrap();
    assert_eq!(
        format!("{manifest:?}"),
        format!(
            "StaticPreexecManifestV1 {{ parent_pid: 2, parent_start_time: 1, executable: {executable:?}, descriptors: {descriptors:?} }}"
        )
    );
}
