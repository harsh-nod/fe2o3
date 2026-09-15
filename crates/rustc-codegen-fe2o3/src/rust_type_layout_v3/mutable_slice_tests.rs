//! Compiler-backed mutable-slice admission and source/ABI substitution regressions.

use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use std::sync::OnceLock;

use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;

use crate::rust_type_layout_general::{
    BackendRepresentationFacts, PointeeLayoutFacts, PointerKind, ScalarPrimitiveFacts,
    SourceScalarKind, TypeLayoutFacts, TypeLayoutKind, extract_general_layout,
};
use crate::test_temp_dir::TestTempDir;

use super::*;

const SOURCE: &str = r#"
#![allow(dead_code, improper_ctypes_definitions)]

type MutableAlias<'a> = &'a mut [i32];

#[repr(transparent)]
struct Wrapped<'a>(&'a mut [i32]);

#[repr(C)]
struct DisjointSlice<T> {
    pointer: *mut T,
    length: usize,
}

fn shared_i32(_: &[i32]) {}
fn shared_f32(_: &[f32]) {}
fn mutable_i32(_: &mut [i32]) {}
fn mutable_f32(_: &mut [f32]) {}
fn mutable_alias(_: MutableAlias<'_>) {}
fn mutable_array(_: &mut [i32; 4]) {}
fn mutable_element(_: &mut i32) {}
fn raw_slice(_: *mut [i32]) {}
fn raw_element(_: *mut i32) {}
fn nested(_: &&mut [i32]) {}
fn wrapped(_: Wrapped<'_>) {}
fn lookalike(_: DisjointSlice<i32>) {}
unsafe fn unsafe_root(_: &[i32]) {}
extern "C" fn c_abi_root(_: &[i32]) {}
"#;

fn launch() -> LaunchContract {
    use fe2o3_artifacts::Dimensions;

    LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
        Dimensions::new(u32::MAX, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap()
}

struct Observation {
    facts: TypeLayoutFacts,
    admission: Result<GeneralTypedArgumentV3, String>,
}

#[derive(Default)]
struct SliceCallbacks {
    observations: BTreeMap<String, Observation>,
    root_errors: BTreeMap<String, String>,
}

impl Callbacks for SliceCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let layout_cx = LayoutCx::new(tcx, TypingEnv::fully_monomorphized());
        for local in tcx.iter_local_def_id() {
            let definition = local.to_def_id();
            if tcx.def_kind(definition) != DefKind::Fn {
                continue;
            }
            let name = tcx.item_name(definition).as_str().to_owned();
            let signature = tcx.instantiate_bound_regions_with_erased(
                tcx.fn_sig(definition).instantiate_identity(),
            );
            if name.ends_with("_root") {
                let error = extract_general_typed_kernel_v3(
                    tcx,
                    Instance::mono(tcx, definition),
                    &launch(),
                )
                .expect_err("substituted root signature must be rejected");
                self.root_errors.insert(name, error.to_string());
                continue;
            }
            let ty = signature.inputs()[0];
            // No fixture type is a trusted device ADT; the index-space parameter
            // is unused for the references and untrusted lookalikes under test.
            let admission = extract_argument(tcx, &layout_cx, ty, tcx.types.unit, 5)
                .map_err(|error| error.to_string());
            self.observations.insert(
                name,
                Observation {
                    facts: extract_general_layout(tcx, ty).expect("extract source layout"),
                    admission,
                },
            );
        }
        Compilation::Stop
    }
}

fn observations() -> &'static SliceCallbacks {
    static RESULTS: OnceLock<SliceCallbacks> = OnceLock::new();
    RESULTS.get_or_init(|| {
        let directory = TestTempDir::create("fe2o3-mutable-slice-layout");
        let source = directory.path().join("fixture.rs");
        fs::write(&source, SOURCE).expect("write compiler fixture");
        let mut command = Command::new("rustc");
        command.args(["--print", "sysroot"]);
        let sysroot =
            crate::process_execution::capture_output(&mut command).expect("query compiler sysroot");
        assert!(sysroot.status.success());
        let sysroot = String::from_utf8(sysroot.stdout).expect("UTF-8 sysroot");
        let args = vec![
            "rustc".to_owned(),
            "--crate-name=mutable_slice_layout_fixture".to_owned(),
            "--crate-type=lib".to_owned(),
            "--edition=2024".to_owned(),
            "--emit=metadata".to_owned(),
            "--sysroot".to_owned(),
            sysroot.trim().to_owned(),
            "-o".to_owned(),
            directory.path().join("fixture.rmeta").display().to_string(),
            source.display().to_string(),
        ];
        let mut callbacks = SliceCallbacks::default();
        rustc_driver::run_compiler(&args, &mut callbacks);
        assert_eq!(callbacks.observations.len(), 12);
        assert_eq!(callbacks.root_errors.len(), 2);
        callbacks
    })
}

