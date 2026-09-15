use std::collections::BTreeMap;

use dialect_gpu::ExecutionCapabilityOp;
use dialect_kernel::{
    CanonicalIdentityAttr, ExecutionCapabilityType, KernelContextIssueOp, KernelContextType,
    ReturnOp, SemanticExceptionalValueAttr, SemanticIeeeRoundingAttr, SemanticNumericalContractV1,
    SemanticNumericalPolicyAttr, SemanticOverflowAttr, SemanticScalarKindAttr,
    SemanticTypedBinaryKindAttr, SemanticTypedBinaryOp, SemanticTypedConstantOp,
    SemanticTypedExpressionRootOp, SemanticTypedExpressionV1, SemanticTypedScalarV1,
    SourceCoordinateAttr,
};
use dialect_proof::{
    AbsoluteErrorF64BitsAttr, ObligationOp, ProofIdAttr, PropertyAttr, RelativeErrorF64BitsAttr,
    RequireNumericalRefinementOp,
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_amdgcn_model::{
    ProductionTargetLaunchEvidenceV13, lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1,
};
use fe2o3_kernel_analysis::{
    KernelCheckStatusV1, PlironSemanticRefinementFindingV1,
    analyze_execution_capability_final_graph_v1, run_pliron_ranked_bounds_check_v1,
    run_pliron_semantic_refinement_check_v1,
};
use fe2o3_kernel_ir::{
    BinaryOp, Constant, NumericalModeV1, OperationKind, Type, ValueId,
    VerifiedCanonicalKernelIrV13, encode_execution_capability_contract_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{ops::FuncOp, types::FunctionType},
    common_traits::Verify,
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
    value::Value,
};
use sha2::{Digest, Sha256};

use super::fixture;

fn append_root(
    context: &mut Context,
    block: Ptr<BasicBlock>,
    value: Value,
    expression: &SemanticTypedExpressionV1,
    policy: SemanticNumericalPolicyAttr,
) -> Value {
    let contract = SemanticNumericalContractV1 {
        policy,
        rounding: SemanticIeeeRoundingAttr::NearestTiesToEven,
        exceptional_values: SemanticExceptionalValueAttr::PreserveExactBits,
    };
    let digest = expression.canonical_transcript_sha256(contract);
    let words = std::array::from_fn(|index| {
        u64::from_le_bytes(digest[index * 8..index * 8 + 8].try_into().unwrap())
    });
    let root = SemanticTypedExpressionRootOp::new(
        context,
        value,
        contract.policy,
        contract.rounding,
        contract.exceptional_values,
        words,
    );
    root.get_operation().insert_at_back(block, context);
    root.result(context)
}

#[test]
fn unused_numerical_policy_cannot_certify_a_different_live_fp_tree() {
    let mut observed = Vec::new();
    for with_policy in [false, true] {
        let module = fixture::floating_point_module(with_policy, NumericalModeV1::StrictIeee);
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let issuance = analyze_execution_capability_final_graph_v1(&canonical, &module, 7).unwrap();
        assert_eq!(issuance.status(), KernelCheckStatusV1::Clean);
        assert!(!issuance.grants_proof_machine_artifact_or_launch_authority());
        let evidence =
            ProductionTargetLaunchEvidenceV13::for_static_launches(&canonical, 7).unwrap();
        let lowered = lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
            &canonical,
            7,
            &evidence,
            ProductionAmdTargetProfileV1::Gfx942,
        )
        .unwrap();
        assert!(!lowered.grants_load_authority());
        assert!(!lowered.grants_launch_authority());
        assert!(!lowered.has_complete_operational_translation_derivation());

        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        dialect_gpu::register_dialect(&mut context).unwrap();
        dialect_proof::register_dialect(&mut context).unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(&mut context, "entry".try_into().unwrap(), signature);
        let entry = function.get_entry_block(&context);
        let logical = FuncOp::new(
            &mut context,
            "logical_transport".try_into().unwrap(),
            signature,
        );
        let logical_entry = logical.get_entry_block(&context);
        let scalar = SemanticTypedScalarV1::new(SemanticScalarKindAttr::Float, 32).unwrap();
        let mut values = BTreeMap::<ValueId, Value>::new();
        let mut expressions = BTreeMap::<ValueId, SemanticTypedExpressionV1>::new();
        let mut policy_count = 0;
        // Raw execution ops are not ranked memory ops. Retain their exact
        // payload separately and project only scalar operations for refinement.
        // Production lowering above independently rejects any non-dead token use.
        for (index, operation) in module.functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .iter()
            .enumerate()
        {
            let result = &operation.results[0];
            let value = match &operation.kind {
                OperationKind::KernelContextIssue(issue) => {
                    let Type::KernelContext(ty) = &result.ty else {
                        unreachable!()
                    };
                    let result_type = KernelContextType::get(
                        &context,
                        ty.root().as_str(),
                        CanonicalIdentityAttr::from_bytes(*ty.kernel_marker()),
                        CanonicalIdentityAttr::from_bytes(*ty.target()),
                        CanonicalIdentityAttr::from_bytes(*ty.launch()),
                    );
                    let source = issue.source();
                    let issued = KernelContextIssueOp::new(
                        &mut context,
                        result_type,
                        CanonicalIdentityAttr::from_bytes(*canonical.identity().digest()),
                        SourceCoordinateAttr::new(0, 0, index as u32),
                        CanonicalIdentityAttr::from_bytes(source.contract()),
                        CanonicalIdentityAttr::from_bytes(source.frontend_unit()),
                        CanonicalIdentityAttr::from_bytes(source.function()),
                        CanonicalIdentityAttr::from_bytes(source.contract()),
                        CanonicalIdentityAttr::from_bytes(source.issuance()),
                    );
                    issued.verify(&context).unwrap();
                    issued
                        .get_operation()
                        .insert_at_back(logical_entry, &context);
                    issued.get_operation().deref(&context).get_result(0)
                }
                OperationKind::ExecutionCapability(contract) => {
                    let Type::ExecutionCapability(ty) = &result.ty else {
                        unreachable!()
                    };
                    let result_type = ExecutionCapabilityType::get(&context, ty).unwrap().into();
                    let identity =
                        Sha256::digest(encode_execution_capability_contract_v1(contract).unwrap())
                            .into();
                    let issued = ExecutionCapabilityOp::new(
                        &mut context,
                        contract,
                        contract.operands.iter().map(|id| values[id]).collect(),
                        vec![result_type],
                        CanonicalIdentityAttr::from_bytes(*canonical.identity().digest()),
                        SourceCoordinateAttr::new(0, 0, index as u32),
                        CanonicalIdentityAttr::from_bytes(identity),
                    )
                    .unwrap();
                    issued.verify(&context).unwrap();
                    assert_eq!(issued.contract(&context).as_ref(), Some(contract));
                    issued
                        .get_operation()
                        .insert_at_back(logical_entry, &context);
                    policy_count += 1;
                    issued.get_operation().deref(&context).get_result(0)
                }
                OperationKind::Constant(Constant::F32Bits(bits)) => {
                    let constant =
                        SemanticTypedConstantOp::new(&mut context, u64::from(*bits), scalar);
                    constant.get_operation().insert_at_back(entry, &context);
                    expressions.insert(
                        result.id,
                        SemanticTypedExpressionV1::Constant {
                            scalar,
                            bits: u64::from(*bits),
                        },
                    );
                    constant.result(&context)
                }
                OperationKind::Binary { op, lhs, rhs } => {
                    let kind = match op {
                        BinaryOp::Multiply => SemanticTypedBinaryKindAttr::Multiply,
                        BinaryOp::Add => SemanticTypedBinaryKindAttr::Add,
                        _ => panic!("unexpected FP operator"),
                    };
                    let binary = SemanticTypedBinaryOp::new(
                        &mut context,
                        kind,
                        SemanticOverflowAttr::Wrapping,
                        scalar,
                        values[lhs],
                        values[rhs],
                    );
                    binary.get_operation().insert_at_back(entry, &context);
                    expressions.insert(
                        result.id,
                        SemanticTypedExpressionV1::Binary {
                            operation: kind,
                            scalar,
                            overflow: SemanticOverflowAttr::Wrapping,
                            lhs: Box::new(expressions[lhs].clone()),
                            rhs: Box::new(expressions[rhs].clone()),
                        },
                    );
                    binary.result(&context)
                }
                _ => panic!("unexpected FP fixture operation"),
            };
            values.insert(result.id, value);
        }
        assert_eq!(policy_count, usize::from(with_policy));
        ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(logical_entry, &context);
        let ieee = SemanticNumericalPolicyAttr::ExactIeeeNearestTiesToEvenPreserveBits;
        let actual = append_root(
            &mut context,
            entry,
            values[&ValueId(6)],
            &expressions[&ValueId(6)],
            ieee,
        );
        let reference = append_root(
            &mut context,
            entry,
            values[&ValueId(5)],
            &expressions[&ValueId(5)],
            ieee,
        );
        let boolean = SemanticTypedScalarV1::new(SemanticScalarKindAttr::Bool, 1).unwrap();
        let truth = SemanticTypedConstantOp::new(&mut context, 1, boolean);
        truth.get_operation().insert_at_back(entry, &context);
        let truth_value = truth.result(&context);
        let domain = append_root(
            &mut context,
            entry,
            truth_value,
            &SemanticTypedExpressionV1::Constant {
                scalar: boolean,
                bits: 1,
            },
            SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence,
        );
        let obligation = ProofIdAttr::new([1, 2, 3, 4]);
        let declaration = ObligationOp::new(
            &mut context,
            obligation.clone(),
            ProofIdAttr::new([5, 6, 7, 8]),
            ProofIdAttr::new([9, 10, 11, 12]),
            PropertyAttr::FunctionalRefinement,
        );
        declaration.get_operation().insert_at_back(entry, &context);
        let requirement = RequireNumericalRefinementOp::new(
            &mut context,
            obligation,
            AbsoluteErrorF64BitsAttr(0.001_f64.to_bits()),
            RelativeErrorF64BitsAttr(0.01_f64.to_bits()),
            actual,
            reference,
            domain,
            domain,
        );
        requirement.get_operation().insert_at_back(entry, &context);
        ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(entry, &context);
        // Deliberately no EvidenceRefOp: issuing a policy is not evidence for this obligation.
        let bounds = run_pliron_ranked_bounds_check_v1(&context, &function);
        assert!(bounds.is_clean(), "{:?}", bounds.findings());
        let report = run_pliron_semantic_refinement_check_v1(&context, &function);
        assert_eq!(
            report.status(),
            KernelCheckStatusV1::Incomplete,
            "{:?}",
            report.findings()
        );
        assert_eq!(
            report.numerical_obligation_count(),
            1,
            "{:?}",
            report.findings()
        );
        assert_eq!(report.policy_checked_numerical_obligation_count(), 0);
        assert!(report.numerical_certificates().is_empty());
        assert!(report.findings().iter().any(|finding| matches!(
            finding, PlironSemanticRefinementFindingV1::NumericalProofIncomplete { actual, reference, reason, .. }
                if actual != reference
                    && *reason == "V1 derives a finite bound only from identical typed IEEE operator trees"
        )));
        assert!(report.findings().iter().any(|finding| matches!(
            finding, PlironSemanticRefinementFindingV1::ReferenceContractIncomplete { reason, .. }
                if *reason == "the exact proof.evidence_ref record is missing"
        )));
        assert_eq!(report.findings().len(), 2, "{:?}", report.findings());
        observed.push((
            report.status(),
            report.numerical_obligation_count(),
            report.policy_checked_numerical_obligation_count(),
            report.numerical_certificates().len(),
            module.required_capabilities,
            lowered.llvm_ir().to_owned(),
        ));
    }
    assert_eq!(observed[0], observed[1]);
}
