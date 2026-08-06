use rustc_hir::ItemKind;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_middle::middle::codegen_fn_attrs::CodegenFnAttrFlags;
use rustc_middle::mir::mono::{CodegenUnit, MonoItem};
use rustc_middle::mir::{AggregateKind, CastKind, Operand, RETURN_PLACE, Rvalue, TerminatorKind};
use rustc_middle::ty::adjustment::PointerCoercion;
use rustc_middle::ty::{
    EarlyBinder, Instance, InstanceKind, TyCtxt, TyKind, TypeVisitableExt, TypingEnv,
};
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypedKernelProfile {
    VecAddV1,
}

#[derive(Clone, Debug)]
pub struct CollectedFunction<'tcx> {
    pub instance: Instance<'tcx>,
    pub is_kernel: bool,
    pub export_name: String,
    /// Present only for registered kernel roots.
    pub(crate) logical_name: Option<String>,
    /// Present only when the registration selects a versioned typed profile.
    pub(crate) typed_profile: Option<TypedKernelProfile>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectError {
    message: String,
}

impl fmt::Display for CollectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CollectError {}

pub fn count_kernels_in_cgus<'tcx>(tcx: TyCtxt<'tcx>, _cgus: &[CodegenUnit<'tcx>]) -> usize {
    registration_candidates(tcx).len()
}

pub fn collect_device_functions<'tcx>(
    tcx: TyCtxt<'tcx>,
    cgus: &[CodegenUnit<'tcx>],
    verbose: bool,
) -> Result<CollectionResult<'tcx>, CollectError> {
    let mut collector = DeviceCollector::new(tcx, verbose);

    for root in kernel_roots(tcx, cgus).map_err(CollectError::from)? {
        let instance = root.target;
        let raw_name = tcx.def_path_str(instance.def_id());
        let logical_name = root.logical_name;
        let export_name = root.export_name;
        if verbose {
            eprintln!("[collector] root kernel: {raw_name} -> {export_name}");
        }
        collector.add_root(instance, logical_name, export_name, root.typed_profile);
    }

    collector.collect()
}

pub fn dump_device_functions<'tcx>(tcx: TyCtxt<'tcx>, functions: &[CollectedFunction<'tcx>]) {
    let mut rows = functions
        .iter()
        .map(|function| {
            let def_id = function.instance.def_id();
            debug_assert_eq!(function.is_kernel, function.logical_name.is_some());
            debug_assert!(function.is_kernel || function.typed_profile.is_none());
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
                match function.typed_profile {
                    Some(TypedKernelProfile::VecAddV1) => "kernel/typed-vecadd-v1",
                    None if function.is_kernel => "kernel",
                    None => "device",
                },
                function.logical_name.clone(),
                tcx.crate_name(def_id.krate).to_string(),
                tcx.def_path_str(def_id),
                mir_stats,
            )
        })
        .collect::<Vec<_>>();

    rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.3.cmp(&b.3)));

    eprintln!("\n=== fe2o3 device function collection ===");
    for (export_name, kind, logical_name, crate_name, path, mir_stats) in rows {
        eprintln!("  [{kind}] {export_name}");
        if let Some(logical_name) = logical_name.filter(|name| name != &export_name) {
            eprintln!("      logical name: {logical_name}");
        }
        eprintln!("      crate: {crate_name}");
        eprintln!("      path: {path}");
        eprintln!("      MIR:  {mir_stats}");
    }
    eprintln!("========================================\n");
}

#[derive(Clone, Debug)]
struct RegistrationRecord<T> {
    registration_path: String,
    item_name: String,
    magic: u64,
    version: u16,
    kind: u16,
    logical_name: String,
    export_name: String,
    target_symbol: String,
    target_identity: String,
    target: T,
}

#[derive(Clone, Debug)]
struct KernelRoot<T> {
    target: T,
    logical_name: String,
    export_name: String,
    typed_profile: Option<TypedKernelProfile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RegistrationError {
    registration_path: String,
    reason: String,
}

impl RegistrationError {
    fn new(registration_path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            registration_path: registration_path.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid fe2o3 kernel registration `{}`: {}",
            self.registration_path, self.reason
        )
    }
}

