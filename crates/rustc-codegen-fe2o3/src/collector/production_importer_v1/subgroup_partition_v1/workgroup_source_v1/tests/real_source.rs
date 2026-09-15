//! Inspect authenticated device metadata, never an invented Workgroup issuer.

use super::*;
use rustc_middle::mir::Operand;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{CurrentTarget, InitialEpoch, KernelCapabilityBrand, RegisteredLaunch,
    Subgroup, SubgroupWidth64, WorkgroupCapability, WorkgroupEpoch};
type Brand<'a> = KernelCapabilityBrand<'a, u8, CurrentTarget, RegisteredLaunch>;
type Workgroup<'a> = WorkgroupCapability<'a, Brand<'a>, InitialEpoch>;
type Epoch<'a> = WorkgroupEpoch<'a, Brand<'a>, InitialEpoch>;
type Group<'a> = Subgroup<'a, SubgroupWidth64, Brand<'a>, InitialEpoch>;
pub fn same_owner<'a>(w: &'a Workgroup<'a>) -> (Group<'a>, &'a Epoch<'a>) {
    (w.subgroup::<SubgroupWidth64>(), w.epoch())
}
pub fn other_owner<'a>(w: &'a Workgroup<'a>, other: &'a Workgroup<'a>) -> (Group<'a>, &'a Epoch<'a>) {
    (w.subgroup::<SubgroupWidth64>(), other.epoch())
}
pub fn epoch(w: &u32) -> &u32 { w }
pub fn subgroup(w: &u32) -> u32 { *w }
"#;

struct RealSourceProbe {
    amdgpu: bool,
    completed: bool,
}

fn observed_subgroup_types<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    types: &mut Vec<SemanticTypeDeclV1>,
) -> SemanticTypeIdV1 {
    let identity = rustc_type_identity_v1(tcx, ty);
    if let Some(index) = types.iter().position(|decl| decl.identity() == identity) {
        return SemanticTypeIdV1::from_index(index.try_into().unwrap());
    }
    let layout = tcx
        .layout_of(TypingEnv::fully_monomorphized().as_query_input(ty))
        .unwrap();
    let size = layout.size.bytes();
    let align = layout.align.abi.bytes();
    let (shape, layout) = match *ty.kind() {
        TyKind::Uint(UintTy::U32) => (
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
            SemanticTypeLayoutV1::new(Some(size), align).unwrap(),
        ),
        TyKind::Adt(definition, arguments) if definition.is_struct() => {
            let fields = definition
                .non_enum_variant()
                .fields
                .iter()
                .map(|field| {
                    let ty = tcx.normalize_erasing_regions(
                        TypingEnv::fully_monomorphized(),
                        field.ty(tcx, arguments),
                    );
                    observed_subgroup_types(tcx, ty, types)
                })
                .collect::<Vec<_>>();
            let offsets = (0..fields.len())
                .map(|index| layout.fields.offset(index).bytes())
                .collect();
            (
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
                SemanticTypeLayoutV1::aggregate(
                    Some(size),
                    align,
                    SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
                )
                .unwrap(),
            )
        }
        _ => panic!("unexpected subgroup storage: {ty:?}"),
    };
    let id = SemanticTypeIdV1::from_index(types.len().try_into().unwrap());
    // Layout-only observations exercise the child check, not a production ABI or root.
    types.push(SemanticTypeDeclV1::new(
        identity,
        SemanticLayoutIdentityV1::from_sha256([90; 32]),
        layout,
        shape,
    ));
    id
}

