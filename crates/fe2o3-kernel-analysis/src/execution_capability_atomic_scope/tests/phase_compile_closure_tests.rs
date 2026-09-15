//! Raw scanning tests; no phase lifecycle is admitted here.
use super::super::{
    ExecutionCapabilityAtomicScopeErrorV1, analyze_execution_capability_atomic_scope_v1,
};
use super::{atomic, module};
use fe2o3_kernel_ir::{
    ExecutionMemoryScopeV1, ExecutionSafetyObligationsV1, Operation, OperationKind, PhaseKeyV1,
    PhaseOperationSourceV1, ReusablePhaseOpV1, ReusablePhaseOperationV1,
};

fn phase() -> Operation {
    let OperationKind::ExecutionCapability(contract) = atomic(ExecutionMemoryScopeV1::Device).kind
    else {
        unreachable!()
    };
    Operation::new(
        vec![],
        OperationKind::ReusablePhase(ReusablePhaseOpV1 {
            operands: vec![],
            operation: ReusablePhaseOperationV1::End { storage_count: 0 },
            provenance: contract.provenance,
            source: PhaseOperationSourceV1::WrapperEnd {
                phase: PhaseKeyV1::from_untrusted_bytes([7; 32]),
                wrapper_normal_target: 0,
                source_protocol: [8; 32],
            },
            obligations: ExecutionSafetyObligationsV1::from_bits(
                ReusablePhaseOperationV1::End { storage_count: 0 }.required_obligations(),
            ),
        }),
    )
}

#[test]
fn phase_does_not_hide_atomic_in_reachable_helper() {
    for scope in [
        ExecutionMemoryScopeV1::Workgroup,
        ExecutionMemoryScopeV1::Subgroup,
    ] {
        let mut module = module(scope, 128);
        for function in &mut module.functions {
            let block = &mut function.body.as_mut().unwrap().blocks[0];
            block.operations.insert(0, phase());
            block.operations.push(phase());
        }
        assert!(
            matches!(analyze_execution_capability_atomic_scope_v1(&module),
            Err(ExecutionCapabilityAtomicScopeErrorV1::InsufficientScope { function, .. })
                if function.as_str() == "helper")
        );
    }
}

#[test]
fn phase_is_not_counted_as_atomic_and_does_not_stop_call_traversal() {
    let mut module = module(ExecutionMemoryScopeV1::Device, 128);
    for function in &mut module.functions {
        let block = &mut function.body.as_mut().unwrap().blocks[0];
        block.operations.insert(0, phase());
        block.operations.push(phase());
    }
    let report = analyze_execution_capability_atomic_scope_v1(&module).unwrap();
    assert_eq!(report.checked_kernel_roots(), 1);
    assert_eq!(report.checked_atomic_operations(), 1);
}
