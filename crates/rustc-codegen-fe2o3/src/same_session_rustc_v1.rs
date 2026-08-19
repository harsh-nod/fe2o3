//! Same-session custody for one ordinary-Rust scalar kernel import.
//!
//! This boundary is intentionally private. It observes typed rustc values while
//! `TyCtxt` is live, authenticates the existing inert frontend import, and then
//! releases only owned fe2o3 records. No rustc arena value crosses the release.

use std::collections::BTreeSet;
use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_rustc_front::{
    AuthenticatedOrdinaryRustScalarKernelImportV1, BasicBlockV1, BlockIdV1,
    CanonicalKernelInstIdV1, CanonicalKernelItemIdV1, ConcreteMonomorphizationIdentityV1,
    DirectCallObservationV1, FrontendSourceSpanV1, FrontendUnitV1, FunctionIdentityV1,
    FunctionImportRoleV1, FunctionRoleV1, MonomorphizedFunctionV1,
    OrdinaryRustScalarKernelObservationV1, ReachableFunctionObservationV1,
    RustItemDefinitionIdentityV1, RustcAbiPassModeV1, RustcAbiValueV1, RustcCallingConventionV1,
    RustcFnAbiFactsV1, RustcFunctionKindV1, RustcMirIdentityV1, RustcSourceIdentityV1,
    SourceFileIdentityV1, SourceLocationV1, StableTypeIdentityV1, TypedSignatureV1,
    authenticate_ordinary_rust_scalar_kernel_v1,
};
use rustc_abi::CanonAbi;
use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_middle::mir::{Body, Operand, TerminatorKind};
use rustc_middle::ty::{
    self, EarlyBinder, GenericArgKind, Instance, InstanceKind, Ty, TyCtxt, TyKind,
    TypeVisitableExt, TypingEnv,
};
use rustc_span::Span;
use rustc_target::callconv::{ArgAbi, PassMode};
use sha2::{Digest as _, Sha256};

use crate::collector::{CollectedFunctionRole, CollectionResult};

const ITEM_CRATE_DOMAIN: &[u8] = b"fe2o3/rustc-session/kernel-crate/v1";
const ITEM_DEFINITION_DOMAIN: &[u8] = b"fe2o3/rustc-session/kernel-item/v1";
const ITEM_GENERIC_DOMAIN: &[u8] = b"fe2o3/rustc-session/kernel-generics/v1";
const INSTANCE_TYPES_DOMAIN: &[u8] = b"fe2o3/rustc-session/kernel-type-args/v1";
const INSTANCE_CONSTS_DOMAIN: &[u8] = b"fe2o3/rustc-session/kernel-const-args/v1";
const INSTANCE_CFG_DOMAIN: &[u8] = b"fe2o3/rustc-session/kernel-cfg/v1";
const FUNCTION_DOMAIN: &[u8] = b"fe2o3/rustc-session/function/v1";
const MONOMORPHIZATION_DOMAIN: &[u8] = b"fe2o3/rustc-session/monomorphization/v1";
const SOURCE_DOMAIN: &[u8] = b"fe2o3/rustc-session/source/v1";
const MIR_DOMAIN: &[u8] = b"fe2o3/rustc-session/optimized-mir/v1";
const CFG_DOMAIN: &[u8] = b"fe2o3/rustc-session/cfg/v1";
const TYPE_DOMAIN: &[u8] = b"fe2o3/rustc-session/type/v1";
const ABI_VALUE_DOMAIN: &[u8] = b"fe2o3/rustc-session/abi-value/v1";
const ABI_CLOSURE_DOMAIN: &[u8] = b"fe2o3/rustc-session/abi-closure/v1";
const CUSTODY_DOMAIN: &[u8] = b"fe2o3/rustc-session/custody/v1";
const MAX_FUNCTIONS: usize = fe2o3_rustc_front::MAX_SCALAR_IMPORT_FUNCTIONS_V1;
const MAX_CALLS: usize = fe2o3_rustc_front::MAX_SCALAR_IMPORT_CALLS_V1;

static NEXT_CUSTODIAN_ID: AtomicU64 = AtomicU64::new(1);

