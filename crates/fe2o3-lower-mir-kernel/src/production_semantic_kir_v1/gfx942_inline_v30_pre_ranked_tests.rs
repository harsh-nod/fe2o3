//! Exercise the complete normal pre-ranked owner path, including both helper
//! effect analyses and canonical call/return correspondence. The inert fixture
//! supplies shape identities; it does not stand in for rustc authentication.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
};
use fe2o3_pliron::{ProductionSemanticMirLimitsV1, ProductionSemanticSsaLimitsV1};

fn fixture(
    kind: SemanticGfx942InlineInstructionV30,
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
    SemanticInlineAssemblySourceV30,
) {
    let mut fixture = IsaFixture::with_intrinsic_callee(kind, SemanticCallableIdV1::from_index(2));
    let source = SemanticSourceProvenanceV1::unavailable();
    let unit = SemanticTypeIdV1::from_index(1);
    fixture.types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([80; 32]),
        SemanticLayoutIdentityV1::from_sha256([81; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    ));
    let value = u32_abi_value();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([61; 32]),
        SemanticLayoutIdentityV1::from_sha256([62; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        2,
        vec![
            SemanticAbiArgumentV1::source(value.clone()),
            SemanticAbiArgumentV1::source(value),
        ],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let mut callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
    ];
    callables.append(&mut fixture.callables);
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        vec![
            SemanticOperandV1::Copy(place(1, ISA_TYPE)),
            SemanticOperandV1::Copy(place(2, ISA_TYPE)),
        ],
        Some(SemanticCallDestinationV1::new(
            place(3, ISA_TYPE),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let root = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([5; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([63; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([64; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([65; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([66; 32]),
        source,
        abi,
        [
            (unit, SemanticLocalRoleV1::Return),
            (ISA_TYPE, SemanticLocalRoleV1::Argument(0)),
            (ISA_TYPE, SemanticLocalRoleV1::Argument(1)),
            (ISA_TYPE, SemanticLocalRoleV1::Temporary),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (ty, role))| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([70 + index as u8; 32]),
                ty,
                role,
                source,
            )
        })
        .collect(),
        ISA_BLOCK,
        [
            SemanticTerminatorKindV1::Call(call),
            SemanticTerminatorKindV1::Return,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, terminator)| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([75 + index as u8; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, terminator),
            )
            .unwrap()
        })
        .collect(),
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"isa_helper_root".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([67; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let occurrence = fixture.call.inline_assembly_source_v30().unwrap();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        fixture.types,
        vec![],
        vec![],
        vec![],
        vec![root, fixture.function],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(admitted.wire_version(), SemanticMirWireVersionV1::V34);
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "isa_helper_root",
            [67; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    (ssa, launch, occurrence)
}

#[test]
fn six_retained_helpers_survive_normal_pre_ranked_materialization_with_exact_instructions() {
    for kind in ISA_KINDS {
        let (ssa, launch, occurrence) = fixture(kind);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 32 * 1024 * 1024);
        budget.reserve_storage(17).unwrap();
        let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), 17);
        assert_eq!(
            owner.helper_source_policy_v1(),
            ProductionHelperSourcePolicyV1::RawEmpty
        );
        let helper_rows = owner.empty_effect_helpers().iter().collect::<Vec<_>>();
        assert_eq!(helper_rows.len(), 1);
        assert_eq!(
            helper_rows[0].semantic_function(),
            SemanticFunctionIdV1::from_index(1)
        );
        let module = owner.executable().module();
        assert_eq!(module.functions.len(), 2);
        let helper = &module.functions[1];
        assert_eq!(helper.role, fe2o3_kernel_ir::FunctionRole::InternalHelper);
        let operations = helper
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .collect::<Vec<_>>();
        let [instruction] = operations.as_slice() else {
            panic!("helper must retain exactly its ISA instruction: {operations:?}")
        };
        let OperationKind::InlineAssembly(assembly) = &instruction.kind else {
            panic!("ISA must not be rewritten as Binary")
        };
        assert_eq!(assembly.mnemonic, kind.mnemonic());
        assert_eq!(assembly.source.frontend_unit, occurrence.frontend_unit());
        assert_eq!(assembly.source.function, *occurrence.function().as_bytes());
        assert_eq!(assembly.source.contract, occurrence.contract());
        assert_eq!(assembly.source.statement, occurrence.statement());
        assert!(!instruction.has_complete_effect_summary());
        let root = &module.functions[0];
        assert!(root.body.as_ref().unwrap().blocks.iter().flat_map(|block| &block.operations).any(|operation| matches!(&operation.kind, OperationKind::Call { callee, .. } if callee == &helper.id)));
        owner.semantic_ssa().verify_replay().unwrap();
    }
}
