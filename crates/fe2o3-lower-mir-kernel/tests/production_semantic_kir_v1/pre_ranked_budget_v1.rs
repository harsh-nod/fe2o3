use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, CanonicalKernelIrReplayAdmissionErrorV12,
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    CanonicalKernelIrWorkBudgetV1, Function, FunctionId, Kernel, KernelIrDecodeError,
    MeteredVerifiedCanonicalKernelIrErrorV12, VerifiedCanonicalKernelIrModuleV12,
    VerifiedCanonicalKernelIrV12,
};
use fe2o3_lower_mir_kernel::{
    ProductionPreRankedKirErrorV1, ProductionPreRankedKirOwnerV1, ProductionSourceLaunchInputV1,
    ProductionSourceLaunchRootInputV1, ProductionSourceLaunchRosterV1,
    SemanticKirAssertOriginErrorV1, SemanticKirAssertOriginStorageV1,
};
use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1};

const WORK_PREFIX: usize = 17;
const STORAGE_PREFIX: usize = 29;
const WIRE: usize = 190;
const SCHEMA_TOKENS: usize = 35;
const IDENTIFIER_BYTES: usize = 81 + 1 + 1 + 1;
// One function and kernel: census2 + (insertion1 + lookup2)*key_width2 + handle1.
const ROLE_WORK: usize = 2 + (1 + 2) * 2 + 1;
const ENCODE_WORK: usize = SCHEMA_TOKENS + (SCHEMA_TOKENS + ROLE_WORK + WIRE + 4);
const BEFORE_INVERSE_SCRATCH: usize =
    ENCODE_WORK + WIRE + 2 * IDENTIFIER_BYTES + ROLE_WORK + SCHEMA_TOKENS;
const INVERSE_COMPARE_WORK: usize = WIRE + 4 + SCHEMA_TOKENS + 1 + ROLE_WORK;

// One-block/zero-edge CFG: indexed rows29, reachability7, RPO14,
// dominators9, intervals24, reducibility20. Singleton numeric sorts do no work.
const CFG_WORK: usize = 29 + 7 + 14 + 9 + 24 + 20;
// Module index census/fill4, module location5, five roster scans5, referenced
// entry query5. Header: two locations10, reserved-prefix checks4, role/arity2.
// Function: CFG, one block index + three scans + end check5, body validation15.
// Kernel: two locations10, geometry2, entry lookup5, reachability scratch/scan6.
const VERIFIER_WORK: usize =
    (4 + 5 + 5 + 5) + (10 + 4 + 2) + (CFG_WORK + 5 + 15) + (10 + 2 + 5 + 6);
const BEFORE_HASH: usize = BEFORE_INVERSE_SCRATCH + INVERSE_COMPARE_WORK + VERIFIER_WORK + WIRE;
const HASH_WORK: usize = 4 + 39 + 2 + 8 + WIRE;
const COMPLETE_WORK: usize = BEFORE_HASH + HASH_WORK;
// Sealing: identity1; function/block index6; root roster3; function association9;
// borrowed source-span index3; per-function seen counts3; exact span checks5;
// function coverage1; assertion coverage1. No assertions means no definition scan.
const ORIGIN_WORK: usize = 1 + 6 + 3 + 9 + 3 + 3 + 5 + 1 + 1;
// Retained helper absence: entry4 + source row2 + physical row2 + absence2.
// Sealing the immutable graph/origin/helper subtotal pays two checked additions.
const HELPER_WORK: usize = 4 + 2 + 2 + 2 + 2;
const MATERIALIZATION_WORK: usize = COMPLETE_WORK + ORIGIN_WORK + HELPER_WORK;

fn empty_helper_payload() -> usize {
    // Four empty Vec headers, the helper receipt, and the checked owner subtotal.
    4 * std::mem::size_of::<Vec<()>>()
        + std::mem::size_of::<fe2o3_lower_mir_kernel::ProductionHelperMemoryStorageV1>()
        + std::mem::size_of::<usize>()
}