macro_rules! stable_fingerprint {
    ($tcx:expr, $value:expr) => {{
        let fingerprint: Fingerprint = $tcx.with_stable_hashing_context(|mut context| {
            let mut hasher = StableHasher::new();
            ($value).hash_stable(&mut context, &mut hasher);
            hasher.finish()
        });
        fingerprint.to_le_bytes()
    }};
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FunctionSemanticSnapshotV1 {
    function: FunctionIdentityV1,
    monomorphization: ConcreteMonomorphizationIdentityV1,
    mir: RustcMirIdentityV1,
    fn_abi: [u8; 32],
    cfg: [u8; 32],
    blocks: u32,
    edges: u32,
}

#[derive(Debug)]
struct RustKernelSemanticSnapshotV1 {
    functions: Vec<FunctionSemanticSnapshotV1>,
    mir_closure: [u8; 32],
    abi_closure: [u8; 32],
}

/// Owned, inert data released by the private same-session custodian.
///
/// It deliberately has no `Clone` implementation and exposes no rustc-private
/// value, executable callback, or compiler authority.
pub(crate) struct OwnerControlledRustKernelImportV1 {
    imported: AuthenticatedOrdinaryRustScalarKernelImportV1,
    semantic: RustKernelSemanticSnapshotV1,
    custody_binding: [u8; 32],
}

impl OwnerControlledRustKernelImportV1 {
    pub(crate) fn imported(&self) -> &AuthenticatedOrdinaryRustScalarKernelImportV1 {
        &self.imported
    }

    pub(crate) const fn custody_binding(&self) -> &[u8; 32] {
        &self.custody_binding
    }

    pub(crate) fn function_count(&self) -> usize {
        self.semantic.functions.len()
    }

    pub(crate) const fn mir_closure(&self) -> &[u8; 32] {
        &self.semantic.mir_closure
    }

    pub(crate) const fn abi_closure(&self) -> &[u8; 32] {
        &self.semantic.abi_closure
    }

    pub(crate) const fn grants_compiler_authority(&self) -> bool {
        false
    }
}

struct SessionBoundRustKernelImportV1<'tcx> {
    owner: u64,
    imported: AuthenticatedOrdinaryRustScalarKernelImportV1,
    semantic: RustKernelSemanticSnapshotV1,
    observed_item: CanonicalKernelItemIdV1,
    observed_instance: CanonicalKernelInstIdV1,
    observed_mir_closure: [u8; 32],
    observed_abi_closure: [u8; 32],
    custody_binding: [u8; 32],
    invariant_session: PhantomData<fn(&'tcx ()) -> &'tcx ()>,
}

struct RustcSessionCustodianV1<'tcx> {
    owner: u64,
    pending: bool,
    consumed: bool,
    invariant_session: PhantomData<fn(&'tcx ()) -> &'tcx ()>,
}

impl<'tcx> RustcSessionCustodianV1<'tcx> {
    fn new(_tcx: TyCtxt<'tcx>) -> Self {
        let owner = NEXT_CUSTODIAN_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            owner,
            pending: false,
            consumed: false,
            invariant_session: PhantomData,
        }
    }

    fn capture(
        &mut self,
        tcx: TyCtxt<'tcx>,
        collection: &CollectionResult<'tcx>,
    ) -> Result<SessionBoundRustKernelImportV1<'tcx>, SameSessionRustcErrorV1> {
        if self.pending || self.consumed {
            return Err(SameSessionRustcErrorV1::StaleCustodian);
        }
        let captured = capture_in_session(tcx, collection, self.owner)?;
        self.pending = true;
        Ok(captured)
    }

    fn release(
        &mut self,
        captured: SessionBoundRustKernelImportV1<'tcx>,
    ) -> Result<OwnerControlledRustKernelImportV1, SameSessionRustcErrorV1> {
        if self.consumed || !self.pending {
            return Err(SameSessionRustcErrorV1::StaleCustodian);
        }
        if captured.owner != self.owner {
            return Err(SameSessionRustcErrorV1::ForeignCustodian);
        }
        if captured.observed_item != captured.imported.kernel_item() {
            return Err(SameSessionRustcErrorV1::ItemMismatch);
        }
        if captured.observed_instance != captured.imported.kernel_instance() {
            return Err(SameSessionRustcErrorV1::InstanceMismatch);
        }
        if captured.observed_mir_closure != *captured.imported.mir_closure_identity()
            || captured.observed_mir_closure != captured.semantic.mir_closure
        {
            return Err(SameSessionRustcErrorV1::MirMismatch);
        }
        if captured.observed_abi_closure != captured.semantic.abi_closure {
            return Err(SameSessionRustcErrorV1::AbiMismatch);
        }
        let expected = custody_binding(captured.owner, &captured.imported, &captured.semantic);
        if expected != captured.custody_binding {
            return Err(SameSessionRustcErrorV1::CustodyBindingMismatch);
        }
        self.pending = false;
        self.consumed = true;
        Ok(OwnerControlledRustKernelImportV1 {
            imported: captured.imported,
            semantic: captured.semantic,
            custody_binding: captured.custody_binding,
        })
    }
}

