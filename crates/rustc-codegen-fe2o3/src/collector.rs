use fe2o3_artifacts::{
    BlockSize, Dimensions, LaunchContract, TypeIdentity, derive_generated_host_contract_identity_v1,
};
use fe2o3_kernel_descriptor::MAX_ARGUMENTS_PER_KERNEL;
use fe2o3_rustc_front::{
    ASSEMBLY_OPERAND_ADDRESS_V1, ASSEMBLY_OPERAND_IMMEDIATE_V1, ASSEMBLY_OPERAND_SGPR_V1,
    ASSEMBLY_OPERAND_VGPR_V1, ASSEMBLY_OPTION_NOMEM_V1, ASSEMBLY_OPTION_NOSTACK_V1,
    ASSEMBLY_OPTION_PRESERVES_FLAGS_V1, ASSEMBLY_OPTION_PURE_V1, ASSEMBLY_OPTION_READONLY_V1,
    KERNEL_FRONTEND_REGISTRATION_KIND_V1, KERNEL_FRONTEND_REGISTRATION_MAGIC_V1,
    KERNEL_FRONTEND_REGISTRATION_PREFIX_V1, KERNEL_FRONTEND_REGISTRATION_VERSION_V1,
    KernelFrontendContractV1, decode_kernel_frontend_contract_v1,
};
use reserved_fe2o3_symbols::{
    CrateBindingIdV1, GeneratedHostContractIdV3, KernelBindingIdV1,
    MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1, TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
    TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2, derive_crate_binding_id_v1,
    derive_kernel_binding_id_v1, host_kernel_symbol_v1,
};
use rustc_ast::InlineAsmOptions;
use rustc_hir::def_id::{DefId, DefIndex, LOCAL_CRATE};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BlockCheckMode, ExprKind, ItemKind, Safety, UnsafeSource};
use rustc_middle::middle::codegen_fn_attrs::CodegenFnAttrFlags;
use rustc_middle::mir::interpret::GlobalAlloc;
use rustc_middle::mir::mono::{CodegenUnit, MonoItem};
use rustc_middle::mir::{
    AggregateKind, Body, CastKind, InlineAsmMacro, InlineAsmOperand, Operand, RETURN_PLACE, Rvalue,
    TerminatorKind, UnwindAction,
};
use rustc_middle::ty::adjustment::PointerCoercion;
use rustc_middle::ty::{
    EarlyBinder, Instance, InstanceKind, TyCtxt, TyKind, TypeVisitableExt, TypingEnv,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::production_rustc_drop_v1::{ProductionRustcDropClassV1, classify_rustc_drop_v1};

mod production_importer_v1;

pub(crate) use production_importer_v1::{
    AuthenticatedRustcIdentityInventoryV3, AuthenticatedRustcPreflightPlanV3,
    ProductionSemanticImportErrorV1, construct_production_semantic_mir_v1,
    rust_disjoint_tile_2d_v1, rust_index_witness_space_v1, rust_option_payload_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypedKernelProfile {
    VecAddRustcLayoutV2,
    GeneralScalarSliceRustcLayoutV3 {
        generated_host_contract_identity: GeneratedHostContractIdV3,
    },
}

impl TypedKernelProfile {
    pub(crate) const fn expected_argument_count(self) -> Option<usize> {
        match self {
            Self::VecAddRustcLayoutV2 => Some(3),
            Self::GeneralScalarSliceRustcLayoutV3 { .. } => None,
        }
    }

    pub(crate) const fn accepts_argument_count(self, actual: usize) -> bool {
        match self.expected_argument_count() {
            Some(expected) => actual == expected,
            None => actual > 0 && actual <= MAX_ARGUMENTS_PER_KERNEL,
        }
    }
}

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
    /// Present only when the registration selects a versioned typed profile.
    pub(crate) typed_profile: Option<TypedKernelProfile>,
    /// Present only for a V2 typed registration validated by the backend.
    pub(crate) kernel_binding: Option<KernelBindingIdV1>,
    /// rustc-derived identities for each source argument in a typed profile.
    pub(crate) typed_layout_identities: Option<TypedArgumentListV1<TypeIdentity>>,
    /// Complete rustc-derived contract for a general V3 typed root.
    pub(crate) general_typed_contract:
        Option<crate::rust_type_layout_v3::GeneralTypedKernelContractV3>,
    /// Compiler-authenticated source contract for this exact kernel root.
    pub(crate) frontend_contract: Option<AuthenticatedKernelFrontendContractV1>,
    /// Exact safe-Rust reference/effect binding for this kernel root.
    pub(crate) reference_effect_binding:
        Option<crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1>,
    /// Compiler-private observation derived from this exact monomorphized MIR.
    pub(crate) dead_branches: Option<crate::monomorphization_dead::CompilerDeadBranchObservationV1>,
}

/// Collector-sealed identity of one exact typed kernel root.
///
/// Private fields and the absence of a public constructor ensure downstream
/// code can only receive this value after registration, session binding,
/// function-pointer, symbol, and unique-root validation has completed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedKernelOwner<T> {
    target: T,
    crate_name: String,
    module_path: String,
    logical_name: String,
    export_name: String,
    typed_profile: TypedKernelProfile,
    target_def_path: String,
    crate_binding: CrateBindingIdV1,
    kernel_binding: KernelBindingIdV1,
    observed_symbol: String,
}

#[derive(Clone, Debug)]
pub(crate) struct AuthenticatedKernelOwners<T> {
    owners: Vec<AuthenticatedKernelOwner<T>>,
}

impl<T> Default for AuthenticatedKernelOwners<T> {
    fn default() -> Self {
        Self { owners: Vec::new() }
    }
}

impl<T> AuthenticatedKernelOwners<T> {
    fn push(&mut self, owner: AuthenticatedKernelOwner<T>) {
        self.owners.push(owner);
    }

    fn as_slice(&self) -> &[AuthenticatedKernelOwner<T>] {
        &self.owners
    }
}

impl<T: Copy> AuthenticatedKernelOwner<T> {
    pub(crate) const fn target(&self) -> T {
        self.target
    }

    pub(crate) fn crate_name(&self) -> &str {
        &self.crate_name
    }

    pub(crate) fn module_path(&self) -> &str {
        &self.module_path
    }

    pub(crate) fn logical_name(&self) -> &str {
        &self.logical_name
    }

    pub(crate) fn export_name(&self) -> &str {
        &self.export_name
    }

    pub(crate) const fn typed_profile(&self) -> TypedKernelProfile {
        self.typed_profile
    }

    pub(crate) fn target_def_path(&self) -> &str {
        &self.target_def_path
    }

    pub(crate) const fn crate_binding(&self) -> CrateBindingIdV1 {
        self.crate_binding
    }

