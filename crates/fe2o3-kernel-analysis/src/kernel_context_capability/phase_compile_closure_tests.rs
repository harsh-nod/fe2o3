//! Collector-domain tests; raw tokens do not confer canonical or source authority.
use super::{
    KernelContextUseLocationV1, MAX_REQUIREMENT_CLOSURE_WORK_V1, RequirementClosureBudget, capture,
    protected_facts,
};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, ExecutionCapabilityProvenanceV1, ExecutionSafetyObligationsV1,
    ExecutionTypeIdentityV1, Function, FunctionId, KernelContextIssueV1,
    KernelContextSourceIdentityV1, KernelContextTypeV1, Module, Operation, OperationKind,
    PhaseKeyV1, PhaseLoanStateV1, PhaseOperationSourceV1, ReusablePhaseOpV1,
    ReusablePhaseOperationV1, ReusablePhaseTokenRoleV1, ReusablePhaseTokenTypeV1, Signature,
    Terminator, Type, ValueDef, ValueId,
};

fn token() -> ReusablePhaseTokenTypeV1 {
    ReusablePhaseTokenTypeV1 {
        provenance: ExecutionCapabilityProvenanceV1 {
            root: FunctionId::new("entry"),
            kernel_binding: [1; 32],
            frontend_unit: [2; 32],
            kernel_marker: [3; 32],
            target_brand: [4; 32],
            launch_brand: [5; 32],
            issuance: [6; 32],
        },
        phase: PhaseKeyV1::from_untrusted_bytes([7; 32]),
        owner_source: ExecutionTypeIdentityV1::new([8; 32]),
        owner_anchor_epoch: [9; 32],
        outer_brand: [10; 32],
        phase_brand: [11; 32],
        initial_epoch: [12; 32],
        role: ReusablePhaseTokenRoleV1::OwnerLoan(PhaseLoanStateV1::Active),
    }
}

fn module() -> Module {
    let token = token();
    assert!(token.is_complete());
    let context = KernelContextTypeV1::new("entry", [1; 32], [2; 32], [3; 32]);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        vec![ValueDef::new(
            ValueId(2),
            Type::KernelContext(context.clone()),
        )],
        OperationKind::KernelContextIssue(KernelContextIssueV1::new(
            KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]),
        )),
    ));
    block.operations.push(Operation::new(
        vec![],
        OperationKind::ReusablePhase(ReusablePhaseOpV1 {
            operands: vec![ValueId(0), ValueId(1), ValueId(2)],
            operation: ReusablePhaseOperationV1::End { storage_count: 0 },
            provenance: token.provenance.clone(),
            source: PhaseOperationSourceV1::WrapperEnd {
                phase: token.phase,
                wrapper_normal_target: 0,
                source_protocol: [8; 32],
            },
            obligations: ExecutionSafetyObligationsV1::from_bits(
                ReusablePhaseOperationV1::End { storage_count: 0 }.required_obligations(),
            ),
        }),
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(0), ValueId(1)],
    });
    let mut module = Module::new("raw-phase-context-scan");
    module.functions.push(Function::internal_helper(
        "entry",
        Signature::new(
            vec![
                Type::ReusablePhaseToken(token),
                Type::KernelContext(context),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module
}

#[test]
fn phase_token_is_not_context_and_does_not_hide_real_context_uses() {
    let facts = protected_facts(
        &module(),
        &mut RequirementClosureBudget::new(MAX_REQUIREMENT_CLOSURE_WORK_V1),
    )
    .unwrap();
    assert_eq!(facts.context_types.len(), 2);
    assert_eq!(facts.issuances.len(), 1);
    assert!(facts.execution_types.is_empty());
    assert!(facts.execution_operations.is_empty());
    assert_eq!(
        facts
            .context_uses
            .iter()
            .map(|fact| (fact.value(), fact.location().clone()))
            .collect::<Vec<_>>(),
        vec![
            (
                ValueId(1),
                KernelContextUseLocationV1::OperationOperand {
                    block: BlockId(0),
                    operation: 1,
                    operand: 1
                }
            ),
            (
                ValueId(2),
                KernelContextUseLocationV1::OperationOperand {
                    block: BlockId(0),
                    operation: 1,
                    operand: 2
                }
            ),
            (
                ValueId(1),
                KernelContextUseLocationV1::TerminatorOperand {
                    block: BlockId(0),
                    operand: 1
                }
            ),
        ]
    );
}

#[test]
fn raw_phase_scan_does_not_gain_v13_preservation_admission() {
    assert!(capture(&module()).is_err());
}