/// Synchronously imports one exact scalar kernel closure from the active rustc
/// session and releases no rustc-owned value.
pub(crate) fn import_ordinary_rust_kernel_same_session_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
) -> Result<OwnerControlledRustKernelImportV1, SameSessionRustcErrorV1> {
    let mut custodian = RustcSessionCustodianV1::new(tcx);
    let captured = custodian.capture(tcx, collection)?;
    custodian.release(captured)
}

#[derive(Debug)]
pub(crate) enum SameSessionRustcErrorV1 {
    FunctionBound { actual: usize, maximum: usize },
    CallBound { actual: usize, maximum: usize },
    KernelRootCount { actual: usize },
    UnsupportedFunctionRole,
    UnsupportedInstance,
    MissingMir,
    MissingKernelContract,
    MissingTerminator,
    UnsupportedDirectCall,
    UnknownCallee,
    RustcQuery(&'static str),
    InvalidTypedObservation(&'static str),
    Frontend(fe2o3_rustc_front::ValidationError),
    Source(fe2o3_rustc_front::ControlFlowValidationErrorV1),
    Import(fe2o3_rustc_front::OrdinaryRustScalarValidationErrorV1),
    StaleCustodian,
    ForeignCustodian,
    ItemMismatch,
    InstanceMismatch,
    MirMismatch,
    AbiMismatch,
    CustodyBindingMismatch,
}

impl fmt::Display for SameSessionRustcErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FunctionBound { actual, maximum } => {
                write!(
                    formatter,
                    "same-session function bound exceeded: {actual} > {maximum}"
                )
            }
            Self::CallBound { actual, maximum } => {
                write!(
                    formatter,
                    "same-session call bound exceeded: {actual} > {maximum}"
                )
            }
            Self::KernelRootCount { actual } => {
                write!(
                    formatter,
                    "same-session import requires one kernel root, found {actual}"
                )
            }
            Self::UnsupportedFunctionRole => {
                formatter.write_str("same-session import encountered an unsupported function role")
            }
            Self::UnsupportedInstance => formatter
                .write_str("same-session import requires ordinary monomorphized item instances"),
            Self::MissingMir => {
                formatter.write_str("same-session import could not obtain optimized MIR")
            }
            Self::MissingKernelContract => formatter.write_str(
                "same-session import requires an authenticated kernel frontend contract",
            ),
            Self::MissingTerminator => formatter
                .write_str("same-session optimized MIR contains a block without a terminator"),
            Self::UnsupportedDirectCall => formatter
                .write_str("same-session import encountered a non-direct or unresolved call"),
            Self::UnknownCallee => formatter
                .write_str("same-session import observed a callee outside the collected closure"),
            Self::RustcQuery(query) => write!(formatter, "typed rustc query failed: {query}"),
            Self::InvalidTypedObservation(field) => {
                write!(formatter, "typed rustc observation is invalid: {field}")
            }
            Self::Frontend(error) => {
                write!(formatter, "same-session frontend record rejected: {error}")
            }
            Self::Source(error) => write!(
                formatter,
                "same-session source observation rejected: {error}"
            ),
            Self::Import(error) => write!(
                formatter,
                "same-session ordinary-Rust import rejected: {error}"
            ),
            Self::StaleCustodian => {
                formatter.write_str("same-session custodian is stale or already consumed")
            }
            Self::ForeignCustodian => {
                formatter.write_str("same-session receipt belongs to a different custodian")
            }
            Self::ItemMismatch => formatter.write_str("same-session kernel item identity mismatch"),
            Self::InstanceMismatch => {
                formatter.write_str("same-session kernel instance identity mismatch")
            }
            Self::MirMismatch => {
                formatter.write_str("same-session optimized MIR identity mismatch")
            }
            Self::AbiMismatch => formatter.write_str("same-session FnAbi identity mismatch"),
            Self::CustodyBindingMismatch => {
                formatter.write_str("same-session custody binding mismatch")
            }
        }
    }
}

impl std::error::Error for SameSessionRustcErrorV1 {}

impl From<fe2o3_rustc_front::ValidationError> for SameSessionRustcErrorV1 {
    fn from(value: fe2o3_rustc_front::ValidationError) -> Self {
        Self::Frontend(value)
    }
}

