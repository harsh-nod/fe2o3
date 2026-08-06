//! Compiler-side validation for explicit device FFI declarations.

use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DEVICE_FFI_DIRECTION_IMPORT_V1, DEVICE_FFI_MARKER_PREFIX_V1,
    DeviceFfiContractFieldsV1, DeviceFfiContractIdV1, MAX_DEVICE_FFI_EFFECT_BYTES_V1,
    MAX_DEVICE_FFI_PHYSICAL_ABI_BYTES_V1, MAX_DEVICE_FFI_SYMBOL_BYTES_V1,
    MAX_DEVICE_FFI_TARGET_BYTES_V1, derive_device_ffi_contract_id_v1,
};
use rustc_abi::ExternAbi;
use rustc_hir::def_id::DefId;
use rustc_hir::{ItemKind, Safety};
use rustc_middle::middle::codegen_fn_attrs::CodegenFnAttrFlags;
use rustc_middle::mir::mono::{CodegenUnit, MonoItem};
use rustc_middle::mir::{Operand, Rvalue, TerminatorKind};
use rustc_middle::ty::{
    EarlyBinder, FloatTy, Instance, IntTy, Ty, TyCtxt, TyKind, TypeVisitableExt, TypingEnv, UintTy,
};
use rustc_span::Symbol;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const MAX_DEVICE_FFI_CONTRACTS: usize = 128;
const MAX_DEVICE_FFI_ARGUMENTS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum DeviceFfiDirection {
    Import,
    Export,
}

impl DeviceFfiDirection {
    fn from_tag(tag: u16) -> Result<Self, DeviceFfiError> {
        match tag {
            DEVICE_FFI_DIRECTION_IMPORT_V1 => Ok(Self::Import),
            DEVICE_FFI_DIRECTION_EXPORT_V1 => Ok(Self::Export),
            _ => Err(DeviceFfiError::new(format!(
                "unknown device FFI direction {tag}"
            ))),
        }
    }

    const fn tag(self) -> u16 {
        match self {
            Self::Import => DEVICE_FFI_DIRECTION_IMPORT_V1,
            Self::Export => DEVICE_FFI_DIRECTION_EXPORT_V1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeviceFfiContract {
    pub(crate) id: DeviceFfiContractIdV1,
    pub(crate) direction: DeviceFfiDirection,
    pub(crate) symbol: String,
    pub(crate) target: String,
    pub(crate) code_object_version: u16,
    pub(crate) physical_abi: String,
    pub(crate) effects: String,
    pub(crate) semantic_identity: String,
}

#[derive(Clone, Debug)]
pub(crate) struct CollectedDeviceFfi<'tcx> {
    pub(crate) contract: DeviceFfiContract,
    pub(crate) instance: Instance<'tcx>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeviceFfiError {
    reason: String,
}

impl DeviceFfiError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl fmt::Display for DeviceFfiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid fe2o3 device FFI contract: {}",
            self.reason
        )
    }
}

impl std::error::Error for DeviceFfiError {}

pub(crate) fn count_exports_in_cgus<'tcx>(tcx: TyCtxt<'tcx>, cgus: &[CodegenUnit<'tcx>]) -> usize {
    cgu_instances(cgus)
        .filter(|instance| {
            marker_strings(tcx, instance.def_id()).iter().any(|marker| {
                marker.starts_with(DEVICE_FFI_MARKER_PREFIX_V1)
                    && marker
                        .strip_prefix(DEVICE_FFI_MARKER_PREFIX_V1)
                        .is_some_and(|fields| fields.starts_with("2|"))
            })
        })
        .count()
}

pub(crate) fn count_local_registration_candidates(tcx: TyCtxt<'_>) -> usize {
    tcx.hir_free_items()
        .filter(|item_id| {
            let item = tcx.hir_item(*item_id);
            let path = tcx.def_path_str(item.owner_id.def_id.to_def_id());
            path.rsplit("::")
                .next()
                .unwrap_or(&path)
                .starts_with(reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_PREFIX_V1)
        })
        .count()
}

