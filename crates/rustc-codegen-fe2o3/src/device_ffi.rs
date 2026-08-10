//! Compiler-side validation for explicit device FFI declarations.

use reserved_fe2o3_symbols::{
    CROSS_CRATE_DEVICE_EXPORT_ANCHOR_FIELD_COUNT_V1, CROSS_CRATE_DEVICE_EXPORT_ANCHOR_MAGIC_V1,
    CROSS_CRATE_DEVICE_EXPORT_ANCHOR_PREFIX_V1, CROSS_CRATE_DEVICE_EXPORT_ANCHOR_VERSION_V1,
};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_MARKER_PREFIX_V1, DeviceFfiContractFieldsV1, DeviceFfiContractIdV1,
    DeviceFfiDirectionV1, MAX_DEVICE_FFI_ARGUMENTS_V1, MAX_DEVICE_FFI_TARGET_BYTES_V1,
    derive_device_ffi_contract_id_v1, parse_device_ffi_direction_v1, parse_device_ffi_effects_v1,
    parse_device_ffi_physical_abi_v1, validate_device_ffi_effect_abi_v1,
    validate_device_ffi_symbol_v1,
};
use rustc_abi::ExternAbi;
use rustc_ast::LitKind;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::{DefId, DefIndex};
use rustc_hir::{Expr, ExprKind, ItemKind, Safety};
use rustc_middle::middle::codegen_fn_attrs::CodegenFnAttrFlags;
use rustc_middle::mir::interpret::GlobalAlloc;
use rustc_middle::mir::mono::{CodegenUnit, MonoItem};
use rustc_middle::mir::visit::Visitor;
use rustc_middle::mir::{
    AggregateKind, Body, CastKind, ConstOperand, Location, Operand, RETURN_PLACE, Rvalue,
    TerminatorKind, UnwindAction,
};
use rustc_middle::ty::adjustment::PointerCoercion;
use rustc_middle::ty::{
    EarlyBinder, FloatTy, Instance, IntTy, Ty, TyCtxt, TyKind, TypeVisitableExt, TypingEnv, UintTy,
};
use rustc_span::Symbol;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const MAX_DEVICE_FFI_CONTRACTS: usize = 128;

pub(crate) type DeviceFfiDirection = DeviceFfiDirectionV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeviceFfiContract {
    pub(crate) id: DeviceFfiContractIdV1,
    pub(crate) direction: DeviceFfiDirection,
    pub(crate) symbol: String,
    pub(crate) target: String,
    pub(crate) code_object_version_assertion: AssertionOnly<u16>,
    pub(crate) physical_abi: String,
    pub(crate) effects_assertion: AssertionOnly<String>,
    pub(crate) semantic_identity_assertion: AssertionOnly<String>,
}

/// A declaration claim that the compiler has checked only for canonical form
/// and local-registration consistency, not independently derived.
///
/// G1 request construction must not consume this value as evidence. A future
/// bridge must bind it to authenticated producer artifacts and linker facts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct AssertionOnly<T>(T);

impl<T> AssertionOnly<T> {
    fn new(value: T) -> Self {
        Self(value)
    }

