use super::*;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::*;
use fe2o3_compiler_lineage::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_rustc_invocation::{CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcUnitV2};
use std::ffi::OsString;

// Content-only fixture: deliberately not admitted source/proof or rustc custody.
fn fixture(target: &str) -> (Base, CompilerModuleHandoffV2, Vec<u8>) {
    let environment = CompileEnvironmentV2::from_child_environment(
        [
            ("CARGO_CFG_TARGET_ARCH", "amdgcn"),
            ("FE2O3_HSACO_DIR", "/workspace/output"),
            ("FE2O3_TARGET", target),
            ("FE2O3_VERIFY_KERNEL_IR", "1"),
        ]
        .map(|(k, v)| (OsString::from(k), OsString::from(v))),
    )
    .unwrap();
    let rustc = RustcUnitV2::new(
        "/workspace",
        vec![
            "/opt/rustc".into(),
            "--crate-name=native_v4_test".into(),
            "input.rs".into(),
            "--crate-type=lib".into(),
            "-Zcodegen-backend=/opt/backend.so".into(),
        ],
    )
    .unwrap();
    let invocation = RustcInvocationDescriptorV3::new(
        RustcInvocationDescriptorV2::new([4; 32], [6; 32], rustc, environment).unwrap(),
        CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap(),
    )
    .unwrap();
    let target = DeviceTargetV1::parse(target).unwrap();
    let module = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .unwrap(),
        CompilerModuleSymbolManifestV1::new([
            (CompilerModuleSymbolRoleV1::KernelEntry, "kernel"),
            (CompilerModuleSymbolRoleV1::KernelDescriptor, "kernel.kd"),
        ])
        .unwrap(),
        b"define amdgpu_kernel void @kernel() { ret void }\n",
    )
    .unwrap();
    let commitment = InertFinalCompilerModuleCommitmentV3::from_handoff(&module).unwrap();
    macro_rules! inert {
        ($t:ty) => {
            <$t>::from_canonical_preimage(b"opaque fixture".to_vec()).unwrap()
        };
    }
    let receipts = OrderedInertSemanticLineageReceiptsV3::new(
        inert!(InertRustcIdentityInventoryReceiptV3),
        inert!(InertRustcPreflightPlanReceiptV3),
        inert!(InertCanonicalSemanticMirReceiptV3),
        inert!(InertMiddleEndReceiptV3),
        inert!(InertKernelIrReceiptV3),
        inert!(InertMirToKirCorrespondenceReceiptV3),
        inert!(InertFormalMemoryReceiptV3),
        inert!(InertProofBindingReceiptV3),
        inert!(InertTargetBindingReceiptV3),
        inert!(InertDataLayoutReceiptV3),
        inert!(InertAbiReceiptV3),
        inert!(InertExportManifestReceiptV3),
        inert!(InertAmdgpuLoweringReceiptV3),
        inert!(InertSemanticToLlvmReceiptV3),
        InertFinalCompilerModuleCommitmentReceiptV3::from_canonical_preimage(
            commitment.canonical_bytes(),
        )
        .unwrap(),
    );
    let base = Base::new(invocation, target, receipts).unwrap();
    let layout = CarrierLayout::new::<()>(19, 64).unwrap();
    let mut carrier = vec![31; layout.encoded_len()];
    seal_native_refined_forwarding_carrier_v1(layout, &mut carrier, MAX_STORAGE, |_| {
        Ok::<_, ()>(())
    })
    .unwrap();
    (base, module, carrier)
}

