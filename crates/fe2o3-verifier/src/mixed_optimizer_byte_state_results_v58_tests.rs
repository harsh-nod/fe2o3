#[test]
fn shared_byte_state_result_definitions_preserve_complete_unconditional_equivalence() {
    let source = include_str!("mixed_optimizer_byte_results_v55.vrs");
    let prelude = super::super::byte_memory_v30::BYTE_MEMORY_V30;
    assert_eq!(prelude.matches(source).count(), 1);
    for name in ["byte_block_refused", "byte_micro_begin_result"] {
        assert_eq!(source.matches(&format!("spec fn {name}_v58(")).count(), 1);
        assert_eq!(
            source
                .matches(&format!("proof fn {name}_exact_v58("))
                .count(),
            1
        );
    }
    assert!(source.contains("ensures byte_block_refused_v58(state, observations) == (MemoryBlockResultV30 {\n        state: MemoryStateV30 { valid: false, ..state },\n        observations, returned: Seq::empty(),\n    }),"));
    assert!(source.contains("ensures byte_micro_begin_result_v58(state, next_operation) == (MemoryMicroStateV30 {\n        state, observations: Seq::empty(), next_operation,\n    }),"));
    for forbidden in [
        "requires ",
        "assume(",
        "admit(",
        "external_body",
        "axiom",
        "spec_fn",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn shared_byte_state_results_keep_each_model_guard_dispatch_and_observation() {
    for module in [memory_module(1), trap_module_v40(3)] {
        with_inventory(&module, |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            let function = &inventory.functions()[0];
            let definitions = inventory.definitions().len();
            for (namespace, width) in [
                (58, FormalIndexWidth::Bits32),
                (159, FormalIndexWidth::Bits64),
            ] {
                let source = run(floor, LIMIT, LIMIT, |out| {
                    ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        ByteContext::native(width),
                        &allocations,
                        out,
                    )?
                    .emit(namespace, out)
                })
                .0
                .unwrap();
                let blocks = function.blocks.len();
                assert_eq!(
                    source
                        .matches(&format!("spec fn byte_block_{namespace}_"))
                        .count(),
                    blocks
                );
                assert_eq!(
                    source
                        .matches(&format!("spec fn byte_control_{namespace}_"))
                        .count(),
                    blocks
                );
                assert_eq!(
                    source
                        .matches(&format!("spec fn byte_operation_{namespace}_"))
                        .count(),
                    function.operations.len()
                );
                assert_eq!(
                    source
                        .matches("byte_block_refused_v58(s, Seq::empty())")
                        .count(),
                    blocks + 1
                );
                assert_eq!(
                    source
                        .matches("byte_block_refused_v58(done, observations)")
                        .count(),
                    blocks
                );
                assert_eq!(
                    source
                        .matches("byte_block_refused_v58(m.state, m.observations)")
                        .count(),
                    1
                );
                assert_eq!(
                    source
                        .matches(
                            "byte_micro_begin_result_v58(MemoryStateV30 { valid: false, ..s }, -1)"
                        )
                        .count(),
                    1
                );
                for block in function.blocks.clone() {
                    let row = &inventory.blocks()[block];
                    let first = if row.operations.is_empty() {
                        "-1".to_owned()
                    } else {
                        row.operations.start.to_string()
                    };
                    assert!(source.contains(&format!("if s.values.len() != {definitions} || s.pc != {block} {{ byte_block_refused_v58(s, Seq::empty()) }} else {{")));
                    assert!(source.contains(&format!("if done.values.len() != {definitions} || (done.pc != {block} && !trapped) || !byte_state_memory_well_formed_v30(done) || !")));
                    assert!(source.contains(&format!(
                        "if s.pc == {block} {{ byte_micro_begin_result_v58(s, {first}) }} else"
                    )));
                    assert!(source.contains(&format!("if s.pc == {block} {{ byte_block_{namespace}_{block}_v30(s, little_endian) }} else")));
                    assert!(source.contains(&format!(
                        "if m.next_operation == -1 && (m.state.pc == {block} || ("
                    )));
                    assert!(source.contains(&format!("&& m.observations.len() == {} {{ byte_control_{namespace}_{block}_v30(m.state, m.observations) }} else", row.operations.len())));
                    for (prefix, operation) in row.operations.clone().enumerate() {
                        let next = if operation + 1 == row.operations.end {
                            "-1".to_owned()
                        } else {
                            (operation + 1).to_string()
                        };
                        assert!(source.contains(&format!("let step{operation} = byte_operation_{namespace}_{operation}_v30(current, little_endian);\n let current = step{operation}.state;")));
                        assert!(source.contains(&format!("step{operation}.observation,")));
                        assert!(source.contains(&format!("if m.state.pc == {block} && m.next_operation == {operation} && m.observations.len() == {prefix} {{ byte_micro_result_v55(m, byte_operation_{namespace}_{operation}_v30(m.state, little_endian), {next}) }} else")));
                    }
                }
                assert!(source.contains("if trapped { MemoryBlockResultV30 { state: done, observations, returned: Seq::empty() } }"));
                assert_eq!(source.matches("byte_micro_refused_v55(m)").count(), 1);
                assert!(!source.contains(" as byte_"));
            }
        });
    }
}

