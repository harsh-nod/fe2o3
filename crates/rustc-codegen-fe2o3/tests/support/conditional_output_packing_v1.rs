//! Exercise the host checker with genuine compiler evidence and real typed packers.
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_artifacts::{
    AbiField, AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, Mutability, Name,
    PointerWidth, RustDisjointIndexSpaceV1,
};
use fe2o3_compiler_ffi::{CompilerDescriptorSourceV1, InertSemanticCompilerModuleHandoffV3};
use fe2o3_host::{
    CompilerGeneratedArgumentLayoutV1, ConditionalOutputArgumentBindingV1,
    ConditionalPackedCoverageErrorV1, GeneratedDeviceScalarV1, GeneratedKfdArgumentBinding,
    GeneratedKfdWriteSlice, GeneratedRuntimeArgumentBindingV1, GeneratedRuntimeArgumentBudgetV1,
    GeneratedRuntimeArgumentLimitsV1, GeneratedRuntimeWriteSlice,
};
use fe2o3_verifier::{
    CompilerTargetLineageValidationErrorV1, ValidatedConditionalCompilerProofInputsV1,
};

fn layout(name: &str) -> CompilerGeneratedArgumentLayoutV1 {
    CompilerGeneratedArgumentLayoutV1::new_with_disjoint_index_spaces_v1(
        16,
        8,
        PointerWidth::Bits64,
        vec![
            AbiField::new(
                Name::new(name).unwrap(),
                0,
                16,
                8,
                AbiKind::Slice {
                    element_size: 4,
                    element_alignment: 4,
                },
                Mutability::Mutable,
                Access::WriteOnly,
                AddressSpace::Global,
                u32::disjoint_slice_type_identity_for_index_space_v1(
                    PointerWidth::Bits64,
                    RustDisjointIndexSpaceV1::Index1D,
                ),
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
            )
            .unwrap(),
        ],
        vec![Some(RustDisjointIndexSpaceV1::Index1D)],
    )
    .unwrap()
}

pub fn check_actual_fill_packing(
    proof: &ValidatedConditionalCompilerProofInputsV1,
    handoff: &InertSemanticCompilerModuleHandoffV3,
) {
    let source =
        CompilerDescriptorSourceV1::decode(handoff.capsule().receipts().abi().canonical_preimage())
            .unwrap();
    assert_eq!(source.table().kernels().len(), 1);
    let descriptor = &source.table().kernels()[0];
    assert_eq!(descriptor.entry_name().as_str(), "fill_write_only");
    assert_eq!(descriptor.arguments().len(), 1);
    let generated = layout(descriptor.arguments()[0].name().as_str());
    let binding = ConditionalOutputArgumentBindingV1::from_handoff(proof, handoff, &generated)
        .expect("bind actual conditional proof and canonical ABI to the typed U32 output layout");
    for replace_abi in [false, true] {
        let substituted = substituted_handoff(handoff, replace_abi);
        let Err(ConditionalPackedCoverageErrorV1::TargetLineage(error)) =
            ConditionalOutputArgumentBindingV1::from_handoff(proof, &substituted, &generated)
        else {
            panic!("recomputed inert receipts cannot substitute ABI or proof custody");
        };
        if replace_abi {
            assert!(matches!(
                error,
                CompilerTargetLineageValidationErrorV1::IdentityMismatch { field: "ABI" }
            ));
        } else {
            assert!(matches!(
                error,
                CompilerTargetLineageValidationErrorV1::ProofInputMismatch
            ));
        }
    }
    assert_eq!(
        binding.descriptor_source().canonical_bytes(),
        source.canonical_bytes()
    );
    assert!(!binding.grants_runtime_authority());
    assert!(matches!(
        ConditionalOutputArgumentBindingV1::from_handoff(proof, handoff, &layout("wrong_output")),
        Err(ConditionalPackedCoverageErrorV1::PackingPlan(_))
    ));
    let plan = binding.packing_plan();
    for n in [0_usize, 1, 63, 64, 65, 4097] {
        let grid_x = (n.max(1) as u32).div_ceil(64) * 64;
        let geometry = AqlDispatchGeometryV1::new([grid_x, 1, 1], [64, 1, 1]).unwrap();
        let mut values = vec![0xaaaa_aaaau32; n];
        let output = GeneratedKfdWriteSlice::new(&mut values)
            .bind_argument(plan, 0)
            .unwrap();
        let packed =
            GeneratedKfdArgumentBinding::from_compiler_generated_parts(vec![], vec![output])
                .pack(plan)
                .unwrap();
        let checked = packed
            .check_conditional_coverage(&binding, geometry)
            .unwrap();
        assert_eq!(checked.output_elements(), n as u64);
        assert_eq!(checked.geometry(), geometry);
        assert_eq!(checked.kernel_id(), descriptor.kernel_id());
        assert!(!checked.grants_runtime_authority());
        assert!(!checked.authenticates_device_storage());
        if n > 64 {
            assert!(matches!(
                packed.check_conditional_coverage(
                    &binding,
                    AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap()
                ),
                Err(ConditionalPackedCoverageErrorV1::Underlaunch { .. })
            ));
        }
        let (output, mut observer) =
            GeneratedRuntimeWriteSlice::new(vec![0xaaaa_aaaau32; n].into_boxed_slice());
        let mut budget = GeneratedRuntimeArgumentBudgetV1::new(
            plan,
            GeneratedRuntimeArgumentLimitsV1::new(1 << 20, 1 << 20, 4),
        )
        .unwrap();
        let output = output.bind_argument(plan, 0, &mut budget).unwrap();
        let owned =
            GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], vec![output])
                .pack(plan, budget)
                .unwrap();
        let checked_owned = owned
            .check_conditional_coverage(&binding, geometry)
            .unwrap();
        assert_eq!(checked_owned.output_elements(), checked.output_elements());
        assert_eq!(checked_owned.packing_identity(), checked.packing_identity());
        assert!(
            observer.try_take().unwrap().is_none(),
            "coverage checking cannot publish a result"
        );
    }
}

