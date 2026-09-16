//! Real reviewed-provider signatures; none of these descriptors grant execution authority.

use super::*;
use crate::collector::CollectedFunctionRole;
use crate::production_semantic_fn_abi_v1::construct_production_semantic_fn_abis_v1;
use crate::production_semantic_types_v1::construct_production_semantic_types_v1;
use crate::rustc_semantic_adapter_v1::*;
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, RetainedSemanticFunctionAbiProducerV1,
    RetainedSemanticFunctionProducerV1, build_production_semantic_preflight_plan_v1,
};
use crate::semantic_layout_bridge::rustc_semantic_layout_target_v1;
use crate::test_temp_dir::TestTempDir;
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::ty::{self, TypingEnv};
use std::process::Command;

const SOURCE: &str = r#"
use fe2o3_device::{KernelContext, WorkgroupCapability, MaskedTile1D, LaneFragment};
pub struct A;
pub struct B;
type Context = KernelContext<'static, A>;
type OtherContext = KernelContext<'static, B>;
type Brand = fe2o3_device::context::KernelCapabilityBrand<'static, A,
    fe2o3_device::CurrentTarget, fe2o3_device::RegisteredLaunch>;
type Group = WorkgroupCapability<'static, Brand>;
type Tile = MaskedTile1D<'static, u32, 64, 2, Brand>;
type Fragment = LaneFragment<'static, u32, 64, 2, Brand>;
type OtherBrand = fe2o3_device::context::KernelCapabilityBrand<'static, B,
    fe2o3_device::CurrentTarget, fe2o3_device::RegisteredLaunch>;
pub fn witnesses(c: Context, b: OtherContext, _: &mut Context, _: Group,
    _: core::marker::PhantomData<()>, _: &mut Group, _: &[u32], _: usize,
    _: WorkgroupCapability<'static, OtherBrand>, _: MaskedTile1D<'static, u32, 64, 2, OtherBrand>,
    _: LaneFragment<'static, u32, 64, 2, OtherBrand>) -> (Context, OtherContext) {
    (c, b)
}
pub fn issue() -> Context { Context::__compiler_issue() }
pub fn derive(_: &mut Context) -> Group { unreachable!() }
pub fn load(g: &Group, input: &[u32], base: usize) -> Tile { Tile::load_masked(g, input, base) }
pub fn fragment(tile: Tile) -> Fragment { tile.into_fragment() }
pub fn parts(fragment: Fragment) -> ([u32; 2], [bool; 2]) { fragment.into_parts() }
"#;

#[derive(Default)]
struct DescriptorCallbacks {
    completed: bool,
}