fn origin_retained_payload() -> usize {
    // Three Vec headers + receipt, four requested function-association rows.
    // Each association retains root/function/canonical ordinals plus RPO count.
    3 * std::mem::size_of::<Vec<()>>()
        + std::mem::size_of::<SemanticKirAssertOriginStorageV1>()
        + 4 * std::mem::size_of::<(
            SemanticFunctionIdV1,
            SemanticFunctionIdV1,
            fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
            usize,
        )>()
}

fn origin_peak_scratch() -> usize {
    origin_retained_payload()
        + 4 * std::mem::size_of::<(&str, fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1)>()
        + 4 * std::mem::size_of::<[u32; 3]>()
        + 4 * std::mem::size_of::<SemanticFunctionIdV1>()
        + 4 * std::mem::size_of::<usize>()
        + 4 * std::mem::size_of::<&fe2o3_lower_mir_kernel::SemanticKirTerminatorOperationSpanV1>()
}

fn retained_payload() -> usize {
    // FunctionBody is inline in Function. Only its one BasicBlock needs a
    // separate Vec row. Empty signatures, values and capabilities own no heap.
    std::mem::size_of::<VerifiedCanonicalKernelIrModuleV12>()
        + WIRE
        + std::mem::size_of::<Function>()
        + std::mem::size_of::<Kernel>()
        + std::mem::size_of::<BasicBlock>()
        + IDENTIFIER_BYTES
}

fn role_tree_payload() -> usize {
    // Pinned decoded_tree_payload_bound_v12<&FunctionId>(1): three tree nodes
    // (eleven inline keys and fourteen pointer fields) plus four key payloads.
    let key = std::mem::size_of::<&FunctionId>();
    3 * (11 * key + 14 * std::mem::size_of::<usize>()) + 4 * key
}

fn complete_storage() -> usize {
    // Inverse plus role tree dominates both encoder scratch and verifier
    // scratch: module rows5 + max(CFG peak16, retained CFG11 + definitions3).
    assert!(role_tree_payload() > 5 + 16);
    assert!(role_tree_payload() > origin_peak_scratch());
    assert!(role_tree_payload() > origin_retained_payload() + empty_helper_payload());
    retained_payload() + role_tree_payload()
}

fn fixture() -> (ProductionSemanticSsaOwnerV1, ProductionSourceLaunchRosterV1) {
    let unit = SemanticTypeIdV1::from_index(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let owner = owner_from_parts(
        vec![unit_type()],
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(71)),
            unit,
            SemanticLocalRoleV1::Return,
            source,
        )],
        0,
        vec![block(72, vec![], SemanticTerminatorKindV1::Return)],
        b"f",
    );
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(owner, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "logical_f",
            bytes(61),
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1]),
        )],
    )
    .unwrap();
    (ssa, launch)
}

struct Probe {
    result: Result<ProductionPreRankedKirOwnerV1, ProductionPreRankedKirErrorV1>,
    work: usize,
    peak: usize,
    rejected_work: Option<usize>,
    rejected_storage: Option<usize>,
}

fn admit(work_allowance: usize, storage_allowance: usize) -> Probe {
    let (ssa, launch) = fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK_PREFIX + work_allowance);
    work.charge_work(WORK_PREFIX).unwrap();
    let (result, peak, rejected_storage) = {
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            STORAGE_PREFIX + storage_allowance,
        );
        budget.reserve_storage(STORAGE_PREFIX).unwrap();
        // Source analyses need their own allowance even for an empty KIR body.
        // The ledger then admits canonical custody and exact origin/applicability sealing.
        let result = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::new(1, 1, 0),
            &mut budget,
        );
        assert_eq!(budget.storage(), STORAGE_PREFIX);
        (result, budget.peak_storage(), budget.failed_storage())
    };
    Probe {
        result,
        work: work.work(),
        peak,
        rejected_work: work.failed_work(),
        rejected_storage,
    }
}

