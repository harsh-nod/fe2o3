//! Component layout/policy regressions; genuine SDK positives remain managed source tests.

use super::*;
use crate::test_temp_dir::TestTempDir;
use fe2o3_mir_model::semantic_mir_v1::{SemanticMirLimitsV1, SemanticMirResourceV1};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;

const SOURCE: &str = r#"
#![forbid(unsafe_code)]
#![allow(dead_code)]
use core::marker::PhantomData;
#[repr(C)]
struct DisjointSlice<T> { ptr: *mut T, len: usize, marker: PhantomData<fn() -> ()> }
#[repr(C)]
struct WriteOnlyDisjointSlice<T> { ptr: *mut T, len: usize, marker: PhantomData<fn() -> ()> }
struct Siblings<T> { view: DisjointSlice<T>, unrelated: *mut T }
fn shape_u32(value: DisjointSlice<u32>) { core::hint::black_box(value); }
fn shape_f32(value: DisjointSlice<f32>) { core::hint::black_box(value); }
fn write_shape(value: WriteOnlyDisjointSlice<u32>) { core::hint::black_box(value); }
fn raw_elements(value: DisjointSlice<*mut u32>) { core::hint::black_box(value); }
fn siblings(value: Siblings<u32>) { core::hint::black_box(value); }
fn borrowed(value: &mut DisjointSlice<u32>) { core::hint::black_box(value); }
fn scalar_reference(value: &u32) { core::hint::black_box(value); }
fn raw_capture(ptr: *mut u32) -> usize {
    let closure = move || ptr as usize;
    closure()
}
fn lookalike_capture(view: DisjointSlice<u32>) -> usize {
    let closure = move || { let moved = view; core::hint::black_box(moved).len };
    closure()
}
fn nested_closure(seed: u32) -> u32 {
    let inner = move || seed;
    let view = DisjointSlice { ptr: core::ptr::null_mut(), len: 0, marker: PhantomData };
    fn select<T>(view: DisjointSlice<T>, _: &T) -> DisjointSlice<T> { view }
    let view = select(view, &inner);
    let outer = move || { let moved = view; core::hint::black_box(moved).len as u32 };
    outer()
}
"#;

fn definition(tcx: TyCtxt<'_>, name: &str) -> rustc_hir::def_id::DefId {
    tcx.iter_local_def_id()
        .find(|definition| {
            tcx.def_kind(definition.to_def_id()) == DefKind::Fn
                && tcx.item_name(definition.to_def_id()).as_str() == name
        })
        .unwrap_or_else(|| panic!("missing capture component fixture {name}"))
        .to_def_id()
}

fn input_type<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Ty<'tcx> {
    let body = tcx.instance_mir(Instance::mono(tcx, definition(tcx, name)).def);
    assert_eq!(body.arg_count, 1);
    body.local_decls[Local::from_usize(1)].ty
}

fn observe<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> (TypeLayoutFacts, SdkCaptureObservationV1) {
    let facts = extract_capture_layout(tcx, ty).expect("real rustc physical capture layout");
    let observation =
        SdkCaptureObservationV1::observe(tcx, 2, ty, &facts, &mut SourceClosureWorkV1::default())
            .unwrap();
    (facts, observation)
}

fn validate(
    observation: &SdkCaptureObservationV1,
    facts: &TypeLayoutFacts,
    origin: ClosureOriginV1,
) -> Result<(), ClosureProfileErrorV1> {
    observation.validate(
        2,
        observation.capture_type,
        facts,
        origin,
        &mut SourceClosureWorkV1::default(),
    )
}

// These internal records exercise the validator as a data model. They are never
// returned by observe(), used to admit a closure, or presented as SDK authority.
fn model(
    identity: SemanticTypeIdentityV1,
    facts: &TypeLayoutFacts,
    pointer_path: Vec<Step>,
    provider: TrustedDeviceItem,
) -> SdkCaptureObservationV1 {
    SdkCaptureObservationV1 {
        source_index: 2,
        capture_type: identity,
        physical: Some(Box::new(facts.clone())),
        pointers: vec![ViewPointer {
            path: pointer_path,
            view_type: identity,
            provider,
        }],
    }
}

