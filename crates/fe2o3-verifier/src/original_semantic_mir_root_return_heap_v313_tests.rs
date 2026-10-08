//! Source/target shape and emitted proof checks, not Verus qualification.
use super::super::super::super::super::super::invocations::tests as fixtures;
use super::super::super::super::super::slots::tests::with_tile_slots;
use super::*;
use fe2o3_kernel_ir::{ExecutionTileLayoutV1 as Layout, ValueId};

const LIMIT: usize = 512 * 1024 * 1024;
const RETURN_SUPPORT: [&str; 5] = [
    "invocation_private_free_frame_end_identity_v77",
    "invocation_source_put_local_preserves_heap_v78",
    "invocation_source_plain_return_install_preserves_heap_v78",
    "invocation_source_plain_return_preserves_heap_v78",
    "invocation_empty_private_map_has_no_private_source_v78",
];

fn derive(
    model: &PairedInvocations<'_, '_, '_>,
    root: usize,
    block: usize,
    hint: &SourceCutHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<Option<Summary>> {
    let scan = super::scan(model, root, out)?;
    super::derive(model, &scan, root, block, hint, out)
}

fn optional_names(text: &str) -> std::collections::BTreeSet<&'static str> {
    let mut found = std::collections::BTreeSet::new();
    for packet in [
        include_str!("original_semantic_mir_source_constructor_laws_v81.vrs"),
        include_str!("original_semantic_mir_cut_frame_laws_v93.vrs"),
    ] {
        for item in packet.split("// fe2o3_optional_support_v97: ").skip(1) {
            let (name, body) = item.split_once('\n').unwrap();
            if name == "END" {
                continue;
            }
            if text.contains(&format!("proof fn {name}(")) {
                assert_eq!(text.matches(body).count(), 1, "{name}");
                found.insert(name);
            } else {
                assert!(!text.contains(name), "omitted referenced support {name}");
            }
        }
    }
    found
}

fn heap_body(text: &str, root: usize, pc: usize) -> &str {
    text.split_once(&format!(
        "proof fn invocation_paired_cut_{root}_pc{pc}_heap_v85("
    ))
    .unwrap()
    .1
    .split("proof fn ")
    .next()
    .unwrap()
}