    pub(crate) const fn kernel_binding(&self) -> KernelBindingIdV1 {
        self.kernel_binding
    }

    pub(crate) fn observed_symbol(&self) -> &str {
        &self.observed_symbol
    }
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
    reachable_assembly: ReachableAssemblySummaryV1,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ReachableAssemblySummaryV1 {
    blocks: u32,
    operand_bits: u16,
    option_bits: u16,
}

impl AuthenticatedKernelFrontendContractV1 {
    pub(crate) fn registration_path(&self) -> &str {
        &self.registration_path
    }

    pub(crate) const fn target_def_path_hash(&self) -> [u8; 16] {
        self.target_def_path_hash
    }

    pub(crate) fn target_symbol(&self) -> &str {
        &self.target_symbol
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub(crate) const fn contract(&self) -> KernelFrontendContractV1 {
        self.contract
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
            reachable_assembly: ReachableAssemblySummaryV1::default(),
        }
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
    pub(crate) authenticated_kernel_owners: AuthenticatedKernelOwners<Instance<'tcx>>,
    // Private source state retained for compiler-envelope construction.
    pub(crate) device_ffi: crate::device_ffi::DeviceFfiClosure,
    /// Inert canonical observation produced from the successfully closed graph.
    pub(crate) compiler_ffi_observation: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
}

impl<'tcx> CollectionResult<'tcx> {
    pub(crate) fn authenticated_kernel_owners(
        &self,
    ) -> &[AuthenticatedKernelOwner<Instance<'tcx>>] {
        self.authenticated_kernel_owners.as_slice()
    }
}

/// Consumer boundary for the shared rustc graph collector.
///
/// Qualification retains its historical dead-branch projection, three
/// oracle-only terminal classifiers, and eager layout evidence because the
/// explicitly selected oracle routes still consume those records. Production
/// never receives that variant and instead traverses every block through the
/// workload-neutral semantic-terminal registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollectorPurposeV1 {
    QualificationOracle { extended_helper_edges: bool },
    Production,
}

impl CollectorPurposeV1 {
    const fn inspect_block(self, qualification_includes_block: bool) -> bool {
        match self {
            Self::QualificationOracle { .. } => qualification_includes_block,
            Self::Production => true,
        }
    }

    const fn accepts_extended_edges(self) -> bool {
        match self {
            Self::QualificationOracle {
                extended_helper_edges,
            } => extended_helper_edges,
            Self::Production => true,
        }
    }

    const fn is_qualification_oracle(self) -> bool {
        matches!(self, Self::QualificationOracle { .. })
    }

    const fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }
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

/// Collects compatibility evidence for an explicitly selected qualification
/// oracle. The caller must supply both the resolved target and its explicit
/// route token; process environment cannot silently alter collection.
pub(crate) fn collect_qualification_device_functions<'tcx>(
    tcx: TyCtxt<'tcx>,
    cgus: &[CodegenUnit<'tcx>],
    verbose: bool,
    target: &crate::AmdGpuTarget,
    pipeline: crate::qualification_selection::SelectedQualificationOracle,
) -> Result<CollectionResult<'tcx>, CollectError> {
    let extended_helper_edges = pipeline.requires_extended_collector_edges();
    collect_device_functions_for_purpose(
        tcx,
        cgus,
        verbose,
        target.as_str().to_owned(),
        CollectorPurposeV1::QualificationOracle {
            extended_helper_edges,
        },
    )
}

pub(crate) fn collect_authenticated_kernel_closure_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    cgus: &[CodegenUnit<'tcx>],
    verbose: bool,
    target: crate::production_target_v1::RetainedProductionTargetV1,
) -> Result<AuthenticatedCollectedKernelClosureV1<'tcx>, CollectError> {
    let collection = collect_device_functions_for_purpose(
        tcx,
        cgus,
        verbose,
        target.canonical_name().to_owned(),
        CollectorPurposeV1::Production,
    )?;
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
            message: "production-v1 collected no authenticated external device root".to_owned(),
        });
    }
    Ok(AuthenticatedCollectedKernelClosureV1 {
        target,
        collection,
        roots: roots.into_boxed_slice(),
    })
}

