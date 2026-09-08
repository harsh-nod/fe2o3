use fe2o3_artifacts::{BlockSize, Dimensions, LaunchContract};
use fe2o3_kernel_descriptor::MAX_ARGUMENTS_PER_KERNEL;
use fe2o3_rustc_front::{
    ASSEMBLY_OPERAND_ADDRESS_V1, ASSEMBLY_OPERAND_IMMEDIATE_V1, ASSEMBLY_OPERAND_SGPR_V1,
    ASSEMBLY_OPERAND_VGPR_V1, ASSEMBLY_OPTION_NOMEM_V1, ASSEMBLY_OPTION_NOSTACK_V1,
    ASSEMBLY_OPTION_PRESERVES_FLAGS_V1, ASSEMBLY_OPTION_PURE_V1, ASSEMBLY_OPTION_READONLY_V1,
    KERNEL_CONTEXT_FRONTEND_REGISTRATION_KIND_V1, KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1,
    KERNEL_CONTEXT_FRONTEND_REGISTRATION_PREFIX_V1,
    KERNEL_CONTEXT_FRONTEND_REGISTRATION_VERSION_V1, KERNEL_FRONTEND_REGISTRATION_KIND_V1,
    KERNEL_FRONTEND_REGISTRATION_MAGIC_V1, KERNEL_FRONTEND_REGISTRATION_PREFIX_V1,
    KERNEL_FRONTEND_REGISTRATION_VERSION_V1, KERNEL_RESOURCE_REGISTRATION_KIND_V1,
    KERNEL_RESOURCE_REGISTRATION_MAGIC_V1, KERNEL_RESOURCE_REGISTRATION_PREFIX_V1,
    KERNEL_RESOURCE_REGISTRATION_VERSION_V1, KernelContextFrontendContractV1,
    KernelFrontendContractV1, KernelResourceContractV1, decode_kernel_context_frontend_contract_v1,
    decode_kernel_frontend_contract_v1, decode_kernel_resource_contract_v1,
};
use reserved_fe2o3_symbols::{
    CrateBindingIdV1, GeneratedHostContractIdV3, KernelBindingIdV1,
    TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3, derive_crate_binding_id_v1,
    derive_kernel_binding_id_v1, host_kernel_symbol_v1,
};
use rustc_ast::InlineAsmOptions;
use rustc_hir::def::Res;
use rustc_hir::def_id::{DefId, LOCAL_CRATE, LocalDefId};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BlockCheckMode, ExprKind, ItemKind, Mutability, Safety, UnsafeSource};
use rustc_middle::middle::codegen_fn_attrs::CodegenFnAttrFlags;
use rustc_middle::mir::interpret::GlobalAlloc;
use rustc_middle::mir::mono::{CodegenUnit, MonoItem};
use rustc_middle::mir::{
    AggregateKind, Body, CastKind, InlineAsmMacro, InlineAsmOperand, Operand, RETURN_PLACE, Rvalue,
    TerminatorKind, UnwindAction,
};
use rustc_middle::ty::adjustment::PointerCoercion;
use rustc_middle::ty::layout::{LayoutCx, LayoutOf};
use rustc_middle::ty::{
    self, EarlyBinder, GenericArgKind, Instance, InstanceKind, Ty, TyCtxt, TyKind,
    TypeVisitableExt, TypingEnv,
};
use rustc_span::sym;
use rustc_target::callconv::PassMode;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::production_rustc_drop_v1::{ProductionRustcDropClassV1, classify_rustc_drop_v1};

mod production_importer_v1;

pub(crate) use production_importer_v1::{
    AuthenticatedProductionKernelContextsV1, AuthenticatedRustcIdentityInventoryV3,
    AuthenticatedRustcPreflightPlanV3, ConstructedProductionSemanticMirV1,
    ProductionSemanticImportErrorV1, construct_production_semantic_mir_v1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypedArgumentListV1<T> {
    arguments: Vec<T>,
}

impl<T> TypedArgumentListV1<T> {
    pub(crate) fn new(arguments: Vec<T>) -> Result<Self, TypedArgumentListError> {
        if arguments.is_empty() {
            return Err(TypedArgumentListError::Empty);
        }
        if arguments.len() > MAX_ARGUMENTS_PER_KERNEL {
            return Err(TypedArgumentListError::TooMany {
                actual: arguments.len(),
                maximum: MAX_ARGUMENTS_PER_KERNEL,
            });
        }
        Ok(Self { arguments })
    }

    pub(crate) fn as_slice(&self) -> &[T] {
        &self.arguments
    }

    pub(crate) fn len(&self) -> usize {
        self.arguments.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypedArgumentListError {
    Empty,
    TooMany { actual: usize, maximum: usize },
}

impl fmt::Display for TypedArgumentListError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("typed kernel argument list must not be empty"),
            Self::TooMany { actual, maximum } => write!(
                formatter,
                "typed kernel argument count {actual} exceeds maximum {maximum}"
            ),
        }
    }
}

/// Compiler-authoritative role attached when a function enters the closed
/// device graph. Downstream consumers must preserve it rather than reconstruct
/// it from symbol spelling or later reachability analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CollectedFunctionRole {
    KernelEntry,
    InternalHelper,
    DeviceFfiExport,
}

#[derive(Clone, Debug)]
pub struct CollectedFunction<'tcx> {
    pub instance: Instance<'tcx>,
    pub(crate) role: CollectedFunctionRole,
    pub export_name: String,
    /// Present only for registered kernel roots.
    pub(crate) logical_name: Option<String>,
    /// Present only for a generic typed V3 registration.
    pub(crate) generated_host_contract_identity: Option<GeneratedHostContractIdV3>,
    pub(crate) kernel_binding: Option<KernelBindingIdV1>,
    /// Compiler-authenticated source contract for this exact kernel root.
    pub(crate) frontend_contract: Option<AuthenticatedKernelFrontendContractV1>,
    /// Compiler-bound declaration for one logical context argument.
    pub(crate) kernel_context_contract: Option<BoundKernelContextFrontendContractV1>,
    /// Exact safe-Rust reference/effect binding for this kernel root.
    pub(crate) reference_effect_binding:
        Option<crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1>,
    /// Compiler-private observation derived from this exact monomorphized MIR.
    pub(crate) dead_branches: Option<crate::monomorphization_dead::CompilerDeadBranchObservationV1>,
    pub(crate) closure_plan: Option<crate::closure_profile_v1::Gfx942ClosureLoweringV1>,
}

/// Source-level kernel contract authenticated against one exact rustc instance.
///
/// This is compiler evidence only. It grants no code-generation, proof, load,
/// or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedKernelFrontendContractV1 {
    registration_path: String,
    target_def_path_hash: [u8; 16],
    target_symbol: String,
    canonical_bytes: Vec<u8>,
    contract: KernelFrontendContractV1,
    resource: Option<AuthenticatedKernelResourceContractV1>,
    reachable_assembly: ReachableAssemblySummaryV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AuthenticatedKernelResourceContractV1 {
    canonical_bytes: Vec<u8>,
    contract: KernelResourceContractV1,
}

/// Inert context metadata bound to an exact physical root. Full graph
/// authentication occurs after reachable MIR collection and before this value
/// may reach the semantic importer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BoundKernelContextFrontendContractV1 {
    registration_path: String,
    canonical_bytes: Vec<u8>,
    contract: KernelContextFrontendContractV1,
    authenticated_source: Option<AuthenticatedKernelContextSourceV1>,
}

impl BoundKernelContextFrontendContractV1 {
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub(crate) fn contract(&self) -> &KernelContextFrontendContractV1 {
        &self.contract
    }

    pub(crate) fn take_authenticated_source(
        &mut self,
    ) -> Option<AuthenticatedKernelContextSourceV1> {
        self.authenticated_source.take()
    }
}

/// Exact rustc facts retained only after the context sidecar, nominal marker,
/// physical ABI, and unique issuance call have all been authenticated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedKernelContextSourceV1 {
    root_function_identity: [u8; 32],
    kernel_marker_identity: [u8; 32],
    issuance_identity: [u8; 32],
    physical_argument_count: u32,
    logical_argument_count: u32,
}

impl AuthenticatedKernelContextSourceV1 {
    pub(crate) const fn root_function_identity(&self) -> [u8; 32] {
        self.root_function_identity
    }

    pub(crate) const fn kernel_marker_identity(&self) -> [u8; 32] {
        self.kernel_marker_identity
    }

    pub(crate) const fn issuance_identity(&self) -> [u8; 32] {
        self.issuance_identity
    }

    pub(crate) const fn physical_argument_count(&self) -> u32 {
        self.physical_argument_count
    }

    pub(crate) const fn logical_argument_count(&self) -> u32 {
        self.logical_argument_count
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ReachableAssemblySummaryV1 {
    blocks: u32,
    operand_bits: u16,
    option_bits: u16,
}

impl AuthenticatedKernelFrontendContractV1 {
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub(crate) const fn contract(&self) -> KernelFrontendContractV1 {
        self.contract
    }

    pub(crate) const fn resource_contract(&self) -> Option<KernelResourceContractV1> {
        match &self.resource {
            Some(resource) => Some(resource.contract),
            None => None,
        }
    }

    pub(crate) fn resource_canonical_bytes(&self) -> Option<&[u8]> {
        self.resource
            .as_ref()
            .map(|resource| resource.canonical_bytes.as_slice())
    }

    pub(crate) const fn reachable_assembly(&self) -> ReachableAssemblySummaryV1 {
        self.reachable_assembly
    }

    #[cfg(test)]
    pub(crate) fn for_test(contract: KernelFrontendContractV1) -> Self {
        Self {
            registration_path: "tests::__fe2o3_kernel_frontend_contract_v1_kernel".to_owned(),
            target_def_path_hash: [0x5a; 16],
            target_symbol: "fe2o3_kernel_kernel".to_owned(),
            canonical_bytes: fe2o3_rustc_front::encode_kernel_frontend_contract_v1(contract),
            contract,
            resource: None,
            reachable_assembly: ReachableAssemblySummaryV1::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test_with_resource(
        contract: KernelFrontendContractV1,
        resource: KernelResourceContractV1,
    ) -> Self {
        let mut authenticated = Self::for_test(contract);
        authenticated.resource = Some(AuthenticatedKernelResourceContractV1 {
            canonical_bytes: fe2o3_rustc_front::encode_kernel_resource_contract_v1(resource),
            contract: resource,
        });
        authenticated
    }
}

impl ReachableAssemblySummaryV1 {
    pub(crate) const fn blocks(self) -> u32 {
        self.blocks
    }

    pub(crate) const fn operand_bits(self) -> u16 {
        self.operand_bits
    }

    pub(crate) const fn option_bits(self) -> u16 {
        self.option_bits
    }
}

#[derive(Clone, Debug, Default)]
pub struct CollectionResult<'tcx> {
    pub functions: Vec<CollectedFunction<'tcx>>,
    // Private source state retained for compiler-envelope construction.
    pub(crate) device_ffi: crate::device_ffi::DeviceFfiClosure,
    /// Inert canonical observation produced from the successfully closed graph.
    pub(crate) compiler_ffi_observation: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
}

#[derive(Debug)]
struct AuthenticatedProductionRootV1<'tcx> {
    instance: Instance<'tcx>,
    role: CollectedFunctionRole,
    export_name: String,
}

/// Private move-only custody returned only by selector-independent production
/// collection. The semantic importer must consume this value while `TyCtxt`
/// remains live.
pub(crate) struct AuthenticatedCollectedKernelClosureV1<'tcx> {
    target: crate::production_target_v1::RetainedProductionTargetV1,
    collection: CollectionResult<'tcx>,
    roots: Box<[AuthenticatedProductionRootV1<'tcx>]>,
}

impl<'tcx> AuthenticatedCollectedKernelClosureV1<'tcx> {
    pub(crate) fn function_count(&self) -> usize {
        self.collection.functions.len()
    }

    pub(crate) fn compiler_ffi_observation(
        &self,
    ) -> Option<&fe2o3_compiler_ffi::CompilerFfiEnvelopeV1> {
        self.collection.compiler_ffi_observation.as_ref()
    }

    /// Re-derives typed descriptor roots while the collector-sealed rustc
    /// instances are still live. No retained identity is accepted without
    /// independently repeating the rustc layout extraction.
    pub(crate) fn rederive_typed_descriptor_roots(
        &self,
        tcx: TyCtxt<'tcx>,
    ) -> Result<
        Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
        crate::compiler_descriptor::CompilerDescriptorError,
    > {
        crate::compiler_descriptor::typed_descriptor_roots_from_production_collection(
            tcx,
            &self.collection.functions,
        )
    }

    pub(crate) fn exact_codegen_function_symbols_v1(
        &self,
    ) -> impl ExactSizeIterator<Item = (Instance<'tcx>, CollectedFunctionRole, &str)> {
        self.collection.functions.iter().map(|function| {
            (
                function.instance,
                function.role,
                function.export_name.as_str(),
            )
        })
    }
}

impl CollectedFunction<'_> {
    pub(crate) fn is_kernel_entry(&self) -> bool {
        self.role == CollectedFunctionRole::KernelEntry
    }
}

#[derive(Debug)]
enum CollectDecision {
    Collect,
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

pub fn count_kernels_in_cgus<'tcx>(tcx: TyCtxt<'tcx>, cgus: &[CodegenUnit<'tcx>]) -> usize {
    registration_candidates(tcx).len()
        + crate::device_ffi::count_exports_in_cgus(tcx, cgus)
            .max(crate::device_ffi::count_local_registration_candidates(tcx))
}

/// Counts every supported production root using HIR/static registration facts
/// only, before rustc monomorphization or MIR collection is entered.
pub(crate) fn count_production_roots_before_monomorphization_v1(tcx: TyCtxt<'_>) -> usize {
    registration_candidates(tcx)
        .len()
        .saturating_add(crate::device_ffi::count_local_registration_candidates(tcx))
}

pub(crate) fn collect_authenticated_kernel_closure_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    cgus: &[CodegenUnit<'tcx>],
    verbose: bool,
    target: crate::production_target_v1::RetainedProductionTargetV1,
) -> Result<AuthenticatedCollectedKernelClosureV1<'tcx>, CollectError> {
    let collection =
        collect_device_functions(tcx, cgus, verbose, target.canonical_name().to_owned())?;
    let roots = collection
        .functions
        .iter()
        .filter(|function| {
            matches!(
                function.role,
                CollectedFunctionRole::KernelEntry | CollectedFunctionRole::DeviceFfiExport
            )
        })
        .map(|function| AuthenticatedProductionRootV1 {
            instance: function.instance,
            role: function.role,
            export_name: function.export_name.clone(),
        })
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return Err(CollectError {
            message: "production compilation collected no authenticated external device root"
                .to_owned(),
        });
    }
    Ok(AuthenticatedCollectedKernelClosureV1 {
        target,
        collection,
        roots: roots.into_boxed_slice(),
    })
}

fn collect_device_functions<'tcx>(
    tcx: TyCtxt<'tcx>,
    cgus: &[CodegenUnit<'tcx>],
    verbose: bool,
    target: String,
) -> Result<CollectionResult<'tcx>, CollectError> {
    let ffi_declarations =
        crate::device_ffi::collect_declarations(tcx, cgus).map_err(|error| CollectError {
            message: error.to_string(),
        })?;
    let ffi_exports = ffi_declarations
        .iter()
        .filter(|declaration| {
            declaration.contract.direction == crate::device_ffi::DeviceFfiDirection::Export
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut collector = DeviceCollector::new(tcx, verbose, ffi_declarations, target);

    for declaration in ffi_exports {
        if declaration.contract.direction == crate::device_ffi::DeviceFfiDirection::Export {
            if verbose {
                eprintln!(
                    "[collector] standalone device export: {} -> {} ({})",
                    tcx.def_path_str(declaration.instance.def_id()),
                    declaration.contract.symbol,
                    declaration.contract.id.to_hex(),
                );
            }
            collector.add_device_export(declaration.instance, declaration.contract.symbol)?;
        }
    }

    for root in kernel_roots(tcx, cgus).map_err(CollectError::from)? {
        let instance = root.target;
        let raw_name = tcx.def_path_str(instance.def_id());
        if verbose {
            eprintln!(
                "[collector] root kernel: {raw_name} -> {}",
                root.export_name
            );
        }
        collector.add_root(root)?;
    }

    collector.collect()
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
    crate_binding: Option<CrateBindingIdV1>,
    kernel_binding: Option<KernelBindingIdV1>,
    profile_tag: Option<String>,
    generated_host_contract_identity: Option<GeneratedHostContractIdV3>,
    target_crate_name: String,
    target_symbol: String,
    target_identity: String,
    target: T,
}

#[derive(Clone, Debug)]
struct KernelRoot<T> {
    target: T,
    logical_name: String,
    export_name: String,
    generated_host_contract_identity: Option<GeneratedHostContractIdV3>,
    kernel_binding: Option<KernelBindingIdV1>,
    frontend_contract: Option<AuthenticatedKernelFrontendContractV1>,
    kernel_context_contract: Option<BoundKernelContextFrontendContractV1>,
    reference_effect_binding:
        Option<crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1>,
}

#[derive(Clone, Debug)]
struct FrontendContractRegistrationRecord<T> {
    registration_path: String,
    item_name: String,
    magic: u64,
    version: u16,
    kind: u16,
    logical_name: String,
    canonical_bytes: Vec<u8>,
    contract: KernelFrontendContractV1,
    target_symbol: String,
    target_identity: String,
    target: T,
}

#[derive(Clone, Debug)]
struct KernelContextContractRegistrationRecord<T> {
    registration_path: String,
    item_name: String,
    magic: u64,
    version: u16,
    kind: u16,
    logical_name: String,
    canonical_bytes: Vec<u8>,
    contract: KernelContextFrontendContractV1,
    target_identity: String,
    target: T,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObservedKernelContextIssuanceV1 {
    rustc_block: u32,
    terminal_function_identity: [u8; 32],
}

const KERNEL_CONTEXT_ISSUANCE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3/rustc/kernel-context-issuance/v1";

fn derive_kernel_context_issuance_identity_v1(
    root_function_identity: &[u8; 32],
    mir_body_identity: &[u8; 32],
    block_identity: &[u8; 32],
    terminal_function_identity: &[u8; 32],
    kernel_marker_identity: &[u8; 32],
) -> [u8; 32] {
    crate::rustc_semantic_adapter_v1::domain_digest(
        KERNEL_CONTEXT_ISSUANCE_IDENTITY_DOMAIN_V1,
        &[
            root_function_identity,
            mir_body_identity,
            block_identity,
            terminal_function_identity,
            kernel_marker_identity,
        ],
    )
}

#[derive(Clone, Debug)]
struct ResourceContractRegistrationRecord<T> {
    registration_path: String,
    item_name: String,
    magic: u64,
    version: u16,
    kind: u16,
    logical_name: String,
    canonical_bytes: Vec<u8>,
    contract: KernelResourceContractV1,
    target_symbol: String,
    target_identity: String,
    target: T,
}

#[derive(Clone, Debug)]
struct ReferenceBindingRegistrationRecord<T> {
    registration_path: String,
    item_name: String,
    logical_name: String,
    kernel: T,
    reference: T,
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

    let session_crate_name = tcx.crate_name(LOCAL_CRATE).to_string();
    let mut roots = validate_registration_records(
        records,
        session_crate_binding(tcx),
        Some(&session_crate_name),
    )?;
    let frontend_records = decode_frontend_contract_registrations(tcx, &functions_by_symbol)?;
    bind_frontend_contract_registrations(tcx, &mut roots, frontend_records)?;
    let context_records = decode_kernel_context_contract_registrations(tcx, &functions_by_symbol)?;
    bind_kernel_context_contract_registrations(tcx, &mut roots, context_records)?;
    let resource_records = decode_resource_contract_registrations(tcx, &functions_by_symbol)?;
    bind_resource_contract_registrations(tcx, &mut roots, resource_records)?;
    let reference_records = decode_reference_binding_registrations(tcx)?;
    bind_reference_binding_registrations(tcx, &mut roots, reference_records)?;
    Ok(roots)
}

fn general_typed_launch_v3(
    frontend: Option<&AuthenticatedKernelFrontendContractV1>,
    registration_path: &str,
) -> Result<LaunchContract, RegistrationError> {
    const DEFAULT: [u32; 3] = [256, 1, 1];
    const MAX_THREADS: u32 = 256;

    let (dimensions, max_grid) = match frontend.and_then(|frontend| frontend.contract().launch()) {
        None => (DEFAULT, [u32::MAX, 1, 1]),
        Some(launch) => {
            if launch.min_workgroups_per_compute_unit().is_some() {
                return Err(RegistrationError::new(
                    registration_path,
                    "general typed V3 does not support launch occupancy constraints",
                ));
            }
            let Some(required) = launch.required() else {
                return Err(RegistrationError::new(
                    registration_path,
                    "general typed V3 explicit launch requires required dimensions",
                ));
            };
            let required = required.as_array();
            if launch
                .maximum()
                .is_some_and(|maximum| required != maximum.as_array())
            {
                return Err(RegistrationError::new(
                    registration_path,
                    "general typed V3 explicit launch requires identical required and maximum dimensions",
                ));
            }
            if required[0] == 0 || required[0] > MAX_THREADS || required[1] != 1 || required[2] != 1
            {
                return Err(RegistrationError::new(
                    registration_path,
                    "general typed V3 requires exact [N, 1, 1] launch dimensions with N in 1..=256",
                ));
            }
            (
                required,
                launch
                    .max_grid()
                    .map(|dimensions| dimensions.as_array())
                    .unwrap_or([u32::MAX, 1, 1]),
            )
        }
    };
    let resources = frontend
        .and_then(AuthenticatedKernelFrontendContractV1::resource_contract)
        .map(|resource| {
            (
                resource.static_shared_memory_bytes(),
                resource.max_dynamic_shared_memory_bytes(),
            )
        })
        .unwrap_or_default();
    LaunchContract::new(
        1,
        BlockSize::Exact(
            Dimensions::new(dimensions[0], dimensions[1], dimensions[2])
                .map_err(|error| RegistrationError::new(registration_path, error.to_string()))?,
        ),
        Dimensions::new(max_grid[0], max_grid[1], max_grid[2])
            .map_err(|error| RegistrationError::new(registration_path, error.to_string()))?,
        resources.0,
        resources.1,
    )
    .map_err(|error| RegistrationError::new(registration_path, error.to_string()))
}

pub(crate) fn rederive_general_typed_launch_for_descriptor_v1(
    frontend: Option<&AuthenticatedKernelFrontendContractV1>,
    kernel: &str,
) -> Result<LaunchContract, String> {
    general_typed_launch_v3(frontend, kernel).map_err(|error| error.to_string())
}

fn encode_lower_hex(bytes: &[u8]) -> String {
    use fmt::Write as _;
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

fn session_crate_binding(tcx: TyCtxt<'_>) -> Option<CrateBindingIdV1> {
    let metadata = &tcx.sess.opts.cg.metadata;
    if metadata.is_empty() {
        return None;
    }
    let crate_name = tcx.crate_name(LOCAL_CRATE);
    Some(derive_crate_binding_id_v1(
        crate_name.as_str(),
        metadata.iter().map(String::as_str),
    ))
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

fn frontend_contract_candidates<'tcx>(
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
                .starts_with(KERNEL_FRONTEND_REGISTRATION_PREFIX_V1)
                .then_some((path, item_name, def_id, item))
        })
        .collect()
}

fn kernel_context_contract_candidates<'tcx>(
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
                .starts_with(KERNEL_CONTEXT_FRONTEND_REGISTRATION_PREFIX_V1)
                .then_some((path, item_name, def_id, item))
        })
        .collect()
}

