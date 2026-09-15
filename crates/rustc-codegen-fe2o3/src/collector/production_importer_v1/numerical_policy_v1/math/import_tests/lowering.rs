//! Actual registered source -> ranked checks -> replayed SSA -> typed KIR.
//! Uses collected identities and descriptor roots, never synthetic root inputs.
use super::*;
use fe2o3_kernel_ir::{
    ExecutionCapabilityOperationV1 as E, NumericalPolicyMathOperationV1 as M, OperationKind,
    VerifiedCanonicalKernelIrV13,
};
use fe2o3_lower_mir_kernel::{ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

pub(super) fn check(
    imported: crate::collector::ConstructedProductionSemanticMirV1,
    typed_roots: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
) {
    assert_source_context_entry_is_retained(&imported);
    let typed_roots = crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(
        typed_roots,
        &imported.semantic_mir,
    )
    .unwrap();
    crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
        &typed_roots,
        &imported.semantic_mir,
    )
    .unwrap();
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        imported.semantic_mir,
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    ssa.verify_replay().unwrap();
    let expansion = *ssa.execution_expansion().identity();
    let inputs = typed_roots
        .iter()
        .map(|typed| {
            crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                typed.logical_name(),
                typed.kernel_binding_bytes(),
                typed.source_launch().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let ranked = crate::production_ranked_projection_v1::project_and_verify_ranked_semantic_mir_with_contexts_v1(
        ssa,
        &inputs,
        &imported.reference_effect_bindings,
        Some(&imported.kernel_contexts),
    )
    .expect("actual Math source must retain all ranked and refinement checks");
    let contexts = imported
        .kernel_contexts
        .into_lowering_inputs(
            &imported.rustc_identity_inventory,
            &imported.rustc_target,
            ranked.roots(),
            &typed_roots,
        )
        .unwrap();
    let (receipt, verification) = ranked
        .into_verified_roster_receipt()
        .unwrap()
        .into_module_verified_receipt()
        .unwrap();
    assert!(verification.every_functional_verification_is_coherent());
    let lowered =
        ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks_with_kernel_contexts(
            receipt,
            ProductionSemanticKirLimitsV1::default(),
            contexts,
        )
        .expect("actual defined Math bridge, Bind and all 13 FP32 calls must lower");
    lowered.verify_equivalence().unwrap();
    lowered
        .canonical_kernel_ir_v13()
        .unwrap()
        .revalidate()
        .unwrap();
    let mut getter = None;
    let mut bound = None;
    let mut consumers = BTreeSet::new();
    for op in lowered
        .module()
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|b| &b.operations)
    {
        let OperationKind::ExecutionCapability(c) = &op.kind else {
            continue;
        };
        let E::NumericalPolicyMath(math) = c.operation else {
            continue;
        };
        assert_eq!(
            c.source
                .occurrence
                .expect("retain actual source occurrence")
                .expansion_identity(),
            expansion
        );
        match math {
            M::MathDerive { .. } => {
                assert!(getter.replace(op.results[0].id).is_none());
                assert_eq!(c.operands.len(), 1);
            }
            M::Bind { .. } => {
                assert_eq!(c.operands.first(), getter.as_ref());
                assert!(bound.replace(op.results[0].id).is_none());
                assert_eq!(c.operands.len(), 2);
            }
            M::F32 { function, .. } => {
                assert_eq!(c.operands.first(), bound.as_ref());
                assert!(consumers.insert(format!("{function:?}")));
            }
        }
    }
    assert!(getter.is_some() && bound.is_some());
    assert_eq!(consumers.len(), 13);
    for mutation in 0..4 {
        let mut changed = lowered.module().clone();
        let operation = changed
            .functions
            .iter_mut()
            .filter_map(|f| f.body.as_mut())
            .flat_map(|body| &mut body.blocks)
            .flat_map(|b| &mut b.operations)
            .find(|op| {
                matches!(&op.kind, OperationKind::ExecutionCapability(c)
                if matches!(c.operation, E::NumericalPolicyMath(M::Bind { .. })))
            })
            .unwrap();
        let OperationKind::ExecutionCapability(c) = &mut operation.kind else {
            unreachable!()
        };
        match mutation {
            0 => c.source.occurrence = None,
            1 => c.operands.swap(0, 1),
            2 => c.operands[0] = operation.results[0].id,
            _ => {
                c.obligations = fe2o3_kernel_ir::ExecutionSafetyObligationsV1::from_bits(
                    c.obligations.bits()
                        & !fe2o3_kernel_ir::ExecutionSafetyObligationsV1::LIFETIME_VALIDITY,
                )
            }
        }
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(changed).is_err(),
            "accepted source-output mutation {mutation}"
        );
    }
}

fn assert_source_context_entry_is_retained(
    imported: &crate::collector::ConstructedProductionSemanticMirV1,
) {
    let source = &imported.semantic_mir;
    let [root_id] = source.roots() else {
        panic!("one actual registered Math root")
    };
    let context = imported.kernel_contexts.roots.iter()
        .find(|context| context.selected_root == *root_id)
        .expect("the actual root retains its authenticated Context custody");
    assert!(context.entry_transfer.is_some(), "the erased entry must retain its checked source initializer/use relation");
    let root = &source.functions()[root_id.index() as usize];
    let mut issued = None;
    let mut transferred = None;
    for block in root.blocks() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else { continue; };
        match &source.callables()[call.callee().index() as usize] {
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context }, ..
            } => assert!(issued.replace(*context).is_none()),
            SemanticCallableDeclV1::Defined { function }
                if source.functions()[function.index() as usize].identity().as_bytes()
                    == &context.logical_helper_identity => {
                let Some(SemanticOperandV1::Constant(value)) = call.arguments().first() else {
                    panic!("retain rustc's erased operand; do not rewrite the original MIR")
                };
                assert_eq!(value.value(), &SemanticConstantValueV1::ZeroSized);
                assert!(transferred.replace(value.ty()).is_none());
            }
            _ => {}
        }
    }
    assert!(issued.is_some());
    assert_eq!(issued, transferred);
}

#[test]
#[ignore = "requires cached real AMD device/core metadata"]
fn policy_math_all13_source_kir_gfx942() {
    super::run_with_lowering(
        "gfx942",
        "lowering::policy_math_all13_source_kir_gfx942",
        true,
        true,
    );
}

#[test]
#[ignore = "requires cached real AMD device/core metadata"]
fn policy_math_all13_source_kir_gfx950() {
    super::run_with_lowering(
        "gfx950",
        "lowering::policy_math_all13_source_kir_gfx950",
        true,
        true,
    );
}