fn shared_state_result_program_v58(
    inventory: &Inventory<'_>,
    physical: &Physical<'_, '_>,
    floor: usize,
    work: usize,
    storage: usize,
) -> (Result<String>, usize, usize) {
    let allocations = NoAllocations(inventory.owner());
    run(floor, work, storage, |out| {
        emit!(
            out,
            "use vstd::prelude::*;\nverus! {{\n{}",
            super::super::byte_memory_v30::BYTE_MEMORY_V30
        );
        ByteFunctionV30::derive(
            inventory,
            physical,
            Function(0),
            ByteContext::native(FormalIndexWidth::Bits64),
            &allocations,
            out,
        )?
        .emit(258, out)?;
        emit!(out, "}}\n");
        Ok(())
    })
}

#[test]
fn shared_byte_state_results_reduce_complete_source_without_dropping_blocks() {
    let count = 128;
    with_inventory(&trap_module_v40(count), |inventory, physical, floor| {
        let source = shared_state_result_program_v58(inventory, physical, floor, LIMIT, LIMIT)
            .0
            .unwrap();
        let blocks = count + 1;
        let helpers = include_str!("mixed_optimizer_byte_results_v55.vrs");
        assert_eq!(source.matches(helpers).count(), 1);
        assert_eq!(source.matches("spec fn byte_block_258_").count(), blocks);
        assert_eq!(source.matches("spec fn byte_control_258_").count(), blocks);
        assert_eq!(source.matches("spec fn byte_operation_258_").count(), count);
        // Expand only the six closed constructor shapes of this exact fixture.
        let mut inline = source.replace("byte_block_refused_v58(s, Seq::empty())", "MemoryBlockResultV30 { state: MemoryStateV30 { valid: false, ..s }, observations: Seq::empty(), returned: Seq::empty() }")
            .replace("byte_block_refused_v58(done, observations)", "MemoryBlockResultV30 { state: MemoryStateV30 { valid: false, ..done }, observations, returned: Seq::empty() }")
            .replace("byte_block_refused_v58(m.state, m.observations)", "MemoryBlockResultV30 { state: MemoryStateV30 { valid: false, ..m.state }, observations: m.observations, returned: Seq::empty() }")
            .replace("byte_micro_begin_result_v58(MemoryStateV30 { valid: false, ..s }, -1)", "MemoryMicroStateV30 { state: MemoryStateV30 { valid: false, ..s }, observations: Seq::empty(), next_operation: -1 }");
        for next in
            std::iter::once("-1".to_owned()).chain((0..count).map(|index| index.to_string()))
        {
            inline = inline.replace(&format!("byte_micro_begin_result_v58(s, {next})"), &format!("MemoryMicroStateV30 {{ state: s, observations: Seq::empty(), next_operation: {next} }}"));
        }
        assert_eq!(inline.len() - source.len(), 194 * blocks + 208);
        assert!(inline.len() - source.len() > helpers.len() + 20_000);
        assert!(source.len() < super::super::super::SOURCE_LIMIT);
        assert_eq!(
            source
                .matches("byte_trap_terminal_v40(done, observations,")
                .count(),
            count
        );
        assert_eq!(
            source
                .matches("byte_trap_terminal_v40(m.state, m.observations,")
                .count(),
            count
        );
    });
}

