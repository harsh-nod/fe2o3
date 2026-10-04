//! Public inert transport and resource tests; no signed proof or execution fixture.

use crate::compiler_native_conditional_policy_roster_v1::{
    InertNativeConditionalPolicyRosterV1, NativeConditionalPolicyReconstructionErrorV1 as Error,
    reconstruct_inert_native_conditional_policy_roster_v1 as reconstruct,
};
use crate::compiler_native_conditional_source_packet_v2::{
    NativeConditionalSourcePacketInputV2, NativeConditionalSourceRootV2,
    encode_native_conditional_source_packet_v2,
};
use crate::{
    InertFunctionalRefinementReceiptSignatureV2 as Signature,
    NativeCompilerStagingCommitmentV1 as Staging, NativeConditionalRootPolicyV2,
};
use fe2o3_compiler_lineage::{
    MAX_NATIVE_CONDITIONAL_STORAGE_V1, NativeConditionalPolicyRootInputV1 as RootInput,
    NativeConditionalPolicyRosterInputV1 as RosterInput,
    encode_native_conditional_policy_roster_v1 as encode_roster,
    read_native_conditional_policy_roster_v1 as read_roster,
};
use fe2o3_functional_proof::{
    FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2 as SIGNATURE_BYTES,
    FunctionalRefinementBoundaryV2 as Boundary, FunctionalRefinementImportErrorV2 as ImportError,
    FunctionalRefinementImportPolicyV2 as FormulaPolicy, VerusToolchainIdentityV2 as Toolchain,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Ledger,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::{
    ProductionSourceLaunchInputV1 as Geometry, ProductionSourceLaunchRootInputV1 as Launch,
};
use fe2o3_pliron::ProductionRefinementStagingPolicyV2 as EffectPolicy;
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    convert::Infallible,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

const FLOOR: usize = 37;
const PREFIX: usize = 17;
const LIMIT: usize = MAX_NATIVE_CONDITIONAL_STORAGE_V1;
// RFC 8032 public keys only; neither fixture signs or imports a proof.
const KEYS: [[u8; 32]; 2] = [
    [
        0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07,
        0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07,
        0x51, 0x1a,
    ],
    [
        0x3d, 0x40, 0x17, 0xc3, 0xe8, 0x43, 0x89, 0x5a, 0x92, 0xb7, 0x0a, 0xa7, 0x4d, 0x1b, 0x7e,
        0xbc, 0x9c, 0x98, 0x2c, 0xcf, 0x2e, 0xc4, 0x96, 0x8c, 0xc0, 0xcd, 0x55, 0xf1, 0x2a, 0xf4,
        0x66, 0x0c,
    ],
];
const SIGNERS_FIRST: [[u8; 32]; 2] = [[51; 32], [52; 32]];
const SIGNERS_SECOND: [[u8; 32]; 1] = [[61; 32]];

fn source_packet(semantic_mir: &[u8]) -> Vec<u8> {
    let row = Staging {
        receipt: [1; 32],
        effect: [2; 32],
        signer: [3; 32],
        execution: [4; 32],
        toolchain: [[5; 32], [6; 32], [7; 32], [8; 32], [9; 32]],
    };
    let rows = [
        row,
        Staging {
            receipt: [10; 32],
            ..row
        },
    ];
    let signatures = [
        Signature::from_untrusted_parts([11; SIGNATURE_BYTES], [12; 32]),
        Signature::from_untrusted_parts([13; SIGNATURE_BYTES], [14; 32]),
    ];
    let formulas = [
        Signature::from_untrusted_parts([21; SIGNATURE_BYTES], [22; 32]),
        Signature::from_untrusted_parts([23; SIGNATURE_BYTES], [24; 32]),
    ];
    let roots = [
        NativeConditionalSourceRootV2 {
            semantic_root: 9,
            launch_rank: 1,
            launch: Launch::new(
                " logical/name ",
                [7; 32],
                Geometry::new(2, None, [0, u32::MAX, 3]),
            ),
            induction_bytes: b"induction9",
            recipe_bytes: b"recipe9",
            source_rows_bytes: b"rows9",
            ranked_ir: "diagnostic\ntext",
            cpu_input_bytes: b"cpu9",
            staging_commitments: &rows,
            effect_receipts: &signatures,
            formula_receipt: &formulas[0],
        },
        NativeConditionalSourceRootV2 {
            semantic_root: 4,
            launch_rank: 2,
            launch: Launch::new(
                "other",
                [6; 32],
                Geometry::new(3, Some([1, 2, 3]), [9, 0, 1]),
            ),
            induction_bytes: b"induction4",
            recipe_bytes: b"recipe4",
            source_rows_bytes: b"rows4",
            ranked_ir: "other text",
            cpu_input_bytes: b"cpu4",
            staging_commitments: &rows[1..],
            effect_receipts: &signatures[1..],
            formula_receipt: &formulas[1],
        },
    ];
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    encode_native_conditional_source_packet_v2(
        NativeConditionalSourcePacketInputV2 {
            semantic_mir,
            native_module: b"native/N",
            canonical_kernel_order: &[1, 0],
            roots: &roots,
        },
        &mut budget,
    )
    .unwrap()
    .0
}

fn policy_inputs() -> [RootInput<'static>; 2] {
    [
        RootInput {
            semantic_root: 9,
            kernel_binding: [7; 32],
            effect_signers: &SIGNERS_FIRST,
            effect_toolchain: [[10; 32], [11; 32], [12; 32], [13; 32], [14; 32]],
            formula_verifying_key: KEYS[0],
            formula_toolchain: [[30; 32], [31; 32], [32; 32], [33; 32], [34; 32]],
            formula_boundary: 1,
        },
        RootInput {
            semantic_root: 4,
            kernel_binding: [6; 32],
            effect_signers: &SIGNERS_SECOND,
            effect_toolchain: [[20; 32], [21; 32], [22; 32], [23; 32], [24; 32]],
            formula_verifying_key: KEYS[1],
            formula_toolchain: [[40; 32], [41; 32], [42; 32], [43; 32], [44; 32]],
            formula_boundary: 3,
        },
    ]
}

fn roster_bytes(source: &[u8], roots: &[RootInput<'_>]) -> Vec<u8> {
    encode_roster(
        RosterInput {
            source_packet: source,
            roots,
        },
        MAX_NATIVE_CONDITIONAL_STORAGE_V1,
        |_| Ok::<_, Infallible>(()),
    )
    .unwrap()
}

fn fixture() -> (Vec<u8>, Vec<u8>) {
    let source = source_packet(b"mir\0");
    let bytes = roster_bytes(&source, &policy_inputs());
    (source, bytes)
}

fn incoming(source: &[u8], bytes: &[u8]) -> usize {
    FLOOR + source.len() + bytes.len()
}

fn reseal(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/NATIVE-CONDITIONAL-POLICY-ROSTER/V1\0");
    hash.update((end as u64).to_le_bytes());
    hash.update(&bytes[..end]);
    bytes[end..].copy_from_slice(&hash.finalize());
}

fn toolchain_bytes(value: Toolchain) -> [[u8; 32]; 5] {
    [
        value.verus_executable(),
        value.verus_configuration(),
        value.solver_executable(),
        value.solver_configuration(),
        value.runtime_closure(),
    ]
    .map(|digest| *digest.as_bytes())
}

fn with_expected<R>(
    inputs: &[RootInput<'_>],
    consume: impl FnOnce(&[NativeConditionalRootPolicyV2<'_>]) -> R,
) -> R {
    let toolchain = |fields: [[u8; 32]; 5]| {
        let [verus, verus_config, solver, solver_config, runtime] =
            fields.map(DigestV1::from_untrusted_bytes);
        Toolchain::new(verus, verus_config, solver, solver_config, runtime).unwrap()
    };
    let effects: Vec<_> = inputs
        .iter()
        .map(|input| {
            EffectPolicy::new(
                input
                    .effect_signers
                    .iter()
                    .copied()
                    .map(DigestV1::from_untrusted_bytes),
                toolchain(input.effect_toolchain),
            )
            .unwrap()
        })
        .collect();
    let formulas: Vec<_> = inputs
        .iter()
        .map(|input| {
            let boundary = match input.formula_boundary {
                1 => Boundary::SafeReferenceMirToKernelMir,
                2 => Boundary::SafeReferenceSourceToKernelMir,
                3 => Boundary::SafeReferenceMirToLivePliron,
                _ => panic!("invalid expected policy fixture"),
            };
            FormulaPolicy::new(
                input.formula_verifying_key,
                toolchain(input.formula_toolchain),
                boundary,
            )
            .unwrap()
        })
        .collect();
    let policies: Vec<_> = inputs
        .iter()
        .zip(&effects)
        .zip(&formulas)
        .map(
            |((input, effects), formula)| NativeConditionalRootPolicyV2 {
                semantic_root: input.semantic_root,
                effects,
                formula,
            },
        )
        .collect();
    consume(&policies)
}

fn assert_policies(actual: &[NativeConditionalRootPolicyV2<'_>], expected: &[RootInput<'_>]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.semantic_root, expected.semantic_root);
        assert_eq!(
            actual.effects.signer_identities().len(),
            expected.effect_signers.len()
        );
        assert!(
            actual
                .effects
                .signer_identities()
                .iter()
                .zip(expected.effect_signers)
                .all(|(actual, expected)| actual.as_bytes() == expected)
        );
        assert!(
            !actual
                .effects
                .accepts_signer(DigestV1::from_untrusted_bytes([99; 32]))
        );
        assert_eq!(
            toolchain_bytes(actual.effects.toolchain()),
            expected.effect_toolchain
        );
        assert_eq!(
            actual.formula.verifying_key(),
            &expected.formula_verifying_key
        );
        assert_eq!(
            toolchain_bytes(actual.formula.toolchain()),
            expected.formula_toolchain
        );
        assert_eq!(actual.formula.boundary() as u8, expected.formula_boundary);
    }
}

#[test]
fn externally_provided_two_root_bytes_preserve_every_policy_field() {
    let (source, bytes) = fixture();
    let identity = *read_roster(&bytes, MAX_NATIVE_CONDITIONAL_STORAGE_V1, |_| {
        Ok::<_, Infallible>(())
    })
    .unwrap()
    .identity()
    .sha256();
    let floor = incoming(&source, &bytes);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    let (owner, charge) = reconstruct(&bytes, &source, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(charge.retained_storage()).unwrap();
    assert!(charge.retained_storage() > 0);
    assert_eq!(owner.required_retained_storage(), charge.retained_storage());
    assert_eq!(owner.identity(), &identity);
    assert!(!owner.grants_authority());
    // The retained policies outlive both externally supplied byte buffers.
    drop(bytes);
    drop(source);
    budget.release_storage(floor - FLOOR).unwrap();
    for _ in 0..2 {
        let result = owner
            .with_root_policies(&mut budget, |policies, _| {
                assert_policies(policies, &policy_inputs());
                [policies[0].semantic_root, policies[1].semantic_root]
            })
            .unwrap();
        assert_eq!(result, [9, 4]);
        assert_eq!(budget.storage(), FLOOR + charge.retained_storage());
        assert_eq!(owner.identity(), &identity);
    }
    drop(owner);
    budget.release_storage(charge.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn retained_owner_is_not_clone() {
    trait AmbiguousIfClone<A> {
        fn marker() {}
    }
    impl<T: ?Sized> AmbiguousIfClone<()> for T {}
    struct ImplementsClone;
    impl<T: ?Sized + Clone> AmbiguousIfClone<ImplementsClone> for T {}
    let _ = <InertNativeConditionalPolicyRosterV1 as AmbiguousIfClone<_>>::marker;
}

fn assert_refused(bytes: &[u8], source: &[u8]) -> Error {
    let floor = incoming(source, bytes);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(PREFIX).unwrap();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    assert!(budget.charge_work(usize::MAX).is_err());
    let denials = (budget.failed_work(), budget.failed_storage());
    let Err(error) = reconstruct(bytes, source, &mut budget) else {
        panic!("inconsistent inert roster returned an owner");
    };
    assert!(budget.storage() > floor);
    assert!(budget.work() > PREFIX);
    assert_eq!((budget.failed_work(), budget.failed_storage()), denials);
    error
}

#[test]
fn source_order_root_count_and_each_kernel_binding_must_match() {
    let source = source_packet(b"mir\0");
    let roots = policy_inputs();
    let reordered = [roots[1], roots[0]];
    assert!(matches!(
        assert_refused(&roster_bytes(&source, &reordered), &source),
        Error::Mismatch(_)
    ));
    assert!(matches!(
        assert_refused(&roster_bytes(&source, &roots[..1]), &source),
        Error::Mismatch(_)
    ));
    for index in 0..roots.len() {
        let mut changed = roots;
        changed[index].semantic_root = 15;
        assert!(matches!(
            assert_refused(&roster_bytes(&source, &changed), &source),
            Error::Mismatch(_)
        ));
        let mut changed = roots;
        changed[index].kernel_binding = [99; 32];
        assert!(matches!(
            assert_refused(&roster_bytes(&source, &changed), &source),
            Error::Mismatch(_)
        ));
    }
}

#[test]
fn exact_source_sha256_and_byte_length_are_required() {
    let (source, bytes) = fixture();
    let changed_hash = source_packet(b"mir\x01");
    assert_eq!(changed_hash.len(), source.len());
    assert_ne!(changed_hash, source);
    assert!(matches!(
        assert_refused(&bytes, &changed_hash),
        Error::Mismatch(_)
    ));
    let changed_length = source_packet(b"mir\0extra");
    assert_ne!(changed_length.len(), source.len());
    assert!(matches!(
        assert_refused(&bytes, &changed_length),
        Error::Mismatch(_)
    ));
    // Isolate length from the source hash: the raw SHA256 remains correct.
    let mut wrong_length = bytes.clone();
    wrong_length[24..32].copy_from_slice(&((source.len() + 1) as u64).to_le_bytes());
    reseal(&mut wrong_length);
    read_roster(&wrong_length, LIMIT, |_| Ok::<_, Infallible>(())).unwrap();
    assert!(matches!(
        assert_refused(&wrong_length, &source),
        Error::Mismatch(_)
    ));
    // The same policy rows bound to the replacement packet remain valid input.
    let rebound = roster_bytes(&changed_hash, &policy_inputs());
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = incoming(&changed_hash, &rebound);
    budget.reserve_storage(floor).unwrap();
    let (owner, charge) = reconstruct(&rebound, &changed_hash, &mut budget).unwrap();
    budget.reserve_storage(charge.retained_storage()).unwrap();
    owner
        .with_root_policies(&mut budget, |policies, _| {
            assert_policies(policies, &policy_inputs())
        })
        .unwrap();
    drop(owner);
    budget.release_storage(charge.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn matching_roster_hash_does_not_bypass_source_packet_v2_framing() {
    let mut source = source_packet(b"mir\0");
    source[16] = 1;
    assert!(matches!(
        assert_refused(&roster_bytes(&source, &policy_inputs()), &source),
        Error::Packet(_)
    ));
    let source = b"F2NSRC1\0";
    assert!(matches!(
        assert_refused(&roster_bytes(source, &policy_inputs()), source),
        Error::Packet(_)
    ));
}

#[test]
fn nonzero_weak_formula_key_passes_inert_wire_but_refuses_policy_reconstruction() {
    let source = source_packet(b"mir\0");
    let mut roots = policy_inputs();
    let mut weak = [0; 32];
    weak[0] = 1;
    let digest = |byte| DigestV1::from_untrusted_bytes([byte; 32]);
    let toolchain = Toolchain::new(digest(1), digest(2), digest(3), digest(4), digest(5)).unwrap();
    assert!(matches!(
        FormulaPolicy::new(weak, toolchain, Boundary::SafeReferenceMirToKernelMir),
        Err(ImportError::WeakVerifyingKey)
    ));
    // Place the refusal after one valid root to exercise partial-owner cleanup.
    roots[1].formula_verifying_key = weak;
    let bytes = roster_bytes(&source, &roots);
    read_roster(&bytes, MAX_NATIVE_CONDITIONAL_STORAGE_V1, |_| {
        Ok::<_, Infallible>(())
    })
    .unwrap();
    assert!(matches!(
        assert_refused(&bytes, &source),
        Error::Formula(ImportError::WeakVerifyingKey)
    ));
}

#[test]
fn exact_and_one_short_reconstruction_work_and_storage_preserve_floor() {
    let (source, bytes) = fixture();
    let floor = incoming(&source, &bytes);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.charge_work(PREFIX).unwrap();
    budget.reserve_storage(floor).unwrap();
    drop(reconstruct(&bytes, &source, &mut budget).unwrap());
    let (cost, peak) = (budget.work(), budget.peak_storage());
    assert_eq!(budget.storage(), floor);
    for (work_limit, storage_limit, refusal) in
        [(cost, peak, 0), (cost - 1, peak, 1), (cost, peak - 1, 2)]
    {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.charge_work(PREFIX).unwrap();
        budget.reserve_storage(floor).unwrap();
        let result = reconstruct(&bytes, &source, &mut budget);
        if refusal == 0 {
            drop(result.unwrap());
            assert_eq!((budget.work(), budget.peak_storage()), (cost, peak));
            assert_eq!(budget.storage(), floor);
        } else {
            assert!(result.is_err());
        }
        assert_eq!(budget.failed_work().is_some(), refusal == 1);
        assert_eq!(budget.failed_storage().is_some(), refusal == 2);
        assert!(budget.storage() >= floor);
    }
}

#[test]
fn constructor_refuses_missing_or_one_short_input_backing() {
    let (source, bytes) = fixture();
    for reserved in [0, source.len() + bytes.len() - 1] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(reserved).unwrap();
        assert!(matches!(
            reconstruct(&bytes, &source, &mut budget),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.storage(), reserved);
        assert!(budget.work() > 0);
    }
}

#[test]
fn exact_and_one_short_lending_work_and_storage_gate_callback_entry() {
    let (source, bytes) = fixture();
    let floor = incoming(&source, &bytes);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.charge_work(PREFIX).unwrap();
    budget.reserve_storage(floor).unwrap();
    let (owner, charge) = reconstruct(&bytes, &source, &mut budget).unwrap();
    budget.reserve_storage(charge.retained_storage()).unwrap();
    let retained_floor = budget.storage();
    let scratch = owner
        .with_root_policies(&mut budget, |_, budget| budget.storage() - retained_floor)
        .unwrap();
    let cost = budget.work();
    assert!(scratch > 0);
    assert_eq!(budget.storage(), retained_floor);
    drop(owner);
    budget.release_storage(charge.retained_storage()).unwrap();

    for (work_limit, available, refusal) in [
        (cost, scratch, 0),
        (cost - 1, scratch, 1),
        (cost, scratch - 1, 2),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.charge_work(PREFIX).unwrap();
        budget.reserve_storage(floor).unwrap();
        let (owner, charge) = reconstruct(&bytes, &source, &mut budget).unwrap();
        budget.reserve_storage(charge.retained_storage()).unwrap();
        let padding = LIMIT - budget.storage() - available;
        budget.reserve_storage(padding).unwrap();
        let before = budget.storage();
        let called = Cell::new(false);
        let result = owner.with_root_policies(&mut budget, |policies, _| {
            called.set(true);
            assert_policies(policies, &policy_inputs());
            77
        });
        assert_eq!(called.get(), refusal == 0);
        if refusal == 0 {
            assert_eq!(result.unwrap(), 77);
            assert_eq!(budget.work(), cost);
            assert_eq!(budget.storage(), before);
            assert_eq!(budget.peak_storage(), LIMIT);
        } else {
            assert!(result.is_err());
            assert!(budget.storage() >= before);
        }
        assert_eq!(budget.failed_work().is_some(), refusal == 1);
        assert_eq!(budget.failed_storage().is_some(), refusal == 2);
        let terminal = budget.storage();
        drop(owner);
        assert_eq!(budget.storage(), terminal);
    }
}

#[test]
fn independently_expected_policies_require_exact_order_and_all_original_fields() {
    let (source, bytes) = fixture();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(incoming(&source, &bytes)).unwrap();
    let (owner, charge) = reconstruct(&bytes, &source, &mut budget).unwrap();
    budget.reserve_storage(charge.retained_storage()).unwrap();
    let floor = budget.storage();
    let original = policy_inputs();
    with_expected(&original, |expected| {
        owner.require_expected_policies(expected, &mut budget)
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
    for expected in [&original[..1], &original[1..], &[]] {
        assert!(matches!(
            with_expected(expected, |expected| owner
                .require_expected_policies(expected, &mut budget)),
            Err(Error::Mismatch(_))
        ));
        assert_eq!(budget.storage(), floor);
    }
    let reordered = [original[1], original[0]];
    assert!(matches!(
        with_expected(&reordered, |expected| owner
            .require_expected_policies(expected, &mut budget)),
        Err(Error::Mismatch(_))
    ));
    assert_eq!(budget.storage(), floor);
    for index in 0..original.len() {
        for mutation in 0..15 {
            let mut changed = original;
            match mutation {
                0 => changed[index].semantic_root = 15,
                1 => {
                    changed[index].effect_signers = if index == 0 {
                        &[[51; 32], [99; 32]]
                    } else {
                        &[[99; 32]]
                    }
                }
                2 => changed[index].formula_verifying_key = KEYS[1 - index],
                3 => changed[index].formula_boundary = 2,
                4..=8 => changed[index].effect_toolchain[mutation - 4] = [99; 32],
                9..=13 => changed[index].formula_toolchain[mutation - 9] = [99; 32],
                14 => {
                    changed[index].effect_signers = if index == 0 {
                        &SIGNERS_FIRST[..1]
                    } else {
                        &SIGNERS_FIRST
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    with_expected(&changed, |expected| owner
                        .require_expected_policies(expected, &mut budget)),
                    Err(Error::Mismatch(_))
                ),
                "root {index}, mutation {mutation}"
            );
            assert_eq!(budget.storage(), floor);
        }
    }
    with_expected(&original, |expected| {
        owner.require_expected_policies(expected, &mut budget)
    })
    .unwrap();
    assert!(!owner.grants_authority());
    drop(owner);
    budget.release_storage(charge.retained_storage()).unwrap();
}

#[test]
fn owner_use_requires_the_complete_returned_reservation() {
    let (source, bytes) = fixture();
    let floor = incoming(&source, &bytes);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    let (owner, charge) = reconstruct(&bytes, &source, &mut budget).unwrap();
    let retained = charge.retained_storage();
    drop(source);
    drop(bytes);
    budget.release_storage(floor).unwrap();
    let called = Cell::new(false);
    assert!(matches!(
        owner.with_root_policies(&mut budget, |_, _| called.set(true)),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(!called.get());
    assert_eq!(budget.storage(), 0);
    assert!(matches!(
        with_expected(&policy_inputs(), |expected| owner
            .require_expected_policies(expected, &mut budget)),
        Err(Error::Resource(Resource::Accounting))
    ));
    budget.reserve_storage(retained - 1).unwrap();
    assert!(matches!(
        owner.with_root_policies(&mut budget, |_, _| called.set(true)),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(!called.get());
    assert_eq!(budget.storage(), retained - 1);
    assert!(matches!(
        with_expected(&policy_inputs(), |expected| owner
            .require_expected_policies(expected, &mut budget)),
        Err(Error::Resource(Resource::Accounting))
    ));
    budget.reserve_storage(1).unwrap();
    owner
        .with_root_policies(&mut budget, |policies, _| {
            called.set(true);
            assert_policies(policies, &policy_inputs());
        })
        .unwrap();
    assert!(called.get());
    with_expected(&policy_inputs(), |expected| {
        owner.require_expected_policies(expected, &mut budget)
    })
    .unwrap();
    assert_eq!(budget.storage(), retained);
    drop(owner);
    budget.release_storage(retained).unwrap();
}

#[test]
fn same_live_ledger_at_a_different_budget_address_cannot_lend_policies() {
    let (source, bytes) = fixture();
    let floor = incoming(&source, &bytes);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    let (owner, charge) = reconstruct(&bytes, &source, &mut budget).unwrap();
    budget.reserve_storage(charge.retained_storage()).unwrap();
    let account = budget.work_ledger_identity_v1();
    let original_address = &budget as *const Budget<'_> as usize;
    let mut moved = Box::new(budget);
    assert_ne!(&*moved as *const Budget<'_> as usize, original_address);
    assert!(moved.work_ledger_identity_v1() == account);
    let called = Cell::new(false);
    assert!(matches!(
        owner.with_root_policies(&mut moved, |_, _| called.set(true)),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(!called.get());
    assert!(matches!(
        with_expected(&policy_inputs(), |expected| owner
            .require_expected_policies(expected, &mut moved)),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(moved.storage(), floor + charge.retained_storage());
    drop(owner);
    moved.release_storage(charge.retained_storage()).unwrap();
    assert_eq!(moved.storage(), floor);
}

struct Provisional(Rc<Cell<usize>>);
impl Drop for Provisional {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn intact_callback_success_error_and_unwind_preserve_reservations_and_denials() {
    let (source, bytes) = fixture();
    let floor = incoming(&source, &bytes);
    for exit in 0..3 {
        let drops = Rc::new(Cell::new(0));
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(floor).unwrap();
        let (owner, charge) = reconstruct(&bytes, &source, &mut budget).unwrap();
        budget.reserve_storage(charge.retained_storage()).unwrap();
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let denials = (budget.failed_work(), budget.failed_storage());
        let work_before = budget.work();
        let callback_storage = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            owner.with_root_policies(&mut budget, |policies, budget| {
                assert_policies(policies, &policy_inputs());
                budget.reserve_storage(7).unwrap();
                budget.charge_work(13).unwrap();
                callback_storage.set(budget.storage());
                let provisional = Provisional(drops.clone());
                match exit {
                    0 => Ok(provisional),
                    1 => Err(provisional),
                    _ => panic!("policy callback unwind"),
                }
            })
        }));
        if exit == 2 {
            assert!(result.is_err());
            assert_eq!(budget.storage(), callback_storage.get());
            assert!(budget.storage() > floor + charge.retained_storage() + 7);
        } else {
            let result = result.unwrap().unwrap();
            assert_eq!(result.is_ok(), exit == 0);
            assert_eq!(drops.get(), 0);
            drop(result);
            assert_eq!(budget.storage(), floor + charge.retained_storage() + 7);
        }
        assert_eq!(drops.get(), 1);
        assert!(budget.work() >= work_before + 13);
        assert_eq!((budget.failed_work(), budget.failed_storage()), denials);
        let terminal = budget.storage();
        drop(owner);
        assert_eq!(budget.storage(), terminal);
        if exit != 2 {
            budget
                .release_storage(charge.retained_storage() + 7)
                .unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn damaged_floor_or_substituted_account_discards_callback_result_without_refunding_debt() {
    let (source, bytes) = fixture();
    let floor = incoming(&source, &bytes);
    for foreign in [false, true] {
        for unwind in [false, true] {
            let drops = Rc::new(Cell::new(0));
            let mut ledger = Ledger::new(Work::new(usize::MAX), LIMIT);
            let original_terminal = Cell::new((0, 0));
            let retained = Cell::new(0);
            let result = ledger.with_budget(|budget| {
                budget.reserve_storage(floor).unwrap();
                let (owner, charge) = reconstruct(&bytes, &source, budget).unwrap();
                retained.set(charge.retained_storage());
                budget.reserve_storage(charge.retained_storage()).unwrap();
                let account = budget.work_ledger_identity_v1();
                let mut callback_terminal = None;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    owner.with_root_policies(budget, |policies, budget| {
                        assert_policies(policies, &policy_inputs());
                        let provisional = Provisional(drops.clone());
                        if foreign {
                            original_terminal.set((budget.work(), budget.storage()));
                            let storage = budget.storage();
                            // Two fixed test meters satisfy the callback's arbitrary lifetime.
                            *budget =
                                Budget::new(Box::leak(Box::new(Work::new(usize::MAX))), LIMIT);
                            budget.reserve_storage(storage).unwrap();
                            budget.charge_work(11).unwrap();
                        } else {
                            budget.release_storage(1).unwrap();
                            original_terminal.set((budget.work(), budget.storage()));
                        }
                        callback_terminal = Some((budget.work(), budget.storage()));
                        if unwind {
                            panic!("damaged policy callback");
                        }
                        provisional
                    })
                }));
                assert_eq!(Some((budget.work(), budget.storage())), callback_terminal);
                assert_eq!(budget.work_ledger_identity_v1() == account, !foreign);
                drop(owner);
                assert_eq!(Some((budget.work(), budget.storage())), callback_terminal);
                result
            });
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(Error::Resource(Resource::Accounting))
                ));
            }
            assert_eq!(drops.get(), 1);
            assert_eq!((ledger.work(), ledger.storage()), original_terminal.get());
            assert!(ledger.storage() > floor + retained.get());
        }
    }
}