fn assert_fixture_shape(owner: &ProductionPreRankedKirOwnerV1) {
    let graph = owner.executable().module();
    // Pin the shape used for the independent wire/work/payload calculation.
    assert_eq!(graph.id.as_str().len(), 17 + 64);
    assert!(graph.id.as_str().starts_with("fe2o3::semantic::"));
    assert!(graph.required_capabilities.is_empty());
    let [function] = graph.functions.as_slice() else {
        panic!("fixture changed function roster")
    };
    assert_eq!(function.id.as_str(), "f");
    assert_eq!(function.role, fe2o3_kernel_ir::FunctionRole::KernelEntry);
    assert!(function.signature.parameters.is_empty() && function.signature.results.is_empty());
    assert!(function.required_capabilities.is_empty());
    let body = function.body.as_ref().unwrap();
    assert!(body.parameters.is_empty());
    let [block] = body.blocks.as_slice() else {
        panic!("fixture changed block roster")
    };
    assert_eq!(block.id, fe2o3_kernel_ir::BlockId(0));
    assert!(block.parameters.is_empty() && block.operations.is_empty());
    assert!(
        matches!(&block.terminator, Some(fe2o3_kernel_ir::Terminator::Return { values }) if values.is_empty())
    );
    let [kernel] = graph.kernels.as_slice() else {
        panic!("fixture changed kernel roster")
    };
    assert_eq!(kernel.id.as_str(), "f");
    assert_eq!(kernel.entry.as_str(), "f");
    assert_eq!(
        kernel.domain,
        fe2o3_kernel_ir::LaunchDomain::D1 {
            x: fe2o3_kernel_ir::LaunchExtent::Dynamic
        }
    );
    assert_eq!(
        kernel.workgroup_size,
        Some(fe2o3_kernel_ir::WorkgroupSize::new(64, 1, 1))
    );
    assert!(kernel.required_capabilities.is_empty());
    assert_eq!(owner.executable().canonical().canonical_bytes().len(), WIRE);
}

#[test]
fn pre_ranked_exact_canonical_envelope_preserves_nonzero_prefixes() {
    assert_eq!(
        (
            ENCODE_WORK,
            BEFORE_INVERSE_SCRATCH,
            VERIFIER_WORK,
            BEFORE_HASH,
            COMPLETE_WORK
        ),
        (273, 675, 181, 1_285, 1_528)
    );
    // Header20 + module identifier85 + three roster counts12 = 117.
    // Function ID5 + signature8 + body/blocks27 + capabilities4 = 44.
    // Kernel IDs10 + domain2 + workgroup13 + capabilities4 = 29.
    assert_eq!(WIRE, (20 + 85 + 12) + (5 + 8 + 27 + 4) + (10 + 2 + 13 + 4));
    assert_eq!(ORIGIN_WORK, 32);
    assert_eq!(HELPER_WORK, 12);
    assert_eq!(MATERIALIZATION_WORK, 1_572);
    let exact = admit(MATERIALIZATION_WORK, complete_storage());
    let owner = exact
        .result
        .expect("the exact canonical envelope must admit the fixture");
    assert_eq!(exact.work, WORK_PREFIX + MATERIALIZATION_WORK);
    assert_eq!(exact.peak, STORAGE_PREFIX + complete_storage());
    assert_eq!(exact.rejected_work, None);
    assert_eq!(exact.rejected_storage, None);
    assert_fixture_shape(&owner);
    assert_eq!(
        owner.executable_storage().retained_storage(),
        retained_payload()
    );
    assert_eq!(
        owner.assert_origin_storage().payload_storage(),
        origin_retained_payload()
    );
    assert_eq!(owner.assert_origins().source_site_count(), 0);
    assert_eq!(owner.assert_origins().binding_count(), 0);
    assert_eq!(
        owner.helper_memory_storage_v1().retained_storage(),
        empty_helper_payload()
    );
    assert_eq!(
        owner.retained_analysis_storage_v1(),
        retained_payload() + origin_retained_payload() + empty_helper_payload()
    );
}

