use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, Constant, Function, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV13,
};
use fe2o3_pliron::ProductionFinalGraphTargetContractV1;
use fe2o3_target_spec::{
    TargetArchitectureFamilyV1, TargetArtifactFormatV1, TargetCapabilityDecisionOutcomeV1,
    TargetCapabilityModelIdentityV1, TargetCapabilityQueryV1, TargetCapabilityRequirementV1,
    TargetExecutionModelV1, TargetProfileSpecV1, TargetResourceRequirementV1, TargetVendorV1,
    query_target_capability_v1,
};

use super::*;

#[path = "completion_tests.rs"]
mod completion;

type Subject = ProductionPreparedFinalGraphFunctionalSubjectV1;
type Failure = ProductionFinalGraphFunctionalRefinementErrorV1;

struct TestTarget(TargetCapabilityModelIdentityV1);

impl TargetCapabilityQueryV1 for TestTarget {
    fn model_identity(&self) -> TargetCapabilityModelIdentityV1 {
        self.0
    }

    fn query_outcome(&self, _: TargetCapabilityRequirementV1) -> TargetCapabilityDecisionOutcomeV1 {
        TargetCapabilityDecisionOutcomeV1::Supported
    }
}

fn module(value: u32) -> Module {
    let output = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(value)),
    ));
    block.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("prepared-final-functional");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![output], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    module
}

