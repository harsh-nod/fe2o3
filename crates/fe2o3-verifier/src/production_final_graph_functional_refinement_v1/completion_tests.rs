use fe2o3_kernel_ir::{BinaryOp, WorkgroupSize};
use fe2o3_pliron::ProductionFinalGraphOwnerV1;

use super::*;
use crate::production_final_graph_functional_refinement_v1::prepared::require_verified_graph_subject;
use crate::production_final_graph_functional_refinement_v1::theorem::CanonicalFinalKirSubjectsV1;

#[test]
fn identity_optimization_prepares_an_exact_nonidentical_final_theorem() {
    let source = canonical(7);
    let mut optimized = module(7);
    let block = &mut optimized.functions[0].body.as_mut().unwrap().blocks[0];
    let store = block.operations.pop().unwrap();
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(0)),
    ));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(1),
            rhs: ValueId(2),
        },
    ));
    let OperationKind::Store {
        pointer, access, ..
    } = store.kind
    else {
        panic!("fixture must end in a store");
    };
    block.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer,
            access,
            value: ValueId(3),
        },
    ));
    let final_graph = VerifiedCanonicalKernelIrV13::from_module(optimized).unwrap();
    assert_ne!(source.canonical_bytes(), final_graph.canonical_bytes());
    let target = contract(&source, &final_graph, 8, 7, [9; 32], "test-v1", 1);
    let prepared = Subject::try_new(&source, &final_graph, 8, &target).unwrap();
    let shared = PreparedFinalKirTheoremV1::try_new(&source, &final_graph, 8).unwrap();
    assert_eq!(prepared.generated_source(), shared.generated_source());
    prepared
        .require_exact_subject(&source, &final_graph, 8, &target)
        .unwrap();
    assert!(!prepared.authenticates_verus_execution());
}

fn schedule_only_canonical(name: &str) -> VerifiedCanonicalKernelIrV13 {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new(name);
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    module.kernels.push(kernel);
    VerifiedCanonicalKernelIrV13::from_module(module).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn schedule_contract(
    source: &VerifiedCanonicalKernelIrV13,
    final_graph: &VerifiedCanonicalKernelIrV13,
    epoch: u64,
    neutral_epoch: u64,
    closure: [u8; 32],
    revision: &'static str,
    extra_subgroup: bool,
) -> ProductionFinalGraphTargetContractV1 {
    let base = contract(
        source,
        final_graph,
        epoch,
        neutral_epoch,
        closure,
        revision,
        1,
    );
    let mut decisions = base.decisions().to_vec();
    decisions.push(
        query_target_capability_v1(
            &TestTarget(base.model()),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupDimensions { x: 1, y: 1, z: 1 },
            ),
        )
        .unwrap(),
    );
    if extra_subgroup {
        decisions.push(
            query_target_capability_v1(
                &TestTarget(base.model()),
                TargetCapabilityRequirementV1::SubgroupSize(32),
            )
            .unwrap(),
        );
    }
    decisions.sort_by_key(|decision| decision.requirement());
    ProductionFinalGraphTargetContractV1::try_new(
        final_graph,
        epoch,
        *source.identity(),
        neutral_epoch,
        closure,
        base.model(),
        decisions,
    )
    .unwrap()
}

fn verify_schedule(
    canonical: VerifiedCanonicalKernelIrV13,
    epoch: u64,
    target: ProductionFinalGraphTargetContractV1,
) -> ProductionVerifiedFinalGraphV1 {
    ProductionFinalGraphOwnerV1::try_new(canonical, epoch, target)
        .unwrap()
        .verify()
        .unwrap()
}

