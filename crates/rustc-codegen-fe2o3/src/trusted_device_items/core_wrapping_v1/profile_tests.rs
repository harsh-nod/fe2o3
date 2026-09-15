//! Profile controls and bounded MIR diagnostics for tests only. Cached core
//! metadata is never rebuilt or replaced with a caller-authored helper body.

use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Profile {
    HostSysroot,
    HostCached,
    AmdgpuCached,
}

pub(crate) fn append_args(args: &mut Vec<String>, profile: Profile, mir_opt: Option<u8>) {
    args.extend(["-Cpanic=abort".into(), "-Zalways-encode-mir".into()]);
    if let Some(level) = mir_opt {
        assert_eq!(
            level, 0,
            "only the additional unoptimized profile is reviewed here"
        );
        args.push(format!("-Zmir-opt-level={level}"));
    }
    let names = match profile {
        Profile::HostSysroot => return,
        Profile::HostCached => ["FE2O3_WRAPPING_HOST_CORE", "FE2O3_WRAPPING_HOST_BUILTINS"],
        Profile::AmdgpuCached => [
            "FE2O3_WRAPPING_AMDGPU_CORE",
            "FE2O3_WRAPPING_AMDGPU_BUILTINS",
        ],
    };
    let metadata = names.map(|name| {
        let path = std::path::PathBuf::from(std::env::var_os(name).unwrap_or_else(|| {
            panic!(
                "set {name} to existing pinned metadata; no sysroot build or fallback is permitted"
            )
        }));
        assert!(path.is_file(), "{name}: {}", path.display());
        path
    });
    if profile == Profile::AmdgpuCached {
        let cpu = std::env::var("FE2O3_WRAPPING_TARGET_CPU").unwrap_or_else(|_| "gfx942".into());
        assert!(
            matches!(cpu.as_str(), "gfx942" | "gfx950"),
            "unsupported test CPU"
        );
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            format!("-Ctarget-cpu={cpu}"),
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
        ]);
    }
    args.push("-Zunstable-options".into());
    for (name, path) in ["core", "compiler_builtins"].into_iter().zip(&metadata) {
        args.extend([
            "--extern".into(),
            format!("noprelude,nounused:{name}={}", path.display()),
            "-L".into(),
            format!("dependency={}", path.parent().unwrap().display()),
        ]);
    }
}

fn resolve<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> Option<Instance<'tcx>> {
    let Operand::Constant(constant) = operand else {
        return None;
    };
    let TyKind::FnDef(definition, args) = constant.const_.ty().kind() else {
        return None;
    };
    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *definition, args)
        .ok()
        .flatten()
}

pub(crate) fn called<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .expect("profile fixture caller");
    tcx.optimized_mir(definition)
        .basic_blocks
        .iter()
        .find_map(|block| {
            let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                return None;
            };
            resolve(tcx, func)
        })
        .expect("retained wrapping call")
}

pub(crate) fn require_proof<'tcx>(tcx: TyCtxt<'tcx>, root: Instance<'tcx>) {
    let accepted = prove_core_u32_wrapping_shr_v1(tcx, root).is_some();
    eprintln!(
        "wrapping profile target={} panic={:?} mir_opt={:?} proof={accepted}",
        tcx.sess.target.llvm_target,
        tcx.sess.panic_strategy(),
        tcx.sess.opts.unstable_opts.mir_opt_level
    );
    if !accepted || std::env::var_os("FE2O3_WRAPPING_DUMP_MIR").is_some() {
        dump_graph(tcx, root);
    }
    assert!(
        accepted,
        "cached/source profile is not authenticated; see bounded MIR above"
    );
}

