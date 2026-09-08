use std::collections::BTreeSet;

use fe2o3_kernel_analysis::{
    KernelCapabilityPreservationErrorV1, analyze_kernel_capability_preservation_v1,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, ExecutionCapabilityRequirementV1, Function,
    FunctionId, Kernel, KernelContextSourceIdentityV1, KernelContextTypeV1, LaunchDomain,
    LaunchExtent, Module, Operation, OperationKind, Signature, TargetCapability, Terminator, Type,
    ValueId,
};

fn context(root: &str) -> KernelContextTypeV1 {
    KernelContextTypeV1::new(root, [1; 32], [2; 32], [3; 32])
}

fn source(byte: u8) -> KernelContextSourceIdentityV1 {
    KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [byte; 32])
}

fn requirement() -> TargetCapability {
    TargetCapability::Execution(ExecutionCapabilityRequirementV1::AddressSpace {
        address_space: AddressSpace::Global,
        access: AccessMode::ReadOnly,
    })
}

fn module() -> Module {
    let context = context("entry");
    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.terminator = Some(Terminator::Return { values: vec![] });
    let helper = Function::internal_helper(
        "helper",
        Signature::new(vec![Type::KernelContext(context.clone())], vec![]),
        vec![ValueId(0)],
        vec![helper_block],
    );

    let mut entry_block = BasicBlock::new(BlockId(0));
    entry_block.operations.push(Operation::kernel_context_issue(
        ValueId(0),
        context,
        source(7),
    ));
    entry_block.operations.push(Operation::new(
        vec![],
        fe2o3_kernel_ir::OperationKind::Call {
            callee: FunctionId::new("helper"),
            arguments: vec![ValueId(0)],
        },
    ));
    entry_block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("context-analysis");
    module.functions.extend([
        Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![entry_block],
        ),
        helper,
    ]);
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module.required_capabilities = BTreeSet::from([requirement()]);
    module
}

#[test]
fn analysis_records_provenance_without_granting_semantic_authority() {
    let analysis = analyze_kernel_capability_preservation_v1(&module(), 17).unwrap();
    assert_eq!(analysis.graph_epoch(), 17);
    assert_eq!(analysis.context_types().len(), 2);
    assert_eq!(analysis.context_uses().len(), 1);
    assert_eq!(analysis.issuances().len(), 1);
    assert!(
        analysis.issuances()[0].has_zero_runtime_memory_effect(),
        "issuance is an inert provenance root"
    );
    assert_eq!(analysis.scoped_requirements().len(), 1);
    assert_eq!(analysis.effective_scoped_requirements().len(), 1);
    assert_eq!(analysis.effective_requirements().len(), 1);
    assert!(!analysis.grants_uniformity_authority());
    assert!(!analysis.grants_memory_or_synchronization_authority());
    assert!(!analysis.grants_target_support_authority());
    assert!(!analysis.grants_compiler_refinement_authority());
    assert!(!analysis.grants_artifact_or_launch_authority());

    let replay = analysis.replay_candidate(17, 17, 0, &module()).unwrap();
    assert!(!replay.changed());
    assert_eq!(replay.input_identity(), replay.output_identity());
    assert!(!replay.grants_semantic_preservation_authority());
    assert!(!replay.grants_target_support_authority());
    assert!(!replay.grants_artifact_or_launch_authority());
}

#[test]
fn unrelated_mutation_requires_one_new_epoch_and_replays_protected_facts() {
    let input = module();
    let analysis = analyze_kernel_capability_preservation_v1(&input, 8).unwrap();
    let mut candidate = input;
    candidate
        .required_capabilities
        .insert(TargetCapability::Int64);

    let replay = analysis.replay_candidate(8, 10, 2, &candidate).unwrap();
    assert!(replay.changed());
    assert_ne!(replay.input_identity(), replay.output_identity());
    assert!(matches!(
        analysis.replay_candidate(8, 9, 2, &candidate),
        Err(KernelCapabilityPreservationErrorV1::InvalidOutputEpoch {
            expected: 10,
            observed: 9,
            changed: true,
            ..
        })
    ));
    assert!(matches!(
        analysis.replay_candidate(8, 9, 1, &module()),
        Err(
            KernelCapabilityPreservationErrorV1::MutationPresenceMismatch {
                canonical_changed: false,
                committed_mutations: 1,
            }
        )
    ));
}

#[test]
fn stale_analysis_epoch_fails_before_candidate_admission() {
    let analysis = analyze_kernel_capability_preservation_v1(&module(), 11).unwrap();
    assert!(matches!(
        analysis.replay_candidate(12, 12, 0, &module()),
        Err(KernelCapabilityPreservationErrorV1::StaleAnalysisEpoch {
            captured: 11,
            observed: 12,
        })
    ));
}