fn resource_contract_candidates<'tcx>(
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
                .starts_with(KERNEL_RESOURCE_REGISTRATION_PREFIX_V1)
                .then_some((path, item_name, def_id, item))
        })
        .collect()
}

fn reference_binding_candidates<'tcx>(
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
                .starts_with(reserved_fe2o3_symbols::REFERENCE_BINDING_REGISTRATION_PREFIX_V1)
                .then_some((path, item_name, def_id, item))
        })
        .collect()
}

fn decode_reference_binding_registrations<'tcx>(
    tcx: TyCtxt<'tcx>,
) -> Result<Vec<ReferenceBindingRegistrationRecord<Instance<'tcx>>>, RegistrationError> {
    let mut candidates = reference_binding_candidates(tcx);
    candidates.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    candidates
        .into_iter()
        .map(|(path, item_name, def_id, item)| {
            decode_reference_binding_registration(tcx, def_id, path, item_name, item)
        })
        .collect()
}

fn decode_reference_binding_registration<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: rustc_hir::def_id::LocalDefId,
    registration_path: String,
    item_name: String,
    item: &rustc_hir::Item<'tcx>,
) -> Result<ReferenceBindingRegistrationRecord<Instance<'tcx>>, RegistrationError> {
    if !matches!(item.kind, ItemKind::Static(..)) {
        return Err(RegistrationError::new(
            registration_path,
            "reserved reference-binding name must identify a static item",
        ));
    }
    if tcx.is_mutable_static(def_id.to_def_id()) {
        return Err(RegistrationError::new(
            registration_path,
            "reference-binding static must be immutable",
        ));
    }
    let flags = tcx.codegen_fn_attrs(def_id).flags;
    if !flags.intersects(CodegenFnAttrFlags::USED_COMPILER | CodegenFnAttrFlags::USED_LINKER) {
        return Err(RegistrationError::new(
            registration_path,
            "reference-binding static must carry #[used]",
        ));
    }
    let registration_ty = tcx
        .try_normalize_erasing_regions(
            TypingEnv::fully_monomorphized(),
            tcx.type_of(def_id).instantiate_identity(),
        )
        .map_err(|_| {
            RegistrationError::new(
                &registration_path,
                "reference-binding type did not normalize",
            )
        })?;
    let TyKind::Tuple(types) = registration_ty.kind() else {
        return Err(RegistrationError::new(
            registration_path,
            "reference binding must use the exact V1 tuple type",
        ));
    };
    let exact_type = types.len()
        == reserved_fe2o3_symbols::REFERENCE_BINDING_REGISTRATION_FIELD_COUNT_V1
        && types[0] == tcx.types.u64
        && types[1] == tcx.types.u16
        && types[2] == tcx.types.u16
        && is_shared_str(types[3])
        && matches!(types[4].kind(), TyKind::FnPtr(..))
        && matches!(types[5].kind(), TyKind::FnPtr(..));
    if !exact_type {
        return Err(RegistrationError::new(
            registration_path,
            "reference binding must be (u64, u16, u16, &str, kernel fn pointer, fn())",
        ));
    }
    let body = tcx.mir_for_ctfe(def_id);
    let fields = registration_tuple_fields(
        body,
        reserved_fe2o3_symbols::REFERENCE_BINDING_REGISTRATION_FIELD_COUNT_V1,
        &registration_path,
    )?;
    let magic = registration_integer(tcx, fields[0], tcx.types.u64, "magic", &registration_path)?;
    let version =
        registration_integer(tcx, fields[1], tcx.types.u16, "version", &registration_path)?;
    let kind = registration_integer(tcx, fields[2], tcx.types.u16, "kind", &registration_path)?;
    if magic != u128::from(reserved_fe2o3_symbols::REFERENCE_BINDING_REGISTRATION_MAGIC_V1)
        || version != u128::from(reserved_fe2o3_symbols::REFERENCE_BINDING_REGISTRATION_VERSION_V1)
        || kind != u128::from(reserved_fe2o3_symbols::REFERENCE_BINDING_REGISTRATION_KIND_V1)
    {
        return Err(RegistrationError::new(
            registration_path,
            "reference-binding magic, version, or kind is not canonical V1",
        ));
    }
    let logical_name = registration_string(tcx, fields[3], "logical name", &registration_path)?;
    let kernel = registration_target(tcx, body, fields[4], &registration_path)?;
    let anchor = registration_target(tcx, body, fields[5], &registration_path)?;
    let reference = reference_target_from_anchor_v1(tcx, anchor, &registration_path)?;
    Ok(ReferenceBindingRegistrationRecord {
        registration_path,
        item_name,
        logical_name,
        kernel,
        reference,
    })
}

fn reference_target_from_anchor_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    anchor: Instance<'tcx>,
    registration_path: &str,
) -> Result<Instance<'tcx>, RegistrationError> {
    if anchor.def_id().as_local().is_none() {
        return Err(RegistrationError::new(
            registration_path,
            "reference anchor must be a local generated function",
        ));
    };
    let expected_prefix = "__fe2o3_kernel_reference_anchor_v1_";
    let anchor_path = tcx.def_path_str(anchor.def_id());
    if !final_path_segment(&anchor_path).starts_with(expected_prefix) {
        return Err(RegistrationError::new(
            registration_path,
            "reference anchor does not use the compiler-reserved generated name",
        ));
    }
    let body = tcx.instance_mir(anchor.def);
    let mut candidates = body
        .local_decls
        .iter()
        .filter_map(|declaration| {
            let TyKind::FnDef(def_id, arguments) = declaration.ty.kind() else {
                return None;
            };
            Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, arguments)
                .ok()
                .flatten()
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|instance| {
        (
            tcx.def_path_hash(instance.def_id()).0,
            tcx.symbol_name(*instance).name.to_string(),
        )
    });
    candidates.dedup();
    let [reference] = candidates.as_slice() else {
        return Err(RegistrationError::new(
            registration_path,
            format!(
                "reference anchor must name exactly one resolvable function item; found {}",
                candidates.len(),
            ),
        ));
    };
    if !is_fully_monomorphized(tcx, *reference) {
        return Err(RegistrationError::new(
            registration_path,
            "reference function is not fully monomorphized",
        ));
    }
    Ok(*reference)
}

fn bind_reference_binding_registrations<'tcx>(
    tcx: TyCtxt<'tcx>,
    roots: &mut [KernelRoot<Instance<'tcx>>],
    records: Vec<ReferenceBindingRegistrationRecord<Instance<'tcx>>>,
) -> Result<(), RegistrationError> {
    let roots_by_name = roots
        .iter()
        .enumerate()
        .map(|(index, root)| (root.logical_name.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut binding_counts = BTreeMap::new();
    for record in &records {
        *binding_counts
            .entry(record.logical_name.as_str())
            .or_insert(0usize) += 1;
    }
    if let Some((logical_name, _)) = binding_counts.iter().find(|(_, count)| **count > 1) {
        let duplicate = records
            .iter()
            .find(|record| record.logical_name == *logical_name)
            .expect("duplicate count came from one binding record");
        return Err(RegistrationError::new(
            &duplicate.registration_path,
            "duplicate safe Rust reference binding for one kernel",
        ));
    }
    for record in records {
        let expected_name = format!(
            "{}{}",
            reserved_fe2o3_symbols::REFERENCE_BINDING_REGISTRATION_PREFIX_V1,
            record.logical_name,
        );
        if record.item_name != expected_name {
            return Err(RegistrationError::new(
                record.registration_path,
                "reference-binding item name disagrees with its logical kernel name",
            ));
        }
        let Some(&index) = roots_by_name.get(&record.logical_name) else {
            return Err(RegistrationError::new(
                record.registration_path,
                "orphan safe Rust reference binding has no registered kernel",
            ));
        };
        let root = &mut roots[index];
        if root.target != record.kernel {
            return Err(RegistrationError::new(
                record.registration_path,
                "safe Rust reference binding does not point at the exact registered kernel instance",
            ));
        }
        let binding = crate::reference_effect_v1::authenticate_reference_binding_v1(
            tcx,
            record.registration_path.clone(),
            record.logical_name,
            record.kernel,
            record.reference,
        )
        .map_err(|error| RegistrationError::new(&record.registration_path, error.to_string()))?;
        root.reference_effect_binding = Some(binding);
    }
    Ok(())
}

fn decode_frontend_contract_registrations<'tcx>(
    tcx: TyCtxt<'tcx>,
    functions_by_symbol: &BTreeMap<String, Vec<Instance<'tcx>>>,
) -> Result<Vec<FrontendContractRegistrationRecord<Instance<'tcx>>>, RegistrationError> {
    let mut candidates = frontend_contract_candidates(tcx);
    candidates.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));

    candidates
        .into_iter()
        .map(|(path, item_name, def_id, item)| {
            decode_frontend_contract_registration(
                tcx,
                def_id,
                path,
                item_name,
                item,
                functions_by_symbol,
            )
        })
        .collect()
}

fn decode_frontend_contract_registration<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: rustc_hir::def_id::LocalDefId,
    registration_path: String,
    item_name: String,
    item: &rustc_hir::Item<'tcx>,
    functions_by_symbol: &BTreeMap<String, Vec<Instance<'tcx>>>,
) -> Result<FrontendContractRegistrationRecord<Instance<'tcx>>, RegistrationError> {
    if !matches!(item.kind, ItemKind::Static(..)) {
        return Err(RegistrationError::new(
            registration_path,
            "the reserved frontend-contract name must identify a static item",
        ));
    }
    if tcx.is_mutable_static(def_id.to_def_id()) {
        return Err(RegistrationError::new(
            registration_path,
            "frontend-contract registration statics must be immutable",
        ));
    }
    let flags = tcx.codegen_fn_attrs(def_id).flags;
    if !flags.intersects(CodegenFnAttrFlags::USED_COMPILER | CodegenFnAttrFlags::USED_LINKER) {
        return Err(RegistrationError::new(
            registration_path,
            "frontend-contract registration statics must carry #[used]",
        ));
    }

    let registration_ty = tcx.type_of(def_id).instantiate_identity();
    let TyKind::Tuple(field_types) = registration_ty.kind() else {
        return Err(RegistrationError::new(
            registration_path,
            "frontend-contract registration must use the exact V1 tuple type",
        ));
    };
    let exact_type = field_types.len() == 6
        && field_types[0] == tcx.types.u64
        && field_types[1] == tcx.types.u16
        && field_types[2] == tcx.types.u16
        && is_shared_str(field_types[3])
        && is_shared_u8_slice(field_types[4])
        && matches!(field_types[5].kind(), TyKind::FnPtr(..));
    if !exact_type {
        return Err(RegistrationError::new(
            registration_path,
            "frontend-contract registration type must be `(u64, u16, u16, &str, &[u8], fn pointer)`",
        ));
    }

    let body = tcx.mir_for_ctfe(def_id);
    let fields = registration_tuple_fields(body, 6, &registration_path)?;
    let magic = registration_integer(tcx, fields[0], tcx.types.u64, "magic", &registration_path)?;
    let version =
        registration_integer(tcx, fields[1], tcx.types.u16, "version", &registration_path)?;
    let kind = registration_integer(tcx, fields[2], tcx.types.u16, "kind", &registration_path)?;
    let logical_name = registration_string(tcx, fields[3], "logical name", &registration_path)?;
    let canonical_bytes = registration_bytes(tcx, fields[4], "contract", &registration_path)?;
    let contract = decode_kernel_frontend_contract_v1(&canonical_bytes).map_err(|error| {
        RegistrationError::new(
            &registration_path,
            format!("frontend-contract bytes are invalid: {error}"),
        )
    })?;
    let target = registration_target(tcx, body, fields[5], &registration_path)?;
    let target_symbol = tcx.symbol_name(target).name.to_string();
    let target_identity = tcx.def_path_str(target.def_id());
    let Some(cgu_targets) = functions_by_symbol.get(&target_symbol) else {
        return Err(RegistrationError::new(
            registration_path,
            format!(
                "frontend-contract target `{target_symbol}` was not monomorphized into a codegen unit"
            ),
        ));
    };
    if cgu_targets.as_slice() != [target] {
        return Err(RegistrationError::new(
            registration_path,
            format!("frontend-contract target `{target_symbol}` is ambiguous or inconsistent"),
        ));
    }

    Ok(FrontendContractRegistrationRecord {
        registration_path,
        item_name,
        magic: u64::try_from(magic)
            .map_err(|_| RegistrationError::new("frontend contract", "magic does not fit u64"))?,
        version: u16::try_from(version)
            .map_err(|_| RegistrationError::new("frontend contract", "version does not fit u16"))?,
        kind: u16::try_from(kind)
            .map_err(|_| RegistrationError::new("frontend contract", "kind does not fit u16"))?,
        logical_name,
        canonical_bytes,
        contract,
        target_symbol,
        target_identity,
        target,
    })
}

fn bind_frontend_contract_registrations<'tcx>(
    tcx: TyCtxt<'tcx>,
    roots: &mut [KernelRoot<Instance<'tcx>>],
    mut records: Vec<FrontendContractRegistrationRecord<Instance<'tcx>>>,
) -> Result<(), RegistrationError> {
    records.sort_by(|lhs, rhs| lhs.registration_path.cmp(&rhs.registration_path));
    let roots_by_name = roots
        .iter()
        .enumerate()
        .map(|(index, root)| (root.logical_name.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut targets = BTreeMap::new();

    for record in records {
        let error = |reason| RegistrationError::new(record.registration_path.clone(), reason);
        if record.magic != KERNEL_FRONTEND_REGISTRATION_MAGIC_V1 {
            return Err(error(format!(
                "frontend-contract magic {:#018x} does not match {:#018x}",
                record.magic, KERNEL_FRONTEND_REGISTRATION_MAGIC_V1
            )));
        }
        if record.version != KERNEL_FRONTEND_REGISTRATION_VERSION_V1 {
            return Err(error(format!(
                "unknown frontend-contract registration version {}",
                record.version
            )));
        }
        if record.kind != KERNEL_FRONTEND_REGISTRATION_KIND_V1 {
            return Err(error(format!(
                "unknown frontend-contract registration kind {}",
                record.kind
            )));
        }
        if record.logical_name.is_empty() {
            return Err(error(
                "frontend-contract logical name must not be empty".to_owned(),
            ));
        }
        let expected_item_name = format!(
            "{KERNEL_FRONTEND_REGISTRATION_PREFIX_V1}{}",
            record.logical_name
        );
        if record.item_name != expected_item_name {
            return Err(error(format!(
                "frontend-contract item name `{}` is inconsistent with logical name `{}`",
                record.item_name, record.logical_name
            )));
        }
        let Some(&root_index) = roots_by_name.get(&record.logical_name) else {
            return Err(error(format!(
                "orphan frontend contract has no registered kernel `{}`",
                record.logical_name
            )));
        };
        let root = &mut roots[root_index];
        if root.target != record.target {
            return Err(error(format!(
                "frontend-contract target `{}` is not the exact registered kernel function",
                record.target_identity
            )));
        }
        if root.frontend_contract.is_some() {
            return Err(error(format!(
                "duplicate frontend contract for kernel `{}`",
                record.logical_name
            )));
        }
        if let Some(previous) = targets.insert(
            record.target_identity.clone(),
            record.registration_path.clone(),
        ) {
            return Err(error(format!(
                "duplicate frontend-contract target `{}`; first registered by `{previous}`",
                record.target_identity
            )));
        }
        if (record.contract.unsafe_assembly().is_some()
            || record.contract.unsafe_raw_memory_provider())
            && tcx.fn_sig(record.target.def_id()).skip_binder().safety() != Safety::Unsafe
        {
            return Err(error(if record.contract.unsafe_assembly().is_some() {
                "unsafe-assembly contracts require an unsafe registered kernel function".to_owned()
            } else {
                "unsafe raw-memory provider contracts require an unsafe registered kernel function"
                    .to_owned()
            }));
        }

        root.frontend_contract = Some(AuthenticatedKernelFrontendContractV1 {
            registration_path: record.registration_path,
            target_def_path_hash: tcx.def_path_hash(record.target.def_id()).0.to_le_bytes(),
            target_symbol: record.target_symbol,
            canonical_bytes: record.canonical_bytes,
            contract: record.contract,
            resource: None,
            reachable_assembly: ReachableAssemblySummaryV1::default(),
        });
    }
    Ok(())
}

fn decode_kernel_context_contract_registrations<'tcx>(
    tcx: TyCtxt<'tcx>,
    functions_by_symbol: &BTreeMap<String, Vec<Instance<'tcx>>>,
) -> Result<Vec<KernelContextContractRegistrationRecord<Instance<'tcx>>>, RegistrationError> {
    let mut candidates = kernel_context_contract_candidates(tcx);
    candidates.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    candidates
        .into_iter()
        .map(|(path, item_name, def_id, item)| {
            decode_kernel_context_contract_registration(
                tcx,
                def_id,
                path,
                item_name,
                item,
                functions_by_symbol,
            )
        })
        .collect()
}

fn decode_kernel_context_contract_registration<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: rustc_hir::def_id::LocalDefId,
    registration_path: String,
    item_name: String,
    item: &rustc_hir::Item<'tcx>,
    functions_by_symbol: &BTreeMap<String, Vec<Instance<'tcx>>>,
) -> Result<KernelContextContractRegistrationRecord<Instance<'tcx>>, RegistrationError> {
    if !matches!(item.kind, ItemKind::Static(..)) {
        return Err(RegistrationError::new(
            registration_path,
            "the reserved kernel-context contract name must identify a static item",
        ));
    }
    if tcx.is_mutable_static(def_id.to_def_id()) {
        return Err(RegistrationError::new(
            registration_path,
            "kernel-context contract registration statics must be immutable",
        ));
    }
    let flags = tcx.codegen_fn_attrs(def_id).flags;
    if !flags.intersects(CodegenFnAttrFlags::USED_COMPILER | CodegenFnAttrFlags::USED_LINKER) {
        return Err(RegistrationError::new(
            registration_path,
            "kernel-context contract registration statics must carry #[used]",
        ));
    }

    let registration_ty = tcx.type_of(def_id).instantiate_identity();
    let TyKind::Tuple(field_types) = registration_ty.kind() else {
        return Err(RegistrationError::new(
            registration_path,
            "kernel-context contract registration must use the exact V1 tuple type",
        ));
    };
    let exact_type = field_types.len() == 6
        && field_types[0] == tcx.types.u64
        && field_types[1] == tcx.types.u16
        && field_types[2] == tcx.types.u16
        && is_shared_str(field_types[3])
        && is_shared_u8_slice(field_types[4])
        && matches!(field_types[5].kind(), TyKind::FnPtr(..));
    if !exact_type {
        return Err(RegistrationError::new(
            registration_path,
            "kernel-context contract registration type must be `(u64, u16, u16, &str, &[u8], fn pointer)`",
        ));
    }

    let body = tcx.mir_for_ctfe(def_id);
    let fields = registration_tuple_fields(body, 6, &registration_path)?;
    let magic = registration_integer(tcx, fields[0], tcx.types.u64, "magic", &registration_path)?;
    let version =
        registration_integer(tcx, fields[1], tcx.types.u16, "version", &registration_path)?;
    let kind = registration_integer(tcx, fields[2], tcx.types.u16, "kind", &registration_path)?;
    let logical_name = registration_string(tcx, fields[3], "logical name", &registration_path)?;
    let canonical_bytes = registration_bytes(tcx, fields[4], "contract", &registration_path)?;
    let contract =
        decode_kernel_context_frontend_contract_v1(&canonical_bytes).map_err(|error| {
            RegistrationError::new(
                &registration_path,
                format!("kernel-context contract bytes are invalid: {error}"),
            )
        })?;
    let target = registration_target(tcx, body, fields[5], &registration_path)?;
    let target_symbol = tcx.symbol_name(target).name.to_string();
    let target_identity = tcx.def_path_str(target.def_id());
    let Some(cgu_targets) = functions_by_symbol.get(&target_symbol) else {
        return Err(RegistrationError::new(
            registration_path,
            format!(
                "kernel-context contract target `{target_symbol}` was not monomorphized into a codegen unit"
            ),
        ));
    };
    if cgu_targets.as_slice() != [target] {
        return Err(RegistrationError::new(
            registration_path,
            format!(
                "kernel-context contract target `{target_symbol}` is ambiguous or inconsistent"
            ),
        ));
    }

    Ok(KernelContextContractRegistrationRecord {
        registration_path,
        item_name,
        magic: u64::try_from(magic).map_err(|_| {
            RegistrationError::new("kernel context contract", "magic does not fit u64")
        })?,
        version: u16::try_from(version).map_err(|_| {
            RegistrationError::new("kernel context contract", "version does not fit u16")
        })?,
        kind: u16::try_from(kind).map_err(|_| {
            RegistrationError::new("kernel context contract", "kind does not fit u16")
        })?,
        logical_name,
        canonical_bytes,
        contract,
        target_identity,
        target,
    })
}