impl Callbacks for RealSourceProbe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("workgroup_real_source.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(
            tcx.sess.target.llvm_target.starts_with("amdgcn"),
            self.amdgpu
        );
        for name in ["epoch", "subgroup"] {
            let local = instance(tcx, name);
            assert_eq!(trusted_device_items::classify(tcx, local.def_id()), None);
            assert!(epoch_signature_v1(tcx, local).is_err());
            assert!(subgroup_signature_v1(tcx, local).is_err());
        }
        for (caller_name, epoch_owner) in [("same_owner", 1), ("other_owner", 2)] {
            let caller = instance(tcx, caller_name);
            let body = tcx.instance_mir(caller.def);
            let before = format!("{body:?}");
            let mut subgroup_owner = None;
            let mut observed_epoch_owner = None;
            for block in body.basic_blocks.iter() {
                let TerminatorKind::Call { func, args, .. } = &block.terminator().kind else {
                    continue;
                };
                let TyKind::FnDef(definition, arguments) = *func.ty(&body.local_decls, tcx).kind()
                else {
                    panic!("source call must remain direct");
                };
                let callee = Instance::try_resolve(
                    tcx,
                    TypingEnv::fully_monomorphized(),
                    definition,
                    tcx.erase_and_anonymize_regions(arguments),
                )
                .unwrap()
                .unwrap();
                let abi = tcx
                    .fn_abi_of_instance(
                        TypingEnv::fully_monomorphized()
                            .as_query_input((callee, rustc_middle::ty::List::empty())),
                    )
                    .unwrap();
                assert_eq!(abi.args.len(), 1);
                assert!(matches!(
                    abi.args[0].mode,
                    rustc_target::callconv::PassMode::Direct(_)
                ));
                assert!(matches!(
                    abi.ret.mode,
                    rustc_target::callconv::PassMode::Direct(_)
                ));
                let [argument] = args.as_ref() else {
                    panic!("exact receiver argument")
                };
                let (Operand::Copy(place) | Operand::Move(place)) = &argument.node else {
                    panic!("receiver must retain its source place");
                };
                assert!(place.projection.is_empty());
                if epoch_provider_v1(tcx, callee) {
                    let signature = epoch_signature_v1(tcx, callee).unwrap();
                    let owned = rust_shared_reference_v1(signature.inputs()[0]).unwrap();
                    assert_ne!(
                        rustc_type_identity_v1(tcx, signature.inputs()[0]),
                        rustc_type_identity_v1(tcx, owned)
                    );
                    assert!(observed_epoch_owner.replace(place.local.index()).is_none());
                    assert_eq!(
                        crate::production_semantic_terminal_v1::classify(tcx, callee.def_id()),
                        None
                    );
                    let selected =
                        crate::production_rustc_intrinsic_v1::production_mir_v1(tcx, callee);
                    assert!(std::ptr::eq(selected.body(), tcx.instance_mir(callee.def)));
                    assert!(!selected.is_source_expansion());
                    let mut wrong_owner = tcx.instance_mir(callee.def).clone();
                    let StatementKind::Assign(assignment) =
                        &mut wrong_owner.basic_blocks.as_mut()[START_BLOCK].statements[0].kind
                    else {
                        panic!("reviewed epoch projection");
                    };
                    let Rvalue::Ref(_, _, place) = &mut assignment.1 else {
                        unreachable!()
                    };
                    place.local = RETURN_PLACE;
                    assert!(!reviewed_epoch_body_v1(
                        tcx,
                        callee,
                        &wrong_owner,
                        signature
                    ));
                } else {
                    let signature = subgroup_signature_v1(tcx, callee).unwrap();
                    assert_eq!(abi.ret.layout.size.bytes(), 4);
                    assert!(subgroup_owner.replace(place.local.index()).is_none());
                    let owned = rust_shared_reference_v1(signature.inputs()[0]).unwrap();
                    assert_ne!(
                        rustc_type_identity_v1(tcx, signature.inputs()[0]),
                        rustc_type_identity_v1(tcx, owned)
                    );
                    let mut types = Vec::new();
                    let subgroup = observed_subgroup_types(tcx, signature.output(), &mut types);
                    assert_eq!(
                        types[subgroup.index() as usize].layout().size_bytes(),
                        Some(4)
                    );
                    assert!(exact_subgroup_source_layout_v1(&types, subgroup));
                    assert!(!semantic_exact_inhabited_aggregate_zst_v1(&types, subgroup));
                    let width_index = callee
                        .args
                        .iter()
                        .rposition(|arg| arg.as_type().is_some())
                        .unwrap();
                    let mut changed = callee.args.to_vec();
                    changed[width_index] = tcx.types.u16.into();
                    assert!(
                        subgroup_signature_v1(
                            tcx,
                            Instance {
                                args: tcx.mk_args(&changed),
                                ..callee
                            }
                        )
                        .is_err()
                    );
                }
                let brand_index = callee
                    .args
                    .iter()
                    .position(|arg| arg.as_type().is_some())
                    .unwrap();
                let mut changed = callee.args.to_vec();
                changed[brand_index] = tcx.types.u16.into();
                let changed = Instance {
                    args: tcx.mk_args(&changed),
                    ..callee
                };
                assert!(epoch_signature_v1(tcx, changed).is_err());
                assert!(subgroup_signature_v1(tcx, changed).is_err());
            }
            assert_eq!(subgroup_owner, Some(1));
            assert_eq!(observed_epoch_owner, Some(epoch_owner));
            // Identical receiver types must not collapse two different caller origins.
            if epoch_owner == 2 {
                assert_eq!(
                    body.local_decls[Local::from_usize(1)].ty,
                    body.local_decls[Local::from_usize(2)].ty
                );
                assert_ne!(subgroup_owner, observed_epoch_owner);
            }
            assert_eq!(format!("{body:?}"), before);
        }
        self.completed = true;
        Compilation::Stop
    }
}