fn admission_and_model_negatives(tcx: TyCtxt<'_>) {
    for name in [
        "shape_u32",
        "shape_f32",
        "write_shape",
        "raw_elements",
        "siblings",
        "borrowed",
    ] {
        let ty = input_type(tcx, name);
        let (facts, observation) = observe(tcx, ty);
        assert!(
            observation.pointers.is_empty(),
            "{name}: nominal lookalike must not mint observation"
        );
        assert!(observation.physical.is_none());
        assert!(
            validate(&observation, &facts, ClosureOriginV1::DeviceInternal).is_err(),
            "{name}"
        );
        assert!(
            validate(&observation, &facts, ClosureOriginV1::HostArgument).is_err(),
            "{name}: host origin"
        );
    }
    for name in ["raw_capture", "lookalike_capture", "nested_closure"] {
        let instance = Instance::mono(tcx, definition(tcx, name));
        assert!(
            crate::closure_profile_v1::observe_closures_v2(tcx, instance).is_err(),
            "{name}"
        );
    }
    let (scalar, plain) = observe(tcx, input_type(tcx, "scalar_reference"));
    assert!(validate(&plain, &scalar, ClosureOriginV1::DeviceInternal).is_ok());
    assert!(validate(&plain, &scalar, ClosureOriginV1::HostArgument).is_err());

    let ty = input_type(tcx, "shape_u32");
    let identity = rustc_type_identity_v1(tcx, ty);
    let (facts, _) = observe(tcx, ty);
    // Matching physical layout alone never authenticates this source lookalike.
    check_view_layout(&facts).unwrap();
    let TypeLayoutKind::Adt(adt) = &facts.kind else {
        panic!("view ADT");
    };
    for provider in [
        TrustedDeviceItem::DisjointSlice,
        TrustedDeviceItem::WriteOnlyDisjointSlice,
    ] {
        let observation = model(identity, &facts, vec![Step::Field(0, 0)], provider);
        // This is validation of the internal observation data model, not a
        // successful trusted-provider observation of the source lookalike.
        let mut measured = SourceClosureWorkV1::default();
        observation
            .validate(
                2,
                identity,
                &facts,
                ClosureOriginV1::DeviceInternal,
                &mut measured,
            )
            .unwrap();
        let required = measured.validation_work_for_test();
        let maximum = SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork);
        for remaining in [required - 1, required] {
            let mut bounded = SourceClosureWorkV1::default();
            bounded
                .charge(usize::try_from(maximum - remaining).unwrap())
                .unwrap();
            let result = observation.validate(
                2,
                identity,
                &facts,
                ClosureOriginV1::DeviceInternal,
                &mut bounded,
            );
            assert_eq!(result.is_ok(), remaining == required);
        }
        assert!(validate(&observation, &facts, ClosureOriginV1::HostArgument).is_err());
        assert!(
            observation
                .validate(
                    3,
                    identity,
                    &facts,
                    ClosureOriginV1::DeviceInternal,
                    &mut SourceClosureWorkV1::default()
                )
                .is_err()
        );
        let other = rustc_type_identity_v1(tcx, input_type(tcx, "shape_f32"));
        assert_ne!(other, identity);
        assert!(
            observation
                .validate(
                    2,
                    other,
                    &facts,
                    ClosureOriginV1::DeviceInternal,
                    &mut SourceClosureWorkV1::default()
                )
                .is_err()
        );
        let mut changed = facts.clone();
        changed.abi_alignment_bytes *= 2;
        assert!(validate(&observation, &changed, ClosureOriginV1::DeviceInternal).is_err());
        for path in [
            vec![],
            vec![Step::Field(0, 1)],
            vec![Step::Field(1, 0)],
            vec![Step::Field(0, 0), Step::Pointee],
        ] {
            let mut changed = observation.clone();
            changed.pointers[0].path = path;
            assert!(
                validate(&changed, &facts, ClosureOriginV1::DeviceInternal).is_err(),
                "wrong field path"
            );
        }
        let mut duplicated = observation.clone();
        duplicated.pointers.push(duplicated.pointers[0].clone());
        assert!(validate(&duplicated, &facts, ClosureOriginV1::DeviceInternal).is_err());
        let mut unused = observation.clone();
        let mut extra = unused.pointers[0].clone();
        extra.path.push(Step::Field(99, 99));
        unused.pointers.push(extra);
        assert!(validate(&unused, &facts, ClosureOriginV1::DeviceInternal).is_err());
        let mut changed = observation.clone();
        changed.physical = None;
        assert!(validate(&changed, &facts, ClosureOriginV1::DeviceInternal).is_err());

        for axis in 0..7 {
            let mut changed = facts.clone();
            let TypeLayoutKind::Adt(adt) = &mut changed.kind else {
                unreachable!();
            };
            match axis {
                0 => adt.variants[0].fields[0].offset_bytes = 8,
                1 => adt.variants[0].fields[1].offset_bytes = 0,
                2 => adt.variants[0].fields[0].source_index = 1,
                3 => adt.variants[0].fields[0].memory_index = 1,
                4 => adt.variants[0].fields[2].layout.size_bytes = 8,
                5 => {
                    let TypeLayoutKind::Pointer(pointer) =
                        &mut adt.variants[0].fields[0].layout.kind
                    else {
                        unreachable!();
                    };
                    pointer.kind = PointerKind::ConstRaw;
                }
                6 => {
                    let TypeLayoutKind::Pointer(pointer) =
                        &mut adt.variants[0].fields[0].layout.kind
                    else {
                        unreachable!();
                    };
                    pointer.address_space = 3;
                }
                _ => unreachable!(),
            }
            assert!(
                check_view_layout(&changed).is_err(),
                "physical contract axis {axis}"
            );
            assert!(validate(&observation, &changed, ClosureOriginV1::DeviceInternal).is_err());
        }
        let mut exhausted = SourceClosureWorkV1::default();
        let maximum = SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork);
        exhausted
            .charge(usize::try_from(maximum - 1).unwrap())
            .unwrap();
        assert!(
            observation
                .validate(
                    2,
                    identity,
                    &facts,
                    ClosureOriginV1::DeviceInternal,
                    &mut exhausted
                )
                .is_err()
        );
    }
    // Even an observed carrier pointer cannot excuse a raw-pointer element.
    let raw_ty = input_type(tcx, "raw_elements");
    let (raw_facts, _) = observe(tcx, raw_ty);
    let raw_model = model(
        rustc_type_identity_v1(tcx, raw_ty),
        &raw_facts,
        vec![Step::Field(0, 0)],
        TrustedDeviceItem::DisjointSlice,
    );
    assert!(validate(&raw_model, &raw_facts, ClosureOriginV1::DeviceInternal).is_err());

    let nested = tcx.instance_mir(Instance::mono(tcx, definition(tcx, "nested_closure")).def);
    let nested_ty = nested
        .local_decls
        .iter()
        .find_map(|local| match local.ty.kind() {
            TyKind::Adt(_, arguments)
                if arguments
                    .first()
                    .and_then(|argument| argument.as_type())
                    .is_some_and(|element| matches!(element.kind(), TyKind::Closure(..))) =>
            {
                Some(local.ty)
            }
            _ => None,
        })
        .expect("actual view whose element is a closure");
    let (nested_facts, _) = observe(tcx, nested_ty);
    let nested_model = model(
        rustc_type_identity_v1(tcx, nested_ty),
        &nested_facts,
        vec![Step::Field(0, 0)],
        TrustedDeviceItem::DisjointSlice,
    );
    assert!(
        validate(
            &nested_model,
            &nested_facts,
            ClosureOriginV1::DeviceInternal
        )
        .unwrap_err()
        .to_string()
        .contains("nested closure captures")
    );

    let borrowed_ty = input_type(tcx, "borrowed");
    let (borrowed, _) = observe(tcx, borrowed_ty);
    let borrowed_model = model(
        rustc_type_identity_v1(tcx, borrowed_ty),
        &borrowed,
        vec![Step::Pointee, Step::Field(0, 0)],
        TrustedDeviceItem::DisjointSlice,
    );
    let mut changed = borrowed.clone();
    let TypeLayoutKind::Pointer(pointer) = &mut changed.kind else {
        panic!("reference");
    };
    pointer.kind = PointerKind::SharedReference;
    assert!(validate(&borrowed_model, &changed, ClosureOriginV1::DeviceInternal).is_err());
    assert!(validate(&borrowed_model, &borrowed, ClosureOriginV1::HostArgument).is_err());

    let sibling_ty = input_type(tcx, "siblings");
    let (siblings, _) = observe(tcx, sibling_ty);
    let sibling_model = model(
        rustc_type_identity_v1(tcx, sibling_ty),
        &siblings,
        vec![Step::Field(0, 0), Step::Field(0, 0)],
        TrustedDeviceItem::DisjointSlice,
    );
    assert!(
        validate(&sibling_model, &siblings, ClosureOriginV1::DeviceInternal).is_err(),
        "unrelated wrapper raw field"
    );
    assert_eq!(adt.variants[0].fields[0].source_index, 0);
}

#[derive(Default)]
struct CaptureCallbacks {
    completed: bool,
}

impl Callbacks for CaptureCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        admission_and_model_negatives(tcx);
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
fn sdk_capture_component_rejects_raw_lookalike_host_layout_type_path_and_work_substitution() {
    let directory = TestTempDir::create("fe2o3-sdk-allocation-capture");
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
        "fe2o3_sdk_capture_component".into(),
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
    let mut callbacks = CaptureCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}
