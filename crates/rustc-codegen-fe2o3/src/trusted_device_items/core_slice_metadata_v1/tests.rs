use super::*;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{Local, Place, ProjectionElem, START_BLOCK, Statement};
use rustc_middle::ty::Ty;

const SOURCE: &str = r#"
#![no_std]
pub fn len(value: &[u32]) -> usize { value.len() }
pub fn generic<T>(value: &[T]) -> usize { value.len() }
pub fn local_unsafe(value: &[u32]) -> usize {
    unsafe { core::ptr::read(&value.len()) }
}
"#;

struct MetadataCallbacks {
    completed: bool,
}

impl Callbacks for MetadataCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let def_id = tcx
            .lang_items()
            .slice_len_fn()
            .expect("compiler slice-length identity");
        let core = tcx.lang_items().sized_trait().unwrap();
        assert_eq!(def_id.krate, core.krate);
        let body = tcx.instance_mir(InstanceKind::Item(def_id));
        let before = format!("{body:?}");
        let elements = [
            tcx.types.bool,
            tcx.types.u8,
            tcx.types.u32,
            tcx.types.u64,
            tcx.types.i32,
            tcx.types.f32,
            tcx.types.unit,
            Ty::new_tup(tcx, &[tcx.types.u64, tcx.types.bool]),
        ];
        for element in elements {
            let instance = Instance::new_raw(def_id, tcx.mk_args(&[element.into()]));
            assert!(authenticate_v1(tcx, instance), "generic element {element}");
            assert!(metadata_body_v1(tcx, instance, body));
        }
        let instance = Instance::new_raw(def_id, tcx.mk_args(&[tcx.types.u32.into()]));
        let reject = |label: &str, changed: &Body<'tcx>| {
            assert!(!metadata_body_v1(tcx, instance, changed), "{label}");
        };
        for local in tcx.hir_body_owners() {
            let name = tcx.item_name(local.to_def_id());
            let args = if name.as_str() == "generic" {
                tcx.mk_args(&[tcx.types.u32.into()])
            } else {
                tcx.mk_args(&[])
            };
            let foreign = Instance::new_raw(local.to_def_id(), args);
            assert!(!authenticate_v1(tcx, foreign), "local/shadow helper {name}");
            let mut changed = body.clone();
            changed.source.instance = foreign.def;
            reject("metadata-shaped body with a different owner", &changed);
        }
        assert!(!authenticate_v1(
            tcx,
            Instance::new_raw(def_id, tcx.mk_args(&[]))
        ));
        assert!(!authenticate_v1(
            tcx,
            Instance::new_raw(
                def_id,
                tcx.mk_args(&[tcx.types.u32.into(), tcx.types.u64.into(),])
            )
        ));
        assert!(!authenticate_v1(
            tcx,
            Instance {
                def: InstanceKind::Intrinsic(def_id),
                args: instance.args,
            }
        ));

        let mut changed = body.clone();
        changed.arg_count = 2;
        reject("extra argument", &changed);
        let mut changed = body.clone();
        changed
            .local_decls
            .push(changed.local_decls[Local::from_usize(1)].clone());
        reject("extra local", &changed);
        let mut changed = body.clone();
        changed.local_decls[Local::from_usize(0)].ty = tcx.types.u32;
        reject("non-usize result", &changed);
        for receiver in [
            Ty::new_ref(
                tcx,
                tcx.lifetimes.re_erased,
                Ty::new_slice(tcx, tcx.types.u32),
                Mutability::Mut,
            ),
            Ty::new_ref(
                tcx,
                tcx.lifetimes.re_erased,
                Ty::new_slice(tcx, tcx.types.u64),
                Mutability::Not,
            ),
            Ty::new_ptr(tcx, Ty::new_slice(tcx, tcx.types.u32), Mutability::Not),
        ] {
            let mut changed = body.clone();
            changed.local_decls[Local::from_usize(1)].ty = receiver;
            reject(
                "changed reference mutability, element, or raw-pointer input",
                &changed,
            );
        }
        let mut changed = body.clone();
        let extra = changed.basic_blocks[START_BLOCK].clone();
        changed.basic_blocks.as_mut().push(extra);
        reject("extra block, even if unreachable", &changed);
        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[START_BLOCK].is_cleanup = true;
        reject("cleanup body", &changed);
        let mut changed = body.clone();
        let info = changed.basic_blocks[START_BLOCK].statements[0].source_info;
        changed.basic_blocks.as_mut()[START_BLOCK]
            .statements
            .push(Statement::new(info, StatementKind::Nop));
        reject("additional statement", &changed);
        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[START_BLOCK]
            .terminator
            .as_mut()
            .unwrap()
            .kind = TerminatorKind::Unreachable;
        reject("different control flow", &changed);
        let mut changed = body.clone();
        changed.basic_blocks.as_mut()[START_BLOCK].terminator = None;
        reject("absent return", &changed);
        for value in [
            Rvalue::Use(Operand::Copy(Place::from(Local::from_usize(1)))),
            Rvalue::UnaryOp(UnOp::Not, Operand::Copy(Place::from(Local::from_usize(1)))),
            Rvalue::UnaryOp(
                UnOp::PtrMetadata,
                Operand::Copy(Place::from(Local::from_usize(0))),
            ),
            Rvalue::UnaryOp(
                UnOp::PtrMetadata,
                Operand::Copy(Place {
                    local: Local::from_usize(1),
                    projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
                }),
            ),
        ] {
            let mut changed = body.clone();
            let StatementKind::Assign(assignment) =
                &mut changed.basic_blocks.as_mut()[START_BLOCK].statements[0].kind
            else {
                panic!("metadata assignment");
            };
            assignment.1 = value;
            reject("wrong operation, source, or dereference", &changed);
        }
        let mut changed = body.clone();
        let StatementKind::Assign(assignment) =
            &mut changed.basic_blocks.as_mut()[START_BLOCK].statements[0].kind
        else {
            panic!("metadata assignment");
        };
        assignment.0 = Place::from(Local::from_usize(1));
        reject("write to input instead of result", &changed);
        assert_eq!(
            format!("{body:?}"),
            before,
            "actual rustc MIR remains unchanged"
        );
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
fn exact_core_slice_metadata_admission_checks_generic_instances_and_hostile_mir() {
    let directory = TestTempDir::create("fe2o3-core-slice-metadata");
    let source = directory.path().join("fixture.rs");
    std::fs::write(&source, SOURCE).unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let sysroot = String::from_utf8(output.stdout).unwrap();
    let args = vec![
        "rustc".into(),
        "--crate-name=fe2o3_core_slice_metadata_fixture".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zmir-opt-level=0".into(),
        "-Zinline-mir=no".into(),
        "-Cpanic=abort".into(),
        "--sysroot".into(),
        sysroot.trim().into(),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = MetadataCallbacks { completed: false };
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}
