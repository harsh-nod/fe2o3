use super::*;
use fe2o3_kernel_ir::{
    ExecutionCapabilityOperationV1 as E, ExecutionCapabilityRoleV1 as R, OperationKind, Type,
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
    let owner = ProductionSemanticMirOwnerV1::try_new(
        imported.semantic_mir,
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(owner, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    ssa.verify_replay().unwrap();
    let expansion = *ssa.execution_expansion().identity();
    let inputs = typed_roots
        .iter()
        .map(|root| {
            crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                root.logical_name(),
                root.kernel_binding_bytes(),
                root.source_launch().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let ranked = crate::production_ranked_projection_v1::project_and_verify_ranked_semantic_mir_v1(
        ssa,
        &inputs,
        &imported.reference_effect_bindings,
    )
    .expect("actual scoped memory must retain ordinary ranked ownership and source evidence");
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
        .expect("retain exact borrowed receiver, Option custody, and moved conversion input");
    lowered.verify_equivalence().unwrap();
    lowered
        .canonical_kernel_ir_v13()
        .unwrap()
        .revalidate()
        .unwrap();
    let module = lowered.module();
    let mut seen = BTreeSet::new();
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        let types = body
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .parameters
                    .iter()
                    .chain(block.operations.iter().flat_map(|op| &op.results))
            })
            .map(|v| (v.id, &v.ty))
            .collect::<BTreeMap<_, _>>();
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            let OperationKind::ExecutionCapability(contract) = &operation.kind else {
                continue;
            };
            let (input, output, issue) = match &contract.operation {
                E::WorkgroupMemoryIndexV2 {
                    workgroup,
                    witness,
                    option,
                    ..
                } => {
                    assert_eq!(contract.signature.output(), *option);
                    (*workgroup, *witness, true)
                }
                E::WorkgroupMemoryIndexIntoDisjoint {
                    input_witness,
                    output_witness,
                } => (*input_witness, *output_witness, false),
                _ => continue,
            };
            let [operand] = contract.operands.as_slice() else {
                panic!("one real source argument");
            };
            let Type::ExecutionCapability(input_type) = types[operand] else {
                panic!("logical source operand");
            };
            assert_eq!(input_type.source_type, input);
            assert_eq!(
                input_type.role,
                if issue {
                    R::Workgroup
                } else {
                    R::WorkgroupMemoryIndex
                }
            );
            let [result] = operation.results.as_slice() else {
                panic!("one exact payload result");
            };
            let Type::ExecutionCapability(output_type) = &result.ty else {
                panic!("scoped result");
            };
            assert_eq!(output_type.source_type, output);
            assert_eq!(output_type.role, R::WorkgroupMemoryIndex);
            assert_eq!(output_type.provenance, input_type.provenance);
            assert_eq!(output_type.workgroup_brand, input_type.workgroup_brand);
            assert_eq!(output_type.epoch, input_type.epoch);
            let occurrence = contract
                .source
                .occurrence
                .expect("original expanded call occurrence");
            assert_eq!(occurrence.expansion_identity(), expansion);
            assert!(seen.insert((function.id.clone(), issue)));
        }
    }
    for kernel in &module.kernels {
        assert!(seen.contains(&(kernel.entry.clone(), true)));
        assert!(seen.contains(&(kernel.entry.clone(), false)));
    }
    for mutation in 0..5 {
        let mut wrong = module.clone();
        let op = wrong
            .functions
            .iter_mut()
            .filter_map(|f| f.body.as_mut())
            .flat_map(|b| &mut b.blocks)
            .flat_map(|b| &mut b.operations)
            .find(|op| {
                matches!(&op.kind, OperationKind::ExecutionCapability(c)
                if matches!(c.operation, E::WorkgroupMemoryIndexIntoDisjoint { .. }))
            })
            .unwrap();
        let OperationKind::ExecutionCapability(c) = &mut op.kind else {
            unreachable!()
        };
        match mutation {
            0 => c.epoch_before.as_mut().unwrap()[0] ^= 1,
            1 => c.workgroup_brand.as_mut().unwrap()[0] ^= 1,
            2 => c.provenance.issuance[0] ^= 1,
            3 => c.operands[0] = op.results[0].id,
            4 => op.results[0].ty = Type::INDEX,
            _ => unreachable!(),
        }
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(wrong).is_err(),
            "reject actual-owner substitution {mutation}"
        );
    }
}
