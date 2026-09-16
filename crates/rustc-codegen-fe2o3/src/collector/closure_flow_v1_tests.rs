use super::super::{DeviceCollector, KernelRoot, kernel_context_auth_v1};
use super::*;
use crate::closure_profile_v1::{authenticate_once_shim_v1, observe_closures_v2};
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use rustc_middle::ty::{ClosureKind, InstanceKind};

const SOURCE: &str = r#"
#![allow(dead_code, unused_unsafe)]
struct Token(u32);
#[inline(never)]
fn apply<F: FnOnce(u32) -> u32>(value: u32, f: F) -> u32 { f(value) }
#[inline(never)]
fn hop<F: FnOnce(u32) -> u32>(f: F, value: u32) -> u32 { apply(value, f) }
#[inline(never)]
fn through_fn<F: Fn(u32) -> u32>(f: F, value: u32) -> u32 { hop(f, value) }
#[inline(never)]
fn through_mut<F: FnMut(u32) -> u32>(f: F, value: u32) -> u32 { hop(f, value) }
#[inline(never)]
fn repeat<F: Fn(u32) -> u32 + Copy>(f: F) -> u32 { hop(f, 1) ^ hop(f, 2) }
pub fn repeated(seed: u32) -> u32 { repeat(move |x| seed ^ x) }
pub fn by_value(seed: u32) -> u32 { through_fn(move |x| seed ^ x, 7) }
pub fn by_reference(seed: &u32) -> u32 { through_fn(|x| *seed ^ x, 7) }
pub fn mutable(seed: &mut u32) -> u32 { through_mut(|x| { *seed ^= x; *seed }, 7) }
pub fn once(seed: u32) -> u32 {
    let token = Token(seed);
    hop(move |x| { let moved = token; moved.0 ^ x }, 7)
}
pub fn unsafe_body(seed: u32) -> u32 { through_fn(move |x| unsafe { seed ^ x }, 7) }
unsafe fn unsafe_helper(seed: u32) -> u32 { seed }
pub fn unsafe_reachable(seed: u32) -> u32 {
    through_fn(move |x| unsafe { unsafe_helper(seed) ^ x }, 7)
}
#[inline(never)]
fn ordinary(seed: u32) -> u32 { seed ^ 2 }
pub fn no_closures(seed: u32) -> u32 { ordinary(seed) }
pub fn eight(seed: u32) -> [u32; 8] {
    let a = move || seed;
    let b = move || seed ^ 1;
    let c = move || seed ^ 2;
    let d = move || seed ^ 3;
    let e = move || seed ^ 4;
    let f = move || seed ^ 5;
    let g = move || seed ^ 6;
    let h = move || seed ^ 7;
    let alias = h;
    [a(), b(), c(), d(), e(), f(), g(), alias()]
}
pub fn nine(seed: u32) -> [u32; 9] {
    let a = move || seed;
    let b = move || seed ^ 1;
    let c = move || seed ^ 2;
    let d = move || seed ^ 3;
    let e = move || seed ^ 4;
    let f = move || seed ^ 5;
    let g = move || seed ^ 6;
    let h = move || seed ^ 7;
    let i = move || seed ^ 8;
    [a(), b(), c(), d(), e(), f(), g(), h(), i()]
}
#[inline(never)]
fn call_shared<F: Fn() -> u32>(f: F) -> u32 { f() }
#[inline(never)]
pub fn projected<F: Fn() -> u32 + Copy>(pair: (F,)) -> u32 { call_shared(pair.0) }
pub fn local(seed: &u32) -> u32 { let f = move || *seed; call_shared(f) }
"#;

fn local<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    Instance::mono(
        tcx,
        tcx.hir_body_owners()
            .find(|id| {
                tcx.def_kind(id.to_def_id()) == DefKind::Fn
                    && tcx.item_name(id.to_def_id()).as_str() == name
            })
            .unwrap()
            .to_def_id(),
    )
}