impl From<RegistrationError> for CollectError {
    fn from(error: RegistrationError) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

fn kernel_roots<'tcx>(
    tcx: TyCtxt<'tcx>,
    cgus: &[CodegenUnit<'tcx>],
) -> Result<Vec<KernelRoot<Instance<'tcx>>>, RegistrationError> {
    let mut functions_by_symbol = BTreeMap::new();

    for cgu in cgus {
        for (item, _data) in cgu.items() {
            let MonoItem::Fn(instance) = item else {
                continue;
            };
            if !is_fully_monomorphized(tcx, *instance) {
                continue;
            }

            let symbol = tcx.symbol_name(*instance).name.to_string();
            functions_by_symbol
                .entry(symbol)
                .or_insert_with(Vec::new)
                .push(*instance);
        }
    }
    for instances in functions_by_symbol.values_mut() {
        instances.sort_by_key(|instance| tcx.def_path_str(instance.def_id()));
    }

    let mut candidates = registration_candidates(tcx);
    candidates.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));

    let mut records = Vec::with_capacity(candidates.len());
    for (path, item_name, def_id, item) in candidates {
        if !matches!(item.kind, ItemKind::Static(..)) {
            return Err(RegistrationError::new(
                path,
                "the reserved registration name must identify a static item",
            ));
        }
        if tcx.is_mutable_static(def_id.to_def_id()) {
            return Err(RegistrationError::new(
                path,
                "registration statics must be immutable",
            ));
        }

        let flags = tcx.codegen_fn_attrs(def_id).flags;
        if !flags.intersects(CodegenFnAttrFlags::USED_COMPILER | CodegenFnAttrFlags::USED_LINKER) {
            return Err(RegistrationError::new(
                path,
                "registration statics must carry #[used]",
            ));
        }

        records.push(decode_registration_static(
            tcx,
            def_id,
            path,
            item_name,
            &functions_by_symbol,
        )?);
    }

    validate_registration_records(records)
}

fn registration_candidates<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Vec<(
    String,
    String,
    rustc_hir::def_id::LocalDefId,
    &'tcx rustc_hir::Item<'tcx>,
)> {
    tcx.hir_free_items()
        .filter_map(|item_id| {
            let item = tcx.hir_item(item_id);
            let def_id = item.owner_id.def_id;
            let path = tcx.def_path_str(def_id.to_def_id());
            let item_name = final_path_segment(&path).to_string();
            item_name
                .starts_with(reserved_fe2o3_symbols::KERNEL_REGISTRATION_PREFIX)
                .then_some((path, item_name, def_id, item))
        })
        .collect()
}

