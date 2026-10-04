fn assert_complete_constructor_equalities_v60(source: &str) {
    let compact = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let source = compact(source);
    for expected in [
        r#"ensures byte_block_refused_v58(state, observations) == (MemoryBlockResultV30 {
            state: MemoryStateV30 { valid: false, ..state },
            observations, returned: Seq::empty(),
        }), { }"#,
        r#"ensures byte_micro_begin_result_v58(state, next_operation) == (MemoryMicroStateV30 {
            state, observations: Seq::empty(), next_operation,
        }), { }"#,
        r#"ensures byte_result_v55(before, after, operation, effect)
            == (MemoryOperationResultV30 {
                state: after,
                observation: MemoryOperationObservationV30 {
                    operation, before, after,
                    valid_before: before.valid, valid_after: after.valid, effect,
                },
            }), { }"#,
        r#"ensures ({
            let result = byte_refused_v55(before, operation);
            let after = MemoryStateV30 { valid: false, ..before };
            result == (MemoryOperationResultV30 {
                state: after,
                observation: MemoryOperationObservationV30 {
                    operation, before, after,
                    valid_before: before.valid, valid_after: false,
                    effect: MemoryOperationEffectV30::Refused,
                },
            })
        }), { }"#,
        r#"ensures byte_micro_result_v55(before, result, next_operation)
            == (MemoryMicroResultV30 {
                next: MemoryMicroStateV30 {
                    state: result.state,
                    observations: before.observations.push(result.observation),
                    next_operation,
                },
                observation: result.observation,
            }), { }"#,
        r#"ensures ({
            let state = MemoryStateV30 { valid: false, ..before.state };
            let observation = MemoryOperationObservationV30 {
                operation: MemorySourceOperationV30 { function: -1, block: -1, operation: -1 },
                before: before.state, after: state,
                valid_before: before.state.valid, valid_after: false,
                effect: MemoryOperationEffectV30::Refused,
            };
            byte_micro_refused_v55(before) == (MemoryMicroResultV30 {
                next: MemoryMicroStateV30 {
                    state, observations: before.observations.push(observation), next_operation: -1,
                },
                observation,
            })
        }), { }"#,
    ] {
        assert_eq!(source.matches(&compact(expected)).count(), 1, "{expected}");
    }
    for (constructor, count) in [
        ("MemoryBlockResultV30", 1),
        ("MemoryMicroStateV30", 1),
        ("MemoryOperationResultV30", 2),
        ("MemoryMicroResultV30", 2),
    ] {
        assert_eq!(
            source.matches(&format!("== ({constructor} {{")).count(),
            count
        );
        assert!(!source.contains(&format!("== {constructor} {{")));
    }
}

#[test]
fn shared_byte_constructor_equalities_have_parser_safe_complete_rhs() {
    let source = include_str!("mixed_optimizer_byte_results_v55.vrs");
    assert_complete_constructor_equalities_v60(source);
    assert_eq!(source.matches("proof fn ").count(), 6);
    assert_eq!(source.matches("ensures ").count(), 6);
    assert!(!source.contains("requires "));
}

#[test]
fn generated_byte_frame_and_fragment_comparisons_parenthesize_rhs() {
    let memory = super::super::byte_memory_v30::BYTE_MEMORY_V30;
    assert!(memory.contains("frames.active[i] == (MemoryDynamicFrameV30 { owner, invocation }),"));
    assert!(!memory.contains("== MemoryDynamicFrameV30 {"));
    for source in [
        include_str!("mixed_optimizer_byte_view_laws_v38.vrs"),
        include_str!("original_semantic_mir_native_provenance_laws_v39.vrs"),
    ] {
        assert_eq!(
            source
                .matches("== (MemoryByteV37::PointerFragment { pointer, width, ordinal }),")
                .count(),
            1
        );
        assert!(!source.contains("== MemoryByteV37::PointerFragment {"));
        assert!(!source.contains("assume("));
        assert!(!source.contains("external_body"));
    }
}

