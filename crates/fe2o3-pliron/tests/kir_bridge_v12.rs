use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;
use fe2o3_pliron::{KirBridgeErrorV1, PlironOptimizationPlanV1, PlironSession, ShellLimits};

fn session() -> PlironSession {
    PlironSession::new(
        ShellLimits::default(),
        [
            dialect_gpu::dialect_registration().expect("valid gpu registration"),
            dialect_kernel::dialect_registration().expect("valid kernel registration"),
        ],
    )
    .expect("fresh Pliron session")
}

fn context_type() -> KernelContextTypeV1 {
    KernelContextTypeV1::new("entry", [1; 32], [2; 32], [3; 32])
}

fn source_identity() -> KernelContextSourceIdentityV1 {
    KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32])
}

fn production_v12_module() -> Module {
    let context = context_type();
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
        source_identity(),
    ));
    entry_block.operations.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new("helper"),
            arguments: vec![ValueId(0)],
        },
    ));
    entry_block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry_block],
    );

    let mut module = Module::new("production-v12-capability-bridge");
    module.functions = vec![entry, helper];
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module.required_capabilities = [
        ExecutionCapabilityRequirementV1::AddressSpace {
            address_space: AddressSpace::Global,
            access: AccessMode::ReadOnly,
        },
        ExecutionCapabilityRequirementV1::Barrier {
            execution_scope: SynchronizationScope::Workgroup,
            memory_scope: SynchronizationScope::Workgroup,
            ordering: MemoryOrdering::AcquireRelease,
            address_spaces: BTreeSet::from([AddressSpace::Workgroup]),
        },
        ExecutionCapabilityRequirementV1::Resource(
            ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(256),
        ),
    ]
    .into_iter()
    .map(TargetCapability::Execution)
    .collect();
    module
}

#[test]
fn production_v12_context_and_requirements_round_trip_exactly() {
    let input = VerifiedCanonicalKernelIrV12::from_module(production_v12_module()).unwrap();
    let canonical_bytes = {
        let mut owner = session();
        let graph = owner.import_canonical_kir_v12_o0(&input).unwrap();
        let (output, report) = owner.extract_canonical_kir_v12_o0(&graph).unwrap();
        assert!(report.is_exact());
        output.canonical_bytes().to_vec()
    };

    let decoded = decode_module_v12(&canonical_bytes).expect("fresh V12 decode");
    let reencoded = VerifiedCanonicalKernelIrV12::from_module(decoded).expect("fresh V12 encode");
    let mut fresh = session();
    let graph = fresh.import_canonical_kir_v12_o0(&reencoded).unwrap();
    let (output, report) = fresh.extract_canonical_kir_v12_o0(&graph).unwrap();

    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert!(report.is_exact());
    assert_eq!(report.input(), report.output());
}

#[test]
fn production_v12_optimization_retains_capability_correspondence() {
    let input = VerifiedCanonicalKernelIrV12::from_module(production_v12_module()).unwrap();
    let mut session = session();
    let graph = session.import_canonical_kir_v12_o0(&input).unwrap();
    session
        .execute_optimization_v1(graph.root(), &PlironOptimizationPlanV1::standard())
        .unwrap();
    let (output, receipt) = session
        .extract_optimized_canonical_kir_v12_v1(&graph)
        .unwrap();

    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert_eq!(receipt.input(), receipt.output());
    assert_eq!(receipt.correspondence().len(), graph.correspondence().len());
}

#[test]
fn prior_bridge_endpoints_reject_a_v12_graph() {
    let input = VerifiedCanonicalKernelIrV12::from_module(production_v12_module()).unwrap();
    let mut session = session();
    let graph = session.import_canonical_kir_v12_o0(&input).unwrap();
    assert_eq!(
        session.extract_canonical_kir_v11_o0(&graph),
        Err(KirBridgeErrorV1::GraphIdentityMismatch)
    );
}