pub(crate) fn collect_declarations<'tcx>(
    tcx: TyCtxt<'tcx>,
    cgus: &[CodegenUnit<'tcx>],
) -> Result<Vec<CollectedDeviceFfi<'tcx>>, DeviceFfiError> {
    let expected_target = std::env::var("FE2O3_TARGET")
        .ok()
        .filter(|target| !target.trim().is_empty())
        .unwrap_or_else(|| "gfx1100".to_owned());
    let mut declarations = Vec::new();
    let mut seen_instances = BTreeSet::new();

    for instance in cgu_instances(cgus) {
        let identity = tcx.symbol_name(instance).name.to_string();
        if !seen_instances.insert(identity) {
            continue;
        }
        if let Some(contract) = contract_for_instance(tcx, instance, &expected_target)? {
            declarations.push(CollectedDeviceFfi { contract, instance });
            if declarations.len() > MAX_DEVICE_FFI_CONTRACTS {
                return Err(DeviceFfiError::new(format!(
                    "more than {MAX_DEVICE_FFI_CONTRACTS} declarations were collected"
                )));
            }
        }
    }

    validate_local_registrations(tcx, &declarations)?;
    validate_contract_set(&mut declarations)?;
    Ok(declarations)
}

fn validate_local_registrations<'tcx>(
    tcx: TyCtxt<'tcx>,
    declarations: &[CollectedDeviceFfi<'tcx>],
) -> Result<(), DeviceFfiError> {
    let expected = declarations
        .iter()
        .filter(|declaration| declaration.instance.def_id().is_local())
        .map(|declaration| declaration.contract.id)
        .collect::<BTreeSet<_>>();
    let mut observed = BTreeSet::new();

    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);
        let def_id = item.owner_id.def_id;
        let path = tcx.def_path_str(def_id.to_def_id());
        let item_name = path.rsplit("::").next().unwrap_or(&path);
        let Some(contract_hex) =
            item_name.strip_prefix(reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_PREFIX_V1)
        else {
            continue;
        };
        let id = DeviceFfiContractIdV1::from_hex(contract_hex).map_err(|error| {
            DeviceFfiError::new(format!(
                "registration `{path}` has invalid identity: {error}"
            ))
        })?;
        if !matches!(item.kind, ItemKind::Static(..)) {
            return Err(DeviceFfiError::new(format!(
                "reserved registration `{path}` is not a static"
            )));
        }
        if tcx.is_mutable_static(def_id.to_def_id()) {
            return Err(DeviceFfiError::new(format!(
                "registration `{path}` must be immutable"
            )));
        }
        let flags = tcx.codegen_fn_attrs(def_id).flags;
        if !flags.intersects(CodegenFnAttrFlags::USED_COMPILER | CodegenFnAttrFlags::USED_LINKER) {
            return Err(DeviceFfiError::new(format!(
                "registration `{path}` must carry #[used]"
            )));
        }
        let ty = tcx.type_of(def_id).instantiate_identity();
        let TyKind::Tuple(fields) = ty.kind() else {
            return Err(DeviceFfiError::new(format!(
                "registration `{path}` must use the exact V1 tuple"
            )));
        };
        let valid = fields.len() == reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_V1_FIELD_COUNT
            && fields[0] == tcx.types.u64
            && fields[1] == tcx.types.u16
            && fields[2] == tcx.types.u16
            && is_shared_str(fields[3])
            && is_shared_str(fields[4])
            && is_shared_str(fields[5])
            && fields[6] == tcx.types.u16
            && is_shared_str(fields[7])
            && is_shared_str(fields[8])
            && is_shared_str(fields[9])
            && is_shared_str(fields[10])
            && matches!(fields[11].kind(), TyKind::FnPtr(..));
        if !valid {
            return Err(DeviceFfiError::new(format!(
                "registration `{path}` does not use the exact V1 tuple"
            )));
        }
        if !observed.insert(id) {
            return Err(DeviceFfiError::new(format!(
                "duplicate local registration identity {}",
                id.to_hex()
            )));
        }
    }

    if observed != expected {
        let missing = expected.difference(&observed).next().map(|id| id.to_hex());
        let orphan = observed.difference(&expected).next().map(|id| id.to_hex());
        return Err(DeviceFfiError::new(format!(
            "local registration set does not match compiler markers (missing={missing:?}, orphan={orphan:?})"
        )));
    }
    Ok(())
}