fn run(
    layout: Layout,
    work: usize,
    storage: usize,
    enabled: bool,
    examine: impl FnOnce(&str),
) -> (Result<()>, usize, usize, usize) {
    fixtures::run_variant(work, storage, false, |plan, out| {
        with_tile_slots(plan, layout, out, |slots, out| {
            let mut program = SourceByteProgram::derive(plan, slots, out)?;
            let mut paired =
                PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
            let mut selected = Vec::new();
            for (root, row) in paired.roots.iter().enumerate() {
                let hints = row.step_hints.as_ref().unwrap();
                assert!(!hints.conserves_heap);
                for (block, cut) in row.cuts.iter().enumerate() {
                    let Some(cut) = cut else { continue };
                    let hint = hints
                        .cuts
                        .iter()
                        .find(|hint| hint.pc == cut.source)
                        .unwrap();
                    if let Some(summary) = derive(&paired, root, block, hint, out)? {
                        assert_eq!(cut.instance, row.instances.start);
                        assert_eq!(hint.statements, 0);
                        assert_eq!(hint.root_unit_return_v313.as_ref().unwrap().0, row.owner);
                        assert_eq!(summary.target_pc, row.blocks.start + block);
                        selected.push((root, summary));
                    }
                }
            }
            assert_eq!(selected.len(), paired.roots.len());
            assert!(selected.iter().any(|(root, summary)| *root > 0
                && summary.source_pc > 0
                && summary.target_pc > 0));
            if !enabled {
                for root in &mut paired.roots {
                    for hint in &mut root.step_hints.as_mut().unwrap().cuts {
                        hint.root_unit_return_v313 = None;
                    }
                }
            }
            program.emit(out)?;
            paired.emit(out)?;
            program.emit_cut_frame_proofs_v93(Some(&paired), out)?;
            super::super::super::super::super::support_closure::retain_referenced(out)?;
            for (root, summary) in selected {
                let row = &paired.roots[root];
                let hint = row
                    .step_hints
                    .as_ref()
                    .unwrap()
                    .cuts
                    .iter()
                    .find(|hint| hint.pc == summary.source_pc)
                    .unwrap();
                let instance = hint.instance;
                let source_fuel = hint.statements.checked_add(1).unwrap();
                let follow_fuel = super::super::target_follow_fuel(&paired, row, out)?;
                let body = heap_body(&out.text, root, summary.source_pc);
                let header = body.split_once("\n{\n").unwrap().0;
                assert_eq!(
                    header,
                    format!(
                        "source: InvocationSourceByteStateV36, target: MemoryStateV30)\n requires invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1),\n source.machine.pc == {},\n ensures invocation_byte_states_related_v36(invocation_paired_source_step_{root}_v36(source).state.machine, invocation_paired_actual_step_{root}_v36(target).state, invocation_source_byte_map_{root}_v36(invocation_paired_source_step_{root}_v36(source).state, invocation_paired_actual_step_{root}_v36(target).state)),",
                        summary.source_pc,
                    )
                );
                if enabled {
                    assert!(body.contains(&format!("invocation_source_plain_return_preserves_heap_v78(source, {}, {}, MemoryValueV30::Unit, None, -1int, little_endian);", summary.begin, summary.end)));
                    assert!(body.contains("invocation_empty_private_map_has_no_private_source_v78(source.machine.memory, target.memory, map);"));
                    assert!(body.contains("assert(original == returned.source)"));
                    assert!(
                        body.contains("assert(actual == MemoryStateV30 { pc: -1int, ..target })")
                    );
                    assert!(body.contains(
                        "original.machine.frames.execution == source.machine.frames.execution"
                    ));
                } else {
                    assert!(!body.contains("invocation_source_plain_return_preserves_heap_v78("));
                    assert!(body.contains(&format!(
                        "reveal_with_fuel(invocation_source_micro_run_{root}_{instance}_v36, {source_fuel});"
                    )));
                    assert!(body.contains(&format!(
                        "reveal_with_fuel(invocation_byte_follow_{root}_v36, {follow_fuel});"
                    )));
                }
                let mut ordinary = false;
                for line in body.split_once("\n{\n").unwrap().1.lines().map(str::trim) {
                    if line.starts_with("hide(") {
                        assert!(!ordinary);
                    } else if !line.is_empty() {
                        ordinary = true;
                    }
                }
                for forbidden in ["assume(", "admit(", "external_body"] {
                    assert!(!body.contains(forbidden));
                }
            }
            let retained = optional_names(&out.text);
            for name in RETURN_SUPPORT {
                assert_eq!(retained.contains(name), enabled, "{name}");
            }
            examine(&out.text);
            Ok(())
        })
    })
}

#[test]
fn root_unit_return_heap_uses_actual_coordinates_and_original_headers_in_both_layouts() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run(layout, LIMIT, LIMIT, true, |_| {}).0.unwrap();
    }
}

#[test]
fn root_unit_return_heap_requires_exact_target_return_shape() {
    let empty = Terminator::Return { values: vec![] };
    assert!(target_shape(0, 0, &empty));
    assert!(!target_shape(1, 0, &empty));
    assert!(!target_shape(0, 1, &empty));
    assert!(!target_shape(0, 0, &Terminator::Unreachable));
    assert!(!target_shape(
        0,
        0,
        &Terminator::Return {
            values: vec![ValueId(0)]
        }
    ));
}

