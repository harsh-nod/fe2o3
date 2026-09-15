use super::*;

fn target_module(lowered: &ProductionSemanticKirOwnerV1, gfx950: bool) -> fe2o3_kernel_ir::Module {
    let mut module = lowered.module().clone();
    let target = if gfx950 {
        fe2o3_kernel_ir::gfx950_xnack_minus_target_capability()
    } else {
        gfx942_xnack_minus_target_capability()
    };
    module.required_capabilities.insert(target.clone());
    for function in &mut module.functions {
        if function.body.is_some() {
            function.required_capabilities.insert(target.clone());
        }
    }
    for kernel in &mut module.kernels {
        kernel.required_capabilities.insert(target.clone());
        kernel
            .required_capabilities
            .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
    }
    verify_module(&module).unwrap();
    module
}

fn exact_llvm(module: &fe2o3_kernel_ir::Module, gfx950: bool) -> String {
    if gfx950 {
        lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(module).unwrap()
    } else {
        lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(module).unwrap()
    }
}

fn definition_body<'a>(llvm: &'a str, name: &fe2o3_kernel_ir::FunctionId) -> &'a str {
    let start = llvm.find(&format!("define internal i64 @{name}(")).unwrap();
    let (_, body) = llvm[start..].split_once(" {\n").unwrap();
    body.split_once("\n}\n").unwrap().0
}

fn assert_exact_length_helper(llvm: &str, helper: &fe2o3_kernel_ir::Function) {
    let body = helper.body.as_ref().unwrap();
    let [block] = body.blocks.as_slice() else {
        panic!("source length helper has one block")
    };
    let [length, cast] = block.operations.as_slice() else {
        panic!("source length then ABI return cast")
    };
    assert!(
        matches!(length.kind, OperationKind::SliceLength { slice } if slice == body.parameters[0])
    );
    assert!(
        matches!(cast.kind, OperationKind::Cast { kind: CastKind::Bitcast, value, .. } if value == length.results[0].id)
    );
    assert!(llvm.contains(&format!(
        "define internal i64 @{}(ptr addrspace(1) %arg0.data, i64 %arg0.len)",
        helper.id
    )));
    assert_eq!(
        definition_body(llvm, &helper.id),
        format!(
            "bb{}:\n  %v{} = add i64 %arg0.len, 0\n  %v{} = add i64 %v{}, 0\n  ret i64 %v{}",
            block.id.0,
            length.results[0].id.0,
            cast.results[0].id.0,
            length.results[0].id.0,
            cast.results[0].id.0,
        )
    );
}

#[test]
fn source_shared_slice_length_reaches_exact_llvm_on_both_targets_with_a_real_root_read() {
    for f32_element in [false, true] {
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            owner(Shape::RootRead, f32_element),
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        lowered.verify_equivalence().unwrap();
        let effects = analyze_interprocedural_effects_v1(lowered.module()).unwrap();
        let entry = &lowered.module().functions[0];
        let helper = &lowered.module().functions[1];
        assert!(!effects.function(&entry.id).unwrap().is_complete_and_pure());
        assert!(effects.function(&helper.id).unwrap().is_complete_and_pure());
        assert!(
            entry
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(operation.kind, OperationKind::Load { .. }))
        );
        for gfx950 in [false, true] {
            let module = target_module(&lowered, gfx950);
            let llvm = exact_llvm(&module, gfx950);
            assert_eq!(llvm, exact_llvm(&module, gfx950));
            assert!(llvm.contains(if gfx950 {
                "\"target-cpu\"=\"gfx950\""
            } else {
                "\"target-cpu\"=\"gfx942\""
            }));
            assert!(llvm.contains(&format!(
                "call i64 @{}(ptr addrspace(1) %arg0.data, i64 %arg0.len)",
                helper.id
            )));
            assert!(llvm.contains(if f32_element {
                " = load float, ptr addrspace(1) "
            } else {
                " = load i32, ptr addrspace(1) "
            }));
            assert_exact_length_helper(&llvm, helper);
        }
    }
}