    pub(crate) fn asserted_for_consistency_check(&self) -> &T {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct DeviceFfiSourceOwner {
    pub(crate) crate_name: String,
    pub(crate) item_path: String,
    pub(crate) def_path_hash: [u8; 16],
    pub(crate) concrete_instance_symbol: String,
}

impl DeviceFfiSourceOwner {
    fn label(&self) -> String {
        format!(
            "{} [def-path-hash={}]",
            self.item_path,
            lower_hex(&self.def_path_hash)
        )
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct DeviceFfiInstanceIdentity {
    def_path_hash: [u8; 16],
    concrete_instance_symbol: String,
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum DeviceFfiLinkRole {
    /// The definition must be assigned to a typed external input by G5.
    RequiresExternalDefinition,
    /// A future exact compiler-module input must provide the definition.
    ///
    /// This requirement does not assert that the current backend emits such a module.
    RequiresCompilerModuleDefinition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClosedDeviceFfiContract {
    pub(crate) contract: DeviceFfiContract,
    pub(crate) owner: DeviceFfiSourceOwner,
    /// Expected role asserted from declaration direction, not a bound linker input role.
    pub(crate) link_role_assertion: AssertionOnly<DeviceFfiLinkRole>,
}

/// Inert, locally authenticated compiler evidence after device graph traversal.
///
/// This private value is copied into an authority-free
/// `fe2o3-compiler-ffi` envelope after collection succeeds. That adapter does
/// not construct a G1 request; later wiring must still bind external provider
/// artifacts and an exact compiler module through a closure-aware protocol.
/// Code-object version, effects, semantic identity, and expected link roles are
/// declaration assertions, represented by [`AssertionOnly`], not derived facts.
///
/// Cross-crate V1 is intentionally bounded to one explicit local anchor for an
/// exact nongeneric gfx942:xnack- code-object V6 export and one exact producer
/// registration in the function's source crate. Other upstream assertions are
/// rejected with `AUTH001` or an `XCR` profile diagnostic; broader producer
/// evidence transport remains unsupported.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct DeviceFfiClosure {
    pub(crate) target: Option<String>,
    pub(crate) code_object_version_assertion: Option<AssertionOnly<u16>>,
    pub(crate) imports: Vec<ClosedDeviceFfiContract>,
    pub(crate) exports: Vec<ClosedDeviceFfiContract>,
}

impl DeviceFfiClosure {
    pub(crate) fn is_empty(&self) -> bool {
        self.imports.is_empty() && self.exports.is_empty()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CollectedDeviceFfi<'tcx> {
    pub(crate) contract: DeviceFfiContract,
    pub(crate) instance: Instance<'tcx>,
    pub(crate) owner: DeviceFfiSourceOwner,
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

    fn coded(code: &'static str, reason: impl Into<String>) -> Self {
        Self::new(format!("[FE2O3-FFI-{code}] {}", reason.into()))
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
    let local_registrations = local_registrations(tcx)?;
    let cross_crate_anchors = cross_crate_export_anchors(tcx)?;

    for instance in local_registrations
        .iter()
        .map(|registration| registration.instance)
        .chain(cross_crate_anchors.iter().map(|anchor| anchor.instance))
        .chain(cgu_instances(cgus))
    {
        let identity = stable_instance_identity(tcx, instance);
        if !seen_instances.insert(identity) {
            continue;
        }
        if let Some(contract) = contract_for_instance(tcx, instance, &expected_target)? {
            declarations.push(collected_declaration(tcx, instance, contract));
            enforce_contract_bound(declarations.len())?;
        }
    }

    validate_local_registration_set(&declarations, &local_registrations)?;
    validate_contract_set(&mut declarations)?;
    Ok(declarations)
}

#[derive(Clone, Debug)]
struct LocalDeviceFfiRegistration<'tcx> {
    id: DeviceFfiContractIdV1,
    instance: Instance<'tcx>,
    path: String,
}

fn local_registrations<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Result<Vec<LocalDeviceFfiRegistration<'tcx>>, DeviceFfiError> {
    let mut registrations = Vec::new();
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
        let ItemKind::Static(_, _, _, _) = item.kind else {
            return Err(DeviceFfiError::coded(
                "REG001",
                format!("reserved registration `{path}` is not a static"),
            ));
        };
        if tcx.is_mutable_static(def_id.to_def_id()) {
            return Err(DeviceFfiError::coded(
                "REG002",
                format!("registration `{path}` must be immutable"),
            ));
        }
        let flags = tcx.codegen_fn_attrs(def_id).flags;
        if !flags.intersects(CodegenFnAttrFlags::USED_COMPILER | CodegenFnAttrFlags::USED_LINKER) {
            return Err(DeviceFfiError::coded(
                "REG003",
                format!("registration `{path}` must carry #[used]"),
            ));
        }
        validate_registration_type(tcx, def_id, &path)?;
        let body = tcx.hir_maybe_body_owned_by(def_id).ok_or_else(|| {
            DeviceFfiError::coded(
                "REG004",
                format!("registration `{path}` has no available initializer body"),
            )
        })?;
        let ExprKind::Tup(fields) = body.value.kind else {
            return Err(DeviceFfiError::coded(
                "REG005",
                format!("registration `{path}` initializer is not the exact V1 tuple"),
            ));
        };
        if fields.len() != reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_V1_FIELD_COUNT {
            return Err(DeviceFfiError::coded(
                "REG005",
                format!("registration `{path}` initializer is not the exact V1 tuple"),
            ));
        }

        let id = DeviceFfiContractIdV1::from_hex(contract_hex).map_err(|error| {
            DeviceFfiError::coded(
                "REG006",
                format!("registration `{path}` has invalid identity: {error}"),
            )
        })?;
        let function_expression = &fields[11];
        if !matches!(function_expression.kind, ExprKind::Path(_)) {
            return Err(DeviceFfiError::coded(
                "REG007",
                format!(
                    "registration `{path}` must end in one direct function item and no cast or wrapper"
                ),
            ));
        }
        let function_ty = tcx.typeck(def_id).expr_ty(function_expression);
        let TyKind::FnDef(function, args) = function_ty.kind() else {
            return Err(DeviceFfiError::coded(
                "REG007",
                format!("registration `{path}` does not bind an exact function definition"),
            ));
        };
        let instance =
            Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *function, args)
                .map_err(|_| {
                    DeviceFfiError::coded(
                        "REG008",
                        format!("registration `{path}` function normalization failed"),
                    )
                })?
                .ok_or_else(|| {
                    DeviceFfiError::coded(
                        "REG008",
                        format!(
                            "registration `{path}` function did not resolve to a concrete instance"
                        ),
                    )
                })?;
        let contract = contract_assertion_for_def(tcx, instance.def_id())?.ok_or_else(|| {
            DeviceFfiError::coded(
                "REG009",
                format!(
                    "registration `{path}` function `{}` has no device FFI marker",
                    tcx.def_path_str(instance.def_id())
                ),
            )
        })?;
        if id != contract.id {
            return Err(DeviceFfiError::coded(
                "REG014",
                format!(
                    "registration `{path}` identity {} does not match pointed function contract {}",
                    id.to_hex(),
                    contract.id.to_hex()
                ),
            ));
        }
        validate_registration_initializer(&path, fields, id, &contract)?;
        registrations.push(LocalDeviceFfiRegistration { id, instance, path });
    }
    registrations.sort_by(|lhs, rhs| {
        lhs.id.cmp(&rhs.id).then_with(|| {
            stable_instance_identity(tcx, lhs.instance)
                .cmp(&stable_instance_identity(tcx, rhs.instance))
        })
    });
    Ok(registrations)
}

fn validate_registration_type(
    tcx: TyCtxt<'_>,
    def_id: rustc_hir::def_id::LocalDefId,
    path: &str,
) -> Result<(), DeviceFfiError> {
    let ty = tcx.type_of(def_id).instantiate_identity();
    let TyKind::Tuple(fields) = ty.kind() else {
        return Err(DeviceFfiError::coded(
            "REG010",
            format!("registration `{path}` must use the exact V1 tuple type"),
        ));
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
        return Err(DeviceFfiError::coded(
            "REG010",
            format!("registration `{path}` must use the exact V1 tuple type"),
        ));
    }
    Ok(())
}

fn validate_registration_initializer(
    path: &str,
    fields: &[Expr<'_>],
    id: DeviceFfiContractIdV1,
    contract: &DeviceFfiContract,
) -> Result<(), DeviceFfiError> {
    expect_registration_integer(
        path,
        "magic",
        &fields[0],
        u128::from(reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_MAGIC_V1),
    )?;
    expect_registration_integer(
        path,
        "version",
        &fields[1],
        u128::from(reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_VERSION_V1),
    )?;
    expect_registration_integer(
        path,
        "direction",
        &fields[2],
        u128::from(contract.direction.tag()),
    )?;
    expect_registration_string(path, "contract identity", &fields[3], &id.to_hex())?;
    expect_registration_string(path, "symbol", &fields[4], &contract.symbol)?;
    expect_registration_string(path, "calling convention", &fields[5], "C")?;
    expect_registration_integer(
        path,
        "code-object version",
        &fields[6],
        u128::from(
            *contract
                .code_object_version_assertion
                .asserted_for_consistency_check(),
        ),
    )?;
    expect_registration_string(path, "target", &fields[7], &contract.target)?;
    expect_registration_string(path, "physical ABI", &fields[8], &contract.physical_abi)?;
    expect_registration_string(
        path,
        "effects",
        &fields[9],
        contract.effects_assertion.asserted_for_consistency_check(),
    )?;
    expect_registration_string(
        path,
        "semantic identity",
        &fields[10],
        contract
            .semantic_identity_assertion
            .asserted_for_consistency_check(),
    )
}

fn expect_registration_integer(
    path: &str,
    field: &str,
    expression: &Expr<'_>,
    expected: u128,
) -> Result<(), DeviceFfiError> {
    let ExprKind::Lit(literal) = expression.kind else {
        return Err(registration_field_mismatch(path, field));
    };
    let LitKind::Int(observed, _) = literal.node else {
        return Err(registration_field_mismatch(path, field));
    };
    if observed != expected {
        return Err(registration_field_mismatch(path, field));
    }
    Ok(())
}

fn expect_registration_string(
    path: &str,
    field: &str,
    expression: &Expr<'_>,
    expected: &str,
) -> Result<(), DeviceFfiError> {
    let ExprKind::Lit(literal) = expression.kind else {
        return Err(registration_field_mismatch(path, field));
    };
    let LitKind::Str(observed, _) = literal.node else {
        return Err(registration_field_mismatch(path, field));
    };
    if observed.as_str() != expected {
        return Err(registration_field_mismatch(path, field));
    }
    Ok(())
}

fn registration_field_mismatch(path: &str, field: &str) -> DeviceFfiError {
    DeviceFfiError::coded(
        "REG011",
        format!("registration `{path}` initializer {field} does not match its function marker"),
    )
}

pub(crate) fn stable_instance_identity<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> DeviceFfiInstanceIdentity {
    DeviceFfiInstanceIdentity {
        def_path_hash: tcx.def_path_hash(instance.def_id()).0.to_le_bytes(),
        concrete_instance_symbol: tcx.symbol_name(instance).name.to_string(),
    }
}

pub(crate) fn source_owner_matches_instance<'tcx>(
    tcx: TyCtxt<'tcx>,
    owner: &DeviceFfiSourceOwner,
    instance: Instance<'tcx>,
) -> bool {
    let identity = stable_instance_identity(tcx, instance);
    owner.def_path_hash == identity.def_path_hash
        && owner.concrete_instance_symbol == identity.concrete_instance_symbol
}

pub(crate) fn collected_declaration<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    contract: DeviceFfiContract,
) -> CollectedDeviceFfi<'tcx> {
    let def_id = instance.def_id();
    let crate_name = tcx.crate_name(def_id.krate).as_str().to_owned();
    let raw_path = tcx.def_path_str(def_id);
    let item_path = if def_id.is_local() {
        format!("{crate_name}::{raw_path}")
    } else {
        raw_path
    };
    CollectedDeviceFfi {
        contract,
        instance,
        owner: DeviceFfiSourceOwner {
            crate_name,
            item_path,
            def_path_hash: tcx.def_path_hash(def_id).0.to_le_bytes(),
            concrete_instance_symbol: tcx.symbol_name(instance).name.to_string(),
        },
    }
}

fn validate_local_registration_set<'tcx>(
    declarations: &[CollectedDeviceFfi<'tcx>],
    registrations: &[LocalDeviceFfiRegistration<'tcx>],
) -> Result<(), DeviceFfiError> {
    let expected = declarations
        .iter()
        .filter(|declaration| declaration.instance.def_id().is_local())
        .map(|declaration| declaration.contract.id)
        .collect::<BTreeSet<_>>();
    let mut observed = BTreeSet::new();
    for registration in registrations {
        if !observed.insert(registration.id) {
            return Err(DeviceFfiError::coded(
                "REG012",
                format!(
                    "duplicate local registration identity {} at `{}`",
                    registration.id.to_hex(),
                    registration.path
                ),
            ));
        }
    }

    if observed != expected {
        let missing = expected.difference(&observed).next().map(|id| id.to_hex());
        let orphan = observed.difference(&expected).next().map(|id| id.to_hex());
        return Err(DeviceFfiError::coded(
            "REG013",
            format!(
                "local registration set does not match compiler markers (missing={missing:?}, orphan={orphan:?})"
            ),
        ));
    }
    Ok(())
}

fn is_shared_str(ty: Ty<'_>) -> bool {
    matches!(ty.kind(), TyKind::Ref(_, inner, mutability) if inner.is_str() && !mutability.is_mut())
}

#[derive(Clone, Debug)]
struct CrossCrateDeviceExportAnchor<'tcx> {
    id: DeviceFfiContractIdV1,
    instance: Instance<'tcx>,
    path: String,
}

fn authenticate_upstream_export<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    contract: &DeviceFfiContract,
) -> Result<bool, DeviceFfiError> {
    let matching_anchors = cross_crate_export_anchors(tcx)?
        .into_iter()
        .filter(|anchor| anchor.id == contract.id && anchor.instance == instance)
        .collect::<Vec<_>>();
    if matching_anchors.is_empty() {
        return Ok(false);
    }
    if matching_anchors.len() != 1 {
        return Err(DeviceFfiError::coded(
            "XCR001",
            format!(
                "upstream export `{}` has {} local import anchors; exactly one is required",
                tcx.def_path_str(instance.def_id()),
                matching_anchors.len()
            ),
        ));
    }
    if contract.direction != DeviceFfiDirection::Export
        || contract.target != "gfx942:xnack-"
        || *contract
            .code_object_version_assertion
            .asserted_for_consistency_check()
            != 6
    {
        return Err(DeviceFfiError::coded(
            "XCR002",
            format!(
                "cross-crate device exports are bounded to gfx942:xnack-, code-object V6 exports; `{}` is outside that profile",
                tcx.def_path_str(instance.def_id())
            ),
        ));
    }

    let registrations = upstream_export_registrations(tcx, instance, contract)?;
    if registrations.len() != 1 {
        return Err(DeviceFfiError::coded(
            "XCR003",
            format!(
                "upstream export `{}` has {} exact producer registrations in crate `{}`; exactly one is required",
                tcx.def_path_str(instance.def_id()),
                registrations.len(),
                tcx.crate_name(instance.def_id().krate)
            ),
        ));
    }
    Ok(true)
}