fn collector<'tcx>(tcx: TyCtxt<'tcx>, root: Instance<'tcx>) -> DeviceCollector<'tcx> {
    let mut collector = DeviceCollector::new(
        tcx,
        false,
        Vec::new(),
        "gfx942:xnack-".into(),
        kernel_context_auth_v1::capture_context_producers_v1(tcx).unwrap(),
    );
    collector
        .add_root(KernelRoot {
            target: root,
            logical_name: "component_source_root".into(),
            export_name: "component_source_root".into(),
            generated_host_contract_identity: None,
            kernel_binding: None,
            frontend_contract: None,
            kernel_context_contract: None,
            reference_effect_binding: None,
        })
        .unwrap();
    collector
}

fn collect<'tcx>(
    tcx: TyCtxt<'tcx>,
    name: &str,
) -> (CollectionResult<'tcx>, AuthenticatedClosureFlowV1<'tcx>) {
    let (collection, _, flow) = collector(tcx, local(tcx, name)).collect().unwrap();
    (collection, flow)
}

struct CheckCallbacks(for<'tcx> fn(TyCtxt<'tcx>), bool);

impl Callbacks for CheckCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        (self.0)(tcx);
        self.1 = true;
        Compilation::Stop
    }
}

fn with_source(check: for<'tcx> fn(TyCtxt<'tcx>)) {
    let directory = TestTempDir::create("fe2o3-closure-flow");
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
        "--crate-name=fe2o3_closure_flow".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zmir-opt-level=0".into(),
        "-Coverflow-checks=off".into(),
        "-Cpanic=abort".into(),
        "--sysroot".into(),
        sysroot.trim().into(),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = CheckCallbacks(check, false);
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.1);
}

#[test]
fn real_generic_forwarding_preserves_local_origins_and_every_shim_body() {
    with_source(|tcx| {
        for name in ["by_value", "by_reference", "mutable", "once", "repeated"] {
            let (collection, flow) = collect(tcx, name);
            let observations = observe_v1(
                tcx,
                &collection.functions,
                &flow.graph,
                &mut SourceClosureWorkV1::default(),
            )
            .unwrap();
            let environments = observations.iter().flatten().flat_map(|a| a.environments());
            assert!(environments.clone().count() >= 3, "{name}");
            assert!(
                environments
                    .clone()
                    .all(|e| e.origin == ClosureOriginV1::DeviceInternal),
                "{name}"
            );
            let shims = collection
                .functions
                .iter()
                .filter_map(|f| {
                    authenticate_once_shim_v1(tcx, f.instance)
                        .unwrap()
                        .map(|body| (f.instance, body))
                })
                .collect::<Vec<_>>();
            assert_eq!(shims.len(), usize::from(name != "once"), "{name}");
            if name == "repeated" {
                let repeat = collection
                    .functions
                    .iter()
                    .find(|function| tcx.item_name(function.instance.def_id()).as_str() == "repeat")
                    .unwrap();
                let sites = flow.graph[&stable_instance_identity(tcx, repeat.instance)]
                    .values()
                    .collect::<Vec<_>>();
                assert_eq!(sites.len(), 1);
                assert_eq!(
                    sites[0].len(),
                    2,
                    "callee deduplication must retain both calls"
                );
            }
            for (shim, body) in shims {
                assert!(collection.functions.iter().any(|f| f.instance == body));
                assert!(
                    flow.graph[&stable_instance_identity(tcx, shim)]
                        .contains_key(&stable_instance_identity(tcx, body))
                );
            }
            flow.revalidate_for_import_v1(tcx, &collection).unwrap();
        }
    });
}

#[test]
fn external_and_mixed_formal_origins_keep_host_reference_restrictions() {
    with_source(|tcx| {
        let (mut collection, flow) = collect(tcx, "by_reference");
        let formal = collection
            .functions
            .iter_mut()
            .find(|f| tcx.item_name(f.instance.def_id()).as_str() == "hop")
            .unwrap();
        formal.role = CollectedFunctionRole::DeviceFfiExport;
        let error = observe_v1(
            tcx,
            &collection.functions,
            &flow.graph,
            &mut SourceClosureWorkV1::default(),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("host closure references"),
            "{error}"
        );
        assert!(
            flow.revalidate_for_import_v1(tcx, &collection)
                .unwrap_err()
                .to_string()
                .contains("role roster")
        );
    });
}

