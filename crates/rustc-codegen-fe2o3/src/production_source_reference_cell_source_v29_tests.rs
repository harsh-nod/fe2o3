//! Authentic Rust scalar-cell source -> complete checked pending tile transport.
//! This is not native/SIM execution or a final allocation/safety grant.
use super::*;
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

use fe2o3_kernel_ir::{AddressSpace, OperationKind, ScalarType, Type};
use fe2o3_lower_mir_kernel::{
    ProductionCheckedTileScalarTransportV29 as Checked, ProductionTileScalarOrderV29 as Order,
    ProductionTileScalarTransportErrorV29 as TransportError,
    ProductionTileSourceSpanV29 as SourceSpan,
};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_reference_cell_source_tests::source_reference_cell_source_child";
const CASES: &[(&str, &str)] = &[
    ("repeated", "repeated"),
    ("loop", "loop"),
    ("return_alias", "return_alias"),
];

fn program(case: &str) -> String {
    let body = match case {
        "repeated" => {
            "let mut second = value.wrapping_add(9); cell_step(&mut value); cell_step(&mut second); value = value.wrapping_add(second);"
        }
        "loop" => {
            "let mut left = (base & 3) as u32; while left != 0 { cell_step(&mut value); left = left.wrapping_sub(1); }"
        }
        "return_alias" => {
            "let alias = cell_return(&mut value); *alias = (*alias).wrapping_add(5); value = *alias;"
        }
        _ => panic!("unrequested source-cell case"),
    };
    let control_flow = if case == "loop" {
        // The source counter starts at base & 3 and decreases without wrapping.
        ", control_flow(loop_bounds(3))"
    } else {
        ""
    };
    format!(
        r#"use fe2o3_device::{{kernel, thread, DisjointSlice, KernelContext, MaskedTile1D}};
#[inline(never)]
fn cell_step(value: &mut u32) {{ *value = (*value).wrapping_add(3); }}
#[inline(never)]
fn cell_return(value: &mut u32) -> &mut u32 {{ cell_step(value); value }}
#[kernel(typed, launch(required=[64,1,1], max=[64,1,1]){control_flow})]
pub fn source_cell_probe(mut ctx: KernelContext<'_>, input: &[u32], base: u64, mut output: DisjointSlice<u32>) {{
    ctx.with_workgroup(|workgroup| {{
        let mut value = base as u32;
        {body}
        let tile = MaskedTile1D::<u32,64,2,_>::load_masked(&workgroup, input, value as usize);
        let ([a,b],[ma,mb]) = tile.into_fragment().into_parts();
        let sum = value.wrapping_add(if ma {{ a }} else {{ 0 }}).wrapping_add(if mb {{ b }} else {{ 0 }});
        if let Some(slot) = output.get_mut(thread::index_1d()) {{ *slot = sum; }}
    }});
}}
"#
    )
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Observation {
    source: [u8; 32],
    pending: [u8; 32],
    current: [u8; 32],
    request: [u8; 32],
    source_file: [u8; 32],
    target: u16,
    opt: u8,
    mir: u8,
    case: String,
    callbacks: usize,
    helper_instances: usize,
    helper_source_writes: usize,
    mapped_private_writes: usize,
    exact_backings: usize,
}

