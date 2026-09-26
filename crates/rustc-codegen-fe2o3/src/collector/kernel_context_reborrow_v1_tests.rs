//! Real-rustc-type component checks, not production context or transport admission.

use super::*;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{FakeBorrowKind, MutBorrowKind, Place};

const SOURCE: &str = r#"
#![allow(dead_code)]
pub fn scalar(value: &u32) -> &u32 { std::hint::black_box(&*value) }
pub fn slice_u32(value: &[u32]) -> &[u32] { std::hint::black_box(&*value) }
pub fn slice_f32(value: &[f32]) -> &[f32] { std::hint::black_box(&*value) }
pub fn array_seven(value: &[u32; 7]) -> &[u32; 7] { std::hint::black_box(&*value) }
pub fn array_eight(value: &[u32; 8]) -> &[u32; 8] { std::hint::black_box(&*value) }
pub fn pair<'a>(left: &'a [u32], right: &'a [u32]) -> (&'a [u32], &'a [u32]) {
    (std::hint::black_box(&*left), std::hint::black_box(&*right))
}
pub fn mutable(value: &mut [u32]) -> &mut [u32] { std::hint::black_box(value) }
pub fn raw_const(value: *const [u32]) -> *const [u32] { value }
pub fn raw_mut(value: *mut [u32]) -> *mut [u32] { value }
pub fn plain(value: u32) -> u32 { value }
pub fn static_slice(value: &'static [u32]) -> &'static [u32] {
    std::hint::black_box(&*value)
}
"#;

fn definition(tcx: TyCtxt<'_>, name: &str) -> rustc_hir::def_id::DefId {
    tcx.iter_local_def_id()
        .find(|definition| {
            tcx.def_kind(definition.to_def_id()) == DefKind::Fn
                && tcx.item_name(definition.to_def_id()).as_str() == name
        })
        .unwrap_or_else(|| panic!("missing component fixture {name}"))
        .to_def_id()
}

fn body<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> &'tcx Body<'tcx> {
    tcx.instance_mir(Instance::mono(tcx, definition(tcx, name)).def)
}

fn input_type<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Ty<'tcx> {
    let body = body(tcx, name);
    assert_eq!(body.arg_count, 1, "{name}");
    body.local_decls[Local::from_usize(1)].ty
}

fn temporary<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, ty: Ty<'tcx>) -> Local {
    body.local_decls
        .iter_enumerated()
        .find(|(local, declaration)| {
            local.index() > body.arg_count
                && tcx.erase_and_anonymize_regions(declaration.ty)
                    == tcx.erase_and_anonymize_regions(ty)
        })
        .map(|(local, _)| local)
        .expect("ordinary Rust forwarding has a real reference temporary")
}

fn dereference<'tcx>(tcx: TyCtxt<'tcx>, local: Local) -> Place<'tcx> {
    Place {
        local,
        projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
    }
}

fn forward<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    values: &mut Values,
    source: Local,
    destination: Local,
) -> Result<Origin, CollectError> {
    reborrow_operand(
        tcx,
        body,
        values,
        dereference(tcx, source),
        Place::from(destination),
        BorrowKind::Shared,
    )
}

fn reject<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    values: &mut Values,
    source: Local,
    destination: Local,
    label: &str,
) {
    let before = values.origins.clone();
    let phase = values.phase;
    assert!(
        forward(tcx, body, values, source, destination).is_err(),
        "{label}"
    );
    assert_eq!(values.origins, before, "{label}: rejected read is inert");
    assert_eq!(values.phase, phase, "{label}: rejected read keeps phase");
}