#[test]
fn import_rejects_stale_sites_observations_bodies_and_missing_helpers() {
    with_source(|tcx| {
        let (collection, mut flow) = collect(tcx, "by_value");
        flow.graph
            .values_mut()
            .next()
            .unwrap()
            .values_mut()
            .next()
            .unwrap()
            .clear();
        assert!(
            flow.revalidate_for_import_v1(tcx, &collection)
                .unwrap_err()
                .to_string()
                .contains("occurrences changed")
        );
        let (mut collection, flow) = collect(tcx, "by_value");
        collection
            .functions
            .iter_mut()
            .find(|f| f.closure_observation.is_some())
            .unwrap()
            .closure_observation = None;
        assert!(
            flow.revalidate_for_import_v1(tcx, &collection)
                .unwrap_err()
                .to_string()
                .contains("observation changed")
        );
        let (collection, mut flow) = collect(tcx, "no_closures");
        flow.bodies[0][0] ^= 1;
        assert!(
            flow.revalidate_for_import_v1(tcx, &collection)
                .unwrap_err()
                .to_string()
                .contains("MIR changed")
        );
        let (mut collection, mut flow) = collect(tcx, "no_closures");
        collection
            .functions
            .retain(|f| f.role == CollectedFunctionRole::KernelEntry);
        flow.functions = collection
            .functions
            .iter()
            .map(|f| (f.instance, f.role))
            .collect();
        flow.bodies = collection
            .functions
            .iter()
            .map(|f| rustc_mir_body_sha256_v1(tcx, f.instance))
            .collect();
        flow.graph.clear();
        assert!(
            flow.revalidate_for_import_v1(tcx, &collection)
                .unwrap_err()
                .to_string()
                .contains("uncollected non-boundary")
        );
    });
}

#[test]
fn generated_shims_do_not_hide_unsafe_closure_source_or_helpers() {
    with_source(|tcx| {
        for (name, expected) in [
            ("unsafe_body", "unsafe block"),
            ("unsafe_reachable", "unsafe function"),
        ] {
            let error = match collector(tcx, local(tcx, name)).collect() {
                Ok(_) => panic!("unsafe closure source admitted: {name}"),
                Err(error) => error.to_string(),
            };
            assert!(error.contains(expected), "{name}: {error}");
            assert!(error.contains("closure"), "{error}");
            assert!(error.contains("call_once"), "{error}");
        }
    });
}

#[test]
fn complete_once_shim_identity_is_required() {
    with_source(|tcx| {
        let (collection, _) = collect(tcx, "by_value");
        let shim = collection
            .functions
            .iter()
            .map(|f| f.instance)
            .find(|i| matches!(i.def, InstanceKind::ClosureOnceShim { .. }))
            .unwrap();
        let body = authenticate_once_shim_v1(tcx, shim).unwrap().unwrap();
        assert_eq!(
            Instance::resolve_closure(tcx, body.def_id(), body.args, ClosureKind::FnOnce),
            shim
        );
        let InstanceKind::ClosureOnceShim { call_once, .. } = shim.def else {
            unreachable!()
        };
        let mut changed = shim;
        changed.def = InstanceKind::ClosureOnceShim {
            call_once,
            track_caller: true,
        };
        assert!(authenticate_once_shim_v1(tcx, changed).is_err());
        changed.def = InstanceKind::ClosureOnceShim {
            call_once: body.def_id(),
            track_caller: false,
        };
        assert!(authenticate_once_shim_v1(tcx, changed).is_err());
        changed = shim;
        changed.args = tcx.mk_args(&[shim.args[0], tcx.types.unit.into()]);
        assert!(authenticate_once_shim_v1(tcx, changed).is_err());
    });
}