fn is_shared_str(ty: Ty<'_>) -> bool {
    matches!(ty.kind(), TyKind::Ref(_, inner, mutability) if inner.is_str() && !mutability.is_mut())
}

pub(crate) fn contract_for_instance<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    expected_target: &str,
) -> Result<Option<DeviceFfiContract>, DeviceFfiError> {
    let def_id = instance.def_id();
    let markers = marker_strings(tcx, def_id)
        .into_iter()
        .filter(|marker| marker.starts_with(DEVICE_FFI_MARKER_PREFIX_V1))
        .collect::<Vec<_>>();
    if markers.is_empty() {
        return Ok(None);
    }
    if markers.len() != 1 {
        return Err(DeviceFfiError::new(format!(
            "`{}` carries multiple compiler markers",
            tcx.def_path_str(def_id)
        )));
    }
    let contract = parse_marker(&markers[0])?;
    if contract.target != expected_target {
        return Err(DeviceFfiError::new(format!(
            "`{}` declares target `{}` but this compilation targets `{expected_target}`",
            tcx.def_path_str(def_id),
            contract.target
        )));
    }
    if tcx.generics_of(def_id).count() != 0 || !instance.args.is_empty() {
        return Err(DeviceFfiError::new(format!(
            "`{}` is generic; V1 device FFI roots require one concrete nongeneric identity",
            tcx.def_path_str(def_id)
        )));
    }
    validate_rustc_signature(tcx, instance, &contract)?;

    let has_body = tcx.is_mir_available(def_id);
    match contract.direction {
        DeviceFfiDirection::Export if !has_body => {
            return Err(DeviceFfiError::new(format!(
                "export `{}` has no MIR body",
                tcx.def_path_str(def_id)
            )));
        }
        DeviceFfiDirection::Import if has_body => {
            return Err(DeviceFfiError::new(format!(
                "import `{}` unexpectedly has a Rust body",
                tcx.def_path_str(def_id)
            )));
        }
        _ => {}
    }
    if contract.direction == DeviceFfiDirection::Export {
        validate_export_body(tcx, instance)?;
    }

    let attrs = tcx.codegen_fn_attrs(def_id);
    if attrs
        .symbol_name
        .is_none_or(|symbol| symbol.as_str() != contract.symbol)
    {
        return Err(DeviceFfiError::new(format!(
            "`{}` symbol metadata does not match `{}`",
            tcx.def_path_str(def_id),
            contract.symbol
        )));
    }
    if !attrs.target_features.is_empty() {
        return Err(DeviceFfiError::new(format!(
            "`{}` carries per-function target features outside its canonical target contract",
            tcx.def_path_str(def_id)
        )));
    }

    Ok(Some(contract))
}