fn actual_type_shapes(tcx: TyCtxt<'_>) {
    for (name, expected) in [
        ("scalar", tcx.types.u32),
        ("slice_u32", tcx.types.u32),
        ("slice_f32", tcx.types.f32),
        ("array_seven", tcx.types.u32),
        ("array_eight", tcx.types.u32),
    ] {
        let ty = input_type(tcx, name);
        let TyKind::Ref(_, referent, rustc_hir::Mutability::Not) = ty.kind() else {
            panic!("{name}: actual shared reference required");
        };
        match name {
            "scalar" => assert_eq!(*referent, expected),
            "slice_u32" | "slice_f32" => {
                assert!(matches!(referent.kind(), TyKind::Slice(element) if *element == expected));
            }
            _ => {
                assert!(
                    matches!(referent.kind(), TyKind::Array(element, _) if *element == expected)
                );
            }
        }
        assert!(shared_reference_type(tcx, ty).is_some());
    }
    assert_ne!(
        input_type(tcx, "array_seven"),
        input_type(tcx, "array_eight")
    );
    for name in ["mutable", "raw_const", "raw_mut", "plain"] {
        assert!(
            shared_reference_type(tcx, input_type(tcx, name)).is_none(),
            "{name}"
        );
    }
    assert!(matches!(
        input_type(tcx, "mutable").kind(),
        TyKind::Ref(_, _, rustc_hir::Mutability::Mut)
    ));
    assert!(matches!(
        input_type(tcx, "raw_const").kind(),
        TyKind::RawPtr(_, rustc_hir::Mutability::Not)
    ));
    assert!(matches!(
        input_type(tcx, "raw_mut").kind(),
        TyKind::RawPtr(_, rustc_hir::Mutability::Mut)
    ));
    assert_eq!(input_type(tcx, "plain"), tcx.types.u32);
}

fn input_and_temporary_forwarding(tcx: TyCtxt<'_>) {
    for name in [
        "scalar",
        "slice_u32",
        "slice_f32",
        "array_seven",
        "array_eight",
    ] {
        let body = body(tcx, name);
        let before = format!("{body:?}");
        let input = Local::from_usize(1);
        let destination = temporary(tcx, body, body.local_decls[input].ty);
        let mut values = Values::new(body.local_decls.len(), body.arg_count).unwrap();
        let origin = forward(tcx, body, &mut values, input, destination).unwrap();
        assert_eq!(origin, Origin::Argument(0), "{name}: exact physical input");
        values.assign(destination.index(), origin).unwrap();
        let through_temporary =
            forward(tcx, body, &mut values, destination, Local::from_usize(0)).unwrap();
        assert_eq!(
            through_temporary, origin,
            "{name}: original ordinal survives temporary"
        );
        assert_eq!(values.read(input.index(), false).unwrap(), origin);
        assert_eq!(values.read(destination.index(), false).unwrap(), origin);
        assert_eq!(
            format!("{body:?}"),
            before,
            "{name}: supplied MIR remains unchanged"
        );

        // The adapter computes an origin; the existing assignment guard owns writes.
        let same_input = forward(tcx, body, &mut values, input, input).unwrap();
        assert!(values.assign(input.index(), same_input).is_err());
    }
}