fn collect_device_functions_for_purpose<'tcx>(
    tcx: TyCtxt<'tcx>,
    cgus: &[CodegenUnit<'tcx>],
    verbose: bool,
    target: String,
    purpose: CollectorPurposeV1,
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
    let gfx942_xnack_minus = target == "gfx942:xnack-";
    let retain_qualification_layout_evidence = purpose.is_qualification_oracle();
    let mut collector = DeviceCollector::new(tcx, verbose, ffi_declarations, target, purpose);

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

    for root in kernel_roots(
        tcx,
        cgus,
        gfx942_xnack_minus,
        retain_qualification_layout_evidence,
    )
    .map_err(CollectError::from)?
    {
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

pub fn dump_device_functions<'tcx>(tcx: TyCtxt<'tcx>, functions: &[CollectedFunction<'tcx>]) {
    let mut rows = functions
        .iter()
        .map(|function| {
            let def_id = function.instance.def_id();
            debug_assert_eq!(function.is_kernel_entry(), function.logical_name.is_some());
            debug_assert!(function.is_kernel_entry() || function.typed_profile.is_none());
            debug_assert!(function.is_kernel_entry() || function.kernel_binding.is_none());
            debug_assert!(function.is_kernel_entry() || function.typed_layout_identities.is_none());
            debug_assert!(function.is_kernel_entry() || function.general_typed_contract.is_none());
            debug_assert!(function.is_kernel_entry() || function.frontend_contract.is_none());
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
                    Some(TypedKernelProfile::VecAddRustcLayoutV2) => {
                        "kernel/typed-vecadd-rustc-layout-v2"
                    }
                    Some(TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. }) => {
                        "kernel/typed-general-rustc-layout-v3"
                    }
                    None => match function.role {
                        CollectedFunctionRole::KernelEntry => "kernel",
                        CollectedFunctionRole::InternalHelper => "internal-helper",
                        CollectedFunctionRole::DeviceFfiExport => "device-ffi-export",
                    },
                },
                function.logical_name.clone(),
                tcx.crate_name(def_id.krate).to_string(),
                tcx.def_path_str(def_id),
                tcx.symbol_name(function.instance).name.to_string(),
                mir_stats,
            )
        })
        .collect::<Vec<_>>();

    rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.3.cmp(&b.3)));

    eprintln!("\n=== fe2o3 device function collection ===");
    for (export_name, kind, logical_name, crate_name, path, identity, mir_stats) in rows {
        eprintln!("  [{kind}] {export_name}");
        if let Some(logical_name) = logical_name.filter(|name| name != &export_name) {
            eprintln!("      logical name: {logical_name}");
        }
        eprintln!("      crate: {crate_name}");
        eprintln!("      path: {path}");
        eprintln!("      instance: {identity}");
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
    typed_profile: Option<TypedKernelProfile>,
    kernel_binding: Option<KernelBindingIdV1>,
    authenticated_owner: Option<AuthenticatedKernelOwner<T>>,
    typed_layout_identities: Option<TypedArgumentListV1<TypeIdentity>>,
    general_typed_contract: Option<crate::rust_type_layout_v3::GeneralTypedKernelContractV3>,
    frontend_contract: Option<AuthenticatedKernelFrontendContractV1>,
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
    gfx942_xnack_minus: bool,
    retain_qualification_layout_evidence: bool,
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
    for record in &records {
        if record.target.def_id().krate != LOCAL_CRATE {
            authenticate_external_typed_v2_registration(tcx, record)?;
        }
    }
    let mut roots = validate_registration_records(
        records,
        session_crate_binding(tcx),
        Some(&session_crate_name),
        gfx942_xnack_minus,
    )?;
    let frontend_records = decode_frontend_contract_registrations(tcx, &functions_by_symbol)?;
    bind_frontend_contract_registrations(tcx, &mut roots, frontend_records)?;
    let reference_records = decode_reference_binding_registrations(tcx)?;
    bind_reference_binding_registrations(tcx, &mut roots, reference_records)?;
    if !retain_qualification_layout_evidence {
        for root in &mut roots {
            root.typed_layout_identities = None;
            root.general_typed_contract = None;
        }
        return Ok(roots);
    }
    for root in &mut roots {
        let registration_path = format!(
            "{}{}",
            reserved_fe2o3_symbols::KERNEL_REGISTRATION_PREFIX,
            root.logical_name
        );
        match root.typed_profile {
            Some(TypedKernelProfile::VecAddRustcLayoutV2) => {
                let evidence =
                    crate::rust_type_layout::extract_exact_typed_vecadd_layout(tcx, root.target)
                        .map_err(|error| {
                            RegistrationError::new(
                                &registration_path,
                                format!("rustc type/layout evidence extraction failed: {error}"),
                            )
                        })?;
                let identities = evidence
                    .into_iter()
                    .map(|argument| argument.type_identity())
                    .collect();
                root.typed_layout_identities =
                    Some(TypedArgumentListV1::new(identities).map_err(|error| {
                        RegistrationError::new(&registration_path, error.to_string())
                    })?);
                root.general_typed_contract = None;
            }
            Some(TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
                generated_host_contract_identity,
            }) => {
                if crate::collected_workgroup_sync_v1::quarantine_scoped_atomic_general_contract(
                    &root.logical_name,
                    &root.export_name,
                ) {
                    root.typed_layout_identities = None;
                    root.general_typed_contract = None;
                    continue;
                }
                let launch =
                    general_typed_launch_v3(root.frontend_contract.as_ref(), &registration_path)?;
                let contract = crate::rust_type_layout_v3::extract_general_typed_kernel_v3(
                    tcx,
                    root.target,
                    &root.logical_name,
                    &root.export_name,
                    &launch,
                )
                .map_err(|error| {
                    RegistrationError::new(
                        &registration_path,
                        format!("general rustc type/layout extraction failed: {error}"),
                    )
                })?;
                let kernel_binding = root.kernel_binding.ok_or_else(|| {
                    RegistrationError::new(&registration_path, "V3 root has no kernel binding")
                })?;
                let derived = derive_generated_host_contract_identity_v1(
                    MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
                    kernel_binding.as_bytes(),
                    &root.logical_name,
                    &root.export_name,
                    contract.abi(),
                    contract.launch(),
                );
                if derived.as_bytes() != &generated_host_contract_identity.as_bytes() {
                    return Err(RegistrationError::new(
                        &registration_path,
                        format!(
                            "generated host-contract identity {} disagrees with rustc-derived identity {}",
                            generated_host_contract_identity.to_hex(),
                            encode_lower_hex(derived.as_bytes())
                        ),
                    ));
                }
                let identities = contract
                    .arguments()
                    .iter()
                    .map(|argument| argument.type_identity())
                    .collect();
                root.typed_layout_identities =
                    Some(TypedArgumentListV1::new(identities).map_err(|error| {
                        RegistrationError::new(&registration_path, error.to_string())
                    })?);
                root.general_typed_contract = Some(contract);
            }
            None => {
                root.typed_layout_identities = None;
                root.general_typed_contract = None;
            }
        }
    }
    Ok(roots)
}