fn cross_crate_export_anchors<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Result<Vec<CrossCrateDeviceExportAnchor<'tcx>>, DeviceFfiError> {
    let mut anchors = Vec::new();
    for item_id in tcx.hir_free_items() {
        let item = tcx.hir_item(item_id);
        let def_id = item.owner_id.def_id;
        let path = tcx.def_path_str(def_id.to_def_id());
        let item_name = path.rsplit("::").next().unwrap_or(&path);
        if !item_name.starts_with(CROSS_CRATE_DEVICE_EXPORT_ANCHOR_PREFIX_V1) {
            continue;
        }
        if !matches!(item.kind, ItemKind::Static(_, _, _, _)) {
            return Err(DeviceFfiError::coded(
                "XCR004",
                format!("reserved cross-crate device anchor `{path}` is not a static"),
            ));
        }
        if tcx.is_mutable_static(def_id.to_def_id()) {
            return Err(DeviceFfiError::coded(
                "XCR005",
                format!("cross-crate device anchor `{path}` must be immutable"),
            ));
        }
        let flags = tcx.codegen_fn_attrs(def_id).flags;
        if !flags.intersects(CodegenFnAttrFlags::USED_COMPILER | CodegenFnAttrFlags::USED_LINKER) {
            return Err(DeviceFfiError::coded(
                "XCR006",
                format!("cross-crate device anchor `{path}` must carry #[used]"),
            ));
        }
        validate_cross_crate_anchor_type(tcx, def_id, &path)?;
        let body = tcx.mir_for_ctfe(def_id);
        let fields =
            static_tuple_fields(body, CROSS_CRATE_DEVICE_EXPORT_ANCHOR_FIELD_COUNT_V1, &path)?;
        expect_mir_integer(
            tcx,
            fields[0],
            tcx.types.u64,
            u128::from(CROSS_CRATE_DEVICE_EXPORT_ANCHOR_MAGIC_V1),
            "anchor magic",
            &path,
        )?;
        expect_mir_integer(
            tcx,
            fields[1],
            tcx.types.u16,
            u128::from(CROSS_CRATE_DEVICE_EXPORT_ANCHOR_VERSION_V1),
            "anchor version",
            &path,
        )?;
        let id = DeviceFfiContractIdV1::from_hex(&mir_string(
            tcx,
            fields[2],
            "contract identity",
            &path,
        )?)
        .map_err(|error| {
            DeviceFfiError::coded(
                "XCR007",
                format!("cross-crate device anchor `{path}` has invalid identity: {error}"),
            )
        })?;
        let instance = mir_function_target(tcx, body, fields[3], &path)?;
        if instance.def_id().is_local() {
            return Err(DeviceFfiError::coded(
                "XCR008",
                format!("cross-crate device anchor `{path}` points to a local function"),
            ));
        }
        let contract = contract_assertion_for_def(tcx, instance.def_id())?.ok_or_else(|| {
            DeviceFfiError::coded(
                "XCR009",
                format!(
                    "cross-crate device anchor `{path}` points to `{}` without a device FFI marker",
                    tcx.def_path_str(instance.def_id())
                ),
            )
        })?;
        if contract.id != id {
            return Err(DeviceFfiError::coded(
                "XCR010",
                format!(
                    "cross-crate device anchor `{path}` identity {} does not match exact function contract {}",
                    id.to_hex(),
                    contract.id.to_hex()
                ),
            ));
        }
        anchors.push(CrossCrateDeviceExportAnchor { id, instance, path });
    }
    anchors.sort_by(|lhs, rhs| lhs.path.cmp(&rhs.path));
    Ok(anchors)
}

fn validate_cross_crate_anchor_type(
    tcx: TyCtxt<'_>,
    def_id: rustc_hir::def_id::LocalDefId,
    path: &str,
) -> Result<(), DeviceFfiError> {
    let ty = tcx
        .try_normalize_erasing_regions(
            TypingEnv::fully_monomorphized(),
            tcx.type_of(def_id).instantiate_identity(),
        )
        .map_err(|_| {
            DeviceFfiError::coded(
                "XCR011",
                format!("cross-crate device anchor `{path}` type did not normalize"),
            )
        })?;
    let TyKind::Tuple(fields) = ty.kind() else {
        return Err(DeviceFfiError::coded(
            "XCR011",
            format!("cross-crate device anchor `{path}` must use the exact V1 tuple type"),
        ));
    };
    let valid = fields.len() == CROSS_CRATE_DEVICE_EXPORT_ANCHOR_FIELD_COUNT_V1
        && fields[0] == tcx.types.u64
        && fields[1] == tcx.types.u16
        && is_shared_str(fields[2])
        && matches!(fields[3].kind(), TyKind::FnPtr(..));
    if !valid {
        return Err(DeviceFfiError::coded(
            "XCR011",
            format!("cross-crate device anchor `{path}` must use the exact V1 tuple type"),
        ));
    }
    Ok(())
}

fn upstream_export_registrations<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    contract: &DeviceFfiContract,
) -> Result<Vec<DefId>, DeviceFfiError> {
    let crate_num = instance.def_id().krate;
    let expected_name = format!(
        "{}{}",
        reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_PREFIX_V1,
        contract.id.to_hex()
    );
    let mut registrations = Vec::new();
    for index in 0..tcx.num_extern_def_ids(crate_num) {
        let def_id = DefId {
            krate: crate_num,
            index: DefIndex::from_usize(index),
        };
        if !matches!(tcx.def_kind(def_id), DefKind::Static { .. }) {
            continue;
        }
        let path = tcx.def_path_str(def_id);
        if path.rsplit("::").next().unwrap_or(&path) != expected_name {
            continue;
        }
        if tcx.is_mutable_static(def_id) {
            return Err(DeviceFfiError::coded(
                "XCR012",
                format!("upstream producer registration `{path}` must be immutable"),
            ));
        }
        let flags = tcx.codegen_fn_attrs(def_id).flags;
        if !flags.intersects(CodegenFnAttrFlags::USED_COMPILER | CodegenFnAttrFlags::USED_LINKER) {
            return Err(DeviceFfiError::coded(
                "XCR013",
                format!("upstream producer registration `{path}` must carry #[used]"),
            ));
        }
        validate_upstream_registration_type(tcx, def_id, &path)?;
        validate_upstream_registration_initializer(tcx, def_id, instance, contract, &path)?;
        registrations.push(def_id);
    }
    registrations.sort_by_key(|def_id| tcx.def_path_str(*def_id));
    Ok(registrations)
}