fn inspect(
    checked: &Checked<'_>,
    expected_helper: SemanticFunctionIdentityV1,
    case: &str,
    budget: &mut Budget<'_>,
) -> Result<Observation, TransportError> {
    let semantic = checked.source_semantic(budget)?;
    budget.charge_work(semantic.functions().len())?;
    let mut helpers = semantic
        .functions()
        .iter()
        .enumerate()
        .filter(|(_, function)| function.identity() == expected_helper);
    let (helper_id, helper) = helpers
        .next()
        .expect("the exact live rustc helper was imported");
    assert!(helpers.next().is_none());
    let input = helper.abi().source_input_types()[0];
    let SemanticTypeShapeV1::Pointer(pointer) = semantic.types()[input.index() as usize].shape()
    else {
        panic!("shared source reference type")
    };
    assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
    assert_eq!(pointer.mutability(), SemanticMutabilityV1::Mutable);
    assert!(matches!(
        semantic.types()[pointer.pointee().index() as usize].shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32
        })
    ));
    let mut writes = std::collections::BTreeSet::new();
    for (block, body) in helper.blocks().iter().enumerate() {
        for (ordinal, statement) in body.statements().iter().enumerate() {
            budget.charge_work(1)?;
            let destination = match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => Some(assignment.destination()),
                SemanticStatementKindV1::Store(store) => Some(store.destination()),
                _ => None,
            };
            if destination.is_some_and(|place| {
                place.ty() == pointer.pointee()
                    && place.projections().iter().any(|projection| {
                        projection.kind() == SemanticProjectionKindV1::Dereference
                    })
            }) {
                writes.insert((block, ordinal));
            }
        }
    }
    assert_eq!(writes.len(), 1, "independent original helper write census");
    let mut expected_instances = std::collections::BTreeSet::new();
    for ordinal in 0..checked.instance_count(budget)? {
        let instance = checked.instance(ordinal, budget)?;
        if instance.function().index() as usize == helper_id {
            assert_eq!(instance.identity(), expected_helper);
            assert!(instance.incoming().is_some());
            expected_instances.insert((instance.root(), instance.instance().index()));
        }
    }
    assert!(!expected_instances.is_empty());
    let pending = checked.pending_ancestor(budget)?;
    let mut mapped = std::collections::BTreeSet::new();
    let mut backings = std::collections::BTreeSet::new();
    for ordinal in 0..checked.source_alias_count(budget)? {
        let alias = checked.source_alias(ordinal, budget)?;
        let SourceSpan::Statement(source) = alias.source() else {
            continue;
        };
        if source.semantic_function().index() as usize != helper_id {
            continue;
        }
        let key = (
            source.semantic_block().index() as usize,
            source.statement_ordinal() as usize,
        );
        if !writes.contains(&key) {
            continue;
        }
        let instance = (alias.root(), alias.instance().index());
        assert!(expected_instances.contains(&instance));
        assert!(mapped.insert((instance, key)));
        let root = checked.root(alias.root(), budget)?.function().0 as usize;
        let body = pending.pending_module().functions[root]
            .body
            .as_ref()
            .unwrap();
        let mut count = 0;
        for segment in alias.segments().iter().flatten() {
            budget.charge_work(body.blocks.len())?;
            let block = body
                .blocks
                .iter()
                .find(|block| block.id == segment.block())
                .unwrap();
            for operation in &block.operations
                [segment.first() as usize..(segment.first() + segment.count()) as usize]
            {
                budget.charge_work(1)?;
                if let OperationKind::Store {
                    pointer: stored,
                    access,
                    ..
                } = operation.kind
                {
                    assert_eq!(access.address_space, AddressSpace::Private);
                    assert!(!access.volatile);
                    assert_eq!(access.alignment, 4);
                    let allocations: Vec<_> = body.blocks[0].operations.iter().filter(|operation|
                        matches!(operation.kind, OperationKind::Alloca { .. })
                        && operation.results.iter().any(|result| result.id == stored
                            && matches!(&result.ty, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Private
                                && pointer.pointee.as_ref() == &Type::Scalar(ScalarType::U32)))).collect();
                    assert_eq!(
                        allocations.len(),
                        1,
                        "mapped source write reaches one genuine retained backing"
                    );
                    backings.insert((root, stored.0));
                    count += 1;
                }
            }
        }
        assert_eq!(
            count, 1,
            "exact mapped private write, not merely compilation"
        );
    }
    assert_eq!(mapped.len(), expected_instances.len() * writes.len());
    assert!(!backings.is_empty());
    Ok(Observation {
        source: *checked.source_identity(budget)?,
        pending: *checked.pending_identity(budget)?.digest(),
        current: *checked.current_identity(budget)?.digest(),
        request: [0; 32],
        source_file: [0; 32],
        target: 0,
        opt: 0,
        mir: 0,
        case: case.into(),
        callbacks: 1,
        helper_instances: expected_instances.len(),
        helper_source_writes: writes.len(),
        mapped_private_writes: mapped.len(),
        exact_backings: backings.len(),
    })
}