#[test]
fn root_unit_return_heap_declines_missing_or_inconsistent_source_guards() {
    for layout in [Layout::Blocked, Layout::Striped] {
        fixtures::run_variant(LIMIT, LIMIT, false, |plan, out| {
            with_tile_slots(plan, layout, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let mut paired =
                    PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                let root = 1;
                let row = &paired.roots[root];
                let (block, pc) = row
                    .cuts
                    .iter()
                    .enumerate()
                    .find_map(|(block, cut)| {
                        cut.as_ref()
                            .filter(|cut| {
                                cut.instance == row.instances.start
                                    && matches!(cut.end, End::Return)
                            })
                            .map(|cut| (block, cut.source))
                    })
                    .unwrap();
                let hints = paired.roots[root].step_hints.as_mut().unwrap();
                let index = hints.cuts.iter().position(|hint| hint.pc == pc).unwrap();
                let mut hint = hints.cuts.remove(index);
                {
                    let inventory = paired.slots.correspondence(out)?.inventory(out.budget)?;
                    let row = &paired.roots[root];
                    let expected_scan = 1
                        + row.blocks.len()
                        + inventory.blocks()[row.blocks.clone()]
                            .iter()
                            .map(|block| block.operations.len())
                            .sum::<usize>();
                    let before = out.budget.work();
                    paired.check(out)?;
                    paired.slots.correspondence(out)?.inventory(out.budget)?;
                    let scan_auth_work = out.budget.work() - before;
                    let before = out.budget.work();
                    let scan = super::scan(&paired, root, out)?;
                    assert_eq!(out.budget.work() - before, scan_auth_work + expected_scan);
                    let before = out.budget.work();
                    paired.check(out)?;
                    paired
                        .slots
                        .check_query_storage_floor(scan.required, out.budget)?;
                    paired.slots.correspondence(out)?.inventory(out.budget)?;
                    let cut_auth_work = out.budget.work() - before;
                    let after_scan = out.budget.work();
                    for _ in 0..128 {
                        assert!(super::derive(&paired, &scan, root, block, &hint, out)?.is_some());
                    }
                    assert_eq!(out.budget.work() - after_scan, 128 * (10 + cut_auth_work));
                    let before = out.budget.storage();
                    assert!(matches!(
                        super::derive(&paired, &scan, 0, block, &hint, out),
                        Err(Error::Statement(_))
                    ));
                    assert_eq!(out.budget.storage(), before);
                }
                assert!(derive(&paired, root, block, &hint, out)?.is_some());
                let coordinates = hint.root_unit_return_v313.take();
                assert!(derive(&paired, root, block, &hint, out)?.is_none());
                hint.root_unit_return_v313 = coordinates;
                hint.statements = 1;
                assert!(derive(&paired, root, block, &hint, out)?.is_none());
                hint.statements = 0;
                hint.operands = 1;
                assert!(derive(&paired, root, block, &hint, out)?.is_none());
                hint.operands = 0;
                hint.call = Some(
                    super::super::super::super::super::source_function::SourceCallHintsV85 {
                        child: 0,
                        arguments: vec![],
                    },
                );
                assert!(derive(&paired, root, block, &hint, out)?.is_none());
                hint.call = None;
                let (owner, _) = hint.root_unit_return_v313.as_mut().unwrap();
                let saved_owner = *owner;
                *owner = u32::MAX;
                assert!(derive(&paired, root, block, &hint, out)?.is_none());
                hint.root_unit_return_v313.as_mut().unwrap().0 = saved_owner;
                let saved = hint.root_unit_return_v313.as_ref().unwrap().1.clone();
                hint.root_unit_return_v313.as_mut().unwrap().1 = 1..0;
                assert!(derive(&paired, root, block, &hint, out)?.is_none());
                hint.root_unit_return_v313.as_mut().unwrap().1 = 0..usize::MAX;
                assert!(derive(&paired, root, block, &hint, out)?.is_none());
                hint.root_unit_return_v313.as_mut().unwrap().1 = saved;
                hint.pc += 1;
                assert!(matches!(
                    derive(&paired, root, block, &hint, out),
                    Err(Error::Statement(_))
                ));
                hint.pc -= 1;
                hint.instance = 1;
                assert!(matches!(
                    derive(&paired, root, block, &hint, out),
                    Err(Error::Statement(_))
                ));
                hint.instance = 0;
                assert!(derive(&paired, root, block, &hint, out)?.is_some());
                let cut = paired.roots[root].cuts[block].as_mut().unwrap();
                cut.end = End::Ordinary;
                assert!(derive(&paired, root, block, &hint, out)?.is_none());
                paired.roots[root].cuts[block].as_mut().unwrap().end = End::Return;
                let instance = paired.roots[root].instances.start;
                let owner = paired.instances[instance]
                    .as_mut()
                    .unwrap()
                    .owners
                    .pop()
                    .unwrap();
                assert!(derive(&paired, root, block, &hint, out)?.is_none());
                paired.instances[instance]
                    .as_mut()
                    .unwrap()
                    .owners
                    .push(owner);
                let binding = paired.instances[instance + 1].as_ref().unwrap().arguments[0];
                paired.instances[instance]
                    .as_mut()
                    .unwrap()
                    .suspended
                    .push(binding);
                assert!(derive(&paired, root, block, &hint, out)?.is_none());
                paired.instances[instance].as_mut().unwrap().suspended.pop();
                paired.instances[instance].as_mut().unwrap().returned = Some(binding);
                assert!(derive(&paired, root, block, &hint, out)?.is_none());
                paired.instances[instance].as_mut().unwrap().returned = None;
                assert!(derive(&paired, root, block, &hint, out)?.is_some());
                assert!(matches!(
                    derive(&paired, usize::MAX, block, &hint, out),
                    Err(Error::Statement(_))
                ));
                assert!(matches!(
                    derive(&paired, root, usize::MAX, &hint, out),
                    Err(Error::Statement(_))
                ));
                paired.roots[root]
                    .step_hints
                    .as_mut()
                    .unwrap()
                    .cuts
                    .insert(index, hint);
                assert!(out.text.is_empty());
                Ok(())
            })
        })
        .0
        .unwrap();
    }
}

