use super::*;

#[test]
fn typed_middle_end_v50_error_preserves_the_original_charge_source() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Denied(usize);
    impl std::fmt::Display for Denied {
        fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(out, "original work refusal {}", self.0)
        }
    }
    impl std::error::Error for Denied {}
    fn require_error_send_sync<T: std::error::Error + Send + Sync>() {}
    require_error_send_sync::<MixedMiddleEndErrorV50<Denied>>();

    let bytes = encoded();
    let denied = Denied(17);
    let error =
        match read_mixed_middle_end_v50(&bytes, MIXED_MIDDLE_END_WORKING_STORAGE_V50, |_| {
            Err(denied)
        }) {
            Err(error) => error,
            Ok(_) => panic!("original work refusal must propagate"),
        };
    assert_eq!(error, MixedMiddleEndErrorV50::Charge(denied));
    assert_eq!(
        std::error::Error::source(&error)
            .unwrap()
            .downcast_ref::<Denied>(),
        Some(&denied)
    );
    assert_eq!(
        error.to_string(),
        "mixed middle-end work refused: original work refusal 17"
    );
}

#[test]
fn typed_middle_end_v50_structural_errors_have_distinct_messages_without_invented_causes() {
    let cases: [(MixedMiddleEndErrorV50, &str); 8] = [
        (
            MixedMiddleEndErrorV50::FieldLength,
            "mandatory field length differs",
        ),
        (
            MixedMiddleEndErrorV50::Length,
            "complete wire extent differs",
        ),
        (
            MixedMiddleEndErrorV50::Arithmetic,
            "resource arithmetic overflow",
        ),
        (MixedMiddleEndErrorV50::Header, "canonical header differs"),
        (
            MixedMiddleEndErrorV50::Reserved,
            "reserved bytes are nonzero",
        ),
        (MixedMiddleEndErrorV50::Identity, "content identity differs"),
        (
            MixedMiddleEndErrorV50::Storage,
            "prepaid scratch extent differs",
        ),
        (
            MixedMiddleEndErrorV50::Binding,
            "exact capsule inputs differ",
        ),
    ];
    for (error, suffix) in cases {
        assert_eq!(error.to_string(), format!("mixed middle-end {suffix}"));
        assert!(std::error::Error::source(&error).is_none());
    }
}

fn input() -> MixedMiddleEndInputV50<'static> {
    // Framing-only payloads deliberately are not admitted source or proof data.
    MixedMiddleEndInputV50 {
        semantic_mir: b"inert semantic",
        source_ssa_identity: &[7; 32],
        original: b"inert original",
        prefix: b"inert prefix",
        licm: b"inert LICM output",
        forwarded: b"inert final",
        prefix_witness: b"inert full witness",
        generated_source: b"inert generated source",
        execution_receipt: b"inert receipt",
    }
}
fn encoded() -> Vec<u8> {
    let mut bytes = vec![
        0;
        MixedMiddleEndLayoutV50::new::<Infallible>(input())
            .unwrap()
            .encoded_len()
    ];
    encode_mixed_middle_end_v50(
        input(),
        &mut bytes,
        MIXED_MIDDLE_END_WORKING_STORAGE_V50,
        |_| Ok::<_, Infallible>(()),
    )
    .unwrap();
    bytes
}

#[test]
fn typed_middle_end_v50_round_trip_retains_every_exact_field_without_authority() {
    let bytes = encoded();
    let decoded = read_mixed_middle_end_v50(&bytes, MIXED_MIDDLE_END_WORKING_STORAGE_V50, |_| {
        Ok::<_, Infallible>(())
    })
    .unwrap();
    assert_eq!(decoded.canonical_bytes(), bytes);
    assert_eq!(decoded.identity().byte_len(), bytes.len() as u64);
    assert_eq!(decoded.input().fields(), input().fields());
    assert!(!decoded.grants_authority());
    decoded
        .check_exact(input(), |_| Ok::<_, Infallible>(()))
        .unwrap();
    assert_eq!(encoded(), bytes, "canonical encoding is deterministic");
}