#[test]
fn compiler_admits_mutable_slices_with_exact_ownership_element_and_extent() {
    for (name, element_kind) in [
        ("mutable_i32", SourceScalarKind::SignedInteger { bits: 32 }),
        (
            "mutable_alias",
            SourceScalarKind::SignedInteger { bits: 32 },
        ),
        ("mutable_f32", SourceScalarKind::Float { bits: 32 }),
    ] {
        let observation = &observations().observations[name];
        let facts = &observation.facts;
        assert_eq!((facts.size_bytes, facts.abi_alignment_bytes), (16, 8));
        let TypeLayoutKind::Pointer(pointer) = &facts.kind else {
            panic!("{name} lost source reference facts");
        };
        assert_eq!(pointer.kind, PointerKind::MutableReference);
        assert_eq!(pointer.address_space, 0);
        let PointeeLayoutFacts::Slice { element, .. } = &pointer.pointee else {
            panic!("{name} lost its dynamic slice extent");
        };
        assert_eq!(element.kind, TypeLayoutKind::Scalar(element_kind));
        assert_eq!((element.size_bytes, element.abi_alignment_bytes), (4, 4));
        let BackendRepresentationFacts::ScalarPair {
            first,
            second,
            second_offset_bytes,
        } = &facts.backend_representation
        else {
            panic!("{name} lost the pointer/length ABI");
        };
        assert_eq!(
            first.primitive,
            ScalarPrimitiveFacts::Pointer { address_space: 0 }
        );
        assert_eq!(
            second.primitive,
            ScalarPrimitiveFacts::Integer {
                bits: 64,
                signed: false
            }
        );
        assert_eq!(
            (first.size_bytes, second.size_bytes, *second_offset_bytes),
            (8, 8, 8)
        );
        let argument = observation.admission.as_ref().unwrap();
        let scalar = if name == "mutable_f32" {
            RustScalarElementTypeV1::F32
        } else {
            RustScalarElementTypeV1::I32
        };
        assert_eq!(
            argument.kind(),
            GeneralTypedArgumentKindV3::MutableSlice(scalar)
        );
        let layout = argument.layout().unwrap();
        assert_eq!(
            layout.rust_type().source_type(),
            RustSourceTypeShapeV1::mutable_slice(scalar)
        );
        let abi = build_abi(std::slice::from_ref(argument)).unwrap();
        let field = &abi.fields()[0];
        assert_eq!(field.ownership(), ArgumentOwnership::UniqueBorrow);
        assert_eq!(field.alias_class(), AliasClass::Exclusive);
        assert_eq!(field.access(), Access::ReadWrite);
        assert_eq!(field.mutability(), Mutability::Mutable);
        assert_eq!(field.address_space(), AddressSpace::Global);
        let disjoint = RustLayoutEvidenceV1::new(
            RustTypeEvidenceV1::new(RustSourceTypeShapeV1::disjoint_slice(
                scalar,
                RustDisjointIndexSpaceV1::Index1D,
            )),
            layout.abi_class(),
            layout.pointer_width(),
            layout.size(),
            layout.abi_alignment(),
            layout.components().to_vec(),
        )
        .unwrap();
        assert_ne!(layout.type_identity(), disjoint.type_identity());
    }
}

#[test]
fn compiler_admits_shared_slices_with_exact_read_only_contracts() {
    for (name, scalar) in [
        ("shared_i32", RustScalarElementTypeV1::I32),
        ("shared_f32", RustScalarElementTypeV1::F32),
    ] {
        let observation = &observations().observations[name];
        let argument = observation.admission.as_ref().unwrap();
        assert_eq!(
            argument.kind(),
            GeneralTypedArgumentKindV3::SharedSlice(scalar)
        );
        let layout = argument.layout().unwrap();
        assert_eq!(
            layout.rust_type().source_type(),
            RustSourceTypeShapeV1::shared_slice(scalar)
        );
        assert_eq!(layout.abi_class(), RustcAbiClassV1::ScalarPair);
        assert_eq!((layout.size(), layout.abi_alignment()), (16, 8));
        assert_eq!(
            layout.components()[0].kind(),
            RustPhysicalComponentKindV1::Pointer {
                mutability: RustPointerMutabilityV1::Const,
                pointee: scalar,
            }
        );
        assert_eq!(
            layout.components()[1].kind(),
            RustPhysicalComponentKindV1::Usize
        );
        let abi = build_abi(std::slice::from_ref(argument)).unwrap();
        let field = &abi.fields()[0];
        assert_eq!(field.ownership(), ArgumentOwnership::SharedBorrow);
        assert_eq!(field.alias_class(), AliasClass::SharedReadOnly);
        assert_eq!(field.mutability(), Mutability::Immutable);
        assert_eq!(field.access(), Access::ReadOnly);
        assert_eq!(field.address_space(), AddressSpace::Global);
    }
}

#[test]
fn compiler_rejects_pointer_array_nested_reference_and_wrapper_substitutions() {
    for name in [
        "mutable_array",
        "mutable_element",
        "raw_slice",
        "raw_element",
        "nested",
        "wrapped",
        "lookalike",
    ] {
        let observation = &observations().observations[name];
        assert!(
            observation.admission.is_err(),
            "unexpected admission of {name}"
        );
    }
    let results = observations();
    let mutable = &results.observations["mutable_i32"].facts;
    for name in [
        "shared_i32",
        "mutable_f32",
        "raw_slice",
        "wrapped",
        "lookalike",
    ] {
        let substitute = &results.observations[name].facts;
        assert_eq!(mutable.size_bytes, substitute.size_bytes);
        assert_eq!(mutable.abi_alignment_bytes, substitute.abi_alignment_bytes);
        assert_ne!(
            mutable.kind, substitute.kind,
            "ABI equality must not erase {name}"
        );
    }
}

