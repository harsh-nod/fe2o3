use super::*;
use crate::expanded_publication_v4_tests::{Fixture, free, graph, history};
use fe2o3_kernel_descriptor as kd;
use sha2::Sha256;
use std::{convert::Infallible, error::Error, fmt, mem::size_of, ops::Range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Frame,
    Descriptor,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Refusal {
    accepted: usize,
    requested: usize,
    limit: usize,
}
impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} + {} > {}", self.accepted, self.requested, self.limit)
    }
}
impl Error for Refusal {}
struct Meter {
    accepted: usize,
    limit: usize,
    calls: Vec<usize>,
}
impl Meter {
    fn new(limit: usize) -> Self {
        Self {
            accepted: 0,
            limit,
            calls: Vec::new(),
        }
    }
    fn charge(&mut self, n: usize) -> Result<(), Refusal> {
        self.calls.push(n);
        if self
            .accepted
            .checked_add(n)
            .is_none_or(|sum| sum > self.limit)
        {
            return Err(Refusal {
                accepted: self.accepted,
                requested: n,
                limit: self.limit,
            });
        }
        self.accepted += n;
        Ok(())
    }
}
fn hash_extent() -> usize {
    size_of::<Sha256>() + size_of::<ExpandedContentIdentityV4>() + 32 + 8
}
fn history_extent() -> usize {
    3 * size_of::<(&[u8], usize)>()
        + 2 * size_of::<[usize; 5]>()
        + 12 * size_of::<usize>()
        + 3 * size_of::<&[u8]>()
        + 2 * size_of::<Range<usize>>()
}
fn association_extent() -> usize {
    size_of::<ExpandedOutputAssociationRefV1<'static>>()
        + size_of::<(&[u8], usize)>()
        + 3 * MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3
        + size_of::<NativeOutputTransitionRootV1>()
        + hash_extent()
}
fn derivation_extent() -> usize {
    size_of::<ExpandedSemanticToLlvmRefV1<'static>>() + size_of::<(&[u8], usize)>() + hash_extent()
}
fn capsule_extent() -> usize {
    2 * size_of::<ExpandedCapsuleRefV4<'static>>()
        + size_of::<(&[u8], usize)>()
        + 16 * size_of::<usize>()
        + 16 * size_of::<ExpandedContentIdentityV4>()
        + EXPANDED_INVOCATION_CHILD_STORAGE_V4
        + association_extent()
        + derivation_extent()
        + history_extent()
        + kd::DESCRIPTOR_READER_SCRATCH_STORAGE_V3
        + kd::DESCRIPTOR_TABLE_VIEW_STORAGE_V3
        + hash_extent()
}

// Public lower-level nominal decoding is an independently admitted child oracle.
// No V4 capsule execution supplies the expected work or a failed-prefix total.
fn read_roster(f: &Fixture, capsule_length: usize) -> Vec<(Phase, usize)> {
    let mut out = vec![
        (Phase::Frame, 60 + 16 * 44),
        (Phase::Frame, 1),
        (Phase::Frame, f.target.len()),
    ];
    for (slot, bytes) in ExpandedReceiptSlotV4::ALL.into_iter().zip(&f.receipts) {
        out.push((Phase::Frame, slot.identity_domain().len() + 8 + bytes.len()));
    }
    out.extend(association_roster(f).into_iter().map(|n| (Phase::Frame, n)));
    out.extend([(Phase::Frame, 40); 9]);
    out.extend([
        (Phase::Frame, 48),
        (Phase::Frame, 160),
        (Phase::Frame, 48),
        (Phase::Frame, 48),
    ]);
    out.push((Phase::Frame, f.receipts[4].len()));
    let descriptor = kd::decode_device_descriptor_table_v3(&f.receipts[10], &mut |n| {
        out.push((Phase::Descriptor, n));
        Ok::<(), Infallible>(())
    })
    .unwrap();
    assert_eq!(descriptor.kernel_count(), f.roots.len());
    drop(descriptor);
    out.extend([
        (Phase::Frame, 216),
        (
            Phase::Frame,
            EXPANDED_OUTPUT_ASSOCIATION_DOMAIN_V1.len() + 8 + f.receipts[7].len(),
        ),
        (Phase::Frame, 200),
        (
            Phase::Frame,
            INERT_PRODUCTION_SEMANTIC_CAPSULE_DOMAIN_V4.len() + 8 + capsule_length,
        ),
    ]);
    out
}
fn association_roster(f: &Fixture) -> Vec<usize> {
    let mut out = vec![
        40 + 19 * 40,
        f.target.len(),
        3 * MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3,
    ];
    out.extend(std::iter::repeat_n(16, f.roots.len()));
    out.extend([
        EXPANDED_ORIGINAL_SOURCE_DOMAIN_V1.len()
            + 8
            + b"opaque-original-source-packet-not-authenticated".len(),
        EXPANDED_FINAL_CATALOG_DOMAIN_V1.len() + 8 + b"opaque-final-catalog-not-admitted".len(),
    ]);
    out
}
fn producer_roster(f: &Fixture, n: usize) -> Vec<(Phase, usize)> {
    let mut out = vec![
        (Phase::Frame, 1),
        (Phase::Frame, f.target.len()),
        (Phase::Frame, n),
    ];
    for (slot, bytes) in ExpandedReceiptSlotV4::ALL.into_iter().zip(&f.receipts) {
        out.push((Phase::Frame, slot.identity_domain().len() + 8 + bytes.len()));
    }
    out.extend(read_roster(f, n));
    out
}
fn refusal(error: ExpandedPublicationErrorV4<Refusal>) -> (Phase, Refusal) {
    assert!(error.source().is_some());
    match error {
        ExpandedPublicationErrorV4::Work(e) => (Phase::Frame, e),
        ExpandedPublicationErrorV4::Descriptor(kd::DescriptorWireErrorV3::Work(e)) => {
            (Phase::Descriptor, e)
        }
        other => panic!("unexpected refusal {other:?}"),
    }
}