fn inert_receipts(
    semantic: &[u8],
    middle: &[u8],
    final_graph: &[u8],
) -> OrderedInertSemanticLineageReceiptsV3 {
    use crate::*;
    // These other stages are explicitly inert framing data, not proof fixtures.
    macro_rules! stage {
        ($ty:ident, $bytes:expr) => {
            $ty::from_canonical_preimage($bytes.to_vec()).unwrap()
        };
    }
    OrderedInertSemanticLineageReceiptsV3::new(
        stage!(InertRustcIdentityInventoryReceiptV3, b"inert inventory"),
        stage!(InertRustcPreflightPlanReceiptV3, b"inert preflight"),
        stage!(InertCanonicalSemanticMirReceiptV3, semantic),
        stage!(InertMiddleEndReceiptV3, middle),
        stage!(InertKernelIrReceiptV3, final_graph),
        stage!(
            InertMirToKirCorrespondenceReceiptV3,
            b"inert correspondence"
        ),
        stage!(InertFormalMemoryReceiptV3, b"inert memory"),
        stage!(InertProofBindingReceiptV3, b"inert proof binding"),
        stage!(InertTargetBindingReceiptV3, b"inert target"),
        stage!(InertDataLayoutReceiptV3, b"inert layout"),
        stage!(InertAbiReceiptV3, b"inert ABI"),
        stage!(InertExportManifestReceiptV3, b"inert exports"),
        stage!(InertAmdgpuLoweringReceiptV3, b"inert target lowering"),
        stage!(InertSemanticToLlvmReceiptV3, b"inert LLVM relation"),
        stage!(InertFinalCompilerModuleCommitmentReceiptV3, b"inert module"),
    )
}

#[test]
fn typed_middle_end_v50_requires_exact_existing_capsule_source_stage_and_final_graph() {
    let bytes = encoded();
    let decoded = read_mixed_middle_end_v50(&bytes, MIXED_MIDDLE_END_WORKING_STORAGE_V50, |_| {
        Ok::<_, Infallible>(())
    })
    .unwrap();
    let input = input();
    let receipts = inert_receipts(input.semantic_mir, &bytes, input.forwarded);
    let expected = input.semantic_mir.len() + bytes.len() + input.forwarded.len();
    let mut work = 0;
    decoded
        .check_capsule(&receipts, |n| {
            work += n;
            Ok::<_, usize>(())
        })
        .unwrap();
    assert_eq!(work, expected);
    let mut work = 0;
    assert_eq!(
        decoded.check_capsule(&receipts, |n| {
            work += n;
            if work <= expected - 1 {
                Ok(())
            } else {
                Err(work)
            }
        }),
        Err(Error::Charge(expected))
    );
    for field in 0..3 {
        let mut fields = [input.semantic_mir, bytes.as_slice(), input.forwarded];
        let mut changed = fields[field].to_vec();
        changed[0] ^= 1;
        fields[field] = &changed;
        // Each outer receipt is freshly sealed, so digest validity cannot replace
        // the exact retained source/stage/output join.
        let receipts = inert_receipts(fields[0], fields[1], fields[2]);
        assert_eq!(
            decoded.check_capsule(&receipts, |_| Ok::<_, Infallible>(())),
            Err(Error::Binding),
            "changed capsule field {field}"
        );
    }
    assert!(!decoded.grants_authority());
}