struct CellCallbacks {
    case: String,
    visits: usize,
    result: Option<Result<Observation, String>>,
}
impl Callbacks for CellCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.visits += 1;
        assert_eq!(self.visits, 1);
        self.result = Some((|| {
            let definition = tcx
                .iter_local_def_id()
                .map(|id| id.to_def_id())
                .find(|&id| {
                    tcx.def_kind(id) == rustc_hir::def::DefKind::Fn
                        && tcx.item_name(id).as_str() == "cell_step"
                })
                .expect("actual source helper");
            let identity = crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(
                tcx,
                rustc_middle::ty::Instance::mono(tcx, definition),
            )
            .function();
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut visits = 0;
            let report = transaction
                .consume_checked_tile_scalar_source_v29(Order::Blocked, |checked, budget| {
                    visits += 1;
                    assert_eq!(visits, 1);
                    inspect(checked, identity, &self.case, budget)
                })
                .map_err(|error| format!("authentic source cell transport refused: {error:?}"))?;
            assert_eq!(visits, 1);
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "strict child; requires exact authenticated source-cell request and a fresh response path from managed parent"]
fn source_reference_cell_source_child() {
    let request = PathBuf::from(env::var_os(ARGS).expect("strict child request"));
    let response = PathBuf::from(env::var_os(RESULT).expect("strict child response"));
    let source =
        PathBuf::from(env::var_os("FE2O3_CONTEXT_PROTOCOL_SOURCE").expect("strict child source"));
    assert!(request.is_absolute() && response.is_absolute() && source.is_absolute());
    assert!(request.is_file() && source.is_file() && !response.exists());
    assert_ne!(request, response);
    assert_ne!(source, response);
    let request_bytes = std::fs::read(&request).unwrap();
    assert!(request_bytes.len() <= REPORT_BYTES);
    let args: Vec<String> = serde_json::from_slice(&request_bytes).unwrap();
    let (target, opt, mir) = validate_arguments(&args).expect("exact managed compiler invocation");
    let source_bytes = std::fs::read(&source).unwrap();
    let case = CASES
        .iter()
        .find(|(_, case)| source_bytes == program(case).as_bytes())
        .map(|(_, case)| *case)
        .expect("exact authentic source fixture");
    let mut callbacks = CellCallbacks {
        case: case.into(),
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
        .expect("real compiler callback")
        .map(|mut row| {
            row.request = hash(&request_bytes);
            row.source_file = hash(&source_bytes);
            row.target = target;
            row.opt = opt;
            row.mir = mir;
            row
        });
    std::fs::write(&response, serde_json::to_vec(&result).unwrap()).unwrap();
    assert!(result.is_ok(), "source cell transport refused: {result:?}");
}

#[test]
#[ignore = "managed real-source gate: authentic SDK, gfx942/gfx950, opt0/mir0 and opt3/mir2; 12 additional configurations"]
fn actual_scalar_cell_sources_reach_complete_checked_pending_transport() {
    let mut seen = std::collections::BTreeSet::new();
    run_actual_sources::<Observation>(
        CASES,
        &[(0, 0), (3, 2)],
        CHILD,
        "CHECKED_SOURCE_REFERENCE_CELLS",
        program,
        |opt, mir, label, row, _| {
            assert_eq!(row.case, label);
            assert_eq!((row.opt, row.mir), (opt, mir));
            assert!([942, 950].contains(&row.target));
            assert_eq!(row.source_file, hash(program(label).as_bytes()));
            for digest in [row.source, row.pending, row.current, row.request] {
                assert_ne!(digest, [0; 32]);
            }
            assert_eq!(row.callbacks, 1);
            assert_eq!(row.helper_source_writes, 1);
            assert!(row.helper_instances > 0 && row.exact_backings > 0);
            assert_eq!(row.mapped_private_writes, row.helper_instances);
            if label == "repeated" {
                assert_eq!((row.helper_instances, row.exact_backings), (2, 2));
            }
            assert!(seen.insert((label.to_string(), row.target, opt, mir)));
        },
    );
    assert_eq!(seen.len(), 12);
}

#[test]
fn source_cell_protocol_preserves_original_programs_and_profiles() {
    let source_hashes: std::collections::BTreeSet<_> = CASES
        .iter()
        .map(|(_, case)| hash(program(case).as_bytes()))
        .collect();
    assert_eq!(source_hashes.len(), 3);
    assert!(validate_arguments(&[]).is_err());
    for (_, case) in CASES {
        let source = program(case);
        assert!(source.contains("#[inline(never)]"));
        assert!(source.contains("fn cell_step(value: &mut u32)"));
        assert!(source.contains("ctx.with_workgroup"));
        assert_eq!(
            source.contains("control_flow(loop_bounds(3))"),
            *case == "loop"
        );
        if *case == "loop" {
            assert!(source.contains("let mut left = (base & 3) as u32;"));
            assert!(source.contains("while left != 0"));
            assert!(source.contains("left = left.wrapping_sub(1);"));
        }
        assert!(!source.contains("unsafe"));
    }
}