#[test]
fn root_unit_return_heap_keeps_memory_bearing_roots_on_generic_path() {
    for layout in [Layout::Blocked, Layout::Striped] {
        fixtures::run_scalar_allocation_variant(LIMIT, LIMIT, |plan, out| {
            with_tile_slots(plan, layout, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let paired =
                    PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                let mut tested = 0;
                for (root, row) in paired.roots.iter().enumerate() {
                    let hints = row.step_hints.as_ref().unwrap();
                    for (block, cut) in row.cuts.iter().enumerate() {
                        let Some(cut) = cut else { continue };
                        let hint = hints
                            .cuts
                            .iter()
                            .find(|hint| hint.pc == cut.source)
                            .unwrap();
                        if hint.root_unit_return_v313.is_some() {
                            assert!(derive(&paired, root, block, hint, out)?.is_none());
                            tested += 1;
                        }
                    }
                }
                assert_eq!(tested, paired.roots.len());
                paired.emit(out)?;
                for (root, row) in paired.roots.iter().enumerate() {
                    let follow_fuel = super::super::target_follow_fuel(&paired, row, out)?;
                    for cut in row
                        .cuts
                        .iter()
                        .flatten()
                        .filter(|cut| matches!(cut.end, End::Return))
                    {
                        let hint = row
                            .step_hints
                            .as_ref()
                            .unwrap()
                            .cuts
                            .iter()
                            .find(|hint| hint.pc == cut.source)
                            .unwrap();
                        let instance = hint.instance;
                        let source_fuel = hint.statements.checked_add(1).unwrap();
                        let body = heap_body(&out.text, root, cut.source);
                        assert!(
                            !body.contains("invocation_source_plain_return_preserves_heap_v78(")
                        );
                        assert!(body.contains(&format!(
                            "reveal_with_fuel(invocation_source_micro_run_{root}_{instance}_v36, {source_fuel});"
                        )));
                        assert!(body.contains(&format!(
                            "reveal_with_fuel(invocation_byte_follow_{root}_v36, {follow_fuel});"
                        )));
                    }
                }
                Ok(())
            })
        })
        .0
        .unwrap();
    }
}