impl From<fe2o3_rustc_front::ControlFlowValidationErrorV1> for SameSessionRustcErrorV1 {
    fn from(value: fe2o3_rustc_front::ControlFlowValidationErrorV1) -> Self {
        Self::Source(value)
    }
}

impl From<fe2o3_rustc_front::OrdinaryRustScalarValidationErrorV1> for SameSessionRustcErrorV1 {
    fn from(value: fe2o3_rustc_front::OrdinaryRustScalarValidationErrorV1) -> Self {
        Self::Import(value)
    }
}

struct FunctionDraftV1<'tcx> {
    instance: Instance<'tcx>,
    role: FunctionImportRoleV1,
    frontend: MonomorphizedFunctionV1,
    item: RustItemDefinitionIdentityV1,
    monomorphization: ConcreteMonomorphizationIdentityV1,
    source: RustcSourceIdentityV1,
    mir: RustcMirIdentityV1,
    source_span: FrontendSourceSpanV1,
    fn_abi: RustcFnAbiFactsV1,
    cfg: [u8; 32],
    blocks: u32,
    edges: u32,
}

fn capture_in_session<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    owner: u64,
) -> Result<SessionBoundRustKernelImportV1<'tcx>, SameSessionRustcErrorV1> {
    if collection.functions.len() > MAX_FUNCTIONS {
        return Err(SameSessionRustcErrorV1::FunctionBound {
            actual: collection.functions.len(),
            maximum: MAX_FUNCTIONS,
        });
    }
    let roots = collection
        .functions
        .iter()
        .filter(|function| function.role == CollectedFunctionRole::KernelEntry)
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        return Err(SameSessionRustcErrorV1::KernelRootCount {
            actual: roots.len(),
        });
    }
    let root = roots[0];
    let contract = root
        .frontend_contract
        .as_ref()
        .ok_or(SameSessionRustcErrorV1::MissingKernelContract)?
        .contract();
    let (kernel_item, kernel_instance) = kernel_identities(tcx, root.instance)?;
    let root_monomorphization =
        ConcreteMonomorphizationIdentityV1::for_kernel_instance(kernel_instance);

    let mut drafts = Vec::with_capacity(collection.functions.len());
    for function in &collection.functions {
        let role = match function.role {
            CollectedFunctionRole::KernelEntry => FunctionImportRoleV1::Kernel,
            CollectedFunctionRole::InternalHelper => FunctionImportRoleV1::Helper,
            CollectedFunctionRole::DeviceFfiExport => {
                return Err(SameSessionRustcErrorV1::UnsupportedFunctionRole);
            }
        };
        if !matches!(function.instance.def, InstanceKind::Item(_))
            || function.instance.args.has_non_region_param()
            || function.instance.args.has_infer()
            || function.instance.args.has_escaping_bound_vars()
            || function.instance.args.has_placeholders()
        {
            return Err(SameSessionRustcErrorV1::UnsupportedInstance);
        }
        if !tcx.is_mir_available(function.instance.def_id()) {
            return Err(SameSessionRustcErrorV1::MissingMir);
        }
        let body = tcx.instance_mir(function.instance.def);
        let instance_fingerprint = stable_fingerprint!(tcx, function.instance);
        let function_identity = function_identity(tcx, function.instance)?;
        let monomorphization = if role == FunctionImportRoleV1::Kernel {
            root_monomorphization
        } else {
            monomorphization_identity(tcx, function.instance)?
        };
        let item = item_definition_identity(tcx, function.instance);
        let fn_abi = observe_fn_abi(tcx, function.instance)?;
        let source_span = source_span(tcx, body.span)?;
        let source_crate =
            stable_fingerprint!(tcx, tcx.crate_hash(function.instance.def_id().krate));
        let source = RustcSourceIdentityV1::new(domain_identity(
            SOURCE_DOMAIN,
            &[&source_crate, &stable_fingerprint!(tcx, body.span)],
        ))?;
        let mir = RustcMirIdentityV1::new(domain_identity(
            MIR_DOMAIN,
            &[&instance_fingerprint, &stable_fingerprint!(tcx, body)],
        ))?;
        let (frontend, cfg, blocks, edges) = frontend_function(
            tcx,
            body,
            role,
            function_identity,
            &fn_abi,
            instance_fingerprint,
        )?;
        drafts.push(FunctionDraftV1 {
            instance: function.instance,
            role,
            frontend,
            item,
            monomorphization,
            source,
            mir,
            source_span,
            fn_abi,
            cfg,
            blocks,
            edges,
        });
    }

    let mut total_calls = 0_usize;
    let mut observations = Vec::with_capacity(drafts.len());
    for draft in &drafts {
        let calls = direct_calls(tcx, draft.instance, &drafts)?;
        total_calls = total_calls.saturating_add(calls.len());
        if total_calls > MAX_CALLS {
            return Err(SameSessionRustcErrorV1::CallBound {
                actual: total_calls,
                maximum: MAX_CALLS,
            });
        }
        observations.push(ReachableFunctionObservationV1::new(
            draft.frontend.identity(),
            draft.role,
            draft.item,
            draft.monomorphization,
            draft.source,
            draft.mir,
            draft.source_span.clone(),
            RustcFunctionKindV1::OrdinaryItem,
            true,
            draft.fn_abi.clone(),
            calls,
        )?);
    }

    let frontend =
        FrontendUnitV1::new(drafts.iter().map(|draft| draft.frontend.clone()).collect())?;
    let observation = OrdinaryRustScalarKernelObservationV1::new(
        frontend,
        kernel_item,
        kernel_instance,
        contract,
        observations,
        Vec::new(),
    )?;
    let imported = authenticate_ordinary_rust_scalar_kernel_v1(observation)?;
    let mut functions = drafts
        .iter()
        .map(|draft| FunctionSemanticSnapshotV1 {
            function: draft.frontend.identity(),
            monomorphization: draft.monomorphization,
            mir: draft.mir,
            fn_abi: *draft.fn_abi.identity(),
            cfg: draft.cfg,
            blocks: draft.blocks,
            edges: draft.edges,
        })
        .collect::<Vec<_>>();
    functions.sort_by_key(|function| function.monomorphization);
    let mir_closure = *imported.mir_closure_identity();
    let abi_closure = abi_closure(&functions);
    let semantic = RustKernelSemanticSnapshotV1 {
        functions,
        mir_closure,
        abi_closure,
    };
    let custody_binding = custody_binding(owner, &imported, &semantic);
    Ok(SessionBoundRustKernelImportV1 {
        owner,
        observed_item: kernel_item,
        observed_instance: kernel_instance,
        observed_mir_closure: mir_closure,
        observed_abi_closure: abi_closure,
        imported,
        semantic,
        custody_binding,
        invariant_session: PhantomData,
    })
}