fn dump_graph<'tcx>(tcx: TyCtxt<'tcx>, root: Instance<'tcx>) {
    let core = tcx.lang_items().sized_trait().unwrap().krate;
    let mut pending = std::collections::VecDeque::from([(root, 0)]);
    let mut seen = Vec::new();
    while let Some((instance, depth)) = pending.pop_front() {
        if seen.contains(&instance) {
            continue;
        }
        if seen.len() == 16 {
            eprintln!("MIR body diagnostic limit reached");
            break;
        }
        seen.push(instance);
        let body = tcx.instance_mir(instance.def);
        super::wrapping_shr::retained_tests::diagnose(tcx, instance);
        eprintln!(
            "MIR {instance:?}: source={:?} phase={:?} args={} blocks={} locals={} scopes={} spread={:?} coroutine={} required_consts={:?}",
            body.source,
            body.phase,
            body.arg_count,
            body.basic_blocks.len(),
            body.local_decls.len(),
            body.source_scopes.len(),
            body.spread_arg,
            body.coroutine.is_some(),
            body.required_consts
        );
        for (local, decl) in body.local_decls.iter_enumerated().take(32) {
            eprintln!("  {local:?}: {:?}", decl.ty);
        }
        let mut callees = Vec::new();
        for (scope, data) in body.source_scopes.iter_enumerated().take(16) {
            eprintln!(
                "  {scope:?}: parent={:?} inlined={:?} inlined_parent={:?}",
                data.parent_scope, data.inlined, data.inlined_parent_scope
            );
            if let Some((callee, _)) = data.inlined {
                callees.push(callee);
            }
        }
        for (block, data) in body.basic_blocks.iter_enumerated().take(16) {
            eprintln!("  {block:?}: cleanup={}", data.is_cleanup);
            for statement in data.statements.iter().take(64) {
                eprintln!("    {statement:?}");
            }
            eprintln!("    {:?}", data.terminator);
            if let Some(terminator) = &data.terminator
                && let TerminatorKind::Assert { msg, .. } = &terminator.kind
                && let rustc_middle::mir::AssertMessage::Overflow(operation, left, right) = &**msg
            {
                eprintln!("    assertion operands: {operation:?}, lhs={left:?}, rhs={right:?}");
            }
            if let Some(terminator) = &data.terminator
                && let TerminatorKind::Call { func, .. } = &terminator.kind
                && let Some(callee) = resolve(tcx, func)
            {
                callees.push(callee);
            }
        }
        if depth < 6 {
            for callee in callees {
                if pending.len() < 32
                    && matches!(callee.def, InstanceKind::Item(_))
                    && callee.def_id().krate == core
                    && tcx.is_mir_available(callee.def_id())
                    && !tcx
                        .fn_sig(callee.def_id())
                        .instantiate(tcx, callee.args)
                        .skip_binder()
                        .output()
                        .is_never()
                {
                    pending.push_back((callee, depth + 1));
                }
            }
        }
    }
}

struct Probe {
    ran: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("wrapping_profile.rs".into()),
            input: "#![no_std]\npub fn route(a: u32, b: u32) -> u32 { a.wrapping_shr(b) }".into(),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let root = called(tcx, "route");
        require_proof(tcx, root);
        if tcx.instance_mir(root.def).source_scopes.len() == 1 {
            super::wrapping_shr::retained_tests::actual_profile(tcx, root);
        }
        let selected = super::super::production_mir_v1::production_mir_v1(tcx, root);
        assert_eq!(selected.instance(), root);
        assert!(selected.is_source_expansion());
        assert!(selected.expansion_fingerprint(tcx).is_some());
        assert_eq!(selected.body().source.instance, root.def);
        assert_eq!(selected.body().basic_blocks.len(), 1);
        assert_eq!(selected.body().local_decls.len(), 4);
        let operations = selected
            .body()
            .basic_blocks
            .iter()
            .flat_map(|block| &block.statements)
            .map(|statement| match &statement.kind {
                StatementKind::Assign(assignment) => match &assignment.1 {
                    Rvalue::BinaryOp(op, _) => *op,
                    _ => panic!("exact expanded binary operation"),
                },
                _ => panic!("exact expanded assignment"),
            })
            .collect::<Vec<_>>();
        assert_eq!(operations, [BinOp::BitAnd, BinOp::Shr]);
        self.ran = true;
        Compilation::Stop
    }
}

fn run_profile(mir_opt: Option<u8>) {
    let profile = match std::env::var("FE2O3_WRAPPING_PROFILE").as_deref() {
        Ok("host-cached") => Profile::HostCached,
        Ok("amdgpu-cached") => Profile::AmdgpuCached,
        Ok("host-sysroot") | Err(std::env::VarError::NotPresent) => Profile::HostSysroot,
        other => panic!("unknown wrapping test profile: {other:?}"),
    };
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=wrapping_profile".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout.clone())
            .unwrap()
            .trim()
            .into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
    ];
    append_args(&mut args, profile, mir_opt);
    args.push("-".into());
    eprintln!("wrapping profile arguments: {args:?}");
    let mut probe = Probe { ran: false };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn core_wrapping_shr_selected_metadata_profile_default() {
    run_profile(None);
}

#[test]
fn core_wrapping_shr_selected_metadata_profile_mir_opt_zero() {
    run_profile(Some(0));
}