fn live_origin_and_order(tcx: TyCtxt<'_>) {
    let body = body(tcx, "slice_u32");
    let input = Local::from_usize(1);
    let temp = temporary(tcx, body, body.local_decls[input].ty);
    let output = Local::from_usize(0);
    for origin in [
        None,
        Some(Origin::Context),
        Some(Origin::Result),
        Some(Origin::Unit),
    ] {
        let mut values = Values::new(body.local_decls.len(), body.arg_count).unwrap();
        values.origins[temp.index()] = origin;
        reject(
            tcx,
            body,
            &mut values,
            temp,
            output,
            "non-argument source origin",
        );
    }
    let mut values = Values::new(body.local_decls.len(), body.arg_count).unwrap();
    let origin = forward(tcx, body, &mut values, input, temp).unwrap();
    values.assign(temp.index(), origin).unwrap();
    assert_eq!(values.read(temp.index(), true).unwrap(), origin);
    reject(
        tcx,
        body,
        &mut values,
        temp,
        output,
        "moved temporary is dead",
    );
    values.read(input.index(), true).unwrap();
    reject(
        tcx,
        body,
        &mut values,
        input,
        temp,
        "moved physical input is dead",
    );
    values
        .assign(temp.index(), Origin::Argument(body.arg_count))
        .unwrap();
    reject(
        tcx,
        body,
        &mut values,
        temp,
        output,
        "out-of-range original ordinal",
    );

    let pair = body_for_pair(tcx);
    let mut values = Values::new(pair.local_decls.len(), pair.arg_count).unwrap();
    let temp = temporary(tcx, pair, pair.local_decls[Local::from_usize(1)].ty);
    let left = forward(tcx, pair, &mut values, Local::from_usize(1), temp).unwrap();
    let right = forward(tcx, pair, &mut values, Local::from_usize(2), temp).unwrap();
    assert_eq!((left, right), (Origin::Argument(0), Origin::Argument(1)));
    // Only the component state machine is exercised; no issuer is authenticated.
    values.phase = Phase::Issued;
    for operands in [
        [Origin::Context, right, left],
        [Origin::Context, left, left],
        [Origin::Context, right, right],
    ] {
        assert!(
            values.call(&operands, 0).is_err(),
            "same-type origin substitution"
        );
        assert_eq!(values.phase, Phase::Issued);
    }
    values.call(&[Origin::Context, left, right], 0).unwrap();
    assert_eq!(values.phase, Phase::Called);
    assert_eq!(values.origins[0], Some(Origin::Result));
}

fn body_for_pair<'tcx>(tcx: TyCtxt<'tcx>) -> &'tcx Body<'tcx> {
    let pair = body(tcx, "pair");
    assert_eq!(pair.arg_count, 2);
    assert_eq!(
        pair.local_decls[Local::from_usize(1)].ty,
        pair.local_decls[Local::from_usize(2)].ty
    );
    pair
}

fn exact_original_types(tcx: TyCtxt<'_>) {
    let body = body(tcx, "slice_u32");
    let input = Local::from_usize(1);
    let source = temporary(tcx, body, body.local_decls[input].ty);
    let destination = Local::from_usize(0);
    for name in [
        "scalar",
        "slice_f32",
        "array_seven",
        "array_eight",
        "mutable",
        "raw_const",
        "raw_mut",
        "plain",
    ] {
        let replacement = input_type(tcx, name);
        assert_ne!(replacement, body.local_decls[input].ty, "{name}");
        for changed_local in [source, destination, input] {
            let mut changed = body.clone();
            changed.local_decls[changed_local].ty = replacement;
            let mut values = Values::new(changed.local_decls.len(), changed.arg_count).unwrap();
            values.assign(source.index(), Origin::Argument(0)).unwrap();
            reject(
                tcx,
                &changed,
                &mut values,
                source,
                destination,
                "one type substituted",
            );
        }
    }
    for name in ["mutable", "raw_const", "raw_mut", "plain"] {
        let mut changed = body.clone();
        for local in [source, destination, input] {
            changed.local_decls[local].ty = input_type(tcx, name);
        }
        let mut values = Values::new(changed.local_decls.len(), changed.arg_count).unwrap();
        values.assign(source.index(), Origin::Argument(0)).unwrap();
        reject(
            tcx,
            &changed,
            &mut values,
            source,
            destination,
            "equal non-shared types",
        );
    }

    let array = self::body(tcx, "array_seven");
    let array_temp = temporary(tcx, array, array.local_decls[input].ty);
    for changed_local in [array_temp, destination, input] {
        let mut changed = array.clone();
        changed.local_decls[changed_local].ty = input_type(tcx, "array_eight");
        let mut values = Values::new(changed.local_decls.len(), changed.arg_count).unwrap();
        values
            .assign(array_temp.index(), Origin::Argument(0))
            .unwrap();
        reject(
            tcx,
            &changed,
            &mut values,
            array_temp,
            destination,
            "array extent substituted",
        );
    }

    let signature = tcx
        .fn_sig(definition(tcx, "static_slice"))
        .instantiate_identity()
        .skip_binder();
    let static_type = signature.inputs()[0];
    assert!(
        matches!(static_type.kind(), TyKind::Ref(region, _, _) if *region == tcx.lifetimes.re_static)
    );
    let erased = tcx.erase_and_anonymize_regions(static_type);
    assert_ne!(
        static_type, erased,
        "region-only variation is genuinely exercised"
    );
    let mut changed = body.clone();
    changed.local_decls[input].ty = static_type;
    changed.local_decls[source].ty = static_type;
    changed.local_decls[destination].ty = erased;
    let mut values = Values::new(changed.local_decls.len(), changed.arg_count).unwrap();
    values.assign(source.index(), Origin::Argument(0)).unwrap();
    assert_eq!(
        forward(tcx, &changed, &mut values, source, destination).unwrap(),
        Origin::Argument(0)
    );
}