const PROBE_METADATA: &str = "fe2o3-workgroup-source-v1";
const CHILD_ENV: &str = "FE2O3_WORKGROUP_SOURCE_PROBE_CHILD";

fn configured_path(name: &str) -> std::path::PathBuf {
    let path = std::path::PathBuf::from(
        std::env::var_os(name).unwrap_or_else(|| panic!("set {name} to existing cached metadata")),
    );
    assert!(path.exists(), "{name}: {}", path.display());
    path
}

fn run_real_source(amdgpu: bool) {
    use fe2o3_rustc_invocation::{
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, derive_cargo_metadata_build_observation_v2,
    };
    let observation = derive_cargo_metadata_build_observation_v2(&[PROBE_METADATA]).to_hex();
    if std::env::var(CHILD_ENV).ok().as_deref() != Some(observation.as_str()) {
        let test = if amdgpu {
            "real_workgroup_amdgpu_source_retains_distinct_borrows_and_lane"
        } else {
            "real_workgroup_host_source_retains_distinct_borrows_and_lane"
        };
        let output = crate::process_execution::capture_output(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([test, "--ignored", "--nocapture", "--test-threads=1"])
                .env(CHILD_ENV, &observation)
                .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "workgroup source child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        return;
    }
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        observation
    );
    let device = configured_path(if amdgpu {
        "FE2O3_CORE_TRY_DEVICE_RMETA"
    } else {
        "FE2O3_POLICY_DEVICE_RLIB"
    });
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=workgroup_real_source".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Cpanic=abort".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        format!("-Cmetadata={PROBE_METADATA}"),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-".into(),
    ];
    if amdgpu {
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            "-Ctarget-cpu=gfx942".into(),
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
            "-Zunstable-options".into(),
            "-L".into(),
            format!(
                "dependency={}",
                configured_path("FE2O3_CORE_TRY_HOST_DEPS").display()
            ),
            "--extern".into(),
            format!(
                "noprelude,nounused:core={}",
                configured_path("FE2O3_CORE_TRY_AMDGPU_CORE").display()
            ),
            "--extern".into(),
            format!(
                "noprelude,nounused:compiler_builtins={}",
                configured_path("FE2O3_CORE_TRY_AMDGPU_BUILTINS").display()
            ),
        ]);
    }
    let mut probe = RealSourceProbe {
        amdgpu,
        completed: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.completed);
}

#[test]
#[ignore = "requires cached FE2O3_POLICY_DEVICE_RLIB with encoded MIR; no dependency build"]
fn real_workgroup_host_source_retains_distinct_borrows_and_lane() {
    run_real_source(false);
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS; no dependency build"]
fn real_workgroup_amdgpu_source_retains_distinct_borrows_and_lane() {
    run_real_source(true);
}