fn bind_kernel_context_contract_registrations<'tcx>(
    tcx: TyCtxt<'tcx>,
    roots: &mut [KernelRoot<Instance<'tcx>>],
    mut records: Vec<KernelContextContractRegistrationRecord<Instance<'tcx>>>,
) -> Result<(), RegistrationError> {
    records.sort_by(|lhs, rhs| lhs.registration_path.cmp(&rhs.registration_path));
    let roots_by_name = roots
        .iter()
        .enumerate()
        .map(|(index, root)| (root.logical_name.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut targets = BTreeMap::new();

    for record in records {
        let error = |reason| RegistrationError::new(record.registration_path.clone(), reason);
        if record.magic != KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1 {
            return Err(error(format!(
                "kernel-context contract magic {:#018x} does not match {:#018x}",
                record.magic, KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1
            )));
        }
        if record.version != KERNEL_CONTEXT_FRONTEND_REGISTRATION_VERSION_V1 {
            return Err(error(format!(
                "unknown kernel-context contract registration version {}",
                record.version
            )));
        }
        if record.kind != KERNEL_CONTEXT_FRONTEND_REGISTRATION_KIND_V1 {
            return Err(error(format!(
                "unknown kernel-context contract registration kind {}",
                record.kind
            )));
        }
        let expected_item_name = format!(
            "{KERNEL_CONTEXT_FRONTEND_REGISTRATION_PREFIX_V1}{}",
            record.logical_name
        );
        if record.item_name != expected_item_name {
            return Err(error(format!(
                "kernel-context contract item name `{}` is inconsistent with logical name `{}`",
                record.item_name, record.logical_name
            )));
        }
        let Some(&root_index) = roots_by_name.get(&record.logical_name) else {
            return Err(error(format!(
                "orphan kernel-context contract has no registered kernel `{}`",
                record.logical_name
            )));
        };
        let root = &mut roots[root_index];
        if root.target != record.target {
            return Err(error(format!(
                "kernel-context contract target `{}` is not the exact registered kernel function",
                record.target_identity
            )));
        }
        let physical_path = tcx.def_path_str(root.target.def_id());
        let physical_name = final_path_segment(&physical_path);
        if record.contract.physical_kernel_root().name() != physical_name {
            return Err(error(format!(
                "kernel-context contract physical root `{}` does not match `{physical_name}`",
                record.contract.physical_kernel_root().name()
            )));
        }
        if root.kernel_context_contract.is_some() {
            return Err(error(format!(
                "duplicate kernel-context contract for kernel `{}`",
                record.logical_name
            )));
        }
        if let Some(previous) = targets.insert(
            record.target_identity.clone(),
            record.registration_path.clone(),
        ) {
            return Err(error(format!(
                "duplicate kernel-context contract target `{}`; first registered by `{previous}`",
                record.target_identity
            )));
        }

        root.kernel_context_contract = Some(BoundKernelContextFrontendContractV1 {
            registration_path: record.registration_path,
            canonical_bytes: record.canonical_bytes,
            contract: record.contract,
            authenticated_source: None,
        });
    }
    Ok(())
}

fn decode_resource_contract_registrations<'tcx>(
    tcx: TyCtxt<'tcx>,
    functions_by_symbol: &BTreeMap<String, Vec<Instance<'tcx>>>,
) -> Result<Vec<ResourceContractRegistrationRecord<Instance<'tcx>>>, RegistrationError> {
    let mut candidates = resource_contract_candidates(tcx);
    candidates.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    candidates
        .into_iter()
        .map(|(path, item_name, def_id, item)| {
            decode_resource_contract_registration(
                tcx,
                def_id,
                path,
                item_name,
                item,
                functions_by_symbol,
            )
        })
        .collect()
}

fn decode_resource_contract_registration<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: rustc_hir::def_id::LocalDefId,
    registration_path: String,
    item_name: String,
    item: &rustc_hir::Item<'tcx>,
    functions_by_symbol: &BTreeMap<String, Vec<Instance<'tcx>>>,
) -> Result<ResourceContractRegistrationRecord<Instance<'tcx>>, RegistrationError> {
    if !matches!(item.kind, ItemKind::Static(..)) {
        return Err(RegistrationError::new(
            registration_path,
            "the reserved resource-contract name must identify a static item",
        ));
    }
    if tcx.is_mutable_static(def_id.to_def_id()) {
        return Err(RegistrationError::new(
            registration_path,
            "resource-contract registration statics must be immutable",
        ));
    }
    let flags = tcx.codegen_fn_attrs(def_id).flags;
    if !flags.intersects(CodegenFnAttrFlags::USED_COMPILER | CodegenFnAttrFlags::USED_LINKER) {
        return Err(RegistrationError::new(
            registration_path,
            "resource-contract registration statics must carry #[used]",
        ));
    }

    let registration_ty = tcx.type_of(def_id).instantiate_identity();
    let TyKind::Tuple(field_types) = registration_ty.kind() else {
        return Err(RegistrationError::new(
            registration_path,
            "resource-contract registration must use the exact V1 tuple type",
        ));
    };
    let exact_type = field_types.len() == 6
        && field_types[0] == tcx.types.u64
        && field_types[1] == tcx.types.u16
        && field_types[2] == tcx.types.u16
        && is_shared_str(field_types[3])
        && is_shared_u8_slice(field_types[4])
        && matches!(field_types[5].kind(), TyKind::FnPtr(..));
    if !exact_type {
        return Err(RegistrationError::new(
            registration_path,
            "resource-contract registration type must be `(u64, u16, u16, &str, &[u8], fn pointer)`",
        ));
    }

    let body = tcx.mir_for_ctfe(def_id);
    let fields = registration_tuple_fields(body, 6, &registration_path)?;
    let magic = registration_integer(tcx, fields[0], tcx.types.u64, "magic", &registration_path)?;
    let version =
        registration_integer(tcx, fields[1], tcx.types.u16, "version", &registration_path)?;
    let kind = registration_integer(tcx, fields[2], tcx.types.u16, "kind", &registration_path)?;
    let logical_name = registration_string(tcx, fields[3], "logical name", &registration_path)?;
    let canonical_bytes =
        registration_bytes(tcx, fields[4], "resource contract", &registration_path)?;
    let contract = decode_kernel_resource_contract_v1(&canonical_bytes).map_err(|error| {
        RegistrationError::new(
            &registration_path,
            format!("resource-contract bytes are invalid: {error}"),
        )
    })?;
    let target = registration_target(tcx, body, fields[5], &registration_path)?;
    let target_symbol = tcx.symbol_name(target).name.to_string();
    let target_identity = tcx.def_path_str(target.def_id());
    let Some(cgu_targets) = functions_by_symbol.get(&target_symbol) else {
        return Err(RegistrationError::new(
            registration_path,
            format!(
                "resource-contract target `{target_symbol}` was not monomorphized into a codegen unit"
            ),
        ));
    };
    if cgu_targets.as_slice() != [target] {
        return Err(RegistrationError::new(
            registration_path,
            format!("resource-contract target `{target_symbol}` is ambiguous or inconsistent"),
        ));
    }

    Ok(ResourceContractRegistrationRecord {
        registration_path,
        item_name,
        magic: u64::try_from(magic)
            .map_err(|_| RegistrationError::new("resource contract", "magic does not fit u64"))?,
        version: u16::try_from(version)
            .map_err(|_| RegistrationError::new("resource contract", "version does not fit u16"))?,
        kind: u16::try_from(kind)
            .map_err(|_| RegistrationError::new("resource contract", "kind does not fit u16"))?,
        logical_name,
        canonical_bytes,
        contract,
        target_symbol,
        target_identity,
        target,
    })
}

fn bind_resource_contract_registrations<'tcx>(
    tcx: TyCtxt<'tcx>,
    roots: &mut [KernelRoot<Instance<'tcx>>],
    mut records: Vec<ResourceContractRegistrationRecord<Instance<'tcx>>>,
) -> Result<(), RegistrationError> {
    records.sort_by(|lhs, rhs| lhs.registration_path.cmp(&rhs.registration_path));
    let roots_by_name = roots
        .iter()
        .enumerate()
        .map(|(index, root)| (root.logical_name.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut targets = BTreeMap::new();

    for record in records {
        let error = |reason| RegistrationError::new(record.registration_path.clone(), reason);
        if record.magic != KERNEL_RESOURCE_REGISTRATION_MAGIC_V1 {
            return Err(error(format!(
                "resource-contract magic {:#018x} does not match {:#018x}",
                record.magic, KERNEL_RESOURCE_REGISTRATION_MAGIC_V1
            )));
        }
        if record.version != KERNEL_RESOURCE_REGISTRATION_VERSION_V1 {
            return Err(error(format!(
                "unknown resource-contract registration version {}",
                record.version
            )));
        }
        if record.kind != KERNEL_RESOURCE_REGISTRATION_KIND_V1 {
            return Err(error(format!(
                "unknown resource-contract registration kind {}",
                record.kind
            )));
        }
        if record.logical_name.is_empty() {
            return Err(error(
                "resource-contract logical name must not be empty".to_owned(),
            ));
        }
        let expected_item_name = format!(
            "{KERNEL_RESOURCE_REGISTRATION_PREFIX_V1}{}",
            record.logical_name
        );
        if record.item_name != expected_item_name {
            return Err(error(format!(
                "resource-contract item name `{}` is inconsistent with logical name `{}`",
                record.item_name, record.logical_name
            )));
        }
        let Some(&root_index) = roots_by_name.get(&record.logical_name) else {
            return Err(error(format!(
                "orphan resource contract has no registered kernel `{}`",
                record.logical_name
            )));
        };
        let root = &mut roots[root_index];
        if root.target != record.target {
            return Err(error(format!(
                "resource-contract target `{}` is not the exact registered kernel function",
                record.target_identity
            )));
        }
        let Some(frontend) = root.frontend_contract.as_mut() else {
            return Err(error(
                "resource contract requires an authenticated frontend launch contract".to_owned(),
            ));
        };
        if frontend.resource.is_some() {
            return Err(error(format!(
                "duplicate resource contract for kernel `{}`",
                record.logical_name
            )));
        }
        if frontend.target_symbol != record.target_symbol
            || frontend.target_def_path_hash
                != tcx.def_path_hash(record.target.def_id()).0.to_le_bytes()
        {
            return Err(error(
                "resource contract disagrees with the authenticated frontend target".to_owned(),
            ));
        }
        if let Some(previous) = targets.insert(
            record.target_identity.clone(),
            record.registration_path.clone(),
        ) {
            return Err(error(format!(
                "duplicate resource-contract target `{}`; first registered by `{previous}`",
                record.target_identity
            )));
        }
        frontend.resource = Some(AuthenticatedKernelResourceContractV1 {
            canonical_bytes: record.canonical_bytes,
            contract: record.contract,
        });
    }
    Ok(())
}

fn decode_registration_static<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: rustc_hir::def_id::LocalDefId,
    registration_path: String,
    item_name: String,
    functions_by_symbol: &BTreeMap<String, Vec<Instance<'tcx>>>,
) -> Result<RegistrationRecord<Instance<'tcx>>, RegistrationError> {
    let registration_ty = tcx
        .try_normalize_erasing_regions(
            TypingEnv::fully_monomorphized(),
            tcx.type_of(def_id).instantiate_identity(),
        )
        .map_err(|_| {
            RegistrationError::new(&registration_path, "registration type did not normalize")
        })?;
    let TyKind::Tuple(fields) = registration_ty.kind() else {
        return Err(RegistrationError::new(
            registration_path,
            "registration must use an exact V1, V2, or V3 tuple type",
        ));
    };

    let is_v1 = fields.len() == reserved_fe2o3_symbols::KERNEL_REGISTRATION_V1_FIELD_COUNT
        && fields[0] == tcx.types.u64
        && fields[1] == tcx.types.u16
        && fields[2] == tcx.types.u16
        && is_shared_str(fields[3])
        && is_shared_str(fields[4])
        && matches!(fields[5].kind(), TyKind::FnPtr(..));
    let is_v2 = fields.len() == reserved_fe2o3_symbols::KERNEL_REGISTRATION_V2_FIELD_COUNT
        && fields[0] == tcx.types.u64
        && fields[1] == tcx.types.u16
        && fields[2] == tcx.types.u16
        && is_shared_str(fields[3])
        && is_shared_str(fields[4])
        && is_shared_str(fields[5])
        && is_shared_str(fields[6])
        && matches!(fields[7].kind(), TyKind::FnPtr(..));
    let is_v3 = fields.len() == reserved_fe2o3_symbols::KERNEL_REGISTRATION_V3_FIELD_COUNT
        && fields[0] == tcx.types.u64
        && fields[1] == tcx.types.u16
        && fields[2] == tcx.types.u16
        && is_shared_str(fields[3])
        && is_shared_str(fields[4])
        && is_shared_str(fields[5])
        && is_shared_str(fields[6])
        && is_shared_str(fields[7])
        && is_shared_str(fields[8])
        && matches!(fields[9].kind(), TyKind::FnPtr(..));
    if !is_v1 && !is_v2 && !is_v3 {
        return Err(RegistrationError::new(
            registration_path,
            "registration type must be the exact V1, V2, or 10-field V3 tuple",
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
                    "registration initializer must contain exactly one tuple value",
                ));
            }
        }
    }
    let fields = aggregate.ok_or_else(|| {
        RegistrationError::new(
            registration_path.clone(),
            "registration initializer does not contain the required tuple value",
        )
    })?;
    let expected_fields = if is_v3 {
        reserved_fe2o3_symbols::KERNEL_REGISTRATION_V3_FIELD_COUNT
    } else if is_v2 {
        reserved_fe2o3_symbols::KERNEL_REGISTRATION_V2_FIELD_COUNT
    } else {
        reserved_fe2o3_symbols::KERNEL_REGISTRATION_V1_FIELD_COUNT
    };
    if fields.len() != expected_fields {
        return Err(RegistrationError::new(
            registration_path,
            "registration initializer has the wrong field count",
        ));
    }
    let fields = fields.iter().collect::<Vec<_>>();

    let magic = registration_integer(tcx, fields[0], tcx.types.u64, "magic", &registration_path)?;
    let version =
        registration_integer(tcx, fields[1], tcx.types.u16, "version", &registration_path)?;
    let kind = registration_integer(tcx, fields[2], tcx.types.u16, "kind", &registration_path)?;
    let logical_name = registration_string(tcx, fields[3], "logical name", &registration_path)?;
    let export_name = registration_string(tcx, fields[4], "export name", &registration_path)?;
    let crate_binding = if is_v2 || is_v3 {
        let value = registration_string(tcx, fields[5], "crate binding", &registration_path)?;
        Some(CrateBindingIdV1::from_hex(&value).map_err(|error| {
            RegistrationError::new(
                &registration_path,
                format!("invalid crate binding: {error}"),
            )
        })?)
    } else {
        None
    };
    let kernel_binding = if is_v2 || is_v3 {
        let value = registration_string(tcx, fields[6], "kernel binding", &registration_path)?;
        Some(KernelBindingIdV1::from_hex(&value).map_err(|error| {
            RegistrationError::new(
                &registration_path,
                format!("invalid kernel binding: {error}"),
            )
        })?)
    } else {
        None
    };
    let profile_tag = if is_v3 {
        Some(registration_string(
            tcx,
            fields[7],
            "profile tag",
            &registration_path,
        )?)
    } else {
        None
    };
    let generated_host_contract_identity = if is_v3 {
        let value = registration_string(
            tcx,
            fields[8],
            "generated host-contract identity",
            &registration_path,
        )?;
        Some(
            GeneratedHostContractIdV3::from_hex(&value).map_err(|error| {
                RegistrationError::new(
                    &registration_path,
                    format!("invalid generated host-contract identity: {error}"),
                )
            })?,
        )
    } else {
        None
    };
    let target_index = if is_v3 {
        9
    } else if is_v2 {
        7
    } else {
        5
    };
    let target = registration_target(tcx, body, fields[target_index], &registration_path)?;
    let target_crate_name = tcx.crate_name(target.def_id().krate).to_string();
    let target_symbol = tcx.symbol_name(target).name.to_string();
    let target_identity = tcx.def_path_str(target.def_id());
    match functions_by_symbol.get(&target_symbol) {
        None if !target.def_id().is_local() && tcx.is_mir_available(target.def_id()) => {}
        None => {
            return Err(RegistrationError::new(
                registration_path,
                format!(
                    "registered target `{target_symbol}` was not monomorphized into a codegen unit"
                ),
            ));
        }
        Some(cgu_targets) if cgu_targets.as_slice() == [target] => {}
        Some(cgu_targets) => {
            let paths = cgu_targets
                .iter()
                .map(|instance| tcx.def_path_str(instance.def_id()))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(RegistrationError::new(
                registration_path,
                format!(
                    "registered target symbol `{target_symbol}` is ambiguous or inconsistent across: {paths}"
                ),
            ));
        }
    }
    let magic = u64::try_from(magic)
        .map_err(|_| RegistrationError::new(&registration_path, "magic does not fit u64"))?;
    let version = u16::try_from(version)
        .map_err(|_| RegistrationError::new(&registration_path, "version does not fit u16"))?;
    let kind = u16::try_from(kind)
        .map_err(|_| RegistrationError::new(&registration_path, "kind does not fit u16"))?;
    if version == reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V1 && !is_v1 {
        return Err(RegistrationError::new(
            &registration_path,
            "registration version 1 requires the exact V1 tuple type",
        ));
    }
    if version == reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V2 && !is_v2 {
        return Err(RegistrationError::new(
            &registration_path,
            "registration version 2 requires the exact V2 tuple type",
        ));
    }
    if version == reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V3 && !is_v3 {
        return Err(RegistrationError::new(
            &registration_path,
            "registration version 3 requires the exact V3 tuple type",
        ));
    }

    Ok(RegistrationRecord {
        registration_path,
        item_name,
        magic,
        version,
        kind,
        logical_name,
        export_name,
        crate_binding,
        kernel_binding,
        profile_tag,
        generated_host_contract_identity,
        target_crate_name,
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

fn registration_bytes<'tcx>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    field: &str,
    registration_path: &str,
) -> Result<Vec<u8>, RegistrationError> {
    let Operand::Constant(constant) = operand else {
        return Err(RegistrationError::new(
            registration_path,
            format!("V1 {field} field must be a byte-slice constant"),
        ));
    };
    if !is_shared_u8_slice(constant.const_.ty()) {
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
    value
        .try_get_slice_bytes_for_diagnostics(tcx)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| {
            RegistrationError::new(
                registration_path,
                format!("V1 {field} field did not evaluate to byte-slice data"),
            )
        })
}

fn registration_tuple_fields<'a, 'tcx>(
    body: &'a rustc_middle::mir::Body<'tcx>,
    expected_fields: usize,
    registration_path: &str,
) -> Result<Vec<&'a Operand<'tcx>>, RegistrationError> {
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
                    "registration initializer must contain exactly one tuple value",
                ));
            }
        }
    }
    let fields = aggregate.ok_or_else(|| {
        RegistrationError::new(
            registration_path,
            "registration initializer does not contain the required tuple value",
        )
    })?;
    if fields.len() != expected_fields {
        return Err(RegistrationError::new(
            registration_path,
            format!(
                "registration initializer has {} fields; expected {expected_fields}",
                fields.len()
            ),
        ));
    }
    Ok(fields.iter().collect())
}

