use super::*;
use crate::inert_semantic_module_handoff_v4_tests::lineage_tests::free;
use crate::inert_semantic_module_handoff_v4_tests::{fixture, outer};
use fe2o3_compiler_lineage::*;
use fe2o3_kernel_descriptor::DescriptorWireErrorV3;
use sha2::Sha256;
use std::{
    convert::Infallible,
    error::Error,
    fmt,
    mem::size_of,
    sync::{Arc, atomic::AtomicUsize},
};

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
fn read_extent() -> usize {
    2 * size_of::<ExpandedCompilerModuleHandoffRefV4<'static>>()
        + size_of::<(&[u8], usize)>()
        + 96
        + EXPANDED_CAPSULE_READ_STORAGE_V4
        + EXPANDED_MODULE_CHILD_STORAGE_V4
        + EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1
        + EXPANDED_COMMITMENT_CHILD_STORAGE_V4
        + hash_extent()
        + 4 * size_of::<ExpandedContentIdentityV4>()
}
fn owner_extent(capacity: usize) -> usize {
    size_of::<InertSemanticCompilerModuleHandoffV4>()
        + size_of::<Vec<u8>>()
        + 2 * size_of::<AtomicUsize>()
        + capacity
}

// Successful lower-level G readers are independently admitted child oracles.
// Their operation lists are composed with the explicitly specified F boundary,
// comparison and hash charges. No F execution is used to derive its expected work.
fn capsule_roster(capsule: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let view = ExpandedCapsuleRefV4::read(capsule, EXPANDED_CAPSULE_READ_STORAGE_V4, &mut |n| {
        out.push(n);
        Ok::<(), Infallible>(())
    })
    .unwrap();
    drop(view);
    out
}
fn preflight_roster(capsule: &[u8], module: &CompilerModuleHandoffV2) -> Vec<usize> {
    let view =
        ExpandedCapsuleRefV4::read(capsule, EXPANDED_CAPSULE_READ_STORAGE_V4, &mut free).unwrap();
    let mut out = Vec::new();
    let association = ExpandedOutputAssociationRefV1::read(
        view.receipt(ExpandedReceiptSlotV4::ProofBinding)
            .canonical_preimage(),
        EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1,
        &mut |n| {
            out.push(n);
            Ok::<(), Infallible>(())
        },
    )
    .unwrap();
    assert_eq!(association.target(), view.target_text());
    drop(association);
    out.extend([
        1,
        module.symbol_manifest().canonical_bytes().len(),
        COMPILER_DESCRIPTOR_SOURCE_DOMAIN_V3.len()
            + 8
            + view
                .receipt(ExpandedReceiptSlotV4::NominalAbi)
                .canonical_preimage()
                .len(),
        4 * 40,
    ]);
    drop(view);
    out
}
fn read_roster(
    capsule: &[u8],
    module: &CompilerModuleHandoffV2,
    outer_length: usize,
) -> Vec<usize> {
    let mut out = vec![44];
    out.extend(capsule_roster(capsule));
    out.extend([1, 96]);
    out.extend(preflight_roster(capsule, module));
    out.extend([
        INERT_COMPILER_MODULE_PAIR_BINDING_DOMAIN_V4.len() + 8 + 96,
        INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DOMAIN_V4.len() + 8 + outer_length,
    ]);
    out
}
fn callback_refusal(e: ExpandedCompilerHandoffErrorV4<Refusal>) -> Refusal {
    let source = e.source().expect("outer typed source");
    assert!(source.source().is_some());
    match e {
        ExpandedCompilerHandoffErrorV4::Lineage(ExpandedPublicationErrorV4::Work(e)) => e,
        ExpandedCompilerHandoffErrorV4::Lineage(ExpandedPublicationErrorV4::Descriptor(
            DescriptorWireErrorV3::Work(e),
        )) => e,
        other => panic!("unexpected failure {other:?}"),
    }
}

