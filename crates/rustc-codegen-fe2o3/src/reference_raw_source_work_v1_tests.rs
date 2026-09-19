use super::*;
use crate::rustc_semantic_plan_v1::SourceClosureWorkV1;
use fe2o3_mir_model::semantic_mir_v1::HARD_MAX_VALIDATION_WORK_V1;
use rustc_span::DUMMY_SP;

fn exact_and_one_short(
    operation: impl Fn(&ReferenceExtractionWorkV1<'_>) -> Result<(), ReferenceBindingErrorV1>,
) -> u64 {
    let mut measured = SourceClosureWorkV1::default();
    operation(&ReferenceExtractionWorkV1::borrowed(&mut measured)).unwrap();
    let cost = measured.validation_work_for_test();
    assert!(cost > 0 && cost < HARD_MAX_VALIDATION_WORK_V1);

    let mut exact = SourceClosureWorkV1::default();
    exact
        .charge((HARD_MAX_VALIDATION_WORK_V1 - cost) as usize)
        .unwrap();
    operation(&ReferenceExtractionWorkV1::borrowed(&mut exact)).unwrap();
    assert_eq!(
        exact.validation_work_for_test(),
        HARD_MAX_VALIDATION_WORK_V1
    );

    let mut short = SourceClosureWorkV1::default();
    let floor = HARD_MAX_VALIDATION_WORK_V1 - cost + 1;
    short.charge(floor as usize).unwrap();
    assert!(operation(&ReferenceExtractionWorkV1::borrowed(&mut short)).is_err());
    assert!(short.validation_work_for_test() >= floor);
    cost
}

fn empty_block(rules: hir::BlockCheckMode) -> hir::Block<'static> {
    hir::Block {
        stmts: &[],
        expr: None,
        hir_id: hir::HirId::INVALID,
        rules,
        span: DUMMY_SP,
        targeted_by_break: false,
    }
}

fn hir_visitor<'a, 'w>(meter: &'a ReferenceExtractionWorkV1<'w>) -> SafeHirVisitor<'a, 'w> {
    SafeHirVisitor {
        work: RawWork::new(meter, 0).unwrap(),
        first_unsafe: None,
    }
}

fn mir_visitor<'a, 'w>(meter: &'a ReferenceExtractionWorkV1<'w>) -> RawMirVisitor<'a, 'w> {
    RawMirVisitor {
        work: RawWork::new(meter, 1).unwrap(),
        error: None,
    }
}

#[test]
fn inspection_is_not_an_accounting_bypass() {
    let error = RawWork::new(&ReferenceExtractionWorkV1::Inspection, 0)
        .err()
        .unwrap();
    assert!(error.to_string().contains("inherited source work ledger"));
}

#[test]
fn logical_visit_classification_is_not_layout_size_accounting() {
    let mut work = SourceClosureWorkV1::default();
    work.charge(17).unwrap();
    {
        let meter = ReferenceExtractionWorkV1::borrowed(&mut work);
        RawWork::new(&meter, 0).unwrap().records(3).unwrap();
        RawWork::new(&meter, 1).unwrap().records(3).unwrap();
        RawWork::new(&meter, 3).unwrap().records(3).unwrap();
    }
    assert_eq!(work.validation_work_for_test(), 17 + 3 + 6 + 12);
}

#[test]
fn raw_record_debits_have_exact_and_one_short_boundaries() {
    assert_eq!(
        exact_and_one_short(|meter| RawWork::new(meter, 3)?.records(9)),
        36
    );
}

#[test]
fn checked_record_overflow_keeps_inherited_work() {
    let mut work = SourceClosureWorkV1::default();
    work.charge(7).unwrap();
    {
        let meter = ReferenceExtractionWorkV1::borrowed(&mut work);
        assert!(
            RawWork::new(&meter, 1)
                .unwrap()
                .records(usize::MAX)
                .is_err()
        );
        assert!(
            RawWork::new(&meter, usize::MAX)
                .unwrap()
                .records(1)
                .is_err()
        );
    }
    assert_eq!(work.validation_work_for_test(), 7);
}

#[test]
fn recursive_visit_depth_is_bounded_without_new_ledger() {
    let mut work = SourceClosureWorkV1::default();
    let meter = ReferenceExtractionWorkV1::borrowed(&mut work);
    let mut raw = RawWork::new(&meter, 0).unwrap();
    for _ in 0..MAX_RAW_SOURCE_DEPTH_V1 {
        finish(raw.enter()).unwrap();
    }
    let error = finish(raw.enter()).unwrap_err();
    assert!(error.to_string().contains("256 recursive visit levels"));
    assert_eq!(raw.depth, MAX_RAW_SOURCE_DEPTH_V1);
}

