use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18;
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticAggregateKindV1, SemanticOperandV1, SemanticPointerKindV1,
    SemanticPointerMetadataV1, SemanticProjectionKindV1, SemanticRvalueKindV1,
    SemanticStatementKindV1, SemanticTypeIdV1, SemanticTypeShapeV1,
};

const SHARED: &str = r#"
let local = a ^ b;
let option: Option<&u32> = if a & 1 == 0 { Some(&local) } else { None };
let result = match option {
    Some(value) => *value ^ b,
    None => a,
};
assert!(result == a);
"#;

const MUTABLE: &str = r#"
let mut local = a ^ b;
let option: Option<&mut u32> = if a & 1 == 0 { Some(&mut local) } else { None };
let result = match option {
    Some(value) => { *value = *value ^ b; *value },
    None => a,
};
assert!(result == a);
"#;

// Original counts: empty/reference constructors, discriminant, thin-reference
// borrow, downcast reference extraction, dereference read, promoted enum local,
// dereference write. Generated counts bind the first three to exact root sites.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct ReferenceEnumCensus {
    original_roots: [u32; 2],
    original: [[usize; 8]; 2],
    generated: [[usize; 3]; 2],
}

fn thin_reference(source: &AdmittedInertSemanticMirV1, ty: SemanticTypeIdV1) -> bool {
    matches!(
        source.types()[ty.index() as usize].shape(),
        SemanticTypeShapeV1::Pointer(pointer)
            if pointer.kind() == SemanticPointerKindV1::Reference
                && pointer.metadata() == SemanticPointerMetadataV1::None
    )
}

fn enum_type(source: &AdmittedInertSemanticMirV1, ty: SemanticTypeIdV1) -> bool {
    matches!(
        source.types()[ty.index() as usize].shape(),
        SemanticTypeShapeV1::Enum { .. }
    )
}

fn root_events(source: &str, root: usize) -> Option<&str> {
    let header = format!(
        "open spec fn invocation_source_byte_event_{root}_0_v36(block: int, statement: int) -> Option<InvocationSourceByteEventV36> {{\n"
    );
    let mut definitions = source.match_indices(&header);
    let (offset, _) = definitions.next()?;
    if definitions.next().is_some() {
        return None;
    }
    source[offset + header.len()..]
        .split_once("\n}\n")
        .map(|pair| pair.0)
}

fn exact_enum_event(body: Option<&str>, block: usize, statement: usize, kind: usize) -> bool {
    let Some(body) = body else { return false };
    let prefix = format!(" if block == {block} && statement == {statement} {{ Some(");
    let mut rows = body.match_indices(&prefix);
    let Some((offset, _)) = rows.next() else {
        return false;
    };
    if rows.next().is_some() {
        return false;
    }
    let tail = &body[offset + prefix.len()..];
    match kind {
        0 | 1 => tail.starts_with("InvocationSourceByteEventV36::EnumConstruct("),
        2 => tail.starts_with("InvocationSourceByteEventV36::Discriminant("),
        _ => false,
    }
}

