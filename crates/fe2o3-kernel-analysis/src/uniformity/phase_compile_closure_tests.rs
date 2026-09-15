//! Raw classifier tests, not canonical phase/lifecycle admission.
use super::{AnalysisReport, Analyzer, Variation};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Constant, ExecutionCapabilityProvenanceV1, ExecutionSafetyObligationsV1,
    Function, FunctionId, Operation, OperationKind, PhaseKeyV1, PhaseOperationSourceV1,
    ReusablePhaseOpV1, ReusablePhaseOperationV1, ScalarType, Signature, Terminator, Type, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn phase_authority_is_varying_even_with_uniform_operands() {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::internal_helper(
        "entry",
        Signature::new(vec![Type::Scalar(ScalarType::U32); 2], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    let phase = Operation::new(
        vec![],
        OperationKind::ReusablePhase(ReusablePhaseOpV1 {
            operands: vec![ValueId(0), ValueId(1)],
            operation: ReusablePhaseOperationV1::End { storage_count: 0 },
            provenance: ExecutionCapabilityProvenanceV1 {
                root: FunctionId::new("entry"),
                kernel_binding: [1; 32],
                frontend_unit: [2; 32],
                kernel_marker: [3; 32],
                target_brand: [4; 32],
                launch_brand: [5; 32],
                issuance: [6; 32],
            },
            source: PhaseOperationSourceV1::WrapperEnd {
                phase: PhaseKeyV1::from_untrusted_bytes([7; 32]),
                wrapper_normal_target: 0,
                source_protocol: [8; 32],
            },
            obligations: ExecutionSafetyObligationsV1::from_bits(
                ReusablePhaseOperationV1::End { storage_count: 0 }.required_obligations(),
            ),
        }),
    );
    assert!(!phase.has_complete_effect_summary());
    let empty = BTreeSet::new();
    for variation in [
        Variation::GridUniform,
        Variation::WorkgroupUniform,
        Variation::Varying,
    ] {
        let parameters = [variation; 2];
        let analyzer = Analyzer::new(
            &function,
            function.body.as_ref().unwrap(),
            AnalysisReport {
                function: function.id.clone(),
                values: BTreeMap::from([(ValueId(0), variation), (ValueId(1), variation)]),
                block_controls: BTreeMap::new(),
                diagnostics: vec![],
            },
            &parameters,
            &empty,
            &empty,
            None,
        );
        assert_eq!(analyzer.value(ValueId(0)), variation);
        assert_eq!(analyzer.operation_variation(&phase), Variation::Varying);
        assert_eq!(
            analyzer.operation_variation(&Operation::new(
                vec![],
                OperationKind::Constant(Constant::U32(0)),
            )),
            Variation::GridUniform,
        );
    }
}