fn registration_target<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &rustc_middle::mir::Body<'tcx>,
    operand: &Operand<'tcx>,
    registration_path: &str,
) -> Result<Instance<'tcx>, RegistrationError> {
    let place = match operand {
        Operand::Copy(place) | Operand::Move(place) => place,
        Operand::Constant(constant) => {
            return constant_function_target(tcx, constant, registration_path);
        }
        Operand::RuntimeChecks(_) => {
            return Err(RegistrationError::new(
                registration_path,
                "registration target field must use one exact function pointer",
            ));
        }
    };
    let Some(target_local) = place.as_local() else {
        return Err(RegistrationError::new(
            registration_path,
            "registration target field must use an unprojected function-pointer local",
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
                    "registration target coercion must directly name a function item",
                ));
            };
            let TyKind::FnDef(def_id, args) = source.const_.ty().kind() else {
                return Err(RegistrationError::new(
                    registration_path,
                    "registration target coercion does not reference a function item",
                ));
            };
            let resolved =
                Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, args)
                    .ok()
                    .flatten()
                    .ok_or_else(|| {
                        RegistrationError::new(
                            registration_path,
                            "registration target function could not be resolved",
                        )
                    })?;
            if target.replace(resolved).is_some() {
                return Err(RegistrationError::new(
                    registration_path,
                    "registration target local has multiple function definitions",
                ));
            }
        }
    }

    target.ok_or_else(|| {
        RegistrationError::new(
            registration_path,
            "registration target function association is missing",
        )
    })
}

fn constant_function_target<'tcx>(
    tcx: TyCtxt<'tcx>,
    constant: &rustc_middle::mir::ConstOperand<'tcx>,
    registration_path: &str,
) -> Result<Instance<'tcx>, RegistrationError> {
    if !matches!(constant.const_.ty().kind(), TyKind::FnPtr(..)) {
        return Err(RegistrationError::new(
            registration_path,
            "registration target constant is not a function pointer",
        ));
    }
    let value = constant
        .const_
        .eval(tcx, TypingEnv::fully_monomorphized(), constant.span)
        .map_err(|_| {
            RegistrationError::new(registration_path, "registration target did not evaluate")
        })?;
    let scalar = value.try_to_scalar().ok_or_else(|| {
        RegistrationError::new(
            registration_path,
            "registration target is not one scalar pointer",
        )
    })?;
    let pointer = scalar
        .to_pointer(&tcx)
        .discard_err()
        .ok_or_else(|| {
            RegistrationError::new(registration_path, "registration target is not a pointer")
        })?
        .into_pointer_or_addr()
        .map_err(|_| {
            RegistrationError::new(registration_path, "registration target has no provenance")
        })?;
    let (provenance, offset) = pointer.into_raw_parts();
    if offset.bytes() != 0 {
        return Err(RegistrationError::new(
            registration_path,
            "registration target has a nonzero function-pointer offset",
        ));
    }
    match tcx.global_alloc(provenance.alloc_id()) {
        GlobalAlloc::Function { instance } => Ok(instance),
        _ => Err(RegistrationError::new(
            registration_path,
            "registration target pointer does not identify a function",
        )),
    }
}

fn is_shared_str(ty: rustc_middle::ty::Ty<'_>) -> bool {
    matches!(ty.kind(), TyKind::Ref(_, inner, mutability) if inner.is_str() && !mutability.is_mut())
}

fn is_shared_u8_slice(ty: rustc_middle::ty::Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Ref(_, inner, mutability)
            if !mutability.is_mut()
                && matches!(inner.kind(), TyKind::Slice(element) if matches!(element.kind(), TyKind::Uint(rustc_middle::ty::UintTy::U8)))
    )
}

fn validate_registration_records<T: Copy>(
    mut records: Vec<RegistrationRecord<T>>,
    expected_crate_binding: Option<CrateBindingIdV1>,
    expected_crate_name: Option<&str>,
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
                "magic {:#018x} does not match registration magic {:#018x}",
                record.magic,
                reserved_fe2o3_symbols::KERNEL_REGISTRATION_MAGIC
            )));
        }
        let generated_host_contract_identity = match (record.version, record.kind) {
            (
                reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V1,
                reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_KERNEL,
            ) => None,
            (
                reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V3,
                reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_TYPED_GENERAL_LAYOUT_V3,
            ) => {
                let profile_tag = record
                    .profile_tag
                    .as_deref()
                    .ok_or_else(|| error("V3 registration has no profile tag".to_owned()))?;
                if profile_tag != TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3 {
                    return Err(error(format!(
                        "V3 profile tag {profile_tag:?} is not the canonical general typed profile {TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3:?}"
                    )));
                }
                Some(record.generated_host_contract_identity.ok_or_else(|| {
                    error("V3 registration has no generated host-contract identity".to_owned())
                })?)
            }
            (reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V2, _) => {
                return Err(error(
                    "registration version 2 has been removed; use the generic typed V3 contract"
                        .to_owned(),
                ));
            }
            (version, kind)
                if kind
                    == reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_TYPED_GENERAL_LAYOUT_V3
                    && version != reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V3 =>
            {
                return Err(error(
                    "general typed registrations require version 3".to_owned(),
                ));
            }
            (reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V3, _) => {
                return Err(error(
                    "version 3 is reserved for the general typed registration kind".to_owned(),
                ));
            }
            (version, _)
                if version != reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V1
                    && version != reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V3 =>
            {
                return Err(error(format!("unknown registration version {version}")));
            }
            (_, kind) => return Err(error(format!("unknown registration kind {kind}"))),
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

        let registration_module = owner_module_path(&record.registration_path);
        let target_module = owner_module_path(&record.target_identity);
        let target_is_external =
            expected_crate_name.is_some_and(|expected| record.target_crate_name != expected);
        if generated_host_contract_identity.is_some() {
            let _expected_crate_name = expected_crate_name.ok_or_else(|| {
                error("typed registration has no rustc session crate identity".to_owned())
            })?;
            if target_is_external {
                return Err(error(
                    "cross-crate generic typed kernels are not yet supported".to_owned(),
                ));
            }
            if registration_module != target_module {
                return Err(error(format!(
                    "registered target module {target_module:?} disagrees with registration module {registration_module:?}"
                )));
            }
        }

        let kernel_binding = match generated_host_contract_identity {
            Some(_) => {
                let crate_binding = record
                    .crate_binding
                    .ok_or_else(|| error("V3 registration has no crate binding".to_owned()))?;
                let expected = expected_crate_binding.ok_or_else(|| {
                    error("V3 registration has no rustc session crate binding".to_owned())
                })?;
                if crate_binding != expected {
                    return Err(error(format!(
                        "crate binding {} disagrees with rustc session binding {}",
                        crate_binding.to_hex(),
                        expected.to_hex()
                    )));
                }
                let declared = record
                    .kernel_binding
                    .ok_or_else(|| error("V3 registration has no kernel binding".to_owned()))?;
                let expected = derive_kernel_binding_id_v1(
                    crate_binding,
                    TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
                    &record.logical_name,
                    &record.export_name,
                );
                if declared != expected {
                    return Err(error(format!(
                        "kernel binding {} disagrees with derived binding {}",
                        declared.to_hex(),
                        expected.to_hex()
                    )));
                }
                Some(declared)
            }
            None => {
                if record.crate_binding.is_some()
                    || record.kernel_binding.is_some()
                    || record.profile_tag.is_some()
                    || record.generated_host_contract_identity.is_some()
                {
                    return Err(error(
                        "V1 registration unexpectedly carries binding IDs".to_owned(),
                    ));
                }
                None
            }
        };

        let expected_target_symbol = match kernel_binding {
            Some(binding) => host_kernel_symbol_v1(binding),
            None => format!(
                "{}{}",
                reserved_fe2o3_symbols::KERNEL_PREFIX,
                record.export_name
            ),
        };
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
            generated_host_contract_identity,
            kernel_binding,
            frontend_contract: None,
            kernel_context_contract: None,
            reference_effect_binding: None,
        });
    }

    roots.sort_by(|lhs, rhs| {
        lhs.logical_name
            .cmp(&rhs.logical_name)
            .then_with(|| lhs.export_name.cmp(&rhs.export_name))
    });
    Ok(roots)
}

fn owner_module_path(path: &str) -> &str {
    path.rsplit_once("::")
        .map_or("", |(module_path, _)| module_path)
}

fn sibling_item_has_name(
    tcx: TyCtxt<'_>,
    owner: DefId,
    candidate: DefId,
    expected_name: &str,
) -> bool {
    tcx.opt_parent(candidate) == tcx.opt_parent(owner)
        && tcx
            .def_key(candidate)
            .get_opt_name()
            .is_some_and(|name| name.as_str() == expected_name)
}

fn sibling_item_diagnostic_path(owner_path: &str, item_name: &str) -> String {
    let module_path = owner_module_path(owner_path);
    if module_path.is_empty() {
        item_name.to_owned()
    } else {
        format!("{module_path}::{item_name}")
    }
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

fn is_authenticated_unsafe_raw_memory_terminal_v1(
    rule: Option<crate::production_semantic_terminal_v1::ProductionSemanticTerminalRuleV1>,
) -> bool {
    use crate::production_semantic_terminal_v1::{
        ProductionExecutionTerminalV1 as Execution, ProductionSemanticTerminalRuleV1 as Rule,
        ProductionTerminalExpansionV1 as Expansion,
    };

    matches!(
        rule,
        Some(Rule::Expand(Expansion::Execution(
            Execution::PrivateMemoryFromRawParts | Execution::WorkgroupMemoryFromRawParts
        )))
    )
}

fn is_fully_monomorphized<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    let generics = tcx.generics_of(instance.def_id());

    for arg in instance.args.iter() {
        if arg.has_param() || arg.has_escaping_bound_vars() {
            return false;
        }
    }

    generics.count() == 0 || !instance.args.is_empty()
}

fn callable_safety(tcx: TyCtxt<'_>, instance: Instance<'_>) -> Safety {
    if tcx.def_kind(instance.def_id()) == rustc_hir::def::DefKind::Closure {
        instance.args.as_closure().sig().skip_binder().safety
    } else {
        tcx.fn_sig(instance.def_id()).skip_binder().safety()
    }
}

struct DeviceCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    seen: BTreeSet<crate::device_ffi::DeviceFfiInstanceIdentity>,
    call_chains: BTreeMap<
        crate::device_ffi::DeviceFfiInstanceIdentity,
        CallChainLink<crate::device_ffi::DeviceFfiInstanceIdentity>,
    >,
    call_edges: BTreeMap<
        crate::device_ffi::DeviceFfiInstanceIdentity,
        BTreeSet<crate::device_ffi::DeviceFfiInstanceIdentity>,
    >,
    reachable_unsafe_calls:
        BTreeMap<crate::device_ffi::DeviceFfiInstanceIdentity, BTreeSet<String>>,
    kernel_context_issuance_calls: BTreeMap<
        crate::device_ffi::DeviceFfiInstanceIdentity,
        Vec<ObservedKernelContextIssuanceV1>,
    >,
    inline_assembly:
        BTreeMap<crate::device_ffi::DeviceFfiInstanceIdentity, ObservedInlineAssemblyV1>,
    used_export_names: BTreeSet<String>,
    worklist: VecDeque<CollectedFunction<'tcx>>,
    result: Vec<CollectedFunction<'tcx>>,
    ffi_declarations: Vec<crate::device_ffi::CollectedDeviceFfi<'tcx>>,
    reachable_ffi_imports: BTreeSet<reserved_fe2o3_symbols::DeviceFfiContractIdV1>,
    expected_target: String,
    inspected_blocks: usize,
    verbose: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallChainLink<T> {
    predecessor: Option<T>,
    label: String,
}

fn reconstruct_call_chain<T: Clone + Ord>(
    links: &BTreeMap<T, CallChainLink<T>>,
    start: &T,
) -> Vec<String> {
    let mut reverse_chain = Vec::new();
    let mut cursor = Some(start.clone());
    for _ in 0..links.len() {
        let Some(identity) = cursor else {
            break;
        };
        let Some(link) = links.get(&identity) else {
            break;
        };
        reverse_chain.push(link.label.clone());
        cursor = link.predecessor.clone();
    }
    reverse_chain.reverse();
    reverse_chain
}

fn root_scoped_call_chains<T: Clone + Ord>(
    edges: &BTreeMap<T, BTreeSet<T>>,
    labels: &BTreeMap<T, String>,
    root: &T,
) -> (BTreeMap<T, CallChainLink<T>>, Vec<T>) {
    let Some(root_label) = labels.get(root) else {
        return (BTreeMap::new(), Vec::new());
    };
    let mut links = BTreeMap::from([(
        root.clone(),
        CallChainLink {
            predecessor: None,
            label: root_label.clone(),
        },
    )]);
    let mut order = Vec::new();
    let mut pending = VecDeque::from([root.clone()]);
    while let Some(caller) = pending.pop_front() {
        order.push(caller.clone());
        let Some(callees) = edges.get(&caller) else {
            continue;
        };
        for callee in callees {
            let Some(label) = labels.get(callee) else {
                continue;
            };
            if links.contains_key(callee) {
                continue;
            }
            links.insert(
                callee.clone(),
                CallChainLink {
                    predecessor: Some(caller.clone()),
                    label: label.clone(),
                },
            );
            pending.push_back(callee.clone());
        }
    }
    (links, order)
}

struct UserUnsafeBlockVisitor<'tcx> {
    tcx: TyCtxt<'tcx>,
    owner: LocalDefId,
    allow_authenticated_raw_memory: bool,
    allow_authenticated_context_issuance: bool,
    first_span: Option<rustc_span::Span>,
}

impl<'tcx> UserUnsafeBlockVisitor<'tcx> {
    fn new(
        tcx: TyCtxt<'tcx>,
        owner: LocalDefId,
        allow_authenticated_raw_memory: bool,
        allow_authenticated_context_issuance: bool,
    ) -> Self {
        Self {
            tcx,
            owner,
            allow_authenticated_raw_memory,
            allow_authenticated_context_issuance,
            first_span: None,
        }
    }

    fn is_exact_authenticated_raw_memory_call(&self, block: &rustc_hir::Block<'tcx>) -> bool {
        let ([], Some(expression)) = (block.stmts, block.expr) else {
            return false;
        };
        let ExprKind::Call(callee, arguments) = expression.kind else {
            return false;
        };
        if arguments.len() != 4 {
            return false;
        }
        let ExprKind::Path(ref path) = callee.kind else {
            return false;
        };
        let Res::Def(_, def_id) = self.tcx.typeck(self.owner).qpath_res(path, callee.hir_id) else {
            return false;
        };
        is_authenticated_unsafe_raw_memory_terminal_v1(
            crate::production_semantic_terminal_v1::classify(self.tcx, def_id),
        )
    }

    fn is_exact_authenticated_context_issuance(&self, block: &rustc_hir::Block<'tcx>) -> bool {
        let ([], Some(expression)) = (block.stmts, block.expr) else {
            return false;
        };
        let ExprKind::Call(callee, []) = expression.kind else {
            return false;
        };
        let ExprKind::Path(ref path) = callee.kind else {
            return false;
        };
        let Res::Def(_, def_id) = self.tcx.typeck(self.owner).qpath_res(path, callee.hir_id) else {
            return false;
        };
        crate::trusted_device_items::classify(self.tcx, def_id)
            == Some(crate::trusted_device_items::TrustedDeviceItem::KernelContextIssue)
    }
}

