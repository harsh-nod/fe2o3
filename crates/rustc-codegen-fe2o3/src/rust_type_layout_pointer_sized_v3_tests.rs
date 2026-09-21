use super::*;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use std::process::Command;

const SOURCE: &str = r#"
#![allow(dead_code, unused_variables)]
type Count = usize;
struct Pair { count: usize, offset: isize }
struct DropValue(usize);
impl Drop for DropValue { fn drop(&mut self) {} }
struct RawField { pointer: *const usize }
union Union { count: usize, offset: isize }
fn arguments(
    unsigned: usize, signed: isize, alias: Count,
    tuple: (usize, isize), array: [usize; 3], pair: Pair,
    fixed: u64, slice: &[u32], pointer_sized_slice: &[usize],
    reference: &usize, raw: *const usize, variant: Option<usize>,
    dropped: DropValue, oversized: [usize; 300], wide: u128,
    nested_wide: (u128,), nested_pointer: RawField, union: Union,
) {}
"#;

#[derive(Default)]
struct LayoutCallbacks {
    observed: bool,
}

fn observe_nominal_semantic_capture<'tcx>(tcx: TyCtxt<'tcx>, inputs: &[Ty<'tcx>]) {
    use crate::production_semantic_types_v1::{
        construct_production_semantic_types_nominal_v35, construct_production_semantic_types_v1,
    };
    use crate::rustc_semantic_adapter_v1::{
        canonical_target_layout_v1, rustc_semantic_layout_identity_v1, rustc_type_identity_v1,
        rustc_type_layout_sha256_v1,
    };
    use crate::rustc_semantic_plan_v1::RetainedSemanticTypeProducerV1;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticRustTypeKindV1 as Kind, SemanticScalarTypeV1, SemanticTypeShapeV1,
    };

    let target = crate::semantic_layout_bridge::rustc_semantic_layout_target_v1(tcx).unwrap();
    let canonical_target = canonical_target_layout_v1(&target);
    let layout_cx = LayoutCx::new(tcx, TypingEnv::fully_monomorphized());
    let producer = |ty| {
        let layout = layout_cx.layout_of(ty).unwrap();
        RetainedSemanticTypeProducerV1 {
            identity: rustc_type_identity_v1(tcx, ty),
            ty,
            layout,
            rustc_layout_sha256: rustc_type_layout_sha256_v1(tcx, layout),
            semantic_layout_identity: rustc_semantic_layout_identity_v1(
                tcx,
                canonical_target,
                layout,
            ),
        }
    };
    let mut producers = [inputs[0], inputs[1], inputs[6], tcx.types.i64].map(&producer);
    producers.sort_by_key(|producer| producer.identity);
    let legacy = construct_production_semantic_types_v1(tcx, &producers)
        .unwrap()
        .into_records();
    let nominal = construct_production_semantic_types_nominal_v35(tcx, &producers)
        .unwrap()
        .into_records();
    assert_eq!(legacy.len(), 4);
    assert_eq!(nominal.len(), legacy.len());
    for ((before, after), producer) in legacy.iter().zip(&nominal).zip(&producers) {
        let (expected, signed) = match producer.ty.kind() {
            TyKind::Uint(UintTy::Usize) => (Kind::Usize, false),
            TyKind::Int(IntTy::Isize) => (Kind::Isize, true),
            TyKind::Uint(UintTy::U64) => (Kind::Ordinary, false),
            TyKind::Int(IntTy::I64) => (Kind::Ordinary, true),
            _ => panic!("actual primitive roster changed"),
        };
        assert_eq!(before.rust_type_kind(), Kind::Ordinary);
        assert_eq!(after.rust_type_kind(), expected);
        assert_eq!(after.identity(), producer.identity);
        assert_eq!(after.layout_identity(), producer.semantic_layout_identity);
        assert_eq!(
            after.shape(),
            &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits: 64 })
        );
        assert_eq!(
            after.layout().size_bytes(),
            Some(producer.layout.size.bytes())
        );
        assert_eq!(
            after.layout().alignment_bytes(),
            producer.layout.align.abi.bytes()
        );
        assert_eq!(after.clone().with_rust_type_kind(Kind::Ordinary), *before);
    }
    let alias = construct_production_semantic_types_nominal_v35(tcx, &[producer(inputs[2])])
        .unwrap()
        .into_records();
    assert_eq!(
        alias.as_slice(),
        &[nominal
            .iter()
            .find(|ty| ty.rust_type_kind() == Kind::Usize)
            .unwrap()
            .clone()]
    );
    assert_eq!(
        construct_production_semantic_types_v1(tcx, &producers)
            .unwrap()
            .into_records(),
        legacy,
    );
}