impl Callbacks for DescriptorCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let find = |name: &str| {
            Instance::mono(
                tcx,
                tcx.hir_body_owners()
                    .find(|id| {
                        tcx.def_kind(id.to_def_id()) == rustc_hir::def::DefKind::Fn
                            && tcx.item_name(id.to_def_id()).as_str() == name
                    })
                    .unwrap()
                    .to_def_id(),
            )
        };
        let signature = source_signature_v1(tcx, find("witnesses")).unwrap();
        let TyKind::Adt(context, context_args) = signature.inputs()[0].kind() else {
            panic!("context")
        };
        assert_eq!(
            trusted_device_items::classify(tcx, context.did()),
            Some(Item::KernelContext),
            "genuine provider authentication: {:?}",
            trusted_device_items::rejected_provider(tcx, context.did())
        );
        let mut functions = ["witnesses", "load", "fragment", "parts"].map(|name| {
            let instance = find(name);
            RetainedSemanticFunctionProducerV1 {
                identities: canonical_function_identities_v1(tcx, instance),
                instance,
                role: CollectedFunctionRole::InternalHelper,
                export_name: None,
                kernel_binding: None,
                generated_host_contract_identity: None,
                frontend_contract: None,
            }
        });
        functions.sort_by_key(|row| row.identities.function());
        let target = canonical_target_layout_v1(&rustc_semantic_layout_target_v1(tcx).unwrap());
        let plan = build_production_semantic_preflight_plan_v1(
            tcx,
            target,
            functions.to_vec().into_boxed_slice(),
            (0..functions.len())
                .map(|index| SemanticFunctionIdV1::from_index(index as u32))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            [7; 32],
            DebugSourceCaptureRequestV2::Disabled,
            None,
        )
        .unwrap();
        let types = construct_production_semantic_types_v1(tcx, plan.type_producers())
            .unwrap()
            .into_records();
        let id = |ty| {
            SemanticTypeIdV1::from_index(
                plan.type_producers()
                    .iter()
                    .position(|row| row.identity == rustc_type_identity_v1(tcx, ty))
                    .unwrap() as u32,
            )
        };
        let other_context = id(signature.inputs()[1]);
        let ordinary_zst = id(signature.inputs()[4]);
        let other_group = id(signature.inputs()[8]);
        let other_tile = id(signature.inputs()[9]);
        let other_fragment = id(signature.inputs()[10]);
        let mut terminals = plan
            .terminal_producers()
            .iter()
            .map(|terminal| (terminal.instance, terminal.expansion))
            .collect::<Vec<_>>();
        for (item, expansion) in [
            (Item::KernelContextIssue, Expansion::ContextIssue),
            (Item::ExecutionWorkgroupCurrent, Expansion::WorkgroupDerive),
        ] {
            let definition = trusted_device_items::definition(tcx, item).unwrap();
            terminals.push((Instance::new_raw(definition, context_args), expansion));
        }
        assert_eq!(terminals.len(), 5);
        for (instance, expansion) in terminals {
            let signature = source_signature_v1(tcx, instance).unwrap();
            let fn_abi = tcx
                .fn_abi_of_instance(
                    TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty())),
                )
                .unwrap();
            let producer = RetainedSemanticFunctionAbiProducerV1 {
                function: SemanticFunctionIdV1::from_index(0),
                identity: rustc_semantic_fn_abi_identity_v1(
                    tcx,
                    canonical_function_identities_v1(tcx, instance).function(),
                    fn_abi,
                ),
                layout_identity: rustc_semantic_fn_abi_layout_identity_v1(tcx, target, fn_abi),
                extern_abi: signature.abi,
                source_inputs: signature.inputs().to_vec().into_boxed_slice(),
                source_output: signature.output(),
                fn_abi,
                rustc_source_signature_sha256: rustc_fn_signature_sha256_v1(tcx, signature),
                rustc_fn_abi_sha256: rustc_fn_abi_sha256_v1(tcx, fn_abi),
            };
            let abi =
                construct_production_semantic_fn_abis_v1(tcx, &[producer], plan.type_producers())
                    .unwrap()
                    .into_records()
                    .pop()
                    .unwrap();
            let output_id = id(signature.output());
            let input_id = || id(signature.inputs()[0]);
            let pointee_id = || {
                let TyKind::Ref(_, pointee, _) = signature.inputs()[0].kind() else {
                    panic!("terminal receiver reference")
                };
                id(*pointee)
            };
            let expected = match expansion {
                Expansion::ContextIssue => Operation::ContextIssue { context: output_id },
                Expansion::WorkgroupDerive => Operation::WorkgroupDerive {
                    context: pointee_id(),
                    workgroup: output_id,
                },
                Expansion::MaskedTileLoadU32 => Operation::MaskedTileLoadU32 {
                    workgroup: pointee_id(),
                    tile: output_id,
                },
                Expansion::MaskedTileIntoFragmentU32 => Operation::MaskedTileIntoFragmentU32 {
                    tile: input_id(),
                    fragment: output_id,
                },
                Expansion::LaneFragmentIntoPartsU32 => Operation::LaneFragmentIntoPartsU32 {
                    fragment: input_id(),
                    parts: output_id,
                },
                _ => unreachable!(),
            };
            assert_eq!(
                construct(tcx, instance, expansion, &abi, &types),
                Some(expected)
            );
            let reject = |label, abi: &SemanticFunctionAbiV1, types: &[SemanticTypeDeclV1]| {
                assert!(
                    construct(tcx, instance, expansion, abi, types).is_none(),
                    "{expansion:?}: {label}"
                );
            };
            let lookalike = find(match expansion {
                Expansion::ContextIssue => "issue",
                Expansion::WorkgroupDerive => "derive",
                Expansion::MaskedTileLoadU32 => "load",
                Expansion::MaskedTileIntoFragmentU32 => "fragment",
                Expansion::LaneFragmentIntoPartsU32 => "parts",
                _ => unreachable!(),
            });
            assert_eq!(source_signature_v1(tcx, lookalike).unwrap(), signature);
            assert!(
                construct(tcx, lookalike, expansion, &abi, &types).is_none(),
                "same-signature untrusted method"
            );
            let output = abi.source_output_type();
            let output_record = &types[output.index() as usize];
            let mut changed = types.clone();
            changed[output.index() as usize] = output_record
                .clone()
                .with_rust_type_kind(SemanticRustTypeKindV1::Execution(Role::Workgroup));
            if !matches!(expansion, Expansion::WorkgroupDerive) {
                reject("wrong output role", &abi, &changed);
            }
            let adjusted = SemanticAbiValueV1::new_with_adjusted_type(
                output,
                SemanticAbiAdjustedTypeV1::new(
                    output,
                    output_record.layout_identity(),
                    output_record.layout().clone(),
                ),
                abi.return_value().mode().clone(),
            );
            reject(
                "adjusted result",
                &replace_abi(
                    &abi,
                    abi.source_input_types().to_vec(),
                    abi.arguments().to_vec(),
                    adjusted,
                    abi.extern_abi(),
                    abi.source_argument_ownership().to_vec(),
                ),
                &types,
            );
            reject(
                "foreign ABI",
                &replace_abi(
                    &abi,
                    abi.source_input_types().to_vec(),
                    abi.arguments().to_vec(),
                    abi.return_value().clone(),
                    SemanticExternAbiV1::Unadjusted,
                    abi.source_argument_ownership().to_vec(),
                ),
                &types,
            );
            reject(
                "return pointee override",
                &replace_abi(
                    &abi,
                    abi.source_input_types().to_vec(),
                    abi.arguments().to_vec(),
                    abi.return_value().clone().with_pointee_override(
                        SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
                    ),
                    abi.extern_abi(),
                    abi.source_argument_ownership().to_vec(),
                ),
                &types,
            );
            match expansion {
                Expansion::WorkgroupDerive | Expansion::MaskedTileLoadU32 => {
                    let foreign = if expansion == Expansion::WorkgroupDerive {
                        other_group
                    } else {
                        other_tile
                    };
                    reject(
                        "same-role foreign output brand",
                        &replace_abi(
                            &abi,
                            abi.source_input_types().to_vec(),
                            abi.arguments().to_vec(),
                            SemanticAbiValueV1::new(foreign, abi.return_value().mode().clone()),
                            abi.extern_abi(),
                            abi.source_argument_ownership().to_vec(),
                        ),
                        &types,
                    );
                }
                Expansion::MaskedTileIntoFragmentU32 | Expansion::LaneFragmentIntoPartsU32 => {
                    let foreign = if expansion == Expansion::MaskedTileIntoFragmentU32 {
                        other_tile
                    } else {
                        other_fragment
                    };
                    reject(
                        "same-role foreign input brand",
                        &replace_abi(
                            &abi,
                            vec![foreign],
                            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                                foreign,
                                abi.arguments()[0].mode().clone(),
                            ))],
                            abi.return_value().clone(),
                            abi.extern_abi(),
                            abi.source_argument_ownership().to_vec(),
                        ),
                        &types,
                    );
                }
                _ => {}
            }

            match expansion {
                Expansion::ContextIssue => {
                    reject(
                        "non-ignored context result",
                        &replace_abi(
                            &abi,
                            vec![],
                            vec![],
                            SemanticAbiValueV1::new(
                                output,
                                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
                            ),
                            abi.extern_abi(),
                            vec![],
                        ),
                        &types,
                    );
                    reject(
                        "extra source argument",
                        &replace_abi(
                            &abi,
                            vec![ordinary_zst],
                            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                                ordinary_zst,
                                SemanticAbiPassModeV1::Ignore,
                            ))],
                            abi.return_value().clone(),
                            abi.extern_abi(),
                            vec![Ownership::ByValue],
                        ),
                        &types,
                    );
                    for foreign in [other_context, ordinary_zst] {
                        reject(
                            "substituted output",
                            &replace_abi(
                                &abi,
                                vec![],
                                vec![],
                                SemanticAbiValueV1::new(foreign, SemanticAbiPassModeV1::Ignore),
                                abi.extern_abi(),
                                vec![],
                            ),
                            &types,
                        );
                    }
                    let mut args = abi.arguments().to_vec();
                    args.push(SemanticAbiArgumentV1::hidden(
                        SemanticAbiHiddenArgumentRoleV1::CallerLocation,
                        SemanticAbiValueV1::new(ordinary_zst, SemanticAbiPassModeV1::Ignore),
                    ));
                    reject(
                        "hidden argument",
                        &replace_abi(
                            &abi,
                            vec![],
                            args,
                            abi.return_value().clone(),
                            abi.extern_abi(),
                            vec![],
                        ),
                        &types,
                    );
                }
                Expansion::WorkgroupDerive | Expansion::MaskedTileLoadU32 => {
                    let input = abi.source_input_types()[0];
                    let record = &types[input.index() as usize];
                    let SemanticTypeShapeV1::Pointer(pointer) = record.shape() else {
                        panic!("reference")
                    };
                    for (pointee, kind, mutability) in [
                        (
                            if expansion == Expansion::WorkgroupDerive {
                                other_context
                            } else {
                                other_group
                            },
                            pointer.kind(),
                            pointer.mutability(),
                        ),
                        (
                            pointer.pointee(),
                            SemanticPointerKindV1::Raw,
                            pointer.mutability(),
                        ),
                        (
                            pointer.pointee(),
                            pointer.kind(),
                            if pointer.mutability() == SemanticMutabilityV1::Mutable {
                                SemanticMutabilityV1::Immutable
                            } else {
                                SemanticMutabilityV1::Mutable
                            },
                        ),
                    ] {
                        let mut changed = types.clone();
                        changed[input.index() as usize] = with_shape(
                            record,
                            SemanticTypeShapeV1::Pointer(
                                SemanticPointerTypeV1::new_with_kind(
                                    pointee,
                                    kind,
                                    mutability,
                                    pointer.address_space(),
                                    pointer.pointer_width_bits(),
                                    pointer.metadata(),
                                )
                                .unwrap(),
                            ),
                        );
                        reject("nested reference substitution", &abi, &changed);
                    }
                }
                Expansion::MaskedTileIntoFragmentU32 => {
                    let mut changed = types.clone();
                    changed[output.index() as usize] = output_record.clone().with_rust_type_kind(
                        SemanticRustTypeKindV1::Execution(Role::LaneFragmentU32 {
                            lanes: 32,
                            elements: 2,
                        }),
                    );
                    reject("wrong geometry", &abi, &changed);
                }
                Expansion::LaneFragmentIntoPartsU32 => {
                    let SemanticTypeShapeV1::Tuple(parts) = output_record.shape() else {
                        panic!("parts")
                    };
                    let mut changed = types.clone();
                    changed[output.index() as usize] = with_shape(
                        output_record,
                        SemanticTypeShapeV1::Tuple(
                            SemanticAggregateTypeV1::new(
                                parts.fields().iter().copied().rev().collect(),
                            )
                            .unwrap(),
                        ),
                    );
                    reject("swapped value/mask arrays", &abi, &changed);
                    for &array in parts.fields() {
                        let mut changed = types.clone();
                        let record = &types[array.index() as usize];
                        let SemanticTypeShapeV1::Array { element, length } = record.shape() else {
                            panic!("array")
                        };
                        changed[array.index() as usize] = with_shape(
                            record,
                            SemanticTypeShapeV1::Array {
                                element: *element,
                                length: length + 1,
                            },
                        );
                        reject("wrong array length", &abi, &changed);
                    }
                }
                _ => unreachable!(),
            }
            if !abi.source_input_types().is_empty() {
                let mut arguments = abi.arguments().to_vec();
                let input = abi.source_input_types()[0];
                let record = &types[input.index() as usize];
                arguments[0] =
                    SemanticAbiArgumentV1::source(SemanticAbiValueV1::new_with_adjusted_type(
                        input,
                        SemanticAbiAdjustedTypeV1::new(
                            input,
                            record.layout_identity(),
                            record.layout().clone(),
                        ),
                        abi.arguments()[0].mode().clone(),
                    ));
                reject(
                    "adjusted input",
                    &replace_abi(
                        &abi,
                        abi.source_input_types().to_vec(),
                        arguments,
                        abi.return_value().clone(),
                        abi.extern_abi(),
                        abi.source_argument_ownership().to_vec(),
                    ),
                    &types,
                );
                let mut ownership = abi.source_argument_ownership().to_vec();
                ownership[0] = Ownership::Unspecified;
                reject(
                    "unknown ownership",
                    &abi.clone()
                        .with_source_argument_ownership(ownership)
                        .unwrap(),
                    &types,
                );
                let mut ownership = abi.source_argument_ownership().to_vec();
                ownership[0] = if ownership[0] == Ownership::UniqueBorrow {
                    Ownership::SharedBorrow
                } else {
                    Ownership::UniqueBorrow
                };
                reject(
                    "wrong specified ownership",
                    &abi.clone()
                        .with_source_argument_ownership(ownership)
                        .unwrap(),
                    &types,
                );
                let mut arguments = abi.arguments().to_vec();
                arguments[0] = SemanticAbiArgumentV1::source(
                    abi.arguments()[0].value().clone().with_pointee_override(
                        SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
                    ),
                );
                reject(
                    "input pointee override",
                    &replace_abi(
                        &abi,
                        abi.source_input_types().to_vec(),
                        arguments,
                        abi.return_value().clone(),
                        abi.extern_abi(),
                        abi.source_argument_ownership().to_vec(),
                    ),
                    &types,
                );
            }
            if expansion == Expansion::MaskedTileLoadU32 {
                let base = abi.source_input_types()[2];
                let mut changed = types.clone();
                changed[base.index() as usize] = with_shape(
                    &types[base.index() as usize],
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        bits: 32,
                        signed: true,
                    }),
                );
                reject("wrong usize base shape", &abi, &changed);
                let SemanticTypeShapeV1::Pointer(pointer) =
                    types[abi.source_input_types()[1].index() as usize].shape()
                else {
                    panic!("slice reference")
                };
                let slice = pointer.pointee();
                let slice_reference = abi.source_input_types()[1];
                let reference_record = &types[slice_reference.index() as usize];
                for (mutability, metadata) in [
                    (SemanticMutabilityV1::Mutable, pointer.metadata()),
                    (pointer.mutability(), SemanticPointerMetadataV1::None),
                ] {
                    let mut changed = types.clone();
                    changed[slice_reference.index() as usize] = with_shape(
                        reference_record,
                        SemanticTypeShapeV1::Pointer(
                            SemanticPointerTypeV1::new_with_kind(
                                slice,
                                pointer.kind(),
                                mutability,
                                pointer.address_space(),
                                pointer.pointer_width_bits(),
                                metadata,
                            )
                            .unwrap(),
                        ),
                    );
                    reject("slice reference mutability/metadata", &abi, &changed);
                }
                for id in [slice_reference, slice] {
                    let mut changed = types.clone();
                    changed[id.index() as usize] = changed[id.index() as usize]
                        .clone()
                        .with_rust_type_kind(SemanticRustTypeKindV1::Execution(Role::Workgroup));
                    reject("nonordinary slice/reference", &abi, &changed);
                }
                let SemanticTypeShapeV1::Slice { element } = types[slice.index() as usize].shape()
                else {
                    panic!("slice")
                };
                let mut changed = types.clone();
                changed[element.index() as usize] = with_shape(
                    &types[element.index() as usize],
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
                );
                reject("wrong slice element shape", &abi, &changed);
            }
            assert_eq!(
                construct(tcx, instance, expansion, &abi, &types),
                Some(expected)
            );
        }
        self.completed = true;
        Compilation::Stop
    }
}