pub(super) fn observe_reference_enums(
    owner: &ProductionSourceOwnedViewV18<'_>,
    generated: &str,
    budget: &mut Budget<'_>,
) -> Result<ReferenceEnumCensus, Error> {
    let source = owner.source_semantic(budget)?;
    let ssa = owner.source_ssa(budget)?;
    assert_eq!(source.roots().len(), 2);
    let mut result = ReferenceEnumCensus::default();
    for (root, function_id) in source.roots().iter().copied().enumerate() {
        budget.charge_work(1)?;
        result.original_roots[root] = function_id.index();
        let function = &source.functions()[function_id.index() as usize];
        let plan = ssa.plan_for_function(function_id).unwrap();
        assert_eq!(plan.function_identity(), function.identity());
        budget.charge_work(generated.len())?;
        let events = root_events(generated, root);
        for variable in plan.plan().promoted_variables() {
            budget.charge_work(1)?;
            let local = &function.locals()[variable.get() as usize];
            result.original[root][6] += usize::from(enum_type(source, local.ty()));
        }
        for (block, original_block) in function.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            for (statement, original) in original_block.statements().iter().enumerate() {
                budget.charge_work(1)?;
                if let SemanticStatementKindV1::Store(store) = original.kind() {
                    for projection in store.destination().projections() {
                        budget.charge_work(1)?;
                        result.original[root][7] +=
                            usize::from(projection.kind() == SemanticProjectionKindV1::Dereference);
                    }
                }
                let SemanticStatementKindV1::Assign(assignment) = original.kind() else {
                    continue;
                };
                let value = assignment.value();
                let mut event_kind = None;
                match value.kind() {
                    SemanticRvalueKindV1::Aggregate(aggregate) => {
                        if let (
                            SemanticAggregateKindV1::EnumVariant(variant),
                            SemanticTypeShapeV1::Enum { variants, .. },
                        ) = (
                            aggregate.kind(),
                            source.types()[value.result_type().index() as usize].shape(),
                        ) {
                            let fields = variants[*variant as usize].fields().fields();
                            assert_eq!(fields.len(), aggregate.operands().len());
                            if fields.is_empty() {
                                event_kind = Some(0);
                            } else {
                                for (field, operand) in fields.iter().zip(aggregate.operands()) {
                                    budget.charge_work(1)?;
                                    assert_eq!(*field, operand.ty());
                                }
                                if fields.len() == 1 && thin_reference(source, fields[0]) {
                                    event_kind = Some(1);
                                }
                            }
                        }
                    }
                    SemanticRvalueKindV1::Discriminant(place) if enum_type(source, place.ty()) => {
                        event_kind = Some(2);
                    }
                    SemanticRvalueKindV1::Borrow { .. } => {
                        result.original[root][3] +=
                            usize::from(thin_reference(source, value.result_type()));
                    }
                    _ => {}
                }
                if let Some(kind) = event_kind {
                    result.original[root][kind] += 1;
                    budget.charge_work(events.map_or(0, str::len))?;
                    result.generated[root][kind] +=
                        usize::from(exact_enum_event(events, block, statement, kind));
                }
                value.kind().try_visit_operands(|operand| {
                    budget.charge_work(1)?;
                    let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand
                    else {
                        return Ok::<_, Error>(());
                    };
                    let mut downcast = false;
                    let mut field = false;
                    let mut dereference = false;
                    for projection in place.projections() {
                        budget.charge_work(1)?;
                        match projection.kind() {
                            SemanticProjectionKindV1::Downcast(_) => downcast = true,
                            SemanticProjectionKindV1::Field(_) if downcast => field = true,
                            SemanticProjectionKindV1::Dereference => dereference = true,
                            _ => {}
                        }
                    }
                    result.original[root][4] += usize::from(
                        enum_type(
                            source,
                            function.locals()[place.local().index() as usize].ty(),
                        ) && downcast
                            && field
                            && thin_reference(source, place.ty()),
                    );
                    result.original[root][5] += usize::from(dereference);
                    Ok(())
                })?;
                if let SemanticRvalueKindV1::Load(load) = value.kind() {
                    for projection in load.source().projections() {
                        budget.charge_work(1)?;
                        result.original[root][5] +=
                            usize::from(projection.kind() == SemanticProjectionKindV1::Dereference);
                    }
                }
                for projection in assignment.destination().projections() {
                    budget.charge_work(1)?;
                    result.original[root][7] +=
                        usize::from(projection.kind() == SemanticProjectionKindV1::Dereference);
                }
            }
        }
    }
    Ok(result)
}

fn complete_reference_enum_census(census: &ReferenceEnumCensus, mutable: bool) -> bool {
    census.original_roots[0] != census.original_roots[1]
        && (0..2).all(|root| {
            census.original[root][..7].iter().all(|count| *count > 0)
                && (census.original[root][7] > 0) == mutable
                && census.generated[root] == census.original[root][..3]
        })
}

#[test]
fn original_reference_enum_event_census_requires_exact_root_instance_and_statement() {
    let definition = |root, instance| {
        format!(
            "open spec fn invocation_source_byte_event_{root}_{instance}_v36(block: int, statement: int) -> Option<InvocationSourceByteEventV36> {{\n if block == 1 && statement == 2 {{ Some(InvocationSourceByteEventV36::EnumConstruct(empty)) }} else if block == 2 && statement == 3 {{ Some(InvocationSourceByteEventV36::EnumConstruct(reference)) }} else if block == 4 && statement == 5 {{ Some(InvocationSourceByteEventV36::Discriminant(read)) }} else {{ None }}\n}}\n"
        )
    };
    let complete = |source: &str| {
        (0..2).all(|root| {
            let body = root_events(source, root);
            exact_enum_event(body, 1, 2, 0)
                && exact_enum_event(body, 2, 3, 1)
                && exact_enum_event(body, 4, 5, 2)
        })
    };
    let first = definition(0, 0);
    let both = format!("{first}{}", definition(1, 0));
    assert!(complete(&both));
    assert!(!complete(&first));
    assert!(!complete(&format!("{first}{}", definition(2, 0))));
    assert!(!complete(&format!(
        "{}{}",
        definition(0, 1),
        definition(1, 1)
    )));
    assert!(!complete(&format!("{both}{first}")));
    assert!(!complete(&both.replace("statement == 5", "statement == 6")));
    assert!(!complete(&both.replace(
        "Some(InvocationSourceByteEventV36::Discriminant(read))",
        "Some(InvocationSourceByteEventV36::Pure)",
    )));
    let helper = "open spec fn generic_enum_helper() {\n Some(InvocationSourceByteEventV36::EnumConstruct(reference)); Some(InvocationSourceByteEventV36::Discriminant(read));\n}\n";
    assert!(!complete(helper));
    assert!(!complete(&format!("{first}{helper}")));
    let duplicated_row = both.replace(
        " else { None }",
        " else if block == 4 && statement == 5 { Some(InvocationSourceByteEventV36::Discriminant(read)) } else { None }",
    );
    assert!(!complete(&duplicated_row));
}