fn general_typed_launch_v3(
    frontend: Option<&AuthenticatedKernelFrontendContractV1>,
    registration_path: &str,
) -> Result<LaunchContract, RegistrationError> {
    const DEFAULT: [u32; 3] = [256, 1, 1];
    const WAVE64: [u32; 3] = [64, 1, 1];

    let dimensions = match frontend.and_then(|frontend| frontend.contract().launch()) {
        None => DEFAULT,
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
            if required != DEFAULT && required != WAVE64 {
                return Err(RegistrationError::new(
                    registration_path,
                    "general typed V3 supports only exact 64x1x1 or 256x1x1 launch dimensions",
                ));
            }
            required
        }
    };
    LaunchContract::new(
        1,
        BlockSize::Exact(
            Dimensions::new(dimensions[0], dimensions[1], dimensions[2])
                .map_err(|error| RegistrationError::new(registration_path, error.to_string()))?,
        ),
        Dimensions::new(u32::MAX, 1, 1)
            .map_err(|error| RegistrationError::new(registration_path, error.to_string()))?,
        0,
        0,
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

fn authenticate_external_typed_v2_registration<'tcx>(
    tcx: TyCtxt<'tcx>,
    record: &RegistrationRecord<Instance<'tcx>>,
) -> Result<(), RegistrationError> {
    if record.version != reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V2
        || record.kind != reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2
    {
        return Err(RegistrationError::new(
            &record.registration_path,
            "cross-crate kernels require an exact producer-owned typed V2 registration",
        ));
    }
    let crate_num = record.target.def_id().krate;
    let actual_crate_name = tcx.crate_name(crate_num).to_string();
    if record.target_crate_name != actual_crate_name {
        return Err(RegistrationError::new(
            &record.registration_path,
            format!(
                "external target crate `{}` disagrees with rustc producer crate `{actual_crate_name}`",
                record.target_crate_name
            ),
        ));
    }
    let expected_name = format!(
        "{}{}",
        reserved_fe2o3_symbols::KERNEL_REGISTRATION_PREFIX,
        record.logical_name,
    );
    let mut matches = Vec::new();
    for index in 0..tcx.num_extern_def_ids(crate_num) {
        let def_id = DefId {
            krate: crate_num,
            index: DefIndex::from_usize(index),
        };
        if !matches!(tcx.def_kind(def_id), rustc_hir::def::DefKind::Static { .. }) {
            continue;
        }
        let path = tcx.def_path_str(def_id);
        if final_path_segment(&path) != expected_name {
            continue;
        }
        if tcx.is_mutable_static(def_id) {
            return Err(RegistrationError::new(
                path,
                "upstream kernel registration must be immutable",
            ));
        }
        let flags = tcx.codegen_fn_attrs(def_id).flags;
        if !flags.intersects(CodegenFnAttrFlags::USED_COMPILER | CodegenFnAttrFlags::USED_LINKER) {
            return Err(RegistrationError::new(
                path,
                "upstream kernel registration must carry #[used]",
            ));
        }
        let ty = tcx
            .try_normalize_erasing_regions(
                TypingEnv::fully_monomorphized(),
                tcx.type_of(def_id).instantiate_identity(),
            )
            .map_err(|_| {
                RegistrationError::new(&path, "upstream kernel registration type did not normalize")
            })?;
        let TyKind::Tuple(types) = ty.kind() else {
            return Err(RegistrationError::new(
                path,
                "upstream kernel registration must use the exact V2 tuple type",
            ));
        };
        let exact_v2 = types.len() == reserved_fe2o3_symbols::KERNEL_REGISTRATION_V2_FIELD_COUNT
            && types[0] == tcx.types.u64
            && types[1] == tcx.types.u16
            && types[2] == tcx.types.u16
            && is_shared_str(types[3])
            && is_shared_str(types[4])
            && is_shared_str(types[5])
            && is_shared_str(types[6])
            && matches!(types[7].kind(), TyKind::FnPtr(..));
        if !exact_v2 {
            return Err(RegistrationError::new(
                path,
                "upstream kernel registration must use the exact V2 tuple type",
            ));
        }
        require_static_registration_integer(
            tcx,
            def_id,
            0,
            u128::from(reserved_fe2o3_symbols::KERNEL_REGISTRATION_MAGIC),
            "magic",
            &path,
        )?;
        require_static_registration_integer(
            tcx,
            def_id,
            1,
            u128::from(reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V2),
            "version",
            &path,
        )?;
        require_static_registration_integer(
            tcx,
            def_id,
            2,
            u128::from(reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2),
            "kind",
            &path,
        )?;
        require_static_registration_string(
            tcx,
            def_id,
            3,
            &record.logical_name,
            "logical name",
            &path,
        )?;
        require_static_registration_string(
            tcx,
            def_id,
            4,
            &record.export_name,
            "export name",
            &path,
        )?;
        require_static_registration_string(
            tcx,
            def_id,
            5,
            &record
                .crate_binding
                .expect("external V2 record has a crate binding")
                .to_hex(),
            "crate binding",
            &path,
        )?;
        require_static_registration_string(
            tcx,
            def_id,
            6,
            &record
                .kernel_binding
                .expect("external V2 record has a kernel binding")
                .to_hex(),
            "kernel binding",
            &path,
        )?;
        let target = crate::static_registration::function(tcx, def_id, 7).map_err(|reason| {
            RegistrationError::new(
                &path,
                format!("upstream kernel registration function is invalid: {reason}"),
            )
        })?;
        if target != record.target {
            return Err(RegistrationError::new(
                path,
                format!(
                    "upstream kernel registration points to `{}`, not imported target `{}`",
                    tcx.def_path_str(target.def_id()),
                    record.target_identity,
                ),
            ));
        }
        if owner_module_path(&path) != owner_module_path(&record.target_identity) {
            return Err(RegistrationError::new(
                path,
                "upstream kernel registration and target must share one producer module",
            ));
        }
        matches.push(def_id);
    }
    if matches.len() != 1 {
        return Err(RegistrationError::new(
            &record.registration_path,
            format!(
                "external kernel `{}` has {} exact producer registrations in crate `{actual_crate_name}`; exactly one is required",
                record.target_identity,
                matches.len(),
            ),
        ));
    }
    Ok(())
}

fn require_static_registration_integer(
    tcx: TyCtxt<'_>,
    def_id: DefId,
    index: usize,
    expected: u128,
    field: &str,
    path: &str,
) -> Result<(), RegistrationError> {
    let observed = crate::static_registration::integer(tcx, def_id, index).map_err(|reason| {
        RegistrationError::new(
            path,
            format!("upstream kernel registration {field} is invalid: {reason}"),
        )
    })?;
    (observed == expected).then_some(()).ok_or_else(|| {
        RegistrationError::new(
            path,
            format!(
                "upstream kernel registration {field} {observed} does not match imported value {expected}"
            ),
        )
    })
}

