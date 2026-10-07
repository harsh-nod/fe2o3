//! Unexecuted diagnostic export through the existing complete nominal generator.
//! This module is a child of the separately reviewed nonvacuity test module.
use super::super::super::{generate_refinement_inner_v49, generate_refinement_v36};
use super::*;
use fe2o3_kernel_ir::{EndiannessV2, ExplicitLaunchExtent, FormalIndexWidth};
use std::fmt::Write as _;

const MAX_MODEL_BYTES: usize = 16 * 1024 * 1024;
const HOOK_REFUSAL: &str = "diagnostic history hook refused before finalization";

#[path = "original_semantic_mir_invocation_source_history_boundaries_v293_tests.rs"]
mod boundary_tests;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Ordinary,
    Noop,
    Witnesses,
    Refuse,
}

fn run_complete(
    mode: Mode,
    examine: impl FnOnce(&[Witness], &[(usize, usize)], &str, [usize; 6]),
) -> (Result<()>, usize, usize, usize) {
    run_complete_bounded(LIMIT, LIMIT, mode, examine)
}

fn run_complete_bounded(
    work: usize,
    storage: usize,
    mode: Mode,
    examine: impl FnOnce(&[Witness], &[(usize, usize)], &str, [usize; 6]),
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_variant(
        work,
        storage,
        false,
        |outer_plan, out| {
            super::super::tests::with_slots(outer_plan, out, |outer_slots, out| {
                let relation = outer_slots.correspondence(out)?;
                let source = relation.source(out.budget)?;
                assert!(std::ptr::eq(source, outer_plan.source(out)?));
                let source_launch = source.source_launch(out.budget)?;
                assert_eq!(source_launch.roots().len(), source.root_count(out.budget)?);
                let mut launches = Vec::new();
                for (root, launch) in source_launch.roots().iter().enumerate() {
                    assert_eq!(launch.selected_root(), source.root(root, out.budget)?.0);
                    let rank = launch.source_rank();
                    let extents = launch.layout().global_extents();
                    assert!((1..=3).contains(&rank));
                    assert!(extents.into_iter().all(|extent| extent > 0));
                    launches.push(ExplicitLaunchExtent::Exact { rank, extents });
                }

                let mut selected = None;
                let mut selected_boundaries = None;
                let mut called = 0usize;
                let census = if mode == Mode::Ordinary {
                    generate_refinement_v36(
                        relation,
                        &launches,
                        FormalIndexWidth::Bits64,
                        EndiannessV2::Little,
                        out,
                    )?
                } else {
                    let mut hook = |plan: &InvocationPlan<'_, '_>,
                                    slots: &SourceSlots<'_, '_>,
                                    program: &SourceByteProgram<'_, '_, '_>,
                                    out: &mut Writer<'_, '_>| {
                        called += 1;
                        assert_eq!(called, 1);
                        assert!(std::ptr::eq(source, plan.source(out)?));
                        assert!(std::ptr::eq(
                            source,
                            slots.correspondence(out)?.source(out.budget)?
                        ));
                        assert!(std::ptr::eq(slots, program.slots));
                        assert!(out.text.starts_with("use vstd::prelude::*;\n"));
                        assert_eq!(out.text.matches("verus! {").count(), 1);
                        if mode == Mode::Refuse {
                            return Err(Error::Statement(HOOK_REFUSAL));
                        }
                        if mode == Mode::Witnesses {
                            let rows = witnesses(plan, program, out)?;
                            assert!(!rows.is_empty());
                            assert!(rows.len() <= 256);
                            for row in &rows {
                                let (r, i) = (row.root, row.instance);
                                for actual in [
                                    format!(
                                        "proof fn invocation_source_micro_run_history_{r}_{i}_v293("
                                    ),
                                    format!(
                                        "proof fn invocation_source_micro_run_composes_{r}_{i}_v292("
                                    ),
                                    format!("spec fn invocation_source_micro_step_{r}_{i}_v36("),
                                ] {
                                    assert_eq!(out.text.matches(&actual).count(), 1, "{actual}");
                                }
                                write!(out, "{}", render(row)).map_err(|_| out.error())?;
                            }
                            let coordinates = boundary_tests::emit(plan, program, out)?;
                            assert!(selected_boundaries.replace(coordinates).is_none());
                            assert!(selected.replace(rows).is_none());
                        }
                        Ok(())
                    };
                    generate_refinement_inner_v49(
                        relation,
                        &launches,
                        FormalIndexWidth::Bits64,
                        EndiannessV2::Little,
                        None,
                        &[],
                        out,
                        Some(&mut hook),
                    )?
                };
                assert_eq!(called, usize::from(mode != Mode::Ordinary));
                assert_eq!(census[0], launches.len());
                let rows = selected.unwrap_or_default();
                let boundaries = selected_boundaries.unwrap_or_default();
                assert_eq!(!rows.is_empty(), mode == Mode::Witnesses);
                assert_eq!(!boundaries.is_empty(), mode == Mode::Witnesses);
                assert!(out.text.len() <= MAX_MODEL_BYTES);
                assert!(out.text.ends_with("}\n"));
                // The shared generator has now retained support and closed the model.
                for row in &rows {
                    assert!(out.text.contains(&render(row)));
                }
                for &(root, instance) in &boundaries {
                    assert!(out.text.contains(&boundary_tests::render(root, instance)));
                }
                examine(&rows, &boundaries, &out.text, census);
                Ok(())
            })
        },
    )
}