fn kernel_identities<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<(CanonicalKernelItemIdV1, CanonicalKernelInstIdV1), SameSessionRustcErrorV1> {
    let def_id = instance.def_id();
    let def_hash = tcx.def_path_hash(def_id);
    let crate_fingerprint = stable_fingerprint!(tcx, def_hash.stable_crate_id());
    let definition_fingerprint = def_hash.0.to_le_bytes();
    let generics_fingerprint = stable_fingerprint!(tcx, tcx.generics_of(def_id));
    let (type_arguments_identity, const_arguments_identity) = generic_arg_identities(tcx, instance);
    let crate_hash_fingerprint = stable_fingerprint!(tcx, tcx.crate_hash(def_id.krate));
    let canonical_item = CanonicalKernelItemIdV1::from_components(
        domain_identity(ITEM_CRATE_DOMAIN, &[&crate_fingerprint]),
        domain_identity(ITEM_DEFINITION_DOMAIN, &[&definition_fingerprint]),
        domain_identity(ITEM_GENERIC_DOMAIN, &[&generics_fingerprint]),
    )?;
    let canonical_instance = CanonicalKernelInstIdV1::from_components(
        canonical_item,
        type_arguments_identity,
        const_arguments_identity,
        domain_identity(INSTANCE_CFG_DOMAIN, &[&crate_hash_fingerprint]),
    )?;
    Ok((canonical_item, canonical_instance))
}

fn generic_arg_identities(tcx: TyCtxt<'_>, instance: Instance<'_>) -> ([u8; 32], [u8; 32]) {
    let mut types = Sha256::new();
    append_digest_field(&mut types, INSTANCE_TYPES_DOMAIN);
    let mut consts = Sha256::new();
    append_digest_field(&mut consts, INSTANCE_CONSTS_DOMAIN);
    for argument in instance.args {
        match argument.kind() {
            GenericArgKind::Type(ty) => {
                append_digest_field(&mut types, &stable_fingerprint!(tcx, ty));
            }
            GenericArgKind::Const(value) => {
                append_digest_field(&mut consts, &stable_fingerprint!(tcx, value));
            }
            GenericArgKind::Lifetime(_) => {}
        }
    }
    (types.finalize().into(), consts.finalize().into())
}