fn decode_registration_static<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: rustc_hir::def_id::LocalDefId,
    registration_path: String,
    item_name: String,
    functions_by_symbol: &BTreeMap<String, Vec<Instance<'tcx>>>,
) -> Result<RegistrationRecord<Instance<'tcx>>, RegistrationError> {
    let registration_ty = tcx.type_of(def_id).instantiate_identity();
    let TyKind::Tuple(fields) = registration_ty.kind() else {
        return Err(RegistrationError::new(
            registration_path,
            "V1 registration must use the exact tuple type",
        ));
    };

    if fields.len() != reserved_fe2o3_symbols::KERNEL_REGISTRATION_V1_FIELD_COUNT
        || fields[0] != tcx.types.u64
        || fields[1] != tcx.types.u16
        || fields[2] != tcx.types.u16
        || !is_shared_str(fields[3])
        || !is_shared_str(fields[4])
        || !matches!(fields[5].kind(), TyKind::FnPtr(..))
    {
        return Err(RegistrationError::new(
            registration_path,
            "V1 registration type must be `(u64, u16, u16, &str, &str, fn pointer)`",
        ));
    }

    let body = tcx.mir_for_ctfe(def_id);
    let mut aggregate = None;
    for block in body.basic_blocks.iter() {
        for statement in &block.statements {
            let Some((place, Rvalue::Aggregate(kind, fields))) = statement.kind.as_assign() else {
                continue;
            };
            if place.as_local() != Some(RETURN_PLACE) || !matches!(**kind, AggregateKind::Tuple) {
                continue;
            }
            if aggregate.replace(fields).is_some() {
                return Err(RegistrationError::new(
                    registration_path,
                    "V1 registration initializer must contain exactly one tuple value",
                ));
            }
        }
    }
    let fields = aggregate.ok_or_else(|| {
        RegistrationError::new(
            registration_path.clone(),
            "V1 registration initializer does not contain the required tuple value",
        )
    })?;
    if fields.len() != reserved_fe2o3_symbols::KERNEL_REGISTRATION_V1_FIELD_COUNT {
        return Err(RegistrationError::new(
            registration_path,
            "V1 registration initializer has the wrong field count",
        ));
    }
    let fields = fields.iter().collect::<Vec<_>>();

    let magic = registration_integer(tcx, fields[0], tcx.types.u64, "magic", &registration_path)?;
    let version =
        registration_integer(tcx, fields[1], tcx.types.u16, "version", &registration_path)?;
    let kind = registration_integer(tcx, fields[2], tcx.types.u16, "kind", &registration_path)?;
    let logical_name = registration_string(tcx, fields[3], "logical name", &registration_path)?;
    let export_name = registration_string(tcx, fields[4], "export name", &registration_path)?;
    let target = registration_target(tcx, body, fields[5], &registration_path)?;
    let target_symbol = tcx.symbol_name(target).name.to_string();
    let target_identity = tcx.def_path_str(target.def_id());
    let Some(cgu_targets) = functions_by_symbol.get(&target_symbol) else {
        return Err(RegistrationError::new(
            registration_path,
            format!(
                "registered target `{target_symbol}` was not monomorphized into a codegen unit"
            ),
        ));
    };
    if cgu_targets.len() != 1 {
        let paths = cgu_targets
            .iter()
            .map(|instance| tcx.def_path_str(instance.def_id()))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(RegistrationError::new(
            registration_path,
            format!("registered target symbol `{target_symbol}` is ambiguous across: {paths}"),
        ));
    }
    if cgu_targets[0] != target {
        return Err(RegistrationError::new(
            registration_path,
            format!("registered target `{target_symbol}` resolved inconsistently"),
        ));
    }
    let magic = u64::try_from(magic)
        .map_err(|_| RegistrationError::new(&registration_path, "magic does not fit u64"))?;
    let version = u16::try_from(version)
        .map_err(|_| RegistrationError::new(&registration_path, "version does not fit u16"))?;
    let kind = u16::try_from(kind)
        .map_err(|_| RegistrationError::new(&registration_path, "kind does not fit u16"))?;

    Ok(RegistrationRecord {
        registration_path,
        item_name,
        magic,
        version,
        kind,
        logical_name,
        export_name,
        target_symbol,
        target_identity,
        target,
    })
}

fn registration_integer<'tcx>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    expected_ty: rustc_middle::ty::Ty<'tcx>,
    field: &str,
    registration_path: &str,
) -> Result<u128, RegistrationError> {
    let Operand::Constant(constant) = operand else {
        return Err(RegistrationError::new(
            registration_path,
            format!("V1 {field} field must be a constant"),
        ));
    };
    if constant.const_.ty() != expected_ty {
        return Err(RegistrationError::new(
            registration_path,
            format!("V1 {field} field has the wrong type"),
        ));
    }
    constant
        .const_
        .try_eval_bits(tcx, TypingEnv::fully_monomorphized())
        .ok_or_else(|| {
            RegistrationError::new(
                registration_path,
                format!("V1 {field} field could not be evaluated"),
            )
        })
}