#[test]
fn native_capsule_v4_reuses_spare_capacity_and_preserves_all_members() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        let (base, module, mut carrier) = fixture(target);
        let original = carrier.clone();
        let capsule =
            CapsuleLayout::new::<()>(base.canonical_bytes().len(), carrier.len()).unwrap();
        let layout =
            HandoffLayout::new(capsule.encoded_len(), module.canonical_bytes().len()).unwrap();
        carrier.reserve_exact(layout.encoded_len() + 8192);
        let pointer = carrier.as_ptr();
        let capacity = carrier.capacity();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, MAX_STORAGE);
        let floor = capacity + base.canonical_bytes().len() + module.canonical_bytes().len() + 73;
        budget.reserve_storage(floor).unwrap();
        let bytes = scoped(&mut budget, |b| pack(carrier, &base, &module, b)).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(bytes.capacity(), capacity);
        assert_eq!(bytes.as_ptr(), pointer);
        budget.reserve_storage(DECODE_STORAGE).unwrap();
        budget
            .charge_work(
                inert_semantic_compiler_module_handoff_decode_work_v4(bytes.len()).unwrap(),
            )
            .unwrap();
        let decoded = Handoff::decode_owned(bytes).unwrap();
        assert_eq!(decoded.canonical_bytes().as_ptr(), pointer);
        assert_eq!(decoded.capsule().carrier_bytes(), original);
        assert_eq!(decoded.capsule().base(), &base);
        assert_eq!(decoded.module_handoff(), &module);
        assert!(!decoded.grants_authority());
        drop(decoded);
        budget.release_storage(DECODE_STORAGE).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn native_capsule_v4_packing_exact_resources_and_one_short_restore_siblings() {
    let run = |work_limit, storage_limit| {
        let (base, module, carrier) = fixture("gfx942:xnack-");
        let floor =
            carrier.capacity() + base.canonical_bytes().len() + module.canonical_bytes().len() + 37;
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = scoped(&mut budget, |b| pack(carrier, &base, &module, b));
        assert_eq!(budget.storage(), floor);
        (result.is_ok(), budget.work(), budget.peak_storage())
    };
    let (ok, work, peak) = run(usize::MAX, MAX_STORAGE);
    assert!(ok);
    assert_eq!(run(work, peak), (true, work, peak));
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
}

#[test]
fn native_capsule_v4_extraction_cannot_supply_protected_invocation() {
    assert!(matches!(
        invocation(&ProductionCompilerCustody::ExtractionOnly),
        Err(E::Mismatch(
            "native capsule requires retained invocation custody"
        ))
    ));
    assert!(
        size_of::<PreparedRefinedForwardingCapsuleV4>()
            >= size_of::<PreparedRefinedForwardingWireV1>()
    );
}

#[test]
fn native_capsule_v4_decode_prepays_metadata_and_work_before_bad_content() {
    let (base, module, carrier) = fixture("gfx942:xnack-");
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget
        .reserve_storage(carrier.capacity() + 4 * 1024 * 1024)
        .unwrap();
    let bytes = scoped(&mut budget, |b| pack(carrier, &base, &module, b)).unwrap();
    let debit = inert_semantic_compiler_module_handoff_decode_work_v4(bytes.len()).unwrap();
    for (extra_work, extra_storage, corrupt) in
        [(0, 0, false), (0, 0, true), (0, 1, true), (1, 0, true)]
    {
        let mut bytes = bytes.clone();
        if corrupt {
            bytes[0] ^= 1;
        }
        let floor = bytes.capacity() + 37;
        let mut work = Work::new(debit - extra_work);
        let mut budget = Budget::new(&mut work, floor + DECODE_STORAGE - extra_storage);
        budget.reserve_storage(floor).unwrap();
        let result = scoped(&mut budget, |b| decode(bytes, b).map(drop));
        assert_eq!(budget.storage(), floor);
        if extra_storage != 0 {
            assert!(matches!(result, Err(E::Resource(Resource::Storage(_)))));
            assert_eq!(budget.work(), 0);
        } else if extra_work != 0 {
            assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
            assert_eq!(budget.work(), 0);
        } else {
            assert_eq!(budget.work(), debit);
            if corrupt {
                assert!(matches!(result, Err(E::Handoff(_))));
            } else {
                result.unwrap();
            }
        }
    }
}