fn exact_places_and_borrow_kinds(tcx: TyCtxt<'_>) {
    let body = body(tcx, "slice_u32");
    let input = Local::from_usize(1);
    let destination = temporary(tcx, body, body.local_decls[input].ty);
    for projection in [
        vec![],
        vec![ProjectionElem::Deref, ProjectionElem::Deref],
        vec![ProjectionElem::Deref, ProjectionElem::Index(input)],
        vec![
            ProjectionElem::Deref,
            ProjectionElem::Subslice {
                from: 0,
                to: 1,
                from_end: true,
            },
        ],
        vec![
            ProjectionElem::Deref,
            ProjectionElem::ConstantIndex {
                offset: 0,
                min_length: 1,
                from_end: false,
            },
        ],
    ] {
        let mut values = Values::new(body.local_decls.len(), body.arg_count).unwrap();
        let before = values.origins.clone();
        let source = Place {
            local: input,
            projection: tcx.mk_place_elems(&projection),
        };
        assert!(
            reborrow_operand(
                tcx,
                body,
                &mut values,
                source,
                Place::from(destination),
                BorrowKind::Shared
            )
            .is_err()
        );
        assert_eq!(values.origins, before);
    }
    for borrow in [
        BorrowKind::Fake(FakeBorrowKind::Shallow),
        BorrowKind::Fake(FakeBorrowKind::Deep),
        BorrowKind::Mut {
            kind: MutBorrowKind::Default,
        },
        BorrowKind::Mut {
            kind: MutBorrowKind::TwoPhaseBorrow,
        },
        BorrowKind::Mut {
            kind: MutBorrowKind::ClosureCapture,
        },
    ] {
        let mut values = Values::new(body.local_decls.len(), body.arg_count).unwrap();
        assert!(
            reborrow_operand(
                tcx,
                body,
                &mut values,
                dereference(tcx, input),
                Place::from(destination),
                borrow
            )
            .is_err()
        );
        assert_eq!(
            values.read(input.index(), false).unwrap(),
            Origin::Argument(0)
        );
    }
    let mut values = Values::new(body.local_decls.len(), body.arg_count).unwrap();
    assert!(
        reborrow_operand(
            tcx,
            body,
            &mut values,
            dereference(tcx, input),
            dereference(tcx, destination),
            BorrowKind::Shared
        )
        .is_err()
    );
}

#[derive(Default)]
struct ReborrowComponentCallbacks {
    completed: bool,
}

impl Callbacks for ReborrowComponentCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        actual_type_shapes(tcx);
        input_and_temporary_forwarding(tcx);
        live_origin_and_order(tcx);
        exact_original_types(tcx);
        exact_places_and_borrow_kinds(tcx);
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
fn context_reborrow_component_checks_real_rustc_types_origins_and_places() {
    let directory = TestTempDir::create("fe2o3-context-reborrow-component");
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
        "--crate-name".into(),
        "fe2o3_context_reborrow_component".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zmir-opt-level=0".into(),
        "-Cpanic=abort".into(),
        "--sysroot".into(),
        sysroot.trim().into(),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = ReborrowComponentCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}