impl<'tcx> Visitor<'tcx> for UserUnsafeBlockVisitor<'tcx> {
    fn visit_block(&mut self, block: &'tcx rustc_hir::Block<'tcx>) {
        if matches!(
            block.rules,
            BlockCheckMode::UnsafeBlock(UnsafeSource::UserProvided)
        ) && !(self.allow_authenticated_raw_memory
            && self.is_exact_authenticated_raw_memory_call(block))
            && !(self.allow_authenticated_context_issuance
                && self.is_exact_authenticated_context_issuance(block))
        {
            self.first_span.get_or_insert(block.span);
        }
        intravisit::walk_block(self, block);
    }

    fn visit_expr(&mut self, expression: &'tcx rustc_hir::Expr<'tcx>) {
        // A closure body is authenticated when its concrete callable instance
        // enters the collected graph, not merely when a closure value is made.
        if !matches!(expression.kind, ExprKind::Closure(_)) {
            intravisit::walk_expr(self, expression);
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ObservedInlineAssemblyV1 {
    blocks: u32,
    operand_bits: u16,
    option_sets: BTreeSet<u16>,
}

impl<'tcx> DeviceCollector<'tcx> {
    fn new(
        tcx: TyCtxt<'tcx>,
        verbose: bool,
        ffi_declarations: Vec<crate::device_ffi::CollectedDeviceFfi<'tcx>>,
        target: String,
    ) -> Self {
        Self {
            tcx,
            seen: BTreeSet::new(),
            call_chains: BTreeMap::new(),
            call_edges: BTreeMap::new(),
            reachable_unsafe_calls: BTreeMap::new(),
            kernel_context_issuance_calls: BTreeMap::new(),
            inline_assembly: BTreeMap::new(),
            used_export_names: BTreeSet::new(),
            worklist: VecDeque::new(),
            result: Vec::new(),
            ffi_declarations,
            reachable_ffi_imports: BTreeSet::new(),
            expected_target: target,
            inspected_blocks: 0,
            verbose,
        }
    }

    fn mark_seen(
        &mut self,
        identity: crate::device_ffi::DeviceFfiInstanceIdentity,
    ) -> Result<bool, CollectError> {
        if self.seen.len() >= fe2o3_rustc_front::MAX_FUNCTIONS_V1 && !self.seen.contains(&identity)
        {
            return Err(CollectError {
                message: format!(
                    "device-reachable function count exceeds hard maximum {}",
                    fe2o3_rustc_front::MAX_FUNCTIONS_V1,
                ),
            });
        }
        Ok(self.seen.insert(identity))
    }

    fn charge_function_blocks(
        &mut self,
        function: &Instance<'tcx>,
        block_count: usize,
    ) -> Result<(), CollectError> {
        if block_count > fe2o3_rustc_front::MAX_BLOCKS_PER_FUNCTION_V1 {
            return Err(self.reachable_error(
                function,
                &format!(
                    "function contains {block_count} MIR blocks; hard maximum is {}",
                    fe2o3_rustc_front::MAX_BLOCKS_PER_FUNCTION_V1,
                ),
                None,
            ));
        }
        let total = self
            .inspected_blocks
            .checked_add(block_count)
            .ok_or_else(|| {
                self.reachable_error(function, "reachable MIR block accounting overflowed", None)
            })?;
        if total > fe2o3_rustc_front::MAX_TOTAL_BLOCKS_V1 {
            return Err(self.reachable_error(
                function,
                &format!(
                    "device closure contains {total} MIR blocks; hard maximum is {}",
                    fe2o3_rustc_front::MAX_TOTAL_BLOCKS_V1,
                ),
                None,
            ));
        }
        self.inspected_blocks = total;
        Ok(())
    }

    fn add_device_export(
        &mut self,
        instance: Instance<'tcx>,
        export_name: String,
    ) -> Result<(), CollectError> {
        if !self.used_export_names.insert(export_name.clone()) {
            return Err(CollectError {
                message: format!(
                    "fe2o3 device FFI export `{export_name}` has duplicate symbol ownership"
                ),
            });
        }
        let identity = self.instance_identity(instance);
        if self.mark_seen(identity.clone())? {
            self.call_chains.insert(
                identity,
                CallChainLink {
                    predecessor: None,
                    label: self.instance_label(instance),
                },
            );
            self.worklist.push_back(CollectedFunction {
                instance,
                role: CollectedFunctionRole::DeviceFfiExport,
                export_name,
                logical_name: None,
                generated_host_contract_identity: None,
                kernel_binding: None,
                frontend_contract: None,
                kernel_context_contract: None,
                reference_effect_binding: None,
                dead_branches: None,
                closure_plan: None,
            });
        }
        Ok(())
    }

    fn add_root(&mut self, root: KernelRoot<Instance<'tcx>>) -> Result<(), CollectError> {
        let KernelRoot {
            target: instance,
            logical_name,
            export_name,
            generated_host_contract_identity,
            kernel_binding,
            frontend_contract,
            kernel_context_contract,
            reference_effect_binding,
        } = root;
        if !self.used_export_names.insert(export_name.clone()) {
            return Err(CollectError {
                message: format!(
                    "fe2o3 kernel export `{export_name}` conflicts with an existing kernel or device FFI symbol"
                ),
            });
        }
        let identity = self.instance_identity(instance);
        if self.mark_seen(identity.clone())? {
            self.call_chains.insert(
                identity.clone(),
                CallChainLink {
                    predecessor: None,
                    label: self.instance_label(instance),
                },
            );
            self.worklist.push_back(CollectedFunction {
                instance,
                role: CollectedFunctionRole::KernelEntry,
                export_name,
                logical_name: Some(logical_name),
                generated_host_contract_identity,
                kernel_binding,
                frontend_contract,
                kernel_context_contract,
                reference_effect_binding,
                dead_branches: None,
                closure_plan: None,
            });
        }
        Ok(())
    }

    fn collect(mut self) -> Result<CollectionResult<'tcx>, CollectError> {
        while let Some(mut function) = self.worklist.pop_front() {
            let def_id = function.instance.def_id();

            if !self.tcx.is_mir_available(def_id) {
                return Err(self.reachable_error(
                    &function.instance,
                    "MIR is unavailable for a collected device function",
                    None,
                ));
            }

            let mir = self.tcx.instance_mir(function.instance.def);
            self.charge_function_blocks(&function.instance, mir.basic_blocks.len())?;
            let closure_plan = if crate::closure_profile_v1::contains_concrete_closure_v1(
                self.tcx,
                function.instance,
            )
            .map_err(|error| {
                self.reachable_error(
                    &function.instance,
                    &format!("closure presence check failed closed: {error}"),
                    None,
                )
            })? {
                let closure_plan = crate::closure_profile_v1::analyze_gfx942_closures_v1(
                    self.tcx,
                    function.instance,
                    crate::closure_profile_v1::ClosureOriginPolicyV1::Either,
                    &self.expected_target,
                )
                .map_err(|error| {
                    self.reachable_error(
                        &function.instance,
                        &format!("bounded gfx942 closure admission failed: {error}"),
                        None,
                    )
                })?;
                if self.verbose {
                    eprintln!(
                        "[collector] gfx942 closure profile: {} environment(s), {} static call(s), {} authenticated higher-order capability call(s), identity {}",
                        closure_plan.environments().len(),
                        closure_plan.calls().len(),
                        closure_plan.higher_order_calls().len(),
                        encode_lower_hex(&closure_plan.identity()),
                    );
                }
                Some(closure_plan)
            } else {
                None
            };
            let dead_branches =
                crate::monomorphization_dead::CompilerDeadBranchObservationV1::observe(
                    self.tcx,
                    function.instance,
                    mir,
                )
                .map_err(|error| {
                    self.reachable_error(
                        &function.instance,
                        &format!("dead-branch observation failed closed: {error}"),
                        None,
                    )
                })?;
            if self.verbose {
                let context = dead_branches.evidence().context();
                eprintln!(
                    "[collector] visiting {} ({} basic blocks, {} V1 decisions, {} policy-excluded)",
                    function.export_name,
                    mir.basic_blocks.len(),
                    dead_branches.evidence().decisions().len(),
                    dead_branches.excluded_blocks().len(),
                );
                eprintln!(
                    "[collector] V1 identity {} mir={} source={} target={}",
                    function.export_name,
                    encode_lower_hex(&context.cfg_identity()),
                    encode_lower_hex(&context.source_identity()),
                    encode_lower_hex(&context.target_identity()),
                );
            }

            for (rustc_block, block) in mir.basic_blocks.iter_enumerated() {
                if let Some(terminator) = &block.terminator {
                    let rustc_block =
                        u32::try_from(rustc_block.as_usize()).map_err(|_| CollectError {
                            message: "kernel-context issuance block exceeds u32".to_owned(),
                        })?;
                    self.process_terminator(
                        rustc_block,
                        &terminator.kind,
                        mir,
                        &function.instance,
                    )?;
                }
            }

            function.dead_branches = Some(dead_branches);
            function.closure_plan = closure_plan;
            self.result.push(function);
        }

        self.authenticate_kernel_context_contracts()?;
        self.authenticate_production_kernel_source_safety()?;
        self.authenticate_reachable_frontend_contracts()?;

        let device_ffi = crate::device_ffi::validate_local_closure(
            self.tcx,
            &mut self.ffi_declarations,
            &self.reachable_ffi_imports,
        )
        .map_err(|error| CollectError {
            message: error.to_string(),
        })?;
        if self.verbose && !device_ffi.is_empty() {
            eprintln!(
                "[collector] validated local device FFI evidence: {} imports, {} exports, target {}, asserted code object v{}",
                device_ffi.imports.len(),
                device_ffi.exports.len(),
                device_ffi.target.as_deref().unwrap_or("<none>"),
                device_ffi
                    .code_object_version_assertion
                    .as_ref()
                    .map(|version| *version.asserted_for_consistency_check())
                    .unwrap_or_default(),
            );
        }

        let mut collection = CollectionResult {
            functions: self.result,
            device_ffi,
            compiler_ffi_observation: None,
        };
        collection.compiler_ffi_observation =
            crate::compiler_ffi_adapter::adapt_collection_v1(self.tcx, &collection).map_err(
                |error| CollectError {
                    message: format!("compiler FFI envelope construction failed: {error}"),
                },
            )?;
        Ok(collection)
    }

    fn process_terminator(
        &mut self,
        rustc_block: u32,
        terminator: &TerminatorKind<'tcx>,
        body: &Body<'tcx>,
        caller: &Instance<'tcx>,
    ) -> Result<(), CollectError> {
        match terminator {
            TerminatorKind::Call { func, unwind, .. } => {
                // `Continue` has no executable cleanup target to traverse. Its
                // no-unwind obligation is discharged by walking the resolved
                // direct callee below and rejecting panic/unwind MIR there.
                if !matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable) {
                    return Err(self.reachable_error(
                        caller,
                        &format!(
                            "[FE2O3-FFI-EDGE002] direct call has an untraversed unwind edge `{unwind:?}`"
                        ),
                        None,
                    ));
                }
                self.process_call_operand(rustc_block, func, caller)
            }
            TerminatorKind::InlineAsm {
                asm_macro,
                operands,
                options,
                targets,
                unwind,
                ..
            } => self.process_inline_assembly(
                *asm_macro,
                operands,
                *options,
                targets.len(),
                *unwind,
                body,
                caller,
            ),
            TerminatorKind::Assert { unwind, .. }
                if matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable) =>
            {
                Ok(())
            }
            TerminatorKind::Drop { place, unwind, .. }
                if matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable) =>
            {
                match classify_rustc_drop_v1(self.tcx, *caller, body, *place) {
                    Ok(ProductionRustcDropClassV1::Trivial) => Ok(()),
                    Ok(ProductionRustcDropClassV1::RequiresDropGlue) => Err(self.reachable_error(
                        caller,
                        "[FE2O3-FFI-EDGE001] unsupported executable MIR edge `Drop requiring drop glue`",
                        None,
                    )),
                    Err(_) => Err(self.reachable_error(
                        caller,
                        "[FE2O3-FFI-EDGE001] Drop place type failed monomorphic normalization",
                        None,
                    )),
                }
            }
            TerminatorKind::Goto { .. }
            | TerminatorKind::SwitchInt { .. }
            | TerminatorKind::Return
            | TerminatorKind::Unreachable => Ok(()),
            unsupported => Err(self.reachable_error(
                caller,
                &format!("[FE2O3-FFI-EDGE001] unsupported executable MIR edge `{unsupported:?}`"),
                None,
            )),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_inline_assembly(
        &mut self,
        asm_macro: InlineAsmMacro,
        operands: &[InlineAsmOperand<'tcx>],
        options: InlineAsmOptions,
        target_count: usize,
        unwind: UnwindAction,
        body: &Body<'tcx>,
        caller: &Instance<'tcx>,
    ) -> Result<(), CollectError> {
        if !matches!(asm_macro, InlineAsmMacro::Asm) {
            return Err(self.reachable_error(
                caller,
                "only local asm! is representable by the V1 kernel frontend contract",
                None,
            ));
        }
        if target_count != 1 || !matches!(unwind, UnwindAction::Unreachable) {
            return Err(self.reachable_error(
                caller,
                "inline assembly with labels, divergence, or unwind behavior is not representable by the V1 kernel frontend contract",
                None,
            ));
        }

        let supported_options = InlineAsmOptions::PURE
            | InlineAsmOptions::NOMEM
            | InlineAsmOptions::READONLY
            | InlineAsmOptions::PRESERVES_FLAGS
            | InlineAsmOptions::NOSTACK;
        let unsupported = options.bits() & !supported_options.bits();
        if unsupported != 0 {
            return Err(self.reachable_error(
                caller,
                &format!(
                    "inline assembly uses options {unsupported:#x} outside the V1 frontend contract"
                ),
                None,
            ));
        }

        let mut operand_bits = 0_u16;
        for operand in operands {
            operand_bits |= match operand {
                InlineAsmOperand::In { value, .. } => {
                    assembly_operand_bit(value.ty(body, self.tcx)).ok_or_else(|| {
                        self.reachable_error(
                            caller,
                            &format!(
                                "inline assembly input type `{}` has no V1 operand classification",
                                value.ty(body, self.tcx)
                            ),
                            None,
                        )
                    })?
                }
                InlineAsmOperand::Out {
                    place: Some(place), ..
                } => {
                    let ty = place.ty(body, self.tcx).ty;
                    assembly_operand_bit(ty).ok_or_else(|| {
                        self.reachable_error(
                            caller,
                            &format!(
                                "inline assembly output type `{ty}` has no V1 operand classification"
                            ),
                            None,
                        )
                    })?
                }
                InlineAsmOperand::InOut {
                    in_value,
                    out_place,
                    ..
                } => {
                    let input_ty = in_value.ty(body, self.tcx);
                    let input = assembly_operand_bit(input_ty).ok_or_else(|| {
                        self.reachable_error(
                            caller,
                            &format!(
                                "inline assembly inout type `{input_ty}` has no V1 operand classification"
                            ),
                            None,
                        )
                    })?;
                    if let Some(place) = out_place {
                        let output_ty = place.ty(body, self.tcx).ty;
                        let output = assembly_operand_bit(output_ty).ok_or_else(|| {
                            self.reachable_error(
                                caller,
                                &format!(
                                    "inline assembly inout output type `{output_ty}` has no V1 operand classification"
                                ),
                                None,
                            )
                        })?;
                        if input != output {
                            return Err(self.reachable_error(
                                caller,
                                "inline assembly inout types disagree on their V1 operand classification",
                                None,
                            ));
                        }
                    }
                    input
                }
                InlineAsmOperand::Const { .. } => ASSEMBLY_OPERAND_IMMEDIATE_V1,
                InlineAsmOperand::Out { place: None, .. }
                | InlineAsmOperand::SymFn { .. }
                | InlineAsmOperand::SymStatic { .. }
                | InlineAsmOperand::Label { .. } => {
                    return Err(self.reachable_error(
                        caller,
                        "inline assembly clobber-only, symbol, and label operands are not representable by the V1 frontend contract",
                        None,
                    ));
                }
            };
        }
        if operand_bits == 0 {
            return Err(self.reachable_error(
                caller,
                "inline assembly has no classifiable V1 operands",
                None,
            ));
        }

        let option_bits = frontend_assembly_option_bits(options);
        let identity = self.instance_identity(*caller);
        let observation = self.inline_assembly.entry(identity).or_default();
        observation.blocks = observation
            .blocks
            .checked_add(1)
            .ok_or_else(|| CollectError {
                message: "reachable inline assembly block count exceeds u32".to_owned(),
            })?;
        observation.operand_bits |= operand_bits;
        observation.option_sets.insert(option_bits);
        Ok(())
    }

    fn authenticate_reachable_frontend_contracts(&mut self) -> Result<(), CollectError> {
        let kernel_indices = self
            .result
            .iter()
            .enumerate()
            .filter_map(|(index, function)| function.is_kernel_entry().then_some(index))
            .collect::<Vec<_>>();

        for index in kernel_indices {
            let function = &self.result[index];
            let identity = self.instance_identity(function.instance);
            let observed = self.reachable_inline_assembly(&identity)?;
            let logical_name = function
                .logical_name
                .as_deref()
                .unwrap_or(&function.export_name);
            let authenticated = reconcile_frontend_contract(
                logical_name,
                &self.expected_target,
                function.frontend_contract.as_ref(),
                observed,
            )?;
            if let Some(contract) = &mut self.result[index].frontend_contract {
                contract.reachable_assembly = authenticated;
            }
        }
        Ok(())
    }

    fn authenticate_kernel_context_contracts(&mut self) -> Result<(), CollectError> {
        let functions = self
            .result
            .iter()
            .map(|function| (self.instance_identity(function.instance), function))
            .collect::<BTreeMap<_, _>>();
        let labels = functions
            .iter()
            .map(|(identity, function)| (identity.clone(), self.instance_label(function.instance)))
            .collect::<BTreeMap<_, _>>();

        let mut authenticated_sources = Vec::new();
        for root in self
            .result
            .iter()
            .filter(|function| function.is_kernel_entry())
        {
            let logical_name = root.logical_name.as_deref().unwrap_or(&root.export_name);
            let root_identity = self.instance_identity(root.instance);
            let (links, order) = root_scoped_call_chains(&self.call_edges, &labels, &root_identity);
            let contract = root.kernel_context_contract.as_ref();

            for identity in &order {
                let issuance_count = self
                    .kernel_context_issuance_calls
                    .get(identity)
                    .map_or(0, Vec::len);
                let expected = if identity == &root_identity {
                    contract.map_or(0, |contract| {
                        usize::from(contract.contract.required_issuance_count())
                    })
                } else {
                    0
                };
                if issuance_count != expected {
                    let chain = reconstruct_call_chain(&links, identity).join(" -> ");
                    return Err(CollectError {
                        message: format!(
                            "[FE2O3-CAP-AUTH001] kernel `{logical_name}` requires {expected} authenticated context issuance call(s) at its physical root but observed {issuance_count}; reachable call chain: {chain}",
                        ),
                    });
                }
            }

            let Some(bound) = contract else {
                continue;
            };
            let _canonical_bytes = bound.canonical_bytes();
            let declared = bound.contract();
            let root_path = self.tcx.def_path_str(root.instance.def_id());
            let expected_helper_name = declared.logical_helper().name();
            let helpers = self
                .result
                .iter()
                .filter(|function| {
                    sibling_item_has_name(
                        self.tcx,
                        root.instance.def_id(),
                        function.instance.def_id(),
                        expected_helper_name,
                    )
                })
                .collect::<Vec<_>>();
            let expected_helper_path =
                sibling_item_diagnostic_path(&root_path, expected_helper_name);
            let [helper] = helpers.as_slice() else {
                return Err(CollectError {
                    message: format!(
                        "[FE2O3-CAP-AUTH002] kernel-context contract `{}` requires exactly one reachable logical helper `{expected_helper_path}`; found {}",
                        bound.registration_path,
                        helpers.len()
                    ),
                });
            };
            if helper.role != CollectedFunctionRole::InternalHelper {
                return Err(CollectError {
                    message: format!(
                        "[FE2O3-CAP-AUTH002] declared logical helper `{expected_helper_path}` is not an internal helper"
                    ),
                });
            }
            let helper_identity = self.instance_identity(helper.instance);
            if !self
                .call_edges
                .get(&root_identity)
                .is_some_and(|callees| callees.contains(&helper_identity))
            {
                return Err(CollectError {
                    message: format!(
                        "[FE2O3-CAP-AUTH002] physical root `{root_path}` does not call its declared logical helper `{expected_helper_path}` directly"
                    ),
                });
            }

            let marker_name = declared.nominal_kernel_marker().name();
            let marker_path = sibling_item_diagnostic_path(&root_path, marker_name);
            let marker_defs = self
                .tcx
                .hir_free_items()
                .filter_map(|item_id| {
                    let item = self.tcx.hir_item(item_id);
                    let def_id = item.owner_id.def_id.to_def_id();
                    sibling_item_has_name(self.tcx, root.instance.def_id(), def_id, marker_name)
                        .then_some(def_id)
                })
                .collect::<Vec<_>>();
            let [marker_def_id] = marker_defs.as_slice() else {
                return Err(CollectError {
                    message: format!(
                        "[FE2O3-CAP-AUTH003] kernel-context contract requires exactly one nominal marker `{marker_path}`; found {}",
                        marker_defs.len()
                    ),
                });
            };
            let marker_ty = self.tcx.type_of(*marker_def_id).instantiate_identity();
            let TyKind::Adt(marker_adt, marker_args) = marker_ty.kind() else {
                return Err(CollectError {
                    message: format!(
                        "[FE2O3-CAP-AUTH003] nominal marker `{marker_path}` is not an ADT"
                    ),
                });
            };
            if !marker_adt.is_enum() || !marker_adt.variants().is_empty() || !marker_args.is_empty()
            {
                return Err(CollectError {
                    message: format!(
                        "[FE2O3-CAP-AUTH003] nominal marker `{marker_path}` is not the exact uninhabited nongeneric enum shape"
                    ),
                });
            }

            let root_signature = self.tcx.normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                self.tcx.instantiate_bound_regions_with_erased(
                    self.tcx
                        .fn_sig(root.instance.def_id())
                        .instantiate(self.tcx, root.instance.args),
                ),
            );
            let helper_signature = self.tcx.normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                self.tcx.instantiate_bound_regions_with_erased(
                    self.tcx
                        .fn_sig(helper.instance.def_id())
                        .instantiate(self.tcx, helper.instance.args),
                ),
            );
            let Some((context_ty, helper_physical_inputs)) =
                helper_signature.inputs().split_first()
            else {
                return Err(CollectError {
                    message: format!(
                        "[FE2O3-CAP-ABI001] logical helper `{expected_helper_path}` has no context input"
                    ),
                });
            };
            let argument_relation = if helper_physical_inputs.len() != root_signature.inputs().len()
            {
                Err(format!(
                    "physical/logical argument counts differ: {}/{}",
                    root_signature.inputs().len(),
                    helper_physical_inputs.len(),
                ))
            } else {
                helper_physical_inputs
                    .iter()
                    .zip(root_signature.inputs())
                    .enumerate()
                    .try_for_each(|(ordinal, (logical, physical))| {
                        authenticate_logical_physical_kernel_argument_v1(
                            self.tcx, marker_ty, *logical, *physical,
                        )
                        .map_err(|detail| {
                            format!(
                                "physical/logical argument {ordinal} violates the exact identity-or-capability-carrier relation: {detail}"
                            )
                        })
                    })
            };
            let return_relation = match authenticate_logical_physical_kernel_signature_v1(
                self.tcx,
                argument_relation,
                helper_signature.output(),
                root_signature.output(),
                helper_signature.abi,
                root_signature.abi,
                helper_signature.safety,
                root_signature.safety,
                helper_signature.c_variadic,
                root_signature.c_variadic,
            ) {
                Ok(relation) => relation,
                Err(detail) => {
                    return Err(logical_physical_kernel_abi_error_v1(
                        &expected_helper_path,
                        &root_path,
                        declared.context_source_ordinal(),
                        &detail,
                    ));
                }
            };
            let TyKind::Adt(context_adt, context_args) = context_ty.kind() else {
                return Err(CollectError {
                    message: format!(
                        "[FE2O3-CAP-AUTH004] logical helper `{expected_helper_path}` first input is not the trusted KernelContext ADT"
                    ),
                });
            };
            if crate::trusted_device_items::classify(self.tcx, context_adt.did())
                != Some(crate::trusted_device_items::TrustedDeviceItem::KernelContext)
            {
                return Err(CollectError {
                    message: format!(
                        "[FE2O3-CAP-AUTH004] logical helper `{expected_helper_path}` first input is not the trusted KernelContext ADT"
                    ),
                });
            }
            let context_types = context_args.types().collect::<Vec<_>>();
            if context_args.len() != 4
                || !matches!(context_args[0].kind(), GenericArgKind::Lifetime(_))
                || context_types.len() != 3
                || context_types[0] != marker_ty
                || !exact_reviewed_marker_v1(
                    self.tcx,
                    context_types[1],
                    "fe2o3_device::context::CurrentTarget",
                )
                || !exact_reviewed_marker_v1(
                    self.tcx,
                    context_types[2],
                    "fe2o3_device::context::RegisteredLaunch",
                )
            {
                return Err(CollectError {
                    message: format!(
                        "[FE2O3-CAP-AUTH005] logical helper `{expected_helper_path}` KernelContext does not carry the exact nominal marker `{marker_path}`, target, and launch brands"
                    ),
                });
            }
            authenticate_nonphysical_kernel_context_abi_v1(
                self.tcx,
                root.instance,
                helper.instance,
                *context_ty,
                root_signature.inputs().len(),
                helper_signature.inputs().len(),
                return_relation,
            )
            .map_err(|detail| CollectError {
                message: format!(
                    "[FE2O3-CAP-ABI001] logical helper `{expected_helper_path}` context ordinal {} is not a nonphysical kernarg: {detail}",
                    declared.context_source_ordinal(),
                ),
            })?;

            let issuance = self
                .kernel_context_issuance_calls
                .get(&root_identity)
                .and_then(|issuances| issuances.first())
                .copied()
                .ok_or_else(|| CollectError {
                    message: format!(
                        "[FE2O3-CAP-AUTH001] kernel `{logical_name}` lost its authenticated context issuance"
                    ),
                })?;
            let root_function_identity =
                crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(
                    self.tcx,
                    root.instance,
                )
                .function();
            let mir_body_identity =
                crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(self.tcx, root.instance);
            let block_identity = crate::rustc_semantic_adapter_v1::rustc_block_identity_v1(
                root_function_identity,
                mir_body_identity,
                issuance.rustc_block,
            );
            let kernel_marker_identity =
                crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(self.tcx, marker_ty);
            let issuance_identity = derive_kernel_context_issuance_identity_v1(
                root_function_identity.as_bytes(),
                &mir_body_identity,
                block_identity.as_bytes(),
                &issuance.terminal_function_identity,
                kernel_marker_identity.as_bytes(),
            );
            let physical_argument_count =
                u32::try_from(root_signature.inputs().len()).map_err(|_| CollectError {
                    message: "physical kernel argument count exceeds u32".to_owned(),
                })?;
            let logical_argument_count =
                u32::try_from(helper_signature.inputs().len()).map_err(|_| CollectError {
                    message: "logical kernel argument count exceeds u32".to_owned(),
                })?;
            if logical_argument_count.checked_sub(physical_argument_count) != Some(1) {
                return Err(CollectError {
                    message: format!(
                        "[FE2O3-CAP-ABI001] logical helper `{expected_helper_path}` does not add exactly one nonphysical context argument"
                    ),
                });
            }
            authenticated_sources.push((
                root_identity,
                AuthenticatedKernelContextSourceV1 {
                    root_function_identity: *root_function_identity.as_bytes(),
                    kernel_marker_identity: *kernel_marker_identity.as_bytes(),
                    issuance_identity,
                    physical_argument_count,
                    logical_argument_count,
                },
            ));
        }

        for (root_identity, authenticated_source) in authenticated_sources {
            let root_index = self
                .result
                .iter()
                .position(|function| {
                    function.is_kernel_entry()
                        && self.instance_identity(function.instance) == root_identity
                })
                .expect("authenticated context root remains in the collected closure");
            let bound = self.result[root_index]
                .kernel_context_contract
                .as_mut()
                .expect("authenticated context root retains its bound sidecar");
            if bound
                .authenticated_source
                .replace(authenticated_source)
                .is_some()
            {
                return Err(CollectError {
                    message: "kernel-context source was authenticated more than once".to_owned(),
                });
            }
        }
        Ok(())
    }

    fn authenticate_production_kernel_source_safety(&self) -> Result<(), CollectError> {
        let functions = self
            .result
            .iter()
            .map(|function| (self.instance_identity(function.instance), function))
            .collect::<BTreeMap<_, _>>();
        let labels = functions
            .iter()
            .map(|(identity, function)| (identity.clone(), self.instance_label(function.instance)))
            .collect::<BTreeMap<_, _>>();

        for root in self
            .result
            .iter()
            .filter(|function| function.is_kernel_entry())
        {
            let frontend = root
                .frontend_contract
                .as_ref()
                .map(|contract| contract.contract);
            if frontend
                .and_then(KernelFrontendContractV1::unsafe_assembly)
                .is_some()
            {
                continue;
            }
            let raw_memory_provider =
                frontend.is_some_and(KernelFrontendContractV1::unsafe_raw_memory_provider);
            let logical_name = root.logical_name.as_deref().unwrap_or(&root.export_name);
            let root_identity = self.instance_identity(root.instance);
            let (links, order) = root_scoped_call_chains(&self.call_edges, &labels, &root_identity);
            let raw_memory_helper_identity = raw_memory_provider
                .then(|| {
                    root.kernel_context_contract
                        .as_ref()
                        .map(|bound| bound.contract().logical_helper().name())
                })
                .flatten()
                .and_then(|expected_name| {
                    functions.iter().find_map(|(identity, function)| {
                        sibling_item_has_name(
                            self.tcx,
                            root.instance.def_id(),
                            function.instance.def_id(),
                            expected_name,
                        )
                        .then(|| identity.clone())
                    })
                });

            for identity in order {
                let function = functions
                    .get(&identity)
                    .expect("root-scoped traversal retains only collected function labels");
                let chain = || reconstruct_call_chain(&links, &identity).join(" -> ");
                let authenticated_raw_terminal = raw_memory_provider
                    && is_authenticated_unsafe_raw_memory_terminal_v1(
                        crate::production_semantic_terminal_v1::classify(
                            self.tcx,
                            function.instance.def_id(),
                        ),
                    );
                let authenticated_raw_boundary = raw_memory_provider
                    && (&identity == &root_identity
                        || raw_memory_helper_identity.as_ref() == Some(&identity));
                if callable_safety(self.tcx, function.instance) == Safety::Unsafe
                    && !authenticated_raw_boundary
                    && !authenticated_raw_terminal
                    && !crate::production_rustc_intrinsic_v1::is_reviewed_core_atomic_function_v1(
                        self.tcx,
                        function.instance,
                    )
                {
                    return Err(CollectError {
                        message: format!(
                            "[FE2O3-CAP-SOURCE001] FE2O3-CAP Rejected root={logical_name} span=unresolved helper_chain={} stage=source-safety: production kernel `{logical_name}` reaches an unsafe function outside its authenticated low-level boundary: `{}`; reachable call chain: {}",
                            chain(),
                            self.instance_label(function.instance),
                            chain(),
                        ),
                    });
                }
                if let Some(callees) = self.reachable_unsafe_calls.get(&identity)
                    && let Some(callee) = callees.first()
                {
                    return Err(CollectError {
                        message: format!(
                            "[FE2O3-CAP-SOURCE002] FE2O3-CAP Rejected root={logical_name} span=unresolved helper_chain={}->{callee} stage=source-safety: ordinary production kernel `{logical_name}` reaches unsafe function instance `{callee}`; reachable call chain: {} -> {callee}",
                            chain(),
                            chain(),
                        ),
                    });
                }

                if crate::trusted_device_items::classify(self.tcx, function.instance.def_id())
                    .is_some_and(
                        crate::production_semantic_terminal_v1::is_traversed_reviewed_helper_v1,
                    )
                    || crate::production_rustc_intrinsic_v1::is_reviewed_device_global_mut_ptr_as_raw_v1(
                        self.tcx,
                        function.instance,
                    )
                {
                    continue;
                }
                let Some(local_def_id) = function.instance.def_id().as_local() else {
                    if crate::trusted_device_items::authenticate_reviewed_safe_core_scalar_bitcast_helper_v1(
                        self.tcx,
                        function.instance,
                    ) {
                        continue;
                    }
                    if crate::trusted_device_items::authenticate_reviewed_safe_core_fabs_f32_helper_v1(
                        self.tcx,
                        function.instance,
                    ) {
                        continue;
                    }
                    if crate::trusted_device_items::authenticate_reviewed_safe_core_f32_is_finite_helper_v1(
                        self.tcx,
                        function.instance,
                    ) {
                        continue;
                    }
                    if crate::production_rustc_intrinsic_v1::is_reviewed_core_atomic_function_v1(
                        self.tcx,
                        function.instance,
                    ) {
                        continue;
                    }
                    match crate::trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
                        self.tcx,
                        function.instance.def_id(),
                    ) {
                        Ok(true) => continue,
                        Ok(false) => {}
                        Err(detail) => {
                            return Err(CollectError {
                                message: format!(
                                    "[FE2O3-CAP-SOURCE003] FE2O3-CAP Rejected root={logical_name} span=unresolved helper_chain={} stage=source-safety: ordinary production kernel `{logical_name}` rejected reviewed external helper `{}`: {detail}; reachable call chain: {}",
                                    chain(),
                                    self.instance_label(function.instance),
                                    chain(),
                                ),
                            });
                        }
                    }
                    return Err(CollectError {
                        message: format!(
                            "[FE2O3-CAP-SOURCE004] FE2O3-CAP Rejected root={logical_name} span=unresolved helper_chain={} stage=source-safety: ordinary production kernel `{logical_name}` cannot authenticate the absence of user-provided unsafe blocks in external helper `{}`: cross-crate HIR is unavailable and optimized MIR does not retain unsafe-block syntax; reachable call chain: {}",
                            chain(),
                            self.instance_label(function.instance),
                            chain(),
                        ),
                    });
                };
                let Some(body) = self.tcx.hir_maybe_body_owned_by(local_def_id) else {
                    return Err(CollectError {
                        message: format!(
                            "[FE2O3-CAP-SOURCE005] FE2O3-CAP Rejected root={logical_name} span=unresolved helper_chain={} stage=source-safety: ordinary production kernel `{logical_name}` cannot authenticate local HIR for reachable function `{}`; reachable call chain: {}",
                            chain(),
                            self.instance_label(function.instance),
                            chain(),
                        ),
                    });
                };
                let authenticated_context_root = identity == root_identity
                    && root
                        .kernel_context_contract
                        .as_ref()
                        .is_some_and(|bound| bound.authenticated_source.is_some());
                let mut visitor = UserUnsafeBlockVisitor::new(
                    self.tcx,
                    local_def_id,
                    raw_memory_provider,
                    authenticated_context_root,
                );
                visitor.visit_body(body);
                if let Some(span) = visitor.first_span {
                    let span = self.tcx.sess.source_map().span_to_diagnostic_string(span);
                    return Err(CollectError {
                        message: format!(
                            "[FE2O3-CAP-SOURCE006] FE2O3-CAP Rejected root={logical_name} span={span} helper_chain={} stage=source-safety: ordinary production kernel `{logical_name}` reaches a safe-signature local helper containing a user-provided unsafe block at {span}; reachable call chain: {}",
                            chain(),
                            chain(),
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    fn reachable_inline_assembly(
        &self,
        root: &crate::device_ffi::DeviceFfiInstanceIdentity,
    ) -> Result<ObservedInlineAssemblyV1, CollectError> {
        let mut visited = BTreeSet::new();
        let mut pending = vec![root.clone()];
        let mut summary = ObservedInlineAssemblyV1::default();
        while let Some(identity) = pending.pop() {
            if !visited.insert(identity.clone()) {
                continue;
            }
            if let Some(observed) = self.inline_assembly.get(&identity) {
                summary.blocks =
                    summary
                        .blocks
                        .checked_add(observed.blocks)
                        .ok_or_else(|| CollectError {
                            message: "reachable inline assembly block count exceeds u32".to_owned(),
                        })?;
                summary.operand_bits |= observed.operand_bits;
                summary
                    .option_sets
                    .extend(observed.option_sets.iter().copied());
            }
            if let Some(callees) = self.call_edges.get(&identity) {
                pending.extend(callees.iter().cloned());
            }
        }
        Ok(summary)
    }

    fn process_call_operand(
        &mut self,
        rustc_block: u32,
        func: &Operand<'tcx>,
        caller: &Instance<'tcx>,
    ) -> Result<(), CollectError> {
        let Operand::Constant(const_op) = func else {
            return Err(self.reachable_error(
                caller,
                "[FE2O3-FFI-CALL001] indirect function-pointer calls are not permitted in the closed device graph",
                None,
            ));
        };

        let ty = const_op.const_.ty();
        let TyKind::FnDef(def_id, args) = ty.kind() else {
            return Err(self.reachable_error(
                caller,
                &format!(
                    "[FE2O3-FFI-CALL002] device call operand has non-function-definition type `{ty}`"
                ),
                None,
            ));
        };
        if let Some(rejection) = crate::trusted_device_items::rejected_provider(self.tcx, *def_id) {
            return Err(self.reachable_error(
                caller,
                &format!(
                    "trusted-provider rejection: diagnostic item `{}` is defined by `{}` but is not bound to the reviewed `{}` compilation unit: {}",
                    rejection.marker,
                    self.tcx.def_path_str(*def_id),
                    rejection.expected_provider_crate,
                    rejection.reason,
                ),
                None,
            ));
        }
        let callee_path = self.tcx.def_path_str(*def_id);
        if callee_path.contains("::panicking::")
            || callee_path.contains("::panic_fmt")
            || callee_path.contains("::begin_panic")
            || callee_path.contains("::unwrap_failed")
            || callee_path.contains("precondition_check")
        {
            return Err(self.reachable_error(
                caller,
                "device code reaches a panic path",
                Some(callee_path),
            ));
        }

        let normalized_args = self.tcx.instantiate_and_normalize_erasing_regions(
            caller.args,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(*args),
        );
        let resolved = match Instance::try_resolve(
            self.tcx,
            TypingEnv::fully_monomorphized(),
            *def_id,
            normalized_args,
        ) {
            Ok(Some(resolved)) => resolved,
            Ok(None) => {
                return Err(self.reachable_error(
                    caller,
                    "direct device callee could not be resolved to a concrete rustc instance",
                    Some(self.tcx.def_path_str(*def_id)),
                ));
            }
            Err(_) => {
                return Err(self.reachable_error(
                    caller,
                    "direct device callee normalization failed",
                    Some(self.tcx.def_path_str(*def_id)),
                ));
            }
        };
        let semantic_terminal =
            crate::production_semantic_terminal_v1::classify(self.tcx, resolved.def_id());
        let authenticated_unsafe_raw_memory_terminal =
            is_authenticated_unsafe_raw_memory_terminal_v1(semantic_terminal);
        let kernel_context_issuance = matches!(
            semantic_terminal,
            Some(crate::production_semantic_terminal_v1::ProductionSemanticTerminalRuleV1::Expand(
                crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1::KernelContextIssue
            ))
        );
        if kernel_context_issuance {
            let caller_identity = self.instance_identity(*caller);
            let terminal_function_identity =
                crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(
                    self.tcx, resolved,
                )
                .function();
            self.kernel_context_issuance_calls
                .entry(caller_identity)
                .or_default()
                .push(ObservedKernelContextIssuanceV1 {
                    rustc_block,
                    terminal_function_identity: *terminal_function_identity.as_bytes(),
                });
        }
        let normalized_intrinsic = if semantic_terminal.is_none() {
            crate::production_rustc_intrinsic_v1::classify(self.tcx, resolved).map_err(|error| {
                self.reachable_error(
                    caller,
                    &format!("unsupported rustc compiler intrinsic: {error}"),
                    Some(self.instance_label(resolved)),
                )
            })?
        } else {
            None
        };
        if normalized_intrinsic.is_none()
            && !kernel_context_issuance
            && !authenticated_unsafe_raw_memory_terminal
            && callable_safety(self.tcx, resolved) == Safety::Unsafe
            && !crate::production_rustc_intrinsic_v1::is_reviewed_core_atomic_function_v1(
                self.tcx, resolved,
            )
        {
            let caller_identity = self.instance_identity(*caller);
            self.reachable_unsafe_calls
                .entry(caller_identity)
                .or_default()
                .insert(format!("{callee_path} [{ty}]"));
        }
        let marked_contract = crate::device_ffi::contract_assertion_for_def(self.tcx, *def_id)
            .map_err(|error| self.reachable_error(caller, &error.to_string(), None))?;
        if marked_contract.is_some() {
            let instance = resolved;
            let contract =
                crate::device_ffi::contract_for_instance(self.tcx, instance, &self.expected_target)
                    .map_err(|error| self.reachable_error(caller, &error.to_string(), None))?
                    .ok_or_else(|| {
                        self.reachable_error(
                            caller,
                            "resolved device FFI declaration lost its compiler marker",
                            Some(self.tcx.def_path_str(*def_id)),
                        )
                    })?;
            let declaration =
                crate::device_ffi::collected_declaration(self.tcx, instance, contract.clone());
            if let Some(existing) = self.ffi_declarations.iter().find(|entry| {
                entry.owner.def_path_hash == declaration.owner.def_path_hash
                    && entry.owner.concrete_instance_symbol
                        == declaration.owner.concrete_instance_symbol
            }) {
                if existing.contract != contract {
                    return Err(self.reachable_error(
                        caller,
                        "reachable device FFI marker disagrees with its collected declaration",
                        Some(self.tcx.def_path_str(*def_id)),
                    ));
                }
            } else {
                crate::device_ffi::enforce_contract_bound(self.ffi_declarations.len() + 1)
                    .map_err(|error| self.reachable_error(caller, &error.to_string(), None))?;
                self.ffi_declarations.push(declaration);
            }
            if contract.direction == crate::device_ffi::DeviceFfiDirection::Import {
                self.reachable_ffi_imports.insert(contract.id);
                if self.verbose {
                    eprintln!(
                        "[collector] external device import: {} -> {} ({})",
                        self.tcx.def_path_str(instance.def_id()),
                        contract.symbol,
                        contract.id.to_hex(),
                    );
                }
                return Ok(());
            }
        }

        // Only an exact rustc diagnostic-item identity may terminate
        // collection without traversing the callee body.
        let registered_terminal =
            crate::production_semantic_terminal_v1::classify(self.tcx, *def_id).is_some();
        if registered_terminal {
            if self.verbose {
                eprintln!(
                    "[collector] stopping at trusted device item {}",
                    self.tcx.def_path_str(*def_id)
                );
            }
            return Ok(());
        }

        if normalized_intrinsic.is_some() {
            if self.verbose {
                eprintln!(
                    "[collector] stopping at normalized rustc intrinsic {}",
                    self.tcx.def_path_str(resolved.def_id())
                );
            }
            return Ok(());
        }

        match self.should_collect_from_crate(*def_id) {
            CollectDecision::Collect => {}
            CollectDecision::Forbidden {
                crate_name,
                fn_path,
            } => {
                return Err(self.reachable_error(
                    caller,
                    &format!(
                        "device code reached forbidden crate `{crate_name}`; device-reachable functions must avoid `std`"
                    ),
                    Some(fn_path),
                ));
            }
        }

        let identity = self.instance_identity(resolved);
        let caller_identity = self.instance_identity(*caller);
        self.call_edges
            .entry(caller_identity.clone())
            .or_default()
            .insert(identity.clone());
        if self.seen.contains(&identity) {
            return Ok(());
        }

        if !is_fully_monomorphized(self.tcx, resolved) {
            return Err(self.reachable_error(
                caller,
                "direct device callee did not resolve to a fully monomorphized instance",
                Some(self.instance_label(resolved)),
            ));
        }

        // Collection stops only at the workload-neutral reviewed device
        // registry. The sole semantic importer applies each registered item's
        // explicit expand-or-reject rule.
        if crate::production_semantic_terminal_v1::classify(self.tcx, resolved.def_id()).is_some() {
            if self.verbose {
                eprintln!(
                    "[collector] stopping at registered semantic terminal {}",
                    self.tcx.def_path_str(resolved.def_id())
                );
            }
            return Ok(());
        }

        if !matches!(resolved.def, InstanceKind::Item(_)) {
            return Err(self.reachable_error(
                caller,
                &format!(
                    "[FE2O3-FFI-CALL003] rustc-generated callable instance `{:?}` is not traversable under the V1 device graph policy",
                    resolved.def
                ),
                Some(self.instance_label(resolved)),
            ));
        }

        // Trait calls name the trait method in MIR, so classification must be
        // repeated against the exact concrete implementation selected by
        // rustc. The classifier authenticates the implementation through the
        // diagnostic-item-marked device type and the exact lang-item trait.
        if let Some(rejection) =
            crate::trusted_device_items::rejected_provider(self.tcx, resolved.def_id())
        {
            return Err(self.reachable_error(
                caller,
                &format!(
                    "trusted-provider rejection: diagnostic item `{}` is defined by `{}` but is not bound to the reviewed `{}` compilation unit: {}",
                    rejection.marker,
                    self.tcx.def_path_str(resolved.def_id()),
                    rejection.expected_provider_crate,
                    rejection.reason,
                ),
                None,
            ));
        }

        if !self.tcx.is_mir_available(resolved.def_id()) {
            return Err(self.reachable_error(
                caller,
                "MIR is unavailable for a device-reachable item; compile the dependency with encoded MIR (for example, an inline Rust definition) or keep the call out of device code",
                Some(self.instance_label(resolved)),
            ));
        }

        if self.is_unreachable_body(resolved.def_id()) {
            return Err(self.reachable_error(
                caller,
                "device code reaches a panic path",
                Some(self.instance_label(resolved)),
            ));
        }

        let name = self.fqdn(resolved.def_id());
        let export_name = self.compute_export_name(&name, resolved);

        if self.verbose {
            eprintln!("[collector] callee: {name} -> {export_name}");
        }

        let inserted = self.mark_seen(identity.clone())?;
        debug_assert!(inserted);
        self.call_chains.insert(
            identity,
            CallChainLink {
                predecessor: Some(caller_identity),
                label: self.instance_label(resolved),
            },
        );
        self.worklist.push_back(CollectedFunction {
            instance: resolved,
            role: CollectedFunctionRole::InternalHelper,
            export_name,
            logical_name: None,
            generated_host_contract_identity: None,
            kernel_binding: None,
            frontend_contract: None,
            kernel_context_contract: None,
            reference_effect_binding: None,
            dead_branches: None,
            closure_plan: None,
        });
        Ok(())
    }

    fn instance_identity(
        &self,
        instance: Instance<'tcx>,
    ) -> crate::device_ffi::DeviceFfiInstanceIdentity {
        crate::device_ffi::stable_instance_identity(self.tcx, instance)
    }

    fn instance_label(&self, instance: Instance<'tcx>) -> String {
        format!(
            "{} [{}]",
            self.fqdn(instance.def_id()),
            self.tcx.symbol_name(instance).name
        )
    }

    fn call_chain(&self, instance: &Instance<'tcx>) -> Vec<String> {
        let identity = self.instance_identity(*instance);
        let chain = reconstruct_call_chain(&self.call_chains, &identity);
        if chain.is_empty() {
            vec![self.instance_label(*instance)]
        } else {
            chain
        }
    }

    fn reachable_error(
        &self,
        caller: &Instance<'tcx>,
        reason: &str,
        callee: Option<String>,
    ) -> CollectError {
        let mut chain = self.call_chain(caller);
        if let Some(callee) = callee {
            chain.push(callee);
        }
        CollectError {
            message: format!(
                "fe2o3 device collection rejected a reachable call: {reason}; reachable call chain: {}",
                chain.join(" -> ")
            ),
        }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LogicalKernelReturnRelationV1 {
    Exact,
    TrustedKernelResultToUnit,
    Mismatch,
}

fn logical_physical_kernel_abi_error_v1(
    logical_helper: &str,
    physical_root: &str,
    context_ordinal: u16,
    detail: &str,
) -> CollectError {
    CollectError {
        message: format!(
            "[FE2O3-CAP-ABI001] logical helper `{logical_helper}` does not equal physical root `{physical_root}` after removing only context ordinal {context_ordinal}: {detail}",
        ),
    }
}

fn authenticate_logical_physical_kernel_signature_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    argument_relation: Result<(), String>,
    logical_output: Ty<'tcx>,
    physical_output: Ty<'tcx>,
    logical_abi: rustc_abi::ExternAbi,
    physical_abi: rustc_abi::ExternAbi,
    logical_safety: Safety,
    physical_safety: Safety,
    logical_c_variadic: bool,
    physical_c_variadic: bool,
) -> Result<LogicalKernelReturnRelationV1, String> {
    authenticate_kernel_signature_components_v1(
        argument_relation,
        logical_kernel_return_relation_v1(tcx, logical_output, physical_output),
        logical_abi,
        physical_abi,
        logical_safety,
        physical_safety,
        logical_c_variadic,
        physical_c_variadic,
    )
}

fn authenticate_kernel_signature_components_v1<Abi: Eq, SignatureSafety: Eq>(
    argument_relation: Result<(), String>,
    return_relation: LogicalKernelReturnRelationV1,
    logical_abi: Abi,
    physical_abi: Abi,
    logical_safety: SignatureSafety,
    physical_safety: SignatureSafety,
    logical_c_variadic: bool,
    physical_c_variadic: bool,
) -> Result<LogicalKernelReturnRelationV1, String> {
    argument_relation?;
    if logical_abi != physical_abi {
        return Err("physical/logical calling ABIs differ".to_owned());
    }
    if logical_safety != physical_safety {
        return Err("physical/logical safety qualifiers differ".to_owned());
    }
    if logical_c_variadic != physical_c_variadic {
        return Err("physical/logical variadic qualifiers differ".to_owned());
    }
    if return_relation == LogicalKernelReturnRelationV1::Mismatch {
        return Err(
            "physical/logical return types differ outside the exact trusted KernelResult-to-unit entry normalization"
                .to_owned(),
        );
    }
    Ok(return_relation)
}

fn logical_kernel_return_relation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    logical: Ty<'tcx>,
    physical: Ty<'tcx>,
) -> LogicalKernelReturnRelationV1 {
    if logical == physical {
        return LogicalKernelReturnRelationV1::Exact;
    }
    if physical.is_unit() && exact_trusted_kernel_result_v1(tcx, logical) {
        return LogicalKernelReturnRelationV1::TrustedKernelResultToUnit;
    }
    LogicalKernelReturnRelationV1::Mismatch
}

fn exact_trusted_kernel_result_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    let TyKind::Adt(result, arguments) = ty.kind() else {
        return false;
    };
    if !tcx.is_diagnostic_item(sym::Result, result.did()) || arguments.len() != 2 {
        return false;
    }
    let Some(ok) = arguments[0].as_type() else {
        return false;
    };
    let Some(error) = arguments[1].as_type() else {
        return false;
    };
    let TyKind::Adt(error, error_arguments) = error.kind() else {
        return false;
    };
    ok.is_unit()
        && error_arguments.is_empty()
        && crate::trusted_device_items::classify(tcx, error.did())
            == Some(crate::trusted_device_items::TrustedDeviceItem::KernelError)
}

fn authenticate_nonphysical_kernel_context_abi_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    physical_root: Instance<'tcx>,
    helper: Instance<'tcx>,
    context: Ty<'tcx>,
    physical_argument_count: usize,
    logical_argument_count: usize,
    return_relation: LogicalKernelReturnRelationV1,
) -> Result<(), String> {
    let layout = LayoutCx::new(tcx, TypingEnv::fully_monomorphized())
        .layout_of(context)
        .map_err(|error| format!("KernelContext layout is unavailable: {error}"))?;
    if layout.size.bytes() != 0 {
        return Err("KernelContext has a nonzero rustc layout".to_owned());
    }

    let typing_env = TypingEnv::fully_monomorphized();
    let helper_query = typing_env.as_query_input((helper, ty::List::empty()));
    let helper_abi = tcx
        .fn_abi_of_instance(helper_query)
        .map_err(|error| format!("logical helper FnAbi is unavailable: {error:?}"))?;
    if helper_abi.args.len() != logical_argument_count {
        return Err("logical helper FnAbi changed its source argument count".to_owned());
    }
    let Some(argument) = helper_abi.args.first() else {
        return Err("logical helper FnAbi omitted context ordinal zero".to_owned());
    };
    if argument.layout.ty != context {
        return Err("logical helper FnAbi rebound context ordinal zero".to_owned());
    }
    if !matches!(&argument.mode, PassMode::Ignore) {
        return Err("KernelContext rustc pass mode is not Ignore".to_owned());
    }

    let root_query = typing_env.as_query_input((physical_root, ty::List::empty()));
    let root_abi = tcx
        .fn_abi_of_instance(root_query)
        .map_err(|error| format!("physical root FnAbi is unavailable: {error:?}"))?;
    if root_abi.args.len() != physical_argument_count
        || helper_abi.args.len().checked_sub(1) != Some(root_abi.args.len())
    {
        return Err(
            "physical root FnAbi does not equal the logical helper FnAbi argument count after one ignored context"
                .to_owned(),
        );
    }
    for (ordinal, (logical, physical)) in helper_abi.args[1..]
        .iter()
        .zip(root_abi.args.iter())
        .enumerate()
    {
        if !logical.eq_abi(physical) {
            return Err(format!(
                "physical/logical argument {ordinal} has a different adjusted rustc ABI"
            ));
        }
    }
    if helper_abi.conv != root_abi.conv {
        return Err("physical/logical adjusted calling conventions differ".to_owned());
    }
    if helper_abi.can_unwind != root_abi.can_unwind {
        return Err("physical/logical adjusted unwind ABIs differ".to_owned());
    }
    if helper_abi.c_variadic != root_abi.c_variadic {
        return Err("physical/logical adjusted variadic ABIs differ".to_owned());
    }
    if helper_abi.fixed_count.checked_sub(1) != Some(root_abi.fixed_count) {
        return Err(
            "physical root adjusted fixed-argument count did not remove exactly one context"
                .to_owned(),
        );
    }
    match return_relation {
        LogicalKernelReturnRelationV1::Exact if !helper_abi.ret.eq_abi(&root_abi.ret) => {
            return Err("physical/logical adjusted return ABIs differ".to_owned());
        }
        LogicalKernelReturnRelationV1::TrustedKernelResultToUnit
            if !root_abi.ret.layout.ty.is_unit()
                || !matches!(&root_abi.ret.mode, PassMode::Ignore) =>
        {
            return Err(
                "trusted KernelResult normalization did not produce the exact ignored unit entry return ABI"
                    .to_owned(),
            );
        }
        LogicalKernelReturnRelationV1::Mismatch => {
            return Err("unreviewed physical/logical return relation reached FnAbi".to_owned());
        }
        LogicalKernelReturnRelationV1::Exact
        | LogicalKernelReturnRelationV1::TrustedKernelResultToUnit => {}
    }
    Ok(())
}

fn authenticate_logical_physical_kernel_argument_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    marker_ty: Ty<'tcx>,
    logical: Ty<'tcx>,
    physical: Ty<'tcx>,
) -> Result<(), String> {
    if logical == physical {
        return Ok(());
    }

    let TyKind::Adt(view, view_arguments) = logical.kind() else {
        return Err("nonidentical logical type is not a reviewed capability view".to_owned());
    };
    if crate::trusted_device_items::classify(tcx, view.did())
        != Some(crate::trusted_device_items::TrustedDeviceItem::CapabilityMemoryView)
    {
        return Err("nonidentical logical type is not the reviewed capability view".to_owned());
    }
    if view_arguments.len() != 5 || !matches!(view_arguments[0].kind(), GenericArgKind::Lifetime(_))
    {
        return Err(
            "reviewed capability view changed its lifetime/type/const generic argument shape"
                .to_owned(),
        );
    }
    let view_types = view_arguments.types().collect::<Vec<_>>();
    let [element, space, role, brand] = view_types.as_slice() else {
        return Err("reviewed capability view changed its generic type arity".to_owned());
    };
    let TyKind::Adt(space_definition, space_arguments) = space.kind() else {
        return Err("capability view address space is not a reviewed marker".to_owned());
    };
    if !space_arguments.is_empty()
        || crate::trusted_device_items::classify(tcx, space_definition.did())
            != Some(crate::trusted_device_items::TrustedDeviceItem::CapabilityGlobalAddressSpace)
    {
        return Err("logical capability view is not in global memory".to_owned());
    }
    authenticate_kernel_capability_brand_v1(tcx, marker_ty, *brand)?;

    let TyKind::Adt(role_definition, role_arguments) = role.kind() else {
        return Err("capability view role is not a reviewed marker".to_owned());
    };
    let role_types = role_arguments.types().collect::<Vec<_>>();
    let role = crate::trusted_device_items::classify(tcx, role_definition.did());
    match role {
        Some(crate::trusted_device_items::TrustedDeviceItem::CapabilityReadOnly)
            if role_arguments.is_empty() =>
        {
            authenticate_physical_slice_v1(*element, physical, Mutability::Not)?;
        }
        Some(crate::trusted_device_items::TrustedDeviceItem::CapabilityAtomicReadWrite)
            if role_arguments.len() == 1 && role_types.len() == 1 =>
        {
            authenticate_physical_slice_v1(*element, physical, Mutability::Not)?;
        }
        Some(crate::trusted_device_items::TrustedDeviceItem::CapabilityExclusiveReadWrite)
            if role_arguments.is_empty() =>
        {
            authenticate_physical_slice_v1(*element, physical, Mutability::Mut)?;
        }
        Some(crate::trusted_device_items::TrustedDeviceItem::CapabilityDisjointWrite)
            if role_arguments.len() == 1 && role_types.len() == 1 =>
        {
            let TyKind::Adt(carrier, carrier_arguments) = physical.kind() else {
                return Err("disjoint capability has no reviewed physical carrier".to_owned());
            };
            if crate::trusted_device_items::classify(tcx, carrier.did())
                != Some(crate::trusted_device_items::TrustedDeviceItem::WriteOnlyDisjointSlice)
            {
                return Err("disjoint capability has the wrong physical carrier".to_owned());
            }
            let carrier_types = carrier_arguments.types().collect::<Vec<_>>();
            if carrier_arguments.len() != 2 || carrier_types.as_slice() != [*element, role_types[0]]
            {
                return Err(
                    "disjoint capability element or index-space type changed at the physical boundary"
                        .to_owned(),
                );
            }
        }
        _ => return Err("capability view role is not an exact supported reviewed role".to_owned()),
    }

    let layout_cx = LayoutCx::new(tcx, TypingEnv::fully_monomorphized());
    let logical_layout = layout_cx
        .layout_of(logical)
        .map_err(|error| format!("logical capability layout is unavailable: {error}"))?;
    let physical_layout = layout_cx
        .layout_of(physical)
        .map_err(|error| format!("physical capability carrier layout is unavailable: {error}"))?;
    if logical_layout.size != physical_layout.size
        || logical_layout.align.abi != physical_layout.align.abi
        || logical_layout.backend_repr != physical_layout.backend_repr
        || logical_layout.is_uninhabited() != physical_layout.is_uninhabited()
    {
        return Err(
            "reviewed capability view and physical carrier have different rustc ABI layouts"
                .to_owned(),
        );
    }
    Ok(())
}

fn authenticate_kernel_capability_brand_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    marker_ty: Ty<'tcx>,
    brand: Ty<'tcx>,
) -> Result<(), String> {
    let TyKind::Adt(definition, arguments) = brand.kind() else {
        return Err("capability brand is not the reviewed kernel brand".to_owned());
    };
    if !crate::trusted_device_items::is_exact_reviewed_provider_definition_v1(
        tcx,
        definition.did(),
        "fe2o3_device::context::KernelCapabilityBrand",
    ) {
        return Err("capability brand is not the reviewed kernel brand".to_owned());
    }
    if arguments.len() != 4 || !matches!(arguments[0].kind(), GenericArgKind::Lifetime(_)) {
        return Err(
            "reviewed kernel brand changed its lifetime/type/const generic argument shape"
                .to_owned(),
        );
    }
    let types = arguments.types().collect::<Vec<_>>();
    let [kernel, target, launch] = types.as_slice() else {
        return Err("reviewed kernel brand changed its generic type arity".to_owned());
    };
    if *kernel != marker_ty
        || !exact_reviewed_marker_v1(tcx, *target, "fe2o3_device::context::CurrentTarget")
        || !exact_reviewed_marker_v1(tcx, *launch, "fe2o3_device::context::RegisteredLaunch")
    {
        return Err(
            "capability brand does not bind this kernel marker, target, and launch".to_owned(),
        );
    }
    Ok(())
}

fn exact_reviewed_marker_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, expected_path: &str) -> bool {
    let TyKind::Adt(definition, arguments) = ty.kind() else {
        return false;
    };
    arguments.is_empty()
        && crate::trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            definition.did(),
            expected_path,
        )
}

