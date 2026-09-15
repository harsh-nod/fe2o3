//! Negative uses the real first phase's consumed SSA operand at the real
//! second Finish call. No replacement source body or positive KIR fixture.
use fe2o3_kernel_ir::{
    DiagnosticCode, ExecutionCapabilityOperationV1 as Exec, Module, OperationKind,
    ReusablePhaseOperationV1 as Phase, Type, verify_module,
};

pub(super) fn check(module: &Module) {
    let functions = module
        .functions
        .iter()
        .enumerate()
        .filter_map(|(index, function)| {
            let body = function.body.as_ref()?;
            let barriers = body
                .blocks
                .iter()
                .enumerate()
                .flat_map(|(block_index, block)| {
                    block
                        .operations
                        .iter()
                        .enumerate()
                        .filter_map(move |(ordinal, op)| match &op.kind {
                            OperationKind::ExecutionCapability(cap)
                                if matches!(cap.operation, Exec::WorkgroupBarrier { .. }) =>
                            {
                                Some((block_index, ordinal, cap, op))
                            }
                            _ => None,
                        })
                })
                .collect::<Vec<_>>();
            (!barriers.is_empty()).then_some((index, body, barriers))
        })
        .collect::<Vec<_>>();
    assert_eq!(functions.len(), 1);
    let (function_index, body, barriers) = &functions[0];
    assert_eq!(barriers.len(), 2);
    let first = barriers[0].2;
    let second = barriers[1].2;
    assert_eq!(first.provenance, second.provenance);
    assert_eq!(first.workgroup_brand, second.workgroup_brand);
    assert_eq!(first.operation, second.operation);
    assert_eq!(first.signature, second.signature);
    assert_ne!(first.source.occurrence, second.source.occurrence);
    assert_ne!(first.epoch_before, second.epoch_before);
    assert_ne!(first.epoch_after, second.epoch_after);
    for (_, _, barrier, _) in barriers {
        assert_ne!(barrier.epoch_before, barrier.epoch_after);
        assert_eq!(barrier.operands.len(), 1);
        let producers = body
            .blocks
            .iter()
            .flat_map(|b| &b.operations)
            .filter_map(|op| {
                op.results
                    .iter()
                    .find(|r| r.id == barrier.operands[0])
                    .map(|result| (op, result))
            })
            .collect::<Vec<_>>();
        assert_eq!(producers.len(), 1);
        let (producer, result) = producers[0];
        let OperationKind::ReusablePhase(begin) = &producer.kind else {
            panic!("Finish must consume the actual Begin-issued Workgroup");
        };
        let Phase::Begin { dynamic_epoch, .. } = begin.operation else {
            panic!("Finish source producer is not Begin");
        };
        let Type::ExecutionCapability(input) = &result.ty else {
            panic!("Workgroup type");
        };
        assert_eq!(input.epoch, Some(dynamic_epoch));
        assert_eq!(barrier.epoch_before, input.epoch);
    }

    let mut stale = module.clone();
    let second_block = barriers[1].0;
    let second_ordinal = barriers[1].1;
    let body_mut = stale.functions[*function_index].body.as_mut().unwrap();
    let block_id = body_mut.blocks[second_block].id;
    let OperationKind::ExecutionCapability(changed) =
        &mut body_mut.blocks[second_block].operations[second_ordinal].kind
    else {
        unreachable!()
    };
    // Retain the second original source call and its output epoch, but consume
    // the first original Workgroup after that Workgroup's real barrier.
    changed.operands[0] = first.operands[0];
    changed.epoch_before = first.epoch_before;
    assert_eq!(changed.source, second.source);
    assert_eq!(changed.epoch_after, second.epoch_after);
    let errors = verify_module(&stale).expect_err("stale original phase operand must reject");
    assert!(
        errors.diagnostics().iter().any(|d| {
            d.code == DiagnosticCode::InvalidExecutionCapability
                && d.location.block == Some(block_id)
                && d.location.operation == Some(second_ordinal)
                && d.message
                    == "execution operation consumes an epoch after a dominating transition"
        }),
        "{errors:?}"
    );
}
