use super::*;
use crate::collector::production_importer_v1::{
    rust_execution_brand_v1, rust_kernel_brand_v1, rust_same_kernel_brand_v1,
    rust_shared_reference_v1, rust_trusted_adt_type_arguments_v1, rust_workgroup_capability_v1,
    rust_workgroup_lds_v1,
};
use rustc_hir::def::DefKind;
use rustc_middle::mir::TerminatorKind;
use rustc_middle::ty::{EarlyBinder, Instance, Ty, TyKind, TypingEnv};
use std::collections::BTreeMap;

struct PhaseProbe {
    completed: bool,
}

impl Callbacks for PhaseProbe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("reusable_phase_import_source.rs".into()),
            input: include_str!("reusable_phase_source.rs").into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(
            crate::collector::session_crate_binding(tcx),
            Some(registration_binding_v1())
        );
        let target = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        )
        .unwrap();
        let mut expected_reads = Vec::new();
        let spoof = tcx
            .iter_local_def_id()
            .find(|id| {
                tcx.def_kind(*id) == DefKind::Struct
                    && tcx.item_name(id.to_def_id()).as_str() == "ReusableWorkgroupBrand"
            })
            .unwrap();
        // Registered terminals remain live call sites, not collected function bodies.
        let mut read_instances = BTreeMap::new();
        for function in &closure.collection.functions {
            let production_mir =
                crate::production_rustc_intrinsic_v1::production_mir_v1(tcx, function.instance);
            let body = production_mir.body();
            for data in body.basic_blocks.iter() {
                let Some(terminator) = &data.terminator else {
                    continue;
                };
                let TerminatorKind::Call { func, .. } = &terminator.kind else {
                    continue;
                };
                let callable = function
                    .instance
                    .try_instantiate_mir_and_normalize_erasing_regions(
                        tcx,
                        TypingEnv::fully_monomorphized(),
                        EarlyBinder::bind(func.ty(body, tcx)),
                    )
                    .expect("collected source call must normalize under its exact caller");
                let TyKind::FnDef(def_id, arguments) = callable.kind() else {
                    continue;
                };
                let instance = Instance::try_resolve(
                    tcx,
                    TypingEnv::fully_monomorphized(),
                    *def_id,
                    arguments,
                )
                .expect("collected direct source call must resolve")
                .expect("collected direct source call must have a concrete instance");
                if trusted_device_items::classify(tcx, instance.def_id())
                    != Some(TrustedDeviceItem::ExecutionLdsReadPublished)
                {
                    continue;
                }
                let identity = canonical_function_identities_v1(tcx, instance).function();
                if let Some(previous) = read_instances.insert(identity, instance) {
                    assert_eq!(
                        previous, instance,
                        "read identity must retain its exact instance"
                    );
                }
            }
        }
        for instance in read_instances.into_values() {
            let signature = tcx.instantiate_bound_regions_with_erased(
                tcx.fn_sig(instance.def_id())
                    .instantiate(tcx, instance.args),
            );
            let lds_ty = rust_shared_reference_v1(signature.inputs()[0]).unwrap();
            let lds = rust_workgroup_lds_v1(tcx, lds_ty)
                .expect("actual reusable published LDS must retain its execution brand");
            let workgroup = rust_workgroup_capability_v1(
                tcx,
                rust_shared_reference_v1(signature.inputs()[1]).unwrap(),
            )
            .unwrap();
            assert_eq!(lds.elements, 256);
            assert!(rust_same_kernel_brand_v1(
                lds.kernel_brand,
                workgroup.kernel_brand
            ));
            assert_eq!(lds.epoch, workgroup.epoch);
            let phase = lds.kernel_brand;
            assert!(
                rust_kernel_brand_v1(tcx, phase.ty).is_none(),
                "plain-root parser stays closed"
            );
            let parents = rust_trusted_adt_type_arguments_v1(
                tcx,
                phase.ty,
                TrustedDeviceItem::ReusableWorkgroupBrand,
            )
            .unwrap();
            let [parent] = parents.as_slice() else {
                panic!("one exact root brand");
            };
            let root = rust_kernel_brand_v1(tcx, *parent).unwrap();
            assert_ne!(phase.ty, root.ty);
            assert!(!rust_same_kernel_brand_v1(phase, root));
            assert_eq!(
                (phase.kernel, phase.target, phase.launch),
                (root.kernel, root.target, root.launch)
            );
            let brand = rustc_type_identity_v1(tcx, phase.ty);
            assert_ne!(brand, rustc_type_identity_v1(tcx, root.ty));

            // A local same-name wrapper and a trusted wrapper around it are both foreign.
            let foreign = Ty::new_adt(
                tcx,
                tcx.adt_def(spoof.to_def_id()),
                tcx.mk_args(&[(*parent).into()]),
            );
            assert!(rust_execution_brand_v1(tcx, foreign).is_none());
            assert!(rust_execution_brand_v1(tcx, tcx.types.f32).is_none());
            let rustc_middle::ty::TyKind::Adt(definition, arguments) = *phase.ty.kind() else {
                unreachable!();
            };
            let changed = arguments
                .iter()
                .map(|argument| match argument.kind() {
                    rustc_middle::ty::GenericArgKind::Type(_) => foreign.into(),
                    _ => argument,
                })
                .collect::<Vec<_>>();
            assert!(
                rust_execution_brand_v1(tcx, Ty::new_adt(tcx, definition, tcx.mk_args(&changed)))
                    .is_none()
            );
            expected_reads.push((
                canonical_function_identities_v1(tcx, instance).function(),
                brand,
                rustc_type_identity_v1(tcx, lds.epoch),
                rustc_type_identity_v1(tcx, lds_ty),
            ));
        }
        assert!(
            !expected_reads.is_empty(),
            "must observe the actual published-array read"
        );
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("actual reusable phases must complete canonical source import");
        let mir = &imported.semantic_mir;
        let mut reads = 0;
        let mut phase_operations = 0;
        for callable in mir.callables() {
            let SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            } = callable
            else {
                continue;
            };
            if expected_reads
                .iter()
                .any(|(_, brand, _, _)| contract.workgroup_brand() == Some(*brand))
            {
                phase_operations += 1;
            }
            let Some((_, brand, epoch, lds_identity)) = expected_reads
                .iter()
                .find(|(identity, ..)| *identity == binding.identity())
            else {
                continue;
            };
            let SemanticExecutionCapabilityOperationV1::LdsReadPublished {
                lds_reference,
                lds,
                workgroup,
                elements,
                ..
            } = contract.operation()
            else {
                panic!("source read must not change operation");
            };
            assert_eq!(elements, 256);
            assert_eq!(contract.workgroup_brand(), Some(*brand));
            assert_eq!(contract.epoch_before(), Some(*epoch));
            assert_eq!(contract.epoch_after(), None);
            assert_eq!(mir.types()[lds.index() as usize].identity(), *lds_identity);
            shared_edge(mir, lds_reference, lds);
            assert!(matches!(mir.types()[workgroup.index() as usize].shape(),
                SemanticTypeShapeV1::Pointer(pointer) if pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.mutability() == SemanticMutabilityV1::Immutable));
            reads += 1;
        }
        assert_eq!(reads, expected_reads.len());
        assert!(
            phase_operations >= 4,
            "retain read, initialization, publication and completion barrier brands"
        );
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
        assert_eq!(decoded.callables(), mir.callables());
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
#[ignore = "requires complete cached AMD metadata via FE2O3_CORE_TRY_*; no Cargo"]
fn reusable_phase_full_import_gfx950() {
    run(
        "collector::production_importer_v1::subgroup_partition_v1::workgroup_source_v1::canonical_transport_v1::import_tests::reusable_phase_import_tests::reusable_phase_full_import_gfx950",
        |args| {
            let mut probe = PhaseProbe { completed: false };
            rustc_driver::run_compiler(args, &mut probe);
            assert!(
                probe.completed,
                "actual reusable source and canonical roundtrip must complete"
            );
        },
    );
}