#[test]
fn original_reference_enum_census_requires_all_original_events_and_promoted_carriers() {
    let complete = ReferenceEnumCensus {
        original_roots: [3, 9],
        original: [[1, 1, 1, 1, 1, 1, 1, 0]; 2],
        generated: [[1; 3]; 2],
    };
    assert!(complete_reference_enum_census(&complete, false));
    assert!(!complete_reference_enum_census(&complete, true));
    for root in 0..2 {
        for kind in 0..7 {
            let mut missing = complete.clone();
            missing.original[root][kind] = 0;
            assert!(
                !complete_reference_enum_census(&missing, false),
                "root {root}, kind {kind}"
            );
        }
        for kind in 0..3 {
            let mut missing = complete.clone();
            missing.generated[root][kind] = 0;
            assert!(!complete_reference_enum_census(&missing, false));
        }
    }
    let mut duplicated = complete.clone();
    duplicated.original_roots[1] = duplicated.original_roots[0];
    assert!(!complete_reference_enum_census(&duplicated, false));
    let mut mutable = complete;
    mutable.original[0][7] = 1;
    assert!(!complete_reference_enum_census(&mutable, true));
    mutable.original[1][7] = 1;
    assert!(complete_reference_enum_census(&mutable, true));
    assert!(!complete_reference_enum_census(&mutable, false));
}

#[test]
fn original_reference_enum_inputs_use_dynamic_safe_borrows_in_both_roots() {
    for (body, reference) in [(SHARED, "Some(&local)"), (MUTABLE, "Some(&mut local)")] {
        assert!(body.contains(reference));
        assert!(body.contains("if a & 1 == 0"));
        assert!(body.contains("match option"));
        assert!(body.contains("Some(value)"));
        assert!(body.contains("None => a"));
        assert!(body.contains("assert!(result == a)"));
        assert!(!body.contains("let _result"));
        for excluded in ["unsafe", "addr_of", "&raw", " as *", "transmute"] {
            assert!(!body.contains(excluded));
        }
        let program = original_program(body);
        assert_eq!(program.matches(reference).count(), 2);
        assert!(program.contains("pub fn z_original(a: u32, b: u32)"));
        assert!(program.contains("pub fn a_original(a: u32, b: u32)"));
    }
}

#[test]
#[ignore = "private actual-rustc child; invoked only by the reference enum parent"]
fn original_reference_enum_worker_child() {
    original_mir_worker_child();
}

#[test]
#[ignore = "requires pinned nightly rust-src/rustc-dev and authentic AMD dependencies"]
fn actual_original_mir_reference_enums_reach_worker_without_forced_addresses() {
    let module = ORIGINAL_CHILD.rsplit_once("::").unwrap().0;
    let child = format!("{module}::reference_enum_tests::original_reference_enum_worker_child");
    run_actual_sources::<OriginalObservation>(
        &[("option_shared", SHARED), ("option_mutable", MUTABLE)],
        &[(0, 0)],
        &child,
        "ORIGINAL_MIR_REFERENCE_ENUM_V45",
        original_program,
        |_, _, case, report, _| {
            assert!(matches!(case, "option_shared" | "option_mutable"));
            assert!(report.work > 0 && report.peak > 0);
            assert_eq!(&report.census[..2], &[2, 2]);
            assert_ne!(report.statement, [0; 32]);
            assert_eq!(report.witness_protocol, [false; 6]);
            assert!(
                complete_reference_enum_census(&report.reference_enums, case == "option_mutable"),
                "incomplete per-root reference enum coverage: {:?}",
                report.reference_enums,
            );
        },
    );
}