impl PreparedRefinedForwardingWireV1 {
    /// Called only by the genuine signed compiler fixture. The synthetic
    /// invocation exercises content construction, never protected custody.
    pub(crate) fn check_native_capsule_content_for_test_v4(
        &self,
        budget: &mut Budget<'_>,
    ) -> R<()> {
        scoped(budget, |budget| {
            budget.reserve_storage(4 * 1024 * 1024)?;
            let (synthetic, module, carrier) = fixture(self.live.native.profile.device_target());
            drop(module);
            drop(carrier);
            assert!(matches!(
                invocation(&self.live.bindings.transaction.compiler_custody),
                Err(E::Mismatch(
                    "native capsule requires retained invocation custody"
                ))
            ));
            let actual = base::build(
                &self.live,
                self.carrier_bytes(),
                synthetic.invocation(),
                budget,
            )?;
            let receipts = actual.receipts();
            budget.reserve_storage(READ_STORAGE + CARRIER_STORAGE)?;
            let limit = budget.storage_limit();
            let carrier =
                read_native_refined_forwarding_carrier_v1(self.carrier_bytes(), limit, |w| {
                    budget.charge_work(w)
                })
                .map_err(E::Carrier)?;
            let frame = read_inert_refined_forwarding_output_v1(carrier.output(), limit, |w| {
                budget.charge_work(w)
            })
            .map_err(E::Framing)?;
            for (receipt, field) in [
                (
                    receipts.semantic_mir().canonical_preimage(),
                    Field::SemanticMir,
                ),
                (
                    receipts.middle_end().canonical_preimage(),
                    Field::OriginalMiddleEnd,
                ),
                (
                    receipts.kernel_ir().canonical_preimage(),
                    Field::OriginalNative,
                ),
                (
                    receipts.mir_to_kir_correspondence().canonical_preimage(),
                    Field::OriginalCorrespondence,
                ),
                (
                    receipts.formal_memory().canonical_preimage(),
                    Field::OriginalFormalMemory,
                ),
                (
                    receipts.proof_binding().canonical_preimage(),
                    Field::OriginalInputV4,
                ),
                (receipts.abi().canonical_preimage(), Field::Descriptor),
            ] {
                same(receipt, frame.field(field), budget)?;
            }
            codec::<InertProofBindingAssociationV4>(
                receipts.proof_binding().canonical_preimage().len(),
                budget,
            )?;
            let proof = InertProofBindingAssociationV4::decode(
                receipts.proof_binding().canonical_preimage(),
            )
            .unwrap();
            same(
                proof.verus_execution_evidence(),
                frame.field(Field::OriginalVerus),
                budget,
            )?;
            same(
                receipts.rustc_preflight_plan().canonical_preimage(),
                self.live
                    .bindings
                    .rustc_preflight_plan
                    .canonical_transcript(),
                budget,
            )?;
            assert_eq!(
                receipts.kernel_ir().canonical_preimage(),
                original_native(&self.live)
            );
            assert_eq!(
                receipts.rustc_identity_inventory().canonical_preimage(),
                self.live
                    .bindings
                    .rustc_identity_inventory
                    .canonical_transcript()
            );
            let target = MultiRootTargetBindingTranscriptV3::decode(
                receipts.target_binding().canonical_preimage(),
            )
            .unwrap();
            let bound = self
                .live
                .history_inputs()
                .prefix
                .prefix
                .prefix
                .prefix
                .input
                .canonical()
                .identity();
            assert_eq!(target.target_bound_kir().sha256(), *bound.digest());
            assert_eq!(
                target.target_bound_kir().byte_len(),
                bound.canonical_length()
            );
            assert_eq!(
                target.native_neutral_subject(),
                &subject(
                    self.live.original().map_err(E::Live)?,
                    self.live.source.catalog()
                )?
            );
            assert_eq!(
                target.protected_rustc_invocation().sha256(),
                actual.invocation_digest().into_bytes()
            );
            let observed_layout = self.live.bindings.rustc_target.rustc_layout();
            codec::<DataLayoutTranscriptV3>(
                receipts.data_layout().canonical_preimage().len(),
                budget,
            )?;
            let data_layout =
                DataLayoutTranscriptV3::decode(receipts.data_layout().canonical_preimage())
                    .unwrap();
            let data_layout = data_layout.inputs().unwrap();
            assert_eq!(data_layout.rustc_llvm_target, observed_layout.llvm_target());
            assert_eq!(
                data_layout.live_rustc_data_layout,
                observed_layout.data_layout()
            );
            assert_eq!(
                data_layout.default_pointer_width_bits,
                observed_layout.default_pointer_width_bits()
            );
            assert_eq!(
                data_layout.final_llvm_data_layout,
                crate::production_target_v1::PRODUCTION_WORKER_DATA_LAYOUT_V1
            );
            let lowering = InertNativeLoweringAssociationV1::decode(
                receipts.amdgpu_lowering().canonical_preimage(),
            )
            .unwrap();
            assert_eq!(
                lowering.inputs().final_native,
                subject(self.live.output(), self.live.source.catalog())?
            );
            assert_eq!(
                lowering.inputs().descriptor.sha256(),
                *self.live.native_output_parts().1.identity().sha256()
            );
            assert_eq!(
                lowering.inputs().module_handoff.sha256(),
                *self.live.native_output_parts().0.identity().sha256()
            );
            assert_eq!(lowering.inputs().profile, self.live.native.profile);
            budget.charge_work(self.live.native.llvm_ir().len())?;
            use sha2::{Digest, Sha256};
            let prefix_digest: [u8; 32] =
                Sha256::digest(self.live.native.llvm_ir().as_bytes()).into();
            assert_eq!(
                lowering.inputs().pre_descriptor_llvm.sha256(),
                prefix_digest
            );
            assert_eq!(
                lowering.inputs().pre_descriptor_llvm.byte_len(),
                self.live.native.llvm_ir().len() as u64
            );
            assert_eq!(
                lowering.inputs().carrier.sha256(),
                *carrier.identity().sha256()
            );
            assert_eq!(
                lowering.inputs().carrier.byte_len(),
                carrier.identity().byte_len()
            );
            let (native, descriptor, _) = self.live.native_output_parts();
            assert_eq!(
                lowering.inputs().descriptor.byte_len(),
                descriptor.identity().byte_len()
            );
            assert_eq!(
                lowering.inputs().module_handoff.byte_len(),
                native.identity().byte_len()
            );
            assert_eq!(
                lowering.inputs().final_llvm.sha256(),
                *native.module_identity().sha256()
            );
            assert_eq!(
                lowering.inputs().final_llvm.byte_len(),
                native.module_identity().byte_len()
            );
            same(
                receipts.export_manifest().canonical_preimage(),
                native.symbol_manifest().canonical_bytes(),
                budget,
            )?;
            codec::<InertFinalCompilerModuleCommitmentV3>(
                MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3,
                budget,
            )?;
            for bytes in [
                native.module_bytes(),
                native.envelope().canonical_bytes(),
                native.symbol_manifest().canonical_bytes(),
                native.canonical_bytes(),
            ] {
                budget.charge_work(bytes.len())?;
            }
            let commitment = InertFinalCompilerModuleCommitmentV3::from_handoff(native).unwrap();
            same(
                receipts
                    .final_compiler_module_commitment()
                    .canonical_preimage(),
                commitment.canonical_bytes(),
                budget,
            )?;
            codec::<SemanticToLlvmAssociationTranscriptV3>(
                receipts.semantic_to_llvm().canonical_preimage().len(),
                budget,
            )?;
            let chain = SemanticToLlvmAssociationTranscriptV3::decode(
                receipts.semantic_to_llvm().canonical_preimage(),
            )
            .unwrap();
            let chain = chain.inputs().unwrap();
            macro_rules! axis {
                ($field:ident, $receipt:ident) => {{
                    let identity = receipts.$receipt().identity();
                    assert_eq!(chain.$field.sha256(), *identity.sha256());
                    assert_eq!(chain.$field.byte_len(), identity.byte_len());
                }};
            }
            axis!(semantic_mir, semantic_mir);
            axis!(middle_end, middle_end);
            axis!(kernel_ir, kernel_ir);
            axis!(mir_to_kir_correspondence, mir_to_kir_correspondence);
            axis!(formal_memory, formal_memory);
            axis!(proof_binding, proof_binding);
            axis!(target_binding, target_binding);
            axis!(data_layout, data_layout);
            axis!(abi, abi);
            axis!(export_manifest, export_manifest);
            axis!(amdgpu_lowering, amdgpu_lowering);
            axis!(
                final_compiler_module_commitment,
                final_compiler_module_commitment
            );
            assert_eq!(
                chain.final_llvm.sha256(),
                *native.module_identity().sha256()
            );
            assert_eq!(
                chain.final_llvm.byte_len(),
                native.module_identity().byte_len()
            );
            assert!(!lowering.grants_authority());
            Ok(())
        })
    }
}