#[test]
fn safe_hir_body_uses_the_inherited_exact_work_boundary() {
    let expression = hir::Expr {
        hir_id: hir::HirId::INVALID,
        kind: hir::ExprKind::Tup(&[]),
        span: DUMMY_SP,
    };
    let body = hir::Body {
        params: &[],
        value: &expression,
    };
    exact_and_one_short(|meter| {
        let mut visitor = hir_visitor(meter);
        finish(visitor.visit_body(&body))?;
        assert!(visitor.first_unsafe.is_none());
        assert_eq!(visitor.work.depth, 0);
        Ok(())
    });
}

#[test]
fn user_unsafe_is_retained_but_compiler_unsafe_is_not() {
    let user = empty_block(hir::BlockCheckMode::UnsafeBlock(
        hir::UnsafeSource::UserProvided,
    ));
    let compiler = empty_block(hir::BlockCheckMode::UnsafeBlock(
        hir::UnsafeSource::CompilerGenerated,
    ));
    let mut work = SourceClosureWorkV1::default();
    let meter = ReferenceExtractionWorkV1::borrowed(&mut work);
    let mut visitor = hir_visitor(&meter);
    finish(visitor.visit_block(&compiler)).unwrap();
    assert!(visitor.first_unsafe.is_none());
    finish(visitor.visit_block(&user)).unwrap();
    assert_eq!(visitor.first_unsafe, Some(DUMMY_SP));
    assert_eq!(visitor.work.depth, 0);
}

#[test]
fn exhausted_hir_work_is_an_error_not_a_clean_unsafe_scan() {
    let mut work = SourceClosureWorkV1::default();
    work.charge(HARD_MAX_VALIDATION_WORK_V1 as usize).unwrap();
    let meter = ReferenceExtractionWorkV1::borrowed(&mut work);
    let mut visitor = hir_visitor(&meter);
    assert!(finish(visitor.visit_block(&empty_block(hir::BlockCheckMode::DefaultBlock))).is_err());
    assert_eq!(visitor.work.depth, 0);
}

#[test]
fn mir_statement_lists_are_prepaid_with_exact_budget() {
    let source_info = mir::SourceInfo::outermost(DUMMY_SP);
    let data = mir::BasicBlockData::new_stmts(
        (0..12)
            .map(|_| mir::Statement::new(source_info, mir::StatementKind::Nop))
            .collect(),
        Some(mir::Terminator {
            source_info,
            kind: mir::TerminatorKind::Return,
        }),
        false,
    );
    exact_and_one_short(|meter| {
        let mut visitor = mir_visitor(meter);
        visitor.visit_basic_block_data(mir::START_BLOCK, &data);
        visitor.finish()
    });
}

fn switch_work(
    case_count: usize,
    meter: &ReferenceExtractionWorkV1<'_>,
) -> Result<(), ReferenceBindingErrorV1> {
    let source_info = mir::SourceInfo::outermost(DUMMY_SP);
    let value = mir::Terminator {
        source_info,
        kind: mir::TerminatorKind::SwitchInt {
            discr: mir::Operand::Copy(mir::Place::from(mir::RETURN_PLACE)),
            targets: mir::SwitchTargets::new(
                (0..case_count).map(|i| (i as u128, mir::START_BLOCK)),
                mir::START_BLOCK,
            ),
        },
    };
    let mut visitor = mir_visitor(meter);
    visitor.visit_terminator(&value, Location::START);
    visitor.finish()
}

#[test]
fn duplicate_switch_targets_still_pay_for_every_case_and_default() {
    let few = exact_and_one_short(|meter| switch_work(2, meter));
    let many = exact_and_one_short(|meter| switch_work(19, meter));
    assert_eq!(many - few, 2 * (19 - 2));
}

#[test]
fn mir_visitor_latches_resource_failure_before_later_refusal() {
    let mut work = SourceClosureWorkV1::default();
    work.charge(HARD_MAX_VALIDATION_WORK_V1 as usize).unwrap();
    let meter = ReferenceExtractionWorkV1::borrowed(&mut work);
    let mut visitor = mir_visitor(&meter);
    assert!(!visitor.records(1));
    let first = visitor.error.as_ref().unwrap().to_string();
    visitor.refuse("unrelated unsupported source");
    assert_eq!(visitor.finish().unwrap_err().to_string(), first);
}

