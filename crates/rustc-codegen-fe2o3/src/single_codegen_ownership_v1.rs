//! Exact rustc mono-item ownership for one production compilation occurrence.

use object::{Object as _, ObjectSymbol as _};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_hir::{InlineAsmOperand, ItemKind};
use rustc_middle::mir::interpret::{AllocId, GlobalAlloc};
use rustc_middle::mir::mono::{CodegenUnit, CollectionMode, MonoItem};
use rustc_middle::mir::{AggregateKind, CastKind, Operand, RETURN_PLACE, Rvalue};
use rustc_middle::ty::adjustment::PointerCoercion;
use rustc_middle::ty::{Instance, TyCtxt, TyKind, TypingEnv};
use rustc_span::Symbol;
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock, Weak};

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactPartitionV1 {
    full: BTreeSet<String>,
    device: BTreeSet<String>,
}

impl ExactPartitionV1 {
    fn new(full: BTreeSet<String>, device: BTreeSet<String>) -> Result<Self, String> {
        if let Some(missing) = device.difference(&full).next() {
            return Err(format!(
                "device-owned mono-item `{missing}` is missing from the full rustc partition"
            ));
        }
        Ok(Self { full, device })
    }

    fn authenticate_and_partition(
        &self,
        current: &BTreeSet<String>,
    ) -> Result<BTreeSet<String>, String> {
        if current != &self.full {
            return Err("the rustc codegen partition is stale or substituted".to_owned());
        }
        Ok(self.full.difference(&self.device).cloned().collect())
    }

    #[cfg(test)]
    fn full(&self) -> &BTreeSet<String> {
        &self.full
    }

    fn device(&self) -> &BTreeSet<String> {
        &self.device
    }
}

fn transitive_device_partition_v1(
    roots: &BTreeSet<String>,
    adjacency: &BTreeMap<String, BTreeSet<String>>,
) -> Result<BTreeSet<String>, String> {
    let mut device = BTreeSet::new();
    let mut pending = roots.iter().cloned().collect::<VecDeque<_>>();
    while let Some(item) = pending.pop_front() {
        if !device.insert(item.clone()) {
            continue;
        }
        let edges = adjacency
            .get(&item)
            .ok_or_else(|| format!("device mono-item `{item}` is absent from the exact graph"))?;
        pending.extend(edges.iter().cloned());
    }
    Ok(device)
}

/// Typed evidence re-derived from collector custody before final publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedTypedRootClaimV1 {
    logical_name: String,
    entry_symbol: String,
    kernel_binding: [u8; 32],
}

impl AuthenticatedTypedRootClaimV1 {
    pub(crate) fn new(
        logical_name: impl Into<String>,
        entry_symbol: impl Into<String>,
        kernel_binding: [u8; 32],
    ) -> Self {
        Self {
            logical_name: logical_name.into(),
            entry_symbol: entry_symbol.into(),
            kernel_binding,
        }
    }
}

/// Exact root evidence derived from a registration only after the collector
/// has authenticated the complete production closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedDeviceRootV1<'tcx> {
    registration: String,
    instance: Instance<'tcx>,
    symbol: String,
    logical_name: String,
    kernel_binding: Option<[u8; 32]>,
}

fn registration_candidates_v1(tcx: TyCtxt<'_>) -> Vec<(String, rustc_hir::def_id::LocalDefId)> {
    let mut candidates = tcx
        .hir_free_items()
        .filter_map(|item_id| {
            let item = tcx.hir_item(item_id);
            let def_id = item.owner_id.def_id;
            let path = tcx.def_path_str(def_id.to_def_id());
            path.rsplit("::")
                .next()
                .is_some_and(|name| {
                    name.starts_with(reserved_fe2o3_symbols::KERNEL_REGISTRATION_PREFIX)
                })
                .then_some((path, def_id))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    candidates
}

fn registration_target_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &rustc_middle::mir::Body<'tcx>,
    operand: &Operand<'tcx>,
    registration: &str,
) -> Result<Instance<'tcx>, String> {
    let place = match operand {
        Operand::Copy(place) | Operand::Move(place) => place,
        Operand::Constant(constant) => {
            if !matches!(constant.const_.ty().kind(), TyKind::FnPtr(..)) {
                return Err(format!(
                    "authenticated registration `{registration}` target is not a function pointer"
                ));
            }
            let value = constant
                .const_
                .eval(tcx, TypingEnv::fully_monomorphized(), constant.span)
                .map_err(|_| {
                    format!("authenticated registration `{registration}` target did not evaluate")
                })?;
            let scalar = value.try_to_scalar().ok_or_else(|| {
                format!(
                    "authenticated registration `{registration}` target is not one scalar pointer"
                )
            })?;
            let pointer = scalar
                .to_pointer(&tcx)
                .discard_err()
                .ok_or_else(|| {
                    format!("authenticated registration `{registration}` target is not a pointer")
                })?
                .into_pointer_or_addr()
                .map_err(|_| {
                    format!("authenticated registration `{registration}` target has no provenance")
                })?;
            let (provenance, offset) = pointer.into_raw_parts();
            if offset.bytes() != 0 {
                return Err(format!(
                    "authenticated registration `{registration}` target has a nonzero pointer offset"
                ));
            }
            return match tcx.global_alloc(provenance.alloc_id()) {
                GlobalAlloc::Function { instance } => Ok(instance),
                _ => Err(format!(
                    "authenticated registration `{registration}` target does not identify a function"
                )),
            };
        }
        Operand::RuntimeChecks(_) => {
            return Err(format!(
                "authenticated registration `{registration}` target is not one exact function pointer"
            ));
        }
    };
    let target_local = place.as_local().ok_or_else(|| {
        format!(
            "authenticated registration `{registration}` target uses a projected function-pointer local"
        )
    })?;

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
                return Err(format!(
                    "authenticated registration `{registration}` target coercion does not directly name a function"
                ));
            };
            let TyKind::FnDef(def_id, args) = source.const_.ty().kind() else {
                return Err(format!(
                    "authenticated registration `{registration}` target coercion does not name a function definition"
                ));
            };
            let resolved = Instance::try_resolve(
                tcx,
                TypingEnv::fully_monomorphized(),
                *def_id,
                args,
            )
            .ok()
            .flatten()
            .ok_or_else(|| {
                format!(
                    "authenticated registration `{registration}` target instance did not resolve"
                )
            })?;
            if target.replace(resolved).is_some() {
                return Err(format!(
                    "authenticated registration `{registration}` target has multiple function definitions"
                ));
            }
        }
    }
    target.ok_or_else(|| {
        format!("authenticated registration `{registration}` target association is missing")
    })
}

