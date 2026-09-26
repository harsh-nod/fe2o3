//! Genuine SDK capture -> production collection/import -> checked context root.
//! No final materialization, memory-authority, transport, or execution claim.
use super::*;
use crate::rust_type_layout_general::{PointerKind, TypeLayoutFacts, TypeLayoutKind};
use crate::rustc_semantic_adapter_v1::{canonical_function_identities_v1, rustc_type_identity_v1};
use crate::trusted_device_items::{self, TrustedDeviceItem};
use rustc_hir::{Mutability, def::DefKind};
use rustc_middle::mir::TerminatorKind;
use rustc_middle::ty::{Instance, TyCtxt, TyKind, TypingEnv};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::sdk_allocation_capture_source_tests::sdk_allocation_capture_source_child";
const CASES: &[(&str, &str)] = &[
    ("write_u32", "write_u32"),
    ("write_f32", "write_f32"),
    ("disjoint_f32", "disjoint_f32"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    WriteU32,
    WriteF32,
    DisjointF32,
}
impl Case {
    fn parse(label: &str) -> Self {
        match label {
            "write_u32" => Self::WriteU32,
            "write_f32" => Self::WriteF32,
            "disjoint_f32" => Self::DisjointF32,
            _ => panic!("unrequested SDK capture case"),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::WriteU32 => "write_u32",
            Self::WriteF32 => "write_f32",
            Self::DisjointF32 => "disjoint_f32",
        }
    }
    fn provider(self) -> (&'static str, TrustedDeviceItem) {
        if self == Self::DisjointF32 {
            (
                "fe2o3_device_disjoint_slice",
                TrustedDeviceItem::DisjointSlice,
            )
        } else {
            (
                "fe2o3_device_write_only_disjoint_slice_v1",
                TrustedDeviceItem::WriteOnlyDisjointSlice,
            )
        }
    }
    fn terminal(self) -> (&'static str, TrustedDeviceItem) {
        if self == Self::DisjointF32 {
            (
                "fe2o3_device_disjoint_slice_get_mut",
                TrustedDeviceItem::DisjointSliceGetMut,
            )
        } else {
            (
                "fe2o3_device_write_only_disjoint_slice_write_v1",
                TrustedDeviceItem::WriteOnlyDisjointSliceWrite,
            )
        }
    }
    fn scalar(self) -> SemanticScalarTypeV1 {
        if self == Self::WriteU32 {
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }
        } else {
            SemanticScalarTypeV1::Float { bits: 32 }
        }
    }
}

fn program(label: &str) -> String {
    let case = Case::parse(label);
    let (carrier, element, write) = match case {
        Case::WriteU32 => (
            "WriteOnlyDisjointSlice",
            "u32",
            "let _ = output.write(thread::index_1d(), 7_u32);",
        ),
        Case::WriteF32 => (
            "WriteOnlyDisjointSlice",
            "f32",
            "let _ = output.write(thread::index_1d(), 7.0_f32);",
        ),
        Case::DisjointF32 => (
            "DisjointSlice",
            "f32",
            "if let Some(slot) = output.get_mut(thread::index_1d()) { *slot = 7.0_f32; }",
        ),
    };
    format!(
        r#"use fe2o3_device::{{kernel, thread, KernelContext, {carrier}}};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn sdk_capture_probe(mut ctx: KernelContext<'_>, mut output: {carrier}<{element}>) {{
    ctx.with_workgroup(|_workgroup| {{
        {write}
    }});
}}
"#
    )
}

#[derive(Debug, Eq, PartialEq)]
struct CompilerCapture {
    helper: SemanticFunctionIdentityV1,
    closure_function: SemanticItemDefinitionIdentityV1,
    closure_type: SemanticTypeIdentityV1,
    capture_type: SemanticTypeIdentityV1,
    carrier_type: SemanticTypeIdentityV1,
    element_type: SemanticTypeIdentityV1,
    terminal: SemanticFunctionIdentityV1,
    source_index: usize,
    physical: TypeLayoutFacts,
}

