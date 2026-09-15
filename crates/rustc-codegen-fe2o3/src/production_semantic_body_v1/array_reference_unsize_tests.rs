use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

const SOURCE: &str = r#"
#![no_std]
pub fn shared(a: &[u32; 4]) -> &[u32] { a }
pub fn mutable(a: &mut [u32; 4]) -> &mut [u32] { a }
pub fn empty(a: &[u32; 0]) -> &[u32] { a }
pub fn zst(a: &[(); 16]) -> &[()] { a }
pub fn empty_zst(a: &[(); 0]) -> &[()] { a }
pub fn different(a: &[f32; 4]) -> &[f32] { a }
pub fn raw(a: *const [u32; 4]) -> *const [u32] { a }
pub trait Marker {}
impl Marker for u32 {}
pub fn dynamic(a: &u32) -> &dyn Marker { a }
"#;

fn instance<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let def = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .expect("fixture function");
    Instance::mono(tcx, def.to_def_id())
}

fn unsizes<'a, 'tcx>(body: &'a Body<'tcx>) -> Vec<&'a Rvalue<'tcx>> {
    body.basic_blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter_map(|statement| {
            let StatementKind::Assign(assignment) = &statement.kind else {
                return None;
            };
            matches!(
                &assignment.1,
                Rvalue::Cast(
                    CastKind::PointerCoercion(
                        rustc_middle::ty::adjustment::PointerCoercion::Unsize,
                        _
                    ),
                    _,
                    _
                )
            )
            .then_some(&assignment.1)
        })
        .collect()
}

struct Probe(bool);
impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("array_reference_unsize.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let different = tcx.instance_mir(instance(tcx, "different").def).return_ty();
        let mutable = tcx.instance_mir(instance(tcx, "mutable").def).return_ty();
        let shared = tcx.instance_mir(instance(tcx, "shared").def).return_ty();
        for (name, length) in [
            ("shared", 4),
            ("mutable", 4),
            ("empty", 0),
            ("zst", 16),
            ("empty_zst", 0),
        ] {
            let instance = instance(tcx, name);
            let body = tcx.instance_mir(instance.def);
            let values = unsizes(body);
            assert_eq!(values.len(), 1, "one retained array unsizing: {name}");
            let value = values[0];
            let before = crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, instance);
            assert_eq!(
                array_reference_unsize_length_v1(tcx, instance, body, value),
                Some(length),
                "{name}"
            );
            let Rvalue::Cast(kind, operand, target) = value else {
                unreachable!()
            };
            for changed_target in [
                different,
                if name == "mutable" { shared } else { mutable },
                operand.ty(body, tcx),
            ] {
                assert_eq!(
                    array_reference_unsize_length_v1(
                        tcx,
                        instance,
                        body,
                        &Rvalue::Cast(*kind, operand.clone(), changed_target)
                    ),
                    None,
                    "{name}: {changed_target:?}"
                );
            }
            assert_eq!(
                array_reference_unsize_length_v1(
                    tcx,
                    instance,
                    body,
                    &Rvalue::Cast(CastKind::PtrToPtr, operand.clone(), *target)
                ),
                None
            );
            assert_eq!(
                before,
                crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, instance)
            );
        }
        for (name, count) in [("raw", 1), ("dynamic", 2)] {
            let instance = instance(tcx, name);
            let body = tcx.instance_mir(instance.def);
            let values = unsizes(body);
            assert_eq!(values.len(), count, "retained negative casts: {name}");
            for value in values {
                assert_eq!(
                    array_reference_unsize_length_v1(tcx, instance, body, value),
                    None,
                    "{name}"
                );
            }
        }
        self.0 = true;
        Compilation::Stop
    }
}

#[test]
fn array_reference_unsize_actual_rustc_types_preserve_only_exact_reference_metadata() {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=array_reference_unsize".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-opt-level=0".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-".into(),
    ];
    let mut probe = Probe(false);
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.0);
}