#[test]
fn deletion_duplication_and_reordering_are_rejected() {
    let input = module();
    let analysis = analyze_kernel_capability_preservation_v1(&input, 0).unwrap();

    let mut deleted = input.clone();
    deleted.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .clear();
    deleted.functions.pop();
    assert!(analysis.replay_candidate(0, 1, 1, &deleted).is_err());

    let mut duplicated = input.clone();
    duplicated.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            1,
            Operation::kernel_context_issue(ValueId(9), context("entry"), source(8)),
        );
    assert!(matches!(
        analysis.replay_candidate(0, 1, 1, &duplicated),
        Err(KernelCapabilityPreservationErrorV1::CandidateRejected { .. })
    ));

    let mut reordered = input;
    reordered.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .swap(0, 1);
    assert!(matches!(
        analysis.replay_candidate(0, 1, 1, &reordered),
        Err(KernelCapabilityPreservationErrorV1::CandidateRejected { .. })
    ));
}

#[test]
fn source_root_and_requirement_substitution_are_rejected() {
    let input = module();
    let analysis = analyze_kernel_capability_preservation_v1(&input, 3).unwrap();

    let mut source_substitution = input.clone();
    source_substitution.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks[0]
        .operations[0] = Operation::kernel_context_issue(ValueId(0), context("entry"), source(9));
    assert!(matches!(
        analysis.replay_candidate(3, 4, 1, &source_substitution),
        Err(KernelCapabilityPreservationErrorV1::IssuancesChanged)
    ));

    let mut root_substitution = input.clone();
    root_substitution.functions[0].id = FunctionId::new("other-entry");
    root_substitution.kernels[0].entry = FunctionId::new("other-entry");
    root_substitution.functions[0].body.as_mut().unwrap().blocks[0].operations[0] =
        Operation::kernel_context_issue(ValueId(0), context("other-entry"), source(7));
    root_substitution.functions[1].signature.parameters[0] =
        Type::KernelContext(context("other-entry"));
    assert!(
        analysis
            .replay_candidate(3, 4, 1, &root_substitution)
            .is_err()
    );

    let mut moved_requirement = input.clone();
    moved_requirement.required_capabilities.clear();
    moved_requirement.functions[0]
        .required_capabilities
        .insert(requirement());
    assert!(matches!(
        analysis.replay_candidate(3, 4, 1, &moved_requirement),
        Err(KernelCapabilityPreservationErrorV1::ScopedRequirementsChanged)
    ));

    let mut removed_requirement = input;
    removed_requirement.required_capabilities.clear();
    assert!(
        analysis
            .replay_candidate(3, 4, 1, &removed_requirement)
            .is_err()
    );
}

#[test]
fn context_omission_and_same_root_brand_substitution_are_rejected() {
    let input = module();
    let analysis = analyze_kernel_capability_preservation_v1(&input, 23).unwrap();

    let mut omitted = input.clone();
    let OperationKind::Call { arguments, .. } =
        &mut omitted.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind
    else {
        panic!("fixture must retain its helper call");
    };
    arguments.clear();
    omitted.functions[1].signature.parameters.clear();
    omitted.functions[1]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .clear();
    assert!(matches!(
        analysis.replay_candidate(23, 24, 1, &omitted),
        Err(KernelCapabilityPreservationErrorV1::ContextTypesChanged)
    ));

    let replacement = KernelContextTypeV1::new("entry", [9; 32], [2; 32], [3; 32]);
    let mut substituted = input;
    substituted.functions[0].body.as_mut().unwrap().blocks[0].operations[0] =
        Operation::kernel_context_issue(ValueId(0), replacement.clone(), source(7));
    substituted.functions[1].signature.parameters[0] = Type::KernelContext(replacement);
    assert!(matches!(
        analysis.replay_candidate(23, 24, 1, &substituted),
        Err(KernelCapabilityPreservationErrorV1::ContextTypesChanged)
    ));
}

#[test]
fn changed_execution_capability_and_unsupported_context_operation_fail_closed() {
    let input = module();
    let analysis = analyze_kernel_capability_preservation_v1(&input, 41).unwrap();

    let mut changed_capability = input.clone();
    changed_capability.required_capabilities = BTreeSet::from([TargetCapability::Execution(
        ExecutionCapabilityRequirementV1::AddressSpace {
            address_space: AddressSpace::Global,
            access: AccessMode::ReadWrite,
        },
    )]);
    assert!(matches!(
        analysis.replay_candidate(41, 42, 1, &changed_capability),
        Err(KernelCapabilityPreservationErrorV1::ScopedRequirementsChanged)
    ));

    let mut unsupported = input;
    unsupported.functions[1].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::kernel_context_issue(
            ValueId(1),
            context("entry"),
            source(11),
        ));
    assert!(matches!(
        analysis.replay_candidate(41, 42, 1, &unsupported),
        Err(KernelCapabilityPreservationErrorV1::CandidateRejected { .. })
    ));
}