fn validate_upstream_registration_initializer<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    instance: Instance<'tcx>,
    contract: &DeviceFfiContract,
    path: &str,
) -> Result<(), DeviceFfiError> {
    expect_static_integer(
        tcx,
        def_id,
        0,
        u128::from(reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_MAGIC_V1),
        "registration magic",
        path,
    )?;
    expect_static_integer(
        tcx,
        def_id,
        1,
        u128::from(reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_VERSION_V1),
        "registration version",
        path,
    )?;
    expect_static_integer(
        tcx,
        def_id,
        2,
        u128::from(contract.direction.tag()),
        "registration direction",
        path,
    )?;
    expect_static_string(
        tcx,
        def_id,
        3,
        "contract identity",
        &contract.id.to_hex(),
        path,
    )?;
    expect_static_string(tcx, def_id, 4, "symbol", &contract.symbol, path)?;
    expect_static_string(tcx, def_id, 5, "calling convention", "C", path)?;
    expect_static_integer(
        tcx,
        def_id,
        6,
        u128::from(
            *contract
                .code_object_version_assertion
                .asserted_for_consistency_check(),
        ),
        "code-object version",
        path,
    )?;
    expect_static_string(tcx, def_id, 7, "target", &contract.target, path)?;
    expect_static_string(tcx, def_id, 8, "physical ABI", &contract.physical_abi, path)?;
    expect_static_string(
        tcx,
        def_id,
        9,
        "effects",
        contract.effects_assertion.asserted_for_consistency_check(),
        path,
    )?;
    expect_static_string(
        tcx,
        def_id,
        10,
        "semantic identity",
        contract
            .semantic_identity_assertion
            .asserted_for_consistency_check(),
        path,
    )?;
    let registered = crate::static_registration::function(tcx, def_id, 11).map_err(|reason| {
        DeviceFfiError::coded(
            "XCR019",
            format!("upstream producer registration `{path}` function is invalid: {reason}"),
        )
    })?;
    if registered != instance {
        return Err(DeviceFfiError::coded(
            "XCR020",
            format!(
                "upstream producer registration `{path}` points to `{}`, not anchored export `{}`",
                tcx.def_path_str(registered.def_id()),
                tcx.def_path_str(instance.def_id()),
            ),
        ));
    }
    Ok(())
}

fn expect_static_integer(
    tcx: TyCtxt<'_>,
    def_id: DefId,
    index: usize,
    expected: u128,
    field: &str,
    path: &str,
) -> Result<(), DeviceFfiError> {
    let observed = crate::static_registration::integer(tcx, def_id, index).map_err(|reason| {
        DeviceFfiError::coded(
            "XCR019",
            format!("upstream producer registration `{path}` {field} is invalid: {reason}"),
        )
    })?;
    if observed != expected {
        return Err(DeviceFfiError::coded(
            "XCR019",
            format!(
                "upstream producer registration `{path}` {field} {observed} does not match {expected}"
            ),
        ));
    }
    Ok(())
}

fn expect_static_string(
    tcx: TyCtxt<'_>,
    def_id: DefId,
    index: usize,
    field: &str,
    expected: &str,
    path: &str,
) -> Result<(), DeviceFfiError> {
    let observed = crate::static_registration::string(tcx, def_id, index).map_err(|reason| {
        DeviceFfiError::coded(
            "XCR019",
            format!("upstream producer registration `{path}` {field} is invalid: {reason}"),
        )
    })?;
    if observed != expected {
        return Err(DeviceFfiError::coded(
            "XCR019",
            format!(
                "upstream producer registration `{path}` {field} `{observed}` does not match `{expected}`"
            ),
        ));
    }
    Ok(())
}

fn validate_upstream_registration_type(
    tcx: TyCtxt<'_>,
    def_id: DefId,
    path: &str,
) -> Result<(), DeviceFfiError> {
    let ty = tcx.type_of(def_id).instantiate_identity();
    let TyKind::Tuple(fields) = ty.kind() else {
        return Err(DeviceFfiError::coded(
            "XCR014",
            format!("upstream producer registration `{path}` must use the exact V1 tuple type"),
        ));
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
        return Err(DeviceFfiError::coded(
            "XCR014",
            format!("upstream producer registration `{path}` must use the exact V1 tuple type"),
        ));
    }
    Ok(())
}

fn static_tuple_fields<'a, 'tcx>(
    body: &'a Body<'tcx>,
    expected_fields: usize,
    path: &str,
) -> Result<Vec<&'a Operand<'tcx>>, DeviceFfiError> {
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
                return Err(DeviceFfiError::coded(
                    "XCR015",
                    format!("cross-crate device anchor `{path}` contains multiple tuple values"),
                ));
            }
        }
    }
    let fields = aggregate.ok_or_else(|| {
        DeviceFfiError::coded(
            "XCR015",
            format!("cross-crate device anchor `{path}` has no tuple initializer"),
        )
    })?;
    if fields.len() != expected_fields {
        return Err(DeviceFfiError::coded(
            "XCR015",
            format!(
                "cross-crate device anchor `{path}` has {} fields; expected {expected_fields}",
                fields.len()
            ),
        ));
    }
    Ok(fields.iter().collect())
}

fn expect_mir_integer<'tcx>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    expected_ty: Ty<'tcx>,
    expected: u128,
    field: &str,
    path: &str,
) -> Result<(), DeviceFfiError> {
    let Operand::Constant(constant) = operand else {
        return Err(DeviceFfiError::coded(
            "XCR016",
            format!("cross-crate device anchor `{path}` {field} is not a constant"),
        ));
    };
    if constant.const_.ty() != expected_ty
        || constant
            .const_
            .try_eval_bits(tcx, TypingEnv::fully_monomorphized())
            != Some(expected)
    {
        return Err(DeviceFfiError::coded(
            "XCR016",
            format!("cross-crate device anchor `{path}` {field} is not canonical"),
        ));
    }
    Ok(())
}

fn mir_string<'tcx>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    field: &str,
    path: &str,
) -> Result<String, DeviceFfiError> {
    let Operand::Constant(constant) = operand else {
        return Err(DeviceFfiError::coded(
            "XCR017",
            format!("cross-crate device anchor `{path}` {field} is not a string constant"),
        ));
    };
    if !is_shared_str(constant.const_.ty()) {
        return Err(DeviceFfiError::coded(
            "XCR017",
            format!("cross-crate device anchor `{path}` {field} has the wrong type"),
        ));
    }
    let value = constant
        .const_
        .eval(tcx, TypingEnv::fully_monomorphized(), constant.span)
        .map_err(|_| {
            DeviceFfiError::coded(
                "XCR017",
                format!("cross-crate device anchor `{path}` {field} could not be evaluated"),
            )
        })?;
    let bytes = value
        .try_get_slice_bytes_for_diagnostics(tcx)
        .ok_or_else(|| {
            DeviceFfiError::coded(
                "XCR017",
                format!("cross-crate device anchor `{path}` {field} is not string data"),
            )
        })?;
    std::str::from_utf8(bytes).map(str::to_owned).map_err(|_| {
        DeviceFfiError::coded(
            "XCR017",
            format!("cross-crate device anchor `{path}` {field} is not UTF-8"),
        )
    })
}

fn mir_function_target<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    operand: &Operand<'tcx>,
    path: &str,
) -> Result<Instance<'tcx>, DeviceFfiError> {
    let place = match operand {
        Operand::Copy(place) | Operand::Move(place) => place,
        Operand::Constant(constant) => {
            return constant_device_function_target(tcx, constant, path);
        }
        Operand::RuntimeChecks(_) => {
            return Err(DeviceFfiError::coded(
                "XCR018",
                format!("cross-crate device anchor `{path}` must use one exact function pointer"),
            ));
        }
    };
    let Some(target_local) = place.as_local() else {
        return Err(DeviceFfiError::coded(
            "XCR018",
            format!("cross-crate device anchor `{path}` uses a projected function pointer"),
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
                return Err(DeviceFfiError::coded(
                    "XCR018",
                    format!("cross-crate device anchor `{path}` function is indirect"),
                ));
            };
            let TyKind::FnDef(def_id, args) = source.const_.ty().kind() else {
                return Err(DeviceFfiError::coded(
                    "XCR018",
                    format!("cross-crate device anchor `{path}` does not name a function item"),
                ));
            };
            let resolved =
                Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, args)
                    .ok()
                    .flatten()
                    .ok_or_else(|| {
                        DeviceFfiError::coded(
                            "XCR018",
                            format!("cross-crate device anchor `{path}` function did not resolve"),
                        )
                    })?;
            if target.replace(resolved).is_some() {
                return Err(DeviceFfiError::coded(
                    "XCR018",
                    format!("cross-crate device anchor `{path}` has multiple function targets"),
                ));
            }
        }
    }
    target.ok_or_else(|| {
        DeviceFfiError::coded(
            "XCR018",
            format!("cross-crate device anchor `{path}` has no exact function target"),
        )
    })
}