#[test]
fn eight_environments_allow_aliases_but_nine_remain_rejected() {
    with_source(|tcx| {
        let body = tcx.instance_mir(local(tcx, "eight").def);
        let aggregate_count = body
            .basic_blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter(|statement| {
                matches!(statement.kind.as_assign(),
                Some((_, rustc_middle::mir::Rvalue::Aggregate(kind, _)))
                    if matches!(kind.as_ref(), rustc_middle::mir::AggregateKind::Closure(..)))
            })
            .count();
        assert_eq!(aggregate_count, 8);
        assert!(
            body.local_decls
                .iter()
                .filter(|local| matches!(local.ty.kind(), TyKind::Closure(..)))
                .count()
                > 8
        );
        let admission = observe_closures_v2(tcx, local(tcx, "eight"))
            .unwrap()
            .unwrap();
        assert_eq!(admission.environments().len(), 8);
        assert_eq!(admission.calls().len(), 8);
        let error = observe_closures_v2(tcx, local(tcx, "nine"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("closure count exceeds 8"), "{error}");
    });
}

#[test]
fn every_recorded_argument_requires_one_binding_even_with_another_local_caller() {
    let recorded = BTreeMap::from([(4, BTreeSet::from([0])), (9, BTreeSet::from([1]))]);
    let run = |consumed: &[(usize, usize)]| {
        consume_arguments_v1(
            &mut recorded.clone(),
            consumed.iter().copied(),
            &mut SourceClosureWorkV1::default(),
        )
    };
    run(&[(4, 0), (9, 1)]).unwrap();
    assert!(
        run(&[(4, 0)])
            .unwrap_err()
            .to_string()
            .contains("no validated provenance")
    );
    assert!(run(&[]).is_err());
    assert!(run(&[(4, 0), (4, 0), (9, 1)]).is_err());
    assert!(run(&[(4, 0), (9, 0)]).is_err());
}

#[test]
fn a_projected_external_input_cannot_inherit_another_callers_local_origin() {
    with_source(|tcx| {
        let (collection, _) = collect(tcx, "local");
        let callee = collection
            .functions
            .iter()
            .find(|function| tcx.item_name(function.instance.def_id()).as_str() == "call_shared")
            .unwrap()
            .instance;
        let projected = tcx
            .hir_body_owners()
            .find(|id| {
                tcx.def_kind(id.to_def_id()) == DefKind::Fn
                    && tcx.item_name(id.to_def_id()).as_str() == "projected"
            })
            .unwrap();
        let projected = Instance::new_raw(projected.to_def_id(), callee.args);
        assert!(tcx.instance_mir(projected.def).basic_blocks.iter().any(
            |block| matches!(&block.terminator().kind, TerminatorKind::Call { func, .. }
                if resolve_direct_call(tcx, projected, func).unwrap() == callee)
        ));
        let mut collector = collector(tcx, local(tcx, "local"));
        collector
            .add_root(KernelRoot {
                target: projected,
                logical_name: "projected_root".into(),
                export_name: "projected_root".into(),
                generated_host_contract_identity: None,
                kernel_binding: None,
                frontend_contract: None,
                kernel_context_contract: None,
                reference_effect_binding: None,
            })
            .unwrap();
        let error = match collector.collect() {
            Ok(_) => panic!("projected external closure was admitted"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("bounded closure"), "{error}");
    });
}

#[test]
fn origin_propagation_rejects_cycles_unresolved_inputs_and_invalid_edges() {
    let run = |origins: &mut [u8], edges: &[Vec<usize>]| {
        propagate_origins_v1(origins, edges, &mut SourceClosureWorkV1::default())
    };
    let mut origins = [LOCAL, EXTERNAL, 0, 0];
    run(&mut origins, &[vec![2], vec![2], vec![3], vec![]]).unwrap();
    assert_eq!(
        origins,
        [LOCAL, EXTERNAL, LOCAL | EXTERNAL, LOCAL | EXTERNAL]
    );
    assert!(run(&mut [LOCAL, 0], &[vec![1], vec![0]]).is_err());
    assert!(run(&mut [LOCAL, 0], &[vec![], vec![]]).is_err());
    assert!(run(&mut [LOCAL], &[vec![1]]).is_err());
    assert!(run(&mut [4], &[vec![]]).is_err());
}