fn authenticate_physical_slice_v1<'tcx>(
    element: Ty<'tcx>,
    physical: Ty<'tcx>,
    expected_mutability: Mutability,
) -> Result<(), String> {
    let TyKind::Ref(_, pointee, mutability) = physical.kind() else {
        return Err("capability view physical carrier is not a slice reference".to_owned());
    };
    let TyKind::Slice(physical_element) = pointee.kind() else {
        return Err("capability view physical carrier is not a slice reference".to_owned());
    };
    if *mutability != expected_mutability || *physical_element != element {
        return Err("capability view physical slice changed element type or mutability".to_owned());
    }
    Ok(())
}

fn assembly_operand_bit(ty: rustc_middle::ty::Ty<'_>) -> Option<u16> {
    match ty.kind() {
        TyKind::Bool | TyKind::Char | TyKind::Int(_) | TyKind::Uint(_) | TyKind::FnDef(..) => {
            Some(ASSEMBLY_OPERAND_SGPR_V1)
        }
        TyKind::Float(_) => Some(ASSEMBLY_OPERAND_VGPR_V1),
        TyKind::Ref(..) | TyKind::RawPtr(..) | TyKind::FnPtr(..) => {
            Some(ASSEMBLY_OPERAND_ADDRESS_V1)
        }
        _ => None,
    }
}