fn substituted_handoff(
    handoff: &InertSemanticCompilerModuleHandoffV3,
    replace_abi: bool,
) -> InertSemanticCompilerModuleHandoffV3 {
    use fe2o3_compiler_lineage as lineage;
    let original = handoff.capsule().receipts();
    macro_rules! copy {
        ($ty:ident, $field:ident) => {
            lineage::$ty::from_canonical_preimage(original.$field().canonical_preimage()).unwrap()
        };
    }
    // Repair the outer receipt/capsule/pair identities so rejection exercises the retained join.
    let abi = if replace_abi {
        alternate_abi(handoff)
    } else {
        original.abi().canonical_preimage().to_vec()
    };
    let association = lineage::InertProofBindingAssociationV4::decode(
        original.proof_binding().canonical_preimage(),
    )
    .unwrap();
    let proof = if replace_abi {
        original.proof_binding().canonical_preimage().to_vec()
    } else {
        let mut wire = association.verus_execution_evidence().to_vec();
        wire.push(0);
        lineage::InertProofBindingAssociationV4::new(association.inputs(), &wire)
            .unwrap()
            .canonical_bytes()
            .to_vec()
    };
    let receipts = lineage::OrderedInertSemanticLineageReceiptsV3::new(
        copy!(
            InertRustcIdentityInventoryReceiptV3,
            rustc_identity_inventory
        ),
        copy!(InertRustcPreflightPlanReceiptV3, rustc_preflight_plan),
        copy!(InertCanonicalSemanticMirReceiptV3, semantic_mir),
        copy!(InertMiddleEndReceiptV3, middle_end),
        copy!(InertKernelIrReceiptV3, kernel_ir),
        copy!(
            InertMirToKirCorrespondenceReceiptV3,
            mir_to_kir_correspondence
        ),
        copy!(InertFormalMemoryReceiptV3, formal_memory),
        lineage::InertProofBindingReceiptV3::from_canonical_preimage(proof).unwrap(),
        copy!(InertTargetBindingReceiptV3, target_binding),
        copy!(InertDataLayoutReceiptV3, data_layout),
        lineage::InertAbiReceiptV3::from_canonical_preimage(abi).unwrap(),
        copy!(InertExportManifestReceiptV3, export_manifest),
        copy!(InertAmdgpuLoweringReceiptV3, amdgpu_lowering),
        copy!(InertSemanticToLlvmReceiptV3, semantic_to_llvm),
        copy!(
            InertFinalCompilerModuleCommitmentReceiptV3,
            final_compiler_module_commitment
        ),
    );
    let capsule = lineage::InertProductionSemanticCapsuleV3::new(
        handoff.capsule().invocation().clone(),
        handoff.capsule().target(),
        receipts,
    )
    .unwrap();
    let module = fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(
        handoff.module_handoff().canonical_bytes(),
    )
    .unwrap();
    InertSemanticCompilerModuleHandoffV3::new(capsule, module).unwrap()
}

fn alternate_abi(handoff: &InertSemanticCompilerModuleHandoffV3) -> Vec<u8> {
    use fe2o3_kernel_descriptor::{
        DeviceDescriptorTableV1, KernelAbiLayoutV1, KernelDescriptorV1, LogicalArgumentV1,
    };
    let source =
        CompilerDescriptorSourceV1::decode(handoff.capsule().receipts().abi().canonical_preimage())
            .unwrap();
    let table = source.table();
    let kernel = &table.kernels()[0];
    let argument = &kernel.arguments()[0];
    let ty = table
        .type_records()
        .iter()
        .find(|ty| ty.identity() == argument.source_type())
        .unwrap();
    let layout = table
        .layout_records()
        .iter()
        .find(|layout| layout.identity() == argument.device_layout())
        .unwrap();
    let argument = LogicalArgumentV1::disjoint_slice(
        0,
        argument.name().clone(),
        ty,
        layout,
        argument.access(),
        8,
    )
    .unwrap();
    let replacement = KernelDescriptorV1::new(
        kernel.kernel_id(),
        kernel.logical_name().clone(),
        kernel.entry_name().clone(),
        kernel.descriptor_symbol().clone(),
        kernel.source_evidence(),
        kernel.executable_ir_evidence(),
        kernel.capabilities().to_vec(),
        KernelAbiLayoutV1::new(24, 24, 8).unwrap(),
        kernel.launch().clone(),
        vec![argument],
    )
    .unwrap();
    let table = DeviceDescriptorTableV1::new(
        table.canonical_code_object_digest(),
        table.code_object_version(),
        table.compiler().clone(),
        table.producer().clone(),
        table.device_target(),
        table.type_records().to_vec(),
        table.layout_records().to_vec(),
        vec![replacement],
    )
    .unwrap();
    CompilerDescriptorSourceV1::new(table)
        .unwrap()
        .canonical_bytes()
        .to_vec()
}