fn proof_names(row: &Witness) -> [String; 4] {
    let suffix = format!("{}_{}_{}_v293", row.root, row.instance, row.block);
    [
        format!("history_witness_initial_active_{suffix}"),
        format!("history_witness_undefined_statement_records_{suffix}"),
        format!("history_witness_actual_valid_prefix_{suffix}"),
        format!("history_witness_at_count_refuses_without_record_{suffix}"),
    ]
}

fn write_record(
    output: &mut impl std::io::Write,
    rows: &[Witness],
    boundaries: &[(usize, usize)],
    model: &str,
    generation_census: [usize; 6],
) {
    use sha2::{Digest, Sha256};
    assert!(!rows.is_empty());
    assert!(rows.len() <= 256);
    assert!(!boundaries.is_empty() && boundaries.len() <= 256);
    assert!(boundaries.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(
        rows.iter()
            .all(|row| boundaries.contains(&(row.root, row.instance)))
    );
    assert!(model.len() <= MAX_MODEL_BYTES);
    write!(output, "{{\"kind\":\"fe2o3-source-micro-history-witness-boundary-model-v293\",\"authority\":false,\"frontend_checked\":false,\"proof_success\":false,\"finalized\":true,\"index_bits\":64,\"little_endian\":true,\"generation_census\":{generation_census:?},\"witness_template_sha256\":\"").unwrap();
    for byte in Sha256::digest(TEMPLATE.as_bytes()) {
        write!(output, "{byte:02x}").unwrap();
    }
    write!(output, "\",\"boundary_template_sha256\":\"").unwrap();
    for byte in Sha256::digest(boundary_tests::TEMPLATE.as_bytes()) {
        write!(output, "{byte:02x}").unwrap();
    }
    write!(output, "\",\"boundary_instances\":[").unwrap();
    for (ordinal, &(root, instance)) in boundaries.iter().enumerate() {
        if ordinal != 0 {
            write!(output, ",").unwrap();
        }
        write!(
            output,
            "{{\"root\":{root},\"instance\":{instance},\"proofs\":["
        )
        .unwrap();
        for (index, name) in boundary_tests::proof_names(root, instance)
            .iter()
            .enumerate()
        {
            assert_eq!(model.matches(&format!("proof fn {name}(")).count(), 1);
            if index != 0 {
                write!(output, ",").unwrap();
            }
            write!(output, "\"{name}\"").unwrap();
        }
        write!(output, "]}}").unwrap();
    }
    write!(output, "],\"selected_blocks\":[").unwrap();
    for (ordinal, row) in rows.iter().enumerate() {
        if ordinal != 0 {
            write!(output, ",").unwrap();
        }
        write!(output, "{{\"root\":{},\"instance\":{},\"block\":{},\"pc\":{},\"locals_end\":{},\"owners\":{:?},\"inputs\":{:?},\"destinations\":{:?},\"proofs\":[",
            row.root, row.instance, row.block, row.pc, row.locals_end, row.owners,
            row.inputs, row.destinations).unwrap();
        for (index, name) in proof_names(row).iter().enumerate() {
            assert_eq!(model.matches(&format!("proof fn {name}(")).count(), 1);
            if index != 0 {
                write!(output, ",").unwrap();
            }
            write!(output, "\"{name}\"").unwrap();
        }
        write!(output, "]}}").unwrap();
    }
    write!(output, "],\"bytes\":{},\"sha256\":\"", model.len()).unwrap();
    for byte in Sha256::digest(model.as_bytes()) {
        write!(output, "{byte:02x}").unwrap();
    }
    write!(output, "\",\"model_hex\":\"").unwrap();
    for byte in model.as_bytes() {
        write!(output, "{byte:02x}").unwrap();
    }
    writeln!(output, "\"}}").unwrap();
}

#[test]
fn source_micro_history_complete_hook_preserves_ordinary_bytes_and_refusal() {
    let mut ordinary = None;
    run_complete(Mode::Ordinary, |rows, boundaries, model, census| {
        assert!(rows.is_empty() && boundaries.is_empty());
        ordinary = Some((model.to_owned(), census));
    })
    .0
    .unwrap();
    let (ordinary, expected_census) = ordinary.unwrap();
    run_complete(Mode::Noop, |rows, boundaries, model, census| {
        assert!(rows.is_empty() && boundaries.is_empty());
        assert_eq!(model, ordinary);
        assert_eq!(census, expected_census);
        assert!(!model.contains("proof fn history_witness_"));
        for prefix in boundary_tests::PROOF_PREFIXES {
            assert!(!model.contains(&format!("proof fn {prefix}_")));
        }
    })
    .0
    .unwrap();
    let refused = run_complete(Mode::Refuse, |_, _, _, _| {
        panic!("a refused hook must not reach complete-model export")
    });
    assert!(matches!(refused.0, Err(Error::Statement(detail)) if detail == HOOK_REFUSAL));
}

#[test]
fn source_micro_history_complete_export_retains_exact_witnesses_and_support() {
    run_complete(Mode::Witnesses, |rows, boundaries, model, census| {
        for definition in [
            "struct MemoryStateV30",
            "struct InvocationSourceByteStateV36",
            "struct InvocationSourceMicroStateV36",
            "spec fn invocation_source_byte_state_well_formed_v36(",
        ] {
            assert_eq!(model.matches(definition).count(), 1, "{definition}");
        }
        assert_eq!(model.matches("verus! {").count(), 1);
        assert!(model.ends_with("}\n"));
        assert_eq!(
            model.matches("proof fn history_witness_").count(),
            rows.len() * 4
        );
        for row in rows {
            assert!(model.contains(&render(row)));
            for (statement, inputs) in row.inputs.iter().enumerate() {
                let scalar =
                    scalar_declaration(model, row.root, row.instance, row.block, statement);
                for local in inputs {
                    assert!(scalar.contains(&format!(
                        "!invocation_source_byte_value_typed_v36(n.machine.values[{local}], 32)"
                    )));
                }
            }
        }
        let mut record = Vec::new();
        write_record(&mut record, rows, boundaries, model, census);
        let record = String::from_utf8(record).unwrap();
        assert!(
            record.starts_with(
                "{\"kind\":\"fe2o3-source-micro-history-witness-boundary-model-v293\""
            )
        );
        assert!(
            record
                .contains("\"authority\":false,\"frontend_checked\":false,\"proof_success\":false")
        );
        let hex = record
            .split_once("\"model_hex\":\"")
            .unwrap()
            .1
            .strip_suffix("\"}\n")
            .unwrap();
        assert_eq!(hex.len(), model.len() * 2);
        for (pair, byte) in hex.as_bytes().chunks_exact(2).zip(model.bytes()) {
            let pair = std::str::from_utf8(pair).unwrap();
            assert_eq!(u8::from_str_radix(pair, 16).unwrap(), byte);
        }
    })
    .0
    .unwrap();
}

#[test]
#[ignore = "diagnostic complete source history witnesses; no executed proof authority"]
fn diagnostic_complete_source_micro_history_witness_boundary_model_export_v293() {
    use std::io::{BufWriter, Write as _};
    run_complete(Mode::Witnesses, |rows, boundaries, model, census| {
        let mut output = BufWriter::new(std::io::stdout().lock());
        write_record(&mut output, rows, boundaries, model, census);
        output.flush().unwrap();
    })
    .0
    .unwrap();
}