fn authenticated_item(
    tcx: TyCtxt<'_>,
    item: (&str, TrustedDeviceItem),
) -> rustc_hir::def_id::DefId {
    let definition = tcx
        .get_diagnostic_item(rustc_span::Symbol::intern(item.0))
        .expect("genuine SDK diagnostic item");
    assert_eq!(
        trusted_device_items::classify(tcx, definition),
        Some(item.1)
    );
    definition
}

// Expectations come from the live compiler types and actual safe terminal call,
// independently of the semantic owner consumed below.
fn compiler_capture(tcx: TyCtxt<'_>, case: Case) -> CompilerCapture {
    let provider = authenticated_item(tcx, case.provider());
    let terminal = authenticated_item(tcx, case.terminal());
    let mut captures = Vec::new();
    for definition in tcx.iter_local_def_id() {
        let definition = definition.to_def_id();
        if tcx.def_kind(definition) != DefKind::Fn || tcx.generics_of(definition).count() != 0 {
            continue;
        }
        let helper = Instance::mono(tcx, definition);
        let body = tcx.instance_mir(helper.def);
        for declaration in &body.local_decls {
            let ty = tcx
                .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), declaration.ty)
                .expect("monomorphic local type");
            let TyKind::Closure(closure_definition, arguments) = ty.kind() else {
                continue;
            };
            for (source_index, capture) in arguments.as_closure().upvar_tys().iter().enumerate() {
                let TyKind::Ref(_, carrier, Mutability::Mut) = capture.kind() else {
                    continue;
                };
                let TyKind::Adt(adt, generic) = carrier.kind() else {
                    continue;
                };
                if adt.did() != provider {
                    continue;
                }
                assert_eq!(
                    trusted_device_items::classify(tcx, adt.did()),
                    Some(case.provider().1)
                );
                assert_eq!(generic.len(), 2);
                let element = generic[0].as_type().expect("SDK element type");
                assert_eq!(
                    element,
                    if case == Case::WriteU32 {
                        tcx.types.u32
                    } else {
                        tcx.types.f32
                    }
                );
                let physical =
                    crate::rust_type_layout_general::extract_capture_layout(tcx, capture)
                        .expect("unchanged real SDK capture layout");
                let TypeLayoutKind::Pointer(reference) = &physical.kind else {
                    panic!("actual borrowed capture")
                };
                assert_eq!(reference.kind, PointerKind::MutableReference);
                let TypeLayoutKind::Adt(view) = &reference.pointee.kind else {
                    panic!("actual SDK carrier")
                };
                assert_eq!(view.variants.len(), 1);
                assert_eq!(view.variants[0].fields.len(), 3);
                let TypeLayoutKind::Pointer(pointer) = &view.variants[0].fields[0].layout.kind
                else {
                    panic!("actual carrier representation")
                };
                assert_eq!(pointer.kind, PointerKind::MutRaw);
                assert_eq!(pointer.address_space, 0);
                assert_eq!(pointer.pointee.size_bytes, 4);
                assert_eq!(reference.pointee.size_bytes, 16);
                let closure = Instance::new_raw(*closure_definition, *arguments);
                let closure_body = tcx.instance_mir(closure.def);
                let mut actual_terminal = None;
                for block in closure_body.basic_blocks.iter() {
                    let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                        continue;
                    };
                    let callee = crate::closure_profile_v1::resolve_direct_call(tcx, closure, func)
                        .expect("actual direct safe SDK call");
                    if callee.def_id() == terminal {
                        assert!(
                            actual_terminal
                                .replace(canonical_function_identities_v1(tcx, callee).function())
                                .is_none()
                        );
                    }
                }
                let row = CompilerCapture {
                    helper: canonical_function_identities_v1(tcx, helper).function(),
                    closure_function: canonical_function_identities_v1(tcx, closure)
                        .item_definition(),
                    closure_type: rustc_type_identity_v1(tcx, ty),
                    capture_type: rustc_type_identity_v1(tcx, capture),
                    carrier_type: rustc_type_identity_v1(tcx, *carrier),
                    element_type: rustc_type_identity_v1(tcx, element),
                    terminal: actual_terminal
                        .expect("write/get_mut occurs inside the actual capturing closure"),
                    source_index,
                    physical,
                };
                if !captures.contains(&row) {
                    captures.push(row);
                }
            }
        }
    }
    assert_eq!(captures.len(), 1, "one exact genuine SDK closure capture");
    captures.pop().unwrap()
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    case: String,
    source: [u8; 32],
    source_file: [u8; 32],
    request: [u8; 32],
    target: u16,
    opt: u8,
    mir: u8,
    callbacks: usize,
    captures: usize,
    owner_inputs: usize,
    terminal_calls: usize,
    derives: usize,
}