fn validate_export_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: Instance<'tcx>,
) -> Result<(), DeviceFfiError> {
    const MAX_REACHABLE_FUNCTIONS: usize = 4_096;
    let mut pending = vec![root];
    let mut seen = BTreeSet::new();

    while let Some(instance) = pending.pop() {
        let identity = tcx.symbol_name(instance).name.to_string();
        if !seen.insert(identity) {
            continue;
        }
        if seen.len() > MAX_REACHABLE_FUNCTIONS {
            return Err(DeviceFfiError::new(
                "device export reachable-function bound exceeded",
            ));
        }
        if !tcx.is_mir_available(instance.def_id()) {
            continue;
        }
        let body = tcx.instance_mir(instance.def);
        if instance.def_id().is_local()
            && body
                .local_decls
                .iter()
                .any(|declaration| declaration.is_ref_to_static())
        {
            return Err(DeviceFfiError::new(format!(
                "device export `{}` reaches a static requiring unsupported relocation handling",
                tcx.def_path_str(instance.def_id())
            )));
        }
        for block in body.basic_blocks.iter() {
            for statement in &block.statements {
                if statement
                    .kind
                    .as_assign()
                    .is_some_and(|(_, value)| matches!(value, Rvalue::ThreadLocalRef(_)))
                {
                    return Err(DeviceFfiError::new(format!(
                        "device export `{}` reaches thread-local storage",
                        tcx.def_path_str(instance.def_id())
                    )));
                }
            }
            let Some(terminator) = &block.terminator else {
                continue;
            };
            match &terminator.kind {
                TerminatorKind::Assert { .. }
                | TerminatorKind::UnwindResume
                | TerminatorKind::UnwindTerminate(_) => {
                    return Err(DeviceFfiError::new(format!(
                        "device export `{}` contains a reachable panic or unwind path",
                        tcx.def_path_str(instance.def_id())
                    )));
                }
                TerminatorKind::Call { func, .. } => {
                    let Operand::Constant(constant) = func else {
                        return Err(DeviceFfiError::new(format!(
                            "device export `{}` contains an indirect call",
                            tcx.def_path_str(instance.def_id())
                        )));
                    };
                    let TyKind::FnDef(def_id, args) = constant.const_.ty().kind() else {
                        return Err(DeviceFfiError::new(format!(
                            "device export `{}` contains a non-function call operand",
                            tcx.def_path_str(instance.def_id())
                        )));
                    };
                    let path = tcx.def_path_str(*def_id);
                    if path.contains("::panicking::")
                        || path.contains("::panic_fmt")
                        || path.contains("::begin_panic")
                        || path.contains("::unwrap_failed")
                        || path.contains("precondition_check")
                    {
                        return Err(DeviceFfiError::new(format!(
                            "device export `{}` reaches panic path `{path}`",
                            tcx.def_path_str(root.def_id())
                        )));
                    }
                    let args = tcx.instantiate_and_normalize_erasing_regions(
                        instance.args,
                        TypingEnv::fully_monomorphized(),
                        EarlyBinder::bind(*args),
                    );
                    let resolved = Instance::try_resolve(
                        tcx,
                        TypingEnv::fully_monomorphized(),
                        *def_id,
                        args,
                    )
                    .ok()
                    .flatten()
                    .ok_or_else(|| {
                        DeviceFfiError::new(format!(
                            "device export call `{path}` did not resolve to a concrete instance"
                        ))
                    })?;
                    let import_boundary = marker_strings(tcx, resolved.def_id())
                        .into_iter()
                        .filter(|marker| marker.starts_with(DEVICE_FFI_MARKER_PREFIX_V1))
                        .map(|marker| parse_marker(&marker))
                        .collect::<Result<Vec<_>, _>>()?;
                    if import_boundary.len() > 1 {
                        return Err(DeviceFfiError::new(format!(
                            "callee `{path}` carries multiple device FFI markers"
                        )));
                    }
                    if import_boundary
                        .first()
                        .is_some_and(|contract| contract.direction == DeviceFfiDirection::Import)
                    {
                        continue;
                    }
                    if resolved.args.has_param() || resolved.args.has_escaping_bound_vars() {
                        return Err(DeviceFfiError::new(format!(
                            "device export call `{path}` is not fully monomorphized"
                        )));
                    }
                    pending.push(resolved);
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn validate_rustc_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    contract: &DeviceFfiContract,
) -> Result<(), DeviceFfiError> {
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.abi != (ExternAbi::C { unwind: false })
        || signature.safety != Safety::Unsafe
        || signature.c_variadic
    {
        return Err(DeviceFfiError::new(format!(
            "`{}` is not an unsafe, non-variadic, non-unwinding C function",
            tcx.def_path_str(instance.def_id())
        )));
    }
    if signature.inputs().len() > MAX_DEVICE_FFI_ARGUMENTS {
        return Err(DeviceFfiError::new("too many physical arguments"));
    }
    let mut physical = String::from("C(");
    for (index, input) in signature.inputs().iter().enumerate() {
        if index != 0 {
            physical.push(',');
        }
        physical.push_str(&canonical_rustc_type(tcx, *input)?);
    }
    physical.push_str(")->");
    if signature.output().is_unit() {
        physical.push_str("unit[size=0,align=1]");
    } else {
        physical.push_str(&canonical_rustc_type(tcx, signature.output())?);
    }
    if physical != contract.physical_abi {
        return Err(DeviceFfiError::new(format!(
            "rustc physical ABI `{physical}` disagrees with marker `{}`",
            contract.physical_abi
        )));
    }
    Ok(())
}

fn canonical_rustc_type(tcx: TyCtxt<'_>, ty: Ty<'_>) -> Result<String, DeviceFfiError> {
    let scalar = match ty.kind() {
        TyKind::Int(IntTy::I8) => Some(("i8", 1)),
        TyKind::Uint(UintTy::U8) => Some(("u8", 1)),
        TyKind::Int(IntTy::I16) => Some(("i16", 2)),
        TyKind::Uint(UintTy::U16) => Some(("u16", 2)),
        TyKind::Int(IntTy::I32) => Some(("i32", 4)),
        TyKind::Uint(UintTy::U32) => Some(("u32", 4)),
        TyKind::Int(IntTy::I64) => Some(("i64", 8)),
        TyKind::Uint(UintTy::U64) => Some(("u64", 8)),
        TyKind::Float(FloatTy::F32) => Some(("f32", 4)),
        TyKind::Float(FloatTy::F64) => Some(("f64", 8)),
        _ => None,
    };
    if let Some((name, size)) = scalar {
        return Ok(format!("{name}[size={size},align={size}]"));
    }

    let TyKind::Adt(adt, arguments) = ty.kind() else {
        return Err(DeviceFfiError::new(format!(
            "unsupported physical ABI type `{ty}`"
        )));
    };
    let pointer = POINTER_DIAGNOSTIC_ITEMS
        .iter()
        .find_map(|(marker, mutable, address_space)| {
            (tcx.get_diagnostic_item(Symbol::intern(marker)) == Some(adt.did()))
                .then_some((*mutable, *address_space))
        })
        .ok_or_else(|| DeviceFfiError::new(format!("unsupported aggregate ABI type `{ty}`")))?;
    if arguments.len() != 1 {
        return Err(DeviceFfiError::new(
            "device pointer has malformed generic arguments",
        ));
    }
    let element = arguments[0]
        .as_type()
        .ok_or_else(|| DeviceFfiError::new("device pointer element is not a type"))?;
    let element = canonical_rustc_type(tcx, element)?;
    let element = element
        .split('[')
        .next()
        .filter(|element| !element.contains("ptr"))
        .ok_or_else(|| DeviceFfiError::new("nested device pointers are unsupported"))?;
    Ok(format!(
        "{}_ptr<{},{}>[size=8,align=8,as={}]",
        if pointer.0 { "mut" } else { "const" },
        pointer.1,
        element,
        pointer.1,
    ))
}

const POINTER_DIAGNOSTIC_ITEMS: &[(&str, bool, &str)] = &[
    ("fe2o3_device_ffi_global_const_ptr_v1", false, "global"),
    ("fe2o3_device_ffi_global_mut_ptr_v1", true, "global"),
    ("fe2o3_device_ffi_constant_ptr_v1", false, "constant"),
    (
        "fe2o3_device_ffi_workgroup_const_ptr_v1",
        false,
        "workgroup",
    ),
    ("fe2o3_device_ffi_workgroup_mut_ptr_v1", true, "workgroup"),
    ("fe2o3_device_ffi_private_const_ptr_v1", false, "private"),
    ("fe2o3_device_ffi_private_mut_ptr_v1", true, "private"),
];

fn cgu_instances<'a, 'tcx>(
    cgus: &'a [CodegenUnit<'tcx>],
) -> impl Iterator<Item = Instance<'tcx>> + 'a {
    cgus.iter().flat_map(|cgu| {
        cgu.items().iter().filter_map(|(item, _)| match item {
            MonoItem::Fn(instance) => Some(*instance),
            _ => None,
        })
    })
}

#[allow(deprecated)]
fn marker_strings(tcx: TyCtxt<'_>, def_id: DefId) -> Vec<String> {
    tcx.get_all_attrs(def_id)
        .iter()
        .filter_map(|attribute| attribute.doc_str())
        .map(|symbol| symbol.as_str().to_owned())
        .collect()
}

fn parse_marker(marker: &str) -> Result<DeviceFfiContract, DeviceFfiError> {
    let payload = marker
        .strip_prefix(DEVICE_FFI_MARKER_PREFIX_V1)
        .ok_or_else(|| DeviceFfiError::new("marker has the wrong prefix"))?;
    let fields = payload.split('|').collect::<Vec<_>>();
    if fields.len() != 9 {
        return Err(DeviceFfiError::new("marker has the wrong field count"));
    }
    let direction_tag = fields[0]
        .parse::<u16>()
        .map_err(|_| DeviceFfiError::new("direction is not a canonical integer"))?;
    let direction = DeviceFfiDirection::from_tag(direction_tag)?;
    let id = DeviceFfiContractIdV1::from_hex(fields[1])
        .map_err(|error| DeviceFfiError::new(format!("invalid contract identity: {error}")))?;
    validate_symbol(fields[2])?;
    if fields[3] != "C" {
        return Err(DeviceFfiError::new("calling convention must be exactly C"));
    }
    let code_object_version = fields[4]
        .parse::<u16>()
        .map_err(|_| DeviceFfiError::new("code-object version is not a canonical integer"))?;
    if !matches!(code_object_version, 4..=6) || code_object_version.to_string() != fields[4] {
        return Err(DeviceFfiError::new("unsupported code-object version"));
    }
    validate_target(fields[5])?;
    if fields[6].is_empty() || fields[6].len() > MAX_DEVICE_FFI_PHYSICAL_ABI_BYTES_V1 {
        return Err(DeviceFfiError::new("physical ABI is empty or oversized"));
    }
    validate_effects(fields[7])?;
    validate_hex_identity(fields[8], "semantic identity")?;
    let canonical_fields = DeviceFfiContractFieldsV1 {
        direction: direction.tag(),
        symbol: fields[2],
        calling_convention: fields[3],
        code_object_version,
        target: fields[5],
        physical_abi: fields[6],
        effects: fields[7],
        semantic_identity: fields[8],
    };
    let derived = derive_device_ffi_contract_id_v1(canonical_fields);
    if derived != id {
        return Err(DeviceFfiError::new(format!(
            "contract identity {} disagrees with derived identity {}",
            id.to_hex(),
            derived.to_hex()
        )));
    }
    validate_effect_abi_compatibility(fields[7], fields[6])?;
    Ok(DeviceFfiContract {
        id,
        direction,
        symbol: fields[2].to_owned(),
        target: fields[5].to_owned(),
        code_object_version,
        physical_abi: fields[6].to_owned(),
        effects: fields[7].to_owned(),
        semantic_identity: fields[8].to_owned(),
    })
}

fn validate_contract_set<'tcx>(
    declarations: &mut Vec<CollectedDeviceFfi<'tcx>>,
) -> Result<(), DeviceFfiError> {
    declarations.sort_by(|lhs, rhs| {
        lhs.contract
            .symbol
            .cmp(&rhs.contract.symbol)
            .then_with(|| lhs.contract.direction.cmp(&rhs.contract.direction))
            .then_with(|| lhs.contract.id.cmp(&rhs.contract.id))
    });
    validate_contract_values(declarations.iter().map(|declaration| &declaration.contract))
}

fn validate_contract_values<'a>(
    contracts: impl IntoIterator<Item = &'a DeviceFfiContract>,
) -> Result<(), DeviceFfiError> {
    let mut ids = BTreeSet::new();
    let mut symbols: BTreeMap<&str, &DeviceFfiContract> = BTreeMap::new();
    for contract in contracts {
        if !ids.insert(contract.id) {
            return Err(DeviceFfiError::new(format!(
                "duplicate contract identity {}",
                contract.id.to_hex()
            )));
        }
        if let Some(previous) = symbols.insert(&contract.symbol, contract) {
            if previous.direction == contract.direction {
                return Err(DeviceFfiError::new(format!(
                    "duplicate {:?} declaration for symbol `{}`",
                    contract.direction, contract.symbol
                )));
            }
            if previous.target != contract.target
                || previous.code_object_version != contract.code_object_version
                || previous.physical_abi != contract.physical_abi
                || previous.effects != contract.effects
                || previous.semantic_identity != contract.semantic_identity
            {
                return Err(DeviceFfiError::new(format!(
                    "conflicting import/export declarations for symbol `{}`",
                    contract.symbol
                )));
            }
        }
    }
    Ok(())
}

fn validate_symbol(symbol: &str) -> Result<(), DeviceFfiError> {
    let mut bytes = symbol.bytes();
    let valid = symbol.len() <= MAX_DEVICE_FFI_SYMBOL_BYTES_V1
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'.' | b'$'))
        && bytes.all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$' | b'@' | b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(DeviceFfiError::new("invalid external symbol"))
    }
}