#[test]
fn independent_typed_extents_include_views_headers_capacity_and_admitted_child_domains() {
    assert_eq!(EXPANDED_PUBLICATION_HASH_STORAGE_V4, hash_extent());
    assert_eq!(EXPANDED_HISTORY_DIRECTORY_STORAGE_V4, history_extent());
    assert_eq!(
        EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1,
        association_extent()
    );
    assert_eq!(
        EXPANDED_SEMANTIC_TO_LLVM_READ_STORAGE_V1,
        derivation_extent()
    );
    assert_eq!(EXPANDED_CAPSULE_READ_STORAGE_V4, capsule_extent());
    for capacity in [0, 1, 137, 4 * 1024 * 1024] {
        assert_eq!(
            expanded_capsule_retained_storage_v4(capacity),
            Some(size_of::<InertProductionSemanticCapsuleV4>() + capacity)
        );
        assert_eq!(
            expanded_capsule_validation_storage_v4(capacity),
            Some(size_of::<InertProductionSemanticCapsuleV4>() + capacity + capsule_extent())
        );
        assert_eq!(
            expanded_history_validation_storage_v4(capacity),
            Some(
                size_of::<InertExpandedHistoryReceiptV4>()
                    + capacity
                    + hash_extent().max(history_extent())
            )
        );
    }
    assert_eq!(expanded_capsule_retained_storage_v4(usize::MAX), None);
    assert_eq!(expanded_history_validation_storage_v4(usize::MAX), None);
    assert_eq!(
        expanded_output_association_encode_storage_v1(usize::MAX),
        None
    );
    assert_eq!(
        expanded_semantic_to_llvm_encode_storage_v1(usize::MAX),
        None
    );
}
#[test]
fn exact_initial_storage_refusal_precedes_any_callback_or_owned_decode_work() {
    let f = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Direct, 3, 0);
    let owner = f.capsule();
    let mut meter = Meter::new(usize::MAX);
    assert!(
        matches!(ExpandedCapsuleRefV4::read(owner.canonical_bytes(), capsule_extent() - 1, &mut |n| meter.charge(n)),
        Err(ExpandedPublicationErrorV4::Storage { required, available }) if required == capsule_extent() && available + 1 == required)
    );
    assert_eq!(meter.accepted, 0);
    assert!(meter.calls.is_empty());
    let mut bytes = Vec::with_capacity(owner.canonical_bytes().len() + 101);
    bytes.extend_from_slice(owner.canonical_bytes());
    let p = size_of::<InertProductionSemanticCapsuleV4>() + bytes.capacity() + capsule_extent();
    assert!(
        matches!(InertProductionSemanticCapsuleV4::decode_owned(bytes, p - 1, &mut |n| meter.charge(n)),
        Err(ExpandedPublicationErrorV4::Storage { required, available }) if required == p && available == p - 1)
    );
    assert!(meter.calls.is_empty());
    let required = size_of::<InertProductionSemanticCapsuleV4>()
        + expanded_capsule_length_v4(&f.input()).unwrap()
        + capsule_extent();
    assert!(
        matches!(InertProductionSemanticCapsuleV4::new(&f.input(), required - 1, &mut |n| meter.charge(n)),
        Err(ExpandedPublicationErrorV4::Storage { required: got, available }) if got == required && available + 1 == got)
    );
    assert!(meter.calls.is_empty());
}
#[test]
fn every_capsule_read_work_boundary_has_independent_exact_prefix_and_typed_phase() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        let f = Fixture::new(target, ExpandedSourceKindV1::Erased, 3, 0);
        let owner = f.capsule();
        let roster = read_roster(&f, owner.canonical_bytes().len());
        let expected: Vec<_> = roster.iter().map(|(_, n)| *n).collect();
        let total = expected.iter().sum();
        let mut successful = Meter::new(total);
        let view =
            ExpandedCapsuleRefV4::read(owner.canonical_bytes(), capsule_extent(), &mut |n| {
                successful.charge(n)
            })
            .unwrap();
        assert_eq!(view.canonical_bytes(), owner.canonical_bytes());
        drop(view);
        assert_eq!(successful.calls, expected);
        assert_eq!(successful.accepted, total);
        let mut prefix = 0;
        for (index, &(phase, requested)) in roster.iter().enumerate() {
            if requested == 0 {
                continue;
            }
            let limit = prefix + requested - 1;
            let mut meter = Meter::new(limit);
            let error = match ExpandedCapsuleRefV4::read(
                owner.canonical_bytes(),
                capsule_extent(),
                &mut |n| meter.charge(n),
            ) {
                Err(e) => e,
                Ok(_) => panic!("work boundary {index} admitted"),
            };
            assert_eq!(
                refusal(error),
                (
                    phase,
                    Refusal {
                        accepted: prefix,
                        requested,
                        limit
                    }
                )
            );
            assert_eq!(meter.accepted, prefix);
            assert_eq!(meter.calls, expected[..=index]);
            prefix += requested;
        }
        assert_eq!(prefix, total);
    }
}
#[test]
fn producer_write_hash_and_decode_work_are_composed_without_unpaid_output() {
    let f = Fixture::new("gfx950:xnack-", ExpandedSourceKindV1::Direct, 1, 0);
    let length = expanded_capsule_length_v4(&f.input()).unwrap();
    let roster = producer_roster(&f, length);
    let expected: Vec<_> = roster.iter().map(|(_, n)| *n).collect();
    let total = expected.iter().sum();
    let mut meter = Meter::new(total);
    let owner =
        InertProductionSemanticCapsuleV4::new(&f.input(), usize::MAX, &mut |n| meter.charge(n))
            .unwrap();
    assert_eq!(meter.calls, expected);
    assert_eq!(meter.accepted, total);
    assert_eq!(owner.canonical_bytes().len(), length);
    let last = *expected.last().unwrap();
    let mut denied = Meter::new(total - 1);
    let err = match InertProductionSemanticCapsuleV4::new(&f.input(), usize::MAX, &mut |n| {
        denied.charge(n)
    }) {
        Err(e) => e,
        Ok(_) => panic!("last hash admitted"),
    };
    assert_eq!(
        refusal(err),
        (
            Phase::Frame,
            Refusal {
                accepted: total - last,
                requested: last,
                limit: total - 1
            }
        )
    );
    assert_eq!(denied.accepted, total - last);
    assert_eq!(denied.calls, expected);
    // Caller-owned inputs remain usable after the attempt's output has dropped.
    assert_eq!(f.capsule().canonical_bytes(), owner.canonical_bytes());
}
#[test]
fn history_directory_range_and_identity_pay_exact_independent_callbacks_and_capacity() {
    for rounds in [1, 2, 16] {
        let original = history(&graph(4), 17, rounds);
        let mut bytes = Vec::with_capacity(original.len() + 97);
        bytes.extend_from_slice(&original);
        let capacity = bytes.capacity();
        let pointer = bytes.as_ptr();
        let required = size_of::<InertExpandedHistoryReceiptV4>()
            + capacity
            + hash_extent().max(history_extent());
        let mut expected = vec![48, 160];
        expected.extend(std::iter::repeat_n(48, rounds));
        expected.push(EXPANDED_HISTORY_RECEIPT_DOMAIN_V4.len() + 8 + bytes.len());
        let mut meter = Meter::new(expected.iter().sum());
        let owner =
            InertExpandedHistoryReceiptV4::from_vec(bytes, required, &mut |n| meter.charge(n))
                .unwrap();
        assert_eq!(owner.canonical_bytes().as_ptr(), pointer);
        assert_eq!(
            owner.retained_storage(),
            size_of::<InertExpandedHistoryReceiptV4>() + capacity
        );
        assert_eq!(meter.calls, expected);
        let range = expanded_history_final_graph_range_v4(
            owner.canonical_bytes(),
            history_extent(),
            &mut free,
        )
        .unwrap();
        assert_eq!(&owner.canonical_bytes()[range], graph(4));
        let mut meter = Meter::new(usize::MAX);
        assert!(
            matches!(expanded_history_final_graph_range_v4(owner.canonical_bytes(), history_extent() - 1, &mut |n| meter.charge(n)),
            Err(ExpandedPublicationErrorV4::Storage { required, available }) if required == history_extent() && available + 1 == required)
        );
        assert!(meter.calls.is_empty());
    }
}
#[test]
fn borrowed_root_queries_charge_and_fail_without_losing_view_lifetime() {
    let f = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Direct, 3, 0);
    let view =
        ExpandedOutputAssociationRefV1::read(&f.receipts[7], association_extent(), &mut free)
            .unwrap();
    for index in 0..3 {
        let mut meter = Meter::new(15);
        assert!(matches!(
            view.root(index, &mut |n| meter.charge(n)),
            Err(ExpandedPublicationErrorV4::Work(Refusal {
                accepted: 0,
                requested: 16,
                limit: 15
            }))
        ));
        assert_eq!(meter.calls, [16]);
        assert_eq!(
            view.root(index, &mut free).unwrap().semantic_root(),
            [7, 31, 88][index]
        );
    }
    assert_eq!(view.canonical_bytes(), f.receipts[7]);
    drop(view);
}
#[test]
fn callback_panic_unwinds_owned_attempt_without_consuming_borrowed_inputs() {
    let f = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Erased, 3, 0);
    let n = expanded_capsule_length_v4(&f.input()).unwrap();
    let roster = producer_roster(&f, n);
    // Final hashing occurs only after the output allocation and all borrowed child reads.
    let mut calls = 0;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = InertProductionSemanticCapsuleV4::new(&f.input(), usize::MAX, &mut |_| -> Result<
            (),
            Infallible,
        > {
            calls += 1;
            if calls == roster.len() {
                panic!("intentional final-hash callback");
            }
            Ok(())
        });
    }));
    assert!(result.is_err());
    assert_eq!(calls, roster.len());
    assert_eq!(f.capsule().canonical_bytes().len(), n);
}

