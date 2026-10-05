use fe2o3_kernel_ir::{Axis, IndexKind, IntrinsicKind, IntrinsicOperation};

fn index_module_v37(copies: usize) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    for _ in 0..copies {
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            for kind in [
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis,
                },
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Workgroup,
                    axis,
                },
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis,
                },
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::WorkgroupSize,
                    axis,
                },
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::WorkgroupCount,
                    axis,
                },
                IntrinsicKind::LaunchExtent { axis },
            ] {
                entry.operations.push(KirOperation::effect_free(
                    ValueDef::new(ValueId(entry.operations.len() as u32), Type::INDEX),
                    OperationKind::Intrinsic(IntrinsicOperation::new(kind, Type::INDEX)),
                ));
            }
        }
    }
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("exact-explicit-execution-index");
    module.functions.push(KirFunction::internal_helper(
        "coordinates",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    module
}

#[test]
fn byte_index_dispatch_requires_explicit_context_for_all_axes_and_hierarchies() {
    with_inventory(&index_module_v37(1), |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        for (width, bytes) in [(FormalIndexWidth::Bits32, 4), (FormalIndexWidth::Bits64, 8)] {
            let text = run(floor, LIMIT, LIMIT, |out| {
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    ByteContext::native(width),
                    &allocations,
                    out,
                )?
                .emit(81, out)
            })
            .0
            .unwrap();
            assert_eq!(text.matches("MemoryOperationEffectV30::Pure").count(), 18);
            assert_eq!(text.matches("None => false").count(), 18);
            assert_eq!(
                text.matches("byte_execution_well_formed_v37(execution)")
                    .count(),
                18
            );
            assert_eq!(
                text.matches(&format!("coordinate < memory_value_modulus_v30({bytes})"))
                    .count(),
                18
            );
            for axis in 0..3 {
                for hierarchy in 0..3 {
                    assert_eq!(
                        text.matches(&format!(
                            "byte_execution_index_v37(execution, {hierarchy}, {axis})"
                        ))
                        .count(),
                        2
                    );
                }
                assert_eq!(text.matches(&format!("{axis} < execution.rank")).count(), 6);
                assert!(text.contains(&format!("let coordinate = execution.workgroup[{axis}]")));
                assert!(text.contains(&format!("let coordinate = execution.extent[{axis}]")));
                assert!(text.contains(&format!(
                    "((execution.extent[{axis}] - 1) / execution.workgroup[{axis}] + 1)"
                )));
            }
            assert_eq!(text.matches("let frames = s.frames;").count(), 18);
            assert!(!text.contains("byte_root_frame_with_execution_v37("));
            assert!(!text.contains("MemoryOperationEffectV30::Read"));
            assert!(!text.contains("MemoryOperationEffectV30::Write"));
        }
    });
}

#[test]
fn byte_index_derivation_resources_scale_linearly_without_hidden_kernel_coordinates() {
    for copies in [1, 4, 16] {
        for returned in [false, true] {
            let mut module = index_module_v37(copies);
            let count = 18 * copies;
            if returned {
                module.functions[0].signature.results = vec![Type::INDEX];
                module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
                    Some(Terminator::Return {
                        values: vec![ValueId((count - 1) as u32)],
                    });
            }
            with_inventory(&module, |inventory, physical, floor| {
                let allocations = NoAllocations(inventory.owner());
                let derive = |out: &mut Writer<'_, '_>| {
                    ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        ByteContext::native(FormalIndexWidth::Bits64),
                        &allocations,
                        out,
                    )
                    .map(|_| ())
                };
                // Owner1 + entry/analysis4 + definitionsN + N*(dispatch4,
                // Alloca6, Storage6, pointer2+24, INDEX2+8, physical19).
                // Control entry5 is fixed; only a returned value adds operand
                // visitation3 and result-type check1. Empty Return has neither.
                let work = 1
                    + 4
                    + count * (1 + 4 + 6 + 6 + 2 + 24 + 2 + 8 + 19)
                    + 5
                    + usize::from(returned) * (3 + 1);
                let storage = floor
                    + super::super::super::SOURCE_LIMIT
                    + headers::<NoAllocations<'_>>()
                    + count * size_of::<ByteOperationV30<'_, '_>>();
                let exact = run(floor, work, storage, derive);
                assert!(exact.0.unwrap().is_empty());
                assert_eq!((exact.1, exact.2), (work, storage));
                assert!(matches!(
                    run(floor, work - 1, storage, derive).0,
                    Err(Error::Resource(Resource::Work(error)))
                        if error.limit() == work - 1 && error.actual() == work
                ));
                assert!(matches!(
                    run(floor, work, storage - 1, derive).0,
                    Err(Error::Resource(Resource::Storage(error)))
                        if error.limit() == storage - 1 && error.actual() == storage
                ));
            });
        }
    }
}

#[test]
fn byte_execution_context_is_never_fabricated_by_legacy_roots_or_changed_by_frames() {
    let text = super::super::byte_memory_v30::BYTE_MEMORY_V30;
    let legacy = text
        .split("spec fn byte_root_frame_v30")
        .nth(1)
        .unwrap()
        .split("spec fn byte_execution_well_formed_v37")
        .next()
        .unwrap();
    assert!(legacy.contains("execution: None"));
    assert!(!legacy.contains("Some("));
    for (begin, end) in [
        ("spec fn byte_enter_frame_v30", "spec fn byte_pop_frame_v30"),
        (
            "spec fn byte_pop_frame_v30",
            "spec fn byte_allocation_in_frame_v30",
        ),
    ] {
        let function = text.split(begin).nth(1).unwrap().split(end).next().unwrap();
        assert!(function.contains("execution: frames.execution"));
    }
    assert!(text.contains("execution: Some(execution)"));
    assert!(text.contains("execution.rank <= axis ==> execution.extent[axis] == 1"));
}
