use super::*;
use fe2o3_amdgcn_model::{
    LoweringDiagnosticCode, ProductionSemanticAnchorKirIdentityV1,
    lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1 as native_942,
    lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1 as native_950,
    lower_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1 as legacy_942,
    lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1 as legacy_950,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1 as legacy_kernel_942,
    lower_kernel_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1 as legacy_kernel_950,
};

fn exact_binding(input: &Owner, output: &Owner, profile: Profile, llvm: &str) {
    let canonical = output.canonical().identity();
    let output_digest = canonical
        .digest()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let input_digest = input
        .canonical()
        .identity()
        .digest()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let target = match profile {
        Profile::Gfx942 => "gfx942:xnack-",
        Profile::Gfx950 => "gfx950:xnack-",
    };
    let encoded = format!(
        "!\"sha256:{output_digest}\", !\"kir-version:12\", i64 {}, !\"target:{target}\"",
        canonical.canonical_length()
    );
    assert_eq!(llvm.matches(&encoded).count(), 1, "missing exact O binding");
    assert_ne!(input.canonical().identity(), canonical);
    assert!(!llvm.contains(&format!("!\"sha256:{input_digest}\"")));
}

fn assert_historical_paths_refuse(output: &Owner, profile: Profile) {
    let identity = ProductionSemanticAnchorKirIdentityV1::from_v12(output);
    assert_eq!(identity.version(), 12);
    assert_eq!(identity.sha256(), *output.canonical().identity().digest());
    assert_eq!(
        identity.byte_len(),
        output.canonical().identity().canonical_length()
    );
    let kernel = &output.module().kernels[0].id;
    let (module_result, kernel_result) = match profile {
        Profile::Gfx942 => (
            legacy_942(output.module(), identity),
            legacy_kernel_942(output.module(), kernel, identity),
        ),
        Profile::Gfx950 => (
            legacy_950(output.module(), identity),
            legacy_kernel_950(output.module(), kernel, identity),
        ),
    };
    for result in [module_result, kernel_result] {
        assert!(
            result
                .unwrap_err()
                .contains(LoweringDiagnosticCode::SemanticAnchorIdentityMismatch)
        );
    }
}

fn source_with_store() -> Module {
    let mut source = neutral(true);
    let function = &mut source.functions[0];
    function.signature = Signature::new(
        vec![
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                fe2o3_kernel_ir::AddressSpace::Global,
                fe2o3_kernel_ir::AccessMode::ReadWrite,
            ),
            Type::Scalar(ScalarType::U32),
        ],
        vec![],
    );
    function.body.as_mut().unwrap().parameters = vec![ValueId(0), ValueId(1)];
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access: fe2o3_kernel_ir::MemoryAccess::new(
                    fe2o3_kernel_ir::AddressSpace::Global,
                    4,
                ),
            },
        ));
    source
}

#[test]
fn actual_changed_v12_output_lowers_with_its_own_anchor_identity_on_both_targets() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for nonempty in [false, true] {
            let source = if nonempty {
                source_with_store()
            } else {
                neutral(true)
            };
            let binding = bind_production_target_v1(&source, profile).unwrap();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.charge_work(PRIOR_WORK).unwrap();
            budget.reserve_storage(PREFIX).unwrap();
            let (input, input_storage) =
                Owner::from_module_ref_with_verification_budget_v12(binding.module(), &mut budget)
                    .unwrap();
            budget
                .reserve_storage(input_storage.retained_storage())
                .unwrap();
            let checked = optimize_checked_canonical_kernel_ir_v1(&input, &mut budget).unwrap();
            budget
                .reserve_storage(checked.storage().retained_storage())
                .unwrap();
            assert_ne!(
                input.canonical().identity(),
                checked.owner().canonical().identity()
            );
            let original = &input.module().functions[0].body.as_ref().unwrap().blocks[0];
            let output = &checked.owner().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks[0];
            assert_eq!(original.operations.len(), 2 + usize::from(nonempty));
            assert_eq!(output.operations.len(), usize::from(nonempty));
            let owner_rows = checked.owner().module().functions.as_ptr();
            let floor = budget.storage();
            let before = budget.work();
            let llvm = match profile {
                Profile::Gfx942 => native_942(checked.owner()),
                Profile::Gfx950 => native_950(checked.owner()),
            }
            .unwrap();
            exact_binding(&input, checked.owner(), profile, &llvm);
            if nonempty {
                assert!(llvm.contains("store i32"));
                assert!(llvm.contains("llvm.pseudoprobe"));
                assert!(!llvm.contains("!fe2o3.semantic_anchor.absence.v1"));
            } else {
                assert!(llvm.contains("!\"no_operations\""));
                assert!(!llvm.contains("llvm.pseudoprobe"));
            }
            assert_eq!(checked.owner().module().functions.as_ptr(), owner_rows);
            // Lowering retains its existing separate meter/size policy; the
            // caller's live N/B/O canonical ledger is neither reset nor freed.
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.work(), before);
            assert_historical_paths_refuse(checked.owner(), profile);
            let wrong_target = match profile {
                Profile::Gfx942 => native_950(checked.owner()),
                Profile::Gfx950 => native_942(checked.owner()),
            };
            assert!(wrong_target.is_err());
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.work(), before);
            assert_eq!(budget.failed_storage(), None);
            drop(llvm);
            let checked_storage = checked.storage().retained_storage();
            drop(checked);
            budget.release_storage(checked_storage).unwrap();
            drop(input);
            budget
                .release_storage(input_storage.retained_storage())
                .unwrap();
            assert_eq!(budget.storage(), PREFIX);
        }
    }
}