#[test]
fn association_and_derivation_encoders_have_exact_independent_write_and_hash_boundaries() {
    let f = Fixture::new("gfx942:xnack-", ExpandedSourceKindV1::Direct, 3, 0);
    let a = ExpandedOutputAssociationRefV1::read(&f.receipts[7], association_extent(), &mut free)
        .unwrap();
    let axes = [
        ExpandedOutputAxisV1::OriginalSource,
        ExpandedOutputAxisV1::SemanticMir,
        ExpandedOutputAxisV1::Correspondence,
        ExpandedOutputAxisV1::OriginalNeutral,
        ExpandedOutputAxisV1::PreBind,
        ExpandedOutputAxisV1::BoundInput,
        ExpandedOutputAxisV1::History,
        ExpandedOutputAxisV1::FinalGraph,
        ExpandedOutputAxisV1::FinalCatalog,
        ExpandedOutputAxisV1::OriginalFormal,
        ExpandedOutputAxisV1::FinalFormal,
        ExpandedOutputAxisV1::Descriptor,
        ExpandedOutputAxisV1::SymbolManifest,
        ExpandedOutputAxisV1::NativeModule,
        ExpandedOutputAxisV1::FinalCommitment,
        ExpandedOutputAxisV1::TargetBinding,
        ExpandedOutputAxisV1::DataLayout,
        ExpandedOutputAxisV1::AmdgpuLowering,
        ExpandedOutputAxisV1::FfiEnvelope,
    ]
    .map(|axis| a.axis(axis));
    let input = ExpandedOutputAssociationInputsV1 {
        source_kind: a.source_kind(),
        target: a.target(),
        axes,
        roots: &f.roots,
        original_source: a.original_source(),
        final_catalog: a.final_catalog(),
    };
    let n = 40
        + 19 * 40
        + f.target.len()
        + 16 * f.roots.len()
        + input.original_source.len()
        + input.final_catalog.len();
    assert_eq!(expanded_output_association_length_v1(&input), Some(n));
    let p = size_of::<Vec<u8>>() + n + association_extent();
    assert_eq!(expanded_output_association_encode_storage_v1(n), Some(p));
    let mut meter = Meter::new(usize::MAX);
    assert!(
        matches!(encode_expanded_output_association_v1(&input, p - 1, &mut |n| meter.charge(n)),
        Err(ExpandedPublicationErrorV4::Storage { required, available }) if required == p && available == p - 1)
    );
    assert!(meter.calls.is_empty());
    let mut expected = vec![n];
    expected.extend(association_roster(&f));
    let total = expected.iter().sum();
    let mut meter = Meter::new(total);
    let bytes = encode_expanded_output_association_v1(&input, usize::MAX, &mut |n| meter.charge(n))
        .unwrap();
    assert_eq!(bytes, f.receipts[7]);
    assert_eq!(meter.calls, expected);
    assert_eq!(meter.accepted, total);
    let mut prefix = 0;
    for (index, &requested) in expected.iter().enumerate() {
        let limit = prefix + requested - 1;
        let mut meter = Meter::new(limit);
        let error =
            encode_expanded_output_association_v1(&input, usize::MAX, &mut |n| meter.charge(n))
                .unwrap_err();
        assert_eq!(
            refusal(error),
            (
                Phase::Frame,
                Refusal {
                    accepted: prefix,
                    requested,
                    limit
                }
            )
        );
        assert_eq!(meter.calls, expected[..=index]);
        prefix += requested;
    }
    let p = size_of::<Vec<u8>>()
        + 216
        + derivation_extent()
        + 5 * size_of::<ExpandedContentIdentityV4>();
    assert_eq!(expanded_semantic_to_llvm_encode_storage_v1(216), Some(p));
    let mut meter = Meter::new(usize::MAX);
    assert!(
        matches!(encode_expanded_semantic_to_llvm_v1(&a, p - 1, &mut |n| meter.charge(n)),
        Err(ExpandedPublicationErrorV4::Storage { required, available }) if required == p && available == p - 1)
    );
    assert!(meter.calls.is_empty());
    let expected = [
        EXPANDED_OUTPUT_ASSOCIATION_DOMAIN_V1.len() + 8 + f.receipts[7].len(),
        216,
    ];
    let total = expected.iter().sum();
    let mut meter = Meter::new(total);
    assert_eq!(
        encode_expanded_semantic_to_llvm_v1(&a, usize::MAX, &mut |n| meter.charge(n)).unwrap(),
        f.receipts[13]
    );
    assert_eq!(meter.calls, expected);
    assert_eq!(meter.accepted, total);
    let mut meter = Meter::new(total - 1);
    assert_eq!(
        refusal(
            encode_expanded_semantic_to_llvm_v1(&a, usize::MAX, &mut |n| meter.charge(n))
                .unwrap_err()
        ),
        (
            Phase::Frame,
            Refusal {
                accepted: expected[0],
                requested: 216,
                limit: total - 1
            }
        )
    );
}