fn registration_string<'tcx>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    field: &str,
    registration_path: &str,
) -> Result<String, RegistrationError> {
    let Operand::Constant(constant) = operand else {
        return Err(RegistrationError::new(
            registration_path,
            format!("V1 {field} field must be a string constant"),
        ));
    };
    if !is_shared_str(constant.const_.ty()) {
        return Err(RegistrationError::new(
            registration_path,
            format!("V1 {field} field has the wrong type"),
        ));
    }
    let value = constant
        .const_
        .eval(tcx, TypingEnv::fully_monomorphized(), constant.span)
        .map_err(|_| {
            RegistrationError::new(
                registration_path,
                format!("V1 {field} field could not be evaluated"),
            )
        })?;
    let bytes = value
        .try_get_slice_bytes_for_diagnostics(tcx)
        .ok_or_else(|| {
            RegistrationError::new(
                registration_path,
                format!("V1 {field} field did not evaluate to string bytes"),
            )
        })?;
    std::str::from_utf8(bytes).map(str::to_owned).map_err(|_| {
        RegistrationError::new(registration_path, format!("V1 {field} field is not UTF-8"))
    })
}

fn registration_target<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &rustc_middle::mir::Body<'tcx>,
    operand: &Operand<'tcx>,
    registration_path: &str,
) -> Result<Instance<'tcx>, RegistrationError> {
    let place = match operand {
        Operand::Copy(place) | Operand::Move(place) => place,
        Operand::Constant(_) | Operand::RuntimeChecks(_) => {
            return Err(RegistrationError::new(
                registration_path,
                "V1 target field must directly use a reified function pointer",
            ));
        }
    };
    let Some(target_local) = place.as_local() else {
        return Err(RegistrationError::new(
            registration_path,
            "V1 target field must use an unprojected function-pointer local",
        ));
    };

    let mut target = None;
    for block in body.basic_blocks.iter() {
        for statement in &block.statements {
            let Some((place, Rvalue::Cast(cast, source, _))) = statement.kind.as_assign() else {
                continue;
            };
            if place.as_local() != Some(target_local)
                || !matches!(
                    cast,
                    CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer(_), _)
                )
            {
                continue;
            }
            let Operand::Constant(source) = source else {
                return Err(RegistrationError::new(
                    registration_path,
                    "V1 target coercion must directly name a function item",
                ));
            };
            let TyKind::FnDef(def_id, args) = source.const_.ty().kind() else {
                return Err(RegistrationError::new(
                    registration_path,
                    "V1 target coercion does not reference a function item",
                ));
            };
            let resolved =
                Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, args)
                    .ok()
                    .flatten()
                    .ok_or_else(|| {
                        RegistrationError::new(
                            registration_path,
                            "V1 target function could not be resolved",
                        )
                    })?;
            if target.replace(resolved).is_some() {
                return Err(RegistrationError::new(
                    registration_path,
                    "V1 target local has multiple function definitions",
                ));
            }
        }
    }

    target.ok_or_else(|| {
        RegistrationError::new(
            registration_path,
            "V1 target function association is missing",
        )
    })
}

fn is_shared_str(ty: rustc_middle::ty::Ty<'_>) -> bool {
    matches!(ty.kind(), TyKind::Ref(_, inner, mutability) if inner.is_str() && !mutability.is_mut())
}