#[test]
fn shared_byte_state_results_keep_exact_and_one_short_source_budgets() {
    with_inventory(&trap_module_v40(3), |inventory, physical, floor| {
        let emit = |work, storage| {
            shared_state_result_program_v58(inventory, physical, floor, work, storage)
        };
        let measured = emit(LIMIT, LIMIT);
        let source = measured.0.unwrap();
        let exact = emit(measured.1, measured.2);
        assert_eq!(exact.0.unwrap(), source);
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        assert!(matches!(emit(measured.1 - 1, measured.2).0,
            Err(Error::Resource(Resource::Work(error))) if error.limit() == measured.1 - 1 && error.actual() == measured.1));
        assert!(matches!(emit(measured.1, measured.2 - 1).0,
            Err(Error::Resource(Resource::Storage(error))) if error.limit() == measured.2 - 1 && error.actual() == measured.2));
    });
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_byte_state_result_constructors_equal_original_inline_expressions() {
    use crate::{CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusRuntimeLeaseV1};
    use std::time::{Duration, Instant};

    let program = with_state_result_equivalence_program_v58();
    assert_complete_constructor_equalities_v60(&program);
    let source = CanonicalGeneratedVerusProofInputV3::new(program.into_bytes())
        .expect("canonical complete constructor equivalence program");
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open(
        "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
    )
    .expect("requires the actual public pinned-runtime lease");
    runtime
        .revalidate()
        .expect("revalidate before constructor equivalence proof");
    let mut attempt = runtime
        .begin_attempt()
        .expect("acquire constructor equivalence attempt");
    let output = runtime
        .execute_generated_rust_verify(
            &mut attempt,
            &source,
            Instant::now() + Duration::from_secs(120),
            16 * 1024,
        )
        .expect("execute complete sealed source through the protected runtime");
    runtime
        .revalidate()
        .expect("revalidate after constructor equivalence proof");
    attempt
        .complete()
        .expect("complete constructor equivalence attempt");
    crate::functional_refinement_receipt_v2::validate_proved_output(&output)
        .expect("unconditional complete state/result constructor equivalences must verify");
}

fn with_state_result_equivalence_program_v58() -> String {
    let mut program = None;
    with_inventory(&trap_module_v40(3), |inventory, physical, floor| {
        program = Some(
            shared_state_result_program_v58(inventory, physical, floor, LIMIT, LIMIT)
                .0
                .unwrap(),
        );
    });
    program.unwrap()
}

#[test]
fn shared_byte_state_result_equivalence_program_is_complete_and_unconditional() {
    let source = with_state_result_equivalence_program_v58();
    assert_complete_constructor_equalities_v60(&source);
    for name in [
        "byte_block_refused_exact_v58",
        "byte_micro_begin_result_exact_v58",
        "byte_operation_result_exact_v55",
        "byte_operation_refused_exact_v55",
        "byte_micro_result_exact_v55",
        "byte_micro_refused_exact_v55",
    ] {
        assert_eq!(source.matches(&format!("proof fn {name}(")).count(), 1);
    }
    assert_eq!(source.matches("spec fn byte_block_258_").count(), 4);
    assert_eq!(source.matches("spec fn byte_control_258_").count(), 4);
    assert_eq!(source.matches("spec fn byte_operation_258_").count(), 3);
    assert!(source.starts_with("use vstd::prelude::*;\nverus! {\n"));
    assert!(source.ends_with("}\n"));
    assert!(!source.contains("assume("));
    assert!(!source.contains("external_body"));
}