fn item_definition_identity(
    tcx: TyCtxt<'_>,
    instance: Instance<'_>,
) -> RustItemDefinitionIdentityV1 {
    RustItemDefinitionIdentityV1::new(domain_identity(
        ITEM_DEFINITION_DOMAIN,
        &[&tcx.def_path_hash(instance.def_id()).0.to_le_bytes()],
    ))
    .expect("SHA-256 item identity is nonzero")
}

fn function_identity(
    tcx: TyCtxt<'_>,
    instance: Instance<'_>,
) -> Result<FunctionIdentityV1, SameSessionRustcErrorV1> {
    FunctionIdentityV1::new(domain_identity(
        FUNCTION_DOMAIN,
        &[&stable_fingerprint!(tcx, instance)],
    ))
    .map_err(Into::into)
}

fn monomorphization_identity(
    tcx: TyCtxt<'_>,
    instance: Instance<'_>,
) -> Result<ConcreteMonomorphizationIdentityV1, SameSessionRustcErrorV1> {
    ConcreteMonomorphizationIdentityV1::new(domain_identity(
        MONOMORPHIZATION_DOMAIN,
        &[&stable_fingerprint!(tcx, instance)],
    ))
    .map_err(Into::into)
}

fn stable_type_identity(
    tcx: TyCtxt<'_>,
    ty: Ty<'_>,
) -> Result<StableTypeIdentityV1, SameSessionRustcErrorV1> {
    StableTypeIdentityV1::new(domain_identity(
        TYPE_DOMAIN,
        &[&stable_fingerprint!(tcx, ty)],
    ))
    .map_err(Into::into)
}

fn observe_fn_abi<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<RustcFnAbiFactsV1, SameSessionRustcErrorV1> {
    let query = TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty()));
    let abi = tcx
        .fn_abi_of_instance(query)
        .map_err(|_| SameSessionRustcErrorV1::RustcQuery("fn_abi_of_instance"))?;
    if abi.conv != CanonAbi::Rust {
        return Err(SameSessionRustcErrorV1::InvalidTypedObservation(
            "non-Rust calling convention",
        ));
    }
    let arguments = abi
        .args
        .iter()
        .map(|argument| observe_abi_value(tcx, argument))
        .collect::<Result<Vec<_>, _>>()?;
    let return_value = observe_abi_value(tcx, &abi.ret)?;
    RustcFnAbiFactsV1::new(
        domain_identity(ABI_CLOSURE_DOMAIN, &[&stable_fingerprint!(tcx, abi)]),
        RustcCallingConventionV1::Rust,
        arguments,
        return_value,
        abi.c_variadic,
        abi.can_unwind,
    )
    .map_err(Into::into)
}

fn observe_abi_value<'tcx>(
    tcx: TyCtxt<'tcx>,
    argument: &ArgAbi<'tcx, Ty<'tcx>>,
) -> Result<RustcAbiValueV1, SameSessionRustcErrorV1> {
    let pass_mode = match &argument.mode {
        PassMode::Ignore => RustcAbiPassModeV1::Ignore,
        PassMode::Direct(_) => RustcAbiPassModeV1::Direct,
        PassMode::Cast { .. } => RustcAbiPassModeV1::Cast,
        PassMode::Pair(..) => RustcAbiPassModeV1::Pair,
        PassMode::Indirect { .. } => RustcAbiPassModeV1::Indirect,
    };
    let rust_type = stable_type_identity(tcx, argument.layout.ty)?;
    let size = argument.layout.size.bytes();
    let alignment = argument.layout.align.abi.bytes();
    let mode = [pass_mode as u8];
    let exact_argument = stable_fingerprint!(tcx, argument);
    RustcAbiValueV1::new(
        rust_type,
        domain_identity(
            ABI_VALUE_DOMAIN,
            &[
                rust_type.as_bytes(),
                &size.to_le_bytes(),
                &alignment.to_le_bytes(),
                &mode,
                &exact_argument,
            ],
        ),
        size,
        alignment,
        pass_mode,
    )
    .map_err(Into::into)
}

