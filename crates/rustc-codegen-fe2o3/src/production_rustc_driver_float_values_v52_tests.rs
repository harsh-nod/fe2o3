use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBinaryOpV1, SemanticOperandV1, SemanticProjectionKindV1, SemanticRvalueKindV1,
    SemanticScalarTypeV1, SemanticStatementKindV1, SemanticTypeShapeV1,
};

#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct FloatCensus {
    roots: [u32; 2],
    // Per root: original F32/F64 adds, matched source events, loads, stores.
    rows: [[usize; 6]; 2],
}

fn root_events(source: &str, root: usize) -> Option<&str> {
    let header = format!(
        "open spec fn invocation_source_byte_event_{root}_0_v36(block: int, statement: int) -> Option<InvocationSourceByteEventV36> {{\n"
    );
    let mut matches = source.match_indices(&header);
    let (offset, _) = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    source[offset + header.len()..]
        .split_once("\n}\n")
        .map(|pair| pair.0)
}

fn exact_add(body: Option<&str>, block: usize, statement: usize, width: u16) -> bool {
    let Some(body) = body else {
        return false;
    };
    let prefix = format!(" if block == {block} && statement == {statement} {{ Some(");
    let mut rows = body.match_indices(&prefix);
    let Some((offset, _)) = rows.next() else {
        return false;
    };
    if rows.next().is_some() {
        return false;
    }
    let row = body[offset + prefix.len()..]
        .split(" } else")
        .next()
        .unwrap();
    row.starts_with(
        "InvocationSourceByteEventV36::ScalarOperands(InvocationSourceScalarOperandsV48 {",
    ) && row.contains(&format!(
        ", operation: 11int, input_bits: {width}int, input_signed: false, output_bits: {width}int"
    ))
}

pub(super) fn observe_floats(
    owner: &ProductionSourceOwnedViewV18<'_>,
    generated: &str,
    budget: &mut Budget<'_>,
) -> Result<FloatCensus, Error> {
    let source = owner.source_semantic(budget)?;
    assert_eq!(source.roots().len(), 2);
    let mut result = FloatCensus::default();
    for (root, function_id) in source.roots().iter().copied().enumerate() {
        budget.charge_work(1 + generated.len())?;
        result.roots[root] = function_id.index();
        let events = root_events(generated, root);
        let function = &source.functions()[function_id.index() as usize];
        let float_width = |ty: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1| match source
            .types()[ty.index() as usize]
            .shape()
        {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float {
                bits: bits @ (32 | 64),
            }) => Some(*bits),
            _ => None,
        };
        for (block, original_block) in function.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            for (statement, original) in original_block.statements().iter().enumerate() {
                budget.charge_work(1)?;
                if let SemanticStatementKindV1::Store(store) = original.kind() {
                    if float_width(store.destination().ty()).is_some() {
                        result.rows[root][5] += 1;
                    }
                }
                let SemanticStatementKindV1::Assign(assignment) = original.kind() else {
                    continue;
                };
                let value = assignment.value();
                if let SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left,
                    right,
                } = value.kind()
                {
                    if let Some(bits) = float_width(left.ty()) {
                        assert_eq!(right.ty(), left.ty());
                        assert_eq!(value.result_type(), left.ty());
                        let width = usize::from(bits == 64);
                        result.rows[root][width] += 1;
                        budget.charge_work(events.map_or(0, str::len))?;
                        result.rows[root][2 + width] +=
                            usize::from(exact_add(events, block, statement, bits));
                    }
                }
                value.kind().try_visit_operands(|operand| {
                    budget.charge_work(1)?;
                    if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) = operand
                    {
                        if float_width(place.ty()).is_some() {
                            for projection in place.projections() {
                                budget.charge_work(1)?;
                                result.rows[root][4] += usize::from(
                                    projection.kind() == SemanticProjectionKindV1::Dereference,
                                );
                            }
                        }
                    }
                    Ok::<_, Error>(())
                })?;
                if let SemanticRvalueKindV1::Load(load) = value.kind() {
                    if float_width(load.source().ty()).is_some() {
                        result.rows[root][4] += 1;
                    }
                }
                if float_width(assignment.destination().ty()).is_some() {
                    for projection in assignment.destination().projections() {
                        budget.charge_work(1)?;
                        result.rows[root][5] +=
                            usize::from(projection.kind() == SemanticProjectionKindV1::Dereference);
                    }
                }
            }
        }
    }
    Ok(result)
}

#[test]
fn original_float_event_census_requires_exact_roots_sites_width_and_opcode() {
    let definition = |root, instance, bits| {
        format!(
            "open spec fn invocation_source_byte_event_{root}_{instance}_v36(block: int, statement: int) -> Option<InvocationSourceByteEventV36> {{\n if block == 2 && statement == 3 {{ Some(InvocationSourceByteEventV36::ScalarOperands(InvocationSourceScalarOperandsV48 {{ destination: d, operation: 11int, input_bits: {bits}int, input_signed: false, output_bits: {bits}int }})) }} else {{ None }}\n}}\n"
        )
    };
    for bits in [32, 64] {
        let first = definition(0, 0, bits);
        let both = format!("{first}{}", definition(1, 0, bits));
        let complete =
            |text: &str| (0..2).all(|root| exact_add(root_events(text, root), 2, 3, bits));
        assert!(complete(&both));
        for bad in [
            first.clone(),
            format!("{both}{first}"),
            format!("{}{}", definition(0, 1, bits), definition(1, 1, bits)),
            both.replace("operation: 11int", "operation: 12int"),
            both.replace("statement == 3", "statement == 4"),
            both.replace("input_signed: false", "input_signed: true"),
            both.replace(&format!("input_bits: {bits}int"), "input_bits: 16int"),
        ] {
            assert!(!complete(&bad));
        }
    }
}

fn float_program(element: &str) -> String {
    assert!(matches!(element, "f32" | "f64"));
    let kernel = |name| {
        format!(
            r#"
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn {name}(a: &[{element}], b: &[{element}], mut c: fe2o3_device::DisjointSlice<{element}>) {{
    let index = fe2o3_device::thread::index_1d();
    let i = index.get();
    let Some(destination) = c.get_mut(index) else {{ return; }};
    if i >= a.len() || i >= b.len() {{ fe2o3_device::trap(); }}
    *destination = a[i] + b[i];
}}
"#
        )
    };
    format!(
        "use fe2o3_device::kernel;\n{}{}",
        kernel("z_original"),
        kernel("a_original")
    )
}

#[test]
#[ignore = "requires pinned nightly rust-src/rustc-dev and authentic AMD dependencies"]
fn actual_original_float_load_add_store_reaches_the_typed_final_worker() {
    run_actual_sources::<OriginalObservation>(
        &[("f32", "f32"), ("f64", "f64")],
        &[(0, 0)],
        ORIGINAL_CHILD,
        "ORIGINAL_FLOAT_WORKER_V52",
        float_program,
        |_, _, case, report, _| {
            let width = usize::from(case == "f64");
            assert!(report.work > 0 && report.peak > 0);
            assert_ne!(report.floats.roots[0], report.floats.roots[1]);
            for row in report.floats.rows {
                assert!(row[width] > 0 && row[4] >= 2 && row[5] > 0);
                assert_eq!(row[2 + width], row[width]);
                assert_eq!(row[1 - width], 0);
            }
        },
    );
}