fn assert_interned_pattern_refused(value: ty::Ty<'_>) {
    let expected = "raw reference source pattern types are outside the metered census";
    // One census plus one prepaid hash visit; no base or pattern child visit.
    for floor in [17, HARD_MAX_VALIDATION_WORK_V1 - 2] {
        let mut work = SourceClosureWorkV1::default();
        work.charge(floor as usize).unwrap();
        {
            let meter = ReferenceExtractionWorkV1::borrowed(&mut work);
            let mut visitor = RawTypeVisitor {
                work: RawWork::new(&meter, 1).unwrap(),
            };
            assert_eq!(
                finish(value.visit_with(&mut visitor))
                    .unwrap_err()
                    .to_string(),
                expected
            );
            assert_eq!(visitor.work.depth, 0);
        }
        assert_eq!(work.validation_work_for_test(), floor + 2);
    }

    let mut short = SourceClosureWorkV1::default();
    short
        .charge((HARD_MAX_VALIDATION_WORK_V1 - 1) as usize)
        .unwrap();
    let meter = ReferenceExtractionWorkV1::borrowed(&mut short);
    let mut visitor = RawTypeVisitor {
        work: RawWork::new(&meter, 1).unwrap(),
    };
    let error = finish(value.visit_with(&mut visitor)).unwrap_err();
    assert_ne!(
        error.to_string(),
        expected,
        "resource failure must precede refusal"
    );
    assert_eq!(visitor.work.depth, 0);
}

#[test]
fn interned_pattern_types_are_refused_before_unmetered_pattern_recursion() {
    use crate::test_temp_dir::TestTempDir;
    use rustc_driver::{Callbacks, Compilation};
    use rustc_interface::interface::Compiler;

    struct PatternCallbacks(bool);

    impl Callbacks for PatternCallbacks {
        fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            let base = ty::Ty::new_imm_ptr(tcx, tcx.types.u32);
            assert_eq!(
                exact_and_one_short(|meter| {
                    let mut visitor = RawTypeVisitor {
                        work: RawWork::new(meter, 1)?,
                    };
                    finish(base.visit_with(&mut visitor))?;
                    assert_eq!(visitor.work.depth, 0);
                    Ok(())
                }),
                4
            );

            // These are real interned types, not source syntax or forged rows.
            let leaf = tcx.mk_pat(ty::PatternKind::NotNull);
            let pair = tcx.mk_pat(ty::PatternKind::Or(tcx.mk_patterns(&[leaf, leaf])));
            let wide = tcx.mk_pat(ty::PatternKind::Or(tcx.mk_patterns(&[leaf; 16])));
            let mut nested = pair;
            for _ in 0..8 {
                nested = tcx.mk_pat(ty::PatternKind::Or(tcx.mk_patterns(&[nested, leaf])));
            }
            for pattern in [leaf, pair, wide, nested] {
                let value = ty::Ty::new_pat(tcx, base, pattern);
                assert!(matches!(value.kind(), ty::Pat(_, _)));
                assert_interned_pattern_refused(value);
            }

            let definition = tcx
                .hir_body_owners()
                .find(|id| tcx.item_name(id.to_def_id()).as_str() == "while_fixture")
                .expect("missing while fixture");
            let instance = Instance::mono(tcx, definition.to_def_id());
            let body = tcx.instance_mir(instance.def);
            let mut work = SourceClosureWorkV1::default();
            work.charge(17).unwrap();
            let meter = ReferenceExtractionWorkV1::borrowed(&mut work);
            authenticate_safe_local_reference_v1(&meter, tcx, instance).unwrap();
            charge_reference_source_v1(tcx, instance, &meter).unwrap();
            let mut never_locals = 0;
            for (local, declaration) in body.local_decls.iter_enumerated() {
                if matches!(declaration.ty.kind(), ty::Never) {
                    never_locals += 1;
                    let error =
                        super::super::lower_place_v1(&meter, tcx, body, mir::Place::from(local), 0)
                            .unwrap_err();
                    assert!(error.to_string().contains("uninhabited local"), "{error}");
                }
            }
            assert!(
                never_locals > 0,
                "opt0 while fixture must retain Never locals"
            );
            self.0 = true;
            Compilation::Stop
        }
    }

    let directory = TestTempDir::create("fe2o3-raw-reference-pattern");
    let source = directory.path().join("fixture.rs");
    std::fs::write(
        &source,
        r#"
pub fn while_fixture(seed: u32) {
    let mut value = seed;
    let mut index = 0_u32;
    while index < 3 {
        value ^= index;
        index += 1;
    }
    let _value = value;
}
"#,
    )
    .unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let sysroot = String::from_utf8(output.stdout).unwrap();
    let args = vec![
        "rustc".into(),
        "--crate-name=fe2o3_raw_reference_pattern".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zmir-opt-level=0".into(),
        "-Copt-level=0".into(),
        "-Coverflow-checks=off".into(),
        "-Cpanic=abort".into(),
        "--sysroot".into(),
        sysroot.trim().into(),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = PatternCallbacks(false);
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.0, "raw pattern callback did not run");
}