#[test]
fn typed_middle_end_v50_rejects_tampering_splicing_trailing_and_legacy_tags() {
    let bytes = encoded();
    for index in 0..bytes.len() {
        let mut changed = bytes.clone();
        changed[index] ^= 1;
        assert!(
            read_mixed_middle_end_v50(&changed, MIXED_MIDDLE_END_WORKING_STORAGE_V50, |_| Ok::<
                _,
                Infallible,
            >(
                ()
            ))
            .is_err(),
            "unsealed mutation at {index}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(
        read_mixed_middle_end_v50(&trailing, MIXED_MIDDLE_END_WORKING_STORAGE_V50, |_| Ok::<
            _,
            Infallible,
        >(
            ()
        ))
        .is_err()
    );
    for field in 0..9 {
        let mut payload = input().fields()[field].to_vec();
        payload[0] ^= 1;
        let mut fields = input().fields();
        fields[field] = &payload;
        let changed_input = MixedMiddleEndInputV50::from_fields(fields);
        let mut changed = bytes.clone();
        encode_mixed_middle_end_v50(
            changed_input,
            &mut changed,
            MIXED_MIDDLE_END_WORKING_STORAGE_V50,
            |_| Ok::<_, Infallible>(()),
        )
        .unwrap();
        let decoded =
            read_mixed_middle_end_v50(&changed, MIXED_MIDDLE_END_WORKING_STORAGE_V50, |_| {
                Ok::<_, Infallible>(())
            })
            .unwrap();
        assert_eq!(
            decoded.check_exact(input(), |_| Ok::<_, Infallible>(())),
            Err(Error::Binding),
            "resealed field {field} must not join the original"
        );
    }
    for tag in [3_u16, 10, 18, 28, 29] {
        let mut changed = bytes.clone();
        changed[8..10].copy_from_slice(&tag.to_le_bytes());
        assert!(matches!(
            read_mixed_middle_end_v50(&changed, MIXED_MIDDLE_END_WORKING_STORAGE_V50, |_| Ok::<
                _,
                Infallible,
            >(
                ()
            )),
            Err(Error::Header)
        ));
    }
}

#[test]
fn typed_middle_end_v50_exact_work_storage_and_aggregate_bounds() {
    let fixed_scratch = 2 * size_of::<MixedMiddleEndLayoutV50>()
        + 2 * size_of::<MixedMiddleEndRefV50<'static>>()
        + 2 * size_of::<MixedMiddleEndInputV50<'static>>()
        + 34 * size_of::<Range<usize>>()
        + 18 * size_of::<&[u8]>()
        + 36 * size_of::<usize>()
        + 2 * size_of::<Sha256>()
        + 2 * pair::HEADER
        + 2 * size_of::<pair::Layout>()
        + 2 * size_of::<pair::Identity>()
        + 2 * size_of::<Option<pair::Identity>>()
        + 32
        + 8;
    assert_eq!(MIXED_MIDDLE_END_WORKING_STORAGE_V50, fixed_scratch);
    let fields = input().fields();
    let mut n = fields.map(<[u8]>::len);
    let overhead = 8 * pair::OVERHEAD;
    let others = n.iter().sum::<usize>() - n[0];
    n[0] = MAX - overhead - others;
    assert_eq!(
        MixedMiddleEndLayoutV50::from_lengths::<Infallible>(n)
            .unwrap()
            .encoded_len(),
        MAX
    );
    n[0] += 1;
    assert!(matches!(
        MixedMiddleEndLayoutV50::from_lengths::<Infallible>(n),
        Err(Error::Length)
    ));
    for i in 0..9 {
        let mut n = fields.map(<[u8]>::len);
        n[i] = 0;
        assert_eq!(
            MixedMiddleEndLayoutV50::from_lengths::<Infallible>(n),
            Err(Error::FieldLength)
        );
        n[i] = MAXIMA[i] + 1;
        assert_eq!(
            MixedMiddleEndLayoutV50::from_lengths::<Infallible>(n),
            Err(Error::FieldLength)
        );
    }
    let mut bytes = encoded();
    let mut work = 0usize;
    encode_mixed_middle_end_v50(
        input(),
        &mut bytes,
        MIXED_MIDDLE_END_WORKING_STORAGE_V50,
        |n| {
            work += n;
            Ok::<_, usize>(())
        },
    )
    .unwrap();
    // Independent oracle: all payload copies plus eight fixed pair seal/hash visits.
    let layout = MixedMiddleEndLayoutV50::new::<Infallible>(input()).unwrap();
    let expected = fields.iter().map(|f| f.len()).sum::<usize>()
        + (0..8)
            .map(|i| {
                layout.nodes[i].encoded_len() - 32
                    + POLICIES[i].domain.len()
                    + 8
                    + 128
                    + 2 * pair::HEADER
                    + 32
            })
            .sum::<usize>();
    assert_eq!(work, expected);
    let before = bytes.clone();
    assert_eq!(
        encode_mixed_middle_end_v50(
            input(),
            &mut bytes,
            MIXED_MIDDLE_END_WORKING_STORAGE_V50,
            |n| if n <= work - 1 { Ok(()) } else { Err(n) }
        ),
        Err(Error::Charge(work))
    );
    assert_eq!(bytes, before, "work refusal must precede output mutation");
    assert_eq!(
        encode_mixed_middle_end_v50(
            input(),
            &mut bytes,
            MIXED_MIDDLE_END_WORKING_STORAGE_V50 - 1,
            |_| Ok::<_, usize>(())
        ),
        Err(Error::Storage)
    );
    let mut read_work = 0;
    read_mixed_middle_end_v50(&bytes, MIXED_MIDDLE_END_WORKING_STORAGE_V50, |n| {
        read_work += n;
        Ok::<_, usize>(())
    })
    .unwrap();
    assert!(matches!(
        read_mixed_middle_end_v50(&bytes, MIXED_MIDDLE_END_WORKING_STORAGE_V50 - 1, |_| Ok::<
            _,
            usize,
        >(
            ()
        )),
        Err(Error::Storage)
    ));
    assert!(matches!(
        read_mixed_middle_end_v50(&bytes, STORAGE_MAX + 1, |_| Ok::<_, usize>(())),
        Err(Error::Storage)
    ));
    let mut actual = 0;
    assert!(
        matches!(read_mixed_middle_end_v50(&bytes, MIXED_MIDDLE_END_WORKING_STORAGE_V50,
        |n| { actual += n; if actual <= read_work - 1 { Ok(()) } else { Err(actual) } }),
        Err(Error::Charge(n)) if n == read_work)
    );
}

#[test]
fn typed_middle_end_v50_rejects_licm_as_final_and_legacy_receipt_substitution() {
    let bytes = encoded();
    let decoded = read_mixed_middle_end_v50(&bytes, MIXED_MIDDLE_END_WORKING_STORAGE_V50, |_| {
        Ok::<_, Infallible>(())
    })
    .unwrap();
    let input = input();
    let wrong_capsule = inert_receipts(input.semantic_mir, &bytes, input.licm);
    assert_eq!(
        decoded.check_capsule(&wrong_capsule, |_| Ok::<_, Infallible>(())),
        Err(Error::Binding)
    );
    let swapped = MixedMiddleEndInputV50 {
        licm: input.forwarded,
        forwarded: input.licm,
        ..input
    };
    assert_eq!(
        decoded.check_exact(swapped, |_| Ok::<_, Infallible>(())),
        Err(Error::Binding)
    );
    let old = crate::MixedMiddleEndInputV29 {
        semantic_mir: input.semantic_mir,
        source_ssa_identity: input.source_ssa_identity,
        original: input.original,
        prefix: input.prefix,
        final_graph: input.licm,
        prefix_witness: input.prefix_witness,
        generated_source: input.generated_source,
        execution_receipt: input.execution_receipt,
    };
    let mut legacy = vec![
        0;
        crate::MixedMiddleEndLayoutV29::new::<Infallible>(old)
            .unwrap()
            .encoded_len()
    ];
    crate::encode_mixed_middle_end_v29(
        old,
        &mut legacy,
        crate::MIXED_MIDDLE_END_WORKING_STORAGE_V29,
        |_| Ok::<_, Infallible>(()),
    )
    .unwrap();
    assert!(matches!(
        read_mixed_middle_end_v50(&legacy, MIXED_MIDDLE_END_WORKING_STORAGE_V50, |_| Ok::<
            _,
            Infallible,
        >(
            ()
        )),
        Err(Error::Header)
    ));
    assert!(matches!(
        crate::read_mixed_middle_end_v29(
            &bytes,
            crate::MIXED_MIDDLE_END_WORKING_STORAGE_V29,
            |_| Ok::<_, Infallible>(())
        ),
        Err(crate::MixedMiddleEndErrorV29::Header)
    ));
}

#[test]
fn typed_middle_end_v50_authenticates_every_node_even_when_ancestors_are_resealed() {
    let bytes = encoded();
    let layout = MixedMiddleEndLayoutV50::new::<Infallible>(input()).unwrap();
    let ranges = layout.ranges();
    assert_eq!(&bytes[..8], b"F2MC50R\0");
    assert_eq!(&bytes[8..10], &50_u16.to_le_bytes());
    for length in 0..bytes.len() {
        assert!(
            read_mixed_middle_end_v50(
                &bytes[..length],
                MIXED_MIDDLE_END_WORKING_STORAGE_V50,
                |_| Ok::<_, Infallible>(())
            )
            .is_err()
        );
    }
    for node in 0..8 {
        for offset in [0, 8, 12, 40] {
            let mut changed = bytes.clone();
            changed[ranges[node].start + offset] ^= 1;
            let mut child = node;
            while child != 0 {
                let parent = CHILDREN
                    .iter()
                    .position(|children| children.contains(&child))
                    .unwrap();
                pair::seal(
                    &POLICIES[parent],
                    layout.nodes[parent],
                    &mut changed[ranges[parent].clone()],
                    MIXED_MIDDLE_END_WORKING_STORAGE_V50,
                    |_| Ok::<_, Infallible>(()),
                )
                .unwrap();
                child = parent;
            }
            assert!(
                read_mixed_middle_end_v50(
                    &changed,
                    MIXED_MIDDLE_END_WORKING_STORAGE_V50,
                    |_| Ok::<_, Infallible>(())
                )
                .is_err(),
                "node {node} header offset {offset}"
            );
        }
    }
}

#[test]
fn typed_middle_end_v50_callback_failure_and_drop_panic_precede_mutation() {
    let mut bytes = encoded();
    let before = bytes.clone();
    struct Captured;
    impl Drop for Captured {
        fn drop(&mut self) {
            panic!("injected captured destructor failure");
        }
    }
    let captured = Captured;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        encode_mixed_middle_end_v50(
            input(),
            &mut bytes,
            MIXED_MIDDLE_END_WORKING_STORAGE_V50,
            move |_| {
                let _ = &captured;
                Ok::<_, Infallible>(())
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(bytes, before);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        encode_mixed_middle_end_v50(
            input(),
            &mut bytes,
            MIXED_MIDDLE_END_WORKING_STORAGE_V50,
            |_| -> Result<(), Infallible> { panic!("injected charge panic") },
        )
    }));
    assert!(result.is_err());
    assert_eq!(bytes, before);
}