#[test]
fn independent_outer_extents_include_shared_control_actual_capacity_and_live_children() {
    assert_eq!(
        EXPANDED_HANDOFF_SHARED_VECTOR_STORAGE_V4,
        size_of::<Vec<u8>>() + 2 * size_of::<AtomicUsize>()
    );
    assert_eq!(EXPANDED_HANDOFF_READ_STORAGE_V4, read_extent());
    assert_eq!(
        EXPANDED_MODULE_CHILD_STORAGE_V4,
        3 * MAX_COMPILER_FFI_ENVELOPE_BYTES_V1
            + 3 * MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1
            + 8 * 1024 * 1024
            + 2 * size_of::<CompilerModuleHandoffV2>()
    );
    assert_eq!(
        EXPANDED_COMMITMENT_CHILD_STORAGE_V4,
        3 * MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3
            + size_of::<InertFinalCompilerModuleCommitmentV3>()
    );
    for capacity in [0, 1, 137, 4 * 1024 * 1024] {
        assert_eq!(
            expanded_handoff_retained_storage_v4(capacity),
            Some(owner_extent(capacity))
        );
        assert_eq!(
            expanded_handoff_validation_storage_v4(capacity),
            Some(owner_extent(capacity) + read_extent())
        );
    }
    assert_eq!(expanded_handoff_retained_storage_v4(usize::MAX), None);
    assert_eq!(expanded_handoff_validation_storage_v4(usize::MAX), None);
    assert_eq!(expanded_handoff_length_v4(usize::MAX, 1), None);
    assert_eq!(expanded_handoff_length_v4(1, usize::MAX), None);
    assert_eq!(expanded_handoff_length_v4(0, 1), None);
}
#[test]
fn initial_outer_storage_refuses_exact_extent_before_work_or_shared_child_creation() {
    let owner = outer("gfx942:xnack-", ExpandedSourceKindV1::Direct, 3);
    let p = owner_extent(owner.bytes.capacity()) + read_extent();
    let mut meter = Meter::new(usize::MAX);
    assert!(matches!(owner.view(p - 1, &mut |n| meter.charge(n)),
        Err(ExpandedCompilerHandoffErrorV4::Lineage(ExpandedPublicationErrorV4::Storage { required, available })) if required == p && available == p - 1));
    assert_eq!(Arc::strong_count(&owner.bytes), 1);
    assert!(meter.calls.is_empty());
    let mut bytes = Vec::with_capacity(owner.canonical_bytes().len() + 131);
    bytes.extend_from_slice(owner.canonical_bytes());
    let required = owner_extent(bytes.capacity()) + read_extent();
    assert!(
        matches!(InertSemanticCompilerModuleHandoffV4::decode_owned(bytes, required - 1, &mut |n| meter.charge(n)),
        Err(ExpandedCompilerHandoffErrorV4::Lineage(ExpandedPublicationErrorV4::Storage { required: got, available })) if got == required && available == required - 1)
    );
    assert_eq!(meter.accepted, 0);
    assert!(meter.calls.is_empty());
}
#[test]
fn every_outer_work_prefix_and_failure_releases_the_shared_module_reference() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        let (f, m) = fixture(target, ExpandedSourceKindV1::Erased, 3);
        let capsule = f.capsule();
        let owner =
            InertSemanticCompilerModuleHandoffV4::new(&capsule, &m, usize::MAX, &mut free).unwrap();
        let expected = read_roster(capsule.canonical_bytes(), &m, owner.canonical_bytes().len());
        let total = expected.iter().sum();
        let p = owner_extent(owner.bytes.capacity()) + read_extent();
        let mut meter = Meter::new(total);
        let view = owner.view(p, &mut |n| meter.charge(n)).unwrap();
        assert_eq!(meter.calls, expected);
        assert_eq!(meter.accepted, total);
        assert_eq!(Arc::strong_count(&owner.bytes), 2);
        drop(view);
        assert_eq!(Arc::strong_count(&owner.bytes), 1);
        let mut prefix = 0;
        for (index, &requested) in expected.iter().enumerate() {
            if requested == 0 {
                continue;
            }
            let limit = prefix + requested - 1;
            let mut meter = Meter::new(limit);
            let error = match owner.view(p, &mut |n| meter.charge(n)) {
                Err(e) => e,
                Ok(_) => panic!("work prefix {index} admitted"),
            };
            assert_eq!(
                callback_refusal(error),
                Refusal {
                    accepted: prefix,
                    requested,
                    limit
                }
            );
            assert_eq!(meter.accepted, prefix);
            assert_eq!(meter.calls, expected[..=index]);
            assert_eq!(
                Arc::strong_count(&owner.bytes),
                1,
                "retained child after callback {index}"
            );
            prefix += requested;
        }
        assert_eq!(prefix, total);
    }
}
#[test]
fn outer_producer_pays_both_inputs_output_and_independent_second_decode() {
    let (f, m) = fixture("gfx950:xnack-", ExpandedSourceKindV1::Direct, 3);
    let capsule = f.capsule();
    let n = 44 + capsule.canonical_bytes().len() + m.canonical_bytes().len() + 96;
    let mut expected = capsule_roster(capsule.canonical_bytes());
    expected.extend(preflight_roster(capsule.canonical_bytes(), &m));
    expected.push(n);
    expected.extend(read_roster(capsule.canonical_bytes(), &m, n));
    let total = expected.iter().sum();
    let mut meter = Meter::new(total);
    let owner = InertSemanticCompilerModuleHandoffV4::new(&capsule, &m, usize::MAX, &mut |n| {
        meter.charge(n)
    })
    .unwrap();
    assert_eq!(owner.canonical_bytes().len(), n);
    assert_eq!(meter.calls, expected);
    assert_eq!(meter.accepted, total);
    assert_eq!(Arc::strong_count(&owner.bytes), 1);
    let required = owner_extent(n) + read_extent();
    let mut meter = Meter::new(usize::MAX);
    assert!(
        matches!(InertSemanticCompilerModuleHandoffV4::new(&capsule, &m, required - 1, &mut |n| meter.charge(n)),
        Err(ExpandedCompilerHandoffErrorV4::Lineage(ExpandedPublicationErrorV4::Storage { required: got, available })) if got == required && available + 1 == got)
    );
    assert!(meter.calls.is_empty());
    let last = *expected.last().unwrap();
    let mut meter = Meter::new(total - 1);
    let error =
        match InertSemanticCompilerModuleHandoffV4::new(&capsule, &m, usize::MAX, &mut |n| {
            meter.charge(n)
        }) {
            Err(e) => e,
            Ok(_) => panic!("last outer hash admitted"),
        };
    assert_eq!(
        callback_refusal(error),
        Refusal {
            accepted: total - last,
            requested: last,
            limit: total - 1
        }
    );
    assert_eq!(meter.calls, expected);
    assert_eq!(meter.accepted, total - last);
    assert_eq!(
        owner
            .view(usize::MAX, &mut free)
            .unwrap()
            .module_handoff()
            .canonical_bytes(),
        m.canonical_bytes()
    );
}
#[test]
fn callback_panic_after_module_creation_drops_the_live_shared_child() {
    let (f, m) = fixture("gfx942:xnack-", ExpandedSourceKindV1::Erased, 3);
    let capsule = f.capsule();
    let owner =
        InertSemanticCompilerModuleHandoffV4::new(&capsule, &m, usize::MAX, &mut free).unwrap();
    let expected = read_roster(capsule.canonical_bytes(), &m, owner.canonical_bytes().len());
    let mut calls = 0;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = owner.view(usize::MAX, &mut |_| -> Result<(), Infallible> {
            calls += 1;
            if calls == expected.len() {
                assert_eq!(Arc::strong_count(&owner.bytes), 2);
                panic!("intentional outer hash callback");
            }
            Ok(())
        });
    }));
    assert!(result.is_err());
    assert_eq!(calls, expected.len());
    assert_eq!(Arc::strong_count(&owner.bytes), 1);
    assert_eq!(
        owner.view(usize::MAX, &mut free).unwrap().canonical_bytes(),
        owner.canonical_bytes()
    );
}