fn type_id(
    source: &AdmittedInertSemanticMirV1,
    identity: SemanticTypeIdentityV1,
    budget: &mut Budget<'_>,
) -> Result<SemanticTypeIdV1, ResourceError> {
    budget.charge_work(source.types().len())?;
    let mut matches = source
        .types()
        .iter()
        .enumerate()
        .filter(|(_, ty)| ty.identity() == identity);
    let (index, _) = matches.next().expect("compiler type retained by import");
    assert!(matches.next().is_none());
    Ok(SemanticTypeIdV1::from_index(u32::try_from(index).unwrap()))
}

fn inspect(
    root: ProductionCheckedContextRootV29<'_>,
    budget: &mut Budget<'_>,
    expected: &CompilerCapture,
    case: Case,
) -> Result<Observation, ProductionContextRootErrorV29> {
    let source = root.semantic_ssa().source_semantic();
    assert_eq!(source.wire_version(), SemanticMirWireVersionV1::V29);
    assert_eq!(root.helper().identity(), expected.helper);
    let closure_ty = type_id(source, expected.closure_type, budget)?;
    let capture_ty = type_id(source, expected.capture_type, budget)?;
    let carrier_ty = type_id(source, expected.carrier_type, budget)?;
    let element_ty = type_id(source, expected.element_type, budget)?;
    let SemanticTypeShapeV1::Aggregate(closure) =
        source.types()[closure_ty.index() as usize].shape()
    else {
        panic!("imported closure aggregate")
    };
    assert_eq!(closure.fields()[expected.source_index], capture_ty);
    let SemanticTypeShapeV1::Pointer(capture) = source.types()[capture_ty.index() as usize].shape()
    else {
        panic!("imported mutable capture reference")
    };
    assert_eq!(capture.kind(), SemanticPointerKindV1::Reference);
    assert_eq!(capture.mutability(), SemanticMutabilityV1::Mutable);
    assert_eq!(capture.pointee(), carrier_ty);
    let SemanticTypeShapeV1::Aggregate(carrier) =
        source.types()[carrier_ty.index() as usize].shape()
    else {
        panic!("imported authentic SDK carrier")
    };
    assert_eq!(carrier.fields().len(), 3);
    let SemanticTypeShapeV1::Pointer(pointer) =
        source.types()[carrier.fields()[0].index() as usize].shape()
    else {
        panic!("preserved SDK pointer representation")
    };
    assert_eq!(pointer.kind(), SemanticPointerKindV1::Raw);
    assert_eq!(pointer.mutability(), SemanticMutabilityV1::Mutable);
    assert_eq!(pointer.pointee(), element_ty);
    assert_eq!(
        source.types()[element_ty.index() as usize].shape(),
        &SemanticTypeShapeV1::Scalar(case.scalar())
    );
    budget.charge_work(
        root.helper().locals().len() + root.helper().abi().source_input_types().len(),
    )?;
    assert!(
        root.helper()
            .locals()
            .iter()
            .any(|local| local.ty() == closure_ty)
    );
    let mut owner_inputs = 0;
    for (ordinal, input) in root.helper().abi().source_input_types().iter().enumerate() {
        if *input == carrier_ty {
            assert_eq!(
                root.helper().abi().source_argument_ownership()[ordinal],
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner
            );
            owner_inputs += 1;
        }
    }
    assert_eq!(owner_inputs, 1);
    let mut terminal_calls = 0;
    let mut derives = 0;
    for function in source.functions() {
        budget.charge_work(1)?;
        for block in function.blocks() {
            budget.charge_work(1)?;
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let SemanticCallableDeclV1::CompilerIntrinsic {
                binding, operation, ..
            } = &source.callables()[call.callee().index() as usize]
            else {
                continue;
            };
            if matches!(
                operation,
                SemanticCompilerIntrinsicOperationV1::Execution(
                    SemanticExecutionOperationV29::WorkgroupDerive { .. }
                )
            ) {
                derives += 1;
            }
            if binding.identity() != expected.terminal {
                continue;
            }
            assert_eq!(
                function.item_definition_identity(),
                expected.closure_function
            );
            let (view, element, argc) = match operation {
                SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                    disjoint_slice,
                    element,
                    kind,
                    ..
                } => {
                    assert_ne!(case, Case::DisjointF32);
                    assert_eq!(
                        *kind,
                        SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint: false }
                    );
                    (*disjoint_slice, *element, 3)
                }
                SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                    disjoint_slice,
                    element,
                    ..
                } => {
                    assert_eq!(case, Case::DisjointF32);
                    (*disjoint_slice, *element, 2)
                }
                other => panic!("wrong actual SDK terminal: {other:?}"),
            };
            assert_eq!(view, carrier_ty);
            assert_eq!(element, element_ty);
            assert_eq!(call.arguments().len(), argc);
            assert_eq!(binding.abi().source_input_types().len(), argc);
            assert_eq!(binding.abi().source_input_types()[0], capture_ty);
            assert_eq!(
                binding.abi().source_argument_ownership()[0],
                SemanticSourceArgumentOwnershipV1::UniqueBorrow
            );
            let (SemanticOperandV1::Move(receiver) | SemanticOperandV1::Copy(receiver)) =
                &call.arguments()[0]
            else {
                panic!("retained terminal receiver place")
            };
            assert_eq!(receiver.ty(), capture_ty);
            if case != Case::DisjointF32 {
                assert_eq!(binding.abi().source_input_types()[2], element_ty);
            }
            terminal_calls += 1;
        }
    }
    assert_eq!(terminal_calls, 1);
    assert_eq!(derives, 1);
    Ok(Observation {
        case: case.label().into(),
        source: *root.semantic_ssa().source_semantic_sha256(),
        source_file: [0; 32],
        request: [0; 32],
        target: 0,
        opt: 0,
        mir: 0,
        callbacks: 1,
        captures: 1,
        owner_inputs,
        terminal_calls,
        derives,
    })
}