#[test]
fn completion_subject_comparison_accepts_an_actually_verified_owner() {
    // A no-write fixture can pass the real schedule without inventing a
    // functional receipt or bypassing the still-fail-closed write projection.
    let source = schedule_only_canonical("completion-source");
    let final_graph = schedule_only_canonical("completion-final");
    let subjects = CanonicalFinalKirSubjectsV1::try_new(&source, &final_graph, 8).unwrap();
    let target = schedule_contract(&source, &final_graph, 8, 7, [9; 32], "test-v1", false);
    let owner_target = schedule_contract(&source, &final_graph, 8, 7, [9; 32], "test-v1", false);
    let mut verified = verify_schedule(final_graph, 8, owner_target);
    require_verified_graph_subject(&subjects, &target, &mut verified).unwrap();
    verified.revalidate_live().unwrap();
}

#[test]
fn completion_rejects_wrong_verified_graph_and_actual_epoch() {
    let source = schedule_only_canonical("completion-source");
    let final_graph = schedule_only_canonical("completion-final");
    let subjects = CanonicalFinalKirSubjectsV1::try_new(&source, &final_graph, 8).unwrap();
    let target = schedule_contract(&source, &final_graph, 8, 7, [9; 32], "test-v1", false);
    let substituted = schedule_only_canonical("substituted-final");
    let other_target = schedule_contract(&source, &substituted, 8, 7, [9; 32], "test-v1", false);
    let mut other = verify_schedule(substituted, 8, other_target);
    assert!(matches!(
        require_verified_graph_subject(&subjects, &target, &mut other),
        Err(Failure::FinalGraphSubjectMismatch),
    ));
    let wrong_epoch = schedule_contract(&source, &final_graph, 9, 7, [9; 32], "test-v1", false);
    let mut other = verify_schedule(final_graph, 9, wrong_epoch);
    assert!(matches!(
        require_verified_graph_subject(&subjects, &target, &mut other),
        Err(Failure::FinalGraphSubjectMismatch),
    ));
}

#[test]
fn completion_rejects_every_substituted_owner_target_record() {
    let source = schedule_only_canonical("completion-source");
    let final_graph = schedule_only_canonical("completion-final");
    let subjects = CanonicalFinalKirSubjectsV1::try_new(&source, &final_graph, 8).unwrap();
    let target = schedule_contract(&source, &final_graph, 8, 7, [9; 32], "test-v1", false);
    for (neutral_epoch, closure, revision, extra_subgroup) in [
        (6, [9; 32], "test-v1", false),
        (7, [8; 32], "test-v1", false),
        (7, [9; 32], "test-v2", false),
        (7, [9; 32], "test-v1", true),
    ] {
        let candidate = schedule_only_canonical("completion-final");
        let owner_target = schedule_contract(
            &source,
            &candidate,
            8,
            neutral_epoch,
            closure,
            revision,
            extra_subgroup,
        );
        let mut verified = verify_schedule(candidate, 8, owner_target);
        assert!(matches!(
            require_verified_graph_subject(&subjects, &target, &mut verified),
            Err(Failure::TargetContractSubjectMismatch),
        ));
    }
}

#[test]
fn completion_rejects_substituted_owner_source_canonical() {
    let source = schedule_only_canonical("completion-source");
    let wrong_source = schedule_only_canonical("substituted-source");
    let final_graph = schedule_only_canonical("completion-final");
    let subjects = CanonicalFinalKirSubjectsV1::try_new(&source, &final_graph, 8).unwrap();
    let target = schedule_contract(&source, &final_graph, 8, 7, [9; 32], "test-v1", false);
    let owner_target =
        schedule_contract(&wrong_source, &final_graph, 8, 7, [9; 32], "test-v1", false);
    let mut verified = verify_schedule(final_graph, 8, owner_target);
    assert!(matches!(
        require_verified_graph_subject(&subjects, &target, &mut verified),
        Err(Failure::TargetContractSubjectMismatch),
    ));
}

#[test]
fn consuming_completion_requires_an_executed_proof_but_no_runtime() {
    let _completion: fn(
        ProductionPreparedFinalGraphFunctionalExecutionV1,
        &mut ProductionVerifiedFinalGraphV1,
    )
        -> Result<ProductionFinalGraphFunctionalRefinementExecutionV2, Failure> =
        ProductionPreparedFinalGraphFunctionalExecutionV1::complete_with_verified_graph;
}