#[test]
fn pre_ranked_one_under_work_denies_hash_without_spending_rejected_chunk() {
    let short = admit(COMPLETE_WORK - 1, complete_storage());
    assert!(matches!(
        short.result,
        Err(ProductionPreRankedKirErrorV1::Canonical(CanonicalKernelIrReplayAdmissionErrorV12::Canonical(
            MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(error)
        ))) if error.actual() == WORK_PREFIX + COMPLETE_WORK
            && error.limit() == WORK_PREFIX + COMPLETE_WORK - 1
    ));
    assert_eq!(short.work, WORK_PREFIX + BEFORE_HASH);
    assert_eq!(short.peak, STORAGE_PREFIX + complete_storage());
    assert_eq!(short.rejected_work, Some(WORK_PREFIX + COMPLETE_WORK));
    assert_eq!(short.rejected_storage, None);
}

#[test]
fn pre_ranked_one_under_storage_denies_inverse_comparison_scratch() {
    let short = admit(COMPLETE_WORK, complete_storage() - 1);
    assert!(matches!(
        short.result,
        Err(ProductionPreRankedKirErrorV1::Canonical(CanonicalKernelIrReplayAdmissionErrorV12::Decode(
            KernelIrDecodeError::Resource(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
        ))) if error.actual() == STORAGE_PREFIX + complete_storage()
            && error.limit() == STORAGE_PREFIX + complete_storage() - 1
    ));
    let encoder_peak =
        std::mem::size_of::<VerifiedCanonicalKernelIrV12>() + WIRE + role_tree_payload();
    assert_eq!(short.work, WORK_PREFIX + BEFORE_INVERSE_SCRATCH);
    assert_eq!(
        short.peak,
        STORAGE_PREFIX + encoder_peak.max(retained_payload())
    );
    assert_eq!(short.rejected_work, None);
    assert_eq!(
        short.rejected_storage,
        Some(STORAGE_PREFIX + complete_storage())
    );
}

#[test]
fn pre_ranked_one_under_complete_work_denies_origin_coverage_without_resetting_history() {
    let origin_complete = COMPLETE_WORK + ORIGIN_WORK;
    let short = admit(origin_complete - 1, complete_storage());
    assert!(
        matches!(short.result, Err(ProductionPreRankedKirErrorV1::Lowering(
        ProductionSemanticKirErrorV1::AssertOrigin(SemanticKirAssertOriginErrorV1::Resource(
            CanonicalKernelIrVerificationResourceErrorV1::Work(error)
        ))
    )) if error.actual() == WORK_PREFIX + origin_complete
        && error.limit() == WORK_PREFIX + origin_complete - 1)
    );
    assert_eq!(short.work, WORK_PREFIX + origin_complete - 1);
    assert_eq!(short.rejected_work, Some(WORK_PREFIX + origin_complete));
    assert_eq!(short.rejected_storage, None);
    assert_eq!(short.peak, STORAGE_PREFIX + complete_storage());
}

#[test]
fn pre_ranked_one_under_retained_total_denies_the_last_two_checked_additions() {
    let short = admit(MATERIALIZATION_WORK - 1, complete_storage());
    assert!(matches!(short.result,
        Err(ProductionPreRankedKirErrorV1::Lowering(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                CanonicalKernelIrVerificationResourceErrorV1::Work(error)
            )
        )) if error.actual() == WORK_PREFIX + MATERIALIZATION_WORK
            && error.limit() == WORK_PREFIX + MATERIALIZATION_WORK - 1
    ));
    assert_eq!(short.work, WORK_PREFIX + MATERIALIZATION_WORK - 2);
    assert_eq!(
        short.rejected_work,
        Some(WORK_PREFIX + MATERIALIZATION_WORK)
    );
    assert_eq!(short.rejected_storage, None);
    assert_eq!(short.peak, STORAGE_PREFIX + complete_storage());
}
