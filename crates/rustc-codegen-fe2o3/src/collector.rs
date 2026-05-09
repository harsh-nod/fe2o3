use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_middle::mir::mono::{CodegenUnit, MonoItem};
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_middle::ty::{
    EarlyBinder, Instance, InstanceKind, TyCtxt, TyKind, TypeVisitableExt, TypingEnv,
};
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Debug)]
pub struct CollectedFunction<'tcx> {
    pub instance: Instance<'tcx>,
    pub is_kernel: bool,
    pub export_name: String,
}

#[derive(Clone, Debug, Default)]
pub struct CollectionResult<'tcx> {
    pub functions: Vec<CollectedFunction<'tcx>>,
}

#[derive(Debug)]
enum CollectDecision {
    Collect,
    SkipIntentional,
    Forbidden { crate_name: String, fn_path: String },
}

pub fn count_kernels_in_cgus<'tcx>(tcx: TyCtxt<'tcx>, cgus: &[CodegenUnit<'tcx>]) -> usize {
    kernel_roots(tcx, cgus).len()
}

pub fn collect_device_functions<'tcx>(
    tcx: TyCtxt<'tcx>,
    cgus: &[CodegenUnit<'tcx>],
    verbose: bool,
) -> CollectionResult<'tcx> {
    let mut collector = DeviceCollector::new(tcx, verbose);

    for instance in kernel_roots(tcx, cgus) {
        let raw_name = tcx.def_path_str(instance.def_id());
        let export_name = kernel_export_name(&raw_name);
        if verbose {
            eprintln!("[collector] root kernel: {raw_name} -> {export_name}");
        }
        collector.add_root(instance, export_name);
    }

    collector.collect()
}

pub fn dump_device_functions<'tcx>(tcx: TyCtxt<'tcx>, functions: &[CollectedFunction<'tcx>]) {
    let mut rows = functions
        .iter()
        .map(|function| {
            let def_id = function.instance.def_id();
            let mir_stats = if tcx.is_mir_available(def_id) {
                let mir = tcx.instance_mir(function.instance.def);
                format!(
                    "{} bb, {} locals, {} args",
                    mir.basic_blocks.len(),
                    mir.local_decls.len(),
                    mir.arg_count
                )
            } else {
                "no MIR".to_string()
            };
            (
                function.export_name.clone(),
                if function.is_kernel {
                    "kernel"
                } else {
                    "device"
                },
                tcx.crate_name(def_id.krate).to_string(),
                tcx.def_path_str(def_id),
                mir_stats,
            )
        })
        .collect::<Vec<_>>();

    rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.2.cmp(&b.2)));

    eprintln!("\n=== fe2o3 device function collection ===");
    for (export_name, kind, crate_name, path, mir_stats) in rows {
        eprintln!("  [{kind}] {export_name}");
        eprintln!("      crate: {crate_name}");
        eprintln!("      path: {path}");
        eprintln!("      MIR:  {mir_stats}");
    }
    eprintln!("========================================\n");
}

fn kernel_roots<'tcx>(tcx: TyCtxt<'tcx>, cgus: &[CodegenUnit<'tcx>]) -> Vec<Instance<'tcx>> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    for cgu in cgus {
        for (item, _data) in cgu.items() {
            let MonoItem::Fn(instance) = item else {
                continue;
            };

            let name = tcx.def_path_str(instance.def_id());
            if !is_kernel_symbol(&name) {
                continue;
            }
            if name.contains("{closure") || name.contains("::closure") {
                continue;
            }
            if !is_fully_monomorphized(tcx, *instance) {
                continue;
            }

            let symbol = tcx.symbol_name(*instance).name.to_string();
            if seen.insert(symbol) {
                roots.push(*instance);
            }
        }
    }

    roots.sort_by_key(|instance| tcx.def_path_str(instance.def_id()));
    roots
}

fn is_kernel_symbol(name: &str) -> bool {
    name.contains(reserved_fe2o3_symbols::KERNEL_PREFIX)
}

fn kernel_export_name(name: &str) -> String {
    let local = name.rsplit("::").next().unwrap_or(name);
    local
        .strip_prefix(reserved_fe2o3_symbols::KERNEL_PREFIX)
        .unwrap_or(local)
        .to_string()
}

fn is_fully_monomorphized<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    let generics = tcx.generics_of(instance.def_id());

    for arg in instance.args.iter() {
        if let Some(ty) = arg.as_type()
            && ty.has_param()
        {
            return false;
        }
    }

    generics.count() == 0 || !instance.args.is_empty()
}

struct DeviceCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    seen: HashSet<String>,
    used_export_names: HashSet<String>,
    worklist: VecDeque<CollectedFunction<'tcx>>,
    result: Vec<CollectedFunction<'tcx>>,
    verbose: bool,
}

impl<'tcx> DeviceCollector<'tcx> {
    fn new(tcx: TyCtxt<'tcx>, verbose: bool) -> Self {
        Self {
            tcx,
            seen: HashSet::new(),
            used_export_names: HashSet::new(),
            worklist: VecDeque::new(),
            result: Vec::new(),
            verbose,
        }
    }

    fn add_root(&mut self, instance: Instance<'tcx>, export_name: String) {
        let symbol = self.tcx.symbol_name(instance).name.to_string();
        if self.seen.insert(symbol) {
            self.used_export_names.insert(export_name.clone());
            self.worklist.push_back(CollectedFunction {
                instance,
                is_kernel: true,
                export_name,
            });
        }
    }