#[test]
fn source_slice_join_and_repeated_calls_keep_the_same_data_length_carrier_in_exact_llvm() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        owner(Shape::BranchAndTwoCalls, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    let entry = &lowered.module().functions[0];
    let helper = &lowered.module().functions[1];
    let body = entry.body.as_ref().unwrap();
    let (join, merged) = body
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .find_map(|operation| match &operation.kind {
                    OperationKind::Call { arguments, .. } => Some((block, arguments[0])),
                    _ => None,
                })
        })
        .unwrap();
    let parameter = join
        .parameters
        .iter()
        .position(|parameter| parameter.id == merged)
        .unwrap();
    let mut data = Vec::new();
    let mut lengths = Vec::new();
    for predecessor in &body.blocks {
        if let Some(Terminator::Branch { target, arguments }) = &predecessor.terminator
            && *target == join.id
        {
            let original = body
                .parameters
                .iter()
                .position(|value| *value == arguments[parameter])
                .unwrap();
            data.push(format!("[ %arg{original}.data, %bb{} ]", predecessor.id.0));
            lengths.push(format!("[ %arg{original}.len, %bb{} ]", predecessor.id.0));
        }
    }
    assert_eq!(data.len(), 2);
    for gfx950 in [false, true] {
        let llvm = exact_llvm(&target_module(&lowered, gfx950), gfx950);
        assert!(llvm.contains(&format!(
            "%v{}.data = phi ptr addrspace(1) {}",
            merged.0,
            data.join(", ")
        )));
        assert!(llvm.contains(&format!(
            "%v{}.len = phi i64 {}",
            merged.0,
            lengths.join(", ")
        )));
        assert_eq!(
            llvm.matches(&format!("call i64 @{}(", helper.id)).count(),
            2
        );
        assert!(llvm.contains(&format!(
            "call i64 @{}(ptr addrspace(1) %v{}.data, i64 %v{}.len)",
            helper.id, merged.0, merged.0
        )));
        assert_exact_length_helper(&llvm, helper);
    }
}

fn source_forwarding_owner() -> ProductionSemanticMirOwnerV1 {
    let original = owner(Shape::RootRead, false);
    let semantic = original.semantic();
    let helper = &semantic.functions()[1];
    let forwarding_call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(2),
            vec![SemanticOperandV1::Copy(local_place(1, ty(3)))],
            Some(SemanticCallDestinationV1::new(
                local_place(0, ty(4)),
                edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let forwarding = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        helper.source(),
        helper.abi().clone(),
        helper.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        vec![
            block(201, vec![], forwarding_call),
            block(202, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap();
    let length = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(210)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(211)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(212)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(213)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(214)),
        helper.source(),
        helper.abi().clone(),
        helper.locals().to_vec(),
        helper.entry(),
        helper.blocks().to_vec(),
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(semantic.functions()[0].abi().layout_identity()),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![semantic.functions()[0].clone(), forwarding, length],
        (0..3)
            .map(|index| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index)))
            .collect(),
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

#[test]
fn actual_source_forwarding_helper_retains_both_slice_arguments_and_purity_at_exact_llvm() {
    let lowered = ProductionSemanticKirOwnerV1::try_lower(
        source_forwarding_owner(),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    lowered.verify_equivalence().unwrap();
    assert_eq!(lowered.module().functions.len(), 3);
    let effects = analyze_interprocedural_effects_v1(lowered.module()).unwrap();
    assert!(
        !effects
            .function(&lowered.module().functions[0].id)
            .unwrap()
            .is_complete_and_pure()
    );
    let forwarding = &lowered.module().functions[1];
    let length = &lowered.module().functions[2];
    assert!(
        effects
            .function(&forwarding.id)
            .unwrap()
            .is_complete_and_pure()
    );
    assert!(effects.function(&length.id).unwrap().is_complete_and_pure());
    for gfx950 in [false, true] {
        let llvm = exact_llvm(&target_module(&lowered, gfx950), gfx950);
        assert!(llvm.contains(&format!(
            "call i64 @{}(ptr addrspace(1) %arg0.data, i64 %arg0.len)",
            forwarding.id
        )));
        assert!(definition_body(&llvm, &forwarding.id).contains(&format!(
            "call i64 @{}(ptr addrspace(1) %arg0.data, i64 %arg0.len)",
            length.id
        )));
        assert_exact_length_helper(&llvm, length);
    }
}