fn validate_registration_records<T: Copy>(
    mut records: Vec<RegistrationRecord<T>>,
) -> Result<Vec<KernelRoot<T>>, RegistrationError> {
    records.sort_by(|lhs, rhs| lhs.registration_path.cmp(&rhs.registration_path));

    let mut logical_names = BTreeMap::new();
    let mut export_names = BTreeMap::new();
    let mut target_identities = BTreeMap::new();
    let mut roots = Vec::with_capacity(records.len());

    for record in records {
        let error = |reason| RegistrationError::new(record.registration_path.clone(), reason);
        if record.magic != reserved_fe2o3_symbols::KERNEL_REGISTRATION_MAGIC {
            return Err(error(format!(
                "magic {:#018x} does not match V1 magic {:#018x}",
                record.magic,
                reserved_fe2o3_symbols::KERNEL_REGISTRATION_MAGIC
            )));
        }
        if record.version != reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V1 {
            return Err(error(format!(
                "unknown registration version {}",
                record.version
            )));
        }
        let typed_profile = match record.kind {
            reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_KERNEL => None,
            reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1 => {
                Some(TypedKernelProfile::VecAddV1)
            }
            _ => return Err(error(format!("unknown registration kind {}", record.kind))),
        };
        if record.logical_name.is_empty() {
            return Err(error("logical name must not be empty".to_string()));
        }
        if record.export_name.is_empty() {
            return Err(error("export name must not be empty".to_string()));
        }

        let expected_item_name = format!(
            "{}{}",
            reserved_fe2o3_symbols::KERNEL_REGISTRATION_PREFIX,
            record.logical_name
        );
        if record.item_name != expected_item_name {
            return Err(error(format!(
                "item name `{}` is inconsistent with logical name `{}`",
                record.item_name, record.logical_name
            )));
        }

        let expected_target_symbol = format!(
            "{}{}",
            reserved_fe2o3_symbols::KERNEL_PREFIX,
            record.export_name
        );
        if record.target_symbol != expected_target_symbol {
            return Err(error(format!(
                "target symbol `{}` is inconsistent with export name `{}`",
                record.target_symbol, record.export_name
            )));
        }

        reject_duplicate(
            &mut logical_names,
            &record.logical_name,
            &record.registration_path,
            "logical name",
        )?;
        reject_duplicate(
            &mut export_names,
            &record.export_name,
            &record.registration_path,
            "export name",
        )?;
        reject_duplicate(
            &mut target_identities,
            &record.target_identity,
            &record.registration_path,
            "target identity",
        )?;

        roots.push(KernelRoot {
            target: record.target,
            logical_name: record.logical_name,
            export_name: record.export_name,
            typed_profile,
        });
    }

    roots.sort_by(|lhs, rhs| {
        lhs.logical_name
            .cmp(&rhs.logical_name)
            .then_with(|| lhs.export_name.cmp(&rhs.export_name))
    });
    Ok(roots)
}

fn reject_duplicate(
    seen: &mut BTreeMap<String, String>,
    value: &str,
    registration_path: &str,
    field: &str,
) -> Result<(), RegistrationError> {
    if let Some(previous) = seen.insert(value.to_string(), registration_path.to_string()) {
        return Err(RegistrationError::new(
            registration_path,
            format!("duplicate {field} `{value}`; first registered by `{previous}`"),
        ));
    }
    Ok(())
}