fn constant_device_function_target<'tcx>(
    tcx: TyCtxt<'tcx>,
    constant: &ConstOperand<'tcx>,
    path: &str,
) -> Result<Instance<'tcx>, DeviceFfiError> {
    if !matches!(constant.const_.ty().kind(), TyKind::FnPtr(..)) {
        return Err(DeviceFfiError::coded(
            "XCR018",
            format!("cross-crate device anchor `{path}` target is not a function pointer"),
        ));
    }
    let value = constant
        .const_
        .eval(tcx, TypingEnv::fully_monomorphized(), constant.span)
        .map_err(|_| {
            DeviceFfiError::coded(
                "XCR018",
                format!("cross-crate device anchor `{path}` target did not evaluate"),
            )
        })?;
    let scalar = value.try_to_scalar().ok_or_else(|| {
        DeviceFfiError::coded(
            "XCR018",
            format!("cross-crate device anchor `{path}` target is not one scalar pointer"),
        )
    })?;
    let pointer = scalar
        .to_pointer(&tcx)
        .discard_err()
        .ok_or_else(|| {
            DeviceFfiError::coded(
                "XCR018",
                format!("cross-crate device anchor `{path}` target is not a pointer"),
            )
        })?
        .into_pointer_or_addr()
        .map_err(|_| {
            DeviceFfiError::coded(
                "XCR018",
                format!("cross-crate device anchor `{path}` target has no provenance"),
            )
        })?;
    let (provenance, offset) = pointer.into_raw_parts();
    if offset.bytes() != 0 {
        return Err(DeviceFfiError::coded(
            "XCR018",
            format!("cross-crate device anchor `{path}` target has a nonzero offset"),
        ));
    }
    match tcx.global_alloc(provenance.alloc_id()) {
        GlobalAlloc::Function { instance } => Ok(instance),
        _ => Err(DeviceFfiError::coded(
            "XCR018",
            format!("cross-crate device anchor `{path}` target is not a function allocation"),
        )),
    }
}

pub(crate) fn contract_for_instance<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    expected_target: &str,
) -> Result<Option<DeviceFfiContract>, DeviceFfiError> {
    let def_id = instance.def_id();
    let Some(contract) = contract_assertion_for_def(tcx, def_id)? else {
        return Ok(None);
    };
    if !def_id.is_local() {
        if authenticate_upstream_export(tcx, instance, &contract)? {
            return validate_contract_for_instance(tcx, instance, expected_target, contract)
                .map(Some);
        }
        return Err(DeviceFfiError::coded(
            "AUTH001",
            format!(
                "upstream marker on `{}` is assertion-only and has no authenticated V1 producer registration evidence",
                tcx.def_path_str(def_id)
            ),
        ));
    }
    validate_contract_for_instance(tcx, instance, expected_target, contract).map(Some)
}

/// Parses a source marker assertion without granting it producer authority.
/// Callers constructing local evidence must additionally validate the exact
/// registration static; upstream assertions cannot become V1 closure entries.
pub(crate) fn contract_assertion_for_def(
    tcx: TyCtxt<'_>,
    def_id: DefId,
) -> Result<Option<DeviceFfiContract>, DeviceFfiError> {
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
    parse_marker(&markers[0]).map(Some).map_err(|error| {
        DeviceFfiError::new(format!(
            "declaration `{}`: {}",
            tcx.def_path_str(def_id),
            error.reason
        ))
    })
}

fn validate_contract_for_instance<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    expected_target: &str,
    contract: DeviceFfiContract,
) -> Result<DeviceFfiContract, DeviceFfiError> {
    let def_id = instance.def_id();
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

    Ok(contract)
}