struct CaptureCallbacks {
    case: Case,
    visits: usize,
    result: Option<Result<Observation, String>>,
}
impl Callbacks for CaptureCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_pipeline::ProductionPipelineError;
        use fe2o3_lower_mir_kernel::{ProductionPreRankedKirErrorV1, ProductionSemanticKirErrorV1};
        self.visits += 1;
        assert_eq!(self.visits, 1);
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            // Capture producer custody before instance_mir can steal its input.
            // Derive compiler expectations before semantic import/observation.
            let expected = compiler_capture(tcx, self.case);
            let mut observation = None;
            let result = transaction.observe_context_handoff_v29(|root, budget| {
                assert!(
                    observation.is_none(),
                    "exactly one checked root after authentic collection/import"
                );
                observation = Some(inspect(root, budget, &expected, self.case)?);
                Ok(())
            });
            match result {
                Err(error)
                    if matches!(
                        *error,
                        ProductionPipelineError::PreRankedMaterialization(
                            ProductionPreRankedKirErrorV1::Lowering(
                                ProductionSemanticKirErrorV1::Unsupported {
                                    detail: "execution capabilities require checked canonical KIR materialization",
                                    ..
                                }
                            )
                        )
                    ) => {}
                Err(error) => return Err(format!("unexpected source boundary: {error}")),
                Ok(()) => return Err("staged callback unexpectedly materialized".into()),
            }
            assert_eq!(compiler_capture(tcx, self.case), expected);
            observation.ok_or_else(|| {
                "refusal occurred before the checked context-root observation".into()
            })
        })());
        Compilation::Stop
    }
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn validate_arguments(args: &[String]) -> Result<(u16, u8, u8), &'static str> {
    if args.len() != 27 {
        return Err("exact parent argc");
    }
    for (index, expected) in [
        (1, "--crate-name"),
        (2, "fe2o3_production_ranked_bounds_fixture"),
        (3, "--crate-type=lib"),
        (4, "--edition=2024"),
        (5, "--target=amdgcn-amd-amdhsa"),
        (
            7,
            "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        ),
        (9, "-Cdebug-assertions=off"),
        (10, "-Coverflow-checks=on"),
        (12, "-Zalways-encode-mir"),
        (13, "-Zunstable-options"),
        (14, "--cfg=feature=\"provider_context_protocol\""),
        (15, "--emit=metadata"),
        (16, "--sysroot"),
        (23, "--out-dir"),
    ] {
        if args[index] != expected {
            return Err("changed parent argument");
        }
    }
    let (opt, mir) = match (args[8].as_str(), args[11].as_str()) {
        ("-Copt-level=0", "-Zmir-opt-level=0") => (0, 0),
        ("-Copt-level=3", "-Zmir-opt-level=2") => (3, 2),
        _ => return Err("unrequested optimization profile"),
    };
    let target = match args[6].as_str() {
        "-Ctarget-cpu=gfx942" => 942,
        "-Ctarget-cpu=gfx950" => 950,
        _ => return Err("target"),
    };
    for (index, prefix) in [
        (18, "--extern=fe2o3_device="),
        (19, "--extern=noprelude:core="),
        (20, "--extern=noprelude:compiler_builtins="),
    ] {
        let file = args[index].strip_prefix(prefix).ok_or("extern ordering")?;
        if !Path::new(file).is_absolute() || !Path::new(file).is_file() {
            return Err("extern file");
        }
    }
    for index in [21, 22] {
        let directory = args[index]
            .strip_prefix("-Ldependency=")
            .ok_or("dependency ordering")?;
        if !Path::new(directory).is_absolute() || !Path::new(directory).is_dir() {
            return Err("dependency directory");
        }
    }
    let manifest = env::var_os("CARGO_MANIFEST_DIR").ok_or("manifest environment")?;
    if Path::new(&args[25]) != Path::new(&manifest).join("src/lib.rs") {
        return Err("fixture source path");
    }
    for index in [17, 24] {
        if !Path::new(&args[index]).is_absolute() || !Path::new(&args[index]).is_dir() {
            return Err("compiler paths");
        }
    }
    let metadata = args[26]
        .strip_prefix("-Cmetadata=")
        .filter(|value| !value.is_empty())
        .ok_or("metadata binding")?;
    let actual: Vec<OsString> = args.iter().map(OsString::from).collect();
    let RustcInvocationV2::Compile(compile) =
        classify_rustc_invocation_v2(&actual).map_err(|_| "compiler invocation")?
    else {
        return Err("not a compiler invocation");
    };
    let ordered = ordered_rustc_codegen_metadata_v1(compile).map_err(|_| "metadata")?;
    let build = derive_cargo_metadata_build_observation_v2(&ordered);
    if env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).map_err(|_| "build binding")?
        != build.to_hex()
    {
        return Err("foreign build binding");
    }
    let binding = derive_crate_binding_id_v1(&args[2], [metadata]);
    if env::var(CRATE_BINDING_ID_ENV_V1).map_err(|_| "crate binding")? != binding.to_hex() {
        return Err("foreign crate binding");
    }
    Ok((target, opt, mir))
}