fn frontend_assembly_option_bits(options: InlineAsmOptions) -> u16 {
    let mut bits = 0;
    for (option, frontend_bit) in [
        (InlineAsmOptions::NOMEM, ASSEMBLY_OPTION_NOMEM_V1),
        (InlineAsmOptions::READONLY, ASSEMBLY_OPTION_READONLY_V1),
        (InlineAsmOptions::PURE, ASSEMBLY_OPTION_PURE_V1),
        (
            InlineAsmOptions::PRESERVES_FLAGS,
            ASSEMBLY_OPTION_PRESERVES_FLAGS_V1,
        ),
        (InlineAsmOptions::NOSTACK, ASSEMBLY_OPTION_NOSTACK_V1),
    ] {
        if options.contains(option) {
            bits |= frontend_bit;
        }
    }
    bits
}

fn reconcile_frontend_contract(
    logical_name: &str,
    expected_target: &str,
    authenticated: Option<&AuthenticatedKernelFrontendContractV1>,
    observed: ObservedInlineAssemblyV1,
) -> Result<ReachableAssemblySummaryV1, CollectError> {
    let declared = authenticated.and_then(|contract| contract.contract.unsafe_assembly());
    if observed.blocks == 0 {
        if declared.is_some() {
            return Err(CollectError {
                message: format!(
                    "kernel `{logical_name}` declares unsafe assembly but no asm! block is reachable from its exact rustc instance"
                ),
            });
        }
        return Ok(ReachableAssemblySummaryV1::default());
    }

    let Some(declared) = declared else {
        return Err(CollectError {
            message: format!(
                "kernel `{logical_name}` reaches inline assembly without an authenticated unsafe_asm frontend contract"
            ),
        });
    };
    let target_arch = expected_target.split(':').next().unwrap_or(expected_target);
    if declared.target().canonical_name() != target_arch {
        return Err(CollectError {
            message: format!(
                "kernel `{logical_name}` unsafe-assembly target `{}` disagrees with compiler target `{expected_target}`",
                declared.target().canonical_name()
            ),
        });
    }
    if observed.operand_bits != declared.operand_bits() {
        return Err(CollectError {
            message: format!(
                "kernel `{logical_name}` unsafe-assembly operand declaration {:#x} disagrees with reachable MIR operands {:#x}",
                declared.operand_bits(),
                observed.operand_bits
            ),
        });
    }
    let [observed_options] = observed.option_sets.iter().copied().collect::<Vec<_>>()[..] else {
        return Err(CollectError {
            message: format!(
                "kernel `{logical_name}` reaches asm! blocks with different option sets that one V1 contract cannot represent"
            ),
        });
    };
    if observed_options != declared.option_bits() {
        return Err(CollectError {
            message: format!(
                "kernel `{logical_name}` unsafe-assembly option declaration {:#x} disagrees with reachable MIR options {observed_options:#x}",
                declared.option_bits()
            ),
        });
    }

    Ok(ReachableAssemblySummaryV1 {
        blocks: observed.blocks,
        operand_bits: observed.operand_bits,
        option_bits: observed_options,
    })
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
    use super::{
        AuthenticatedKernelFrontendContractV1, CallChainLink, KernelRoot,
        LogicalKernelReturnRelationV1, ObservedInlineAssemblyV1, RegistrationError,
        RegistrationRecord, TypedArgumentListError, TypedArgumentListV1,
        authenticate_kernel_signature_components_v1, general_typed_launch_v3,
        logical_physical_kernel_abi_error_v1, reconcile_frontend_contract, reconstruct_call_chain,
        root_scoped_call_chains, validate_registration_records as validate_records,
    };
    use fe2o3_artifacts::{
        BlockSize, DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestBytes, Dimensions,
        TypeIdentity,
    };
    use fe2o3_kernel_descriptor::MAX_ARGUMENTS_PER_KERNEL;
    use fe2o3_rustc_front::{
        ASSEMBLY_OPERAND_SGPR_V1, ASSEMBLY_OPTION_NOMEM_V1, ASSEMBLY_OPTION_NOSTACK_V1,
        ASSEMBLY_OPTION_PRESERVES_FLAGS_V1, FrontendGridDimensionsV1, FrontendLaunchBoundsV1,
        FrontendUnsafeAssemblyDeclarationV1, FrontendUnsafeAssemblyTargetV1,
        FrontendWorkgroupDimensionsV1, KernelFrontendContractV1, KernelResourceContractV1,
    };
    use reserved_fe2o3_symbols::{
        GeneratedHostContractIdV3, KERNEL_PREFIX, KERNEL_REGISTRATION_KIND_KERNEL,
        KERNEL_REGISTRATION_KIND_TYPED_GENERAL_LAYOUT_V3, KERNEL_REGISTRATION_MAGIC,
        KERNEL_REGISTRATION_PREFIX, KERNEL_REGISTRATION_VERSION_V1, KERNEL_REGISTRATION_VERSION_V2,
        KERNEL_REGISTRATION_VERSION_V3, KernelBindingIdV1,
        TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3, derive_crate_binding_id_v1,
        derive_kernel_binding_id_v1, host_kernel_symbol_v1,
    };
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn predecessor_call_chains_reconstruct_without_per_node_prefix_copies() {
        let links = BTreeMap::from([
            (
                1_u8,
                CallChainLink {
                    predecessor: None,
                    label: "root".to_owned(),
                },
            ),
            (
                2,
                CallChainLink {
                    predecessor: Some(1),
                    label: "helper_a".to_owned(),
                },
            ),
            (
                3,
                CallChainLink {
                    predecessor: Some(2),
                    label: "helper_b".to_owned(),
                },
            ),
        ]);

        assert_eq!(
            reconstruct_call_chain(&links, &3),
            ["root", "helper_a", "helper_b"],
        );
        assert!(reconstruct_call_chain(&links, &9).is_empty());
    }

    #[test]
    fn source_safety_call_chains_are_reconstructed_per_root() {
        let edges = BTreeMap::from([
            (1_u8, BTreeSet::from([3_u8])),
            (2_u8, BTreeSet::from([3_u8])),
            (3_u8, BTreeSet::from([4_u8])),
        ]);
        let labels = BTreeMap::from([
            (1_u8, "ordinary_root".to_owned()),
            (2_u8, "low_level_root".to_owned()),
            (3_u8, "shared_helper".to_owned()),
            (4_u8, "unsafe_leaf".to_owned()),
        ]);

        let (ordinary, order) = root_scoped_call_chains(&edges, &labels, &1);
        assert_eq!(order, [1, 3, 4]);
        assert_eq!(
            reconstruct_call_chain(&ordinary, &4),
            ["ordinary_root", "shared_helper", "unsafe_leaf"],
        );

        let (low_level, order) = root_scoped_call_chains(&edges, &labels, &2);
        assert_eq!(order, [2, 3, 4]);
        assert_eq!(
            reconstruct_call_chain(&low_level, &4),
            ["low_level_root", "shared_helper", "unsafe_leaf"],
        );
    }

    fn type_identity(byte: u8) -> TypeIdentity {
        TypeIdentity::new(
            DeclaredRustTypeIdentity::from_untrusted_bytes(DigestBytes::from_bytes([byte; 32])),
            DeclaredRustLayoutIdentity::from_untrusted_bytes(DigestBytes::from_bytes(
                [byte.wrapping_add(1); 32],
            )),
        )
    }

    #[test]
    fn typed_argument_lists_are_owned_and_not_fixed_to_three_arguments() {
        let one = TypedArgumentListV1::new(vec![type_identity(1)]).unwrap();
        let two = TypedArgumentListV1::new(vec![type_identity(2), type_identity(3)]).unwrap();

        assert_eq!(one.len(), 1);
        assert_eq!(two.len(), 2);
        assert_ne!(one.as_slice(), two.as_slice());
    }

    #[test]
    fn typed_argument_lists_reject_empty_and_oversized_collections() {
        assert_eq!(
            TypedArgumentListV1::<TypeIdentity>::new(Vec::new()),
            Err(TypedArgumentListError::Empty)
        );
        assert!(matches!(
            TypedArgumentListV1::new(vec![type_identity(7); MAX_ARGUMENTS_PER_KERNEL + 1]),
            Err(TypedArgumentListError::TooMany { actual, maximum })
                if actual == MAX_ARGUMENTS_PER_KERNEL + 1
                    && maximum == MAX_ARGUMENTS_PER_KERNEL
        ));
    }

    fn validate_registration_records<T: Copy>(
        records: Vec<RegistrationRecord<T>>,
    ) -> Result<Vec<KernelRoot<T>>, RegistrationError> {
        let expected_crate_binding = records.iter().find_map(|record| record.crate_binding);
        validate_records(records, expected_crate_binding, Some("fixture"))
    }

    fn registration(
        path: &str,
        logical_name: &str,
        export_name: &str,
        target: u8,
    ) -> RegistrationRecord<u8> {
        let target_symbol = format!("{KERNEL_PREFIX}{export_name}");
        let module_path = path.rsplit_once("::").map_or("", |(module, _)| module);
        let target_identity = if module_path.is_empty() {
            target_symbol.clone()
        } else {
            format!("{module_path}::{target_symbol}")
        };
        RegistrationRecord {
            registration_path: path.to_string(),
            item_name: format!("{KERNEL_REGISTRATION_PREFIX}{logical_name}"),
            magic: KERNEL_REGISTRATION_MAGIC,
            version: KERNEL_REGISTRATION_VERSION_V1,
            kind: KERNEL_REGISTRATION_KIND_KERNEL,
            logical_name: logical_name.to_string(),
            export_name: export_name.to_string(),
            crate_binding: None,
            kernel_binding: None,
            profile_tag: None,
            generated_host_contract_identity: None,
            target_crate_name: "fixture".to_owned(),
            target_symbol,
            target_identity,
            target,
        }
    }

    fn general_typed_registration(
        path: &str,
        logical_name: &str,
        export_name: &str,
        target: u8,
    ) -> RegistrationRecord<u8> {
        let mut registration = registration(path, logical_name, export_name, target);
        let crate_binding = derive_crate_binding_id_v1("fixture", ["metadata"]);
        let kernel_binding = derive_kernel_binding_id_v1(
            crate_binding,
            TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
            logical_name,
            export_name,
        );
        registration.version = KERNEL_REGISTRATION_VERSION_V3;
        registration.kind = KERNEL_REGISTRATION_KIND_TYPED_GENERAL_LAYOUT_V3;
        registration.crate_binding = Some(crate_binding);
        registration.kernel_binding = Some(kernel_binding);
        registration.profile_tag = Some(TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3.to_owned());
        registration.generated_host_contract_identity =
            Some(GeneratedHostContractIdV3::from_bytes([0x63; 32]));
        registration.target_symbol = host_kernel_symbol_v1(kernel_binding);
        registration
    }

    #[test]
    fn genuine_v1_registration_becomes_a_kernel_root() {
        let roots = validate_registration_records(vec![registration(
            "crate::__fe2o3_kernel_registration_plain",
            "plain",
            "plain",
            7,
        )])
        .unwrap();

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].target, 7);
        assert_eq!(roots[0].logical_name, "plain");
        assert_eq!(roots[0].export_name, "plain");
        assert_eq!(roots[0].generated_host_contract_identity, None);
    }

    #[test]
    fn general_v3_registration_carries_the_complete_profile_identity() {
        let registration = general_typed_registration(
            "crate::__fe2o3_kernel_registration_alpha",
            "alpha",
            "alpha",
            3,
        );
        let roots = validate_registration_records(vec![registration]).unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].target, 3);
        assert_eq!(
            roots[0]
                .generated_host_contract_identity
                .unwrap()
                .as_bytes(),
            [0x63; 32],
        );
    }

    #[test]
    fn authenticated_kernel_owner_rejects_owner_profile_and_build_mutations() {
        let registration = general_typed_registration(
            "general_genuine::__fe2o3_kernel_registration_alpha",
            "alpha",
            "alpha",
            3,
        );
        let expected_crate_binding = registration.crate_binding.unwrap();

        let error =
            validate_records(vec![registration.clone()], None, Some("fixture")).unwrap_err();
        assert!(error.reason.contains("no rustc session crate binding"));

        let error = validate_records(
            vec![registration.clone()],
            Some(expected_crate_binding),
            None,
        )
        .unwrap_err();
        assert!(error.reason.contains("no rustc session crate identity"));

        let mut wrong_crate = registration.clone();
        wrong_crate.target_crate_name = "other".to_owned();
        let error = validate_records(
            vec![wrong_crate],
            Some(expected_crate_binding),
            Some("fixture"),
        )
        .unwrap_err();
        assert!(error.reason.contains("cross-crate generic typed kernels"));

        let mut wrong_module = registration.clone();
        wrong_module.target_identity = format!("spoof::{}", wrong_module.target_symbol);
        let error = validate_registration_records(vec![wrong_module]).unwrap_err();
        assert!(
            error
                .reason
                .contains("registered target module \"spoof\" disagrees")
        );

        let mut wrong_logical = registration.clone();
        wrong_logical.logical_name = "beta".to_owned();
        let error = validate_registration_records(vec![wrong_logical]).unwrap_err();
        assert!(
            error
                .reason
                .contains("inconsistent with logical name `beta`")
        );

        let mut wrong_export = registration.clone();
        wrong_export.export_name = "beta".to_owned();
        let error = validate_registration_records(vec![wrong_export]).unwrap_err();
        assert!(error.reason.contains("disagrees with derived binding"));

        let mut wrong_profile = registration.clone();
        wrong_profile.profile_tag = Some("wrong-profile".to_owned());
        let error = validate_registration_records(vec![wrong_profile]).unwrap_err();
        assert!(
            error
                .reason
                .contains("not the canonical general typed profile")
        );

        let mut wrong_crate_binding = registration.clone();
        wrong_crate_binding.crate_binding = Some(derive_crate_binding_id_v1("other", ["metadata"]));
        let error = validate_records(
            vec![wrong_crate_binding],
            Some(expected_crate_binding),
            Some("fixture"),
        )
        .unwrap_err();
        assert!(
            error
                .reason
                .contains("disagrees with rustc session binding")
        );

        let mut wrong_kernel_binding = registration.clone();
        wrong_kernel_binding.kernel_binding = Some(KernelBindingIdV1::from_bytes([0x99; 32]));
        let error = validate_registration_records(vec![wrong_kernel_binding]).unwrap_err();
        assert!(error.reason.contains("disagrees with derived binding"));

        let mut wrong_symbol = registration.clone();
        wrong_symbol.target_symbol = "spoofed-symbol".to_owned();
        let error = validate_registration_records(vec![wrong_symbol]).unwrap_err();
        assert!(error.reason.contains("target symbol `spoofed-symbol`"));

        let mut duplicate = general_typed_registration(
            "general_genuine::__fe2o3_kernel_registration_beta",
            "beta",
            "beta",
            4,
        );
        duplicate.target_identity = registration.target_identity.clone();
        let error = validate_registration_records(vec![registration, duplicate]).unwrap_err();
        assert!(error.reason.contains("duplicate target identity"));
    }

    #[test]
    fn general_v3_rejects_profile_binding_identity_and_version_mutations() {
        let registration = general_typed_registration(
            "crate::__fe2o3_kernel_registration_alpha",
            "alpha",
            "alpha",
            3,
        );

        let mut wrong_profile = registration.clone();
        wrong_profile.profile_tag = Some("fe2o3.manifest-derived-scalar-slice.v2".to_owned());
        assert!(
            validate_registration_records(vec![wrong_profile])
                .unwrap_err()
                .reason
                .contains("not the canonical general typed profile")
        );

        let mut wrong_binding = registration.clone();
        wrong_binding.kernel_binding = Some(KernelBindingIdV1::from_bytes([0x99; 32]));
        assert!(
            validate_registration_records(vec![wrong_binding])
                .unwrap_err()
                .reason
                .contains("disagrees with derived binding")
        );

        let mut old_version = registration.clone();
        old_version.version = KERNEL_REGISTRATION_VERSION_V2;
        assert!(
            validate_registration_records(vec![old_version])
                .unwrap_err()
                .reason
                .contains("registration version 2 has been removed")
        );

        let mut old_kind = registration;
        old_kind.kind = KERNEL_REGISTRATION_KIND_TYPED_GENERAL_LAYOUT_V3 + 1;
        assert!(
            validate_registration_records(vec![old_kind])
                .unwrap_err()
                .reason
                .contains("version 3 is reserved")
        );
    }

    #[test]
    fn general_v3_roots_are_ordered_independently_of_registration_discovery() {
        let zeta = general_typed_registration(
            "crate::__fe2o3_kernel_registration_zeta",
            "zeta",
            "zeta",
            2,
        );
        let alpha = general_typed_registration(
            "crate::__fe2o3_kernel_registration_alpha",
            "alpha",
            "alpha",
            1,
        );
        let roots = validate_registration_records(vec![zeta, alpha]).unwrap();
        assert_eq!(
            roots
                .iter()
                .map(|root| root.logical_name.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "zeta"]
        );
    }

    #[test]
    fn generic_typed_registration_identity_and_host_symbol_fail_closed() {
        let typed = general_typed_registration(
            "crate::__fe2o3_kernel_registration_typed",
            "typed",
            "typed",
            7,
        );
        let wrong_crate = derive_crate_binding_id_v1("other", ["metadata"]);
        let error =
            validate_records(vec![typed.clone()], Some(wrong_crate), Some("fixture")).unwrap_err();
        assert!(
            error
                .reason
                .contains("disagrees with rustc session binding")
        );

        let mut wrong_kernel = typed.clone();
        wrong_kernel.kernel_binding = Some(derive_kernel_binding_id_v1(
            wrong_kernel.crate_binding.unwrap(),
            TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
            "different",
            "different",
        ));
        let error = validate_registration_records(vec![wrong_kernel]).unwrap_err();
        assert!(error.reason.contains("disagrees with derived binding"));

        let mut logical_host_symbol = typed;
        logical_host_symbol.target_symbol = format!("{KERNEL_PREFIX}typed");
        let error = validate_registration_records(vec![logical_host_symbol]).unwrap_err();
        assert!(error.reason.contains("target symbol"));
        assert!(error.reason.contains("inconsistent with export name"));
    }

    #[test]
    fn kernel_prefix_spoof_without_registration_is_not_a_root() {
        let roots = validate_registration_records::<u8>(Vec::new()).unwrap();
        assert!(roots.is_empty());
    }

    #[test]
    fn general_typed_launch_accepts_required_only_exact_profiles_and_rejects_ambiguity() {
        let authenticated =
            |required: Option<[u32; 3]>, maximum: Option<[u32; 3]>, occupancy: Option<u16>| {
                let launch = FrontendLaunchBoundsV1::new(
                    required.map(|value| FrontendWorkgroupDimensionsV1::new(value).unwrap()),
                    maximum.map(|value| FrontendWorkgroupDimensionsV1::new(value).unwrap()),
                    occupancy,
                )
                .unwrap();
                AuthenticatedKernelFrontendContractV1::for_test(
                    KernelFrontendContractV1::new(Some(launch), None).unwrap(),
                )
            };

        let default = general_typed_launch_v3(None, "registration").unwrap();
        assert_eq!(
            default.block_size(),
            BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap())
        );
        for dimensions in [
            [1, 1, 1],
            [3, 1, 1],
            [64, 1, 1],
            [65, 1, 1],
            [255, 1, 1],
            [256, 1, 1],
        ] {
            let frontend = authenticated(Some(dimensions), None, None);
            let launch = general_typed_launch_v3(Some(&frontend), "registration").unwrap();
            assert_eq!(
                launch.block_size(),
                BlockSize::Exact(
                    Dimensions::new(dimensions[0], dimensions[1], dimensions[2]).unwrap()
                )
            );

            let frontend = authenticated(Some(dimensions), Some(dimensions), None);
            let launch = general_typed_launch_v3(Some(&frontend), "registration").unwrap();
            assert_eq!(
                launch.block_size(),
                BlockSize::Exact(
                    Dimensions::new(dimensions[0], dimensions[1], dimensions[2]).unwrap()
                )
            );
        }

        let finite_launch = FrontendLaunchBoundsV1::new_with_max_grid(
            Some(FrontendWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
            Some(FrontendWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
            None,
            Some(FrontendGridDimensionsV1::new([1, 1, 1]).unwrap()),
        )
        .unwrap();
        let finite_frontend = AuthenticatedKernelFrontendContractV1::for_test(
            KernelFrontendContractV1::new(Some(finite_launch), None).unwrap(),
        );
        assert_eq!(
            general_typed_launch_v3(Some(&finite_frontend), "registration")
                .unwrap()
                .max_grid(),
            Dimensions::new(1, 1, 1).unwrap()
        );

        let resource_frontend = AuthenticatedKernelFrontendContractV1::for_test_with_resource(
            KernelFrontendContractV1::new(Some(finite_launch), None).unwrap(),
            KernelResourceContractV1::new(256, 1_024).unwrap(),
        );
        let resource_launch =
            general_typed_launch_v3(Some(&resource_frontend), "registration").unwrap();
        assert_eq!(resource_launch.static_shared_memory_bytes(), 256);
        assert_eq!(resource_launch.max_dynamic_shared_memory_bytes(), 1_024);
        assert_ne!(resource_frontend.resource_canonical_bytes(), Some(&[][..]),);

        for frontend in [
            authenticated(None, Some([64, 1, 1]), None),
            authenticated(Some([64, 1, 1]), Some([256, 1, 1]), None),
            authenticated(Some([256, 1, 1]), Some([256, 1, 1]), Some(2)),
            authenticated(Some([32, 2, 1]), None, None),
            authenticated(Some([257, 1, 1]), None, None),
        ] {
            assert!(general_typed_launch_v3(Some(&frontend), "registration").is_err());
        }
    }

    #[test]
    fn reachable_assembly_reconciliation_is_exact_and_target_bound() {
        let option_bits = ASSEMBLY_OPTION_NOMEM_V1
            | ASSEMBLY_OPTION_NOSTACK_V1
            | ASSEMBLY_OPTION_PRESERVES_FLAGS_V1;
        let contract = KernelFrontendContractV1::new(
            None,
            Some(
                FrontendUnsafeAssemblyDeclarationV1::new(
                    FrontendUnsafeAssemblyTargetV1::AmdGpuGfx942,
                    ASSEMBLY_OPERAND_SGPR_V1,
                    option_bits,
                    0,
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let authenticated = AuthenticatedKernelFrontendContractV1::for_test(contract);
        let observed = ObservedInlineAssemblyV1 {
            blocks: 2,
            operand_bits: ASSEMBLY_OPERAND_SGPR_V1,
            option_sets: BTreeSet::from([option_bits]),
        };

        let summary = reconcile_frontend_contract(
            "kernel",
            "gfx942:xnack-",
            Some(&authenticated),
            observed.clone(),
        )
        .unwrap();
        assert_eq!(summary.blocks, 2);
        assert_eq!(summary.operand_bits, ASSEMBLY_OPERAND_SGPR_V1);
        assert_eq!(summary.option_bits, option_bits);

        for (target, contract, observation, expected) in [
            (
                "gfx1100",
                Some(&authenticated),
                observed.clone(),
                "disagrees with compiler target",
            ),
            (
                "gfx942",
                None,
                observed.clone(),
                "without an authenticated unsafe_asm",
            ),
            (
                "gfx942",
                Some(&authenticated),
                ObservedInlineAssemblyV1 {
                    operand_bits: fe2o3_rustc_front::ASSEMBLY_OPERAND_VGPR_V1,
                    ..observed.clone()
                },
                "operand declaration",
            ),
            (
                "gfx942",
                Some(&authenticated),
                ObservedInlineAssemblyV1 {
                    option_sets: BTreeSet::from([option_bits, ASSEMBLY_OPTION_NOMEM_V1]),
                    ..observed
                },
                "different option sets",
            ),
        ] {
            let error =
                reconcile_frontend_contract("kernel", target, contract, observation).unwrap_err();
            assert!(
                error.to_string().contains(expected),
                "missing `{expected}` in {error}"
            );
        }
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
                .contains("does not match registration magic")
        );

        let mut unknown_version = base.clone();
        unknown_version.version = KERNEL_REGISTRATION_VERSION_V3 + 1;
        assert!(
            validate_registration_records(vec![unknown_version])
                .unwrap_err()
                .reason
                .contains("unknown registration version")
        );

        for kind in [
            0,
            KERNEL_REGISTRATION_KIND_TYPED_GENERAL_LAYOUT_V3 + 1,
            u16::MAX,
        ] {
            let mut unknown_kind = base.clone();
            unknown_kind.kind = kind;
            assert_eq!(
                validate_registration_records(vec![unknown_kind])
                    .unwrap_err()
                    .reason,
                format!("unknown registration kind {kind}")
            );
        }

        let mut obsolete_typed = base;
        obsolete_typed.version = KERNEL_REGISTRATION_VERSION_V2;
        obsolete_typed.kind = KERNEL_REGISTRATION_KIND_KERNEL;
        assert!(
            validate_registration_records(vec![obsolete_typed])
                .unwrap_err()
                .reason
                .contains("registration version 2 has been removed")
        );
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

        let typed_duplicate = general_typed_registration(
            "crate::b::__fe2o3_kernel_registration_same",
            "same",
            "typed",
            2,
        );
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
        alpha.target_identity = "crate::same-target".to_string();
        beta.target_identity = "crate::same-target".to_string();

        let error = validate_registration_records(vec![alpha, beta]).unwrap_err();
        assert!(
            error
                .reason
                .contains("duplicate target identity `crate::same-target`")
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

    #[test]
    fn production_registration_surface_accepts_v1_and_generic_v3() {
        let ordinary = validate_registration_records(vec![registration(
            "fixture::__fe2o3_kernel_plain",
            "plain",
            "plain",
            1,
        )])
        .unwrap();
        let general = validate_registration_records(vec![general_typed_registration(
            "fixture::__fe2o3_kernel_general",
            "general",
            "general",
            3,
        )])
        .unwrap();

        assert!(ordinary[0].generated_host_contract_identity.is_none());
        assert!(general[0].generated_host_contract_identity.is_some());
    }

    #[test]
    fn production_entry_receives_target_without_reading_process_state() {
        let source = include_str!("collector.rs");
        let production = source
            .split_once("pub(crate) fn collect_authenticated_kernel_closure_v1")
            .unwrap()
            .1
            .split_once("fn collect_device_functions")
            .unwrap()
            .0;
        for forbidden in ["std::env", "QUALIFICATION_ORACLE_ENV", "FE2O3_TARGET"] {
            assert!(
                !production.contains(forbidden),
                "production collection entry contains hidden input {forbidden:?}"
            );
        }
        assert!(production.contains("RetainedProductionTargetV1"));
        assert!(!production.contains("authenticate_before_collection"));
    }

    #[test]
    fn logical_context_signature_normalization_preserves_exact_qualifiers() {
        assert_eq!(
            authenticate_kernel_signature_components_v1(
                Ok(()),
                LogicalKernelReturnRelationV1::Exact,
                "Rust",
                "Rust",
                "safe",
                "safe",
                false,
                false,
            ),
            Ok(LogicalKernelReturnRelationV1::Exact),
        );
        assert_eq!(
            authenticate_kernel_signature_components_v1(
                Ok(()),
                LogicalKernelReturnRelationV1::TrustedKernelResultToUnit,
                "Rust",
                "Rust",
                "safe",
                "safe",
                false,
                false,
            ),
            Ok(LogicalKernelReturnRelationV1::TrustedKernelResultToUnit),
        );
    }

    #[test]
    fn logical_context_signature_normalization_reports_stable_abi_failures() {
        let check =
            |arguments, result, logical_abi, physical_abi, logical_safety, physical_safety| {
                let detail = authenticate_kernel_signature_components_v1(
                    arguments,
                    result,
                    logical_abi,
                    physical_abi,
                    logical_safety,
                    physical_safety,
                    false,
                    false,
                )
                .unwrap_err();
                logical_physical_kernel_abi_error_v1(
                    "fixture::__fe2o3_kernel_body_v1_copy",
                    "fixture::__fe2o3_host_kernel_v1_copy",
                    0,
                    &detail,
                )
                .message
            };

        assert_eq!(
            check(
                Err("physical/logical argument 2 changed a const generic".to_owned()),
                LogicalKernelReturnRelationV1::Exact,
                "Rust",
                "Rust",
                "safe",
                "safe",
            ),
            "[FE2O3-CAP-ABI001] logical helper `fixture::__fe2o3_kernel_body_v1_copy` does not equal physical root `fixture::__fe2o3_host_kernel_v1_copy` after removing only context ordinal 0: physical/logical argument 2 changed a const generic",
        );
        for (message, suffix) in [
            (
                check(
                    Ok(()),
                    LogicalKernelReturnRelationV1::Exact,
                    "C",
                    "Rust",
                    "safe",
                    "safe",
                ),
                "physical/logical calling ABIs differ",
            ),
            (
                check(
                    Ok(()),
                    LogicalKernelReturnRelationV1::Exact,
                    "Rust",
                    "Rust",
                    "unsafe",
                    "safe",
                ),
                "physical/logical safety qualifiers differ",
            ),
            (
                check(
                    Ok(()),
                    LogicalKernelReturnRelationV1::Mismatch,
                    "Rust",
                    "Rust",
                    "safe",
                    "safe",
                ),
                "physical/logical return types differ outside the exact trusted KernelResult-to-unit entry normalization",
            ),
        ] {
            assert!(message.starts_with("[FE2O3-CAP-ABI001]"), "{message}");
            assert!(message.ends_with(suffix), "{message}");
        }
    }
}
