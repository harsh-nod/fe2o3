use super::*;
fn free(_: usize) -> Result<(), ()> {
    Ok(())
}
fn input<'a>(rows: &'a [MixedTargetWorkgroupV89<'a>]) -> MixedTargetSelectionInputsV89<'a> {
    MixedTargetSelectionInputsV89 {
        invocation: TargetLineageIdentityV3::new([1; 32], 11).unwrap(),
        semantic_mir: TargetLineageIdentityV3::new([2; 32], 22).unwrap(),
        kernel_ir: TargetLineageIdentityV3::new([3; 32], 33).unwrap(),
        descriptor_sha256: [4; 32],
        profile: ProductionAmdTargetProfileV1::Gfx942,
        workgroups: rows,
    }
}
fn wire(input: MixedTargetSelectionInputsV89<'_>) -> Vec<u8> {
    let n =
        mixed_target_selection_length_v89(input, MIXED_TARGET_SELECTION_STORAGE_V89, free).unwrap();
    let mut bytes = vec![0; n];
    encode_mixed_target_selection_v89(input, &mut bytes, MIXED_TARGET_SELECTION_STORAGE_V89, free)
        .unwrap();
    bytes
}
#[test]
fn selection_v89_singleton_and_multiple_roots_preserve_one_v18_subject_on_both_targets() {
    let rows = [
        MixedTargetWorkgroupV89 {
            kernel: "z",
            workgroup: [64, 1, 1],
        },
        MixedTargetWorkgroupV89 {
            kernel: "a",
            workgroup: [32, 2, 1],
        },
    ];
    for count in [1, 2] {
        for profile in [
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionAmdTargetProfileV1::Gfx950,
        ] {
            let expected = MixedTargetSelectionInputsV89 {
                profile,
                ..input(&rows[..count])
            };
            let bytes = wire(expected);
            let read =
                MixedTargetSelectionRefV89::read(&bytes, MIXED_TARGET_SELECTION_STORAGE_V89, free)
                    .unwrap();
            let actual = read.inputs();
            assert_eq!(actual.kernel_ir, expected.kernel_ir);
            assert_eq!(actual.invocation, expected.invocation);
            assert_eq!(actual.semantic_mir, expected.semantic_mir);
            assert_eq!(actual.descriptor_sha256, expected.descriptor_sha256);
            assert_eq!(actual.profile, expected.profile);
            assert_eq!(actual.workgroups, expected.workgroups);
            assert!(!read.establishes_refinement_proof() && !read.grants_runtime_authority());
            assert!(crate::MultiRootTargetBindingTranscriptV2::decode(&bytes).is_err());
            assert!(crate::MultiRootTargetBindingTranscriptV3::decode(&bytes).is_err());
            assert!(crate::TargetBindingTranscriptV3::decode(&bytes).is_err());
        }
    }
}
#[test]
fn selection_v89_rejects_all_truncations_old_schema_invalid_census_and_identity() {
    let rows = [
        MixedTargetWorkgroupV89 {
            kernel: "a",
            workgroup: [64, 1, 1],
        },
        MixedTargetWorkgroupV89 {
            kernel: "b",
            workgroup: [32, 2, 1],
        },
    ];
    let bytes = wire(input(&rows));
    for end in 0..bytes.len() {
        assert!(
            MixedTargetSelectionRefV89::read(
                &bytes[..end],
                MIXED_TARGET_SELECTION_STORAGE_V89,
                free
            )
            .is_err()
        );
    }
    for offset in [0usize, 8, 10, 16, 18, 20, 22] {
        let mut bad = bytes.clone();
        bad[offset] ^= 0x80;
        assert!(
            MixedTargetSelectionRefV89::read(&bad, MIXED_TARGET_SELECTION_STORAGE_V89, free)
                .is_err(),
            "{offset}"
        );
    }
    for offset in [24usize, 64, 104, 144] {
        let mut bad = bytes.clone();
        bad[offset..offset + 32].fill(0);
        assert!(
            MixedTargetSelectionRefV89::read(&bad, MIXED_TARGET_SELECTION_STORAGE_V89, free)
                .is_err(),
            "{offset}"
        );
    }
    let mut bad = bytes.clone();
    bad[176..180].copy_from_slice(&0u32.to_le_bytes());
    assert!(
        MixedTargetSelectionRefV89::read(&bad, MIXED_TARGET_SELECTION_STORAGE_V89, free).is_err()
    );
    let mut duplicate = rows;
    duplicate[1].kernel = "a";
    assert!(
        mixed_target_selection_length_v89(
            input(&duplicate),
            MIXED_TARGET_SELECTION_STORAGE_V89,
            free
        )
        .is_err()
    );
    let mut bad = bytes.clone();
    bad[HEADER + 15 + 2] = b'a';
    assert!(
        MixedTargetSelectionRefV89::read(&bad, MIXED_TARGET_SELECTION_STORAGE_V89, free).is_err()
    );
    let mut bad = bytes.clone();
    bad.push(0);
    let n = bad.len() as u32;
    bad[12..16].copy_from_slice(&n.to_le_bytes());
    assert!(
        MixedTargetSelectionRefV89::read(&bad, MIXED_TARGET_SELECTION_STORAGE_V89, free).is_err()
    );
}
#[test]
fn selection_v89_exact_and_one_short_work_and_storage_preserve_output() {
    let rows = [MixedTargetWorkgroupV89 {
        kernel: "root",
        workgroup: [64, 1, 1],
    }];
    let expected = wire(input(&rows));
    let mut measured = 0usize;
    let mut output = vec![0; expected.len()];
    encode_mixed_target_selection_v89(
        input(&rows),
        &mut output,
        MIXED_TARGET_SELECTION_STORAGE_V89,
        |n| {
            measured += n;
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    // Independent encoder oracle: census validation, fixed sort bound, complete copy.
    let expected_work = 33 + MAX_KERNELS + 4 + 5 + 2 * (MAX_NAME_BYTES + 1) + expected.len();
    assert_eq!(measured, expected_work);
    for limit in [measured, measured - 1] {
        let mut left = limit;
        let mut output = vec![0xa5; expected.len()];
        let result = encode_mixed_target_selection_v89(
            input(&rows),
            &mut output,
            MIXED_TARGET_SELECTION_STORAGE_V89,
            |n| {
                left = left.checked_sub(n).ok_or(())?;
                Ok::<(), ()>(())
            },
        );
        if limit == measured {
            result.unwrap();
            assert_eq!(left, 0);
            assert_eq!(output, expected);
        } else {
            assert!(matches!(result, Err(Error::Charge(()))));
            assert!(output.iter().all(|b| *b == 0xa5));
        }
    }
    let mut output = vec![0xa5; expected.len()];
    assert!(matches!(
        encode_mixed_target_selection_v89(
            input(&rows),
            &mut output,
            MIXED_TARGET_SELECTION_STORAGE_V89 - 1,
            free
        ),
        Err(Error::Storage)
    ));
    assert!(output.iter().all(|b| *b == 0xa5));
    assert!(matches!(
        MixedTargetSelectionRefV89::read(&expected, MIXED_TARGET_SELECTION_STORAGE_V89 - 1, free),
        Err(Error::Storage)
    ));
    let mut read_work = 0;
    MixedTargetSelectionRefV89::read(&expected, MIXED_TARGET_SELECTION_STORAGE_V89, |n| {
        read_work += n;
        Ok::<_, ()>(())
    })
    .unwrap();
    for limit in [read_work, read_work - 1] {
        let mut left = limit;
        let r =
            MixedTargetSelectionRefV89::read(&expected, MIXED_TARGET_SELECTION_STORAGE_V89, |n| {
                left = left.checked_sub(n).ok_or(())?;
                Ok::<(), ()>(())
            });
        assert_eq!(r.is_ok(), limit == read_work);
    }
}

#[test]
fn selection_v89_independent_fixed_storage_layout_oracle() {
    let views = 2 * std::mem::size_of::<MixedTargetSelectionRefV89<'static>>();
    let sorted_census = std::mem::size_of::<[usize; fe2o3_kernel_descriptor::MAX_KERNELS]>();
    let inputs = 2 * std::mem::size_of::<MixedTargetSelectionInputsV89<'static>>();
    let readers = 2 * std::mem::size_of::<Reader<'static>>();
    let identities =
        std::mem::size_of::<[TargetLineageIdentityV3; 3]>() + std::mem::size_of::<[u8; 40]>();
    let scalar_scratch = std::mem::size_of::<[u8; 32]>()
        + std::mem::size_of::<[u8; 12]>()
        + 16 * std::mem::size_of::<usize>();
    assert_eq!(
        MIXED_TARGET_SELECTION_STORAGE_V89,
        views + sorted_census + inputs + readers + identities + scalar_scratch
    );
    assert_eq!(
        MAX_MIXED_TARGET_SELECTION_BYTES_V89,
        180 + 128 * (2 + 128 + 12)
    );
}

#[test]
fn selection_v89_and_v53_cannot_be_decoded_as_each_other() {
    let rows = [MixedTargetWorkgroupV89 {
        kernel: "root",
        workgroup: [64, 1, 1],
    }];
    let current = wire(input(&rows));
    assert!(
        crate::MixedTargetSelectionRefV53::read(
            &current,
            crate::MIXED_TARGET_SELECTION_STORAGE_V53,
            free,
        )
        .is_err()
    );
    let old_rows = [crate::MixedTargetWorkgroupV53 {
        kernel: "root",
        workgroup: [64, 1, 1],
    }];
    let x = input(&rows);
    let old_input = crate::MixedTargetSelectionInputsV53 {
        invocation: x.invocation,
        semantic_mir: x.semantic_mir,
        kernel_ir: x.kernel_ir,
        descriptor_sha256: x.descriptor_sha256,
        profile: x.profile,
        workgroups: &old_rows,
    };
    let n = crate::mixed_target_selection_length_v53(
        old_input,
        crate::MIXED_TARGET_SELECTION_STORAGE_V53,
        free,
    )
    .unwrap();
    let mut old = vec![0; n];
    crate::encode_mixed_target_selection_v53(
        old_input,
        &mut old,
        crate::MIXED_TARGET_SELECTION_STORAGE_V53,
        free,
    )
    .unwrap();
    assert!(
        MixedTargetSelectionRefV89::read(&old, MIXED_TARGET_SELECTION_STORAGE_V89, free).is_err()
    );
    for use_old_magic in [false, true] {
        let mut crossed = current.clone();
        if use_old_magic {
            crossed[..8].copy_from_slice(&crate::MIXED_TARGET_SELECTION_MAGIC_V53);
        } else {
            crossed[8..10].copy_from_slice(&53u16.to_le_bytes());
        }
        assert!(
            MixedTargetSelectionRefV89::read(&crossed, MIXED_TARGET_SELECTION_STORAGE_V89, free)
                .is_err()
        );
        assert!(
            crate::MixedTargetSelectionRefV53::read(
                &crossed,
                crate::MIXED_TARGET_SELECTION_STORAGE_V53,
                free
            )
            .is_err()
        );
    }
}