#[test]
fn root_unit_return_heap_retains_exact_optional_delta_without_changing_contracts() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let mut baseline = String::new();
        run(layout, LIMIT, LIMIT, false, |text| {
            baseline = text.to_owned()
        })
        .0
        .unwrap();
        run(layout, LIMIT, LIMIT, true, |text| {
            let old = optional_names(&baseline);
            let new = optional_names(text);
            assert!(old.is_subset(&new));
            assert_eq!(
                new.difference(&old)
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>(),
                RETURN_SUPPORT.into_iter().collect()
            );
            let headers = |text: &str| {
                text.split("proof fn ")
                    .skip(1)
                    .filter(|body| {
                        !RETURN_SUPPORT
                            .iter()
                            .any(|name| body.starts_with(&format!("{name}(")))
                    })
                    .map(|body| body.split_once("\n{").unwrap().0.to_owned())
                    .collect::<Vec<_>>()
            };
            assert_eq!(headers(text), headers(&baseline));
        })
        .0
        .unwrap();
    }
}

#[test]
fn root_unit_return_heap_refuses_foreign_and_refunded_accounts_before_any_debit() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    for foreign in [false, true] {
        let result = fixtures::run_variant(LIMIT, LIMIT, false, |plan, out| {
            with_tile_slots(plan, Layout::Blocked, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let paired =
                    PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                let root = 0;
                let row = &paired.roots[root];
                let (block, cut) = row
                    .cuts
                    .iter()
                    .enumerate()
                    .find_map(|(block, cut)| {
                        cut.as_ref()
                            .filter(|cut| {
                                cut.instance == row.instances.start
                                    && matches!(cut.end, End::Return)
                            })
                            .map(|cut| (block, cut))
                    })
                    .unwrap();
                let hint = row
                    .step_hints
                    .as_ref()
                    .unwrap()
                    .cuts
                    .iter()
                    .find(|hint| hint.pc == cut.source)
                    .unwrap();
                let scan = super::scan(&paired, root, out)?;
                if foreign {
                    let mut work = Work::new(LIMIT);
                    let mut budget = Budget::new(&mut work, LIMIT);
                    budget.reserve_storage(out.budget.storage())?;
                    let before = (budget.work(), budget.storage(), budget.peak_storage());
                    {
                        let mut writer = Writer::new(&mut budget)?;
                        assert!(matches!(
                            super::derive(&paired, &scan, root, block, hint, &mut writer),
                            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                        ));
                        assert!(writer.text.is_empty());
                    }
                    assert_eq!(
                        (budget.work(), budget.storage(), budget.peak_storage()),
                        before
                    );
                } else {
                    assert_eq!(out.budget.storage(), scan.required);
                    out.budget.release_storage(1)?;
                }
                let before = (
                    out.budget.work(),
                    out.budget.storage(),
                    out.budget.peak_storage(),
                    out.text.len(),
                );
                for _ in 0..2 {
                    assert!(matches!(
                        super::derive(&paired, &scan, root, block, hint, out),
                        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                    ));
                    assert_eq!(
                        (
                            out.budget.work(),
                            out.budget.storage(),
                            out.budget.peak_storage(),
                            out.text.len()
                        ),
                        before
                    );
                }
                super::derive(&paired, &scan, root, block, hint, out).map(|_| ())
            })
        });
        assert!(matches!(
            result.0,
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
    }
}

#[test]
fn root_unit_return_heap_exact_and_one_short_accounts_include_output() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let measured = run(layout, LIMIT, LIMIT, true, |_| {});
        measured.0.unwrap();
        assert_eq!(measured.2, fixtures::FLOOR);
        let exact = run(layout, measured.1, measured.3, true, |_| {});
        exact.0.unwrap();
        assert_eq!(exact.2, fixtures::FLOOR);
        for (work, storage) in [(measured.1 - 1, measured.3), (measured.1, measured.3 - 1)] {
            let short = run(layout, work, storage, true, |_| {});
            assert!(short.0.is_err());
            assert_eq!(short.2, fixtures::FLOOR);
        }
    }
    assert_eq!(
        headers(),
        size_of::<Summary>()
            + size_of::<RootScan<'_>>()
            + size_of::<Result<RootScan<'_>>>()
            + size_of::<Option<Summary>>()
            + size_of::<Result<Option<Summary>>>()
            + size_of::<Result<()>>()
            + size_of::<std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>>>()
            + size_of::<std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>>(
            )
            + 12 * size_of::<usize>()
            + 12 * size_of::<&()>()
    );
}