    fn collect(mut self) -> CollectionResult<'tcx> {
        while let Some(function) = self.worklist.pop_front() {
            let def_id = function.instance.def_id();

            if self.tcx.is_mir_available(def_id) {
                let mir = self.tcx.instance_mir(function.instance.def);
                if self.verbose {
                    eprintln!(
                        "[collector] visiting {} ({} basic blocks)",
                        function.export_name,
                        mir.basic_blocks.len()
                    );
                }

                for block in mir.basic_blocks.iter() {
                    if let Some(terminator) = &block.terminator
                        && let TerminatorKind::Call { func, .. } = &terminator.kind
                    {
                        self.process_call_operand(func, &function.instance);
                    }
                }
            }

            self.result.push(function);
        }

        CollectionResult {
            functions: self.result,
        }
    }

    fn process_call_operand(&mut self, func: &Operand<'tcx>, caller: &Instance<'tcx>) {
        let Operand::Constant(const_op) = func else {
            return;
        };

        let ty = const_op.const_.ty();
        let TyKind::FnDef(def_id, args) = ty.kind() else {
            return;
        };

        match self.should_collect_from_crate(*def_id) {
            CollectDecision::Collect => {}
            CollectDecision::SkipIntentional => return,
            CollectDecision::Forbidden {
                crate_name,
                fn_path,
            } => {
                panic!(
                    "\nfe2o3 device code reached forbidden crate `{crate_name}` via `{fn_path}`.\n\
                     Device-reachable functions must avoid `std`; use `core`, `alloc` only when a device allocator exists, or no_std helper crates."
                );
            }
        }

        let args = self.tcx.instantiate_and_normalize_erasing_regions(
            caller.args,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(*args),
        );

        let Some(resolved) =
            Instance::try_resolve(self.tcx, TypingEnv::fully_monomorphized(), *def_id, args)
                .ok()
                .flatten()
        else {
            return;
        };

        let symbol = self.tcx.symbol_name(resolved).name.to_string();
        if self.seen.contains(&symbol) {
            return;
        }

        if !is_fully_monomorphized(self.tcx, resolved) {
            return;
        }

        if !matches!(resolved.def, InstanceKind::Item(_)) {
            return;
        }

        if !self.tcx.is_mir_available(resolved.def_id()) {
            if self.verbose {
                eprintln!(
                    "[collector] skipping no-MIR callee {}",
                    self.tcx.def_path_str(resolved.def_id())
                );
            }
            return;
        }

        if self.is_unreachable_body(resolved.def_id()) {
            if self.verbose {
                eprintln!(
                    "[collector] skipping intrinsic stub {}",
                    self.tcx.def_path_str(resolved.def_id())
                );
            }
            return;
        }

        let name = self.fqdn(resolved.def_id());
        let export_name = self.compute_export_name(&name, resolved);

        if self.verbose {
            eprintln!("[collector] callee: {name} -> {export_name}");
        }

        self.seen.insert(symbol);
        self.worklist.push_back(CollectedFunction {
            instance: resolved,
            is_kernel: false,
            export_name,
        });
    }

    fn should_collect_from_crate(&self, def_id: DefId) -> CollectDecision {
        if def_id.krate == LOCAL_CRATE {
            return CollectDecision::Collect;
        }

        let crate_name = self.tcx.crate_name(def_id.krate);
        let crate_name = crate_name.as_str();
        let path = self.tcx.def_path_str(def_id);

        if path.contains(reserved_fe2o3_symbols::KERNEL_PREFIX) {
            return CollectDecision::Collect;
        }

        if crate_name == "std" {
            return CollectDecision::Forbidden {
                crate_name: crate_name.to_string(),
                fn_path: path,
            };
        }

        if path.contains("::fmt::")
            || path.contains("::panicking::")
            || path.contains("precondition_check")
        {
            return CollectDecision::SkipIntentional;
        }

        CollectDecision::Collect
    }

    fn fqdn(&self, def_id: DefId) -> String {
        let path = self.tcx.def_path_str(def_id);
        if def_id.krate == LOCAL_CRATE {
            format!("{}::{}", self.tcx.crate_name(LOCAL_CRATE), path)
        } else {
            path
        }
    }

    fn compute_export_name(&mut self, name: &str, instance: Instance<'tcx>) -> String {
        let has_generic_args = !instance.args.is_empty();
        let has_invalid_chars = name
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == ':'));

        let simple = name.replace("::", "__");
        if has_generic_args || has_invalid_chars || self.used_export_names.contains(&simple) {
            let symbol = self.tcx.symbol_name(instance).name.to_string();
            let sanitized = sanitize_symbol_name(&symbol);
            self.used_export_names.insert(sanitized.clone());
            sanitized
        } else {
            self.used_export_names.insert(simple.clone());
            simple
        }
    }

    fn is_unreachable_body(&self, def_id: DefId) -> bool {
        if !self.tcx.is_mir_available(def_id) {
            return false;
        }

        let mir = self.tcx.optimized_mir(def_id);
        if mir.basic_blocks.len() > 2 {
            return false;
        }

        for block in mir.basic_blocks.iter() {
            let Some(terminator) = &block.terminator else {
                continue;
            };
            match &terminator.kind {
                TerminatorKind::Call { func, .. } => {
                    if let Some(callee) = self.call_def_id(func) {
                        let path = self.tcx.def_path_str(callee);
                        if path.contains("::panicking::") || path.contains("::rt::panic") {
                            return true;
                        }
                    }
                }
                TerminatorKind::Unreachable => {}
                _ => return false,
            }
        }

        false
    }

    fn call_def_id(&self, func: &Operand<'tcx>) -> Option<DefId> {
        let Operand::Constant(const_op) = func else {
            return None;
        };
        let ty = const_op.const_.ty();
        if let TyKind::FnDef(def_id, _) = ty.kind() {
            Some(*def_id)
        } else {
            None
        }
    }
}

fn sanitize_symbol_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
