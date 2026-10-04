use super::*;
use fe2o3_kernel_ir::{
    AssemblyConstraint, AssemblyOperand, AssemblyOperandKind, AssemblyOption,
    AssemblySourceIdentity, InlineAssembly, InlineAssemblyTarget, VerificationContractKeyV12,
    VerificationContractOperationV12, WorkgroupPipelineEventKindV12,
};

#[test]
fn ordering_and_memory_axes_remain_distinct_through_calls() {
    let mut source = module(&[("root", &["helper"], 0), ("helper", &[], 1)]);
    for function in &mut source.functions {
        function.signature.parameters = vec![
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Workgroup,
                AccessMode::ReadOnly,
            ),
            Type::INDEX,
        ];
    }
    let block = &mut source.functions[1].body.as_mut().unwrap().blocks[0];
    let OperationKind::Load { access, .. } = &mut block.operations[0].kind else {
        unreachable!()
    };
    access.address_space = AddressSpace::Workgroup;
    block.operations.push(Operation::new(
        vec![],
        OperationKind::VerificationContract(
            VerificationContractOperationV12::WorkgroupPipelineEvent {
                contract: VerificationContractKeyV12::new(11),
                kind: WorkgroupPipelineEventKindV12::Stage,
                storage: ValueId(0),
                epoch: ValueId(1),
            },
        ),
    ));
    for only_ordering in [false, true] {
        if only_ordering {
            source.functions[1].body.as_mut().unwrap().blocks[0]
                .operations
                .remove(0);
        }
        let (owner, _) = admit(&source);
        let (inventory, _) = inventory(&owner);
        let (report, _) = report(&inventory);
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        assert_eq!(
            report.decision(Function(0), &mut budget).unwrap(),
            Decision::CompleteNonempty
        );
        let (mut reads, mut ordering) = (0, 0);
        report
            .try_visit(Function(0), &mut budget, |event| {
                assert_eq!(event.function(), Function(1));
                assert_eq!(event.call_path(), [0]);
                match event.kind() {
                    CanonicalKirCallEffectKindV1::Physical(_) => reads += 1,
                    CanonicalKirCallEffectKindV1::CompilerOrdering(_) => ordering += 1,
                }
                Ok::<_, Error>(())
            })
            .unwrap();
        assert_eq!((reads, ordering), (usize::from(!only_ordering), 1));
    }
}

#[test]
fn opaque_empty_assembly_never_acquires_empty_facts() {
    let mut source = module(&[("root", &["helper"], 0), ("helper", &[], 0)]);
    let op = Operation::new(
        vec![],
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: "s_nop".into(),
            operands: vec![AssemblyOperand {
                kind: AssemblyOperandKind::ImmediateI32(0),
                constraint: AssemblyConstraint::ImmediateI32,
            }],
            options: [AssemblyOption::NoMemory, AssemblyOption::NoStack].into(),
            declared_effects: Default::default(),
        }),
    );
    source
        .required_capabilities
        .extend(op.required_capabilities());
    for function in &mut source.functions {
        function
            .required_capabilities
            .extend(op.required_capabilities());
    }
    source.kernels[0]
        .required_capabilities
        .extend(op.required_capabilities());
    source.functions[1].body.as_mut().unwrap().blocks[0]
        .operations
        .push(op);
    let (owner, _) = admit(&source);
    let (inventory, _) = inventory(&owner);
    let (report, _) = report(&inventory);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    assert_eq!(
        report.decision(Function(0), &mut budget).unwrap(),
        Decision::Incomplete
    );
    assert_eq!(
        report.decision(Function(1), &mut budget).unwrap(),
        Decision::Incomplete
    );
}

#[test]
fn stored_order_includes_unreachable_effects_and_preserves_volatile_payload() {
    let mut source = module(&[("root", &["helper"], 0), ("helper", &[], 0)]);
    let mut dead = function("dead", &[], 1).body.unwrap().blocks.remove(0);
    dead.id = BlockId(37);
    let OperationKind::Load { access, .. } = &mut dead.operations[0].kind else {
        unreachable!()
    };
    access.volatile = true;
    source.functions[1].body.as_mut().unwrap().blocks.push(dead);
    let (owner, _) = admit(&source);
    let (inventory, _) = inventory(&owner);
    let (report, _) = report(&inventory);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    assert_eq!(
        report.decision(Function(0), &mut budget).unwrap(),
        Decision::CompleteNonempty
    );
    let mut seen = 0;
    report
        .try_visit(Function(0), &mut budget, |event| {
            let CanonicalKirCallEffectKindV1::Physical(effect) = event.kind() else {
                unreachable!()
            };
            let OperationKind::Load { access, .. } = effect.operation.kind else {
                unreachable!()
            };
            assert!(access.volatile);
            seen += 1;
            Ok::<_, Error>(())
        })
        .unwrap();
    assert_eq!(seen, 1);
}

#[test]
fn complete_empty_does_not_infer_scalar_values_or_skip_invalid_locator() {
    let (owner, _) = admit(&module(&[
        ("root", &["empty", "empty"], 0),
        ("empty", &[], 0),
    ]));
    let (inventory, _) = inventory(&owner);
    let (report, _) = report(&inventory);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    assert_eq!(
        report.decision(Function(0), &mut budget).unwrap(),
        Decision::CompleteEmpty
    );
    report
        .try_visit(Function(0), &mut budget, |_| -> Result<()> {
            panic!("no effects");
        })
        .unwrap();
    assert_eq!(
        report.decision(Function(u32::MAX), &mut budget),
        Err(Error::InvalidFunction(Function(u32::MAX)))
    );
}