fn final_path_segment(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
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

    fn add_root(
        &mut self,
        instance: Instance<'tcx>,
        logical_name: String,
        export_name: String,
        typed_profile: Option<TypedKernelProfile>,
    ) {
        let symbol = self.tcx.symbol_name(instance).name.to_string();
        if self.seen.insert(symbol) {
            self.used_export_names.insert(export_name.clone());
            self.worklist.push_back(CollectedFunction {
                instance,
                is_kernel: true,
                export_name,
                logical_name: Some(logical_name),
                typed_profile,
            });
        }
    }

    fn collect(mut self) -> Result<CollectionResult<'tcx>, CollectError> {
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
                        self.process_call_operand(func, &function.instance)?;
                    }
                }
            }

            self.result.push(function);
        }

        Ok(CollectionResult {
            functions: self.result,
        })
    }

    fn process_call_operand(
        &mut self,
        func: &Operand<'tcx>,
        caller: &Instance<'tcx>,
    ) -> Result<(), CollectError> {
        let Operand::Constant(const_op) = func else {
            return Ok(());
        };

        let ty = const_op.const_.ty();
        let TyKind::FnDef(def_id, args) = ty.kind() else {
            return Ok(());
        };

        match self.should_collect_from_crate(*def_id) {
            CollectDecision::Collect => {}
            CollectDecision::SkipIntentional => return Ok(()),
            CollectDecision::Forbidden {
                crate_name,
                fn_path,
            } => {
                return Err(CollectError {
                    message: format!(
                        "fe2o3 device code reached forbidden crate `{crate_name}` via `{fn_path}`; device-reachable functions must avoid `std`"
                    ),
                });
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
            return Ok(());
        };

        let symbol = self.tcx.symbol_name(resolved).name.to_string();
        if self.seen.contains(&symbol) {
            return Ok(());
        }

        if !is_fully_monomorphized(self.tcx, resolved) {
            return Ok(());
        }

        if !matches!(resolved.def, InstanceKind::Item(_)) {
            return Ok(());
        }

        if !self.tcx.is_mir_available(resolved.def_id()) {
            if self.verbose {
                eprintln!(
                    "[collector] skipping no-MIR callee {}",
                    self.tcx.def_path_str(resolved.def_id())
                );
            }
            return Ok(());
        }

        if self.is_unreachable_body(resolved.def_id()) {
            if self.verbose {
                eprintln!(
                    "[collector] skipping intrinsic stub {}",
                    self.tcx.def_path_str(resolved.def_id())
                );
            }
            return Ok(());
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
            logical_name: None,
            typed_profile: None,
        });
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::{RegistrationRecord, TypedKernelProfile, validate_registration_records};
    use reserved_fe2o3_symbols::{
        KERNEL_PREFIX, KERNEL_REGISTRATION_KIND_KERNEL, KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1,
        KERNEL_REGISTRATION_MAGIC, KERNEL_REGISTRATION_PREFIX, KERNEL_REGISTRATION_VERSION_V1,
    };

    fn registration(
        path: &str,
        logical_name: &str,
        export_name: &str,
        target: u8,
    ) -> RegistrationRecord<u8> {
        RegistrationRecord {
            registration_path: path.to_string(),
            item_name: format!("{KERNEL_REGISTRATION_PREFIX}{logical_name}"),
            magic: KERNEL_REGISTRATION_MAGIC,
            version: KERNEL_REGISTRATION_VERSION_V1,
            kind: KERNEL_REGISTRATION_KIND_KERNEL,
            logical_name: logical_name.to_string(),
            export_name: export_name.to_string(),
            target_symbol: format!("{KERNEL_PREFIX}{export_name}"),
            target_identity: format!("target-{target}"),
            target,
        }
    }

    #[test]
    fn genuine_v1_registration_becomes_a_kernel_root() {
        let roots = validate_registration_records(vec![registration(
            "crate::__fe2o3_kernel_registration_vecadd",
            "vecadd",
            "vecadd",
            7,
        )])
        .unwrap();

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].target, 7);
        assert_eq!(roots[0].logical_name, "vecadd");
        assert_eq!(roots[0].export_name, "vecadd");
        assert_eq!(roots[0].typed_profile, None);
    }

    #[test]
    fn typed_vecadd_registration_carries_its_profile_into_the_kernel_root() {
        let mut typed = registration(
            "crate::__fe2o3_kernel_registration_vecadd",
            "vecadd",
            "vecadd",
            7,
        );
        typed.kind = KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1;

        let roots = validate_registration_records(vec![typed]).unwrap();

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].target, 7);
        assert_eq!(roots[0].logical_name, "vecadd");
        assert_eq!(roots[0].export_name, "vecadd");
        assert_eq!(roots[0].typed_profile, Some(TypedKernelProfile::VecAddV1));
    }

    #[test]
    fn kernel_prefix_spoof_without_registration_is_not_a_root() {
        let roots = validate_registration_records::<u8>(Vec::new()).unwrap();
        assert!(roots.is_empty());
    }

    #[test]
    fn malformed_magic_and_unknown_version_or_kind_fail_closed() {
        let base = registration("crate::__fe2o3_kernel_registration_bad", "bad", "bad", 1);

        let mut malformed = base.clone();
        malformed.magic ^= 1;
        assert!(
            validate_registration_records(vec![malformed])
                .unwrap_err()
                .reason
                .contains("does not match V1 magic")
        );

        let mut unknown_version = base.clone();
        unknown_version.version = KERNEL_REGISTRATION_VERSION_V1 + 1;
        assert!(
            validate_registration_records(vec![unknown_version])
                .unwrap_err()
                .reason
                .contains("unknown registration version")
        );

        for kind in [0, KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1 + 1, u16::MAX] {
            let mut unknown_kind = base.clone();
            unknown_kind.kind = kind;
            assert_eq!(
                validate_registration_records(vec![unknown_kind])
                    .unwrap_err()
                    .reason,
                format!("unknown registration kind {kind}")
            );
        }
    }

    #[test]
    fn duplicate_logical_and_export_names_fail_closed() {
        let logical_error = validate_registration_records(vec![
            registration(
                "crate::a::__fe2o3_kernel_registration_same",
                "same",
                "alpha",
                1,
            ),
            registration(
                "crate::b::__fe2o3_kernel_registration_same",
                "same",
                "beta",
                2,
            ),
        ])
        .unwrap_err();
        assert!(
            logical_error
                .reason
                .contains("duplicate logical name `same`")
        );

        let export_error = validate_registration_records(vec![
            registration(
                "crate::__fe2o3_kernel_registration_alpha",
                "alpha",
                "same",
                1,
            ),
            registration("crate::__fe2o3_kernel_registration_beta", "beta", "same", 2),
        ])
        .unwrap_err();
        assert!(export_error.reason.contains("duplicate export name `same`"));

        let mut typed_duplicate = registration(
            "crate::b::__fe2o3_kernel_registration_same",
            "same",
            "typed",
            2,
        );
        typed_duplicate.kind = KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1;
        let cross_kind_error = validate_registration_records(vec![
            registration(
                "crate::a::__fe2o3_kernel_registration_same",
                "same",
                "basic",
                1,
            ),
            typed_duplicate,
        ])
        .unwrap_err();
        assert!(
            cross_kind_error
                .reason
                .contains("duplicate logical name `same`")
        );
    }

    #[test]
    fn duplicate_target_identities_fail_closed() {
        let mut alpha = registration(
            "crate::__fe2o3_kernel_registration_alpha",
            "alpha",
            "alpha",
            1,
        );
        let mut beta = registration("crate::__fe2o3_kernel_registration_beta", "beta", "beta", 2);
        alpha.target_identity = "same-target".to_string();
        beta.target_identity = "same-target".to_string();

        let error = validate_registration_records(vec![alpha, beta]).unwrap_err();
        assert!(
            error
                .reason
                .contains("duplicate target identity `same-target`")
        );
    }

    #[test]
    fn inconsistent_item_or_target_associations_fail_closed() {
        let mut item = registration(
            "crate::__fe2o3_kernel_registration_alpha",
            "alpha",
            "alpha",
            1,
        );
        item.item_name = format!("{KERNEL_REGISTRATION_PREFIX}beta");
        assert!(
            validate_registration_records(vec![item])
                .unwrap_err()
                .reason
                .contains("inconsistent with logical name")
        );

        let mut target = registration(
            "crate::__fe2o3_kernel_registration_alpha",
            "alpha",
            "alpha",
            1,
        );
        target.target_symbol = format!("{KERNEL_PREFIX}beta");
        assert!(
            validate_registration_records(vec![target])
                .unwrap_err()
                .reason
                .contains("inconsistent with export name")
        );
    }

    #[test]
    fn multiple_kernels_are_sorted_deterministically() {
        let roots = validate_registration_records(vec![
            registration("crate::__fe2o3_kernel_registration_zeta", "zeta", "zeta", 2),
            registration(
                "crate::__fe2o3_kernel_registration_alpha",
                "alpha",
                "alpha",
                1,
            ),
        ])
        .unwrap();

        assert_eq!(
            roots
                .iter()
                .map(|root| (root.logical_name.as_str(), root.target))
                .collect::<Vec<_>>(),
            vec![("alpha", 1), ("zeta", 2)]
        );
    }
}