fn validate_target(target: &str) -> Result<(), DeviceFfiError> {
    if target.len() > MAX_DEVICE_FFI_TARGET_BYTES_V1 {
        return Err(DeviceFfiError::new("target is oversized"));
    }
    let mut parts = target.split(':');
    let processor = parts.next().unwrap_or_default();
    if !processor.starts_with("gfx")
        || processor.len() <= 3
        || !processor[3..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(DeviceFfiError::new("target processor is not canonical"));
    }
    let mut previous = None;
    let mut names = BTreeSet::new();
    for feature in parts {
        if !matches!(feature, "sramecc+" | "sramecc-" | "xnack+" | "xnack-")
            || previous.is_some_and(|previous: &str| previous >= feature)
            || !names.insert(&feature[..feature.len() - 1])
        {
            return Err(DeviceFfiError::new("target features are not canonical"));
        }
        previous = Some(feature);
    }
    Ok(())
}

fn validate_effects(effects: &str) -> Result<(), DeviceFfiError> {
    if effects.len() > MAX_DEVICE_FFI_EFFECT_BYTES_V1 {
        return Err(DeviceFfiError::new("effects are oversized"));
    }
    if effects == "none" {
        return Ok(());
    }
    let mut previous = None;
    for effect in effects.split(',') {
        if !matches!(
            effect,
            "atomic_global"
                | "atomic_workgroup"
                | "barrier_workgroup"
                | "read_constant"
                | "read_global"
                | "read_private"
                | "read_workgroup"
                | "write_global"
                | "write_private"
                | "write_workgroup"
        ) || previous.is_some_and(|previous: &str| previous >= effect)
        {
            return Err(DeviceFfiError::new("effects are not canonical"));
        }
        previous = Some(effect);
    }
    Ok(())
}

