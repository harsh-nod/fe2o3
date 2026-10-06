#[test]
fn shared_control_inputs_retain_all_blocks_and_exact_interpretation() {
    let count = 128;
    with_inventory(&trap_module_v40(count), |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
            for classified in [false, true] {
                let namespace = if classified { 299 } else { 199 };
                let source = run(floor, LIMIT, LIMIT, |out| {
                    let contracts = Contracts::derive(inventory, width, out)?;
                    let context = if classified {
                        contracts.emit(99, out)?;
                        ByteContext::classified(width, &contracts, 99)
                    } else {
                        ByteContext::native(width)
                    };
                    ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        context,
                        &allocations,
                        out,
                    )?
                    .emit(namespace, out)
                })
                .0
                .unwrap();
                let interpretation = if classified {
                    "(byte_target_view_contracts_match_99_v38(done.memory, true) || byte_target_view_contracts_match_99_v38(done.memory, false))"
                } else {
                    "byte_native_view_inputs_v38(done.memory, done.values)"
                };
                let definitions = inventory.definitions().len();
                let predicate = format!(
                    "done.values.len() == {definitions} && byte_state_memory_well_formed_v30(done) && {interpretation}"
                );
                let declaration = format!(
                    "spec fn byte_control_inputs_{namespace}_v99(done: MemoryStateV30) -> bool {{ {predicate} }}\n"
                );
                assert_eq!(source.matches(&declaration).count(), 1);
                let use_site = format!("!byte_control_inputs_{namespace}_v99(done)");
                assert_eq!(source.matches(&use_site).count(), count + 1);
                for block in inventory.functions()[0].blocks.clone() {
                    assert!(source.contains(&format!(
                        "if (done.pc != {block} && !trapped) || {use_site} {{ byte_block_refused_v58(done, observations) }} else {{"
                    )));
                    assert_eq!(
                        source
                            .matches(&format!("spec fn byte_control_{namespace}_{block}_v30("))
                            .count(),
                        1
                    );
                }
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
                let inline = source
                    .replacen(&declaration, "", 1)
                    .replace(&use_site, &format!("!({predicate})"));
                assert!(inline.len() > source.len() + 10_000);
                assert!(!source.contains("assume(") && !source.contains("external_body"));
            }
        }
    });
}

#[test]
fn shared_control_inputs_keep_exact_and_one_short_emission_budgets() {
    with_inventory(&trap_module_v40(3), |inventory, physical, floor| {
        let allocations = NoAllocations(inventory.owner());
        for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
            for classified in [false, true] {
                let emit = |out: &mut Writer<'_, '_>| {
                    let contracts = Contracts::derive(inventory, width, out)?;
                    let context = if classified {
                        contracts.emit(99, out)?;
                        ByteContext::classified(width, &contracts, 99)
                    } else {
                        ByteContext::native(width)
                    };
                    ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        context,
                        &allocations,
                        out,
                    )?
                    .emit(399, out)
                };
                let measured = run(floor, LIMIT, LIMIT, emit);
                let source = measured.0.unwrap();
                let exact = run(floor, measured.1, measured.2, emit);
                assert_eq!(exact.0.unwrap(), source);
                assert_eq!((exact.1, exact.2), (measured.1, measured.2));
                assert!(matches!(run(floor, measured.1 - 1, measured.2, emit).0,
                    Err(Error::Resource(Resource::Work(limit)))
                        if limit.limit() == measured.1 - 1 && limit.actual() == measured.1));
                assert!(matches!(run(floor, measured.1, measured.2 - 1, emit).0,
                    Err(Error::Resource(Resource::Storage(limit)))
                        if limit.limit() == measured.2 - 1 && limit.actual() == measured.2));
            }
        }
    });
}
