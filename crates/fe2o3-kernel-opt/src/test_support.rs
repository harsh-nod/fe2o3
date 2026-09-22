//! Shared test-only producer chain; no source, compiler, artifact or launch authority.
use crate::*;
use crate::{
    CanonicalRefinedForwardingHistoryInputsV1 as Inputs,
    CanonicalRefinedForwardingHistoryLimitsV1 as Limits, POLICY8_COMMUTATIVE_PASS_NAME_V1 as PASS,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirOperationCoordinateV1 as Site,
    InertCanonicalKirTransitionGraphIdentityV1 as Identity,
    InertCanonicalKirTransitionReceiptV1 as Transition, Module,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_pliron::prepare_owned_commutative_bitwise_continuation_v1;
const WORK: usize = 1_000_000_000;
const STORAGE: usize = MAX_REFINED_FORWARDING_HISTORY_STORAGE_V1;

/// Run actual B-through-F producers on supplied B. Panics are fixture failures.
pub fn with_refined_forwarding_history_module_v1<T>(
    module: &Module,
    expected_p8_pairs: usize,
    run: impl FnOnce(Inputs<'_>, usize) -> T,
) -> T {
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    let (input, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut b).unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    let p5 = optimize_checked_canonical_kernel_ir_policy5_v1(&input, &mut b).unwrap();
    b.reserve_storage(p5.retained_storage()).unwrap();
    let p6 = continue_checked_canonical_kernel_ir_policy6_v1(&input, p5, &mut b).unwrap();
    b.reserve_storage(p6.retained_storage()).unwrap();
    let p4 = encode_checked_canonical_policy4_execution_receipt_v1(
        &input,
        p6.intermediate_policy5().intermediate_policy4(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(p4.storage().retained_storage()).unwrap();
    let (transition, storage) = Transition::from_candidate_with_budget(
        p6.intermediate_policy5().owner().canonical().identity(),
        p6.owner().canonical().identity(),
        p6.continuation().occurrences().candidate(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(storage.retained_storage()).unwrap();
    let p7 = prepare_owned_redundant_store_continuation_v1(p6.owner(), &mut b).unwrap();
    b.reserve_storage(p7.retained_storage()).unwrap();
    let mut record = vec![0; 384];
    record[..16].copy_from_slice(b"F2P7EX1\0\x01\0\x07\0\0\x01\0\0");
    record[16..272].copy_from_slice(p6.execution().canonical_bytes());
    for (offset, owner) in [(272, p6.owner()), (312, p7.output())] {
        let id = owner.canonical().identity();
        record[offset..offset + 32].copy_from_slice(id.digest());
        record[offset + 32..offset + 40].copy_from_slice(&id.canonical_length().to_le_bytes());
    }
    let mut coordinate = |s: Site| {
        for n in [s.block.function.0, s.block.block, s.operation] {
            record.extend(n.to_le_bytes());
        }
    };
    for row in p7.rows() {
        coordinate(row.anchor);
        coordinate(row.removed);
    }
    for row in p7.retained_operations() {
        coordinate(row.input);
        coordinate(row.output);
    }
    for (offset, n) in [
        (352, p7.rows().len()),
        (360, p7.retained_operations().len()),
        (368, 1),
        (376, record.len()),
    ] {
        record[offset..offset + 8].copy_from_slice(&(n as u64).to_le_bytes());
    }
    b.reserve_storage(record.capacity()).unwrap();
    let p8 = prepare_owned_commutative_bitwise_continuation_v1(p7.output(), &mut b).unwrap();
    b.reserve_storage(p8.retained_storage()).unwrap();
    assert_eq!(p8.proved_pairs(), expected_p8_pairs);
    let p = prepare_owned_private_cell_promotion_v1(p8.output(), &mut b).unwrap();
    b.reserve_storage(p.retained_storage()).unwrap();
    let h = prepare_owned_loop_preheaders_v1(p.output(), &mut b).unwrap();
    b.reserve_storage(h.retained_storage()).unwrap();
    let l = prepare_owned_licm_v1(h.output(), &mut b).unwrap();
    b.reserve_storage(l.retained_storage()).unwrap();
    let r = prepare_owned_induction_refinement_v1(l.output(), Default::default(), &mut b).unwrap();
    b.reserve_storage(r.retained_storage()).unwrap();
    let f =
        prepare_owned_cross_block_forwarding_v1(r.output(), Default::default(), &mut b).unwrap();
    b.reserve_storage(f.retained_storage()).unwrap();
    let p5 = p6.intermediate_policy5();
    let p4owner = p5.intermediate_policy4();
    let prefix = CanonicalPolicy8SemanticInputsV1 {
        prefix: CanonicalPolicy7SemanticInputsV1 {
            prefix: CanonicalPolicy6SemanticInputsV1 {
                prefix: CanonicalPolicy5SemanticInputsV1 {
                    input: &input,
                    intermediate: p4owner.intermediate_policy3().owner(),
                    stored: p4owner.owner(),
                    output: p5.owner(),
                    policy4_wire: p4.canonical_bytes(),
                    policy5_record: p5.execution().canonical_bytes(),
                    load_rows: p5.load_forwarding_rows(),
                },
                output: p6.owner(),
                continuation: CanonicalPolicy6ContinuationClaimsV1 {
                    composition_record: p6.execution().canonical_bytes(),
                    integer_record: p6.continuation().execution().canonical_bytes(),
                    transition_wire: transition.canonical_bytes(),
                },
            },
            output: p7.output(),
            continuation: CanonicalPolicy7ContinuationClaimsV1 {
                execution_record: &record,
                deletion_rows: p7.rows(),
                retained_operations: p7.retained_operations(),
            },
        },
        output: p8.output(),
        continuation: CanonicalPolicy8ContinuationClaimsV1 {
            pass_name: PASS,
            input: Identity::from_verified(p7.output().canonical().identity()),
            output: Identity::from_verified(p8.output().canonical().identity()),
            occurrences: p8.occurrences().candidate(),
        },
    };
    run(
        Inputs {
            prefix,
            promoted: p.output(),
            selected_allocations: p.selected_allocations(),
            promotion_origins: p.origins(),
            preheaders: h.output(),
            preheader_rows: h.preheaders(),
            licm: l.output(),
            licm_origins: l.origins(),
            refined: r.output(),
            refinement_origins: r.origins(),
            output: f.output(),
            forwarding_origins: f.origins(),
            limits: Limits {
                refinement: r.limits(),
                forwarding: f.limits(),
            },
        },
        b.storage(),
    )
}
