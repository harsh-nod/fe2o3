//! No imported successes: these pin shared failure ordering and legacy debits.
use super::*;
use fe2o3_functional_proof::{
    FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2, FunctionalRefinementBindingV2, SafeReferenceKindV2,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_pliron::{
    ProductionRankedBlockV1, ProductionRankedTerminatorV1, encode_production_ranked_recipe_v1,
};

fn toolchain() -> VerusToolchainIdentityV2 {
    let d = DigestV1::from_untrusted_bytes([9; 32]);
    VerusToolchainIdentityV2::new(d, d, d, d, d).unwrap()
}
fn fixture() -> (
    Vec<u8>,
    InertFunctionalRefinementReceiptSignatureV2,
    NativeCompilerStagingCommitmentV1,
) {
    let kernel = ProductionRankedKernelV1::new(
        "no_signed_claims",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let bytes = encode_production_ranked_recipe_v1(&kernel, &mut budget)
        .unwrap()
        .0;
    (
        bytes,
        InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
            [0; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
            [0; 32],
        ),
        NativeCompilerStagingCommitmentV1 {
            receipt: [1; 32],
            effect: [2; 32],
            signer: [3; 32],
            execution: [4; 32],
            toolchain: [[5; 32]; 5],
        },
    )
}

#[test]
fn legacy_adapter_and_shared_noop_hooks_preserve_errors_work_storage_and_denials() {
    let (bytes, signature, row) = fixture();
    for malformed in [false, true] {
        let bytes = if malformed {
            b"bad".as_slice()
        } else {
            bytes.as_slice()
        };
        let run = |hooks: bool| {
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(29).unwrap();
            budget.charge_work(19).unwrap();
            assert!(budget.charge_work(usize::MAX).is_err());
            assert!(budget.reserve_storage(usize::MAX).is_err());
            let error = if hooks {
                decode_signed_recipe_with_import_hooks_v1(
                    bytes,
                    std::slice::from_ref(&signature),
                    &[row],
                    toolchain(),
                    &mut budget,
                    |_, _, _| panic!("no claim may import"),
                    |_| panic!("no proof may be retained"),
                )
            } else {
                decode_signed_recipe_v1(
                    bytes,
                    std::slice::from_ref(&signature),
                    &[row],
                    toolchain(),
                    &mut budget,
                )
            }
            .err()
            .unwrap();
            if malformed {
                assert!(matches!(error, E::RankedRecipeWire(_)));
            } else {
                assert!(matches!(error, E::Mismatch("unused signed effect receipt")));
            }
            (
                error.to_string(),
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            )
        };
        assert_eq!(run(false), run(true));
    }
}

#[test]
fn empty_or_incomplete_signed_roster_refuses_before_decode_or_hooks() {
    let (bytes, signature, row) = fixture();
    for (signatures, rows) in [
        (&[][..], &[][..]),
        (std::slice::from_ref(&signature), &[][..]),
        (&[][..], std::slice::from_ref(&row)),
    ] {
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, 0);
        assert!(matches!(
            decode_signed_recipe_with_import_hooks_v1(
                &bytes,
                signatures,
                rows,
                toolchain(),
                &mut budget,
                |_, _, _| panic!("malformed roster reached import"),
                |_| panic!("retained proof")
            ),
            Err(E::Mismatch("complete nonempty signed effect roster"))
        ));
        assert_eq!((budget.work(), budget.storage()), (0, 0));
    }
}

#[test]
fn inert_wire_never_becomes_an_imported_effect_even_with_an_explicit_binding() {
    let (_, signature, row) = fixture();
    let d = DigestV1::from_untrusted_bytes([7; 32]);
    let binding =
        FunctionalRefinementBindingV2::new(SafeReferenceKindV2::Mir, d, DigestV1::ZERO, d, d, d, d)
            .unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let error =
        ranked_source::import_ordered_effect_v1(d, binding, &signature, &row, toolchain(), |n| {
            budget.charge_work(n)
        })
        .err()
        .unwrap();
    assert!(matches!(error, E::EffectReceipt(_)));
    assert_eq!(budget.work(), signature.wire().len() + 32 + 289);
    assert_eq!(budget.storage(), 0);
}