#[test]
fn compiler_rejects_unsafe_and_non_rust_root_abi_substitutions() {
    let errors = &observations().root_errors;
    assert!(errors["unsafe_root"].contains("must be safe functions"));
    assert!(errors["c_abi_root"].contains("must use the non-variadic Rust ABI"));
}

#[test]
fn compiler_observed_slice_evidence_rejects_physical_abi_substitutions() {
    let argument = observations().observations["shared_i32"]
        .admission
        .as_ref()
        .unwrap();
    let layout = argument.layout().unwrap();
    let components = layout.components().to_vec();
    let swapped = vec![
        RustPhysicalComponentV1::new(0, 8, 8, RustPhysicalComponentKindV1::Usize).unwrap(),
        RustPhysicalComponentV1::new(8, 8, 8, components[0].kind()).unwrap(),
    ];
    let wrong_element = vec![
        RustPhysicalComponentV1::new(
            0,
            8,
            8,
            RustPhysicalComponentKindV1::Pointer {
                mutability: RustPointerMutabilityV1::Const,
                pointee: RustScalarElementTypeV1::F32,
            },
        )
        .unwrap(),
        components[1],
    ];
    let wrong_mutability = vec![
        RustPhysicalComponentV1::new(
            0,
            8,
            8,
            RustPhysicalComponentKindV1::Pointer {
                mutability: RustPointerMutabilityV1::Mut,
                pointee: RustScalarElementTypeV1::I32,
            },
        )
        .unwrap(),
        components[1],
    ];
    for (abi_class, width, size, alignment, components) in [
        (
            RustcAbiClassV1::Scalar,
            PointerWidth::Bits64,
            16,
            8,
            components.clone(),
        ),
        (
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits32,
            16,
            8,
            components.clone(),
        ),
        (
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            8,
            8,
            components.clone(),
        ),
        (
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            4,
            components.clone(),
        ),
        (
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            8,
            vec![components[0]],
        ),
        (
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            8,
            swapped,
        ),
        (
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            8,
            wrong_element,
        ),
        (
            RustcAbiClassV1::ScalarPair,
            PointerWidth::Bits64,
            16,
            8,
            wrong_mutability,
        ),
    ] {
        assert!(
            RustLayoutEvidenceV1::new(
                layout.rust_type(),
                abi_class,
                width,
                size,
                alignment,
                components,
            )
            .is_err()
        );
    }
}

#[test]
fn compiler_observed_contract_identity_binds_ownership_type_root_and_abi() {
    let results = observations();
    let argument = results.observations["mutable_i32"]
        .admission
        .as_ref()
        .unwrap();
    let abi = build_abi(std::slice::from_ref(argument)).unwrap();
    let identity = |binding, logical, export, abi: &AbiLayout| {
        fe2o3_artifacts::derive_generated_host_contract_identity_v1(
            reserved_fe2o3_symbols::MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
            binding,
            logical,
            export,
            abi,
            &launch(),
        )
    };
    let expected = identity([0x42; 32], "root", "entry", &abi);
    assert_eq!(expected, identity([0x42; 32], "root", "entry", &abi));
    for (binding, logical, export) in [
        ([0x43; 32], "root", "entry"),
        ([0x42; 32], "other", "entry"),
        ([0x42; 32], "root", "other"),
    ] {
        assert_ne!(expected, identity(binding, logical, export, &abi));
    }
    let different_element = results.observations["mutable_f32"]
        .admission
        .as_ref()
        .unwrap();
    let different_type = build_abi(std::slice::from_ref(different_element)).unwrap();
    assert_ne!(
        expected,
        identity([0x42; 32], "root", "entry", &different_type)
    );

    let field = &abi.fields()[0];
    for (offset, ownership, mutability, access, alias) in [
        (
            8,
            ArgumentOwnership::UniqueBorrow,
            Mutability::Mutable,
            Access::ReadWrite,
            AliasClass::Exclusive,
        ),
        (
            0,
            ArgumentOwnership::SharedBorrow,
            Mutability::Immutable,
            Access::ReadOnly,
            AliasClass::SharedReadOnly,
        ),
    ] {
        let substitution = AbiField::new(
            field.name().clone(),
            offset,
            field.size(),
            field.alignment(),
            field.kind(),
            mutability,
            access,
            field.address_space(),
            field.type_identity(),
            ownership,
            alias,
        )
        .unwrap();
        let substituted = AbiLayout::new(
            offset + abi.size(),
            abi.alignment(),
            abi.pointer_width(),
            vec![substitution],
        )
        .unwrap();
        assert_ne!(
            expected,
            identity([0x42; 32], "root", "entry", &substituted)
        );
    }
}