fn source_with_pair_helper() -> Module {
    use fe2o3_kernel_ir::{AccessMode, AddressSpace, FunctionId, MemoryAccess};
    let u32_type = Type::Scalar(ScalarType::U32);
    let u64_type = Type::Scalar(ScalarType::U64);
    let mut source = neutral(true);
    let entry = &mut source.functions[0];
    entry.signature = Signature::new(
        vec![
            Type::pointer(
                u32_type.clone(),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
            Type::pointer(
                u64_type.clone(),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
            u32_type.clone(),
            u64_type.clone(),
        ],
        vec![],
    );
    let body = entry.body.as_mut().unwrap();
    body.parameters = vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)];
    body.blocks[0].operations.push(Operation::new(
        vec![
            ValueDef::new(ValueId(101), u32_type.clone()),
            ValueDef::new(ValueId(102), u64_type.clone()),
        ],
        OperationKind::Call {
            callee: FunctionId::new("pair_helper"),
            arguments: vec![ValueId(2), ValueId(3)],
        },
    ));
    for (pointer, value, width) in [(0, 101, 4), (1, 102, 8)] {
        body.blocks[0].operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(pointer),
                value: ValueId(value),
                access: MemoryAccess::new(AddressSpace::Global, width),
            },
        ));
    }
    let mut helper = BasicBlock::new(BlockId(19));
    helper.terminator = Some(Terminator::Return {
        values: vec![ValueId(0), ValueId(1)],
    });
    source.functions.push(Function::internal_helper(
        "pair_helper",
        Signature::new(
            vec![u32_type.clone(), u64_type.clone()],
            vec![u32_type, u64_type],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![helper],
    ));
    source
}

#[test]
fn native_multiple_body_absence_keeps_actual_two_result_helper_abi_and_owner_floor() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let source = source_with_pair_helper();
        let binding = bind_production_target_v1(&source, profile).unwrap();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(PRIOR_WORK).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let (input, storage) =
            Owner::from_module_ref_with_verification_budget_v12(binding.module(), &mut budget)
                .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let checked = optimize_checked_canonical_kernel_ir_v1(&input, &mut budget).unwrap();
        let retained = checked.storage().retained_storage();
        budget.reserve_storage(retained).unwrap();
        let output = checked.owner();
        assert_ne!(input.canonical().identity(), output.canonical().identity());
        assert_eq!(output.module().functions.len(), 2);
        let entry = output
            .module()
            .function(&output.module().kernels[0].entry)
            .unwrap();
        let body = entry.body.as_ref().unwrap();
        let calls = body.blocks.iter().flat_map(|b| &b.operations).filter(|op| {
            matches!(&op.kind, OperationKind::Call { callee, .. } if callee.as_str() == "pair_helper")
        }).collect::<Vec<_>>();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].results.len(), 2);
        assert_eq!(calls[0].results[0].ty, Type::Scalar(ScalarType::U32));
        assert_eq!(calls[0].results[1].ty, Type::Scalar(ScalarType::U64));
        drop(calls);
        let owner_rows = output.module().functions.as_ptr();
        let floor = budget.storage();
        let before = budget.work();
        let lower = match profile {
            Profile::Gfx942 => native_942,
            Profile::Gfx950 => native_950,
        };
        let llvm = lower(output).unwrap();
        assert_eq!(llvm, lower(output).unwrap());
        exact_binding(&input, output, profile, &llvm);
        assert!(llvm.contains("!\"multiple_defined_bodies\""));
        assert!(llvm.contains("!fe2o3.semantic_anchor.absence.v1"));
        assert!(!llvm.contains("!fe2o3.semantic_anchor.v1 ="));
        assert!(!llvm.contains("llvm.pseudoprobe"));
        assert!(llvm.contains("%fe2o3.helper.result.pair_helper = type { i32, i64 }"));
        assert!(llvm.contains("define internal %fe2o3.helper.result.pair_helper @pair_helper("));
        assert_eq!(
            llvm.matches(" = extractvalue %fe2o3.helper.result.pair_helper ")
                .count(),
            2
        );
        assert_eq!(
            llvm.matches(" = insertvalue %fe2o3.helper.result.pair_helper ")
                .count(),
            2
        );
        assert_eq!(llvm.matches("store i32").count(), 1);
        assert_eq!(llvm.matches("store i64").count(), 1);
        assert_historical_paths_refuse(output, profile);

        let late_error = {
            let text = lower(output).unwrap();
            exact_binding(&input, output, profile, &text);
            Err::<(), _>("diagnostic continuation refused")
        };
        assert_eq!(late_error, Err("diagnostic continuation refused"));
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let text = lower(output).unwrap();
            exact_binding(&input, output, profile, &text);
            panic!("diagnostic continuation unwind");
        }));
        assert!(unwind.is_err());
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), before);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(output.module().functions.as_ptr(), owner_rows);
        drop(llvm);
        drop(checked);
        budget.release_storage(retained).unwrap();
        drop(input);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), PREFIX);
        assert_eq!(work.failed_work(), None);
    }
}
