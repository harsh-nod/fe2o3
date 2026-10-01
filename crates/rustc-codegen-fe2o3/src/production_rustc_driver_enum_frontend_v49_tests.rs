//! Compiler-import observations only. The unchanged assertion-bearing reference
//! enum Worker parent remains separately required for end-to-end admission.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticAggregateKindV1, SemanticOperandV1,
    SemanticProjectionKindV1, SemanticRvalueKindV1, SemanticStatementKindV1,
    SemanticTerminatorKindV1, SemanticTypeShapeV1,
};

const SIGNED: &str = r#"
#[repr(i32)]
enum Choice { Negative(u32) = -7, Named { value: u32 } = 23, Empty = 91 }
use Choice::{Negative as Left, Named as Right, Empty as Absent};
let choice = if a & 1 == 0 { Left(a) } else if a & 2 == 0 { Right { value: b } } else { Absent };
let _result = match choice { Left(value) => value ^ b, Right { value } => value ^ a, _ => a | b };
"#;

const CONSTANTS: &str = r#"
mod values { pub const Some: u32 = 17; pub const None: u32 = 93; }
use values::{Some as Present, None as Absent};
let _result = match a { Present => a ^ b, Absent => a | b, _ => b };
"#;

#[derive(Debug, Default, Serialize, Deserialize)]
struct ImportedRoot {
    enum_tags: Vec<Vec<u128>>,
    constructors: [usize; 3],
    discriminants: usize,
    downcasts: usize,
    switch_values: Vec<u128>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ImportReport {
    roots: [ImportedRoot; 2],
    semantic_bytes: usize,
    later_outcome: String,
}

fn complete_import(case: &str, report: &ImportReport) -> bool {
    report.semantic_bytes > 0
        && !report.later_outcome.is_empty()
        && report.roots.iter().all(|root| match case {
            "signed_constructors" => {
                root.enum_tags == [vec![u128::from((-7_i32) as u32), 23, 91]]
                    && root.constructors.iter().all(|count| *count > 0)
                    && root.discriminants > 0
                    && root.downcasts > 0
                    && root.switch_values.contains(&u128::from((-7_i32) as u32))
                    && root.switch_values.contains(&23)
            }
            "constant_lookalikes" => {
                root.enum_tags.is_empty()
                    && root.constructors == [0; 3]
                    && (root.discriminants, root.downcasts) == (0, 0)
                    && root.switch_values.contains(&17)
                    && root.switch_values.contains(&93)
            }
            _ => false,
        })
}

#[test]
fn original_match_import_report_requires_both_roots_and_distinct_rustc_semantics() {
    let mut report = ImportReport {
        roots: std::array::from_fn(|_| ImportedRoot {
            enum_tags: vec![vec![u128::from((-7_i32) as u32), 23, 91]],
            constructors: [1; 3],
            discriminants: 1,
            downcasts: 2,
            switch_values: vec![u128::from((-7_i32) as u32), 23],
        }),
        semantic_bytes: 1,
        later_outcome: "later source admission is not this report's claim".into(),
    };
    assert!(complete_import("signed_constructors", &report));
    assert!(!complete_import("constant_lookalikes", &report));
    for root in 0..2 {
        report.roots[root].constructors[1] = 0;
        assert!(!complete_import("signed_constructors", &report));
        report.roots[root].constructors[1] = 1;
        report.roots[root].enum_tags[0][0] = 0;
        assert!(!complete_import("signed_constructors", &report));
        report.roots[root].enum_tags[0][0] = u128::from((-7_i32) as u32);
    }
    report.roots = std::array::from_fn(|_| ImportedRoot {
        switch_values: vec![17, 93],
        ..ImportedRoot::default()
    });
    assert!(complete_import("constant_lookalikes", &report));
    assert!(!complete_import("signed_constructors", &report));
    report.roots[1].discriminants = 1;
    assert!(!complete_import("constant_lookalikes", &report));
}

fn observe_import(source: &AdmittedInertSemanticMirV1) -> Result<ImportReport, String> {
    if source.roots().len() != 2 {
        return Err("expected both original registered roots".into());
    }
    let mut result = ImportReport {
        roots: std::array::from_fn(|_| ImportedRoot::default()),
        semantic_bytes: source.canonical_encoding().len(),
        later_outcome: String::new(),
    };
    for (root, function) in source.roots().iter().copied().enumerate() {
        let function = &source.functions()[function.index() as usize];
        let report = &mut result.roots[root];
        for local in function.locals() {
            if let SemanticTypeShapeV1::Enum { variants, .. } =
                source.types()[local.ty().index() as usize].shape()
            {
                let tags: Vec<_> = variants.iter().map(|row| row.discriminant()).collect();
                if !report.enum_tags.contains(&tags) {
                    report.enum_tags.push(tags);
                }
            }
        }
        for block in function.blocks() {
            for statement in block.statements() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                match assignment.value().kind() {
                    SemanticRvalueKindV1::Aggregate(aggregate) => {
                        if let SemanticAggregateKindV1::EnumVariant(variant) = aggregate.kind() {
                            let slot = report
                                .constructors
                                .get_mut(*variant as usize)
                                .ok_or("unexpected original enum variant")?;
                            *slot += 1;
                            let SemanticTypeShapeV1::Enum { variants, .. } = source.types()
                                [assignment.value().result_type().index() as usize]
                                .shape()
                            else {
                                return Err("original enum constructor has a non-enum type".into());
                            };
                            let fields = variants[*variant as usize].fields().fields();
                            if fields.len() != aggregate.operands().len()
                                || !fields
                                    .iter()
                                    .zip(aggregate.operands())
                                    .all(|(ty, operand)| *ty == operand.ty())
                            {
                                return Err("original enum constructor field types differ".into());
                            }
                        }
                    }
                    SemanticRvalueKindV1::Discriminant(place) => {
                        if !matches!(
                            source.types()[place.ty().index() as usize].shape(),
                            SemanticTypeShapeV1::Enum { .. }
                        ) {
                            return Err("nominal tag was fabricated from a non-enum".into());
                        }
                        report.discriminants += 1;
                    }
                    _ => {}
                }
                assignment.value().kind().try_visit_operands(|operand| {
                    if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) = operand
                    {
                        report.downcasts += place
                            .projections()
                            .iter()
                            .filter(|projection| {
                                matches!(projection.kind(), SemanticProjectionKindV1::Downcast(_))
                            })
                            .count();
                    }
                    Ok::<_, String>(())
                })?;
            }
            if let SemanticTerminatorKindV1::SwitchInt { targets, .. } = block.terminator().kind() {
                report
                    .switch_values
                    .extend(targets.values().iter().map(|target| target.value()));
            }
        }
    }
    Ok(result)
}