fn frontend_function<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    role: FunctionImportRoleV1,
    identity: FunctionIdentityV1,
    fn_abi: &RustcFnAbiFactsV1,
    instance_fingerprint: [u8; 16],
) -> Result<(MonomorphizedFunctionV1, [u8; 32], u32, u32), SameSessionRustcErrorV1> {
    let mut blocks = Vec::with_capacity(body.basic_blocks.len());
    let mut cfg_preimage = Vec::with_capacity(body.basic_blocks.len() * 24);
    let mut edge_count = 0_usize;
    for (block_id, block) in body.basic_blocks.iter_enumerated() {
        let terminator = block
            .terminator
            .as_ref()
            .ok_or(SameSessionRustcErrorV1::MissingTerminator)?;
        let successors = terminator
            .successors()
            .map(|successor| successor.as_usize())
            .collect::<BTreeSet<_>>();
        edge_count = edge_count.saturating_add(successors.len());
        let block_index = u32::try_from(block_id.as_usize()).map_err(|_| {
            SameSessionRustcErrorV1::InvalidTypedObservation("CFG block index overflow")
        })?;
        cfg_preimage.extend_from_slice(&block_index.to_le_bytes());
        cfg_preimage.extend_from_slice(
            &u32::try_from(successors.len())
                .map_err(|_| {
                    SameSessionRustcErrorV1::InvalidTypedObservation("CFG successor count overflow")
                })?
                .to_le_bytes(),
        );
        let successors = successors
            .into_iter()
            .map(|successor| {
                let successor = u32::try_from(successor).map_err(|_| {
                    SameSessionRustcErrorV1::InvalidTypedObservation("CFG successor overflow")
                })?;
                cfg_preimage.extend_from_slice(&successor.to_le_bytes());
                Ok(BlockIdV1::new(successor))
            })
            .collect::<Result<Vec<_>, SameSessionRustcErrorV1>>()?;
        let primary_span = block
            .statements
            .first()
            .map_or(terminator.source_info.span, |statement| {
                statement.source_info.span
            });
        blocks.push(BasicBlockV1::new(
            BlockIdV1::new(block_index),
            source_location(tcx, primary_span)?,
            successors,
        )?);
    }
    let parameters = fn_abi
        .arguments()
        .iter()
        .map(RustcAbiValueV1::rust_type)
        .collect();
    let signature = TypedSignatureV1::new(parameters, fn_abi.return_value().rust_type())?;
    let diagnostic_name = match role {
        FunctionImportRoleV1::Kernel => "kernel",
        FunctionImportRoleV1::Helper => "helper",
    };
    let frontend = MonomorphizedFunctionV1::new(
        identity,
        if role == FunctionImportRoleV1::Kernel {
            FunctionRoleV1::Kernel
        } else {
            FunctionRoleV1::Helper
        },
        diagnostic_name,
        source_location(tcx, body.span)?,
        signature,
        BlockIdV1::new(0),
        blocks,
    )?;
    let blocks = u32::try_from(body.basic_blocks.len()).map_err(|_| {
        SameSessionRustcErrorV1::InvalidTypedObservation("CFG block count overflow")
    })?;
    let edges = u32::try_from(edge_count)
        .map_err(|_| SameSessionRustcErrorV1::InvalidTypedObservation("CFG edge count overflow"))?;
    Ok((
        frontend,
        domain_identity(CFG_DOMAIN, &[&instance_fingerprint, &cfg_preimage]),
        blocks,
        edges,
    ))
}

fn direct_calls<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    drafts: &[FunctionDraftV1<'tcx>],
) -> Result<Vec<DirectCallObservationV1>, SameSessionRustcErrorV1> {
    let body = tcx.instance_mir(caller.def);
    let mut calls = Vec::new();
    for block in body.basic_blocks.iter() {
        let terminator = block
            .terminator
            .as_ref()
            .ok_or(SameSessionRustcErrorV1::MissingTerminator)?;
        let TerminatorKind::Call { func, .. } = &terminator.kind else {
            continue;
        };
        let Operand::Constant(constant) = func else {
            return Err(SameSessionRustcErrorV1::UnsupportedDirectCall);
        };
        let TyKind::FnDef(def_id, args) = constant.const_.ty().kind() else {
            return Err(SameSessionRustcErrorV1::UnsupportedDirectCall);
        };
        let args = tcx.instantiate_and_normalize_erasing_regions(
            caller.args,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(*args),
        );
        let resolved = Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, args)
            .map_err(|_| SameSessionRustcErrorV1::UnsupportedDirectCall)?
            .ok_or(SameSessionRustcErrorV1::UnsupportedDirectCall)?;
        let callee = drafts
            .iter()
            .find(|draft| draft.instance == resolved)
            .ok_or(SameSessionRustcErrorV1::UnknownCallee)?;
        calls.push(DirectCallObservationV1::new(
            callee.monomorphization,
            source_span(tcx, terminator.source_info.span)?,
        ));
    }
    Ok(calls)
}