#[test]
#[ignore = "strict child; requires exact authentic SDK source request and fresh response path from managed parent"]
fn sdk_allocation_capture_source_child() {
    let request =
        std::path::PathBuf::from(env::var_os(ARGS).expect("strict child actual-source request"));
    let response =
        std::path::PathBuf::from(env::var_os(RESULT).expect("strict child fresh report"));
    let source = std::path::PathBuf::from(
        env::var_os("FE2O3_CONTEXT_PROTOCOL_SOURCE").expect("strict child original source"),
    );
    assert!(request.is_absolute() && response.is_absolute() && source.is_absolute());
    assert!(request.is_file() && source.is_file() && !response.exists());
    assert_ne!(request, response);
    assert_ne!(source, response);
    let request_bytes = std::fs::read(&request).unwrap();
    assert!(request_bytes.len() <= REPORT_BYTES);
    let args: Vec<String> = serde_json::from_slice(&request_bytes).unwrap();
    let (target, opt, mir) = validate_arguments(&args).expect("exact authentic parent invocation");
    let source_bytes = std::fs::read(&source).unwrap();
    let case = CASES
        .iter()
        .find(|(_, label)| source_bytes == program(label).as_bytes())
        .map(|(_, label)| Case::parse(label))
        .expect("exact requested SDK capture fixture");
    let mut callbacks = CaptureCallbacks {
        case,
        visits: 0,
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert_eq!(std::fs::read(&request).unwrap(), request_bytes);
    assert_eq!(std::fs::read(&source).unwrap(), source_bytes);
    assert_eq!(callbacks.visits, 1);
    assert!(!response.exists());
    let result = callbacks
        .result
        .expect("actual callback ran")
        .map(|mut row| {
            row.request = hash(&request_bytes);
            row.source_file = hash(&source_bytes);
            row.target = target;
            row.opt = opt;
            row.mir = mir;
            row
        });
    std::fs::write(&response, serde_json::to_vec(&result).unwrap()).unwrap();
    assert!(
        result.is_ok(),
        "genuine SDK capture source refused: {result:?}"
    );
}

#[test]
#[ignore = "managed actual-source gate: authentic SDK, gfx942/gfx950, opt0/mir0 and opt3/mir2; 12 additional configurations"]
fn actual_sdk_allocation_view_captures_reach_checked_context_root() {
    let mut configurations = std::collections::BTreeSet::new();
    run_actual_sources::<Observation>(
        CASES,
        &[(0, 0), (3, 2)],
        CHILD,
        "CHECKED_SDK_ALLOCATION_CAPTURE_SOURCE",
        program,
        |opt, mir, label, row, _| {
            assert_eq!(row.case, label);
            assert_eq!((row.opt, row.mir), (opt, mir));
            assert!([942, 950].contains(&row.target));
            assert_eq!(row.source_file, hash(program(label).as_bytes()));
            assert_ne!(row.source, [0; 32]);
            assert_ne!(row.request, [0; 32]);
            assert_eq!(
                (
                    row.callbacks,
                    row.captures,
                    row.owner_inputs,
                    row.terminal_calls,
                    row.derives
                ),
                (1, 1, 1, 1, 1)
            );
            assert!(
                configurations.insert((row.target, opt, mir, label.to_owned())),
                "unique source configuration"
            );
        },
    );
    assert_eq!(configurations.len(), 12);
}