fn kernel_registration_instance_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: rustc_hir::def_id::LocalDefId,
    registration: &str,
) -> Result<Instance<'tcx>, String> {
    let registration_ty = tcx
        .try_normalize_erasing_regions(
            TypingEnv::fully_monomorphized(),
            tcx.type_of(def_id).instantiate_identity(),
        )
        .map_err(|_| {
            format!("authenticated registration `{registration}` type did not normalize")
        })?;
    let TyKind::Tuple(types) = registration_ty.kind() else {
        return Err(format!(
            "authenticated registration `{registration}` is no longer a tuple"
        ));
    };
    let target_index = match types.len() {
        reserved_fe2o3_symbols::KERNEL_REGISTRATION_V1_FIELD_COUNT => 5,
        reserved_fe2o3_symbols::KERNEL_REGISTRATION_V2_FIELD_COUNT => 7,
        reserved_fe2o3_symbols::KERNEL_REGISTRATION_V3_FIELD_COUNT => 9,
        count => {
            return Err(format!(
                "authenticated registration `{registration}` changed to {count} fields"
            ));
        }
    };
    if !matches!(types[target_index].kind(), TyKind::FnPtr(..)) {
        return Err(format!(
            "authenticated registration `{registration}` target type changed"
        ));
    }

    let body = tcx.mir_for_ctfe(def_id);
    let mut tuple = None;
    for block in body.basic_blocks.iter() {
        for statement in &block.statements {
            let Some((place, Rvalue::Aggregate(kind, fields))) = statement.kind.as_assign() else {
                continue;
            };
            if place.as_local() != Some(RETURN_PLACE) || !matches!(**kind, AggregateKind::Tuple) {
                continue;
            }
            if tuple.replace(fields).is_some() {
                return Err(format!(
                    "authenticated registration `{registration}` has multiple tuple initializers"
                ));
            }
        }
    }
    let fields = tuple.ok_or_else(|| {
        format!("authenticated registration `{registration}` tuple initializer is missing")
    })?;
    if fields.len() != types.len() {
        return Err(format!(
            "authenticated registration `{registration}` type and initializer disagree"
        ));
    }
    let target = fields
        .iter()
        .nth(target_index)
        .expect("validated registration target field exists");
    registration_target_v1(tcx, body, target, registration)
}

pub(crate) fn rederive_roots_from_collector_custody_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    codegen_units: &[CodegenUnit<'tcx>],
    _collector_custody: &crate::collector::AuthenticatedCollectedKernelClosureV1<'tcx>,
    expected_root_count: usize,
    typed_claims: &[AuthenticatedTypedRootClaimV1],
) -> Result<Vec<AuthenticatedDeviceRootV1<'tcx>>, String> {
    let mut claims_by_symbol = typed_claims
        .iter()
        .map(|claim| (claim.entry_symbol.clone(), claim))
        .collect::<BTreeMap<_, _>>();
    if claims_by_symbol.len() != typed_claims.len() {
        return Err("collector custody contains duplicate typed entry symbols".to_owned());
    }

    let mut roots = Vec::new();
    for (registration, def_id) in registration_candidates_v1(tcx) {
        let instance = kernel_registration_instance_v1(tcx, def_id, &registration)?;
        let symbol = tcx.symbol_name(instance).name.to_string();
        let typed = claims_by_symbol.remove(&symbol);
        if let Some(claim) = typed {
            let binding =
                reserved_fe2o3_symbols::KernelBindingIdV1::from_bytes(claim.kernel_binding);
            let expected_symbol = reserved_fe2o3_symbols::host_kernel_symbol_v1(binding);
            if symbol != expected_symbol {
                return Err(format!(
                    "authenticated typed root `{}` binds `{expected_symbol}` but its exact registered instance emits `{symbol}`",
                    claim.logical_name
                ));
            }
        }
        roots.push(AuthenticatedDeviceRootV1 {
            logical_name: typed
                .map(|claim| claim.logical_name.clone())
                .unwrap_or_else(|| tcx.def_path_str(instance.def_id())),
            registration,
            instance,
            symbol,
            kernel_binding: typed.map(|claim| claim.kernel_binding),
        });
    }

    let ffi = crate::device_ffi::collect_declarations(tcx, codegen_units)
        .map_err(|error| format!("device FFI root re-derivation failed: {error}"))?;
    for declaration in ffi.into_iter().filter(|declaration| {
        declaration.contract.direction == crate::device_ffi::DeviceFfiDirection::Export
    }) {
        let instance = declaration.instance;
        roots.push(AuthenticatedDeviceRootV1 {
            registration: format!("device-ffi:{}", declaration.contract.id.to_hex()),
            instance,
            symbol: tcx.symbol_name(instance).name.to_string(),
            logical_name: declaration.owner.item_path,
            kernel_binding: None,
        });
    }

    if !claims_by_symbol.is_empty() {
        return Err(format!(
            "{} typed collector root(s) do not name an exact authenticated registration instance",
            claims_by_symbol.len()
        ));
    }
    if roots.len() != expected_root_count {
        return Err(format!(
            "exact authenticated registration custody contains {} root instance(s), but pre-monomorphization custody retained {expected_root_count}",
            roots.len()
        ));
    }
    let duplicate_instance = roots.iter().enumerate().any(|(position, root)| {
        roots[..position]
            .iter()
            .any(|previous| previous.instance == root.instance)
    });
    if duplicate_instance {
        return Err(
            "multiple authenticated registrations resolve to one exact root instance".to_owned(),
        );
    }
    Ok(roots)
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct MonoItemIdentityV1 {
    kind: &'static str,
    symbol: String,
    definition: String,
    instance: String,
}

impl MonoItemIdentityV1 {
    fn canonical_key(&self) -> String {
        format!(
            "{}\0{}\0{}\0{}",
            self.kind, self.symbol, self.definition, self.instance
        )
    }
}