fn source_location(
    tcx: TyCtxt<'_>,
    span: Span,
) -> Result<SourceLocationV1, SameSessionRustcErrorV1> {
    let observation = span_observation(tcx, span)?;
    SourceLocationV1::new(
        SourceFileIdentityV1::new(observation.file_identity)?,
        observation.start_line,
        observation.start_column,
    )
    .map_err(Into::into)
}

fn source_span(
    tcx: TyCtxt<'_>,
    span: Span,
) -> Result<FrontendSourceSpanV1, SameSessionRustcErrorV1> {
    let observation = span_observation(tcx, span)?;
    FrontendSourceSpanV1::new(
        format!("rustc-source:{}", encode_hex(&observation.file_identity)),
        observation.start_line,
        observation.start_column,
        observation.end_line,
        observation.end_column,
    )
    .map_err(Into::into)
}

struct SpanObservationV1 {
    file_identity: [u8; 32],
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
}

fn span_observation(
    tcx: TyCtxt<'_>,
    span: Span,
) -> Result<SpanObservationV1, SameSessionRustcErrorV1> {
    let canonical = span.source_callsite();
    if span.is_dummy() || canonical.is_dummy() {
        return Err(SameSessionRustcErrorV1::InvalidTypedObservation(
            "dummy source span",
        ));
    }
    let source_map = tcx.sess.source_map();
    let start = source_map.lookup_char_pos(canonical.lo());
    let end = source_map.lookup_char_pos(canonical.hi());
    if start.file.stable_id != end.file.stable_id
        || start.line == 0
        || end.line == 0
        || (start.line, start.col.0) > (end.line, end.col.0)
    {
        return Err(SameSessionRustcErrorV1::InvalidTypedObservation(
            "invalid source span",
        ));
    }
    let start_line = u32::try_from(start.line)
        .map_err(|_| SameSessionRustcErrorV1::InvalidTypedObservation("source line overflow"))?;
    let end_line = u32::try_from(end.line)
        .map_err(|_| SameSessionRustcErrorV1::InvalidTypedObservation("source line overflow"))?;
    let start_column = u32::try_from(start.col.0.saturating_add(1))
        .map_err(|_| SameSessionRustcErrorV1::InvalidTypedObservation("source column overflow"))?;
    let end_column = u32::try_from(end.col.0.saturating_add(1))
        .map_err(|_| SameSessionRustcErrorV1::InvalidTypedObservation("source column overflow"))?;
    Ok(SpanObservationV1 {
        file_identity: domain_identity(
            SOURCE_DOMAIN,
            &[&stable_fingerprint!(tcx, start.file.stable_id)],
        ),
        start_line,
        start_column,
        end_line,
        end_column,
    })
}

fn abi_closure(functions: &[FunctionSemanticSnapshotV1]) -> [u8; 32] {
    let mut digest = Sha256::new();
    append_digest_field(&mut digest, ABI_CLOSURE_DOMAIN);
    for function in functions {
        append_digest_field(&mut digest, function.monomorphization.as_bytes());
        append_digest_field(&mut digest, &function.fn_abi);
    }
    digest.finalize().into()
}

fn custody_binding(
    owner: u64,
    imported: &AuthenticatedOrdinaryRustScalarKernelImportV1,
    semantic: &RustKernelSemanticSnapshotV1,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    append_digest_field(&mut digest, CUSTODY_DOMAIN);
    append_digest_field(&mut digest, &owner.to_le_bytes());
    append_digest_field(&mut digest, imported.kernel_item().as_bytes());
    append_digest_field(&mut digest, imported.kernel_instance().as_bytes());
    append_digest_field(&mut digest, imported.import_identity());
    append_digest_field(&mut digest, &semantic.mir_closure);
    append_digest_field(&mut digest, &semantic.abi_closure);
    for function in &semantic.functions {
        append_digest_field(&mut digest, function.function.as_bytes());
        append_digest_field(&mut digest, function.monomorphization.as_bytes());
        append_digest_field(&mut digest, function.mir.as_bytes());
        append_digest_field(&mut digest, &function.fn_abi);
        append_digest_field(&mut digest, &function.cfg);
        append_digest_field(&mut digest, &function.blocks.to_le_bytes());
        append_digest_field(&mut digest, &function.edges.to_le_bytes());
    }
    digest.finalize().into()
}

fn domain_identity(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    append_digest_field(&mut digest, domain);
    for field in fields {
        append_digest_field(&mut digest, field);
    }
    digest.finalize().into()
}

fn append_digest_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    digest.update(bytes);
}

fn encode_hex(bytes: &[u8]) -> String {
    use fmt::Write as _;

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

#[cfg(test)]
mod tests;