fn require_static_registration_string(
    tcx: TyCtxt<'_>,
    def_id: DefId,
    index: usize,
    expected: &str,
    field: &str,
    path: &str,
) -> Result<(), RegistrationError> {
    let observed = crate::static_registration::string(tcx, def_id, index).map_err(|reason| {
        RegistrationError::new(
            path,
            format!("upstream kernel registration {field} is invalid: {reason}"),
        )
    })?;
    (observed == expected).then_some(()).ok_or_else(|| {
        RegistrationError::new(
            path,
            format!(
                "upstream kernel registration {field} `{observed}` does not match imported value `{expected}`"
            ),
        )
    })
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
        if record.contract.unsafe_assembly().is_some()
            && tcx.fn_sig(record.target.def_id()).skip_binder().safety() != Safety::Unsafe
        {
            return Err(error(
                "unsafe-assembly contracts require an unsafe registered kernel function".to_owned(),
            ));
        }

        root.frontend_contract = Some(AuthenticatedKernelFrontendContractV1 {
            registration_path: record.registration_path,
            target_def_path_hash: tcx.def_path_hash(record.target.def_id()).0.to_le_bytes(),
            target_symbol: record.target_symbol,
            canonical_bytes: record.canonical_bytes,
            contract: record.contract,
            reachable_assembly: ReachableAssemblySummaryV1::default(),
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
    allow_external_typed_v2: bool,
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
        let typed_profile = match (record.version, record.kind) {
            (
                reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V1,
                reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_KERNEL,
            ) => None,
            (
                reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V2,
                reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2,
            ) => Some(TypedKernelProfile::VecAddRustcLayoutV2),
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
                        "V3 profile tag `{profile_tag}` is not the canonical general typed profile `{TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3}`"
                    )));
                }
                let generated_host_contract_identity =
                    record.generated_host_contract_identity.ok_or_else(|| {
                        error("V3 registration has no generated host-contract identity".to_owned())
                    })?;
                Some(TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
                    generated_host_contract_identity,
                })
            }
            (reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V1, kind)
                if kind
                    == reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2 =>
            {
                return Err(error("typed registrations require version 2".to_owned()));
            }
            (_, kind)
                if kind == reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1 =>
            {
                return Err(error(
                    "typed vecadd profile V1 uses unauthenticated opaque layout identities and is no longer accepted"
                        .to_owned(),
                ));
            }
            (reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V2, kind)
                if kind == reserved_fe2o3_symbols::KERNEL_REGISTRATION_KIND_KERNEL =>
            {
                return Err(error(
                    "ordinary registrations must remain version 1".to_owned(),
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
                    && version != reserved_fe2o3_symbols::KERNEL_REGISTRATION_VERSION_V2
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
        if typed_profile.is_some() {
            let _expected_crate_name = expected_crate_name.ok_or_else(|| {
                error("typed registration has no rustc session crate identity".to_owned())
            })?;
            if target_is_external {
                if typed_profile != Some(TypedKernelProfile::VecAddRustcLayoutV2) {
                    return Err(error(
                        "cross-crate kernels are bounded to typed V2".to_owned(),
                    ));
                }
                if !allow_external_typed_v2 {
                    return Err(error(
                        "cross-crate typed V2 kernels are bounded to gfx942:xnack-".to_owned(),
                    ));
                }
            } else if registration_module != target_module {
                return Err(error(format!(
                    "registered target module `{target_module}` disagrees with registration module `{registration_module}`"
                )));
            }
        }

        let kernel_binding = match typed_profile {
            Some(TypedKernelProfile::VecAddRustcLayoutV2) => {
                let crate_binding = record
                    .crate_binding
                    .ok_or_else(|| error("V2 registration has no crate binding".to_owned()))?;
                let expected = expected_crate_binding.ok_or_else(|| {
                    error("V2 registration has no rustc session crate binding".to_owned())
                })?;
                if !target_is_external && crate_binding != expected {
                    return Err(error(format!(
                        "crate binding {} disagrees with rustc session binding {}",
                        crate_binding.to_hex(),
                        expected.to_hex()
                    )));
                }
                let declared = record
                    .kernel_binding
                    .ok_or_else(|| error("V2 registration has no kernel binding".to_owned()))?;
                let expected = derive_kernel_binding_id_v1(
                    crate_binding,
                    TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
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
            Some(TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. }) => {
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

        let authenticated_owner = match typed_profile {
            Some(typed_profile) => Some(AuthenticatedKernelOwner {
                target: record.target,
                crate_name: record.target_crate_name.clone(),
                module_path: target_module.to_owned(),
                logical_name: record.logical_name.clone(),
                export_name: record.export_name.clone(),
                typed_profile,
                target_def_path: record.target_identity.clone(),
                crate_binding: record
                    .crate_binding
                    .expect("typed registration crate binding was validated above"),
                kernel_binding: kernel_binding
                    .expect("typed registration kernel binding was validated above"),
                observed_symbol: record.target_symbol.clone(),
            }),
            None => None,
        };

        roots.push(KernelRoot {
            target: record.target,
            logical_name: record.logical_name,
            export_name: record.export_name,
            typed_profile,
            kernel_binding,
            authenticated_owner,
            typed_layout_identities: None,
            general_typed_contract: None,
            frontend_contract: None,
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
        if arg.has_param() || arg.has_escaping_bound_vars() {
            return false;
        }
    }

    generics.count() == 0 || !instance.args.is_empty()
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
    inline_assembly:
        BTreeMap<crate::device_ffi::DeviceFfiInstanceIdentity, ObservedInlineAssemblyV1>,
    used_export_names: BTreeSet<String>,
    worklist: VecDeque<CollectedFunction<'tcx>>,
    result: Vec<CollectedFunction<'tcx>>,
    authenticated_kernel_owners: AuthenticatedKernelOwners<Instance<'tcx>>,
    ffi_declarations: Vec<crate::device_ffi::CollectedDeviceFfi<'tcx>>,
    reachable_ffi_imports: BTreeSet<reserved_fe2o3_symbols::DeviceFfiContractIdV1>,
    expected_target: String,
    purpose: CollectorPurposeV1,
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

#[derive(Default)]
struct UserUnsafeBlockVisitor {
    first_span: Option<rustc_span::Span>,
}

impl<'tcx> Visitor<'tcx> for UserUnsafeBlockVisitor {
    fn visit_block(&mut self, block: &'tcx rustc_hir::Block<'tcx>) {
        if matches!(
            block.rules,
            BlockCheckMode::UnsafeBlock(UnsafeSource::UserProvided)
        ) {
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
        purpose: CollectorPurposeV1,
    ) -> Self {
        Self {
            tcx,
            seen: BTreeSet::new(),
            call_chains: BTreeMap::new(),
            call_edges: BTreeMap::new(),
            reachable_unsafe_calls: BTreeMap::new(),
            inline_assembly: BTreeMap::new(),
            used_export_names: BTreeSet::new(),
            worklist: VecDeque::new(),
            result: Vec::new(),
            authenticated_kernel_owners: AuthenticatedKernelOwners::default(),
            ffi_declarations,
            reachable_ffi_imports: BTreeSet::new(),
            expected_target: target,
            purpose,
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
                typed_profile: None,
                kernel_binding: None,
                typed_layout_identities: None,
                general_typed_contract: None,
                frontend_contract: None,
                reference_effect_binding: None,
                dead_branches: None,
            });
        }
        Ok(())
    }

    fn add_root(&mut self, root: KernelRoot<Instance<'tcx>>) -> Result<(), CollectError> {
        let KernelRoot {
            target: instance,
            logical_name,
            export_name,
            typed_profile,
            kernel_binding,
            authenticated_owner,
            typed_layout_identities,
            general_typed_contract,
            frontend_contract,
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
            if let Some(owner) = authenticated_owner {
                debug_assert_eq!(owner.target, instance);
                self.authenticated_kernel_owners.push(owner);
            }
            self.worklist.push_back(CollectedFunction {
                instance,
                role: CollectedFunctionRole::KernelEntry,
                export_name,
                logical_name: Some(logical_name),
                typed_profile,
                kernel_binding,
                typed_layout_identities,
                general_typed_contract,
                frontend_contract,
                reference_effect_binding,
                dead_branches: None,
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
            if crate::closure_profile_v1::contains_concrete_closure_v1(self.tcx, function.instance)
                .map_err(|error| {
                    self.reachable_error(
                        &function.instance,
                        &format!("closure presence check failed closed: {error}"),
                        None,
                    )
                })?
            {
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
                        "[collector] gfx942 closure profile: {} environment(s), {} static call(s), identity {}",
                        closure_plan.environments().len(),
                        closure_plan.calls().len(),
                        encode_lower_hex(&closure_plan.identity()),
                    );
                }
            }
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

            for (block_id, block) in mir.basic_blocks.iter_enumerated() {
                if !self
                    .purpose
                    .inspect_block(dead_branches.includes_block(block_id.as_usize()))
                {
                    continue;
                }
                if let Some(terminator) = &block.terminator {
                    self.process_terminator(
                        &terminator.kind,
                        mir,
                        &function.instance,
                        function.is_kernel_entry(),
                    )?;
                }
            }

            function.dead_branches = Some(dead_branches);
            self.result.push(function);
        }

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
            authenticated_kernel_owners: self.authenticated_kernel_owners,
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
        terminator: &TerminatorKind<'tcx>,
        body: &Body<'tcx>,
        caller: &Instance<'tcx>,
        is_kernel_root: bool,
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
                self.process_call_operand(func, caller)
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
                if (is_kernel_root || self.purpose.accepts_extended_edges())
                    && matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable) =>
            {
                Ok(())
            }
            TerminatorKind::Drop { place, unwind, .. }
                if self.purpose.accepts_extended_edges()
                    && matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable) =>
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

    fn authenticate_production_kernel_source_safety(&self) -> Result<(), CollectError> {
        if !self.purpose.is_production() {
            return Ok(());
        }

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
            if root
                .frontend_contract
                .as_ref()
                .and_then(|contract| contract.contract.unsafe_assembly())
                .is_some()
            {
                continue;
            }
            let logical_name = root.logical_name.as_deref().unwrap_or(&root.export_name);
            let root_identity = self.instance_identity(root.instance);
            let (links, order) = root_scoped_call_chains(&self.call_edges, &labels, &root_identity);

            for identity in order {
                let function = functions
                    .get(&identity)
                    .expect("root-scoped traversal retains only collected function labels");
                let chain = || reconstruct_call_chain(&links, &identity).join(" -> ");
                if self
                    .tcx
                    .fn_sig(function.instance.def_id())
                    .skip_binder()
                    .safety()
                    == Safety::Unsafe
                {
                    return Err(CollectError {
                        message: format!(
                            "ordinary production kernel `{logical_name}` reaches unsafe function instance `{}`; reachable call chain: {}",
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
                            "ordinary production kernel `{logical_name}` reaches unsafe function instance `{callee}`; reachable call chain: {} -> {callee}",
                            chain(),
                        ),
                    });
                }

                let Some(local_def_id) = function.instance.def_id().as_local() else {
                    return Err(CollectError {
                        message: format!(
                            "ordinary production kernel `{logical_name}` cannot authenticate the absence of user-provided unsafe blocks in external helper `{}`: cross-crate HIR is unavailable and optimized MIR does not retain unsafe-block syntax; reachable call chain: {}",
                            self.instance_label(function.instance),
                            chain(),
                        ),
                    });
                };
                let Some(body) = self.tcx.hir_maybe_body_owned_by(local_def_id) else {
                    return Err(CollectError {
                        message: format!(
                            "ordinary production kernel `{logical_name}` cannot authenticate local HIR for reachable function `{}`; reachable call chain: {}",
                            self.instance_label(function.instance),
                            chain(),
                        ),
                    });
                };
                let mut visitor = UserUnsafeBlockVisitor::default();
                visitor.visit_body(body);
                if let Some(span) = visitor.first_span {
                    return Err(CollectError {
                        message: format!(
                            "ordinary production kernel `{logical_name}` reaches a safe-signature local helper containing a user-provided unsafe block at {}; reachable call chain: {}",
                            self.tcx.sess.source_map().span_to_diagnostic_string(span),
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
        if self.purpose.is_production()
            && self.tcx.fn_sig(*def_id).skip_binder().safety() == Safety::Unsafe
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
            let instance = Instance::try_resolve(
                self.tcx,
                TypingEnv::fully_monomorphized(),
                *def_id,
                normalized_args,
            )
            .map_err(|_| {
                self.reachable_error(
                    caller,
                    "device FFI declaration normalization failed",
                    Some(self.tcx.def_path_str(*def_id)),
                )
            })?
            .ok_or_else(|| {
                self.reachable_error(
                    caller,
                    "device FFI declaration did not resolve to a concrete instance",
                    Some(self.tcx.def_path_str(*def_id)),
                )
            })?;
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
        if crate::trusted_device_items::classify(self.tcx, *def_id).is_some() {
            if self.verbose {
                eprintln!(
                    "[collector] stopping at trusted device item {}",
                    self.tcx.def_path_str(*def_id)
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

        let args = normalized_args;

        let resolved = match Instance::try_resolve(
            self.tcx,
            TypingEnv::fully_monomorphized(),
            *def_id,
            args,
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

        // Production stops only at the workload-neutral reviewed device
        // registry. The sole importer must apply that item's explicit
        // expand-or-reject rule; workload/profile classifiers remain oracles.
        if self.purpose.is_production()
            && crate::production_semantic_terminal_v1::classify(self.tcx, resolved.def_id())
                .is_some()
        {
            if self.verbose {
                eprintln!(
                    "[collector] stopping at registered semantic terminal {}",
                    self.tcx.def_path_str(resolved.def_id())
                );
            }
            return Ok(());
        }

        if self.purpose.is_qualification_oracle()
            && crate::collected_workgroup_sync_v1::is_exact_workgroup_sync_rustc_intrinsic(
                self.tcx,
                resolved.def_id(),
            )
        {
            return Ok(());
        }
        if self.purpose.is_qualification_oracle()
            && crate::collected_flash_attention_v1::classify_exact_flash_attention_compiler_intrinsic(
                self.tcx,
                resolved.def_id(),
            )
            .is_some()
        {
            return Ok(());
        }
        if self.purpose.is_qualification_oracle()
            && crate::collected_moe_top2_v1::classify_exact_moe_top2_compiler_intrinsic(
                self.tcx,
                resolved.def_id(),
            )
            .is_some()
        {
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
        if self.purpose.is_qualification_oracle()
            && crate::trusted_device_items::classify(self.tcx, resolved.def_id()).is_some()
        {
            if self.verbose {
                eprintln!(
                    "[collector] stopping at resolved trusted device item {}",
                    self.tcx.def_path_str(resolved.def_id())
                );
            }
            return Ok(());
        }
        if self.purpose.is_qualification_oracle()
            && crate::collected_workgroup_sync_v1::is_exact_workgroup_sync_compiler_intrinsic(
                self.tcx,
                resolved.def_id(),
            )
        {
            if self.verbose {
                eprintln!(
                    "[collector] stopping at exact workgroup-sync compiler intrinsic {}",
                    self.tcx.def_path_str(resolved.def_id())
                );
            }
            return Ok(());
        }
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
            typed_profile: None,
            kernel_binding: None,
            typed_layout_identities: None,
            general_typed_contract: None,
            frontend_contract: None,
            reference_effect_binding: None,
            dead_branches: None,
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
        AuthenticatedKernelFrontendContractV1, CallChainLink, CollectorPurposeV1, KernelRoot,
        ObservedInlineAssemblyV1, RegistrationError, RegistrationRecord, TypedArgumentListError,
        TypedArgumentListV1, TypedKernelProfile, general_typed_launch_v3,
        reconcile_frontend_contract, reconstruct_call_chain, root_scoped_call_chains,
        validate_registration_records as validate_records,
    };
    use fe2o3_artifacts::{
        BlockSize, DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestBytes, Dimensions,
        TypeIdentity,
    };
    use fe2o3_kernel_descriptor::MAX_ARGUMENTS_PER_KERNEL;
    use fe2o3_rustc_front::{
        ASSEMBLY_OPERAND_SGPR_V1, ASSEMBLY_OPTION_NOMEM_V1, ASSEMBLY_OPTION_NOSTACK_V1,
        ASSEMBLY_OPTION_PRESERVES_FLAGS_V1, FrontendLaunchBoundsV1,
        FrontendUnsafeAssemblyDeclarationV1, FrontendUnsafeAssemblyTargetV1,
        FrontendWorkgroupDimensionsV1, KernelFrontendContractV1,
    };
    use reserved_fe2o3_symbols::{
        GeneratedHostContractIdV3, KERNEL_PREFIX, KERNEL_REGISTRATION_KIND_KERNEL,
        KERNEL_REGISTRATION_KIND_TYPED_GENERAL_LAYOUT_V3,
        KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2, KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1,
        KERNEL_REGISTRATION_MAGIC, KERNEL_REGISTRATION_PREFIX, KERNEL_REGISTRATION_VERSION_V1,
        KERNEL_REGISTRATION_VERSION_V2, KERNEL_REGISTRATION_VERSION_V3, KernelBindingIdV1,
        TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3, TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
        derive_crate_binding_id_v1, derive_kernel_binding_id_v1, host_kernel_symbol_v1,
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
        validate_records(records, expected_crate_binding, Some("fixture"), false)
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

    fn typed_registration(
        path: &str,
        logical_name: &str,
        export_name: &str,
        target: u8,
    ) -> RegistrationRecord<u8> {
        let mut registration = registration(path, logical_name, export_name, target);
        let crate_binding = derive_crate_binding_id_v1("fixture", ["metadata"]);
        let kernel_binding = derive_kernel_binding_id_v1(
            crate_binding,
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            logical_name,
            export_name,
        );
        registration.version = KERNEL_REGISTRATION_VERSION_V2;
        registration.kind = KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2;
        registration.crate_binding = Some(crate_binding);
        registration.kernel_binding = Some(kernel_binding);
        registration.target_symbol = host_kernel_symbol_v1(kernel_binding);
        registration
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
        let typed = typed_registration(
            "crate::__fe2o3_kernel_registration_vecadd",
            "vecadd",
            "vecadd",
            7,
        );
        let roots = validate_registration_records(vec![typed]).unwrap();

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].target, 7);
        assert_eq!(roots[0].logical_name, "vecadd");
        assert_eq!(roots[0].export_name, "vecadd");
        assert_eq!(
            roots[0].typed_profile,
            Some(TypedKernelProfile::VecAddRustcLayoutV2)
        );
        assert!(roots[0].kernel_binding.is_some());
    }

    #[test]
    fn gfx942_external_typed_v2_registration_preserves_producer_binding() {
        let mut typed = typed_registration(
            "consumer::__fe2o3_kernel_registration_vecadd",
            "vecadd",
            "vecadd",
            7,
        );
        typed.target_crate_name = "producer".to_owned();
        typed.target_identity = "producer::kernels::__fe2o3_host_kernel_v1_vecadd".to_owned();
        let producer_binding = derive_crate_binding_id_v1("producer", ["producer-metadata"]);
        let kernel_binding = derive_kernel_binding_id_v1(
            producer_binding,
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            "vecadd",
            "vecadd",
        );
        typed.crate_binding = Some(producer_binding);
        typed.kernel_binding = Some(kernel_binding);
        typed.target_symbol = host_kernel_symbol_v1(kernel_binding);
        let consumer_binding = derive_crate_binding_id_v1("consumer", ["consumer-metadata"]);

        let roots = validate_records(
            vec![typed.clone()],
            Some(consumer_binding),
            Some("consumer"),
            true,
        )
        .unwrap();

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].kernel_binding, Some(kernel_binding));
        assert_eq!(
            roots[0].authenticated_owner.as_ref().unwrap().crate_name(),
            "producer"
        );

        let substituted = KernelBindingIdV1::from_bytes([0x5a; 32]);
        typed.kernel_binding = Some(substituted);
        typed.target_symbol = host_kernel_symbol_v1(substituted);
        let error = validate_records(vec![typed], Some(consumer_binding), Some("consumer"), true)
            .unwrap_err();
        assert!(
            error.to_string().contains("disagrees with derived binding"),
            "substituted producer binding was not rejected: {error}"
        );
    }

    #[test]
    fn external_typed_v2_registration_fails_closed_outside_gfx942_profile() {
        let mut typed = typed_registration(
            "consumer::__fe2o3_kernel_registration_vecadd",
            "vecadd",
            "vecadd",
            7,
        );
        typed.target_crate_name = "producer".to_owned();
        typed.target_identity = "producer::kernels::__fe2o3_host_kernel_v1_vecadd".to_owned();
        let producer_binding = derive_crate_binding_id_v1("producer", ["producer-metadata"]);
        let kernel_binding = derive_kernel_binding_id_v1(
            producer_binding,
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            "vecadd",
            "vecadd",
        );
        typed.crate_binding = Some(producer_binding);
        typed.kernel_binding = Some(kernel_binding);
        typed.target_symbol = host_kernel_symbol_v1(kernel_binding);

        let error = validate_records(
            vec![typed],
            Some(derive_crate_binding_id_v1(
                "consumer",
                ["consumer-metadata"],
            )),
            Some("consumer"),
            false,
        )
        .unwrap_err();
        assert!(error.to_string().contains("bounded to gfx942:xnack-"));
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
        assert!(matches!(
            roots[0].typed_profile,
            Some(TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
                generated_host_contract_identity,
            }) if generated_host_contract_identity.as_bytes() == [0x63; 32]
        ));
    }

    #[test]
    fn authenticated_kernel_owner_exposes_stable_owner_and_exact_build_observation() {
        let registration = general_typed_registration(
            "general_genuine::__fe2o3_kernel_registration_alpha",
            "alpha",
            "alpha",
            3,
        );
        let expected_crate_binding = registration.crate_binding.unwrap();
        let expected_kernel_binding = registration.kernel_binding.unwrap();
        let expected_symbol = registration.target_symbol.clone();
        let expected_def_path = registration.target_identity.clone();
        let roots = validate_registration_records(vec![registration]).unwrap();
        let owner = roots[0].authenticated_owner.as_ref().unwrap();

        assert_eq!(owner.target(), 3);
        assert_eq!(owner.crate_name(), "fixture");
        assert_eq!(owner.module_path(), "general_genuine");
        assert_eq!(owner.logical_name(), "alpha");
        assert_eq!(owner.export_name(), "alpha");
        assert!(matches!(
            owner.typed_profile(),
            TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 {
                generated_host_contract_identity,
            } if generated_host_contract_identity.as_bytes() == [0x63; 32]
        ));
        assert_eq!(owner.target_def_path(), expected_def_path);
        assert_eq!(owner.crate_binding(), expected_crate_binding);
        assert_eq!(owner.kernel_binding(), expected_kernel_binding);
        assert_eq!(owner.observed_symbol(), expected_symbol);
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
            validate_records(vec![registration.clone()], None, Some("fixture"), false).unwrap_err();
        assert!(error.reason.contains("no rustc session crate binding"));

        let error = validate_records(
            vec![registration.clone()],
            Some(expected_crate_binding),
            None,
            false,
        )
        .unwrap_err();
        assert!(error.reason.contains("no rustc session crate identity"));

        let mut wrong_crate = registration.clone();
        wrong_crate.target_crate_name = "other".to_owned();
        let error = validate_records(
            vec![wrong_crate],
            Some(expected_crate_binding),
            Some("fixture"),
            false,
        )
        .unwrap_err();
        assert!(error.reason.contains("bounded to typed V2"));

        let mut wrong_module = registration.clone();
        wrong_module.target_identity = format!("spoof::{}", wrong_module.target_symbol);
        let error = validate_registration_records(vec![wrong_module]).unwrap_err();
        assert!(error.reason.contains("target module `spoof`"));

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
            false,
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
                .contains("require version 3")
        );

        let mut old_kind = registration;
        old_kind.kind = KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2;
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
    fn typed_registration_identity_and_host_symbol_fail_closed() {
        let typed = typed_registration(
            "crate::__fe2o3_kernel_registration_vecadd",
            "vecadd",
            "vecadd",
            7,
        );
        let wrong_crate = derive_crate_binding_id_v1("other", ["metadata"]);
        let error = validate_records(
            vec![typed.clone()],
            Some(wrong_crate),
            Some("fixture"),
            false,
        )
        .unwrap_err();
        assert!(
            error
                .reason
                .contains("disagrees with rustc session binding")
        );

        let mut wrong_kernel = typed.clone();
        wrong_kernel.kernel_binding = Some(derive_kernel_binding_id_v1(
            wrong_kernel.crate_binding.unwrap(),
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            "different",
            "different",
        ));
        let error = validate_registration_records(vec![wrong_kernel]).unwrap_err();
        assert!(error.reason.contains("disagrees with derived binding"));

        let mut logical_host_symbol = typed;
        logical_host_symbol.target_symbol = format!("{KERNEL_PREFIX}vecadd");
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
        for dimensions in [[64, 1, 1], [256, 1, 1]] {
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

        for frontend in [
            authenticated(None, Some([64, 1, 1]), None),
            authenticated(Some([64, 1, 1]), Some([256, 1, 1]), None),
            authenticated(Some([256, 1, 1]), Some([256, 1, 1]), Some(2)),
            authenticated(Some([32, 2, 1]), None, None),
            authenticated(Some([128, 1, 1]), None, None),
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
        obsolete_typed.kind = KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1;
        assert!(
            validate_registration_records(vec![obsolete_typed])
                .unwrap_err()
                .reason
                .contains("opaque layout identities")
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

        let typed_duplicate = typed_registration(
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
    fn collector_purposes_keep_output_and_terminal_authority_explicit() {
        let production = CollectorPurposeV1::Production;
        assert!(production.inspect_block(false));
        assert!(production.inspect_block(true));
        assert!(production.accepts_extended_edges());
        assert!(!production.is_qualification_oracle());
        assert!(production.is_production());

        let qualification = CollectorPurposeV1::QualificationOracle {
            extended_helper_edges: false,
        };
        assert!(!qualification.inspect_block(false));
        assert!(!qualification.accepts_extended_edges());
        assert!(qualification.is_qualification_oracle());
        assert!(!qualification.is_production());

        let extended_qualification = CollectorPurposeV1::QualificationOracle {
            extended_helper_edges: true,
        };
        assert!(extended_qualification.accepts_extended_edges());
    }

    #[test]
    fn production_registration_surface_accepts_every_valid_registration_version() {
        let ordinary = validate_registration_records(vec![registration(
            "fixture::__fe2o3_kernel_plain",
            "plain",
            "plain",
            1,
        )])
        .unwrap();
        let typed = validate_registration_records(vec![typed_registration(
            "fixture::__fe2o3_kernel_typed",
            "typed",
            "typed",
            2,
        )])
        .unwrap();
        let general = validate_registration_records(vec![general_typed_registration(
            "fixture::__fe2o3_kernel_general",
            "general",
            "general",
            3,
        )])
        .unwrap();

        assert!(ordinary[0].typed_profile.is_none());
        assert!(matches!(
            typed[0].typed_profile,
            Some(TypedKernelProfile::VecAddRustcLayoutV2)
        ));
        assert!(matches!(
            general[0].typed_profile,
            Some(TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. })
        ));
    }

    #[test]
    fn qualification_entry_requires_resolved_target_and_explicit_route_token() {
        let source = include_str!("collector.rs");
        let qualification = source
            .split_once("pub(crate) fn collect_qualification_device_functions")
            .unwrap()
            .1
            .split_once("pub(crate) fn collect_authenticated_kernel_closure_v1")
            .unwrap()
            .0;
        for forbidden in ["std::env", "QUALIFICATION_ORACLE_ENV", "FE2O3_TARGET"] {
            assert!(
                !qualification.contains(forbidden),
                "qualification collection entry contains hidden input {forbidden:?}"
            );
        }
        assert!(qualification.contains("target: &crate::AmdGpuTarget"));
        assert!(qualification.contains("SelectedQualificationOracle"));
    }

    #[test]
    fn production_entry_receives_target_without_reading_process_state() {
        let source = include_str!("collector.rs");
        let production = source
            .split_once("pub(crate) fn collect_authenticated_kernel_closure_v1")
            .unwrap()
            .1
            .split_once("fn collect_device_functions_for_purpose")
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
}
