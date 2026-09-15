use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

const SOURCE: &str = r#"
pub fn probe(point: usize, input: &[f32], output: &mut [f32]) {
    if input.len() == 8 && output.len() == 8 && point < 8 {
        output[point] = input[point];
    }
}
"#;

struct Probe {
    completed: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("output_slice_metadata.rs".into()),
            input: SOURCE.into(),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let owner = tcx
            .hir_body_owners()
            .find(|id| tcx.item_name(id.to_def_id()).as_str() == "probe")
            .unwrap();
        let instance = Instance::mono(tcx, owner.to_def_id());
        let original = tcx.instance_mir(instance.def);
        let relations = vec![
            ReferenceArgumentRelationV1::PointCoordinate {
                reference_argument: 0,
                axis: 0,
            },
            ReferenceArgumentRelationV1::SharedSliceInput {
                argument: 0,
                element: ReferenceScalarTypeV1::F32,
            },
            ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D {
                argument: 1,
                element: ReferenceScalarTypeV1::F32,
            },
        ];
        let plan = collect(
            tcx,
            original,
            &relations,
            &mut ReferenceSymbolicWorkBudgetV2::default(),
        )
        .unwrap();
        assert_eq!(
            plan.slots.len(),
            2,
            "retain shared reborrow and fake raw metadata carrier"
        );
        assert_eq!(plan.lengths.len(), 2);
        assert!(plan.lengths.values().all(|argument| *argument == 2));
        let (local, slot) = plan.slots.iter().next().unwrap();
        for mutation in 0..7 {
            let mut body = original.clone();
            match mutation {
                0 => {
                    let StatementKind::Assign(a) = &mut body.basic_blocks.as_mut()
                        [slot.consumption.block]
                        .statements[slot.consumption.statement_index]
                        .kind
                    else {
                        unreachable!()
                    };
                    let Rvalue::UnaryOp(_, op @ Operand::Move(_)) = &mut a.1 else {
                        unreachable!()
                    };
                    let Operand::Move(p) = *op else {
                        unreachable!()
                    };
                    *op = Operand::Copy(p);
                }
                1 => {
                    let mut extra = body.basic_blocks[slot.consumption.block].statements
                        [slot.consumption.statement_index]
                        .clone();
                    let StatementKind::Assign(a) = &mut extra.kind else {
                        unreachable!()
                    };
                    a.1 = Rvalue::Use(Operand::Copy(Place::from(*local)));
                    body.basic_blocks.as_mut()[slot.consumption.block]
                        .statements
                        .push(extra);
                }
                2 => {
                    let StatementKind::Assign(a) = &body.basic_blocks[slot.consumption.block]
                        .statements[slot.consumption.statement_index]
                        .kind
                    else {
                        unreachable!()
                    };
                    let destination = a.0.local;
                    body.local_decls[destination].ty = tcx.types.isize;
                }
                3 => {
                    let repeated = body.basic_blocks[slot.definition.block].statements
                        [slot.definition.statement_index]
                        .clone();
                    body.basic_blocks.as_mut()[slot.definition.block]
                        .statements
                        .push(repeated);
                }
                4 => {
                    let mut dead = body.basic_blocks[slot.definition.block].statements
                        [slot.definition.statement_index]
                        .clone();
                    dead.kind = StatementKind::StorageDead(*local);
                    body.basic_blocks.as_mut()[slot.definition.block]
                        .statements
                        .insert(slot.consumption.statement_index, dead);
                }
                5 => {
                    let StatementKind::Assign(a) = &mut body.basic_blocks.as_mut()
                        [slot.definition.block]
                        .statements[slot.definition.statement_index]
                        .kind
                    else {
                        unreachable!()
                    };
                    a.0 = Place::from(Local::from_usize(3));
                }
                6 => body.local_decls[*local].ty = tcx.types.usize,
                _ => unreachable!(),
            }
            assert!(
                matches!(collect(tcx, &body, &relations, &mut ReferenceSymbolicWorkBudgetV2::default()),
                Err(error) if error.to_string().contains("adjacent sole-use source borrow")),
                "mutation {mutation}"
            );
        }
        let mut wrong_type = relations.clone();
        wrong_type[2] = ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D {
            argument: 1,
            element: ReferenceScalarTypeV1::U32,
        };
        assert!(
            matches!(collect(tcx, original, &wrong_type, &mut ReferenceSymbolicWorkBudgetV2::default()),
            Err(error) if error.to_string().contains("adjacent sole-use source borrow"))
        );
        let mut work = ReferenceSymbolicWorkBudgetV2::default();
        work.charge_v2(MAX_REFERENCE_SYMBOLIC_WORK_NODES_V2)
            .unwrap();
        assert!(
            matches!(collect(tcx, original, &relations, &mut work), Err(error) if error.to_string().contains("cumulative expression work"))
        );
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
fn output_slice_metadata_original_mir_and_mutations() {
    const CHILD: &str = "FE2O3_OUTPUT_SLICE_METADATA_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "reference_effect_v1::output_slice_metadata_v1::tests::output_slice_metadata_original_mir_and_mutations", "--nocapture", "--test-threads=1"])
            .env(CHILD, "1").output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        return;
    }
    let sysroot = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=output_slice_metadata".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zalways-encode-mir".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-".into(),
    ];
    let mut probe = Probe { completed: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.completed);
}
