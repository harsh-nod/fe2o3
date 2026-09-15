use super::{
    ExecutionCapabilitySemanticReasonV1 as Reason, analyze_execution_capability_final_graph_v1,
};
use fe2o3_kernel_ir::*;

#[allow(dead_code)]
mod fixture {
    use super::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/subgroup_partition/fixture.rs"
    ));
    pub(super) fn module_fixture() -> Module {
        module()
    }
}

#[test]
fn partition_collectives_require_full_physical_wave_participation() {
    for participants in [16, 64] {
        let mut module = fixture::module_fixture();
        module.kernels[0].workgroup_size = Some(WorkgroupSize::new(participants, 1, 1));
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let report = analyze_execution_capability_final_graph_v1(&canonical, &module, 7).unwrap();
        assert_eq!(report.checked_collective_sites(), 2);
        let incomplete = report
            .findings()
            .iter()
            .filter(|finding| {
                matches!(
                    finding.reason(),
                    Reason::IncompleteCollectiveParticipation {
                        expected: 64,
                        observed: 16
                    }
                )
            })
            .count();
        assert_eq!(incomplete, if participants == 16 { 2 } else { 0 });
        assert!(!report.grants_proof_machine_artifact_or_launch_authority());
    }
}

#[test]
fn partition_collectives_reject_divergent_arrival() {
    let mut module = fixture::module_fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    let entry = &mut body.blocks[0];
    let mut collectives = entry.operations.split_off(6);
    for operation in &mut collectives {
        let OperationKind::ExecutionCapability(contract) = &mut operation.kind else {
            unreachable!()
        };
        contract.source.block = 1;
    }
    entry.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::NotEqual,
                lhs: ValueId(8),
                rhs: ValueId(9),
            },
        ),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(10),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut arrived = BasicBlock::new(BlockId(1));
    arrived.operations = collectives;
    arrived.terminator = Some(Terminator::Return { values: vec![] });
    let mut skipped = BasicBlock::new(BlockId(2));
    skipped.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([arrived, skipped]);
    let required = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .flat_map(Operation::required_capabilities)
        .collect::<std::collections::BTreeSet<_>>();
    module.functions[0]
        .required_capabilities
        .extend(required.clone());
    module.kernels[0]
        .required_capabilities
        .extend(required.clone());
    module.required_capabilities.extend(required);
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let report = analyze_execution_capability_final_graph_v1(&canonical, &module, 7).unwrap();
    assert_eq!(report.checked_collective_sites(), 2);
    assert_eq!(
        report
            .findings()
            .iter()
            .filter(|finding| matches!(
                finding.reason(),
                Reason::NonUniformArrival {
                    scope: SynchronizationScope::Subgroup,
                    ..
                }
            ))
            .count(),
        2
    );
}