fn validate_hex_identity(value: &str, field: &str) -> Result<(), DeviceFfiError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
    {
        Ok(())
    } else {
        Err(DeviceFfiError::new(format!("invalid {field}")))
    }
}

fn validate_effect_abi_compatibility(effects: &str, abi: &str) -> Result<(), DeviceFfiError> {
    for effect in effects.split(',').filter(|effect| *effect != "none") {
        let required = match effect {
            "read_constant" => "const_ptr<constant,",
            "read_global" => "ptr<global,",
            "read_private" => "ptr<private,",
            "read_workgroup" => "ptr<workgroup,",
            "write_global" | "atomic_global" => "mut_ptr<global,",
            "write_private" => "mut_ptr<private,",
            "write_workgroup" | "atomic_workgroup" => "mut_ptr<workgroup,",
            "barrier_workgroup" => continue,
            _ => unreachable!("effect grammar was validated"),
        };
        if !abi.contains(required) {
            return Err(DeviceFfiError::new(format!(
                "effect `{effect}` has no compatible physical pointer argument"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(direction: u16, symbol: &str, abi: &str) -> String {
        let fields = DeviceFfiContractFieldsV1 {
            direction,
            symbol,
            calling_convention: "C",
            code_object_version: 5,
            target: "gfx942",
            physical_abi: abi,
            effects: "none",
            semantic_identity: "1111111111111111111111111111111111111111111111111111111111111111",
        };
        let id = derive_device_ffi_contract_id_v1(fields);
        reserved_fe2o3_symbols::device_ffi_marker_v1(id, fields)
    }

    #[test]
    fn canonical_marker_round_trips() {
        let parsed = parse_marker(&marker(
            DEVICE_FFI_DIRECTION_EXPORT_V1,
            "helper",
            "C(u32[size=4,align=4])->unit[size=0,align=1]",
        ))
        .unwrap();
        assert_eq!(parsed.direction, DeviceFfiDirection::Export);
        assert_eq!(parsed.symbol, "helper");
        assert_eq!(parsed.target, "gfx942");
    }

    #[test]
    fn every_marker_field_is_hash_bound() {
        let base = marker(
            DEVICE_FFI_DIRECTION_IMPORT_V1,
            "helper",
            "C()->unit[size=0,align=1]",
        );
        for replacement in ["other", "gfx950", "read_global"] {
            let mutated = if replacement == "other" {
                base.replacen("|helper|", "|other|", 1)
            } else if replacement == "gfx950" {
                base.replacen("|gfx942|", "|gfx950|", 1)
            } else {
                base.replacen("|none|", "|read_global|", 1)
            };
            assert!(
                parse_marker(&mutated)
                    .unwrap_err()
                    .reason
                    .contains("identity")
            );
        }
    }

    #[test]
    fn malformed_markers_fail_closed_without_panicking() {
        let base = marker(
            DEVICE_FFI_DIRECTION_IMPORT_V1,
            "helper",
            "C()->unit[size=0,align=1]",
        );
        for length in 0..base.len() {
            assert!(parse_marker(&base[..length]).is_err());
        }
        assert!(parse_marker(&(base + "|trailing")).is_err());
    }

    #[test]
    fn duplicate_and_conflicting_symbol_sets_are_rejected() {
        let export = parse_marker(&marker(
            DEVICE_FFI_DIRECTION_EXPORT_V1,
            "helper",
            "C()->unit[size=0,align=1]",
        ))
        .unwrap();
        assert!(validate_contract_values([&export, &export]).is_err());

        let import = parse_marker(&marker(
            DEVICE_FFI_DIRECTION_IMPORT_V1,
            "helper",
            "C()->unit[size=0,align=1]",
        ))
        .unwrap();
        validate_contract_values([&export, &import]).unwrap();

        let conflicting = parse_marker(&marker(
            DEVICE_FFI_DIRECTION_IMPORT_V1,
            "helper",
            "C(u32[size=4,align=4])->unit[size=0,align=1]",
        ))
        .unwrap();
        assert!(
            validate_contract_values([&export, &conflicting])
                .unwrap_err()
                .reason
                .contains("conflicting")
        );
    }
}