impl Callbacks for LayoutCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert!(!self.observed);
        let definition = tcx
            .iter_local_def_id()
            .find(|id| {
                tcx.def_kind(id.to_def_id()) == DefKind::Fn
                    && tcx.item_name(id.to_def_id()).as_str() == "arguments"
            })
            .expect("actual source function");
        let signature = tcx
            .instantiate_bound_regions_with_erased(tcx.fn_sig(definition).instantiate_identity());
        let inputs = signature.inputs();
        assert_eq!(inputs.len(), 18);
        let layout_cx = LayoutCx::new(tcx, TypingEnv::fully_monomorphized());
        require_64_bit_target(tcx, &layout_cx).unwrap();
        for (index, ty) in inputs.iter().copied().take(6).enumerate() {
            let actual = extract_argument(tcx, &layout_cx, ty, tcx.types.unit, index).unwrap();
            let expected = layout_cx.layout_of(ty).unwrap();
            assert_eq!(
                actual.kind(),
                match index {
                    0 | 2 => GeneralTypedArgumentKindV3::CompilerLaidOutUsize,
                    1 => GeneralTypedArgumentKindV3::CompilerLaidOutIsize,
                    _ => GeneralTypedArgumentKindV3::CompilerLaidOutByValue,
                }
            );
            assert!(actual.kind().is_compiler_laid_out());
            assert_eq!(actual.size(), expected.size.bytes());
            assert_eq!(u64::from(actual.alignment()), expected.align.abi.bytes());
            assert!(actual.layout().is_none());
            assert!(actual.type_identity().is_none());
            if index < 3 {
                assert_eq!(actual.size(), 8);
                assert_eq!(actual.abi_class(), RustcAbiClassV1::Scalar);
                assert!(
                    scalar_type(ty).is_none(),
                    "never forge a fixed-width source shape"
                );
            }
        }
        let identity = |ty| crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(tcx, ty);
        assert_eq!(identity(inputs[0]), identity(inputs[2]));
        assert_ne!(identity(inputs[0]), identity(tcx.types.u64));
        assert_ne!(identity(inputs[1]), identity(tcx.types.i64));
        observe_nominal_semantic_capture(tcx, inputs);
        for (index, expected) in [
            (
                6,
                GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::U64),
            ),
            (
                7,
                GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::U32),
            ),
        ] {
            let actual =
                extract_argument(tcx, &layout_cx, inputs[index], tcx.types.unit, index).unwrap();
            assert_eq!(actual.kind(), expected);
            assert!(actual.layout().is_some());
            assert!(actual.type_identity().is_some());
        }
        for (index, diagnostic) in [
            (8, "unsupported shared-slice element type"),
            (9, "unsupported type"),
            (10, "unsupported type"),
            (11, "variant-aware packing evidence"),
            (12, "requires drop"),
            (13, "bounded component domain"),
            (14, "unsupported type"),
            (15, "unsupported field type"),
            (16, "contains a pointer or reference"),
            (17, "by-value union arguments are unsupported"),
        ] {
            let error = extract_argument(tcx, &layout_cx, inputs[index], tcx.types.unit, index)
                .expect_err("existing unsupported layout must still refuse");
            assert!(
                error.to_string().contains(diagnostic),
                "argument {index}: {error}"
            );
        }
        self.observed = true;
        Compilation::Stop
    }
}

#[test]
fn pointer_sized_by_value_arguments_keep_actual_rustc_layout_and_identity() {
    let directory = TestTempDir::create("fe2o3-pointer-sized-layout");
    let source = directory.path().join("fixture.rs");
    let output = directory.path().join("fixture.rmeta");
    std::fs::write(&source, SOURCE).unwrap();
    let mut command = Command::new("rustc");
    command.args(["--print", "sysroot"]);
    let sysroot = crate::process_execution::capture_output(&mut command).unwrap();
    assert!(sysroot.status.success());
    let sysroot = String::from_utf8(sysroot.stdout).unwrap();
    let args = vec![
        "rustc".to_owned(),
        "--crate-name=fe2o3_pointer_sized_layout_fixture".to_owned(),
        "--crate-type=lib".to_owned(),
        "--edition=2024".to_owned(),
        "--emit=metadata".to_owned(),
        "--sysroot".to_owned(),
        sysroot.trim().to_owned(),
        "-o".to_owned(),
        output.display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = LayoutCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.observed);
}