fn with_shape(record: &SemanticTypeDeclV1, shape: SemanticTypeShapeV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        record.identity(),
        record.layout_identity(),
        record.layout().clone(),
        shape,
    )
    .with_rust_type_kind(record.rust_type_kind())
    .with_rustc_abi_properties(record.abi_properties())
}

fn replace_abi(
    abi: &SemanticFunctionAbiV1,
    inputs: Vec<SemanticTypeIdV1>,
    arguments: Vec<SemanticAbiArgumentV1>,
    result: SemanticAbiValueV1,
    extern_abi: SemanticExternAbiV1,
    ownership: Vec<Ownership>,
) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        abi.identity(),
        abi.layout_identity(),
        if extern_abi == SemanticExternAbiV1::Unadjusted {
            SemanticCanonAbiV1::C
        } else {
            SemanticCanonAbiV1::Rust
        },
        extern_abi,
        abi.can_unwind(),
        false,
        inputs.len() as u32,
        inputs,
        result.ty(),
        arguments,
        result,
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap()
}

#[test]
fn genuine_execution_descriptors_reject_substituted_types_and_abis() {
    const CHILD: &str = "FE2O3_EXECUTION_DESCRIPTOR_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let name = format!(
            "{}::genuine_execution_descriptors_reject_substituted_types_and_abis",
            module_path!()
                .strip_prefix("rustc_codegen_fe2o3::")
                .unwrap()
        );
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--nocapture"])
            .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .env(CHILD, "1")
            .env(
                fe2o3_rustc_invocation::CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                "07".repeat(32),
            )
            .output()
            .unwrap();
        assert!(
            child.status.success() && String::from_utf8_lossy(&child.stdout).contains("1 passed"),
            "descriptor child failed\n{}\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
        return;
    }
    assert_eq!(std::env::var(CHILD).unwrap(), "1");
    let directory = TestTempDir::create("fe2o3-execution-descriptors");
    let source = directory.path().join("fixture.rs");
    std::fs::write(&source, SOURCE).unwrap();
    let executable = std::env::current_exe().unwrap();
    let dependencies = executable.parent().unwrap();
    let providers = std::fs::read_dir(dependencies)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "rlib")
                && path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("libfe2o3_device-")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        providers.len(),
        1,
        "unambiguous Cargo-built provider required"
    );
    let sysroot = Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=fe2o3_execution_descriptor_fixture".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zmir-opt-level=0".into(),
        "-Cpanic=abort".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "--extern".into(),
        format!("fe2o3_device={}", providers[0].display()),
        "-L".into(),
        format!("dependency={}", dependencies.display()),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = DescriptorCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}