fn canonical(value: u32) -> VerifiedCanonicalKernelIrV13 {
    VerifiedCanonicalKernelIrV13::from_module(module(value)).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn contract(
    source: &VerifiedCanonicalKernelIrV13,
    final_graph: &VerifiedCanonicalKernelIrV13,
    final_epoch: u64,
    neutral_epoch: u64,
    closure: [u8; 32],
    revision: &'static str,
    resource: u32,
) -> ProductionFinalGraphTargetContractV1 {
    let profile = TargetProfileSpecV1::from_static_parts(
        TargetVendorV1::Other,
        TargetArchitectureFamilyV1::Other,
        "prepared-functional-test",
        None,
        None,
        TargetArtifactFormatV1::NativeObject,
        TargetExecutionModelV1::GpuGrid,
        None,
        &[],
    );
    let model = TargetCapabilityModelIdentityV1::new(profile, revision).unwrap();
    let decision = query_target_capability_v1(
        &TestTarget(model),
        TargetCapabilityRequirementV1::Resource(
            TargetResourceRequirementV1::WorkgroupInvocationsAtMost(resource),
        ),
    )
    .unwrap();
    ProductionFinalGraphTargetContractV1::try_new(
        final_graph,
        final_epoch,
        *source.identity(),
        neutral_epoch,
        closure,
        model,
        [decision],
    )
    .unwrap()
}

#[test]
fn preparation_needs_no_protected_runtime_or_source_proof() {
    let source = canonical(7);
    let final_graph = canonical(7);
    let target = contract(&source, &final_graph, 8, 7, [9; 32], "test-v1", 1);
    let prepared = Subject::try_new(&source, &final_graph, 8, &target).unwrap();
    prepared
        .require_exact_subject(&source, &final_graph, 8, &target)
        .unwrap();
    assert_eq!(prepared.source_canonical(), &source);
    assert_eq!(prepared.final_canonical(), &final_graph);
    assert_eq!(prepared.source_kernels(), module(7).kernels);
    assert_eq!(prepared.final_kernels(), module(7).kernels);
    assert!(!prepared.authenticates_verus_execution());
    assert!(!prepared.grants_final_graph_verification_authority());
    assert!(!prepared.grants_artifact_or_launch_authority());
}

#[test]
fn preparing_a_changed_rhs_does_not_claim_the_generated_theorem_is_true() {
    let source = canonical(7);
    let changed = canonical(8);
    let target = contract(&source, &changed, 8, 7, [9; 32], "test-v1", 1);
    let prepared = Subject::try_new(&source, &changed, 8, &target).unwrap();
    let unchanged = PreparedFinalKirTheoremV1::try_new(&source, &source, 8).unwrap();
    assert_ne!(prepared.generated_source(), unchanged.generated_source());
    assert!(!prepared.authenticates_verus_execution());
    assert!(!prepared.grants_final_graph_verification_authority());
}

#[test]
fn source_final_and_epoch_substitution_are_rejected() {
    let source = canonical(7);
    let final_graph = canonical(7);
    let changed = canonical(8);
    let target = contract(&source, &final_graph, 8, 7, [9; 32], "test-v1", 1);
    let prepared = Subject::try_new(&source, &final_graph, 8, &target).unwrap();
    assert!(matches!(
        prepared.require_exact_subject(&changed, &final_graph, 8, &target),
        Err(Failure::SourceGraphSubjectMismatch),
    ));
    assert!(matches!(
        prepared.require_exact_subject(&source, &changed, 8, &target),
        Err(Failure::FinalGraphSubjectMismatch),
    ));
    assert!(matches!(
        prepared.require_exact_subject(&source, &final_graph, 9, &target),
        Err(Failure::FinalGraphSubjectMismatch),
    ));
    for (source, final_graph, epoch) in [
        (&changed, &final_graph, 8),
        (&source, &changed, 8),
        (&source, &final_graph, 9),
    ] {
        assert!(matches!(
            Subject::try_new(source, final_graph, epoch, &target),
            Err(Failure::TargetContractSubjectMismatch),
        ));
    }
}

#[test]
fn target_substitution_checks_records_not_only_closure_identity() {
    let source = canonical(7);
    let final_graph = canonical(7);
    let target = contract(&source, &final_graph, 8, 7, [9; 32], "test-v1", 1);
    let prepared = Subject::try_new(&source, &final_graph, 8, &target).unwrap();
    for replacement in [
        contract(&source, &final_graph, 8, 6, [9; 32], "test-v1", 1),
        contract(&source, &final_graph, 8, 7, [8; 32], "test-v1", 1),
        contract(&source, &final_graph, 8, 7, [9; 32], "test-v2", 1),
        contract(&source, &final_graph, 8, 7, [9; 32], "test-v1", 2),
    ] {
        assert!(matches!(
            prepared.require_exact_subject(&source, &final_graph, 8, &replacement),
            Err(Failure::TargetContractSubjectMismatch),
        ));
    }
    let empty_closure = contract(&source, &final_graph, 8, 7, [0; 32], "test-v1", 1);
    assert!(matches!(
        Subject::try_new(&source, &final_graph, 8, &empty_closure),
        Err(Failure::TargetClosureMissing),
    ));
}

#[test]
fn detached_module_mutation_cannot_mutate_prepared_bytes_or_rosters() {
    let mut detached = module(7);
    detached.kernels.push(Kernel::new(
        "another-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    let source = VerifiedCanonicalKernelIrV13::from_module(detached.clone()).unwrap();
    let target = contract(&source, &source, 8, 7, [9; 32], "test-v1", 1);
    let prepared = Subject::try_new(&source, &source, 8, &target).unwrap();
    let original_roster = detached.kernels.clone();
    detached.kernels.reverse();
    let reordered = VerifiedCanonicalKernelIrV13::from_module(detached).unwrap();
    assert_eq!(prepared.source_kernels(), original_roster);
    assert_eq!(prepared.final_kernels(), original_roster);
    assert_eq!(
        prepared.final_canonical().canonical_bytes(),
        source.canonical_bytes()
    );
    assert!(matches!(
        prepared.require_exact_subject(&source, &reordered, 8, &target),
        Err(Failure::FinalGraphSubjectMismatch),
    ));
}

#[test]
fn prepared_and_verified_routes_share_generated_source_and_obligation_recipe() {
    let source = canonical(7);
    let final_graph = canonical(7);
    let target = contract(&source, &final_graph, 8, 7, [9; 32], "test-v1", 1);
    let prepared = Subject::try_new(&source, &final_graph, 8, &target).unwrap();
    let shared = PreparedFinalKirTheoremV1::try_new(&source, &final_graph, 8).unwrap();
    assert_eq!(prepared.generated_source(), shared.generated_source());
    let identity = |source: &VerifiedCanonicalKernelIrV13,
                    final_graph: &VerifiedCanonicalKernelIrV13,
                    epoch,
                    product,
                    generated| {
        final_graph_obligation_identity_v1(
            source.identity(),
            final_graph.identity(),
            epoch,
            product,
            generated,
            1,
            FinalKirNumericalModelV1::ExactBitVector,
        )
    };
    let product = DigestV1::from_untrusted_bytes([3; 32]);
    let generated = shared.generated_source();
    let expected = identity(&source, &final_graph, 8, product, generated);
    assert_eq!(
        expected,
        identity(
            &source,
            &final_graph,
            8,
            product,
            prepared.generated_source()
        )
    );
    let changed = canonical(8);
    assert_ne!(
        expected,
        identity(&changed, &final_graph, 8, product, generated)
    );
    assert_ne!(expected, identity(&source, &changed, 8, product, generated));
    assert_ne!(
        expected,
        identity(&source, &final_graph, 9, product, generated)
    );
    assert_ne!(
        expected,
        identity(
            &source,
            &final_graph,
            8,
            DigestV1::from_untrusted_bytes([4; 32]),
            generated
        )
    );
    assert_ne!(
        expected,
        identity(
            &source,
            &final_graph,
            8,
            product,
            DigestV1::from_untrusted_bytes([5; 32])
        )
    );
}