#[test]
fn complete_history_transfer_and_hash_refuse_the_independently_derived_first_extent() {
    let bytes = history(&graph(4), 0, 2);
    let required = size_of::<InertExpandedHistoryReceiptV4>()
        + bytes.capacity()
        + hash_extent().max(history_extent());
    let mut meter = Meter::new(usize::MAX);
    assert!(
        matches!(InertExpandedHistoryReceiptV4::from_vec(bytes, required - 1, &mut |n| meter.charge(n)),
        Err(ExpandedPublicationErrorV4::Storage { required: got, available }) if got == required && available == required - 1)
    );
    assert!(meter.calls.is_empty());
    assert!(
        matches!(expanded_content_identity_v4(b"domain\0", b"payload", hash_extent() - 1, &mut |n| meter.charge(n)),
        Err(ExpandedPublicationErrorV4::Storage { required, available }) if required == hash_extent() && available + 1 == required)
    );
    assert!(meter.calls.is_empty());
    let requested = b"domain\0".len() + 8 + b"payload".len();
    let mut meter = Meter::new(requested - 1);
    assert_eq!(
        refusal(
            expanded_content_identity_v4(b"domain\0", b"payload", hash_extent(), &mut |n| meter
                .charge(n))
            .unwrap_err()
        ),
        (
            Phase::Frame,
            Refusal {
                accepted: 0,
                requested,
                limit: requested - 1
            }
        )
    );
}