fn validate_export_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: Instance<'tcx>,
) -> Result<(), DeviceFfiError> {
    const MAX_REACHABLE_FUNCTIONS: usize = 4_096;
    let mut pending = vec![root];
    let mut seen = BTreeSet::new();

    while let Some(instance) = pending.pop() {
        let identity = stable_instance_identity(tcx, instance);
        if !seen.insert(identity) {
            continue;
        }
        if seen.len() > MAX_REACHABLE_FUNCTIONS {
            return Err(DeviceFfiError::new(
                "device export reachable-function bound exceeded",
            ));
        }
        if !tcx.is_mir_available(instance.def_id()) {
            return Err(DeviceFfiError::coded(
                "EDGE004",
                format!(
                    "device export `{}` reaches `{}` without traversable MIR",
                    tcx.def_path_str(root.def_id()),
                    tcx.def_path_str(instance.def_id())
                ),
            ));
        }
        let body = tcx.instance_mir(instance.def);
        let dead_branches = crate::monomorphization_dead::CompilerDeadBranchObservationV1::observe(
            tcx, instance, body,
        )
        .map_err(|error| {
            DeviceFfiError::new(format!(
                "device export `{}` dead-branch observation failed closed: {error}",
                tcx.def_path_str(instance.def_id())
            ))
        })?;
        if let Some(static_def_id) = referenced_static(tcx, body) {
            return Err(DeviceFfiError::new(format!(
                "device export `{}` reaches static `{}` requiring unsupported relocation handling",
                tcx.def_path_str(instance.def_id()),
                tcx.def_path_str(static_def_id),
            )));
        }
        for (block_id, block) in body.basic_blocks.iter_enumerated() {
            if !dead_branches.includes_block(block_id.as_usize()) {
                continue;
            }
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
                TerminatorKind::Call { func, unwind, .. } => {
                    // `Continue` propagates rather than naming an executable
                    // cleanup block. The resolved callee is added to the
                    // closed traversal below, where panic/unwind MIR fails.
                    if !matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable) {
                        return Err(DeviceFfiError::coded(
                            "EDGE002",
                            format!(
                                "device export `{}` contains an untraversed call unwind edge `{unwind:?}`",
                                tcx.def_path_str(instance.def_id())
                            ),
                        ));
                    }
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
                        if !resolved.def_id().is_local() {
                            return Err(DeviceFfiError::coded(
                                "AUTH001",
                                format!(
                                    "upstream marker on `{path}` is assertion-only and has no authenticated V1 producer registration evidence"
                                ),
                            ));
                        }
                        continue;
                    }
                    if resolved.args.has_param() || resolved.args.has_escaping_bound_vars() {
                        return Err(DeviceFfiError::new(format!(
                            "device export call `{path}` is not fully monomorphized"
                        )));
                    }
                    pending.push(resolved);
                }
                TerminatorKind::Goto { .. }
                | TerminatorKind::SwitchInt { .. }
                | TerminatorKind::Return
                | TerminatorKind::Unreachable => {}
                unsupported => {
                    return Err(DeviceFfiError::coded(
                        "EDGE001",
                        format!(
                            "device export `{}` contains unsupported executable MIR edge `{unsupported:?}`",
                            tcx.def_path_str(instance.def_id())
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn referenced_static<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> Option<DefId> {
    struct StaticReferenceVisitor<'tcx> {
        tcx: TyCtxt<'tcx>,
        referenced: Option<DefId>,
    }

    impl<'tcx> Visitor<'tcx> for StaticReferenceVisitor<'tcx> {
        fn visit_const_operand(&mut self, constant: &ConstOperand<'tcx>, _location: Location) {
            if self.referenced.is_none() {
                self.referenced = constant.check_static_ptr(self.tcx);
            }
        }
    }

    let mut visitor = StaticReferenceVisitor {
        tcx,
        referenced: None,
    };
    visitor.visit_body(body);
    visitor.referenced
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
    if signature.inputs().len() > MAX_DEVICE_FFI_ARGUMENTS_V1 {
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
    let direction = parse_device_ffi_direction_v1(fields[0])
        .map_err(|error| DeviceFfiError::new(error.to_string()))?;
    let id = DeviceFfiContractIdV1::from_hex(fields[1])
        .map_err(|error| DeviceFfiError::new(format!("invalid contract identity: {error}")))?;
    validate_device_ffi_symbol_v1(fields[2])
        .map_err(|error| DeviceFfiError::new(error.to_string()))?;
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
    let physical_abi = parse_device_ffi_physical_abi_v1(fields[6])
        .map_err(|error| DeviceFfiError::new(error.to_string()))?;
    let effects = parse_device_ffi_effects_v1(fields[7])
        .map_err(|error| DeviceFfiError::new(error.to_string()))?;
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
    validate_device_ffi_effect_abi_v1(&effects, &physical_abi)
        .map_err(|error| DeviceFfiError::new(error.to_string()))?;
    Ok(DeviceFfiContract {
        id,
        direction,
        symbol: fields[2].to_owned(),
        target: fields[5].to_owned(),
        code_object_version_assertion: AssertionOnly::new(code_object_version),
        physical_abi: fields[6].to_owned(),
        effects_assertion: AssertionOnly::new(fields[7].to_owned()),
        semantic_identity_assertion: AssertionOnly::new(fields[8].to_owned()),
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
            .then_with(|| lhs.owner.cmp(&rhs.owner))
    });
    let closed = declarations
        .iter()
        .map(|declaration| ClosedDeviceFfiContract {
            contract: declaration.contract.clone(),
            owner: declaration.owner.clone(),
            link_role_assertion: AssertionOnly::new(match declaration.contract.direction {
                DeviceFfiDirection::Import => DeviceFfiLinkRole::RequiresExternalDefinition,
                DeviceFfiDirection::Export => DeviceFfiLinkRole::RequiresCompilerModuleDefinition,
            }),
        })
        .collect::<Vec<_>>();
    validate_source_bindings(&closed)?;
    validate_contract_values(declarations.iter().map(|declaration| &declaration.contract))
}

fn validate_contract_values<'a>(
    contracts: impl IntoIterator<Item = &'a DeviceFfiContract>,
) -> Result<(), DeviceFfiError> {
    let mut ids = BTreeSet::new();
    let mut symbols: BTreeMap<&str, &DeviceFfiContract> = BTreeMap::new();
    let mut semantics: BTreeMap<&str, &DeviceFfiContract> = BTreeMap::new();
    let mut target = None;
    let mut code_object_version = None;
    for contract in contracts {
        if contract
            .symbol
            .starts_with(reserved_fe2o3_symbols::KERNEL_PREFIX)
            || contract.symbol.starts_with("__fe2o3_")
        {
            return Err(DeviceFfiError::new(format!(
                "device FFI symbol `{}` uses a compiler-reserved namespace",
                contract.symbol
            )));
        }
        if !ids.insert(contract.id) {
            return Err(DeviceFfiError::new(format!(
                "duplicate contract identity {}",
                contract.id.to_hex()
            )));
        }
        if let Some(previous) = symbols.insert(&contract.symbol, contract) {
            if previous.direction == contract.direction && previous == contract {
                return Err(DeviceFfiError::new(format!(
                    "duplicate {:?} declaration for symbol `{}`",
                    contract.direction, contract.symbol
                )));
            }
            return Err(DeviceFfiError::new(format!(
                "conflicting {:?}/{:?} declarations for symbol `{}`",
                previous.direction, contract.direction, contract.symbol
            )));
        }
        if let Some(previous) = semantics.insert(
            contract
                .semantic_identity_assertion
                .asserted_for_consistency_check(),
            contract,
        ) && (previous.symbol != contract.symbol
            || previous.direction != contract.direction
            || previous.physical_abi != contract.physical_abi
            || previous.effects_assertion != contract.effects_assertion)
        {
            return Err(DeviceFfiError::new(format!(
                "semantic identity `{}` is claimed by incompatible symbols `{}` and `{}`",
                contract
                    .semantic_identity_assertion
                    .asserted_for_consistency_check(),
                previous.symbol,
                contract.symbol
            )));
        }
        if let Some(previous) = target.replace(contract.target.as_str())
            && previous != contract.target
        {
            return Err(DeviceFfiError::new(format!(
                "device FFI closure mixes targets `{previous}` and `{}`",
                contract.target
            )));
        }
        let asserted_code_object_version = *contract
            .code_object_version_assertion
            .asserted_for_consistency_check();
        if let Some(previous) = code_object_version.replace(asserted_code_object_version)
            && previous != asserted_code_object_version
        {
            return Err(DeviceFfiError::new(format!(
                "device FFI closure mixes code-object versions `{previous}` and `{}`",
                asserted_code_object_version
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_local_closure<'tcx>(
    tcx: TyCtxt<'tcx>,
    declarations: &mut Vec<CollectedDeviceFfi<'tcx>>,
    reachable_imports: &BTreeSet<DeviceFfiContractIdV1>,
) -> Result<DeviceFfiClosure, DeviceFfiError> {
    enforce_contract_bound(declarations.len())?;
    let local_registrations = local_registrations(tcx)?;
    validate_local_registration_set(declarations, &local_registrations)?;
    validate_contract_set(declarations)?;
    close_contracts(
        declarations
            .iter()
            .map(|declaration| ClosedDeviceFfiContract {
                contract: declaration.contract.clone(),
                owner: declaration.owner.clone(),
                link_role_assertion: AssertionOnly::new(match declaration.contract.direction {
                    DeviceFfiDirection::Import => DeviceFfiLinkRole::RequiresExternalDefinition,
                    DeviceFfiDirection::Export => {
                        DeviceFfiLinkRole::RequiresCompilerModuleDefinition
                    }
                }),
            })
            .collect(),
        reachable_imports,
    )
}

fn close_contracts(
    declarations: Vec<ClosedDeviceFfiContract>,
    reachable_imports: &BTreeSet<DeviceFfiContractIdV1>,
) -> Result<DeviceFfiClosure, DeviceFfiError> {
    // Structural consistency only. Compiler callers must establish source
    // authority in `validate_local_closure` before entering this helper.
    enforce_contract_bound(declarations.len())?;
    validate_source_bindings(&declarations)?;
    validate_contract_values(declarations.iter().map(|entry| &entry.contract))?;

    for declaration in &declarations {
        let expected_role = match declaration.contract.direction {
            DeviceFfiDirection::Import => DeviceFfiLinkRole::RequiresExternalDefinition,
            DeviceFfiDirection::Export => DeviceFfiLinkRole::RequiresCompilerModuleDefinition,
        };
        if declaration
            .link_role_assertion
            .asserted_for_consistency_check()
            != &expected_role
        {
            return Err(DeviceFfiError::new(format!(
                "device FFI `{}` has link role {:?} incompatible with direction {:?}",
                declaration.contract.symbol,
                declaration
                    .link_role_assertion
                    .asserted_for_consistency_check(),
                declaration.contract.direction,
            )));
        }
    }

    let declared_imports = declarations
        .iter()
        .filter(|entry| entry.contract.direction == DeviceFfiDirection::Import)
        .map(|entry| entry.contract.id)
        .collect::<BTreeSet<_>>();
    if let Some(unknown) = reachable_imports.difference(&declared_imports).next() {
        return Err(DeviceFfiError::new(format!(
            "reachable import {} has no collected declaration",
            unknown.to_hex()
        )));
    }

    let mut imports = Vec::new();
    let mut exports = Vec::new();
    for declaration in declarations {
        match declaration.contract.direction {
            DeviceFfiDirection::Import => {
                if !reachable_imports.contains(&declaration.contract.id) {
                    return Err(DeviceFfiError::new(format!(
                        "import `{}` declared by `{}` is host-only or unreachable from the final device graph",
                        declaration.contract.symbol,
                        declaration.owner.label(),
                    )));
                }
                imports.push(declaration);
            }
            DeviceFfiDirection::Export => exports.push(declaration),
        }
    }
    let canonical = |lhs: &ClosedDeviceFfiContract, rhs: &ClosedDeviceFfiContract| {
        lhs.contract
            .symbol
            .cmp(&rhs.contract.symbol)
            .then_with(|| lhs.contract.id.cmp(&rhs.contract.id))
            .then_with(|| lhs.owner.cmp(&rhs.owner))
    };
    imports.sort_by(canonical);
    exports.sort_by(canonical);

    let first = imports.first().or_else(|| exports.first());
    Ok(DeviceFfiClosure {
        target: first.map(|entry| entry.contract.target.clone()),
        code_object_version_assertion: first
            .map(|entry| entry.contract.code_object_version_assertion),
        imports,
        exports,
    })
}

pub(crate) fn enforce_contract_bound(count: usize) -> Result<(), DeviceFfiError> {
    if count > MAX_DEVICE_FFI_CONTRACTS {
        return Err(DeviceFfiError::coded(
            "BOUND001",
            format!(
                "device FFI closure contains {count} contracts; maximum is {MAX_DEVICE_FFI_CONTRACTS}"
            ),
        ));
    }
    Ok(())
}

fn validate_source_bindings(
    declarations: &[ClosedDeviceFfiContract],
) -> Result<(), DeviceFfiError> {
    let mut instance_owners = BTreeMap::new();
    let mut source_items = BTreeMap::new();
    let mut ids = BTreeMap::new();
    let mut symbols: BTreeMap<&str, &ClosedDeviceFfiContract> = BTreeMap::new();
    for declaration in declarations {
        if let Some(previous) = ids.insert(declaration.contract.id, declaration) {
            return Err(DeviceFfiError::new(format!(
                "duplicate device FFI contract {} is owned by both `{}` and `{}`",
                declaration.contract.id.to_hex(),
                previous.owner.label(),
                declaration.owner.label(),
            )));
        }
        if let Some(previous) = symbols.insert(&declaration.contract.symbol, declaration) {
            return Err(DeviceFfiError::new(format!(
                "duplicate or conflicting providers for device FFI symbol `{}` are owned by `{}` and `{}` ({:?} versus {:?})",
                declaration.contract.symbol,
                previous.owner.label(),
                declaration.owner.label(),
                previous.contract.direction,
                declaration.contract.direction,
            )));
        }
        let instance_key = (
            declaration.owner.def_path_hash,
            declaration.owner.concrete_instance_symbol.as_str(),
        );
        if let Some(previous) = instance_owners.insert(instance_key, declaration.owner.label()) {
            return Err(DeviceFfiError::new(format!(
                "device FFI instance `{}` is attributed to both `{previous}` and `{}`",
                declaration.owner.concrete_instance_symbol,
                declaration.owner.label(),
            )));
        }
        if let Some(previous) =
            source_items.insert(declaration.owner.def_path_hash, declaration.contract.id)
        {
            return Err(DeviceFfiError::new(format!(
                "source item `{}` owns multiple device FFI contracts {} and {}",
                declaration.owner.label(),
                previous.to_hex(),
                declaration.contract.id.to_hex(),
            )));
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use reserved_fe2o3_symbols::{DEVICE_FFI_DIRECTION_EXPORT_V1, DEVICE_FFI_DIRECTION_IMPORT_V1};

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

    fn contract(
        direction: DeviceFfiDirection,
        symbol: &str,
        target: &str,
        code_object_version: u16,
        physical_abi: &str,
        effects: &str,
        semantic_byte: u8,
    ) -> DeviceFfiContract {
        let semantic_identity = format!("{semantic_byte:02x}").repeat(32);
        let fields = DeviceFfiContractFieldsV1 {
            direction: direction.tag(),
            symbol,
            calling_convention: "C",
            code_object_version,
            target,
            physical_abi,
            effects,
            semantic_identity: &semantic_identity,
        };
        DeviceFfiContract {
            id: derive_device_ffi_contract_id_v1(fields),
            direction,
            symbol: symbol.to_owned(),
            target: target.to_owned(),
            code_object_version_assertion: AssertionOnly::new(code_object_version),
            physical_abi: physical_abi.to_owned(),
            effects_assertion: AssertionOnly::new(effects.to_owned()),
            semantic_identity_assertion: AssertionOnly::new(semantic_identity),
        }
    }

    fn closed(
        contract: DeviceFfiContract,
        crate_name: &str,
        item_name: &str,
    ) -> ClosedDeviceFfiContract {
        let link_role_assertion = AssertionOnly::new(match contract.direction {
            DeviceFfiDirection::Import => DeviceFfiLinkRole::RequiresExternalDefinition,
            DeviceFfiDirection::Export => DeviceFfiLinkRole::RequiresCompilerModuleDefinition,
        });
        ClosedDeviceFfiContract {
            owner: DeviceFfiSourceOwner {
                crate_name: crate_name.to_owned(),
                item_path: format!("{crate_name}::{item_name}"),
                def_path_hash: fake_def_path_hash(crate_name, item_name),
                concrete_instance_symbol: format!("_R_{crate_name}_{item_name}"),
            },
            contract,
            link_role_assertion,
        }
    }

    fn fake_def_path_hash(crate_name: &str, item_name: &str) -> [u8; 16] {
        let mut hash = [0_u8; 16];
        for (index, byte) in crate_name.bytes().chain(item_name.bytes()).enumerate() {
            hash[index % hash.len()] ^= byte;
        }
        hash
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
    fn marker_direction_requires_exact_canonical_decimal_spelling() {
        let marker = marker(
            DEVICE_FFI_DIRECTION_IMPORT_V1,
            "helper",
            "C()->unit[size=0,align=1]",
        );
        for direction in ["01", "+1", " 1", "1 ", "0", "3"] {
            let malformed = marker.replacen("|1|", &format!("|{direction}|"), 1);
            let error = parse_marker(&malformed).unwrap_err();
            assert!(
                error.reason.contains("noncanonical device FFI direction"),
                "unexpected error for {direction:?}: {error}"
            );
        }
    }

    #[test]
    fn rustc_marker_parser_matches_the_shared_device_ffi_golden_corpus() {
        const CORPUS: &str =
            include_str!("../../reserved-fe2o3-symbols/tests/data/device_ffi_grammar_v1.tsv");
        const SEMANTIC: &str = "1111111111111111111111111111111111111111111111111111111111111111";

        for line in CORPUS.lines().filter(|line| !line.starts_with('#')) {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 7, "malformed corpus row: {line}");
            let [name, expected, direction, symbol, abi, effects, golden] = fields.as_slice()
            else {
                unreachable!("field count was checked")
            };
            let parsed_direction = parse_device_ffi_direction_v1(direction);
            if parsed_direction.is_err() {
                let marker = marker(
                    DEVICE_FFI_DIRECTION_IMPORT_V1,
                    symbol,
                    "C()->unit[size=0,align=1]",
                )
                .replacen("|1|", &format!("|{direction}|"), 1);
                let error = parse_marker(&marker).unwrap_err();
                assert_eq!(*expected, "direction", "{name}");
                assert!(error.reason.contains("noncanonical device FFI direction"));
                continue;
            }
            let direction = parsed_direction.unwrap();
            let contract_fields = DeviceFfiContractFieldsV1 {
                direction: direction.tag(),
                symbol,
                calling_convention: "C",
                code_object_version: 5,
                target: "gfx942",
                physical_abi: abi,
                effects,
                semantic_identity: SEMANTIC,
            };
            let id = derive_device_ffi_contract_id_v1(contract_fields);
            let marker = reserved_fe2o3_symbols::device_ffi_marker_v1(id, contract_fields);
            match parse_marker(&marker) {
                Ok(contract) => {
                    assert_eq!(*expected, "ok", "{name}");
                    assert_eq!(contract.id.to_hex(), *golden, "{name}");
                }
                Err(error) => assert_eq!(
                    rustc_grammar_error_class(&error.reason),
                    *expected,
                    "{name}: {error}"
                ),
            }
        }
    }

    fn rustc_grammar_error_class(message: &str) -> &'static str {
        if message.contains("direction") {
            "direction"
        } else if message.contains("external symbol") {
            "symbol"
        } else if message.contains("physical ABI") {
            "physical_abi"
        } else if message.contains("effects are") {
            "effects"
        } else if message.contains("compatible physical pointer") {
            "effect_abi"
        } else {
            panic!("unexpected rustc grammar diagnostic: {message}")
        }
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
        assert!(
            validate_contract_values([&export, &import])
                .unwrap_err()
                .reason
                .contains("conflicting")
        );

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

    #[test]
    fn abstract_contract_set_is_canonical_and_preserves_source_ownership() {
        let import = closed(
            contract(
                DeviceFfiDirection::Import,
                "external_add",
                "gfx942",
                5,
                "C(u32[size=4,align=4])->u32[size=4,align=4]",
                "none",
                0x11,
            ),
            "consumer",
            "same_logical_name",
        );
        let export_a = closed(
            contract(
                DeviceFfiDirection::Export,
                "export_a",
                "gfx942",
                5,
                "C()->unit[size=0,align=1]",
                "none",
                0x22,
            ),
            "provider_a",
            "same_logical_name",
        );
        let export_b = closed(
            contract(
                DeviceFfiDirection::Export,
                "export_b",
                "gfx942",
                5,
                "C()->unit[size=0,align=1]",
                "none",
                0x33,
            ),
            "provider_b",
            "same_logical_name",
        );
        let reachable = BTreeSet::from([import.contract.id]);

        let first = close_contracts(
            vec![export_b.clone(), import.clone(), export_a.clone()],
            &reachable,
        )
        .unwrap();
        let reordered = close_contracts(vec![export_a, export_b, import], &reachable).unwrap();

        assert_eq!(first, reordered);
        assert_eq!(first.target.as_deref(), Some("gfx942"));
        assert_eq!(
            first
                .code_object_version_assertion
                .as_ref()
                .map(AssertionOnly::asserted_for_consistency_check),
            Some(&5)
        );
        assert_eq!(first.imports[0].owner.crate_name, "consumer");
        assert_eq!(
            first.imports[0]
                .link_role_assertion
                .asserted_for_consistency_check(),
            &DeviceFfiLinkRole::RequiresExternalDefinition
        );
        assert_eq!(first.exports[0].owner.crate_name, "provider_a");
        assert_eq!(
            first.exports[0]
                .link_role_assertion
                .asserted_for_consistency_check(),
            &DeviceFfiLinkRole::RequiresCompilerModuleDefinition
        );
        assert_eq!(first.exports[1].owner.crate_name, "provider_b");
    }

    #[test]
    fn final_closure_reapplies_the_contract_count_bound() {
        let declaration = closed(
            contract(
                DeviceFfiDirection::Export,
                "bounded_export",
                "gfx942",
                5,
                "C()->unit[size=0,align=1]",
                "none",
                0x44,
            ),
            "provider",
            "bounded_export",
        );
        let error = close_contracts(
            vec![declaration; MAX_DEVICE_FFI_CONTRACTS + 1],
            &BTreeSet::new(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("FE2O3-FFI-BOUND001"));
        assert!(error.to_string().contains("129 contracts; maximum is 128"));
    }

    #[test]
    fn host_only_and_unresolved_imports_fail_closed() {
        let import = closed(
            contract(
                DeviceFfiDirection::Import,
                "external_add",
                "gfx942",
                5,
                "C()->unit[size=0,align=1]",
                "none",
                0x11,
            ),
            "consumer",
            "external_add",
        );
        let error = close_contracts(vec![import.clone()], &BTreeSet::new()).unwrap_err();
        assert!(error.reason.contains("host-only or unreachable"));

        let unknown = contract(
            DeviceFfiDirection::Import,
            "missing",
            "gfx942",
            5,
            "C()->unit[size=0,align=1]",
            "none",
            0x22,
        );
        let error = close_contracts(vec![import], &BTreeSet::from([unknown.id])).unwrap_err();
        assert!(error.reason.contains("has no collected declaration"));
    }

    #[test]
    fn target_version_and_semantic_spoofing_fail_closed() {
        let base = contract(
            DeviceFfiDirection::Export,
            "first",
            "gfx942",
            5,
            "C()->unit[size=0,align=1]",
            "none",
            0x11,
        );
        let wrong_target = contract(
            DeviceFfiDirection::Export,
            "second",
            "gfx950",
            5,
            "C()->unit[size=0,align=1]",
            "none",
            0x22,
        );
        assert!(
            validate_contract_values([&base, &wrong_target])
                .unwrap_err()
                .reason
                .contains("mixes targets")
        );

        let wrong_version = contract(
            DeviceFfiDirection::Export,
            "second",
            "gfx942",
            6,
            "C()->unit[size=0,align=1]",
            "none",
            0x22,
        );
        assert!(
            validate_contract_values([&base, &wrong_version])
                .unwrap_err()
                .reason
                .contains("mixes code-object versions")
        );

        let mut spoofed = contract(
            DeviceFfiDirection::Export,
            "second",
            "gfx942",
            5,
            "C(u32[size=4,align=4])->unit[size=0,align=1]",
            "none",
            0x22,
        );
        spoofed.semantic_identity_assertion = base.semantic_identity_assertion.clone();
        assert!(
            validate_contract_values([&base, &spoofed])
                .unwrap_err()
                .reason
                .contains("semantic identity")
        );
    }

    #[test]
    fn same_symbol_contract_mismatches_and_link_role_swaps_fail_closed() {
        let base = contract(
            DeviceFfiDirection::Export,
            "shared_symbol",
            "gfx942",
            5,
            "C()->unit[size=0,align=1]",
            "none",
            0x11,
        );
        let variants = [
            contract(
                DeviceFfiDirection::Import,
                "shared_symbol",
                "gfx942",
                5,
                "C()->unit[size=0,align=1]",
                "none",
                0x11,
            ),
            contract(
                DeviceFfiDirection::Export,
                "shared_symbol",
                "gfx942",
                5,
                "C(u32[size=4,align=4])->unit[size=0,align=1]",
                "none",
                0x11,
            ),
            contract(
                DeviceFfiDirection::Export,
                "shared_symbol",
                "gfx942",
                5,
                "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]",
                "write_global",
                0x11,
            ),
            contract(
                DeviceFfiDirection::Export,
                "shared_symbol",
                "gfx950",
                5,
                "C()->unit[size=0,align=1]",
                "none",
                0x11,
            ),
            contract(
                DeviceFfiDirection::Export,
                "shared_symbol",
                "gfx942",
                6,
                "C()->unit[size=0,align=1]",
                "none",
                0x11,
            ),
            contract(
                DeviceFfiDirection::Export,
                "shared_symbol",
                "gfx942",
                5,
                "C()->unit[size=0,align=1]",
                "none",
                0x22,
            ),
        ];
        for variant in &variants {
            let error = validate_contract_values([&base, variant]).unwrap_err();
            assert!(
                error.reason.contains("conflicting"),
                "unexpected mismatch diagnostic: {error}"
            );
        }

        let mut swapped = closed(
            contract(
                DeviceFfiDirection::Import,
                "external_add",
                "gfx942",
                5,
                "C()->unit[size=0,align=1]",
                "none",
                0x33,
            ),
            "consumer",
            "external_add",
        );
        swapped.link_role_assertion =
            AssertionOnly::new(DeviceFfiLinkRole::RequiresCompilerModuleDefinition);
        let reachable = BTreeSet::from([swapped.contract.id]);
        assert!(
            close_contracts(vec![swapped], &reachable)
                .unwrap_err()
                .reason
                .contains("link role")
        );
    }

    #[test]
    fn duplicate_abstract_providers_fail_with_stable_ownership() {
        let first = closed(
            contract(
                DeviceFfiDirection::Export,
                "duplicate_provider",
                "gfx942",
                5,
                "C()->unit[size=0,align=1]",
                "none",
                0x11,
            ),
            "provider_a",
            "same_logical_name",
        );
        let second = closed(
            contract(
                DeviceFfiDirection::Export,
                "duplicate_provider",
                "gfx942",
                5,
                "C()->unit[size=0,align=1]",
                "none",
                0x11,
            ),
            "provider_b",
            "same_logical_name",
        );
        let error = close_contracts(vec![second, first], &BTreeSet::new()).unwrap_err();
        assert!(
            error.reason.contains("duplicate device FFI contract")
                && error.reason.contains("provider_a::same_logical_name")
                && error.reason.contains("provider_b::same_logical_name"),
            "unexpected duplicate-provider diagnostic: {error}"
        );
    }

    #[test]
    fn closed_graph_adapts_to_one_complete_neutral_envelope() {
        let import = closed(
            contract(
                DeviceFfiDirection::Import,
                "external_add",
                "gfx942",
                5,
                "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]",
                "read_global",
                0x11,
            ),
            "consumer",
            "external_add",
        );
        let export = closed(
            contract(
                DeviceFfiDirection::Export,
                "rust_helper",
                "gfx942",
                5,
                "C(u32[size=4,align=4])->u32[size=4,align=4]",
                "none",
                0x22,
            ),
            "provider",
            "rust_helper",
        );
        let closure = close_contracts(
            vec![export, import.clone()],
            &BTreeSet::from([import.contract.id]),
        )
        .unwrap();
        let envelope = crate::compiler_ffi_adapter::adapt_closure_v1(&closure, |entry| {
            entry.contract.symbol == "rust_helper"
        })
        .unwrap()
        .unwrap();

        assert_eq!(envelope.target().to_string(), "gfx942");
        assert_eq!(
            envelope.code_object_version(),
            fe2o3_compiler_ffi::CodeObjectVersion::V5
        );
        assert_eq!(envelope.inspection().import_count(), 1);
        assert_eq!(envelope.inspection().export_count(), 1);
        assert_eq!(
            envelope.identity().to_hex(),
            "5c7007cfa776e4a5a5838cb2dd6e055b1bc4a67a05486a1df65fb408c6541165"
        );
        assert!(!envelope.grants_link_authority());

        assert!(matches!(
            crate::compiler_ffi_adapter::adapt_closure_v1(&closure, |_| false),
            Err(
                crate::compiler_ffi_adapter::CompilerFfiAdapterError::ExportMissingFromCollection(
                    symbol
                )
            ) if symbol == "rust_helper"
        ));

        let mut malformed = closure;
        malformed.exports[0].contract.semantic_identity_assertion =
            AssertionOnly::new("AA".repeat(32));
        assert!(matches!(
            crate::compiler_ffi_adapter::adapt_closure_v1(&malformed, |_| true),
            Err(
                crate::compiler_ffi_adapter::CompilerFfiAdapterError::InvalidSemanticIdentity(
                    symbol
                )
            ) if symbol == "rust_helper"
        ));
    }

    #[test]
    fn reserved_symbols_and_effect_address_space_mismatches_fail_closed() {
        let reserved = contract(
            DeviceFfiDirection::Export,
            "fe2o3_kernel_spoofed",
            "gfx942",
            5,
            "C()->unit[size=0,align=1]",
            "none",
            0x11,
        );
        assert!(
            validate_contract_values([&reserved])
                .unwrap_err()
                .reason
                .contains("reserved namespace")
        );

        let abi = "C(mut_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]";
        let marker = marker(DEVICE_FFI_DIRECTION_IMPORT_V1, "external_add", abi).replacen(
            "|none|",
            "|write_workgroup|",
            1,
        );
        assert!(
            parse_marker(&marker)
                .unwrap_err()
                .reason
                .contains("identity")
        );

        let fields = DeviceFfiContractFieldsV1 {
            direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
            symbol: "external_add",
            calling_convention: "C",
            code_object_version: 5,
            target: "gfx942",
            physical_abi: abi,
            effects: "write_workgroup",
            semantic_identity: "1111111111111111111111111111111111111111111111111111111111111111",
        };
        let marker = reserved_fe2o3_symbols::device_ffi_marker_v1(
            derive_device_ffi_contract_id_v1(fields),
            fields,
        );
        assert!(
            parse_marker(&marker)
                .unwrap_err()
                .reason
                .contains("compatible physical pointer")
        );
    }
}