#[derive(Default)]
struct ImportCallbacks {
    result: Option<Result<ImportReport, String>>,
}

impl Callbacks for ImportCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let observed = std::rc::Rc::new(std::cell::RefCell::new((0usize, None)));
            let callback = std::rc::Rc::clone(&observed);
            let outcome = crate::collector::semantic_import_observation_v1_tests::with_observer(
                Box::new(move |_, source| {
                    let mut state = callback.borrow_mut();
                    state.0 += 1;
                    state.1 = Some(observe_import(source));
                }),
                || transaction.with_original_source_mixed_publication_v28::<(), _>(|_, _| Ok(())),
            );
            let (count, report) = std::rc::Rc::try_unwrap(observed)
                .expect("original observer was removed")
                .into_inner();
            if count != 1 {
                return Err(format!(
                    "expected one mandatory original import, got {count}: {:?}",
                    outcome.err()
                ));
            }
            let mut report = report.ok_or("missing original import report")??;
            // Later admission is recorded, never counted as source/runtime success.
            report.later_outcome = match outcome {
                Ok(_) => "original publication callback reached".into(),
                Err(error) => format!("{error:?}"),
            };
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "private actual-rustc child; exact source requests supplied by its parent"]
fn enum_frontend_import_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callback = ImportCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callback);
    let result = callback
        .result
        .expect("original compiler callback did not run");
    std::fs::write(
        env::var_os(RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "original enum frontend import: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src/rustc-dev and authentic AMD dependencies"]
fn actual_constructor_patterns_and_constant_lookalikes_keep_rustc_semantics() {
    let module = ORIGINAL_CHILD.rsplit_once("::").unwrap().0;
    let child = format!("{module}::enum_frontend_tests::enum_frontend_import_child");
    run_actual_sources::<ImportReport>(
        &[
            ("signed_constructors", SIGNED),
            ("constant_lookalikes", CONSTANTS),
        ],
        &[(0, 0)],
        &child,
        "RUSTC_RESOLVED_MATCH_V49",
        original_program,
        |_, _, case, report, _| {
            assert!(complete_import(case, &report), "{case}: {report:?}");
        },
    );
}