#[test]
fn shared_byte_result_definitions_preserve_complete_snapshots_and_refusal() {
    let source = include_str!("mixed_optimizer_byte_results_v55.vrs");
    let prelude = super::super::byte_memory_v30::BYTE_MEMORY_V30;
    assert_eq!(prelude.matches(source).count(), 1);
    for name in [
        "byte_result_v55",
        "byte_refused_v55",
        "byte_micro_result_v55",
        "byte_micro_refused_v55",
    ] {
        assert_eq!(source.matches(&format!("open spec fn {name}(")).count(), 1);
    }
    for name in [
        "byte_operation_result_exact_v55",
        "byte_operation_refused_exact_v55",
        "byte_micro_result_exact_v55",
        "byte_micro_refused_exact_v55",
    ] {
        assert_eq!(source.matches(&format!("proof fn {name}(")).count(), 1);
    }
    assert!(source.contains("valid_before: before.valid, valid_after: after.valid, effect"));
    assert!(source.contains("before, MemoryStateV30 { valid: false, ..before }"));
    assert!(source.contains("operation, MemoryOperationEffectV30::Refused"));
    assert!(source.contains("observations: before.observations.push(result.observation)"));
    assert!(source.contains("MemorySourceOperationV30 { function: -1, block: -1, operation: -1 }"));
    assert!(source.contains("valid_before: before.state.valid, valid_after: false"));
    assert!(
        source.contains("observations: before.observations.push(observation), next_operation: -1")
    );
    for forbidden in [
        "assume(",
        "admit(",
        "external_body",
        "axiom",
        "requires ",
        "spec_fn",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn shared_byte_source_keeps_every_operation_under_the_unchanged_source_cap() {
    use fe2o3_kernel_ir::Constant;
    let count = 1024_u32;
    let mut entry = BasicBlock::new(BlockId(0));
    for index in 0..count {
        entry.operations.push(KirOperation::effect_free(
            ValueDef::new(ValueId(index), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(index)),
        ));
    }
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("complete-bounded-byte-source");
    module.functions.push(KirFunction::internal_helper(
        "body",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    with_inventory(&module, |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        let emit = |out: &mut Writer<'_, '_>| {
            ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                ByteContext::native(FormalIndexWidth::Bits64),
                &allocations,
                out,
            )?
            .emit(55, out)
        };
        let measured = run(floor, LIMIT, LIMIT, emit);
        let text = measured.0.unwrap();
        assert!(text.len() < super::super::super::SOURCE_LIMIT);
        assert_eq!(text.matches("open spec fn byte_inputs_55_v55(").count(), 1);
        assert_eq!(
            text.matches("open spec fn byte_operation_55_").count(),
            count as usize
        );
        assert_eq!(
            text.matches("byte_result_v55(s, state, operation, effect)")
                .count(),
            count as usize
        );
        assert_eq!(
            text.matches("byte_refused_v55(s, operation)").count(),
            count as usize
        );
        assert_eq!(
            text.matches("byte_micro_result_v55(m, byte_operation_55_")
                .count(),
            count as usize
        );
        assert_eq!(
            text.matches("byte_state_memory_well_formed_v30(s)").count(),
            1
        );
        for operation in 0..count {
            assert!(text.contains(&format!("open spec fn byte_operation_55_{operation}_v30(")));
            assert!(text.contains(&format!("function: 0, block: 0, operation: {operation}")));
            assert!(text.contains(&format!(
                "m.next_operation == {operation} && m.observations.len() == {operation}"
            )));
        }
        assert!(text.contains(
            "byte_micro_result_v55(m, byte_operation_55_1023_v30(m.state, little_endian), -1)"
        ));
        assert!(text.contains("m.observations.len() == 1024 { byte_control_55_0_v30"));
        let exact = run(floor, measured.1, measured.2, emit);
        assert_eq!(exact.0.unwrap(), text);
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        assert!(matches!(run(floor, measured.1 - 1, measured.2, emit).0,
            Err(Error::Resource(Resource::Work(error)))
                if error.limit() == measured.1 - 1 && error.actual() == measured.1));
        assert!(matches!(run(floor, measured.1, measured.2 - 1, emit).0,
            Err(Error::Resource(Resource::Storage(error)))
                if error.limit() == measured.2 - 1 && error.actual() == measured.2));
    });
}

#[test]
fn generated_source_section_diagnostics_preserve_the_cap_and_first_error() {
    use std::fmt::Write as _;
    let limit = super::super::super::SOURCE_LIMIT;
    assert_eq!(limit, crate::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3);
    assert_eq!(limit, 2 * 1024 * 1024);
    let fill = "x".repeat(limit);
    let exact = run(0, LIMIT, LIMIT, |out| {
        out.write_str(&fill).map_err(|_| out.error())?;
        Ok(())
    });
    assert_eq!(exact.0.unwrap().len(), limit);
    let refused = run(0, LIMIT, LIMIT, |out| {
        out.write_str(&fill).map_err(|_| out.error())?;
        out.write_str("!").unwrap_err();
        let error = out.error();
        let error = out.source_section_error(error, "exact test section");
        assert!(matches!(error, Error::GeneratedSourceLimit {
            section: "exact test section", emitted_bytes, limit_bytes,
        } if emitted_bytes == limit && limit_bytes == limit));
        Err(out.source_section_error(error, "outer section must not replace the inner section"))
    });
    assert!(matches!(refused.0, Err(Error::GeneratedSourceLimit {
        section: "exact test section", emitted_bytes, limit_bytes,
    }) if emitted_bytes == limit && limit_bytes == limit));
    let resource = run(0, 0, LIMIT, |out| {
        out.write_str("x").unwrap_err();
        let error = out.error();
        Err(out.source_section_error(error, "not a source limit"))
    });
    assert!(
        matches!(resource.0, Err(Error::Resource(Resource::Work(error)))
        if error.limit() == 0 && error.actual() == 1)
    );
    let statement = run(0, LIMIT, LIMIT, |out| {
        Err(out.source_section_error(
            Error::Statement("unmodeled operation"),
            "not a source limit",
        ))
    });
    assert!(matches!(
        statement.0,
        Err(Error::Statement("unmodeled operation"))
    ));
}