fn mono_item_identity_v1<'tcx>(tcx: TyCtxt<'tcx>, item: MonoItem<'tcx>) -> MonoItemIdentityV1 {
    let (kind, instance) = match item {
        MonoItem::Fn(instance) => ("function", instance.to_string()),
        MonoItem::Static(_) => ("static", String::new()),
        MonoItem::GlobalAsm(_) => ("global-assembly", String::new()),
    };
    MonoItemIdentityV1 {
        kind,
        symbol: item.symbol_name(tcx).name.to_string(),
        definition: tcx.def_path_str(item.def_id()),
        instance,
    }
}

fn is_reserved_root_symbol_v1(symbol: &str) -> bool {
    symbol.starts_with("__fe2o3_host_kernel_v1_")
        || symbol.starts_with(reserved_fe2o3_symbols::KERNEL_PREFIX)
}

pub(crate) fn reject_unclaimed_reserved_roots_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    codegen_units: &[CodegenUnit<'tcx>],
) -> Result<(), String> {
    for cgu in codegen_units {
        for &item in cgu.items().keys() {
            if let MonoItem::Fn(_) = item {
                let symbol = item.symbol_name(tcx).name;
                if is_reserved_root_symbol_v1(symbol) {
                    return Err(format!(
                        "reserved device-root symbol `{symbol}` has no authenticated collector root"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn collect_allocation_edges_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    alloc_id: AllocId,
    visited: &mut BTreeSet<AllocId>,
    edges: &mut Vec<MonoItem<'tcx>>,
) {
    if !visited.insert(alloc_id) {
        return;
    }
    match tcx.global_alloc(alloc_id) {
        GlobalAlloc::Static(def_id) => {
            let instance = Instance::mono(tcx, def_id);
            if tcx.should_codegen_locally(instance) {
                edges.push(MonoItem::Static(def_id));
            }
        }
        GlobalAlloc::Memory(allocation) => {
            for provenance in allocation.inner().provenance().ptrs().values() {
                collect_allocation_edges_v1(tcx, provenance.alloc_id(), visited, edges);
            }
        }
        GlobalAlloc::Function { instance } => {
            if tcx.should_codegen_locally(instance) {
                edges.push(MonoItem::Fn(instance));
            }
        }
        GlobalAlloc::VTable(ty, dynamic_ty) => {
            let principal = dynamic_ty
                .principal()
                .map(|principal| tcx.instantiate_bound_regions_with_erased(principal));
            collect_allocation_edges_v1(
                tcx,
                tcx.vtable_allocation((ty, principal)),
                visited,
                edges,
            );
        }
        GlobalAlloc::TypeId { .. } => {}
    }
}

fn direct_edges_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    item: MonoItem<'tcx>,
) -> Result<Vec<MonoItem<'tcx>>, String> {
    match item {
        MonoItem::Fn(instance) => tcx
            .items_of_instance((instance, CollectionMode::UsedItems))
            .map(|(used, _)| used.iter().map(|item| item.node).collect())
            .map_err(|_| {
                format!(
                    "failed to derive rustc mono-item edges for function `{}`",
                    tcx.def_path_str(instance.def_id())
                )
            }),
        MonoItem::Static(def_id) => {
            if tcx.is_thread_local_static(def_id) {
                return Err(format!(
                    "device ownership does not admit thread-local static `{}`",
                    tcx.def_path_str(def_id)
                ));
            }
            let allocation = tcx.eval_static_initializer(def_id).map_err(|_| {
                format!(
                    "failed to evaluate static initializer `{}` for exact ownership",
                    tcx.def_path_str(def_id)
                )
            })?;
            let mut edges = Vec::new();
            let mut visited = BTreeSet::new();
            for provenance in allocation.inner().provenance().ptrs().values() {
                collect_allocation_edges_v1(tcx, provenance.alloc_id(), &mut visited, &mut edges);
            }
            Ok(edges)
        }
        MonoItem::GlobalAsm(item_id) => {
            let item = tcx.hir_item(item_id);
            let ItemKind::GlobalAsm { asm, .. } = item.kind else {
                return Err(format!(
                    "mono-item `{}` changed from global assembly after collection",
                    tcx.def_path_str(item_id.owner_id.def_id.to_def_id())
                ));
            };
            let mut edges = Vec::new();
            for (operand, span) in asm.operands {
                match *operand {
                    InlineAsmOperand::SymFn { expr } => {
                        let ty = tcx.typeck(item_id.owner_id).expr_ty(expr);
                        if let TyKind::FnDef(def_id, args) = *ty.kind() {
                            let instance = Instance::expect_resolve(
                                tcx,
                                TypingEnv::fully_monomorphized(),
                                def_id,
                                args,
                                *span,
                            );
                            if tcx.should_codegen_locally(instance) {
                                edges.push(MonoItem::Fn(instance));
                            }
                        }
                    }
                    InlineAsmOperand::SymStatic { def_id, .. } => {
                        let instance = Instance::mono(tcx, def_id);
                        if tcx.should_codegen_locally(instance) {
                            edges.push(MonoItem::Static(def_id));
                        }
                    }
                    InlineAsmOperand::Const { .. } => {}
                    InlineAsmOperand::In { .. }
                    | InlineAsmOperand::Out { .. }
                    | InlineAsmOperand::InOut { .. }
                    | InlineAsmOperand::SplitInOut { .. }
                    | InlineAsmOperand::Label { .. } => {
                        return Err(format!(
                            "global assembly `{}` contains an invalid ownership operand at {span:?}",
                            tcx.def_path_str(item_id.owner_id.def_id.to_def_id()),
                        ));
                    }
                }
            }
            Ok(edges)
        }
    }
}

fn is_authenticated_registration_static_v1(
    identity: &MonoItemIdentityV1,
    edges: &BTreeSet<String>,
    root_keys: &BTreeSet<String>,
) -> bool {
    identity.kind == "static"
        && [
            reserved_fe2o3_symbols::KERNEL_REGISTRATION_PREFIX,
            fe2o3_rustc_front::KERNEL_FRONTEND_REGISTRATION_PREFIX_V1,
            fe2o3_rustc_front::KERNEL_CONTEXT_FRONTEND_REGISTRATION_PREFIX_V1,
            fe2o3_rustc_front::KERNEL_RESOURCE_REGISTRATION_PREFIX_V1,
            fe2o3_rustc_front::CONTROL_FLOW_REGISTRATION_PREFIX_V1,
            reserved_fe2o3_symbols::REFERENCE_BINDING_REGISTRATION_PREFIX_V1,
        ]
        .iter()
        .any(|prefix| {
            identity
                .definition
                .rsplit("::")
                .next()
                .is_some_and(|name| name.starts_with(prefix))
        })
        && !edges.is_disjoint(root_keys)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodegenUnitOwnershipReceiptV1 {
    partition: ExactPartitionV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FinalV13DeviceSymbolRoleV1 {
    KernelEntry,
    DeviceFfiExport,
    InternalHelper,
}

/// Exact final-V13 symbols derived by joining authenticated collector custody
/// to exact rustc mono-item instances.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FinalV13ExpectedDeviceSymbolRosterV1(
    BTreeMap<String, FinalV13DeviceSymbolRoleV1>,
);

impl FinalV13ExpectedDeviceSymbolRosterV1 {
    fn from_graph<'tcx>(
        tcx: TyCtxt<'tcx>,
        device_items: &BTreeSet<String>,
        inventory: &BTreeMap<String, MonoItemIdentityV1>,
        collector: &crate::collector::AuthenticatedCollectedKernelClosureV1<'tcx>,
    ) -> Result<Self, String> {
        let expected_functions = device_items
            .iter()
            .filter(|key| inventory[*key].kind == "function")
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut mapped_functions = BTreeSet::new();
        let mut symbols = BTreeMap::new();
        for (instance, role, symbol) in collector.exact_codegen_function_symbols_v1() {
            let key = mono_item_identity_v1(tcx, MonoItem::Fn(instance)).canonical_key();
            if !expected_functions.contains(&key) {
                return Err(format!(
                    "collector final-V13 function `{}` is absent from the exact device mono graph",
                    tcx.def_path_str(instance.def_id())
                ));
            }
            if !mapped_functions.insert(key) {
                return Err(format!(
                    "collector final-V13 custody repeats exact function instance `{}`",
                    tcx.def_path_str(instance.def_id())
                ));
            }
            let role = match role {
                crate::collector::CollectedFunctionRole::KernelEntry => {
                    FinalV13DeviceSymbolRoleV1::KernelEntry
                }
                crate::collector::CollectedFunctionRole::DeviceFfiExport => {
                    FinalV13DeviceSymbolRoleV1::DeviceFfiExport
                }
                crate::collector::CollectedFunctionRole::InternalHelper => {
                    FinalV13DeviceSymbolRoleV1::InternalHelper
                }
            };
            if let Some(previous) = symbols.insert(symbol.to_owned(), role) {
                return Err(format!(
                    "collector final-V13 symbol `{symbol}` has duplicate roles {previous:?} and {role:?}"
                ));
            }
        }
        if mapped_functions != expected_functions {
            let missing = expected_functions
                .difference(&mapped_functions)
                .next()
                .expect("unequal function sets have one missing member");
            return Err(format!(
                "device mono-item `{}` has no exact collector final-V13 symbol",
                inventory[missing].definition
            ));
        }
        Ok(Self(symbols))
    }

    pub(crate) fn verify_compiler_module_manifest_v1(
        self,
        manifest: &fe2o3_compiler_ffi::CompilerModuleSymbolManifestV1,
    ) -> Result<(), String> {
        use fe2o3_compiler_ffi::CompilerModuleSymbolRoleV1 as ManifestRole;

        let mut observed = BTreeMap::new();
        for (manifest_role, expected_role) in [
            (
                ManifestRole::KernelEntry,
                FinalV13DeviceSymbolRoleV1::KernelEntry,
            ),
            (
                ManifestRole::DeviceFfiExport,
                FinalV13DeviceSymbolRoleV1::DeviceFfiExport,
            ),
            (
                ManifestRole::InternalHelper,
                FinalV13DeviceSymbolRoleV1::InternalHelper,
            ),
        ] {
            for symbol in manifest.symbols(manifest_role) {
                observed.insert(symbol.to_owned(), expected_role);
            }
        }
        if observed == self.0 {
            return Ok(());
        }
        if let Some((symbol, role)) = self
            .0
            .iter()
            .find(|entry| observed.get(entry.0) != Some(entry.1))
        {
            return Err(format!(
                "authenticated rustc ownership expects final-V13 symbol `{symbol}` with role {role:?}, but the inspected compiler module does not"
            ));
        }
        let (symbol, role) = observed
            .iter()
            .find(|entry| self.0.get(entry.0) != Some(entry.1))
            .expect("unequal symbol rosters have one unexpected member");
        Err(format!(
            "inspected compiler module contains unowned final-V13 symbol `{symbol}` with role {role:?}"
        ))
    }
}

fn reject_device_global_assembly_v1(
    device_items: &BTreeSet<String>,
    adjacency: &BTreeMap<String, BTreeSet<String>>,
    inventory: &BTreeMap<String, MonoItemIdentityV1>,
) -> Result<(), String> {
    if let Some(global_assembly) = device_items
        .iter()
        .filter_map(|key| inventory.get(key))
        .find(|identity| identity.kind == "global-assembly")
    {
        return Err(format!(
            "[FE2O3-OWN-GASM001] final V13 cannot emit device-reachable global assembly `{}`",
            global_assembly.definition
        ));
    }
    for (caller, edges) in adjacency {
        if inventory[caller].kind == "global-assembly" && !edges.is_disjoint(device_items) {
            return Err(format!(
                "[FE2O3-OWN-GASM001] final V13 cannot emit global assembly `{}` that references the authenticated device graph",
                inventory[caller].definition
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SingleCodegenOwnershipReceiptV1 {
    codegen_units: BTreeMap<String, CodegenUnitOwnershipReceiptV1>,
    device_items: BTreeSet<String>,
    device_symbols: BTreeSet<String>,
    device_root_symbols: BTreeSet<String>,
}

pub(crate) struct PreparedSingleCodegenOwnershipV1 {
    receipt: SingleCodegenOwnershipReceiptV1,
    final_v13_symbols: FinalV13ExpectedDeviceSymbolRosterV1,
}

impl PreparedSingleCodegenOwnershipV1 {
    pub(crate) fn into_parts(
        self,
    ) -> (
        SingleCodegenOwnershipReceiptV1,
        FinalV13ExpectedDeviceSymbolRosterV1,
    ) {
        (self.receipt, self.final_v13_symbols)
    }
}

pub(crate) fn build_receipt_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    codegen_units: &[CodegenUnit<'tcx>],
    authenticated_roots: &[AuthenticatedDeviceRootV1<'tcx>],
    collector: &crate::collector::AuthenticatedCollectedKernelClosureV1<'tcx>,
) -> Result<PreparedSingleCodegenOwnershipV1, String> {
    if authenticated_roots.is_empty() {
        return Err("collector custody contains no authenticated device root".to_owned());
    }

    let mut occurrences = BTreeMap::<MonoItemIdentityV1, Vec<MonoItem<'tcx>>>::new();
    let mut codegen_unit_items = BTreeMap::<String, BTreeSet<MonoItemIdentityV1>>::new();
    for cgu in codegen_units {
        let mut items = BTreeSet::new();
        for &item in cgu.items().keys() {
            let identity = mono_item_identity_v1(tcx, item);
            occurrences.entry(identity.clone()).or_default().push(item);
            if !items.insert(identity) {
                return Err(format!(
                    "codegen unit `{}` contains a duplicate mono-item identity",
                    cgu.name()
                ));
            }
        }
        if codegen_unit_items
            .insert(cgu.name().to_string(), items)
            .is_some()
        {
            return Err(format!("duplicate rustc codegen unit `{}`", cgu.name()));
        }
    }

    let mut roots_by_symbol = BTreeMap::new();
    for root in authenticated_roots {
        if let Some(previous) = roots_by_symbol.insert(root.symbol.clone(), root) {
            return Err(format!(
                "collector custody assigns root symbol `{}` to both exact instances `{}` and `{}`",
                root.symbol,
                tcx.def_path_str(previous.instance.def_id()),
                tcx.def_path_str(root.instance.def_id()),
            ));
        }
    }

    let mut root_items = Vec::new();
    let mut root_keys = BTreeSet::new();
    for root in authenticated_roots {
        let root_item = MonoItem::Fn(root.instance);
        let identity = mono_item_identity_v1(tcx, root_item);
        let candidates = occurrences.get(&identity).cloned().unwrap_or_default();
        let exact_occurrences = candidates
            .iter()
            .filter(|candidate| **candidate == root_item)
            .count();
        if candidates.len() != 1 || exact_occurrences != 1 {
            let binding = root
                .kernel_binding
                .map(|binding| format!("{:02x?}", binding))
                .unwrap_or_else(|| "<untyped>".to_owned());
            return Err(format!(
                "authenticated root `{}` from `{}` with binding {binding} maps its exact instance to {} codegen occurrences; exactly one is required",
                root.logical_name,
                root.registration,
                candidates.len(),
            ));
        }
        if identity.symbol != root.symbol {
            return Err(format!(
                "authenticated root `{}` changed symbol from `{}` to `{}`",
                root.logical_name, root.symbol, identity.symbol
            ));
        }
        root_keys.insert(identity.canonical_key());
        root_items.push(root_item);
    }

    for (identity, items) in &occurrences {
        if identity.kind == "function" && is_reserved_root_symbol_v1(&identity.symbol) {
            let authenticated = authenticated_roots.iter().any(|root| {
                items
                    .iter()
                    .any(|item| *item == MonoItem::Fn(root.instance))
            });
            if authenticated {
                continue;
            }
            return Err(format!(
                "reserved device-root symbol `{}` is not the exact instance named by authenticated collector custody",
                identity.symbol
            ));
        }
    }

    let inventory = occurrences
        .keys()
        .map(|identity| (identity.canonical_key(), identity.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut adjacency = BTreeMap::<String, BTreeSet<String>>::new();
    for (identity, items) in &occurrences {
        let edges = direct_edges_v1(tcx, items[0])?
            .into_iter()
            .map(|item| mono_item_identity_v1(tcx, item).canonical_key())
            .filter(|key| inventory.contains_key(key))
            .collect();
        adjacency.insert(identity.canonical_key(), edges);
    }

    let roots = root_items
        .into_iter()
        .map(|item| mono_item_identity_v1(tcx, item).canonical_key())
        .collect::<BTreeSet<_>>();
    let mut device_items = transitive_device_partition_v1(&roots, &adjacency)?;

    reject_device_global_assembly_v1(&device_items, &adjacency, &inventory)?;
    let final_v13_symbols = FinalV13ExpectedDeviceSymbolRosterV1::from_graph(
        tcx,
        &device_items,
        &inventory,
        collector,
    )?;

    for (key, identity) in &inventory {
        let edges = &adjacency[key];
        if is_authenticated_registration_static_v1(identity, edges, &root_keys) {
            device_items.insert(key.clone());
        }
    }

    for (caller, edges) in &adjacency {
        if device_items.contains(caller) {
            continue;
        }
        if let Some(callee) = edges.intersection(&device_items).next() {
            return Err(format!(
                "host mono-item `{}` also reaches device-owned mono-item `{}`; context-specific cloning is unavailable, so code generation failed closed",
                inventory[caller].definition, inventory[callee].definition
            ));
        }
    }

    let mut receipts = BTreeMap::new();
    for (name, full_items) in codegen_unit_items {
        let owned = full_items
            .iter()
            .map(MonoItemIdentityV1::canonical_key)
            .filter(|item| device_items.contains(item))
            .collect::<BTreeSet<_>>();
        let full = full_items
            .iter()
            .map(MonoItemIdentityV1::canonical_key)
            .collect::<BTreeSet<_>>();
        receipts.insert(
            name,
            CodegenUnitOwnershipReceiptV1 {
                partition: ExactPartitionV1::new(full, owned)?,
            },
        );
    }
    let accounted = receipts
        .values()
        .flat_map(|receipt| receipt.partition.device().iter().cloned())
        .collect::<BTreeSet<_>>();
    if accounted != device_items {
        let missing = device_items
            .difference(&accounted)
            .next()
            .map(String::as_str)
            .unwrap_or("unknown mono-item");
        return Err(format!(
            "device mono-item `{missing}` is missing from the exact rustc codegen partition"
        ));
    }

    let device_symbols = device_items
        .iter()
        .filter_map(|key| inventory.get(key))
        .map(|identity| identity.symbol.clone())
        .collect();
    Ok(PreparedSingleCodegenOwnershipV1 {
        receipt: SingleCodegenOwnershipReceiptV1 {
            codegen_units: receipts,
            device_items,
            device_symbols,
            device_root_symbols: roots_by_symbol.into_keys().collect(),
        },
        final_v13_symbols,
    })
}

type RustcCodegenUnitProvider = for<'tcx> fn(TyCtxt<'tcx>, Symbol) -> &'tcx CodegenUnit<'tcx>;

static LLVM_CODEGEN_UNIT_PROVIDER: OnceLock<RustcCodegenUnitProvider> = OnceLock::new();
static ACTIVE_COMPILATIONS: OnceLock<Mutex<CompilationRegistryV1>> = OnceLock::new();

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CompilationKeyV1 {
    session_address: usize,
    stable_crate_id: String,
}

impl CompilationKeyV1 {
    fn from_tcx(tcx: TyCtxt<'_>) -> Self {
        Self {
            session_address: tcx.sess as *const _ as usize,
            stable_crate_id: format!("{:?}", tcx.stable_crate_id(LOCAL_CRATE)),
        }
    }
}

#[derive(Clone, Debug)]
struct ActiveCompilationV1 {
    generation: u64,
    receipt: Weak<SingleCodegenOwnershipReceiptV1>,
}

#[derive(Default)]
struct CompilationRegistryV1 {
    next_generation: u64,
    active: BTreeMap<CompilationKeyV1, ActiveCompilationV1>,
}

impl CompilationRegistryV1 {
    fn install(
        &mut self,
        key: CompilationKeyV1,
        receipt: &Arc<SingleCodegenOwnershipReceiptV1>,
    ) -> Result<u64, String> {
        if self.active.contains_key(&key) {
            return Err(
                "stale or concurrent ownership already exists for this rustc compilation occurrence"
                    .to_owned(),
            );
        }
        let generation = self
            .next_generation
            .checked_add(1)
            .ok_or_else(|| "single-code-generation occurrence generation overflowed".to_owned())?;
        self.next_generation = generation;
        self.active.insert(
            key,
            ActiveCompilationV1 {
                generation,
                receipt: Arc::downgrade(receipt),
            },
        );
        Ok(generation)
    }

    fn resolve(
        &self,
        key: &CompilationKeyV1,
    ) -> Result<Option<Arc<SingleCodegenOwnershipReceiptV1>>, String> {
        self.active
            .get(key)
            .map(|active| {
                active.receipt.upgrade().ok_or_else(|| {
                    "compilation ownership route outlived its occurrence authority".to_owned()
                })
            })
            .transpose()
    }

    fn retire(&mut self, key: &CompilationKeyV1, generation: u64) -> Result<(), String> {
        match self.active.get(key) {
            Some(entry) if entry.generation == generation => {
                self.active.remove(key);
                Ok(())
            }
            Some(_) => Err("a stale occurrence attempted to retire newer ownership".to_owned()),
            None => Err("the compilation occurrence ownership was already retired".to_owned()),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CompilationOccurrenceV1 {
    key: CompilationKeyV1,
    generation: u64,
    receipt_authority: Arc<SingleCodegenOwnershipReceiptV1>,
    armed: bool,
}

pub(crate) fn install_receipt_v1(
    tcx: TyCtxt<'_>,
    receipt: SingleCodegenOwnershipReceiptV1,
) -> Result<CompilationOccurrenceV1, String> {
    if receipt.device_items.is_empty() || receipt.device_root_symbols.is_empty() {
        return Err("device ownership receipt contains no owned kernel graph".to_owned());
    }
    let key = CompilationKeyV1::from_tcx(tcx);
    let receipt_authority = Arc::new(receipt);
    let registry = ACTIVE_COMPILATIONS.get_or_init(|| Mutex::new(CompilationRegistryV1::default()));
    let mut registry = registry
        .lock()
        .map_err(|_| "single-code-generation ownership registry was poisoned".to_owned())?;
    let generation = registry.install(key.clone(), &receipt_authority)?;
    Ok(CompilationOccurrenceV1 {
        key,
        generation,
        receipt_authority,
        armed: true,
    })
}

fn retire_occurrence_v1(occurrence: &CompilationOccurrenceV1) -> Result<(), String> {
    let registry = ACTIVE_COMPILATIONS
        .get()
        .ok_or_else(|| "single-code-generation ownership registry is absent".to_owned())?;
    let mut registry = registry
        .lock()
        .map_err(|_| "single-code-generation ownership registry was poisoned".to_owned())?;
    registry.retire(&occurrence.key, occurrence.generation)
}

impl CompilationOccurrenceV1 {
    pub(crate) fn preflight_delegated_codegen_units(&self, tcx: TyCtxt<'_>) -> Result<(), String> {
        for (name, receipt) in &self.receipt_authority.codegen_units {
            let codegen_unit = tcx.codegen_unit(Symbol::intern(name));
            let observed = codegen_unit
                .items()
                .keys()
                .copied()
                .map(|item| mono_item_identity_v1(tcx, item).canonical_key())
                .collect::<BTreeSet<_>>();
            let expected = receipt
                .partition
                .full
                .difference(&receipt.partition.device)
                .cloned()
                .collect::<BTreeSet<_>>();
            if observed != expected {
                return Err(format!(
                    "delegated codegen unit `{name}` was cached before this compilation occurrence's exact host/device partition"
                ));
            }
        }
        Ok(())
    }

    fn retire(&mut self) -> Result<(), String> {
        if !self.armed {
            return Err("the compilation occurrence ownership was already retired".to_owned());
        }
        retire_occurrence_v1(self)?;
        self.armed = false;
        Ok(())
    }
}

impl Drop for CompilationOccurrenceV1 {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.retire();
        }
    }
}

pub(crate) struct DelegatedCodegenV1 {
    inner: Option<Box<dyn Any>>,
    occurrence: Option<CompilationOccurrenceV1>,
}

fn reject_device_symbols_in_object_v1(
    path: &Path,
    device_symbols: &BTreeSet<String>,
) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|error| {
        format!(
            "failed to read delegated LLVM object `{}`: {error}",
            path.display()
        )
    })?;
    let object = object::File::parse(bytes.as_slice()).map_err(|error| {
        format!(
            "failed to inspect delegated LLVM object `{}`: {error}",
            path.display()
        )
    })?;
    for symbol in object.symbols() {
        if !symbol.is_definition() {
            continue;
        }
        let name = symbol.name_bytes().map_err(|error| {
            format!(
                "delegated LLVM object `{}` has an unreadable defined symbol: {error}",
                path.display()
            )
        })?;
        if let Some(device) = device_symbols
            .iter()
            .find(|device| device.as_bytes() == name)
        {
            return Err(format!(
                "delegated LLVM object `{}` defines device-owned symbol `{device}`",
                path.display()
            ));
        }
    }
    Ok(())
}

fn verify_delegated_objects_v1(
    modules: &rustc_codegen_ssa::CompiledModules,
    receipt: &SingleCodegenOwnershipReceiptV1,
) -> Result<(), String> {
    for module in modules
        .modules
        .iter()
        .chain(modules.allocator_module.iter())
    {
        if let Some(object) = module.object.as_deref() {
            reject_device_symbols_in_object_v1(object, &receipt.device_symbols)?;
        }
    }
    Ok(())
}

impl DelegatedCodegenV1 {
    pub(crate) fn new(inner: Box<dyn Any>, occurrence: Option<CompilationOccurrenceV1>) -> Self {
        Self {
            inner: Some(inner),
            occurrence,
        }
    }

    pub(crate) fn take_inner(&mut self) -> Box<dyn Any> {
        self.inner
            .take()
            .expect("delegated LLVM codegen state was consumed once")
    }

    pub(crate) fn verify_delegated_objects(
        &self,
        modules: &rustc_codegen_ssa::CompiledModules,
    ) -> Result<(), String> {
        let Some(occurrence) = &self.occurrence else {
            return Ok(());
        };
        verify_delegated_objects_v1(modules, &occurrence.receipt_authority)
    }

    pub(crate) fn retire(&mut self) -> Result<(), String> {
        let Some(mut occurrence) = self.occurrence.take() else {
            return Ok(());
        };
        occurrence.retire()
    }
}

pub(crate) fn install_codegen_unit_provider_v1(providers: &mut rustc_middle::util::Providers) {
    let llvm_provider = providers.queries.codegen_unit;
    if let Some(retained) = LLVM_CODEGEN_UNIT_PROVIDER.get() {
        if !std::ptr::fn_addr_eq(*retained, llvm_provider) {
            panic!("rustc_codegen_llvm changed its codegen-unit provider within one process");
        }
    } else {
        LLVM_CODEGEN_UNIT_PROVIDER
            .set(llvm_provider)
            .expect("LLVM codegen-unit provider was checked before installation");
    }
    providers.queries.codegen_unit = host_only_codegen_unit_v1;
}

fn host_only_codegen_unit_v1<'tcx>(tcx: TyCtxt<'tcx>, name: Symbol) -> &'tcx CodegenUnit<'tcx> {
    let original_provider = LLVM_CODEGEN_UNIT_PROVIDER
        .get()
        .copied()
        .unwrap_or_else(|| {
            tcx.dcx()
                .fatal("[rustc-codegen-fe2o3] LLVM codegen-unit provider custody is unavailable")
        });
    let original = original_provider(tcx, name);
    let current_items = original
        .items()
        .keys()
        .copied()
        .map(|item| mono_item_identity_v1(tcx, item).canonical_key())
        .collect::<BTreeSet<_>>();
    let key = CompilationKeyV1::from_tcx(tcx);
    let receipt = ACTIVE_COMPILATIONS
        .get_or_init(|| Mutex::new(CompilationRegistryV1::default()))
        .lock()
        .unwrap_or_else(|_| {
            tcx.dcx().fatal(
                "[rustc-codegen-fe2o3] single-code-generation ownership registry was poisoned",
            )
        })
        .resolve(&key)
        .unwrap_or_else(|error| tcx.dcx().fatal(format!("[rustc-codegen-fe2o3] {error}")));
    let Some(receipt) = receipt else {
        return original;
    };
    let Some(cgu_receipt) = receipt.codegen_units.get(name.as_str()) else {
        tcx.dcx().fatal(format!(
            "[rustc-codegen-fe2o3] codegen unit `{name}` is absent from this compilation occurrence's ownership receipt"
        ));
    };
    let host_items = cgu_receipt
        .partition
        .authenticate_and_partition(&current_items)
        .unwrap_or_else(|error| {
            tcx.dcx().fatal(format!(
                "[rustc-codegen-fe2o3] codegen unit `{name}` changed after V13 ownership was authenticated: {error}"
            ))
        });

    let mut host = CodegenUnit::new(original.name());
    if original.is_primary() {
        host.make_primary();
    }
    if original.is_code_coverage_dead_code_cgu() {
        host.make_code_coverage_dead_code_cgu();
    }
    for (&item, &data) in original.items() {
        if host_items.contains(&mono_item_identity_v1(tcx, item).canonical_key()) {
            host.items_mut().insert(item, data);
        }
    }
    host.compute_size_estimate();
    let retained = host
        .items()
        .keys()
        .copied()
        .map(|item| mono_item_identity_v1(tcx, item).canonical_key())
        .collect::<BTreeSet<_>>();
    if !retained.is_disjoint(&receipt.device_items) {
        tcx.dcx().fatal(format!(
            "[rustc-codegen-fe2o3] codegen unit `{name}` retained a device-owned mono-item"
        ));
    }
    tcx.arena.alloc(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn receipt(root: &str) -> SingleCodegenOwnershipReceiptV1 {
        SingleCodegenOwnershipReceiptV1 {
            codegen_units: BTreeMap::new(),
            device_items: BTreeSet::from([root.to_owned()]),
            device_symbols: BTreeSet::from([root.to_owned()]),
            device_root_symbols: BTreeSet::from([root.to_owned()]),
        }
    }

    fn identity(kind: &'static str, symbol: &str, definition: &str) -> MonoItemIdentityV1 {
        MonoItemIdentityV1 {
            kind,
            symbol: symbol.to_owned(),
            definition: definition.to_owned(),
            instance: String::new(),
        }
    }

    fn key(session_address: usize, stable_crate_id: &str) -> CompilationKeyV1 {
        CompilationKeyV1 {
            session_address,
            stable_crate_id: stable_crate_id.to_owned(),
        }
    }

    #[test]
    fn compilation_registry_is_occurrence_scoped_and_rejects_stale_reuse() {
        let alpha = key(1, "alpha");
        let beta = key(2, "beta");
        let mut registry = CompilationRegistryV1::default();

        let alpha_authority = Arc::new(receipt("alpha-root"));
        let beta_authority = Arc::new(receipt("beta-root"));
        let alpha_first = registry.install(alpha.clone(), &alpha_authority).unwrap();
        let beta_first = registry.install(beta.clone(), &beta_authority).unwrap();
        assert_ne!(alpha_first, beta_first);
        let forged = Arc::new(receipt("forged"));
        assert!(registry.install(alpha.clone(), &forged).is_err());
        assert!(Arc::ptr_eq(
            &registry.resolve(&alpha).unwrap().unwrap(),
            &alpha_authority
        ));

        registry.retire(&alpha, alpha_first).unwrap();
        let alpha_second_authority = Arc::new(receipt("alpha-root-2"));
        let alpha_second = registry
            .install(alpha.clone(), &alpha_second_authority)
            .unwrap();
        assert_ne!(alpha_first, alpha_second);
        assert!(registry.retire(&alpha, alpha_first).is_err());
        assert!(registry.active.contains_key(&alpha));
        registry.retire(&alpha, alpha_second).unwrap();
        registry.retire(&beta, beta_first).unwrap();
        assert!(registry.active.is_empty());
    }

    #[test]
    fn registry_rejects_an_expired_weak_route() {
        let key = key(3, "expired");
        let receipt = {
            let authority = Arc::new(receipt("expired"));
            Arc::downgrade(&authority)
        };
        let mut registry = CompilationRegistryV1::default();
        registry.active.insert(
            key.clone(),
            ActiveCompilationV1 {
                generation: 1,
                receipt,
            },
        );
        assert!(registry.resolve(&key).is_err());
    }

    #[test]
    fn exact_partitions_cover_one_and_multiple_roots() {
        let full = set(&["device::alpha", "device::zeta", "device::shared", "host"]);
        let partition = ExactPartitionV1::new(
            full.clone(),
            set(&["device::alpha", "device::zeta", "device::shared"]),
        )
        .unwrap();
        assert_eq!(partition.full(), &full);
        assert_eq!(
            partition.authenticate_and_partition(&full).unwrap(),
            set(&["host"])
        );
    }

    #[test]
    fn exact_generic_instance_and_same_named_host_function_remain_distinct() {
        let root = "app::root";
        let u32_helper = "helper[upstream]::generic::<u32>";
        let u64_helper = "helper[upstream]::generic::<u64>";
        let host_same_name = "app::host::generic";
        let full = set(&[root, u32_helper, u64_helper, host_same_name]);
        let partition = ExactPartitionV1::new(full.clone(), set(&[root, u32_helper])).unwrap();
        assert_eq!(
            partition.authenticate_and_partition(&full).unwrap(),
            set(&[u64_helper, host_same_name])
        );
    }

    #[test]
    fn stale_missing_and_substituted_partitions_fail_closed() {
        assert!(ExactPartitionV1::new(set(&["host"]), set(&["missing-root"])).is_err());
        let partition = ExactPartitionV1::new(set(&["root", "host"]), set(&["root"])).unwrap();
        assert!(
            partition
                .authenticate_and_partition(&set(&["host"]))
                .is_err()
        );
        assert!(
            partition
                .authenticate_and_partition(&set(&["forged-root", "host"]))
                .is_err()
        );
    }

    #[test]
    fn graph_closure_tracks_exact_helpers_and_statics() {
        let adjacency = BTreeMap::from([
            ("root".to_owned(), set(&["helper::<u32>", "static"])),
            ("helper::<u32>".to_owned(), set(&[])),
            ("helper::<u64>".to_owned(), set(&[])),
            ("static".to_owned(), set(&["helper::<u32>"])),
        ]);
        assert_eq!(
            transitive_device_partition_v1(&set(&["root"]), &adjacency).unwrap(),
            set(&["root", "helper::<u32>", "static"])
        );
    }

    #[test]
    fn device_global_assembly_has_a_stable_rejection() {
        let inventory = BTreeMap::from([
            (
                "root".to_owned(),
                identity("function", "root", "crate::root"),
            ),
            (
                "asm".to_owned(),
                identity("global-assembly", "global_asm_1", "crate::device_asm"),
            ),
        ]);
        let adjacency = BTreeMap::from([
            ("root".to_owned(), set(&[])),
            ("asm".to_owned(), set(&["root"])),
        ]);
        let error =
            reject_device_global_assembly_v1(&set(&["root"]), &adjacency, &inventory).unwrap_err();
        assert!(error.starts_with("[FE2O3-OWN-GASM001]"));
        assert!(error.contains("crate::device_asm"));
    }

    #[test]
    fn final_v13_roster_requires_the_exact_inspected_manifest_roles() {
        use fe2o3_compiler_ffi::CompilerModuleSymbolRoleV1 as Role;

        let roster = FinalV13ExpectedDeviceSymbolRosterV1(BTreeMap::from([
            ("kernel".to_owned(), FinalV13DeviceSymbolRoleV1::KernelEntry),
            (
                "helper".to_owned(),
                FinalV13DeviceSymbolRoleV1::InternalHelper,
            ),
        ]));
        let exact = fe2o3_compiler_ffi::CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, "kernel"),
            (Role::KernelDescriptor, "kernel.kd"),
            (Role::InternalHelper, "helper"),
        ])
        .unwrap();
        roster.verify_compiler_module_manifest_v1(&exact).unwrap();

        let substituted = FinalV13ExpectedDeviceSymbolRosterV1(BTreeMap::from([(
            "kernel".to_owned(),
            FinalV13DeviceSymbolRoleV1::KernelEntry,
        )]));
        let manifest = fe2o3_compiler_ffi::CompilerModuleSymbolManifestV1::new([
            (Role::KernelEntry, "forged"),
            (Role::KernelDescriptor, "forged.kd"),
        ])
        .unwrap();
        assert!(
            substituted
                .verify_compiler_module_manifest_v1(&manifest)
                .unwrap_err()
                .contains("authenticated rustc ownership expects")
        );
    }

    #[test]
    fn object_inspection_rejects_an_observed_device_symbol() {
        let executable = std::env::current_exe().unwrap();
        let bytes = std::fs::read(&executable).unwrap();
        let object = object::File::parse(bytes.as_slice()).unwrap();
        let symbol = object
            .symbols()
            .find(|symbol| symbol.is_definition() && symbol.name_bytes().is_ok())
            .and_then(|symbol| symbol.name().ok())
            .expect("test executable has one named defined symbol")
            .to_owned();
        let error =
            reject_device_symbols_in_object_v1(&executable, &BTreeSet::from([symbol.clone()]))
                .unwrap_err();
        assert!(error.contains(&symbol));
        reject_device_symbols_in_object_v1(&executable, &BTreeSet::new()).unwrap();
    }
}
