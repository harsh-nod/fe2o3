//! Sole consuming boundary from production rustc collection to semantic MIR.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use dialect_amdgcn::DeviceValueDiagnosticItem;
use fe2o3_artifacts::{BlockSize, LaunchContract};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, HARD_MAX_FUNCTIONS_V1, HARD_MAX_ROOTS_V1,
    InertSemanticMirRequestV1, SemanticAbiArgumentRoleV1, SemanticAbiPassModeV1,
    SemanticBackendReprV1, SemanticBf16ConversionKindV1, SemanticCallableDeclV1,
    SemanticCallableIdV1, SemanticCanonAbiV1, SemanticCapabilityMemoryContractV1,
    SemanticCompilerIntrinsicIdentityV1, SemanticCompilerIntrinsicOperationV1,
    SemanticDisjointIndexSpaceV1, SemanticExecutionAtomicKindV1,
    SemanticExecutionCapabilityContractV1, SemanticExecutionCapabilityOperationV1,
    SemanticExecutionCapabilitySignatureV1, SemanticExecutionCollectiveKindV1,
    SemanticExecutionMemoryAccessV1, SemanticExecutionMemoryAddressSpaceV1,
    SemanticExecutionMemoryOrderingV1, SemanticExecutionMemoryScopeV1,
    SemanticExecutionMemorySemanticsV1, SemanticExecutionMemorySpacesV1, SemanticExternAbiV1,
    SemanticF32MathFunctionV1, SemanticFunctionAbiV1, SemanticFunctionIdV1,
    SemanticFunctionIdentityV1, SemanticFunctionRoleV1, SemanticGfx950LdsTransposeFormatV1,
    SemanticKernelBindingIdentityV1, SemanticKernelCapabilityFrontendUnitIdentityV1,
    SemanticKernelCapabilityIssuanceIdentityV1, SemanticKernelCapabilityLaunchBrandIdentityV1,
    SemanticKernelCapabilityProvenanceV1, SemanticKernelCapabilityTargetBrandIdentityV1,
    SemanticKernelEntryV1, SemanticKernelLaunchBoundsV1, SemanticKernelResourceContractV1,
    SemanticKernelSourceContractV1, SemanticLinkSymbolV1, SemanticMfmaAccumulatorContractV1,
    SemanticMfmaAccumulatorDistributionV1, SemanticMfmaOperandContractV1,
    SemanticMfmaOperandRoleV1, SemanticMfmaProfileV1, SemanticMfmaRegisterDistributionV1,
    SemanticMfmaStorageLayoutV1, SemanticMirErrorV1, SemanticMirLimitsV1, SemanticMirResourceV1,
    SemanticMirWireVersionV1, SemanticNonBodyCallableBindingV1, SemanticReachableAssemblyV1,
    SemanticScalarTypeV1, SemanticSourceArgumentOwnershipV1, SemanticSubgroupReductionKindV1,
    SemanticTargetDataLayoutV1, SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeIdentityV1,
    SemanticTypeLayoutDetailsV1, SemanticTypeShapeV1, SemanticUnsafeAssemblyDeclarationV1,
    SemanticUnsafeAssemblyTargetV1, SemanticWorkgroupDimensionsV1,
    SemanticWorkgroupPipelineEventV1, SemanticWorkgroupScanKindV1,
    SemanticWriteOnlyDisjointWriteKindV1,
};
use rustc_middle::ty::{
    FloatTy, GenericArgKind, GenericArgsRef, Instance, IntTy, Ty, TyCtxt, TyKind, UintTy,
};
use rustc_span::{Symbol, sym};

use super::{
    AuthenticatedCollectedKernelClosureV1, AuthenticatedProductionRootV1, CollectedFunctionRole,
    CollectionResult,
};
use crate::production_semantic_body_v1::{
    ProductionSemanticBlockBindingV1, ProductionSemanticBodyErrorV1, ProductionSemanticBodyInputV1,
    ProductionSemanticBodyRequestOwnerV1, ProductionSemanticCallableOwnerEntryV1,
    ProductionSemanticDirectCallBindingV1, ProductionSemanticFunctionExportV1,
    ProductionSemanticFunctionIdentitiesV1, ProductionSemanticLocalBindingV1,
    ProductionSemanticNormalizedRustcIntrinsicRecipeV1,
    ProductionSemanticTerminalExpansionRecipeV1, ProductionSemanticTypeBindingV1,
    construct_production_semantic_body_v1,
};
use crate::production_semantic_fn_abi_v1::{
    ConstructedSemanticFunctionAbisV1, ProductionSemanticFnAbiErrorV1,
    construct_production_semantic_fn_abis_v1,
};
use crate::production_semantic_terminal_v1::ProductionBf16ConversionV1;
use crate::production_semantic_types_v1::{
    ProductionSemanticTypeErrorV1, construct_production_semantic_types_v1,
};
use crate::production_target_v1::ProductionTargetErrorV1;
use crate::rustc_semantic_adapter_v1::{
    SemanticIdentityDigestV1, canonical_function_identities_v1, canonical_target_layout_v1,
    rustc_type_identity_v1,
};
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, ProductionSemanticPreflightErrorV1,
    ProductionSemanticPreflightPlanV1, RetainedSemanticFunctionProducerV1,
    build_production_semantic_preflight_plan_v1,
};
use crate::trusted_device_items::{self, TrustedDeviceItem};

const IDENTITY_INVENTORY_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-identity-inventory/v1";
#[cfg(test)]
const PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V1: &[u8] =
    b"fe2o3/semantic-mir/production-compiler-intrinsic/v1";
#[cfg(test)]
const PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V2: &[u8] =
    b"fe2o3/semantic-mir/production-compiler-intrinsic/v2";
#[cfg(test)]
const PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V3: &[u8] =
    b"fe2o3/semantic-mir/production-compiler-intrinsic/v3";
const PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V4: &[u8] =
    b"fe2o3/semantic-mir/production-compiler-intrinsic/v4";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalIdentitySchemaV1 {
    #[cfg(test)]
    IndependentV1,
    #[cfg(test)]
    CombinedV2,
    #[cfg_attr(not(test), allow(dead_code))]
    CombinedV3,
    CombinedV4,
}

#[derive(Debug)]
pub(crate) enum ProductionSemanticImportErrorV1 {
    Target(ProductionTargetErrorV1),
    RootCustodyMismatch,
    LimitExceeded {
        resource: SemanticMirResourceV1,
        actual: u64,
        maximum: u64,
    },
    FunctionIdentityCollision,
    RootIdentityMismatch,
    KernelContextBinding(&'static str),
    CapabilityTerminalRejected {
        root: String,
        span: String,
        helper_chain: String,
        stage: &'static str,
        detail: String,
    },
    Preflight(Box<ProductionSemanticPreflightErrorV1>),
    TypeConstruction(Box<ProductionSemanticTypeErrorV1>),
    FunctionAbiConstruction(Box<ProductionSemanticFnAbiErrorV1>),
    BodyConstruction(Box<ProductionSemanticBodyErrorV1>),
    SemanticSchema(SemanticMirErrorV1),
    LineageTranscriptTooLarge {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    TargetNeutralLoweringPending {
        functions: usize,
        callables: usize,
        rustc_identity_inventory_sha256: [u8; 32],
        rustc_preflight_plan_sha256: [u8; 32],
        semantic_sha256: [u8; 32],
    },
}

impl fmt::Display for ProductionSemanticImportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target(error) => write!(formatter, "semantic import target rejection: {error}"),
            Self::RootCustodyMismatch => formatter.write_str(
                "semantic importer rejected collector root custody before MIR construction",
            ),
            Self::LimitExceeded {
                resource,
                actual,
                maximum,
            } => write!(
                formatter,
                "semantic importer rejected {resource:?} count {actual} before semantic record allocation; maximum is {maximum}",
            ),
            Self::FunctionIdentityCollision => formatter.write_str(
                "semantic importer independently derived a duplicate canonical function identity",
            ),
            Self::RootIdentityMismatch => formatter.write_str(
                "semantic importer could not bind independently derived roots to unique collected functions",
            ),
            Self::KernelContextBinding(detail) => {
                write!(formatter, "semantic importer rejected kernel-context custody: {detail}")
            }
            Self::CapabilityTerminalRejected {
                root,
                span,
                helper_chain,
                stage,
                detail,
            } => write!(
                formatter,
                "FE2O3-CAP Rejected root={root} span={span} helper_chain={helper_chain} stage={stage}: {detail}",
            ),
            Self::Preflight(error) => write!(formatter, "semantic importer {error}"),
            Self::TypeConstruction(error) => {
                write!(formatter, "semantic importer rejected semantic type construction: {error}")
            }
            Self::FunctionAbiConstruction(error) => write!(
                formatter,
                "semantic importer rejected semantic function ABI construction: {error}",
            ),
            Self::BodyConstruction(error) => {
                write!(formatter, "semantic importer rejected semantic body construction: {error}")
            }
            Self::SemanticSchema(error) => {
                write!(formatter, "semantic importer rejected complete semantic MIR: {error}")
            }
            Self::LineageTranscriptTooLarge {
                field,
                actual,
                maximum,
            } => write!(
                formatter,
                "semantic importer {field} transcript uses {actual} bytes, exceeding the lineage receipt maximum {maximum}"
            ),
            Self::TargetNeutralLoweringPending {
                functions,
                callables,
                rustc_identity_inventory_sha256,
                rustc_preflight_plan_sha256,
                semantic_sha256,
            } => write!(
                formatter,
                "FE2O3-CAP Incomplete root=authenticated-set span=authenticated-set helper_chain=authenticated-closure stage=target-neutral-lowering: semantic importer authenticated rustc identity inventory {} and bounded preflight plan {}, then admitted one complete semantic MIR request with {functions} function(s), {callables} callable(s), and canonical identity {}; an owner-held Pliron locator graph was recursively verified for exact semantic equivalence; target-neutral lowering remains pending; no fallback or artifact emission was entered",
                crate::encode_hex(rustc_identity_inventory_sha256),
                crate::encode_hex(rustc_preflight_plan_sha256),
                crate::encode_hex(semantic_sha256),
            ),
        }
    }
}

impl std::error::Error for ProductionSemanticImportErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Target(error) => Some(error),
            Self::Preflight(error) => Some(error.as_ref()),
            Self::TypeConstruction(error) => Some(error.as_ref()),
            Self::FunctionAbiConstruction(error) => Some(error.as_ref()),
            Self::BodyConstruction(error) => Some(error.as_ref()),
            Self::SemanticSchema(error) => Some(error),
            Self::RootCustodyMismatch
            | Self::LimitExceeded { .. }
            | Self::LineageTranscriptTooLarge { .. }
            | Self::FunctionIdentityCollision
            | Self::RootIdentityMismatch
            | Self::KernelContextBinding(_)
            | Self::CapabilityTerminalRejected { .. } => None,
            Self::TargetNeutralLoweringPending { .. } => None,
        }
    }
}

#[derive(Debug)]
struct ProductionSemanticIdentityInventoryV1<'tcx> {
    functions: Box<[RetainedSemanticFunctionProducerV1<'tcx>]>,
    roots: Box<[SemanticFunctionIdV1]>,
    sha256: [u8; 32],
    canonical_transcript: Box<[u8]>,
}

/// Move-only rustc-produced identity-inventory evidence retained by the
/// production transaction. Public hashes cannot construct this owner.
pub(crate) struct AuthenticatedRustcIdentityInventoryV3 {
    sha256: [u8; 32],
    canonical_transcript: Box<[u8]>,
}

impl AuthenticatedRustcIdentityInventoryV3 {
    pub(crate) const fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    pub(crate) fn canonical_transcript(&self) -> &[u8] {
        &self.canonical_transcript
    }
}

/// Move-only rustc-produced preflight-plan evidence retained by the
/// production transaction. Public hashes cannot construct this owner.
pub(crate) struct AuthenticatedRustcPreflightPlanV3 {
    sha256: [u8; 32],
    rustc_identity_inventory_sha256: [u8; 32],
    canonical_transcript: Box<[u8]>,
}

impl AuthenticatedRustcPreflightPlanV3 {
    pub(crate) const fn sha256(&self) -> [u8; 32] {
        self.sha256
    }

    pub(crate) const fn rustc_identity_inventory_sha256(&self) -> [u8; 32] {
        self.rustc_identity_inventory_sha256
    }

    pub(crate) fn canonical_transcript(&self) -> &[u8] {
        &self.canonical_transcript
    }
}

/// Same-session semantic import result transferred into the sole production
/// transaction. Keeping the custody axes named prevents accidental positional
/// substitution as the import surface grows.
pub(crate) struct ConstructedProductionSemanticMirV1 {
    pub(crate) semantic_mir: AdmittedInertSemanticMirV1,
    pub(crate) rustc_identity_inventory: AuthenticatedRustcIdentityInventoryV3,
    pub(crate) rustc_preflight_plan: AuthenticatedRustcPreflightPlanV3,
    pub(crate) rustc_target: crate::production_target_v1::AuthenticatedProductionTargetV1,
    pub(crate) kernel_contexts: AuthenticatedProductionKernelContextsV1,
    pub(crate) reference_effect_bindings:
        crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    pub(crate) debug_source_files: Box<[fe2o3_kernel_ir::DebugSourceMapFileV1]>,
    pub(crate) debug_source_scopes:
        Box<[crate::rustc_semantic_plan_v1::RetainedDebugSourceScopeV2]>,
    pub(crate) debug_source_variables:
        Box<[crate::rustc_semantic_plan_v1::RetainedDebugSourceVariableV2]>,
    pub(crate) debug_capture_gap: Option<fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1>,
}

const KERNEL_CONTEXT_FRONTEND_UNIT_DOMAIN_V1: &[u8] =
    b"fe2o3/production/kernel-context/frontend-unit/v1";
const KERNEL_CONTEXT_TARGET_DOMAIN_V1: &[u8] = b"fe2o3/production/kernel-context/target-brand/v1";
const KERNEL_CONTEXT_LAUNCH_DOMAIN_V1: &[u8] = b"fe2o3/production/kernel-context/launch-brand/v1";
const KERNEL_CONTEXT_CUSTODY_DOMAIN_V1: &[u8] = b"fe2o3/production/kernel-context/custody/v1";

struct CollectedKernelContextV1 {
    root_function_identity: [u8; 32],
    kernel_binding: [u8; 32],
    kernel_marker_identity: [u8; 32],
    launch_brand_identity: [u8; 32],
    issuance_identity: [u8; 32],
    physical_argument_count: u32,
    logical_argument_count: u32,
}

#[derive(Debug)]
struct AuthenticatedProductionKernelContextRootV1 {
    selected_root: SemanticFunctionIdV1,
    root_function_identity: [u8; 32],
    kernel_binding: [u8; 32],
    kernel_marker_identity: [u8; 32],
    launch_brand_identity: [u8; 32],
    issuance_identity: [u8; 32],
    physical_argument_count: u32,
    logical_argument_count: u32,
}

struct ProductionKernelContextRootObservationV1 {
    selected_root: SemanticFunctionIdV1,
    root_function_identity: [u8; 32],
    kernel_binding: [u8; 32],
    launch_brand_identity: [u8; 32],
}

/// Move-only custody for compiler-authenticated context inputs. Its private
/// records cannot be built from caller-provided hashes.
pub(crate) struct AuthenticatedProductionKernelContextsV1 {
    frontend_unit_identity: [u8; 32],
    target_brand_identity: [u8; 32],
    expected_roots: Box<[SemanticFunctionIdV1]>,
    roots: Box<[AuthenticatedProductionKernelContextRootV1]>,
    custody_identity: [u8; 32],
}

fn kernel_context_frontend_unit_identity_v1(
    inventory: &AuthenticatedRustcIdentityInventoryV3,
) -> [u8; 32] {
    let mut digest = SemanticIdentityDigestV1::new(KERNEL_CONTEXT_FRONTEND_UNIT_DOMAIN_V1);
    digest.field(&inventory.sha256());
    digest.field(inventory.canonical_transcript());
    digest.finish()
}

fn kernel_context_target_brand_identity_v1(
    target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
) -> [u8; 32] {
    kernel_context_target_brand_identity_from_contract_v1(
        canonical_target_layout_v1(target.rustc_layout()),
        target.contract(),
    )
}

pub(crate) fn kernel_context_target_brand_identity_from_contract_v1(
    layout: SemanticTargetDataLayoutV1,
    contract: crate::production_backend_v1::ProductionBackendTargetContractV1,
) -> [u8; 32] {
    let mut digest = SemanticIdentityDigestV1::new(KERNEL_CONTEXT_TARGET_DOMAIN_V1);
    digest.field(layout.identity().as_bytes());
    digest.field(contract.canonical_target().as_bytes());
    digest.finish()
}

fn kernel_context_launch_brand_identity_v1(launch: &LaunchContract) -> [u8; 32] {
    let mut digest = SemanticIdentityDigestV1::new(KERNEL_CONTEXT_LAUNCH_DOMAIN_V1);
    digest.field(&[launch.rank()]);
    match launch.block_size() {
        BlockSize::Any => digest.field(&[0]),
        BlockSize::Exact(dimensions) => {
            digest.field(&[1]);
            kernel_context_dimensions_v1(&mut digest, dimensions);
        }
        BlockSize::AtMost(dimensions) => {
            digest.field(&[2]);
            kernel_context_dimensions_v1(&mut digest, dimensions);
        }
    }
    kernel_context_dimensions_v1(&mut digest, launch.max_grid());
    digest.field(&launch.static_shared_memory_bytes().to_le_bytes());
    digest.field(&launch.max_dynamic_shared_memory_bytes().to_le_bytes());
    digest.finish()
}

fn kernel_context_dimensions_v1(
    digest: &mut SemanticIdentityDigestV1,
    dimensions: fe2o3_artifacts::Dimensions,
) {
    digest.field(&dimensions.x().to_le_bytes());
    digest.field(&dimensions.y().to_le_bytes());
    digest.field(&dimensions.z().to_le_bytes());
}

fn collect_authenticated_kernel_contexts_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &mut CollectionResult<'tcx>,
) -> Result<Vec<CollectedKernelContextV1>, ProductionSemanticImportErrorV1> {
    let mut contexts = Vec::new();
    for function in &mut collection.functions {
        if function.kernel_context_contract.is_none() {
            continue;
        }
        if !function.is_kernel_entry() {
            return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "a context sidecar is bound to a non-kernel function",
            ));
        }
        let launch = super::rederive_general_typed_launch_for_descriptor_v1(
            function.frontend_contract.as_ref(),
            &function.export_name,
        )
        .map_err(|_| {
            ProductionSemanticImportErrorV1::KernelContextBinding(
                "a context root lacks one exact authenticated launch contract",
            )
        })?;
        let source = function
            .kernel_context_contract
            .as_mut()
            .and_then(|contract| contract.take_authenticated_source())
            .ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
                "a context sidecar reached import without complete collector authentication",
            ))?;
        let observed_root = canonical_function_identities_v1(tcx, function.instance).function();
        if source.root_function_identity() != *observed_root.as_bytes() {
            return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "the authenticated context root identity is stale",
            ));
        }
        let kernel_binding = function.kernel_binding.ok_or(
            ProductionSemanticImportErrorV1::KernelContextBinding(
                "a context root lacks its authenticated kernel binding",
            ),
        )?;
        contexts.push(CollectedKernelContextV1 {
            root_function_identity: source.root_function_identity(),
            kernel_binding: kernel_binding.as_bytes(),
            kernel_marker_identity: source.kernel_marker_identity(),
            launch_brand_identity: kernel_context_launch_brand_identity_v1(&launch),
            issuance_identity: source.issuance_identity(),
            physical_argument_count: source.physical_argument_count(),
            logical_argument_count: source.logical_argument_count(),
        });
    }
    Ok(contexts)
}

fn bind_authenticated_kernel_contexts_v1(
    contexts: Vec<CollectedKernelContextV1>,
    functions: &[RetainedSemanticFunctionProducerV1<'_>],
    semantic_roots: &[SemanticFunctionIdV1],
    inventory: &AuthenticatedRustcIdentityInventoryV3,
    target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
) -> Result<AuthenticatedProductionKernelContextsV1, ProductionSemanticImportErrorV1> {
    let frontend_unit_identity = kernel_context_frontend_unit_identity_v1(inventory);
    let target_brand_identity = kernel_context_target_brand_identity_v1(target);
    let mut roots = Vec::with_capacity(contexts.len());
    for context in contexts {
        let selected_root = functions
            .iter()
            .position(|function| {
                function.identities.function().as_bytes() == &context.root_function_identity
            })
            .and_then(|index| u32::try_from(index).ok())
            .map(SemanticFunctionIdV1::from_index)
            .filter(|root| semantic_roots.contains(root))
            .ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
                "an authenticated context is not bound to one semantic kernel root",
            ))?;
        roots.push(AuthenticatedProductionKernelContextRootV1 {
            selected_root,
            root_function_identity: context.root_function_identity,
            kernel_binding: context.kernel_binding,
            kernel_marker_identity: context.kernel_marker_identity,
            launch_brand_identity: context.launch_brand_identity,
            issuance_identity: context.issuance_identity,
            physical_argument_count: context.physical_argument_count,
            logical_argument_count: context.logical_argument_count,
        });
    }
    roots.sort_unstable_by_key(|root| root.selected_root);
    if roots
        .windows(2)
        .any(|pair| pair[0].selected_root == pair[1].selected_root)
    {
        return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
            "two authenticated contexts select the same semantic root",
        ));
    }
    let expected_roots = roots
        .iter()
        .map(|root| root.selected_root)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let custody_identity = kernel_context_custody_identity_v1(
        frontend_unit_identity,
        target_brand_identity,
        &expected_roots,
        &roots,
    );
    Ok(AuthenticatedProductionKernelContextsV1 {
        frontend_unit_identity,
        target_brand_identity,
        expected_roots,
        roots: roots.into_boxed_slice(),
        custody_identity,
    })
}

fn kernel_context_custody_identity_v1(
    frontend_unit_identity: [u8; 32],
    target_brand_identity: [u8; 32],
    expected_roots: &[SemanticFunctionIdV1],
    roots: &[AuthenticatedProductionKernelContextRootV1],
) -> [u8; 32] {
    let mut digest = SemanticIdentityDigestV1::new(KERNEL_CONTEXT_CUSTODY_DOMAIN_V1);
    digest.field(&frontend_unit_identity);
    digest.field(&target_brand_identity);
    for root in expected_roots {
        digest.field(&root.index().to_le_bytes());
    }
    for root in roots {
        digest.field(&root.selected_root.index().to_le_bytes());
        digest.field(&root.root_function_identity);
        digest.field(&root.kernel_binding);
        digest.field(&root.kernel_marker_identity);
        digest.field(&root.launch_brand_identity);
        digest.field(&root.issuance_identity);
        digest.field(&root.physical_argument_count.to_le_bytes());
        digest.field(&root.logical_argument_count.to_le_bytes());
    }
    digest.finish()
}

impl AuthenticatedProductionKernelContextsV1 {
    fn validate_carriage(
        &self,
        frontend_unit_identity: [u8; 32],
        target_brand_identity: [u8; 32],
        observed: &[ProductionKernelContextRootObservationV1],
    ) -> Result<(), ProductionSemanticImportErrorV1> {
        if self.frontend_unit_identity != frontend_unit_identity {
            return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "context custody belongs to a different frontend compilation unit",
            ));
        }
        if self.target_brand_identity != target_brand_identity {
            return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "context custody belongs to a different selected target",
            ));
        }
        let carried_roots = self
            .roots
            .iter()
            .map(|root| root.selected_root)
            .collect::<Vec<_>>();
        if carried_roots.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "context custody contains duplicate or noncanonical roots",
            ));
        }
        if carried_roots.as_slice() != self.expected_roots.as_ref() {
            return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "context custody is missing, duplicated, or reordered",
            ));
        }
        if kernel_context_custody_identity_v1(
            self.frontend_unit_identity,
            self.target_brand_identity,
            &self.expected_roots,
            &self.roots,
        ) != self.custody_identity
        {
            return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "context custody identity is stale",
            ));
        }
        for root in &self.roots {
            if [
                self.frontend_unit_identity,
                self.target_brand_identity,
                root.kernel_marker_identity,
                root.launch_brand_identity,
                root.issuance_identity,
            ]
            .contains(&[0; 32])
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "context custody contains an incomplete identity",
                ));
            }
            let current = observed
                .iter()
                .find(|current| current.selected_root == root.selected_root)
                .ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "context custody names a different semantic root roster",
                ))?;
            if current.root_function_identity != root.root_function_identity
                || current.kernel_binding != root.kernel_binding
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "context custody was substituted across kernel roots",
                ));
            }
            if current.launch_brand_identity != root.launch_brand_identity {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "context custody was substituted across launch contracts",
                ));
            }
            if root
                .logical_argument_count
                .checked_sub(root.physical_argument_count)
                != Some(1)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "logical context changed the physical kernel argument count",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn into_lowering_inputs(
        self,
        inventory: &AuthenticatedRustcIdentityInventoryV3,
        target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
        ranked_roots: &[crate::production_ranked_projection_v1::ProductionRankedRootProgramV1],
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    ) -> Result<
        Vec<fe2o3_lower_mir_kernel::ProductionKernelContextLoweringInputV1>,
        ProductionSemanticImportErrorV1,
    > {
        if ranked_roots.len() != typed_roots.len() {
            return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "ranked and typed root rosters differ",
            ));
        }
        let observed = ranked_roots
            .iter()
            .zip(typed_roots)
            .map(|(ranked, typed)| {
                if ranked.kernel_binding() != &typed.kernel_binding_bytes() {
                    return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                        "ranked and typed roots have different kernel bindings",
                    ));
                }
                let launch = typed.source_launch().ok_or(
                    ProductionSemanticImportErrorV1::KernelContextBinding(
                        "context root lost its exact launch contract",
                    ),
                )?;
                Ok(ProductionKernelContextRootObservationV1 {
                    selected_root: ranked.semantic_root(),
                    root_function_identity: *ranked.semantic_root_identity().as_bytes(),
                    kernel_binding: *ranked.kernel_binding(),
                    launch_brand_identity: kernel_context_launch_brand_identity_v1(launch),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.validate_carriage(
            kernel_context_frontend_unit_identity_v1(inventory),
            kernel_context_target_brand_identity_v1(target),
            &observed,
        )?;

        let mut inputs = Vec::with_capacity(self.roots.len());
        for root in self.roots {
            inputs.push(
                fe2o3_lower_mir_kernel::ProductionKernelContextLoweringInputV1::new(
                    root.selected_root,
                    self.frontend_unit_identity,
                    root.kernel_marker_identity,
                    self.target_brand_identity,
                    root.launch_brand_identity,
                    root.issuance_identity,
                ),
            );
        }
        Ok(inputs)
    }
}

pub(crate) fn construct_production_semantic_mir_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
    debug_source_capture: DebugSourceCaptureRequestV2,
) -> Result<ConstructedProductionSemanticMirV1, ProductionSemanticImportErrorV1> {
    let AuthenticatedCollectedKernelClosureV1 {
        target,
        mut collection,
        roots,
    } = closure;
    let target = match target.authenticate_import_session(tcx) {
        Ok(target) => target,
        Err(error) => return Err(ProductionSemanticImportErrorV1::Target(error)),
    };
    let retained_roots = roots
        .iter()
        .map(|root| (root.instance, root.role, root.export_name.as_str()));
    let independently_observed_roots = collection
        .functions
        .iter()
        .filter(|function| {
            matches!(
                function.role,
                super::CollectedFunctionRole::KernelEntry
                    | super::CollectedFunctionRole::DeviceFfiExport
            )
        })
        .map(|function| {
            (
                function.instance,
                function.role,
                function.export_name.as_str(),
            )
        });
    if !exact_ordered_axes_match(retained_roots, independently_observed_roots) {
        return Err(ProductionSemanticImportErrorV1::RootCustodyMismatch);
    }
    let collected_kernel_contexts = collect_authenticated_kernel_contexts_v1(tcx, &mut collection)?;
    let reference_effect_bindings =
        crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::new(
            collection
                .functions
                .iter()
                .filter_map(|function| function.reference_effect_binding.clone())
                .collect(),
        );
    let authenticated_closure_types = collection
        .functions
        .iter()
        .filter_map(|function| function.closure_plan.as_ref())
        .flat_map(|plan| plan.authenticated_closure_type_identities())
        .map(SemanticTypeIdentityV1::from_sha256)
        .collect::<BTreeSet<_>>();
    let identity_inventory = build_identity_inventory_v1(tcx, &target, &collection, &roots)?;
    require_lineage_transcript_bound_v3(
        "rustc identity inventory",
        &identity_inventory.canonical_transcript,
    )?;

    let ProductionSemanticIdentityInventoryV1 {
        functions,
        roots,
        sha256: rustc_identity_inventory_sha256,
        canonical_transcript: rustc_identity_inventory_transcript,
    } = identity_inventory;
    let rustc_identity_inventory = AuthenticatedRustcIdentityInventoryV3 {
        sha256: rustc_identity_inventory_sha256,
        canonical_transcript: rustc_identity_inventory_transcript,
    };
    let kernel_contexts = bind_authenticated_kernel_contexts_v1(
        collected_kernel_contexts,
        &functions,
        &roots,
        &rustc_identity_inventory,
        &target,
    )?;
    let plan = match build_production_semantic_preflight_plan_v1(
        tcx,
        canonical_target_layout_v1(target.rustc_layout()),
        functions,
        roots,
        rustc_identity_inventory_sha256,
        &authenticated_closure_types,
        debug_source_capture,
    ) {
        Ok(plan) => plan,
        Err(error) => return Err(ProductionSemanticImportErrorV1::Preflight(Box::new(error))),
    };
    require_lineage_transcript_bound_v3("rustc preflight plan", plan.canonical_transcript())?;
    let semantic_types = match construct_production_semantic_types_v1(tcx, plan.type_producers()) {
        Ok(types) => types,
        Err(error) => {
            return Err(ProductionSemanticImportErrorV1::TypeConstruction(Box::new(
                error,
            )));
        }
    };
    let semantic_function_abis = match construct_production_semantic_fn_abis_v1(
        tcx,
        plan.function_abi_producers(),
        plan.type_producers(),
    ) {
        Ok(abis) => abis,
        Err(error) => {
            return Err(ProductionSemanticImportErrorV1::FunctionAbiConstruction(
                Box::new(error),
            ));
        }
    };
    let terminal_abi_producers = plan
        .terminal_producers()
        .iter()
        .map(|terminal| terminal.abi.clone())
        .collect::<Vec<_>>();
    let semantic_terminal_abis = match construct_production_semantic_fn_abis_v1(
        tcx,
        &terminal_abi_producers,
        plan.type_producers(),
    ) {
        Ok(abis) => abis,
        Err(error) => {
            return Err(ProductionSemanticImportErrorV1::FunctionAbiConstruction(
                Box::new(error),
            ));
        }
    };
    let semantic_mir = construct_complete_request_v1(
        tcx,
        canonical_target_layout_v1(target.rustc_layout()),
        &plan,
        &kernel_contexts,
        semantic_types.into_records(),
        semantic_function_abis,
        semantic_terminal_abis,
    )?;
    let (
        rustc_preflight_plan_sha256,
        rustc_preflight_plan_transcript,
        debug_source_files,
        debug_source_scopes,
        debug_source_variables,
        debug_capture_gap,
    ) = plan
        .into_identity_transcript_and_debug_files()
        .map_err(|error| ProductionSemanticImportErrorV1::Preflight(Box::new(error)))?;
    drop(collection);
    Ok(ConstructedProductionSemanticMirV1 {
        semantic_mir,
        rustc_identity_inventory,
        rustc_preflight_plan: AuthenticatedRustcPreflightPlanV3 {
            sha256: rustc_preflight_plan_sha256,
            rustc_identity_inventory_sha256,
            canonical_transcript: rustc_preflight_plan_transcript,
        },
        rustc_target: target,
        kernel_contexts,
        reference_effect_bindings,
        debug_source_files,
        debug_source_scopes,
        debug_source_variables,
        debug_capture_gap,
    })
}

fn require_lineage_transcript_bound_v3(
    field: &'static str,
    transcript: &[u8],
) -> Result<(), ProductionSemanticImportErrorV1> {
    if transcript.len() > fe2o3_compiler_lineage::MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 {
        Err(ProductionSemanticImportErrorV1::LineageTranscriptTooLarge {
            field,
            actual: transcript.len(),
            maximum: fe2o3_compiler_lineage::MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
        })
    } else {
        Ok(())
    }
}

fn semantic_function_path_v1(
    root: SemanticFunctionIdV1,
    target: SemanticFunctionIdV1,
    edges: &[(SemanticFunctionIdV1, SemanticFunctionIdV1)],
) -> Option<Vec<SemanticFunctionIdV1>> {
    let mut pending = VecDeque::from([(root, vec![root])]);
    let mut visited = BTreeSet::new();
    while let Some((current, path)) = pending.pop_front() {
        if current == target {
            return Some(path);
        }
        if !visited.insert(current) {
            continue;
        }
        let mut successors = edges
            .iter()
            .filter_map(|(caller, callee)| (*caller == current).then_some(*callee))
            .collect::<Vec<_>>();
        successors.sort_unstable();
        successors.dedup();
        for successor in successors {
            let mut next_path = path.clone();
            next_path.push(successor);
            pending.push_back((successor, next_path));
        }
    }
    None
}

fn capability_terminal_rejection_v1(
    tcx: TyCtxt<'_>,
    plan: &ProductionSemanticPreflightPlanV1<'_>,
    terminal_index: u32,
    root: Option<&AuthenticatedProductionKernelContextRootV1>,
    stage: &'static str,
    error: ProductionSemanticImportErrorV1,
) -> ProductionSemanticImportErrorV1 {
    let (root, span, helper_chain) = capability_terminal_site_v1(tcx, plan, terminal_index, root);
    ProductionSemanticImportErrorV1::CapabilityTerminalRejected {
        root,
        span,
        helper_chain,
        stage,
        detail: error.to_string(),
    }
}

fn capability_terminal_site_v1(
    tcx: TyCtxt<'_>,
    plan: &ProductionSemanticPreflightPlanV1<'_>,
    terminal_index: u32,
    root: Option<&AuthenticatedProductionKernelContextRootV1>,
) -> (String, String, String) {
    let recipes = plan
        .terminal_expansion_producers()
        .iter()
        .filter(|recipe| recipe.terminal == terminal_index)
        .collect::<Vec<_>>();
    let mut spans = recipes
        .iter()
        .map(|recipe| tcx.sess.source_map().span_to_diagnostic_string(recipe.span))
        .collect::<Vec<_>>();
    spans.sort();
    spans.dedup();
    let edges = plan
        .direct_call_producers()
        .iter()
        .map(|edge| (edge.caller, edge.callee))
        .collect::<Vec<_>>();
    let identity = |function: SemanticFunctionIdV1| {
        plan.function_producers()
            .get(function.index() as usize)
            .map(|producer| crate::encode_hex(producer.identities.function().as_bytes()))
            .unwrap_or_else(|| format!("function#{}", function.index()))
    };
    let terminal_identity = plan
        .terminal_producers()
        .get(terminal_index as usize)
        .map(|terminal| crate::encode_hex(terminal.identities.function().as_bytes()))
        .unwrap_or_else(|| format!("terminal#{terminal_index}"));
    let mut chains = recipes
        .iter()
        .map(|recipe| {
            let functions = root
                .and_then(|root| {
                    semantic_function_path_v1(root.selected_root, recipe.caller, &edges)
                })
                .unwrap_or_else(|| vec![recipe.caller]);
            let mut chain = functions.into_iter().map(identity).collect::<Vec<_>>();
            chain.push(terminal_identity.clone());
            chain.join("->")
        })
        .collect::<Vec<_>>();
    chains.sort();
    chains.dedup();
    (
        root.map(|root| crate::encode_hex(&root.root_function_identity))
            .unwrap_or_else(|| "unresolved".to_owned()),
        if spans.is_empty() {
            "unresolved".to_owned()
        } else {
            spans.join(",")
        },
        if chains.is_empty() {
            "unresolved".to_owned()
        } else {
            chains.join("|")
        },
    )
}

fn construct_complete_request_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    target: SemanticTargetDataLayoutV1,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    kernel_contexts: &AuthenticatedProductionKernelContextsV1,
    types: Vec<SemanticTypeDeclV1>,
    function_abis: ConstructedSemanticFunctionAbisV1,
    terminal_abis: ConstructedSemanticFunctionAbisV1,
) -> Result<AdmittedInertSemanticMirV1, ProductionSemanticImportErrorV1> {
    let function_abis = function_abis.into_records();
    let terminal_abis = terminal_abis.into_records();
    if function_abis.len() != plan.function_producers().len() {
        return Err(body_owner_table_mismatch_v1(
            "function ABI producer cardinality",
        ));
    }
    if terminal_abis.len() != plan.terminal_producers().len() {
        return Err(body_owner_table_mismatch_v1(
            "terminal ABI producer cardinality",
        ));
    }
    if plan.body_producers().len() != plan.function_producers().len() {
        return Err(body_owner_table_mismatch_v1(
            "function body producer cardinality",
        ));
    }

    let type_bindings = plan
        .type_producers()
        .iter()
        .enumerate()
        .map(|(index, producer)| {
            let index = u32::try_from(index)
                .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?;
            Ok(ProductionSemanticTypeBindingV1::new(
                producer.ty,
                fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1::from_index(index),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let function_count = u32::try_from(plan.function_producers().len())
        .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?;
    let mut body_owner = build_body_request_owner_v1(plan, types.len(), function_count)?;
    let mut callables = (0..function_count)
        .map(|index| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index)))
        .collect::<Vec<_>>();
    for (index, (terminal, abi)) in plan
        .terminal_producers()
        .iter()
        .zip(&terminal_abis)
        .enumerate()
    {
        let terminal_index = u32::try_from(index)
            .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?;
        let capability_root = capability_memory_root_for_terminal_v1(
            plan,
            kernel_contexts,
            terminal_index,
            terminal.expansion,
        )
        .map_err(|error| {
            if matches!(
                terminal.expansion,
                crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1::Execution(_)
            ) {
                capability_terminal_rejection_v1(
                    tcx,
                    plan,
                    terminal_index,
                    None,
                    "root-authentication",
                    error,
                )
            } else {
                error
            }
        })?;
        let operation = terminal_operation_v1(
            tcx,
            terminal.instance,
            terminal.expansion,
            abi,
            &types,
            capability_root,
            terminal.identities.function(),
            kernel_contexts,
        )
        .map_err(|error| {
            if matches!(
                terminal.expansion,
                crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1::Execution(_)
            ) {
                capability_terminal_rejection_v1(
                    tcx,
                    plan,
                    terminal_index,
                    capability_root,
                    "terminal-authentication",
                    error,
                )
            } else {
                error
            }
        })?;
        let mut digest = SemanticIdentityDigestV1::new(PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V4);
        digest.field(terminal.identities.function().as_bytes());
        digest.field(abi.identity().as_bytes());
        digest.field(&[terminal_operation_tag_for_schema_v1(
            terminal.expansion,
            TerminalIdentitySchemaV1::CombinedV4,
        )]);
        digest.field(
            &u32::try_from(index)
                .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?
                .to_le_bytes(),
        );
        callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                terminal.identities.function(),
                terminal.identities.item_definition(),
                terminal.identities.monomorphization(),
                terminal.identities.generic_type_arguments(),
                terminal.identities.const_generic_arguments(),
                terminal.source.provenance,
                abi.clone(),
            ),
            operation,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(digest.finish()),
        });
    }

    let mut functions = Vec::new();
    functions
        .try_reserve_exact(plan.function_producers().len())
        .map_err(|_| ProductionSemanticImportErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Functions,
            actual: u64::try_from(plan.function_producers().len()).unwrap_or(u64::MAX),
            maximum: HARD_MAX_FUNCTIONS_V1,
        })?;
    for (index, ((function, body), abi)) in plan
        .function_producers()
        .iter()
        .zip(plan.body_producers())
        .zip(function_abis)
        .enumerate()
    {
        let function_id = SemanticFunctionIdV1::from_index(
            u32::try_from(index)
                .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?,
        );
        if body.function != function_id {
            return Err(body_owner_table_mismatch_v1("function body owner ordering"));
        }
        let local_bindings = body
            .locals
            .iter()
            .enumerate()
            .map(|(semantic, local)| {
                Ok(ProductionSemanticLocalBindingV1::new(
                    local.rustc_local,
                    fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1::from_index(
                        u32::try_from(semantic)
                            .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?,
                    ),
                    local.identity,
                    local.source.provenance,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let block_bindings = body
            .blocks
            .iter()
            .enumerate()
            .map(|(semantic, block)| {
                Ok(ProductionSemanticBlockBindingV1::new(
                    block.rustc_block,
                    fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1::from_index(
                        u32::try_from(semantic)
                            .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?,
                    ),
                    block.identity,
                    block.source.provenance,
                    block
                        .statements
                        .iter()
                        .map(|source| source.provenance)
                        .collect(),
                    block.terminator.provenance,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let direct_calls = plan
            .direct_call_producers()
            .iter()
            .filter(|call| call.caller == function_id)
            .map(|call| {
                let callee = plan
                    .function_producers()
                    .get(call.callee.index() as usize)
                    .ok_or(ProductionSemanticImportErrorV1::RootIdentityMismatch)?;
                Ok(ProductionSemanticDirectCallBindingV1::new(
                    call.caller,
                    call.block,
                    callee.instance,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let terminal_expansions = plan
            .terminal_expansion_producers()
            .iter()
            .filter(|recipe| recipe.caller == function_id)
            .map(|recipe| {
                Ok(ProductionSemanticTerminalExpansionRecipeV1::new(
                    recipe.caller,
                    recipe.block,
                    recipe.instance,
                    recipe.expansion,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let normalized_intrinsics = plan
            .normalized_intrinsic_producers()
            .iter()
            .filter(|recipe| recipe.caller == function_id)
            .map(|recipe| {
                ProductionSemanticNormalizedRustcIntrinsicRecipeV1::new(
                    recipe.caller,
                    recipe.block,
                    recipe.instance,
                    recipe.element_type,
                    recipe.operation,
                )
            })
            .collect::<Vec<_>>();
        functions.push(
            construct_production_semantic_body_v1(
                ProductionSemanticBodyInputV1 {
                    tcx,
                    instance: function.instance,
                    body: tcx.instance_mir(function.instance.def),
                    function: function_id,
                    identities: ProductionSemanticFunctionIdentitiesV1::new(
                        function.identities.function(),
                        function.identities.item_definition(),
                        function.identities.monomorphization(),
                        function.identities.generic_type_arguments(),
                        function.identities.const_generic_arguments(),
                    ),
                    role: semantic_function_role_v1(function.role),
                    export: semantic_function_export_v1(function)?,
                    source: body.source.provenance,
                    abi,
                    type_bindings: &type_bindings,
                    local_bindings: &local_bindings,
                    block_bindings: &block_bindings,
                    entry: body.entry,
                    direct_calls: &direct_calls,
                    terminal_expansions: &terminal_expansions,
                    normalized_intrinsics: &normalized_intrinsics,
                },
                &mut body_owner,
            )
            .map_err(|error| ProductionSemanticImportErrorV1::BodyConstruction(Box::new(error)))?,
        );
    }

    let semantic_mir = InertSemanticMirRequestV1::new_with_callables(
        target,
        types,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        functions,
        callables,
        plan.roots().to_vec(),
    )
    .and_then(|request| request.admit_current_production(SemanticMirLimitsV1::default()))
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    validate_execution_terminal_carriage_v1(tcx, plan, kernel_contexts, &semantic_mir)?;
    Ok(semantic_mir)
}

fn validate_execution_terminal_carriage_v1(
    tcx: TyCtxt<'_>,
    plan: &ProductionSemanticPreflightPlanV1<'_>,
    kernel_contexts: &AuthenticatedProductionKernelContextsV1,
    semantic_mir: &AdmittedInertSemanticMirV1,
) -> Result<(), ProductionSemanticImportErrorV1> {
    use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion;

    let function_count = plan.function_producers().len();
    let mut execution_records = 0_usize;
    let mut roster_mask = 0_u64;
    for (terminal_index, terminal) in plan.terminal_producers().iter().enumerate() {
        let Expansion::Execution(execution) = terminal.expansion else {
            continue;
        };
        let callable = semantic_mir
            .callables()
            .get(function_count + terminal_index)
            .ok_or_else(|| body_owner_table_mismatch_v1("execution terminal callable index"))?;
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        } = callable
        else {
            return Err(body_owner_table_mismatch_v1(
                "execution terminal must be a V17 ExecutionCapability callable",
            ));
        };
        if semantic_mir.wire_version() != SemanticMirWireVersionV1::V17
            || binding.identity() != terminal.identities.function()
            || binding.abi().identity() != terminal.abi.identity
            || contract.source_identity() != terminal.identities.function()
        {
            return Err(body_owner_table_mismatch_v1(
                "execution terminal V17 identity carriage",
            ));
        }

        execution_records += 1;
        roster_mask |= 1_u64 << execution.identity_tag();
        if !crate::env_flag(crate::VERBOSE_ENV) {
            continue;
        }

        let terminal_index = u32::try_from(terminal_index)
            .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?;
        let root = capability_memory_root_for_terminal_v1(
            plan,
            kernel_contexts,
            terminal_index,
            terminal.expansion,
        )?;
        let (root, span, helper_chain) =
            capability_terminal_site_v1(tcx, plan, terminal_index, root);
        let def_id = terminal.instance.def_id();
        eprintln!(
            "[FE2O3-CAP-AUDIT001] root={root} span={span} helper_chain={helper_chain} stage=semantic-mir-v17 terminal={execution:?} diagnostic={:?} provider_crate={} provider={} provider_item_sha256={} monomorphization_sha256={} generic_types_sha256={} const_generics_sha256={} fn_abi_sha256={} source={:?} signature={:?} workgroup_brand={:?} epoch_before={:?} epoch_after={:?} obligations=0x{:04x} operation={:?} trap=false",
            execution.trusted_device_item(),
            tcx.crate_name(def_id.krate),
            tcx.def_path_str(def_id),
            crate::encode_hex(terminal.identities.item_definition().as_bytes()),
            crate::encode_hex(terminal.identities.monomorphization().as_bytes()),
            crate::encode_hex(terminal.identities.generic_type_arguments().as_bytes()),
            crate::encode_hex(terminal.identities.const_generic_arguments().as_bytes()),
            crate::encode_hex(binding.abi().identity().as_bytes()),
            terminal.source.provenance,
            contract.signature(),
            contract.workgroup_brand(),
            contract.epoch_before(),
            contract.epoch_after(),
            contract.obligations().bits(),
            contract.operation(),
        );
    }
    if crate::env_flag(crate::VERBOSE_ENV) {
        eprintln!(
            "[FE2O3-CAP-AUDIT002] root=authenticated-set span=authenticated-set helper_chain=authenticated-closure stage=semantic-mir-v17 wire={} execution_records={execution_records} roster_mask=0x{roster_mask:010x} trap_records=0",
            semantic_mir.wire_version().as_u16(),
        );
    }
    Ok(())
}

fn build_body_request_owner_v1<'tcx>(
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    type_count: usize,
    function_count: u32,
) -> Result<ProductionSemanticBodyRequestOwnerV1<'tcx>, ProductionSemanticImportErrorV1> {
    let callable_count = plan
        .function_producers()
        .len()
        .checked_add(plan.terminal_producers().len())
        .ok_or(ProductionSemanticImportErrorV1::RootIdentityMismatch)?;
    let mut entries = Vec::new();
    entries.try_reserve_exact(callable_count).map_err(|_| {
        ProductionSemanticImportErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Callables,
            actual: u64::try_from(callable_count).unwrap_or(u64::MAX),
            maximum: SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::Callables),
        }
    })?;
    for (index, function) in plan.function_producers().iter().enumerate() {
        let callable = u32::try_from(index)
            .map(SemanticCallableIdV1::from_index)
            .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?;
        entries.push(ProductionSemanticCallableOwnerEntryV1::defined(
            function.instance,
            callable,
        ));
    }

    let mut terminal_instances = Vec::new();
    terminal_instances
        .try_reserve_exact(plan.terminal_producers().len())
        .map_err(|_| ProductionSemanticImportErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::Callables,
            actual: u64::try_from(callable_count).unwrap_or(u64::MAX),
            maximum: SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::Callables),
        })?;
    terminal_instances.resize(plan.terminal_producers().len(), None);
    for recipe in plan.terminal_expansion_producers() {
        let slot = terminal_instances
            .get_mut(recipe.terminal as usize)
            .ok_or_else(|| body_owner_table_mismatch_v1("terminal callable owner index"))?;
        let observed = (recipe.instance, recipe.expansion);
        match slot {
            Some(previous) if *previous != observed => {
                return Err(body_owner_table_mismatch_v1(
                    "terminal callable owner instance",
                ));
            }
            Some(_) => {}
            None => *slot = Some(observed),
        }
    }
    for (index, (terminal, observed)) in plan
        .terminal_producers()
        .iter()
        .zip(terminal_instances)
        .enumerate()
    {
        let (instance, expansion) = observed
            .ok_or_else(|| body_owner_table_mismatch_v1("terminal callable owner completeness"))?;
        if expansion != terminal.expansion {
            return Err(body_owner_table_mismatch_v1(
                "terminal callable owner expansion",
            ));
        }
        let terminal_index = u32::try_from(index)
            .map_err(|_| ProductionSemanticImportErrorV1::RootIdentityMismatch)?;
        let callable = function_count
            .checked_add(terminal_index)
            .map(SemanticCallableIdV1::from_index)
            .ok_or(ProductionSemanticImportErrorV1::RootIdentityMismatch)?;
        entries.push(ProductionSemanticCallableOwnerEntryV1::terminal(
            instance, expansion, callable,
        ));
    }

    ProductionSemanticBodyRequestOwnerV1::new(SemanticMirLimitsV1::default(), type_count, &entries)
        .map_err(|error| ProductionSemanticImportErrorV1::BodyConstruction(Box::new(error)))
}

fn body_owner_table_mismatch_v1(table: &'static str) -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::BodyConstruction(Box::new(
        ProductionSemanticBodyErrorV1::IdentityTableMismatch { table },
    ))
}

const fn semantic_function_role_v1(role: CollectedFunctionRole) -> SemanticFunctionRoleV1 {
    match role {
        CollectedFunctionRole::KernelEntry => SemanticFunctionRoleV1::KernelRoot,
        CollectedFunctionRole::InternalHelper => SemanticFunctionRoleV1::InternalHelper,
        CollectedFunctionRole::DeviceFfiExport => SemanticFunctionRoleV1::DeviceFfiExport,
    }
}

fn semantic_function_export_v1(
    function: &RetainedSemanticFunctionProducerV1<'_>,
) -> Result<ProductionSemanticFunctionExportV1, ProductionSemanticImportErrorV1> {
    match function.role {
        CollectedFunctionRole::InternalHelper
            if function.export_name.is_none()
                && function.kernel_binding.is_none()
                && function.frontend_contract.is_none() =>
        {
            Ok(ProductionSemanticFunctionExportV1::None)
        }
        CollectedFunctionRole::DeviceFfiExport
            if function.kernel_binding.is_none() && function.frontend_contract.is_none() =>
        {
            Ok(ProductionSemanticFunctionExportV1::DeviceFfi(
                semantic_link_symbol_v1(function.export_name.as_deref())?,
            ))
        }
        CollectedFunctionRole::KernelEntry => {
            let binding = function
                .kernel_binding
                .ok_or_else(|| body_owner_table_mismatch_v1("kernel binding identity"))?;
            Ok(ProductionSemanticFunctionExportV1::Kernel(
                SemanticKernelEntryV1::new(
                    semantic_link_symbol_v1(function.export_name.as_deref())?,
                    SemanticKernelBindingIdentityV1::from_sha256(binding.as_bytes()),
                    semantic_kernel_source_contract_v1(function.frontend_contract.as_ref())?,
                ),
            ))
        }
        CollectedFunctionRole::InternalHelper | CollectedFunctionRole::DeviceFfiExport => {
            Err(body_owner_table_mismatch_v1("function export metadata"))
        }
    }
}

fn semantic_link_symbol_v1(
    symbol: Option<&str>,
) -> Result<SemanticLinkSymbolV1, ProductionSemanticImportErrorV1> {
    SemanticLinkSymbolV1::new(
        symbol
            .ok_or_else(|| body_owner_table_mismatch_v1("function export symbol"))?
            .as_bytes()
            .to_vec(),
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)
}

fn semantic_kernel_source_contract_v1(
    authenticated: Option<&super::AuthenticatedKernelFrontendContractV1>,
) -> Result<SemanticKernelSourceContractV1, ProductionSemanticImportErrorV1> {
    let Some(authenticated) = authenticated else {
        return SemanticKernelSourceContractV1::new(None, None, None)
            .map_err(ProductionSemanticImportErrorV1::SemanticSchema);
    };
    let frontend = authenticated.contract();
    let launch = frontend
        .launch()
        .map(|launch| {
            SemanticKernelLaunchBoundsV1::new(
                launch
                    .required()
                    .map(|dimensions| SemanticWorkgroupDimensionsV1::new(dimensions.as_array()))
                    .transpose()?,
                launch
                    .maximum()
                    .map(|dimensions| SemanticWorkgroupDimensionsV1::new(dimensions.as_array()))
                    .transpose()?,
                launch.min_workgroups_per_compute_unit(),
            )
        })
        .transpose()
        .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    let resources = authenticated
        .resource_contract()
        .map(|resources| {
            SemanticKernelResourceContractV1::new(
                resources.static_shared_memory_bytes(),
                resources.max_dynamic_shared_memory_bytes(),
            )
        })
        .transpose()
        .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    let unsafe_assembly = frontend
        .unsafe_assembly()
        .map(|assembly| {
            SemanticUnsafeAssemblyDeclarationV1::new(
                match assembly.target() {
                    fe2o3_rustc_front::FrontendUnsafeAssemblyTargetV1::AmdGpuGfx942 => {
                        SemanticUnsafeAssemblyTargetV1::AmdGpuGfx942
                    }
                },
                assembly.operand_bits(),
                assembly.option_bits(),
                assembly.effect_bits(),
            )
        })
        .transpose()
        .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    let reachable_assembly = unsafe_assembly
        .map(|assembly| {
            let reachable = authenticated.reachable_assembly();
            SemanticReachableAssemblyV1::new(
                reachable.blocks(),
                reachable.operand_bits(),
                reachable.option_bits(),
                assembly.effect_bits(),
            )
        })
        .transpose()
        .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    SemanticKernelSourceContractV1::new_with_resources(
        launch,
        resources,
        unsafe_assembly,
        reachable_assembly,
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)
}

fn capability_memory_root_for_terminal_v1<'a>(
    plan: &ProductionSemanticPreflightPlanV1<'_>,
    contexts: &'a AuthenticatedProductionKernelContextsV1,
    terminal: u32,
    expansion: crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1,
) -> Result<Option<&'a AuthenticatedProductionKernelContextRootV1>, ProductionSemanticImportErrorV1>
{
    use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion;
    let is_capability_terminal = matches!(
        expansion,
        Expansion::CapabilityGlobalBindReadOnly
            | Expansion::CapabilityGlobalBindDisjointWrite
            | Expansion::CapabilityGlobalLoad
            | Expansion::CapabilityGlobalStore
            | Expansion::Invocation3DIndex1D
            | Expansion::Execution(_)
    );
    if !is_capability_terminal {
        return Ok(None);
    }

    let callers = plan
        .terminal_expansion_producers()
        .iter()
        .filter(|recipe| recipe.terminal == terminal && recipe.expansion == expansion)
        .map(|recipe| recipe.caller)
        .collect::<BTreeSet<_>>();
    if callers.is_empty() {
        return Err(body_owner_table_mismatch_v1(
            "capability terminal call ownership",
        ));
    }

    let edges = plan
        .direct_call_producers()
        .iter()
        .map(|edge| (edge.caller, edge.callee))
        .collect::<Vec<_>>();
    authenticate_capability_memory_root_v1(
        contexts,
        &callers,
        &edges,
        matches!(
            expansion,
            Expansion::CapabilityGlobalBindReadOnly
                | Expansion::CapabilityGlobalBindDisjointWrite
                | Expansion::Execution(
                    crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1::BindAtomicView
                )
                | Expansion::Execution(
                    crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1::GlobalBindExclusiveReadWrite
                )
        ),
    )
    .map(Some)
}

fn authenticate_capability_memory_root_v1<'a>(
    contexts: &'a AuthenticatedProductionKernelContextsV1,
    callers: &BTreeSet<SemanticFunctionIdV1>,
    edges: &[(SemanticFunctionIdV1, SemanticFunctionIdV1)],
    bind_must_be_root: bool,
) -> Result<&'a AuthenticatedProductionKernelContextRootV1, ProductionSemanticImportErrorV1> {
    let mut selected = None;
    for caller in callers.iter().copied() {
        let roots = contexts
            .roots
            .iter()
            .filter(|root| semantic_function_reaches_v1(root.selected_root, caller, edges))
            .collect::<Vec<_>>();
        let [root] = roots.as_slice() else {
            return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "typed-global terminal is not owned by exactly one authenticated kernel root",
            ));
        };
        if bind_must_be_root && caller != root.selected_root {
            return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "typed-global binding is not issued directly by its physical kernel root",
            ));
        }
        match selected {
            None => selected = Some(*root),
            Some(previous) if std::ptr::eq(previous, *root) => {}
            Some(_) => {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global terminal is shared across authenticated kernel roots",
                ));
            }
        }
    }
    selected.ok_or_else(|| body_owner_table_mismatch_v1("typed-global terminal call ownership"))
}

fn semantic_function_reaches_v1(
    root: SemanticFunctionIdV1,
    target: SemanticFunctionIdV1,
    edges: &[(SemanticFunctionIdV1, SemanticFunctionIdV1)],
) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if current == target {
            return true;
        }
        if !visited.insert(current) {
            continue;
        }
        pending.extend(
            edges
                .iter()
                .filter_map(|(caller, callee)| (*caller == current).then_some(*callee)),
        );
    }
    false
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RustCapabilityMemoryRoleV1<'tcx> {
    ReadOnly,
    ExclusiveReadWrite,
    DisjointWrite {
        index_space: Ty<'tcx>,
        mapping: SemanticDisjointIndexSpaceV1,
    },
    AtomicReadWrite {
        scope: Ty<'tcx>,
    },
}

#[derive(Clone, Copy, Debug)]
struct RustCapabilityMemoryViewV1<'tcx> {
    element: Ty<'tcx>,
    role: RustCapabilityMemoryRoleV1<'tcx>,
    brand: RustKernelBrandV1<'tcx>,
    kernel: Ty<'tcx>,
    target: Ty<'tcx>,
    launch: Ty<'tcx>,
}

fn rust_capability_memory_view_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<RustCapabilityMemoryViewV1<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::CapabilityMemoryView)
    {
        return None;
    }
    let arguments = arguments.types().collect::<Vec<_>>();
    let [element, space, role, brand] = arguments.as_slice() else {
        return None;
    };
    if !rust_supported_capability_memory_scalar_v1(*element)
        || !rust_is_exact_trusted_marker_v1(
            tcx,
            *space,
            TrustedDeviceItem::CapabilityGlobalAddressSpace,
        )
    {
        return None;
    }
    let role = if rust_is_exact_trusted_marker_v1(tcx, *role, TrustedDeviceItem::CapabilityReadOnly)
    {
        RustCapabilityMemoryRoleV1::ReadOnly
    } else if rust_is_exact_trusted_marker_v1(
        tcx,
        *role,
        TrustedDeviceItem::CapabilityExclusiveReadWrite,
    ) {
        RustCapabilityMemoryRoleV1::ExclusiveReadWrite
    } else if let Some(role_arguments) =
        rust_trusted_adt_type_arguments_v1(tcx, *role, TrustedDeviceItem::CapabilityDisjointWrite)
    {
        let [index_space] = role_arguments.as_slice() else {
            return None;
        };
        RustCapabilityMemoryRoleV1::DisjointWrite {
            index_space: *index_space,
            mapping: rust_disjoint_index_space_v1(tcx, *index_space)?,
        }
    } else {
        let role_arguments = rust_trusted_adt_type_arguments_v1(
            tcx,
            *role,
            TrustedDeviceItem::CapabilityAtomicReadWrite,
        )?;
        let [scope] = role_arguments.as_slice() else {
            return None;
        };
        RustCapabilityMemoryRoleV1::AtomicReadWrite { scope: *scope }
    };
    let brand = rust_kernel_brand_v1(tcx, *brand)?;
    Some(RustCapabilityMemoryViewV1 {
        element: *element,
        role,
        brand,
        kernel: brand.kernel,
        target: brand.target,
        launch: brand.launch,
    })
}

#[derive(Clone, Copy, Debug)]
struct RustKernelBrandV1<'tcx> {
    ty: Ty<'tcx>,
    kernel: Ty<'tcx>,
    target: Ty<'tcx>,
    launch: Ty<'tcx>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RustExecutionMemoryRoleV1<'tcx> {
    ReadOnly,
    ExclusiveReadWrite,
    DisjointWrite {
        index_space: Ty<'tcx>,
    },
    AtomicReadWrite {
        scope: Ty<'tcx>,
        semantic_scope: SemanticExecutionMemoryScopeV1,
    },
}

#[derive(Clone, Copy, Debug)]
struct RustWorkgroupMemoryBrandV1<'tcx> {
    ty: Ty<'tcx>,
    kernel_brand: RustKernelBrandV1<'tcx>,
    epoch: Ty<'tcx>,
}

#[derive(Clone, Copy, Debug)]
enum RustExecutionMemoryBrandV1<'tcx> {
    Kernel(RustKernelBrandV1<'tcx>),
    Workgroup(RustWorkgroupMemoryBrandV1<'tcx>),
}

#[derive(Clone, Copy, Debug)]
struct RustExecutionMemoryViewV1<'tcx> {
    ty: Ty<'tcx>,
    element: Ty<'tcx>,
    space: SemanticExecutionMemoryAddressSpaceV1,
    role: RustExecutionMemoryRoleV1<'tcx>,
    brand: RustExecutionMemoryBrandV1<'tcx>,
}

#[derive(Clone, Copy, Debug)]
struct RustWorkgroupCapabilityV1<'tcx> {
    kernel_brand: RustKernelBrandV1<'tcx>,
    epoch: Ty<'tcx>,
}

#[derive(Clone, Copy, Debug)]
struct RustSubgroupV1<'tcx> {
    width: u32,
    kernel_brand: RustKernelBrandV1<'tcx>,
    epoch: Ty<'tcx>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RustExecutionLdsStateV1 {
    Uninitialized,
    InvocationInitialized,
    Published,
}

#[derive(Clone, Copy, Debug)]
struct RustWorkgroupLdsV1<'tcx> {
    ty: Ty<'tcx>,
    element: Ty<'tcx>,
    elements: u64,
    state: RustExecutionLdsStateV1,
    kernel_brand: RustKernelBrandV1<'tcx>,
    epoch: Ty<'tcx>,
}

#[derive(Clone, Copy, Debug)]
struct RustPendingAsyncCopyV1<'tcx> {
    element: Ty<'tcx>,
    elements: u64,
    kernel_brand: RustKernelBrandV1<'tcx>,
    epoch: Ty<'tcx>,
}

#[derive(Clone, Copy, Debug)]
struct RustWorkgroupEpochV1<'tcx> {
    kernel_brand: RustKernelBrandV1<'tcx>,
    epoch: Ty<'tcx>,
}

#[derive(Clone, Copy, Debug)]
struct RustScopedAtomicV1<'tcx> {
    ty: Ty<'tcx>,
    element: Ty<'tcx>,
    address_space: SemanticExecutionMemoryAddressSpaceV1,
    scope: Ty<'tcx>,
    kernel_brand: RustKernelBrandV1<'tcx>,
    epoch: Ty<'tcx>,
}

#[derive(Clone, Copy, Debug)]
struct RustMatrixCapabilityV1<'tcx> {
    ty: Ty<'tcx>,
    subgroup_brand: Ty<'tcx>,
    width: u32,
    kernel_brand: RustKernelBrandV1<'tcx>,
    epoch: Ty<'tcx>,
}

fn rust_exact_reviewed_adt_arguments_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    expected_path: &str,
) -> Option<GenericArgsRef<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    trusted_device_items::is_exact_reviewed_provider_definition_v1(
        tcx,
        definition.did(),
        expected_path,
    )
    .then_some(arguments)
}

fn rust_kernel_brand_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<RustKernelBrandV1<'tcx>> {
    let arguments = rust_exact_reviewed_adt_arguments_v1(
        tcx,
        ty,
        "fe2o3_device::context::KernelCapabilityBrand",
    )?;
    let types = arguments.types().collect::<Vec<_>>();
    let [kernel, target, launch] = types.as_slice() else {
        return None;
    };
    Some(RustKernelBrandV1 {
        ty,
        kernel: *kernel,
        target: *target,
        launch: *launch,
    })
}

fn rust_workgroup_memory_brand_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<RustWorkgroupMemoryBrandV1<'tcx>> {
    let arguments =
        rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::WorkgroupMemoryBrand)?;
    let [kernel_brand, epoch] = arguments.as_slice() else {
        return None;
    };
    Some(RustWorkgroupMemoryBrandV1 {
        ty,
        kernel_brand: rust_kernel_brand_v1(tcx, *kernel_brand)?,
        epoch: *epoch,
    })
}

fn rust_execution_memory_view_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<RustExecutionMemoryViewV1<'tcx>> {
    let arguments =
        rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::CapabilityMemoryView)?;
    let [element, space, role, brand] = arguments.as_slice() else {
        return None;
    };
    if !rust_supported_capability_memory_scalar_v1(*element) {
        return None;
    }
    let space = if rust_is_exact_trusted_marker_v1(
        tcx,
        *space,
        TrustedDeviceItem::CapabilityPrivateAddressSpace,
    ) {
        SemanticExecutionMemoryAddressSpaceV1::Private
    } else if rust_is_exact_trusted_marker_v1(
        tcx,
        *space,
        TrustedDeviceItem::CapabilityGlobalAddressSpace,
    ) {
        SemanticExecutionMemoryAddressSpaceV1::Global
    } else if rust_is_exact_trusted_marker_v1(
        tcx,
        *space,
        TrustedDeviceItem::CapabilityWorkgroupAddressSpace,
    ) {
        SemanticExecutionMemoryAddressSpaceV1::Workgroup
    } else {
        return None;
    };
    let role = if rust_is_exact_trusted_marker_v1(tcx, *role, TrustedDeviceItem::CapabilityReadOnly)
    {
        RustExecutionMemoryRoleV1::ReadOnly
    } else if rust_is_exact_trusted_marker_v1(
        tcx,
        *role,
        TrustedDeviceItem::CapabilityExclusiveReadWrite,
    ) {
        RustExecutionMemoryRoleV1::ExclusiveReadWrite
    } else if let Some(arguments) =
        rust_trusted_adt_type_arguments_v1(tcx, *role, TrustedDeviceItem::CapabilityDisjointWrite)
    {
        let [index_space] = arguments.as_slice() else {
            return None;
        };
        RustExecutionMemoryRoleV1::DisjointWrite {
            index_space: *index_space,
        }
    } else {
        let arguments = rust_trusted_adt_type_arguments_v1(
            tcx,
            *role,
            TrustedDeviceItem::CapabilityAtomicReadWrite,
        )?;
        let [scope] = arguments.as_slice() else {
            return None;
        };
        RustExecutionMemoryRoleV1::AtomicReadWrite {
            scope: *scope,
            semantic_scope: rust_execution_scope_v1(tcx, *scope)?,
        }
    };
    let brand = match space {
        SemanticExecutionMemoryAddressSpaceV1::Private
        | SemanticExecutionMemoryAddressSpaceV1::Global => {
            RustExecutionMemoryBrandV1::Kernel(rust_kernel_brand_v1(tcx, *brand)?)
        }
        SemanticExecutionMemoryAddressSpaceV1::Workgroup => {
            RustExecutionMemoryBrandV1::Workgroup(rust_workgroup_memory_brand_v1(tcx, *brand)?)
        }
    };
    Some(RustExecutionMemoryViewV1 {
        ty,
        element: *element,
        space,
        role,
        brand,
    })
}

fn rust_workgroup_capability_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<RustWorkgroupCapabilityV1<'tcx>> {
    let arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        ty,
        TrustedDeviceItem::ExecutionWorkgroupCapability,
    )?;
    let [kernel_brand, epoch] = arguments.as_slice() else {
        return None;
    };
    Some(RustWorkgroupCapabilityV1 {
        kernel_brand: rust_kernel_brand_v1(tcx, *kernel_brand)?,
        epoch: *epoch,
    })
}

fn rust_subgroup_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<RustSubgroupV1<'tcx>> {
    let arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        ty,
        TrustedDeviceItem::ExecutionSubgroupCapability,
    )?;
    let [width_ty, kernel_brand, epoch] = arguments.as_slice() else {
        return None;
    };
    Some(RustSubgroupV1 {
        width: rust_subgroup_width_v1(tcx, *width_ty)?,
        kernel_brand: rust_kernel_brand_v1(tcx, *kernel_brand)?,
        epoch: *epoch,
    })
}

fn rust_workgroup_lds_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<RustWorkgroupLdsV1<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::ExecutionWorkgroupLds)
    {
        return None;
    }
    let types = arguments.types().collect::<Vec<_>>();
    let [element, state, kernel_brand, epoch] = types.as_slice() else {
        return None;
    };
    let mut consts = arguments.consts();
    let elements = consts.next()?.try_to_target_usize(tcx)?;
    if consts.next().is_some() || elements == 0 {
        return None;
    }
    Some(RustWorkgroupLdsV1 {
        ty,
        element: *element,
        elements,
        state: rust_execution_lds_state_v1(tcx, *state)?,
        kernel_brand: rust_kernel_brand_v1(tcx, *kernel_brand)?,
        epoch: *epoch,
    })
}

fn rust_pending_async_copy_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<RustPendingAsyncCopyV1<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::ExecutionPendingAsyncCopy)
    {
        return None;
    }
    let types = arguments.types().collect::<Vec<_>>();
    let [element, kernel_brand, epoch] = types.as_slice() else {
        return None;
    };
    let mut consts = arguments.consts();
    let elements = consts.next()?.try_to_target_usize(tcx)?;
    if consts.next().is_some() || elements == 0 {
        return None;
    }
    Some(RustPendingAsyncCopyV1 {
        element: *element,
        elements,
        kernel_brand: rust_kernel_brand_v1(tcx, *kernel_brand)?,
        epoch: *epoch,
    })
}

fn rust_workgroup_epoch_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<RustWorkgroupEpochV1<'tcx>> {
    let arguments =
        rust_exact_reviewed_adt_arguments_v1(tcx, ty, "fe2o3_device::execution::WorkgroupEpoch")?;
    let types = arguments.types().collect::<Vec<_>>();
    let [kernel_brand, epoch] = types.as_slice() else {
        return None;
    };
    Some(RustWorkgroupEpochV1 {
        kernel_brand: rust_kernel_brand_v1(tcx, *kernel_brand)?,
        epoch: *epoch,
    })
}

fn rust_scoped_atomic_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<RustScopedAtomicV1<'tcx>> {
    let arguments =
        rust_exact_reviewed_adt_arguments_v1(tcx, ty, "fe2o3_device::execution::ScopedAtomic")?;
    let types = arguments.types().collect::<Vec<_>>();
    let [element, space, scope, workgroup_brand, epoch] = types.as_slice() else {
        return None;
    };
    let address_space = if rust_is_exact_trusted_marker_v1(
        tcx,
        *space,
        TrustedDeviceItem::CapabilityGlobalAddressSpace,
    ) {
        SemanticExecutionMemoryAddressSpaceV1::Global
    } else if rust_is_exact_trusted_marker_v1(
        tcx,
        *space,
        TrustedDeviceItem::CapabilityWorkgroupAddressSpace,
    ) {
        SemanticExecutionMemoryAddressSpaceV1::Workgroup
    } else {
        return None;
    };
    let workgroup_arguments = rust_exact_reviewed_adt_arguments_v1(
        tcx,
        *workgroup_brand,
        "fe2o3_device::execution::WorkgroupBrand",
    )?
    .types()
    .collect::<Vec<_>>();
    let [kernel_brand] = workgroup_arguments.as_slice() else {
        return None;
    };
    Some(RustScopedAtomicV1 {
        ty,
        element: *element,
        address_space,
        scope: *scope,
        kernel_brand: rust_kernel_brand_v1(tcx, *kernel_brand)?,
        epoch: *epoch,
    })
}

fn rust_matrix_capability_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<RustMatrixCapabilityV1<'tcx>> {
    let arguments =
        rust_exact_reviewed_adt_arguments_v1(tcx, ty, "fe2o3_device::matrix::MatrixCapability")?;
    let types = arguments.types().collect::<Vec<_>>();
    let [subgroup_brand] = types.as_slice() else {
        return None;
    };
    let brand_arguments = rust_exact_reviewed_adt_arguments_v1(
        tcx,
        *subgroup_brand,
        "fe2o3_device::execution::SubgroupBrand",
    )?
    .types()
    .collect::<Vec<_>>();
    let [width, kernel_brand, epoch] = brand_arguments.as_slice() else {
        return None;
    };
    Some(RustMatrixCapabilityV1 {
        ty,
        subgroup_brand: *subgroup_brand,
        width: rust_subgroup_width_v1(tcx, *width)?,
        kernel_brand: rust_kernel_brand_v1(tcx, *kernel_brand)?,
        epoch: *epoch,
    })
}

fn rust_execution_lds_state_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<RustExecutionLdsStateV1> {
    [
        (
            "fe2o3_device::execution::WorkgroupLdsUninitialized",
            RustExecutionLdsStateV1::Uninitialized,
        ),
        (
            "fe2o3_device::execution::WorkgroupLdsInvocationInitialized",
            RustExecutionLdsStateV1::InvocationInitialized,
        ),
        (
            "fe2o3_device::execution::WorkgroupLdsPublished",
            RustExecutionLdsStateV1::Published,
        ),
    ]
    .into_iter()
    .find_map(|(path, state)| {
        rust_exact_reviewed_adt_arguments_v1(tcx, ty, path)
            .is_some_and(|arguments| arguments.iter().next().is_none())
            .then_some(state)
    })
}

fn rust_subgroup_width_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<u32> {
    [
        ("fe2o3_device::wave::SubgroupWidth32", 32),
        ("fe2o3_device::wave::SubgroupWidth64", 64),
    ]
    .into_iter()
    .find_map(|(path, width)| {
        rust_exact_reviewed_adt_arguments_v1(tcx, ty, path)
            .is_some_and(|arguments| arguments.iter().next().is_none())
            .then_some(width)
    })
}

fn rust_initial_epoch_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    rust_exact_reviewed_adt_arguments_v1(tcx, ty, "fe2o3_device::execution::InitialEpoch")
        .is_some_and(|arguments| arguments.iter().next().is_none())
}

fn rust_next_epoch_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let arguments =
        rust_exact_reviewed_adt_arguments_v1(tcx, ty, "fe2o3_device::execution::NextEpoch")?;
    let types = arguments.types().collect::<Vec<_>>();
    let [previous] = types.as_slice() else {
        return None;
    };
    Some(*previous)
}

fn rust_execution_scope_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticExecutionMemoryScopeV1> {
    use SemanticExecutionMemoryScopeV1 as Scope;
    [
        ("fe2o3_device::execution::SystemScope", Scope::System),
        ("fe2o3_device::execution::DeviceScope", Scope::Device),
        ("fe2o3_device::execution::WorkgroupScope", Scope::Workgroup),
        ("fe2o3_device::execution::SubgroupScope", Scope::Subgroup),
    ]
    .into_iter()
    .find_map(|(path, scope)| {
        rust_exact_reviewed_adt_arguments_v1(tcx, ty, path)
            .is_some_and(|arguments| arguments.iter().next().is_none())
            .then_some(scope)
    })
}

fn rust_execution_ordering_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticExecutionMemoryOrderingV1> {
    use SemanticExecutionMemoryOrderingV1 as Ordering;
    [
        ("fe2o3_device::execution::Relaxed", Ordering::Relaxed),
        ("fe2o3_device::execution::Acquire", Ordering::Acquire),
        ("fe2o3_device::execution::Release", Ordering::Release),
        (
            "fe2o3_device::execution::AcquireRelease",
            Ordering::AcquireRelease,
        ),
        (
            "fe2o3_device::execution::SequentiallyConsistent",
            Ordering::SequentiallyConsistent,
        ),
    ]
    .into_iter()
    .find_map(|(path, ordering)| {
        rust_exact_reviewed_adt_arguments_v1(tcx, ty, path)
            .is_some_and(|arguments| arguments.iter().next().is_none())
            .then_some(ordering)
    })
}

fn rust_execution_spaces_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticExecutionMemorySpacesV1> {
    use SemanticExecutionMemorySpacesV1 as Spaces;
    [
        ("fe2o3_device::execution::GlobalMemory", Spaces::Global),
        (
            "fe2o3_device::execution::WorkgroupMemory",
            Spaces::Workgroup,
        ),
        (
            "fe2o3_device::execution::GlobalAndWorkgroupMemory",
            Spaces::GlobalAndWorkgroup,
        ),
    ]
    .into_iter()
    .find_map(|(path, spaces)| {
        rust_exact_reviewed_adt_arguments_v1(tcx, ty, path)
            .is_some_and(|arguments| arguments.iter().next().is_none())
            .then_some(spaces)
    })
}

fn rust_execution_semantics_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Option<SemanticExecutionMemorySemanticsV1> {
    let types = instance.args.types().collect::<Vec<_>>();
    let scopes = types
        .iter()
        .filter_map(|ty| rust_execution_scope_v1(tcx, *ty))
        .collect::<Vec<_>>();
    let orderings = types
        .iter()
        .filter_map(|ty| rust_execution_ordering_v1(tcx, *ty))
        .collect::<Vec<_>>();
    let spaces = types
        .iter()
        .filter_map(|ty| rust_execution_spaces_v1(tcx, *ty))
        .collect::<Vec<_>>();
    let ([scope], [ordering], [spaces]) =
        (scopes.as_slice(), orderings.as_slice(), spaces.as_slice())
    else {
        return None;
    };
    Some(SemanticExecutionMemorySemanticsV1::new(
        *scope, *ordering, *spaces,
    ))
}

fn rust_execution_atomic_orders_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Vec<SemanticExecutionMemoryOrderingV1> {
    instance
        .args
        .types()
        .filter_map(|ty| rust_execution_ordering_v1(tcx, ty))
        .collect()
}

fn rust_execution_generic_scope_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Option<(Ty<'tcx>, SemanticExecutionMemoryScopeV1)> {
    let scopes = instance
        .args
        .types()
        .filter_map(|ty| rust_execution_scope_v1(tcx, ty).map(|scope| (ty, scope)))
        .collect::<Vec<_>>();
    let [scope] = scopes.as_slice() else {
        return None;
    };
    Some(*scope)
}

fn rust_execution_generic_width_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Option<u32> {
    let widths = instance
        .args
        .types()
        .filter_map(|ty| rust_subgroup_width_v1(tcx, ty))
        .collect::<Vec<_>>();
    let [width] = widths.as_slice() else {
        return None;
    };
    Some(*width)
}

fn rust_execution_generic_extent_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Option<u64> {
    let extents = instance
        .args
        .consts()
        .filter_map(|value| value.try_to_target_usize(tcx))
        .collect::<Vec<_>>();
    let [extent] = extents.as_slice() else {
        return None;
    };
    (*extent != 0).then_some(*extent)
}

fn rust_tuple_fields_v1(ty: Ty<'_>) -> Option<Vec<Ty<'_>>> {
    let TyKind::Tuple(fields) = ty.kind() else {
        return None;
    };
    Some(fields.iter().collect())
}

fn rust_unit_v1(ty: Ty<'_>) -> bool {
    rust_tuple_fields_v1(ty).is_some_and(|fields| fields.is_empty())
}

fn rust_kernel_brand_matches_root_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    brand: RustKernelBrandV1<'tcx>,
    root: &AuthenticatedProductionKernelContextRootV1,
) -> bool {
    *rustc_type_identity_v1(tcx, brand.kernel).as_bytes() == root.kernel_marker_identity
}

fn rust_same_kernel_brand_v1<'tcx>(
    left: RustKernelBrandV1<'tcx>,
    right: RustKernelBrandV1<'tcx>,
) -> bool {
    left.ty == right.ty
        && left.kernel == right.kernel
        && left.target == right.target
        && left.launch == right.launch
}

fn rust_execution_epoch_transition_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    before: Ty<'tcx>,
    after: Ty<'tcx>,
) -> bool {
    rust_next_epoch_v1(tcx, after) == Some(before)
}

fn rust_execution_memory_role_contract_v1<'tcx>(
    role: RustExecutionMemoryRoleV1<'tcx>,
) -> (
    SemanticExecutionMemoryAccessV1,
    Option<Ty<'tcx>>,
    Option<SemanticExecutionMemoryScopeV1>,
) {
    match role {
        RustExecutionMemoryRoleV1::ReadOnly => {
            (SemanticExecutionMemoryAccessV1::ReadOnly, None, None)
        }
        RustExecutionMemoryRoleV1::ExclusiveReadWrite => (
            SemanticExecutionMemoryAccessV1::ExclusiveReadWrite,
            None,
            None,
        ),
        RustExecutionMemoryRoleV1::DisjointWrite { index_space } => (
            SemanticExecutionMemoryAccessV1::DisjointWrite,
            Some(index_space),
            None,
        ),
        RustExecutionMemoryRoleV1::AtomicReadWrite { semantic_scope, .. } => (
            SemanticExecutionMemoryAccessV1::AtomicReadWrite,
            None,
            Some(semantic_scope),
        ),
    }
}

fn rust_execution_view_matches_workgroup_v1<'tcx>(
    view: RustExecutionMemoryViewV1<'tcx>,
    workgroup: RustWorkgroupCapabilityV1<'tcx>,
) -> bool {
    matches!(
        view.brand,
        RustExecutionMemoryBrandV1::Workgroup(brand)
            if rust_same_kernel_brand_v1(brand.kernel_brand, workgroup.kernel_brand)
                && brand.epoch == workgroup.epoch
    )
}

fn rust_workgroup_memory_index_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    item: TrustedDeviceItem,
) -> Option<(Ty<'tcx>, RustWorkgroupMemoryBrandV1<'tcx>)> {
    let arguments = rust_trusted_adt_type_arguments_v1(tcx, ty, item)?;
    let [index_space, brand] = arguments.as_slice() else {
        return None;
    };
    rust_is_exact_trusted_marker_v1(
        tcx,
        *index_space,
        TrustedDeviceItem::WorkgroupMemoryIndexSpace1D,
    )
    .then_some((*index_space, rust_workgroup_memory_brand_v1(tcx, *brand)?))
}

fn rust_branded_index_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    item: TrustedDeviceItem,
) -> Option<(Ty<'tcx>, Ty<'tcx>)> {
    let arguments = rust_trusted_adt_type_arguments_v1(tcx, ty, item)?;
    let [index_space, brand] = arguments.as_slice() else {
        return None;
    };
    Some((*index_space, *brand))
}

fn rust_mut_raw_pointer_element_v1(ty: Ty<'_>) -> Option<Ty<'_>> {
    match *ty.kind() {
        TyKind::RawPtr(pointee, rustc_hir::Mutability::Mut) => Some(pointee),
        _ => None,
    }
}

fn rust_kernel_context_axes_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(Ty<'tcx>, Ty<'tcx>, Ty<'tcx>)> {
    let arguments = rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::KernelContext)?;
    let [kernel, target, launch] = arguments.as_slice() else {
        return None;
    };
    Some((*kernel, *target, *launch))
}

fn rust_invocation_brand_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let arguments = rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::Invocation3D)?;
    let [brand] = arguments.as_slice() else {
        return None;
    };
    Some(*brand)
}

fn semantic_type_for_rust_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    types: &[SemanticTypeDeclV1],
    ty: Ty<'tcx>,
) -> Result<SemanticTypeIdV1, ProductionSemanticImportErrorV1> {
    let identity = rustc_type_identity_v1(tcx, ty);
    let mut matches = types
        .iter()
        .enumerate()
        .filter(|(_, declaration)| declaration.identity() == identity);
    let Some((index, _)) = matches.next() else {
        return Err(body_owner_table_mismatch_v1(
            "typed-global nested semantic type identity",
        ));
    };
    if matches.next().is_some() {
        return Err(body_owner_table_mismatch_v1(
            "typed-global duplicate semantic type identity",
        ));
    }
    Ok(SemanticTypeIdV1::from_index(u32::try_from(index).map_err(
        |_| ProductionSemanticImportErrorV1::RootIdentityMismatch,
    )?))
}

fn capability_memory_provenance_v1(
    root: &AuthenticatedProductionKernelContextRootV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<SemanticKernelCapabilityProvenanceV1, ProductionSemanticImportErrorV1> {
    SemanticKernelCapabilityProvenanceV1::new(
        root.selected_root,
        SemanticKernelBindingIdentityV1::from_sha256(root.kernel_binding),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256(
            contexts.frontend_unit_identity,
        ),
        SemanticTypeIdentityV1::from_sha256(root.kernel_marker_identity),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256(contexts.target_brand_identity),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256(root.launch_brand_identity),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256(root.issuance_identity),
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)
}

fn require_capability_memory_terminal_abi_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    rust_inputs: &[Ty<'tcx>],
    rust_output: Ty<'tcx>,
    expected_ownership: &[SemanticSourceArgumentOwnershipV1],
) -> Result<(), ProductionSemanticImportErrorV1> {
    if abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.can_unwind()
        || abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || abi.source_input_types().len() != rust_inputs.len()
        || abi.arguments().len() != rust_inputs.len()
        || abi.adjusted_arguments().len() != rust_inputs.len()
        || usize::try_from(abi.fixed_count()).ok() != Some(rust_inputs.len())
        || abi.source_argument_ownership() != expected_ownership
        || abi.return_value().ty() != abi.source_output_type()
        || abi.return_value().adjusted().is_some()
        || abi.return_value().pointee_override().is_some()
    {
        return Err(body_owner_table_mismatch_v1("typed-global terminal FnAbi"));
    }
    for ((semantic, rust), physical) in abi
        .source_input_types()
        .iter()
        .zip(rust_inputs)
        .zip(abi.arguments())
    {
        if physical.role() != SemanticAbiArgumentRoleV1::Source
            || physical.ty() != *semantic
            || physical.value().adjusted().is_some()
            || physical.value().pointee_override().is_some()
            || *semantic != semantic_type_for_rust_v1(tcx, types, *rust)?
        {
            return Err(body_owner_table_mismatch_v1(
                "typed-global terminal physical input ABI",
            ));
        }
    }
    if abi.source_output_type() != semantic_type_for_rust_v1(tcx, types, rust_output)? {
        return Err(body_owner_table_mismatch_v1(
            "typed-global terminal output type identity",
        ));
    }
    Ok(())
}

fn require_execution_terminal_abi_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    rust_inputs: &[Ty<'tcx>],
    rust_output: Ty<'tcx>,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let ownership = rust_inputs
        .iter()
        .map(|ty| match ty.kind() {
            TyKind::Ref(_, _, rustc_hir::Mutability::Not) => {
                SemanticSourceArgumentOwnershipV1::SharedBorrow
            }
            TyKind::Ref(_, _, rustc_hir::Mutability::Mut) => {
                SemanticSourceArgumentOwnershipV1::UniqueBorrow
            }
            _ => SemanticSourceArgumentOwnershipV1::ByValue,
        })
        .collect::<Vec<_>>();
    require_capability_memory_terminal_abi_v1(tcx, abi, types, rust_inputs, rust_output, &ownership)
}

#[allow(clippy::too_many_arguments)]
fn execution_capability_operation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    abi: &SemanticFunctionAbiV1,
    operation: SemanticExecutionCapabilityOperationV1,
    root: &AuthenticatedProductionKernelContextRootV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
    kernel_brand: RustKernelBrandV1<'tcx>,
    epoch_before: Ty<'tcx>,
    epoch_after: Option<Ty<'tcx>>,
    source_identity: SemanticFunctionIdentityV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    if !rust_kernel_brand_matches_root_v1(tcx, kernel_brand, root) {
        return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
            "execution terminal kernel brand does not match its authenticated root",
        ));
    }
    let signature = SemanticExecutionCapabilitySignatureV1::new(
        abi.source_input_types(),
        abi.source_output_type(),
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    let contract = SemanticExecutionCapabilityContractV1::new(
        operation,
        signature,
        capability_memory_provenance_v1(root, contexts)?,
        rustc_type_identity_v1(tcx, kernel_brand.ty),
        rustc_type_identity_v1(tcx, epoch_before),
        epoch_after.map(|epoch| rustc_type_identity_v1(tcx, epoch)),
        source_identity,
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    Ok(SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract })
}

fn kernel_scoped_execution_capability_operation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    abi: &SemanticFunctionAbiV1,
    operation: SemanticExecutionCapabilityOperationV1,
    root: &AuthenticatedProductionKernelContextRootV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
    kernel_brand: RustKernelBrandV1<'tcx>,
    source_identity: SemanticFunctionIdentityV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    if !rust_kernel_brand_matches_root_v1(tcx, kernel_brand, root) {
        return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
            "kernel-scoped execution terminal brand does not match its authenticated root",
        ));
    }
    let signature = SemanticExecutionCapabilitySignatureV1::new(
        abi.source_input_types(),
        abi.source_output_type(),
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    let contract = SemanticExecutionCapabilityContractV1::new_kernel_scoped(
        operation,
        signature,
        capability_memory_provenance_v1(root, contexts)?,
        source_identity,
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    Ok(SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract })
}

fn rust_supported_capability_memory_scalar_v1(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Int(IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64)
            | TyKind::Uint(UintTy::U8 | UintTy::U16 | UintTy::U32 | UintTy::U64)
            | TyKind::Float(FloatTy::F32 | FloatTy::F64)
    )
}

fn rust_supported_atomic_scalar_v1(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Int(IntTy::I32 | IntTy::I64) | TyKind::Uint(UintTy::U32 | UintTy::U64)
    )
}

fn rust_supported_collective_scalar_v1(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Int(IntTy::I32) | TyKind::Uint(UintTy::U32) | TyKind::Float(FloatTy::F32)
    )
}

fn rust_reference_with_mutability_v1(
    ty: Ty<'_>,
    mutability: rustc_hir::Mutability,
) -> Option<Ty<'_>> {
    match *ty.kind() {
        TyKind::Ref(_, pointee, actual) if actual == mutability => Some(pointee),
        _ => None,
    }
}

fn rust_shared_reference_v1(ty: Ty<'_>) -> Option<Ty<'_>> {
    rust_reference_with_mutability_v1(ty, rustc_hir::Mutability::Not)
}

#[allow(clippy::too_many_arguments)]
fn raw_memory_terminal_operation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    terminal: crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    rust_inputs: &[Ty<'tcx>],
    rust_output: Ty<'tcx>,
    root: &AuthenticatedProductionKernelContextRootV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
    source_identity: SemanticFunctionIdentityV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    use crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1 as Terminal;

    if tcx.fn_sig(instance.def_id()).skip_binder().safety() != rustc_hir::Safety::Unsafe
        || !matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize))
        || !rust_is_trusted_adt_v1(
            tcx,
            rust_inputs[3],
            TrustedDeviceItem::UnsafeRawMemoryObligation,
        )
    {
        return Err(body_owner_table_mismatch_v1(
            "raw-memory terminal safety, extent, or obligation ABI",
        ));
    }
    let view = rust_execution_memory_view_v1(tcx, rust_output)
        .ok_or_else(|| body_owner_table_mismatch_v1("raw-memory output view"))?;
    if rust_mut_raw_pointer_element_v1(rust_inputs[1]) != Some(view.element) {
        return Err(body_owner_table_mismatch_v1("raw-memory pointer element"));
    }
    let (access, index_space, atomic_scope) = rust_execution_memory_role_contract_v1(view.role);
    let operation = SemanticExecutionCapabilityOperationV1::RawMemoryBind {
        authority: abi.source_input_types()[0],
        pointer: abi.source_input_types()[1],
        length: abi.source_input_types()[2],
        view: abi.source_output_type(),
        element: semantic_type_for_rust_v1(tcx, types, view.element)?,
        space: view.space,
        access,
        index_space: index_space
            .map(|ty| semantic_type_for_rust_v1(tcx, types, ty))
            .transpose()?,
        atomic_scope,
        unsafe_obligation: abi.source_input_types()[3],
    };
    match terminal {
        Terminal::PrivateMemoryFromRawParts => {
            let context = rust_shared_reference_v1(rust_inputs[0])
                .and_then(|ty| rust_kernel_context_axes_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("private raw-memory context"))?;
            let RustExecutionMemoryBrandV1::Kernel(kernel_brand) = view.brand else {
                return Err(body_owner_table_mismatch_v1("private raw-memory brand"));
            };
            if view.space != SemanticExecutionMemoryAddressSpaceV1::Private
                || context
                    != (
                        kernel_brand.kernel,
                        kernel_brand.target,
                        kernel_brand.launch,
                    )
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "private raw-memory terminal substituted address space or kernel brand",
                ));
            }
            kernel_scoped_execution_capability_operation_v1(
                tcx,
                abi,
                operation,
                root,
                contexts,
                kernel_brand,
                source_identity,
            )
        }
        Terminal::WorkgroupMemoryFromRawParts => {
            let workgroup = rust_shared_reference_v1(rust_inputs[0])
                .and_then(|ty| rust_workgroup_capability_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup raw-memory authority"))?;
            if view.space != SemanticExecutionMemoryAddressSpaceV1::Workgroup
                || !rust_execution_view_matches_workgroup_v1(view, workgroup)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "workgroup raw-memory terminal substituted address space, brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                operation,
                root,
                contexts,
                workgroup.kernel_brand,
                workgroup.epoch,
                None,
                source_identity,
            )
        }
        _ => Err(body_owner_table_mismatch_v1("raw-memory terminal kind")),
    }
}

#[allow(clippy::too_many_arguments)]
fn memory_access_terminal_operation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    terminal: crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    rust_inputs: &[Ty<'tcx>],
    rust_output: Ty<'tcx>,
    root: &AuthenticatedProductionKernelContextRootV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
    source_identity: SemanticFunctionIdentityV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    use crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1 as Terminal;

    let store = matches!(
        terminal,
        Terminal::PrivateMemoryExclusiveStore
            | Terminal::PrivateMemoryDisjointStore
            | Terminal::WorkgroupMemoryExclusiveStore
            | Terminal::WorkgroupMemoryDisjointStore
    );
    let workgroup_scoped = matches!(
        terminal,
        Terminal::WorkgroupMemoryLoad
            | Terminal::WorkgroupMemoryExclusiveLoad
            | Terminal::WorkgroupMemoryExclusiveStore
            | Terminal::WorkgroupMemoryDisjointStore
    );
    let view_ty = if store {
        rust_reference_with_mutability_v1(rust_inputs[0], rustc_hir::Mutability::Mut)
    } else {
        rust_shared_reference_v1(rust_inputs[0])
    }
    .ok_or_else(|| body_owner_table_mismatch_v1("memory-access view borrow"))?;
    let view = rust_execution_memory_view_v1(tcx, view_ty)
        .ok_or_else(|| body_owner_table_mismatch_v1("memory-access view"))?;
    let expected_space = if workgroup_scoped {
        SemanticExecutionMemoryAddressSpaceV1::Workgroup
    } else {
        SemanticExecutionMemoryAddressSpaceV1::Private
    };
    let expected_role = match terminal {
        Terminal::PrivateMemoryLoad | Terminal::WorkgroupMemoryLoad => {
            RustExecutionMemoryRoleV1::ReadOnly
        }
        Terminal::PrivateMemoryExclusiveLoad
        | Terminal::PrivateMemoryExclusiveStore
        | Terminal::WorkgroupMemoryExclusiveLoad
        | Terminal::WorkgroupMemoryExclusiveStore => RustExecutionMemoryRoleV1::ExclusiveReadWrite,
        Terminal::PrivateMemoryDisjointStore | Terminal::WorkgroupMemoryDisjointStore => {
            let RustExecutionMemoryRoleV1::DisjointWrite { index_space } = view.role else {
                return Err(body_owner_table_mismatch_v1("disjoint memory-access role"));
            };
            RustExecutionMemoryRoleV1::DisjointWrite { index_space }
        }
        _ => return Err(body_owner_table_mismatch_v1("memory-access terminal kind")),
    };
    if view.space != expected_space || view.role != expected_role {
        return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
            "memory-access terminal substituted address space or role",
        ));
    }
    let workgroup = workgroup_scoped
        .then(|| {
            rust_inputs
                .get(1)
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_capability_v1(tcx, ty))
        })
        .flatten();
    if workgroup_scoped
        && !workgroup
            .is_some_and(|workgroup| rust_execution_view_matches_workgroup_v1(view, workgroup))
    {
        return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
            "workgroup memory access substituted its workgroup brand or epoch",
        ));
    }
    let index_position = usize::from(workgroup_scoped) + 1;
    let index_ty = rust_inputs[index_position];
    match view.role {
        RustExecutionMemoryRoleV1::DisjointWrite { index_space } => {
            let (witness_space, witness_brand) =
                rust_branded_index_v1(tcx, index_ty, TrustedDeviceItem::DisjointIndex)
                    .ok_or_else(|| body_owner_table_mismatch_v1("disjoint memory witness"))?;
            let expected_brand = match view.brand {
                RustExecutionMemoryBrandV1::Kernel(brand) => brand.ty,
                RustExecutionMemoryBrandV1::Workgroup(brand) => brand.ty,
            };
            if witness_space != index_space || witness_brand != expected_brand {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "disjoint memory access substituted index space or brand",
                ));
            }
        }
        _ if !matches!(index_ty.kind(), TyKind::Uint(UintTy::Usize)) => {
            return Err(body_owner_table_mismatch_v1("memory-access integer index"));
        }
        _ => {}
    }
    let (access, _, _) = rust_execution_memory_role_contract_v1(view.role);
    let operation = if store {
        let value_position = index_position + 1;
        if rust_inputs.get(value_position).copied() != Some(view.element)
            || !matches!(rust_output.kind(), TyKind::Bool)
        {
            return Err(body_owner_table_mismatch_v1("memory-store value or result"));
        }
        SemanticExecutionCapabilityOperationV1::MemoryStore {
            view: abi.source_input_types()[0],
            workgroup: workgroup_scoped.then_some(abi.source_input_types()[1]),
            index: abi.source_input_types()[index_position],
            element: semantic_type_for_rust_v1(tcx, types, view.element)?,
            result: abi.source_output_type(),
            space: view.space,
            access,
        }
    } else {
        if rust_option_payload_v1(tcx, rust_output) != Some(view.element) {
            return Err(body_owner_table_mismatch_v1("memory-load option element"));
        }
        SemanticExecutionCapabilityOperationV1::MemoryLoad {
            view: abi.source_input_types()[0],
            workgroup: workgroup_scoped.then_some(abi.source_input_types()[1]),
            index: abi.source_input_types()[index_position],
            option: abi.source_output_type(),
            element: semantic_type_for_rust_v1(tcx, types, view.element)?,
            space: view.space,
            access,
        }
    };
    match view.brand {
        RustExecutionMemoryBrandV1::Kernel(kernel_brand) => {
            kernel_scoped_execution_capability_operation_v1(
                tcx,
                abi,
                operation,
                root,
                contexts,
                kernel_brand,
                source_identity,
            )
        }
        RustExecutionMemoryBrandV1::Workgroup(_) => {
            let workgroup = workgroup.expect("workgroup-scoped view checked above");
            execution_capability_operation_v1(
                tcx,
                abi,
                operation,
                root,
                contexts,
                workgroup.kernel_brand,
                workgroup.epoch,
                None,
                source_identity,
            )
        }
    }
}

fn execution_terminal_operation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    terminal: crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    capability_root: Option<&AuthenticatedProductionKernelContextRootV1>,
    source_identity: SemanticFunctionIdentityV1,
    kernel_contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    use crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1 as Terminal;

    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let rust_inputs = signature.inputs();
    let rust_output = signature.output();
    if rust_inputs.len() != terminal.source_argument_count() {
        return Err(body_owner_table_mismatch_v1(
            "execution terminal source arity",
        ));
    }
    require_execution_terminal_abi_v1(tcx, abi, types, rust_inputs, rust_output)?;
    let root = capability_root.ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
        "execution terminal lacks unique authenticated kernel-root custody",
    ))?;
    let type_id = |ty| semantic_type_for_rust_v1(tcx, types, ty);

    match terminal {
        Terminal::PrivateMemoryFromRawParts | Terminal::WorkgroupMemoryFromRawParts => {
            raw_memory_terminal_operation_v1(
                tcx,
                instance,
                terminal,
                abi,
                types,
                rust_inputs,
                rust_output,
                root,
                kernel_contexts,
                source_identity,
            )
        }
        Terminal::PrivateMemoryLoad
        | Terminal::PrivateMemoryExclusiveLoad
        | Terminal::PrivateMemoryExclusiveStore
        | Terminal::PrivateMemoryDisjointStore
        | Terminal::WorkgroupMemoryLoad
        | Terminal::WorkgroupMemoryExclusiveLoad
        | Terminal::WorkgroupMemoryExclusiveStore
        | Terminal::WorkgroupMemoryDisjointStore => memory_access_terminal_operation_v1(
            tcx,
            terminal,
            abi,
            types,
            rust_inputs,
            rust_output,
            root,
            kernel_contexts,
            source_identity,
        ),
        Terminal::PrivateMemoryAllocate => {
            let context_ty = rust_shared_reference_v1(rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("private allocation context"))?;
            let context = rust_kernel_context_axes_v1(tcx, context_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("private allocation context type"))?;
            let view = rust_execution_memory_view_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("private allocation output"))?;
            let RustExecutionMemoryBrandV1::Kernel(kernel_brand) = view.brand else {
                return Err(body_owner_table_mismatch_v1("private allocation brand"));
            };
            let elements = rust_execution_generic_extent_v1(tcx, instance)
                .ok_or_else(|| body_owner_table_mismatch_v1("private allocation extent"))?;
            if view.space != SemanticExecutionMemoryAddressSpaceV1::Private
                || view.role != RustExecutionMemoryRoleV1::ExclusiveReadWrite
                || context
                    != (
                        kernel_brand.kernel,
                        kernel_brand.target,
                        kernel_brand.launch,
                    )
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "private allocation substituted element, role, space, or kernel brand",
                ));
            }
            kernel_scoped_execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::PrivateMemoryAllocate {
                    context: abi.source_input_types()[0],
                    view: abi.source_output_type(),
                    element: type_id(view.element)?,
                    elements,
                },
                root,
                kernel_contexts,
                kernel_brand,
                source_identity,
            )
        }
        Terminal::WorkgroupMemoryIndex1D => {
            let workgroup = rust_shared_reference_v1(rust_inputs[0])
                .and_then(|ty| rust_workgroup_capability_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup-memory index authority"))?;
            let witness = rust_option_payload_v1(tcx, rust_output)
                .and_then(|ty| {
                    rust_workgroup_memory_index_v1(tcx, ty, TrustedDeviceItem::ThreadIndex)
                })
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup-memory index output"))?;
            if !rust_same_kernel_brand_v1(workgroup.kernel_brand, witness.1.kernel_brand)
                || workgroup.epoch != witness.1.epoch
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "workgroup-memory index substituted index space, workgroup brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndex {
                    workgroup: abi.source_input_types()[0],
                    witness: abi.source_output_type(),
                },
                root,
                kernel_contexts,
                workgroup.kernel_brand,
                workgroup.epoch,
                None,
                source_identity,
            )
        }
        Terminal::WorkgroupMemoryAllocate => {
            let workgroup = rust_shared_reference_v1(rust_inputs[0])
                .and_then(|ty| rust_workgroup_capability_v1(tcx, ty))
                .ok_or_else(|| {
                    body_owner_table_mismatch_v1("workgroup-memory allocation authority")
                })?;
            let view = rust_execution_memory_view_v1(tcx, rust_output).ok_or_else(|| {
                body_owner_table_mismatch_v1("workgroup-memory allocation output")
            })?;
            let RustExecutionMemoryRoleV1::DisjointWrite { index_space } = view.role else {
                return Err(body_owner_table_mismatch_v1(
                    "workgroup-memory allocation role",
                ));
            };
            let elements = rust_execution_generic_extent_v1(tcx, instance).ok_or_else(|| {
                body_owner_table_mismatch_v1("workgroup-memory allocation extent")
            })?;
            if view.space != SemanticExecutionMemoryAddressSpaceV1::Workgroup
                || !rust_is_exact_trusted_marker_v1(
                    tcx,
                    index_space,
                    TrustedDeviceItem::WorkgroupMemoryIndexSpace1D,
                )
                || !rust_execution_view_matches_workgroup_v1(view, workgroup)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "workgroup-memory allocation substituted role, index space, brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::WorkgroupMemoryAllocate {
                    workgroup: abi.source_input_types()[0],
                    view: abi.source_output_type(),
                    element: type_id(view.element)?,
                    elements,
                    index_space: type_id(index_space)?,
                },
                root,
                kernel_contexts,
                workgroup.kernel_brand,
                workgroup.epoch,
                None,
                source_identity,
            )
        }
        Terminal::WorkgroupMemoryPublish => {
            let input_workgroup =
                rust_workgroup_capability_v1(tcx, rust_inputs[0]).ok_or_else(|| {
                    body_owner_table_mismatch_v1("workgroup-memory publish authority")
                })?;
            let input_view = rust_execution_memory_view_v1(tcx, rust_inputs[1])
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup-memory publish input"))?;
            let output = rust_tuple_fields_v1(rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup-memory publish output"))?;
            let [output_workgroup_ty, output_view_ty] = output.as_slice() else {
                return Err(body_owner_table_mismatch_v1(
                    "workgroup-memory publish tuple",
                ));
            };
            let output_workgroup = rust_workgroup_capability_v1(tcx, *output_workgroup_ty)
                .ok_or_else(|| {
                    body_owner_table_mismatch_v1("workgroup-memory publish output authority")
                })?;
            let output_view =
                rust_execution_memory_view_v1(tcx, *output_view_ty).ok_or_else(|| {
                    body_owner_table_mismatch_v1("workgroup-memory publish output view")
                })?;
            let RustExecutionMemoryRoleV1::DisjointWrite { index_space } = input_view.role else {
                return Err(body_owner_table_mismatch_v1(
                    "workgroup-memory publish input role",
                ));
            };
            if input_view.space != SemanticExecutionMemoryAddressSpaceV1::Workgroup
                || output_view.space != SemanticExecutionMemoryAddressSpaceV1::Workgroup
                || output_view.role != RustExecutionMemoryRoleV1::ReadOnly
                || !rust_is_exact_trusted_marker_v1(
                    tcx,
                    index_space,
                    TrustedDeviceItem::WorkgroupMemoryIndexSpace1D,
                )
                || input_view.element != output_view.element
                || !rust_execution_view_matches_workgroup_v1(input_view, input_workgroup)
                || !rust_same_kernel_brand_v1(
                    input_workgroup.kernel_brand,
                    output_workgroup.kernel_brand,
                )
                || !rust_execution_epoch_transition_v1(
                    tcx,
                    input_workgroup.epoch,
                    output_workgroup.epoch,
                )
                || !rust_execution_view_matches_workgroup_v1(output_view, output_workgroup)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "workgroup-memory publish substituted role, element, brand, or epoch transition",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::WorkgroupMemoryPublish {
                    input_workgroup: abi.source_input_types()[0],
                    input_view: abi.source_input_types()[1],
                    output_view: type_id(output_view.ty)?,
                    transition: abi.source_output_type(),
                    element: type_id(input_view.element)?,
                },
                root,
                kernel_contexts,
                input_workgroup.kernel_brand,
                input_workgroup.epoch,
                Some(output_workgroup.epoch),
                source_identity,
            )
        }
        Terminal::BindAtomicView => {
            let context = rust_inputs
                .first()
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_kernel_context_axes_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("atomic-view context"))?;
            let physical_element = rust_inputs
                .get(1)
                .and_then(|ty| rust_shared_slice_element_v1(*ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("atomic-view physical slice"))?;
            let view = rust_capability_memory_view_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("atomic-view output"))?;
            let RustCapabilityMemoryRoleV1::AtomicReadWrite { scope } = view.role else {
                return Err(body_owner_table_mismatch_v1("atomic-view role"));
            };
            let (generic_scope, semantic_scope) = rust_execution_generic_scope_v1(tcx, instance)
                .ok_or_else(|| body_owner_table_mismatch_v1("atomic-view scope argument"))?;
            if context != (view.kernel, view.target, view.launch)
                || view.element != physical_element
                || scope != generic_scope
                || !rust_supported_atomic_scalar_v1(view.element)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "atomic-view binding substituted context, element, scope, or kernel brand",
                ));
            }
            kernel_scoped_execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::Atomic {
                    kind: SemanticExecutionAtomicKindV1::BindGlobalView,
                    authority: abi.source_input_types()[0],
                    location_input: abi.source_input_types()[1],
                    location: abi.source_output_type(),
                    element: type_id(view.element)?,
                    operand: None,
                    replacement: None,
                    result: abi.source_output_type(),
                    address_space: SemanticExecutionMemoryAddressSpaceV1::Global,
                    scope: semantic_scope,
                    success: None,
                    failure: None,
                },
                root,
                kernel_contexts,
                view.brand,
                source_identity,
            )
        }
        Terminal::WorkgroupDerive => {
            let context_ty = rust_inputs
                .first()
                .and_then(|ty| rust_reference_with_mutability_v1(*ty, rustc_hir::Mutability::Mut))
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup-derive context"))?;
            let context = rust_kernel_context_axes_v1(tcx, context_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup-derive context type"))?;
            let workgroup = rust_workgroup_capability_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup-derive output"))?;
            if context
                != (
                    workgroup.kernel_brand.kernel,
                    workgroup.kernel_brand.target,
                    workgroup.kernel_brand.launch,
                )
                || !rust_initial_epoch_v1(tcx, workgroup.epoch)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "workgroup derivation substituted its context brand or initial epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::WorkgroupDerive {
                    context: abi.source_input_types()[0],
                    workgroup: abi.source_output_type(),
                },
                root,
                kernel_contexts,
                workgroup.kernel_brand,
                workgroup.epoch,
                None,
                source_identity,
            )
        }
        Terminal::SubgroupDerive => {
            let workgroup = rust_inputs
                .first()
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_capability_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup-derive workgroup"))?;
            let subgroup = rust_subgroup_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup-derive output"))?;
            if !rust_same_kernel_brand_v1(workgroup.kernel_brand, subgroup.kernel_brand)
                || workgroup.epoch != subgroup.epoch
                || rust_execution_generic_width_v1(tcx, instance) != Some(subgroup.width)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "subgroup derivation substituted width, workgroup brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::SubgroupDerive {
                    workgroup: abi.source_input_types()[0],
                    subgroup: abi.source_output_type(),
                    width: subgroup.width,
                },
                root,
                kernel_contexts,
                workgroup.kernel_brand,
                workgroup.epoch,
                None,
                source_identity,
            )
        }
        Terminal::LdsAllocate => {
            let workgroup = rust_inputs
                .first()
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_capability_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("LDS allocation workgroup"))?;
            let lds = rust_workgroup_lds_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("LDS allocation output"))?;
            if lds.state != RustExecutionLdsStateV1::Uninitialized
                || !rust_same_kernel_brand_v1(workgroup.kernel_brand, lds.kernel_brand)
                || workgroup.epoch != lds.epoch
                || rust_execution_generic_extent_v1(tcx, instance) != Some(lds.elements)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "LDS allocation substituted extent, state, workgroup brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::LdsAllocate {
                    workgroup: abi.source_input_types()[0],
                    lds: abi.source_output_type(),
                    element: type_id(lds.element)?,
                    elements: lds.elements,
                },
                root,
                kernel_contexts,
                workgroup.kernel_brand,
                workgroup.epoch,
                None,
                source_identity,
            )
        }
        Terminal::WorkgroupBarrier => {
            let input = rust_workgroup_capability_v1(tcx, rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup barrier input"))?;
            let output = rust_workgroup_capability_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup barrier output"))?;
            let semantics = rust_execution_semantics_v1(tcx, instance)
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup barrier semantics"))?;
            if !rust_same_kernel_brand_v1(input.kernel_brand, output.kernel_brand)
                || !rust_execution_epoch_transition_v1(tcx, input.epoch, output.epoch)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "workgroup barrier substituted its brand or epoch transition",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::WorkgroupBarrier {
                    input_workgroup: abi.source_input_types()[0],
                    output_workgroup: abi.source_output_type(),
                    semantics,
                },
                root,
                kernel_contexts,
                input.kernel_brand,
                input.epoch,
                Some(output.epoch),
                source_identity,
            )
        }
        Terminal::SubgroupBarrier => {
            let input = rust_workgroup_capability_v1(tcx, rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup barrier workgroup"))?;
            let subgroup = rust_subgroup_v1(tcx, rust_inputs[1])
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup barrier subgroup"))?;
            let output = rust_tuple_fields_v1(rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup barrier output tuple"))?;
            let [output_workgroup_ty, output_subgroup_ty] = output.as_slice() else {
                return Err(body_owner_table_mismatch_v1(
                    "subgroup barrier output tuple",
                ));
            };
            let output_workgroup = rust_workgroup_capability_v1(tcx, *output_workgroup_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup barrier output workgroup"))?;
            let output_subgroup = rust_subgroup_v1(tcx, *output_subgroup_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup barrier output subgroup"))?;
            let semantics = rust_execution_semantics_v1(tcx, instance)
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup barrier semantics"))?;
            if !rust_same_kernel_brand_v1(input.kernel_brand, subgroup.kernel_brand)
                || !rust_same_kernel_brand_v1(input.kernel_brand, output_workgroup.kernel_brand)
                || !rust_same_kernel_brand_v1(input.kernel_brand, output_subgroup.kernel_brand)
                || input.epoch != subgroup.epoch
                || !rust_execution_epoch_transition_v1(tcx, input.epoch, output_workgroup.epoch)
                || output_workgroup.epoch != output_subgroup.epoch
                || subgroup.width != output_subgroup.width
                || rust_execution_generic_width_v1(tcx, instance) != Some(subgroup.width)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "subgroup barrier substituted participants, width, brand, or epoch transition",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::SubgroupBarrier {
                    input_workgroup: abi.source_input_types()[0],
                    semantics,
                    subgroup: abi.source_input_types()[1],
                    transition: abi.source_output_type(),
                    width: subgroup.width,
                },
                root,
                kernel_contexts,
                input.kernel_brand,
                input.epoch,
                Some(output_workgroup.epoch),
                source_identity,
            )
        }
        Terminal::WorkgroupFence => {
            let workgroup = rust_inputs
                .first()
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_capability_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup fence receiver"))?;
            let semantics = rust_execution_semantics_v1(tcx, instance)
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup fence semantics"))?;
            if !rust_unit_v1(rust_output) {
                return Err(body_owner_table_mismatch_v1("workgroup fence output"));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::WorkgroupFence {
                    workgroup: abi.source_input_types()[0],
                    result: abi.source_output_type(),
                    semantics,
                },
                root,
                kernel_contexts,
                workgroup.kernel_brand,
                workgroup.epoch,
                None,
                source_identity,
            )
        }
        Terminal::LdsPublish => {
            let input = rust_workgroup_capability_v1(tcx, rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("LDS publish workgroup"))?;
            let input_lds = rust_workgroup_lds_v1(tcx, rust_inputs[1])
                .ok_or_else(|| body_owner_table_mismatch_v1("LDS publish input"))?;
            let output = rust_tuple_fields_v1(rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("LDS publish output tuple"))?;
            let [output_workgroup_ty, output_lds_ty] = output.as_slice() else {
                return Err(body_owner_table_mismatch_v1("LDS publish output tuple"));
            };
            let output_workgroup = rust_workgroup_capability_v1(tcx, *output_workgroup_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("LDS publish output workgroup"))?;
            let output_lds = rust_workgroup_lds_v1(tcx, *output_lds_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("LDS publish output LDS"))?;
            if input_lds.state != RustExecutionLdsStateV1::InvocationInitialized
                || output_lds.state != RustExecutionLdsStateV1::Published
                || input_lds.element != output_lds.element
                || input_lds.elements != output_lds.elements
                || rust_execution_generic_extent_v1(tcx, instance) != Some(input_lds.elements)
                || !rust_same_kernel_brand_v1(input.kernel_brand, input_lds.kernel_brand)
                || !rust_same_kernel_brand_v1(input.kernel_brand, output_workgroup.kernel_brand)
                || !rust_same_kernel_brand_v1(input.kernel_brand, output_lds.kernel_brand)
                || input.epoch != input_lds.epoch
                || !rust_execution_epoch_transition_v1(tcx, input.epoch, output_workgroup.epoch)
                || output_workgroup.epoch != output_lds.epoch
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "LDS publish substituted state, extent, element, brand, or epoch transition",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::LdsPublish {
                    input_workgroup: abi.source_input_types()[0],
                    input_lds: abi.source_input_types()[1],
                    output_lds: type_id(output_lds.ty)?,
                    transition: abi.source_output_type(),
                    element: type_id(input_lds.element)?,
                    elements: input_lds.elements,
                },
                root,
                kernel_contexts,
                input.kernel_brand,
                input.epoch,
                Some(output_workgroup.epoch),
                source_identity,
            )
        }
        Terminal::AsyncCopy => {
            let workgroup = rust_inputs
                .first()
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_capability_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("async-copy workgroup"))?;
            let source = rust_inputs
                .get(1)
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_capability_memory_view_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("async-copy source"))?;
            let destination = rust_workgroup_lds_v1(tcx, rust_inputs[3])
                .ok_or_else(|| body_owner_table_mismatch_v1("async-copy destination"))?;
            let pending = rust_pending_async_copy_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("async-copy output"))?;
            if source.role != RustCapabilityMemoryRoleV1::ReadOnly
                || !matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize))
                || destination.state != RustExecutionLdsStateV1::Uninitialized
                || source.element != destination.element
                || destination.element != pending.element
                || destination.elements != pending.elements
                || rust_execution_generic_extent_v1(tcx, instance) != Some(destination.elements)
                || !rust_same_kernel_brand_v1(workgroup.kernel_brand, source.brand)
                || !rust_same_kernel_brand_v1(workgroup.kernel_brand, destination.kernel_brand)
                || !rust_same_kernel_brand_v1(workgroup.kernel_brand, pending.kernel_brand)
                || workgroup.epoch != destination.epoch
                || workgroup.epoch != pending.epoch
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "async copy substituted source role, extent, element, brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::AsyncCopy {
                    workgroup: abi.source_input_types()[0],
                    source_reference: abi.source_input_types()[1],
                    source: pointer_pointee_v1(types, abi.source_input_types()[1])?,
                    index: abi.source_input_types()[2],
                    destination: abi.source_input_types()[3],
                    pending: abi.source_output_type(),
                    element: type_id(destination.element)?,
                    elements: destination.elements,
                },
                root,
                kernel_contexts,
                workgroup.kernel_brand,
                workgroup.epoch,
                None,
                source_identity,
            )
        }
        Terminal::AsyncWait => {
            let input = rust_workgroup_capability_v1(tcx, rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("async-wait workgroup"))?;
            let pending = rust_pending_async_copy_v1(tcx, rust_inputs[1])
                .ok_or_else(|| body_owner_table_mismatch_v1("async-wait pending state"))?;
            let output = rust_tuple_fields_v1(rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("async-wait output tuple"))?;
            let [output_workgroup_ty, output_lds_ty] = output.as_slice() else {
                return Err(body_owner_table_mismatch_v1("async-wait output tuple"));
            };
            let output_workgroup = rust_workgroup_capability_v1(tcx, *output_workgroup_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("async-wait output workgroup"))?;
            let output_lds = rust_workgroup_lds_v1(tcx, *output_lds_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("async-wait output LDS"))?;
            if output_lds.state != RustExecutionLdsStateV1::Published
                || pending.element != output_lds.element
                || pending.elements != output_lds.elements
                || rust_execution_generic_extent_v1(tcx, instance) != Some(pending.elements)
                || !rust_same_kernel_brand_v1(input.kernel_brand, pending.kernel_brand)
                || !rust_same_kernel_brand_v1(input.kernel_brand, output_workgroup.kernel_brand)
                || !rust_same_kernel_brand_v1(input.kernel_brand, output_lds.kernel_brand)
                || input.epoch != pending.epoch
                || !rust_execution_epoch_transition_v1(tcx, input.epoch, output_workgroup.epoch)
                || output_workgroup.epoch != output_lds.epoch
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "async wait substituted pending state, extent, element, brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::AsyncWait {
                    input_workgroup: abi.source_input_types()[0],
                    pending: abi.source_input_types()[1],
                    output_lds: type_id(output_lds.ty)?,
                    transition: abi.source_output_type(),
                    element: type_id(pending.element)?,
                    elements: pending.elements,
                },
                root,
                kernel_contexts,
                input.kernel_brand,
                input.epoch,
                Some(output_workgroup.epoch),
                source_identity,
            )
        }
        Terminal::WorkgroupReduceSum
        | Terminal::WorkgroupInclusiveScanSum
        | Terminal::WorkgroupExclusiveScanSum => {
            let input = rust_workgroup_capability_v1(tcx, rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup collective input"))?;
            let scratch = rust_workgroup_lds_v1(tcx, rust_inputs[1])
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup collective scratch"))?;
            let output = rust_tuple_fields_v1(rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("workgroup collective output"))?;
            let [output_workgroup_ty, output_scratch_ty, output_value] = output.as_slice() else {
                return Err(body_owner_table_mismatch_v1("workgroup collective output"));
            };
            let output_workgroup = rust_workgroup_capability_v1(tcx, *output_workgroup_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("collective output workgroup"))?;
            let output_scratch = rust_workgroup_lds_v1(tcx, *output_scratch_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("collective output scratch"))?;
            if scratch.state != RustExecutionLdsStateV1::Uninitialized
                || output_scratch.state != RustExecutionLdsStateV1::Uninitialized
                || !rust_supported_collective_scalar_v1(scratch.element)
                || rust_inputs[2] != scratch.element
                || *output_value != scratch.element
                || scratch.element != output_scratch.element
                || scratch.elements != output_scratch.elements
                || rust_execution_generic_extent_v1(tcx, instance) != Some(scratch.elements)
                || !rust_same_kernel_brand_v1(input.kernel_brand, scratch.kernel_brand)
                || !rust_same_kernel_brand_v1(input.kernel_brand, output_workgroup.kernel_brand)
                || !rust_same_kernel_brand_v1(input.kernel_brand, output_scratch.kernel_brand)
                || input.epoch != scratch.epoch
                || !rust_execution_epoch_transition_v1(tcx, input.epoch, output_workgroup.epoch)
                || output_workgroup.epoch != output_scratch.epoch
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "workgroup collective substituted kind, participants, scratch, extent, brand, or epoch",
                ));
            }
            let kind = match terminal {
                Terminal::WorkgroupReduceSum => SemanticExecutionCollectiveKindV1::ReduceSum,
                Terminal::WorkgroupInclusiveScanSum => {
                    SemanticExecutionCollectiveKindV1::InclusiveScanSum
                }
                Terminal::WorkgroupExclusiveScanSum => {
                    SemanticExecutionCollectiveKindV1::ExclusiveScanSum
                }
                _ => unreachable!(),
            };
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::WorkgroupCollective {
                    kind,
                    input_workgroup: abi.source_input_types()[0],
                    scratch: abi.source_input_types()[1],
                    element: type_id(scratch.element)?,
                    transition: abi.source_output_type(),
                    elements: scratch.elements,
                },
                root,
                kernel_contexts,
                input.kernel_brand,
                input.epoch,
                Some(output_workgroup.epoch),
                source_identity,
            )
        }
        Terminal::BindGlobalAtomicLocation => {
            let workgroup = rust_inputs
                .first()
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_capability_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("global-atomic workgroup"))?;
            let view = rust_inputs
                .get(1)
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_capability_memory_view_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("global-atomic view"))?;
            let RustCapabilityMemoryRoleV1::AtomicReadWrite { scope } = view.role else {
                return Err(body_owner_table_mismatch_v1("global-atomic view role"));
            };
            let location_ty = rust_option_payload_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("global-atomic output"))?;
            let location = rust_scoped_atomic_v1(tcx, location_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("global-atomic location"))?;
            let (generic_scope, semantic_scope) = rust_execution_generic_scope_v1(tcx, instance)
                .ok_or_else(|| body_owner_table_mismatch_v1("global-atomic scope"))?;
            if !matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize))
                || !rust_supported_atomic_scalar_v1(view.element)
                || view.element != location.element
                || scope != generic_scope
                || scope != location.scope
                || !rust_same_kernel_brand_v1(workgroup.kernel_brand, view.brand)
                || !rust_same_kernel_brand_v1(workgroup.kernel_brand, location.kernel_brand)
                || workgroup.epoch != location.epoch
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "global atomic location substituted view role, scope, element, brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::Atomic {
                    kind: SemanticExecutionAtomicKindV1::BindGlobalLocation,
                    authority: abi.source_input_types()[0],
                    location_input: abi.source_input_types()[1],
                    location: type_id(location.ty)?,
                    element: type_id(location.element)?,
                    operand: Some(abi.source_input_types()[2]),
                    replacement: None,
                    result: abi.source_output_type(),
                    address_space: location.address_space,
                    scope: semantic_scope,
                    success: None,
                    failure: None,
                },
                root,
                kernel_contexts,
                workgroup.kernel_brand,
                workgroup.epoch,
                None,
                source_identity,
            )
        }
        Terminal::AtomicLoad
        | Terminal::AtomicStore
        | Terminal::AtomicFetchAdd
        | Terminal::AtomicCompareExchange => {
            let workgroup = rust_inputs
                .first()
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_capability_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("scoped atomic workgroup"))?;
            let location = rust_inputs
                .get(1)
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_scoped_atomic_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("scoped atomic location"))?;
            let (generic_scope, semantic_scope) = rust_execution_generic_scope_v1(tcx, instance)
                .ok_or_else(|| body_owner_table_mismatch_v1("scoped atomic scope"))?;
            let orders = rust_execution_atomic_orders_v1(tcx, instance);
            let (kind, success, failure, value_contract) = match terminal {
                Terminal::AtomicLoad => (
                    SemanticExecutionAtomicKindV1::Load,
                    orders.first().copied(),
                    None,
                    rust_inputs.len() == 2 && rust_output == location.element,
                ),
                Terminal::AtomicStore => (
                    SemanticExecutionAtomicKindV1::Store,
                    orders.first().copied(),
                    None,
                    rust_inputs.get(2).copied() == Some(location.element)
                        && rust_unit_v1(rust_output),
                ),
                Terminal::AtomicFetchAdd => (
                    SemanticExecutionAtomicKindV1::FetchAdd,
                    orders.first().copied(),
                    None,
                    rust_inputs.get(2).copied() == Some(location.element)
                        && rust_output == location.element,
                ),
                Terminal::AtomicCompareExchange => {
                    let result = rust_result_payloads_v1(tcx, rust_output);
                    (
                        SemanticExecutionAtomicKindV1::CompareExchange,
                        orders.first().copied(),
                        orders.get(1).copied(),
                        rust_inputs.get(2).copied() == Some(location.element)
                            && rust_inputs.get(3).copied() == Some(location.element)
                            && result == Some((location.element, location.element)),
                    )
                }
                _ => unreachable!(),
            };
            let expected_orders = if terminal == Terminal::AtomicCompareExchange {
                2
            } else {
                1
            };
            if !rust_supported_atomic_scalar_v1(location.element)
                || location.scope != generic_scope
                || !rust_same_kernel_brand_v1(workgroup.kernel_brand, location.kernel_brand)
                || workgroup.epoch != location.epoch
                || orders.len() != expected_orders
                || !value_contract
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "scoped atomic substituted operation, type, scope, order, brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::Atomic {
                    kind,
                    authority: abi.source_input_types()[0],
                    location_input: abi.source_input_types()[1],
                    location: type_id(location.ty)?,
                    element: type_id(location.element)?,
                    operand: abi.source_input_types().get(2).copied(),
                    replacement: abi.source_input_types().get(3).copied(),
                    result: abi.source_output_type(),
                    address_space: location.address_space,
                    scope: semantic_scope,
                    success,
                    failure,
                },
                root,
                kernel_contexts,
                workgroup.kernel_brand,
                workgroup.epoch,
                None,
                source_identity,
            )
        }
        Terminal::SubgroupFence => {
            let subgroup = rust_inputs
                .first()
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_subgroup_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup fence receiver"))?;
            let epoch = rust_inputs
                .get(1)
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_epoch_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup fence epoch"))?;
            let semantics = rust_execution_semantics_v1(tcx, instance)
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup fence semantics"))?;
            if !rust_unit_v1(rust_output)
                || !rust_same_kernel_brand_v1(subgroup.kernel_brand, epoch.kernel_brand)
                || subgroup.epoch != epoch.epoch
                || rust_execution_generic_width_v1(tcx, instance) != Some(subgroup.width)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "subgroup fence substituted width, workgroup brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::SubgroupFence {
                    semantics,
                    subgroup_reference: abi.source_input_types()[0],
                    subgroup: pointer_pointee_v1(types, abi.source_input_types()[0])?,
                    epoch: abi.source_input_types()[1],
                    result: abi.source_output_type(),
                    width: subgroup.width,
                },
                root,
                kernel_contexts,
                subgroup.kernel_brand,
                subgroup.epoch,
                None,
                source_identity,
            )
        }
        Terminal::SubgroupReduceSum | Terminal::SubgroupInclusiveScanSum => {
            let subgroup = rust_inputs
                .first()
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_subgroup_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup collective receiver"))?;
            let epoch = rust_inputs
                .get(1)
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_epoch_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup collective epoch"))?;
            let element = rust_inputs[2];
            if !rust_supported_collective_scalar_v1(element)
                || rust_output != element
                || !rust_same_kernel_brand_v1(subgroup.kernel_brand, epoch.kernel_brand)
                || subgroup.epoch != epoch.epoch
                || rust_execution_generic_width_v1(tcx, instance) != Some(subgroup.width)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "subgroup collective substituted kind, type, width, brand, or epoch",
                ));
            }
            let kind = match terminal {
                Terminal::SubgroupReduceSum => SemanticExecutionCollectiveKindV1::ReduceSum,
                Terminal::SubgroupInclusiveScanSum => {
                    SemanticExecutionCollectiveKindV1::InclusiveScanSum
                }
                _ => unreachable!(),
            };
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::SubgroupCollective {
                    kind,
                    subgroup_reference: abi.source_input_types()[0],
                    subgroup: pointer_pointee_v1(types, abi.source_input_types()[0])?,
                    epoch: abi.source_input_types()[1],
                    element: type_id(element)?,
                    width: subgroup.width,
                },
                root,
                kernel_contexts,
                subgroup.kernel_brand,
                subgroup.epoch,
                None,
                source_identity,
            )
        }
        Terminal::MatrixAccess => {
            let subgroup = rust_inputs
                .first()
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_subgroup_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("matrix-access subgroup"))?;
            let epoch = rust_inputs
                .get(1)
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_epoch_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("matrix-access epoch"))?;
            let matrix = rust_matrix_capability_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("matrix-access output"))?;
            if matrix.width != 64
                || subgroup.width != matrix.width
                || !rust_same_kernel_brand_v1(subgroup.kernel_brand, epoch.kernel_brand)
                || !rust_same_kernel_brand_v1(subgroup.kernel_brand, matrix.kernel_brand)
                || subgroup.epoch != epoch.epoch
                || subgroup.epoch != matrix.epoch
                || rust_execution_generic_width_v1(tcx, instance) != Some(subgroup.width)
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "matrix access substituted matrix brand, width, workgroup brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::MatrixAccess {
                    subgroup: abi.source_input_types()[0],
                    epoch: abi.source_input_types()[1],
                    matrix: type_id(matrix.ty)?,
                    subgroup_brand: rustc_type_identity_v1(tcx, matrix.subgroup_brand),
                    width: matrix.width,
                },
                root,
                kernel_contexts,
                subgroup.kernel_brand,
                subgroup.epoch,
                None,
                source_identity,
            )
        }
        Terminal::LdsInitializeByInvocation => {
            let input = rust_workgroup_lds_v1(tcx, rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("LDS initialize input"))?;
            let workgroup = rust_inputs
                .get(1)
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_capability_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("LDS initialize workgroup"))?;
            let output = rust_workgroup_lds_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("LDS initialize output"))?;
            if input.state != RustExecutionLdsStateV1::Uninitialized
                || output.state != RustExecutionLdsStateV1::InvocationInitialized
                || input.element != rust_inputs[2]
                || input.element != output.element
                || input.elements != output.elements
                || rust_execution_generic_extent_v1(tcx, instance) != Some(input.elements)
                || !rust_same_kernel_brand_v1(input.kernel_brand, workgroup.kernel_brand)
                || !rust_same_kernel_brand_v1(input.kernel_brand, output.kernel_brand)
                || input.epoch != workgroup.epoch
                || input.epoch != output.epoch
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "LDS initialization substituted state, extent, element, brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::LdsInitializeByInvocation {
                    input_lds: abi.source_input_types()[0],
                    workgroup: abi.source_input_types()[1],
                    output_lds: abi.source_output_type(),
                    element: type_id(input.element)?,
                    elements: input.elements,
                },
                root,
                kernel_contexts,
                input.kernel_brand,
                input.epoch,
                None,
                source_identity,
            )
        }
        Terminal::LdsReadPublished => {
            let lds = rust_inputs
                .first()
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_lds_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("published LDS read input"))?;
            let workgroup = rust_inputs
                .get(1)
                .and_then(|ty| rust_shared_reference_v1(*ty))
                .and_then(|ty| rust_workgroup_capability_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("published LDS read workgroup"))?;
            let payload = rust_option_payload_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("published LDS read output"))?;
            if lds.state != RustExecutionLdsStateV1::Published
                || lds.element != payload
                || !matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize))
                || rust_execution_generic_extent_v1(tcx, instance) != Some(lds.elements)
                || !rust_same_kernel_brand_v1(lds.kernel_brand, workgroup.kernel_brand)
                || lds.epoch != workgroup.epoch
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "published LDS read substituted state, extent, element, brand, or epoch",
                ));
            }
            execution_capability_operation_v1(
                tcx,
                abi,
                SemanticExecutionCapabilityOperationV1::LdsReadPublished {
                    lds_reference: abi.source_input_types()[0],
                    lds: pointer_pointee_v1(types, abi.source_input_types()[0])?,
                    workgroup: abi.source_input_types()[1],
                    index: abi.source_input_types()[2],
                    option: abi.source_output_type(),
                    element: type_id(lds.element)?,
                    elements: lds.elements,
                },
                root,
                kernel_contexts,
                lds.kernel_brand,
                lds.epoch,
                None,
                source_identity,
            )
        }
        Terminal::GlobalBindExclusiveReadWrite
        | Terminal::GlobalExclusiveLoad
        | Terminal::GlobalExclusiveStore
        | Terminal::GlobalStoreBlock => Err(body_owner_table_mismatch_v1(
            "typed-global terminal entered the execution-capability decoder",
        )),
    }
}

fn terminal_operation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    expansion: crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    capability_root: Option<&AuthenticatedProductionKernelContextRootV1>,
    source_identity: SemanticFunctionIdentityV1,
    kernel_contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
    let inputs = abi.source_input_types();
    let output = abi.source_output_type();
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let rust_inputs = signature.inputs();
    let rust_output = signature.output();
    match expansion {
        ProductionTerminalExpansionV1::Execution(terminal)
            if !terminal.is_typed_global_memory() => execution_terminal_operation_v1(
            tcx,
            instance,
            terminal,
            abi,
            types,
            capability_root,
            source_identity,
            kernel_contexts,
        ),
        ProductionTerminalExpansionV1::KernelContextIssue
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && abi.canon_abi() == SemanticCanonAbiV1::Rust
                && abi.extern_abi() == SemanticExternAbiV1::Rust
                && !abi.c_variadic()
                && rust_kernel_context_axes_v1(tcx, rust_output).is_some()
                && output == abi.return_value().ty()
                && matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
                && abi.return_value().adjusted().is_none()
                && abi.return_value().pointee_override().is_none()
                && abi.arguments().is_empty()
                && abi.hidden_arguments().is_empty()
                && semantic_exact_inhabited_aggregate_zst_v1(types, output) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context: output })
        }
        ProductionTerminalExpansionV1::CapabilityGlobalBindReadOnly => {
            require_capability_memory_terminal_abi_v1(
                tcx,
                abi,
                types,
                rust_inputs,
                rust_output,
                &[
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                ],
            )?;
            let root =
                capability_root.ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global read-only binding lacks authenticated root custody",
                ))?;
            let context_ty = rust_inputs
                .first()
                .and_then(|ty| rust_reference_with_mutability_v1(*ty, rustc_hir::Mutability::Not))
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global context borrow"))?;
            let context_axes = rust_kernel_context_axes_v1(tcx, context_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global context type"))?;
            let physical_element = rust_inputs
                .get(1)
                .and_then(|ty| rust_shared_slice_element_v1(*ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global shared slice"))?;
            let view = rust_capability_memory_view_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global read-only view"))?;
            if view.role != RustCapabilityMemoryRoleV1::ReadOnly
                || view.element != physical_element
                || (view.kernel, view.target, view.launch) != context_axes
                || *rustc_type_identity_v1(tcx, view.kernel).as_bytes()
                    != root.kernel_marker_identity
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global read-only view does not match its authenticated context brand",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly {
                    context: pointer_pointee_v1(types, inputs[0])?,
                    physical: inputs[1],
                    view: output,
                    element: semantic_type_for_rust_v1(tcx, types, view.element)?,
                    contract: SemanticCapabilityMemoryContractV1::global_read_only(),
                    provenance: capability_memory_provenance_v1(root, kernel_contexts)?,
                    source_identity,
                },
            )
        }
        ProductionTerminalExpansionV1::Execution(
            crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1::GlobalBindExclusiveReadWrite,
        ) => {
            require_capability_memory_terminal_abi_v1(
                tcx,
                abi,
                types,
                rust_inputs,
                rust_output,
                &[
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                ],
            )?;
            let root =
                capability_root.ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global exclusive binding lacks authenticated root custody",
                ))?;
            let context_ty = rust_inputs
                .first()
                .and_then(|ty| rust_reference_with_mutability_v1(*ty, rustc_hir::Mutability::Not))
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global context borrow"))?;
            let context_axes = rust_kernel_context_axes_v1(tcx, context_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global context type"))?;
            let physical_element = rust_inputs
                .get(1)
                .and_then(|ty| rust_mutable_slice_element_v1(*ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global exclusive slice"))?;
            let view = rust_capability_memory_view_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global exclusive view"))?;
            if view.role != RustCapabilityMemoryRoleV1::ExclusiveReadWrite
                || view.element != physical_element
                || (view.kernel, view.target, view.launch) != context_axes
                || *rustc_type_identity_v1(tcx, view.kernel).as_bytes()
                    != root.kernel_marker_identity
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global exclusive view does not match its context, allocation, or brand",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindExclusiveReadWrite {
                    context: pointer_pointee_v1(types, inputs[0])?,
                    physical: inputs[1],
                    view: output,
                    element: semantic_type_for_rust_v1(tcx, types, view.element)?,
                    contract: SemanticCapabilityMemoryContractV1::global_exclusive_read_write(),
                    provenance: capability_memory_provenance_v1(root, kernel_contexts)?,
                    source_identity,
                },
            )
        }
        ProductionTerminalExpansionV1::CapabilityGlobalBindDisjointWrite => {
            require_capability_memory_terminal_abi_v1(
                tcx,
                abi,
                types,
                rust_inputs,
                rust_output,
                &[
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                ],
            )?;
            let root =
                capability_root.ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global disjoint binding lacks authenticated root custody",
                ))?;
            let context_ty = rust_inputs
                .first()
                .and_then(|ty| rust_reference_with_mutability_v1(*ty, rustc_hir::Mutability::Not))
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global context borrow"))?;
            let context_axes = rust_kernel_context_axes_v1(tcx, context_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global context type"))?;
            let physical = rust_inputs
                .get(1)
                .copied()
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global physical storage"))?;
            let (physical_element, physical_mapping) =
                rust_write_only_disjoint_slice_v1(tcx, physical).ok_or_else(|| {
                    body_owner_table_mismatch_v1("typed-global write-only disjoint storage")
                })?;
            let physical_arguments = rust_trusted_adt_type_arguments_v1(
                tcx,
                physical,
                TrustedDeviceItem::WriteOnlyDisjointSlice,
            )
            .ok_or_else(|| {
                body_owner_table_mismatch_v1("typed-global physical storage identity")
            })?;
            let physical_index_space = physical_arguments
                .get(1)
                .copied()
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global physical index space"))?;
            let view = rust_capability_memory_view_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global disjoint view"))?;
            let RustCapabilityMemoryRoleV1::DisjointWrite {
                index_space,
                mapping,
            } = view.role
            else {
                return Err(body_owner_table_mismatch_v1("typed-global disjoint role"));
            };
            if view.element != physical_element
                || index_space != physical_index_space
                || mapping != physical_mapping
                || (view.kernel, view.target, view.launch) != context_axes
                || *rustc_type_identity_v1(tcx, view.kernel).as_bytes()
                    != root.kernel_marker_identity
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global disjoint view does not match context, storage, or index brand",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindDisjointWrite {
                    context: pointer_pointee_v1(types, inputs[0])?,
                    physical: inputs[1],
                    view: output,
                    element: semantic_type_for_rust_v1(tcx, types, view.element)?,
                    contract: SemanticCapabilityMemoryContractV1::global_disjoint_write(
                        semantic_type_for_rust_v1(tcx, types, index_space)?,
                        mapping,
                    ),
                    provenance: capability_memory_provenance_v1(root, kernel_contexts)?,
                    source_identity,
                },
            )
        }
        ProductionTerminalExpansionV1::CapabilityGlobalLoad => {
            require_capability_memory_terminal_abi_v1(
                tcx,
                abi,
                types,
                rust_inputs,
                rust_output,
                &[
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ],
            )?;
            let root =
                capability_root.ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global load lacks authenticated root custody",
                ))?;
            let view_ty = rust_inputs
                .first()
                .and_then(|ty| rust_reference_with_mutability_v1(*ty, rustc_hir::Mutability::Not))
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global load receiver"))?;
            let view = rust_capability_memory_view_v1(tcx, view_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global load view"))?;
            let payload = rust_option_payload_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global load Option output"))?;
            if view.role != RustCapabilityMemoryRoleV1::ReadOnly
                || view.element != payload
                || rust_inputs
                    .get(1)
                    .is_none_or(|index| !matches!(index.kind(), TyKind::Uint(UintTy::Usize)))
                || *rustc_type_identity_v1(tcx, view.kernel).as_bytes()
                    != root.kernel_marker_identity
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global load does not match its bound view brand or value contract",
                ));
            }
            Ok(SemanticCompilerIntrinsicOperationV1::CapabilityGlobalLoad {
                view: pointer_pointee_v1(types, inputs[0])?,
                option: output,
                element: semantic_type_for_rust_v1(tcx, types, view.element)?,
                contract: SemanticCapabilityMemoryContractV1::global_read_only(),
                provenance: capability_memory_provenance_v1(root, kernel_contexts)?,
                source_identity,
            })
        }
        ProductionTerminalExpansionV1::Execution(
            crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1::GlobalExclusiveLoad,
        ) => {
            require_capability_memory_terminal_abi_v1(
                tcx,
                abi,
                types,
                rust_inputs,
                rust_output,
                &[
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ],
            )?;
            let root =
                capability_root.ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global exclusive load lacks authenticated root custody",
                ))?;
            let view_ty = rust_inputs
                .first()
                .and_then(|ty| rust_reference_with_mutability_v1(*ty, rustc_hir::Mutability::Not))
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global exclusive load receiver"))?;
            let view = rust_capability_memory_view_v1(tcx, view_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global exclusive load view"))?;
            let payload = rust_option_payload_v1(tcx, rust_output).ok_or_else(|| {
                body_owner_table_mismatch_v1("typed-global exclusive load Option output")
            })?;
            if view.role != RustCapabilityMemoryRoleV1::ExclusiveReadWrite
                || view.element != payload
                || rust_inputs
                    .get(1)
                    .is_none_or(|index| !matches!(index.kind(), TyKind::Uint(UintTy::Usize)))
                || *rustc_type_identity_v1(tcx, view.kernel).as_bytes()
                    != root.kernel_marker_identity
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global exclusive load substituted its role, element, or root brand",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalExclusiveLoad {
                    view: pointer_pointee_v1(types, inputs[0])?,
                    option: output,
                    element: semantic_type_for_rust_v1(tcx, types, view.element)?,
                    contract: SemanticCapabilityMemoryContractV1::global_exclusive_read_write(),
                    provenance: capability_memory_provenance_v1(root, kernel_contexts)?,
                    source_identity,
                },
            )
        }
        ProductionTerminalExpansionV1::CapabilityGlobalStore => {
            require_capability_memory_terminal_abi_v1(
                tcx,
                abi,
                types,
                rust_inputs,
                rust_output,
                &[
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ],
            )?;
            let root =
                capability_root.ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global store lacks authenticated root custody",
                ))?;
            let view_ty = rust_inputs
                .first()
                .and_then(|ty| rust_reference_with_mutability_v1(*ty, rustc_hir::Mutability::Mut))
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global store receiver"))?;
            let view = rust_capability_memory_view_v1(tcx, view_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global store view"))?;
            let RustCapabilityMemoryRoleV1::DisjointWrite {
                index_space,
                mapping,
            } = view.role
            else {
                return Err(body_owner_table_mismatch_v1("typed-global store role"));
            };
            let witness = rust_inputs
                .get(1)
                .copied()
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global store witness"))?;
            let (witness_space, witness_brand) =
                rust_branded_index_v1(tcx, witness, TrustedDeviceItem::DisjointIndex)
                    .ok_or_else(|| body_owner_table_mismatch_v1("typed-global store witness"))?;
            if witness_space != index_space
                || witness_brand != view.brand.ty
                || rust_disjoint_index_space_v1(tcx, witness_space) != Some(mapping)
                || rust_inputs.get(2).copied() != Some(view.element)
                || !matches!(rust_output.kind(), TyKind::Bool)
                || *rustc_type_identity_v1(tcx, view.kernel).as_bytes()
                    != root.kernel_marker_identity
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global store does not match its view, witness, element, or root brand",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalStore {
                    view: pointer_pointee_v1(types, inputs[0])?,
                    witness: inputs[1],
                    element: inputs[2],
                    result: output,
                    contract: SemanticCapabilityMemoryContractV1::global_disjoint_write(
                        semantic_type_for_rust_v1(tcx, types, index_space)?,
                        mapping,
                    ),
                    provenance: capability_memory_provenance_v1(root, kernel_contexts)?,
                    source_identity,
                },
            )
        }
        ProductionTerminalExpansionV1::Execution(
            crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1::GlobalExclusiveStore,
        ) => {
            require_capability_memory_terminal_abi_v1(
                tcx,
                abi,
                types,
                rust_inputs,
                rust_output,
                &[
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ],
            )?;
            let root =
                capability_root.ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global exclusive store lacks authenticated root custody",
                ))?;
            let view_ty = rust_inputs
                .first()
                .and_then(|ty| rust_reference_with_mutability_v1(*ty, rustc_hir::Mutability::Mut))
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global exclusive store receiver"))?;
            let view = rust_capability_memory_view_v1(tcx, view_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global exclusive store view"))?;
            if view.role != RustCapabilityMemoryRoleV1::ExclusiveReadWrite
                || rust_inputs
                    .get(1)
                    .is_none_or(|index| !matches!(index.kind(), TyKind::Uint(UintTy::Usize)))
                || rust_inputs.get(2).copied() != Some(view.element)
                || !matches!(rust_output.kind(), TyKind::Bool)
                || *rustc_type_identity_v1(tcx, view.kernel).as_bytes()
                    != root.kernel_marker_identity
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global exclusive store substituted its role, index, element, or root brand",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalExclusiveStore {
                    view: pointer_pointee_v1(types, inputs[0])?,
                    index: inputs[1],
                    element: inputs[2],
                    result: output,
                    contract: SemanticCapabilityMemoryContractV1::global_exclusive_read_write(),
                    provenance: capability_memory_provenance_v1(root, kernel_contexts)?,
                    source_identity,
                },
            )
        }
        ProductionTerminalExpansionV1::Execution(
            crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1::GlobalStoreBlock,
        ) => {
            require_capability_memory_terminal_abi_v1(
                tcx,
                abi,
                types,
                rust_inputs,
                rust_output,
                &[
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                    SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                    SemanticSourceArgumentOwnershipV1::ByValue,
                ],
            )?;
            let root =
                capability_root.ok_or(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global blocked store lacks authenticated root custody",
                ))?;
            let view_ty = rust_inputs
                .first()
                .and_then(|ty| rust_reference_with_mutability_v1(*ty, rustc_hir::Mutability::Mut))
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global blocked store receiver"))?;
            let view = rust_capability_memory_view_v1(tcx, view_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global blocked store view"))?;
            let RustCapabilityMemoryRoleV1::DisjointWrite {
                index_space,
                mapping,
            } = view.role
            else {
                return Err(body_owner_table_mismatch_v1("typed-global blocked store role"));
            };
            let witness_ty = rust_inputs
                .get(1)
                .and_then(|ty| rust_reference_with_mutability_v1(*ty, rustc_hir::Mutability::Not))
                .ok_or_else(|| body_owner_table_mismatch_v1("typed-global blocked witness borrow"))?;
            let (witness_mapping, lanes_per_block, elements_per_lane, witness_brand) =
                rust_disjoint_block_contract_v1(tcx, witness_ty).ok_or_else(|| {
                    body_owner_table_mismatch_v1("typed-global blocked witness contract")
                })?;
            if mapping != witness_mapping
                || rust_disjoint_index_space_v1(tcx, index_space) != Some(witness_mapping)
                || witness_brand != view.brand.ty
                || rust_inputs
                    .get(2)
                    .is_none_or(|component| !matches!(component.kind(), TyKind::Uint(UintTy::Usize)))
                || rust_inputs.get(3).copied() != Some(view.element)
                || !matches!(rust_output.kind(), TyKind::Bool)
                || *rustc_type_identity_v1(tcx, view.kernel).as_bytes()
                    != root.kernel_marker_identity
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "typed-global blocked store substituted nominal mapping, geometry, brand, component, element, or root",
                ));
            }
            Ok(SemanticCompilerIntrinsicOperationV1::CapabilityGlobalStoreBlock {
                view: pointer_pointee_v1(types, inputs[0])?,
                witness: pointer_pointee_v1(types, inputs[1])?,
                component: inputs[2],
                element: inputs[3],
                result: output,
                contract: SemanticCapabilityMemoryContractV1::global_disjoint_write(
                    semantic_type_for_rust_v1(tcx, types, index_space)?,
                    mapping,
                ),
                lanes_per_block,
                elements_per_lane,
                provenance: capability_memory_provenance_v1(root, kernel_contexts)?,
                source_identity,
            })
        }
        ProductionTerminalExpansionV1::ThreadIndex(axis)
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && matches!(rust_output.kind(), TyKind::Uint(UintTy::U32)) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::ThreadIndex(axis))
        }
        ProductionTerminalExpansionV1::WorkgroupIndex(axis)
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && matches!(rust_output.kind(), TyKind::Uint(UintTy::U32)) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::WorkgroupIndex(axis))
        }
        ProductionTerminalExpansionV1::WorkgroupDimension(axis)
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && matches!(rust_output.kind(), TyKind::Uint(UintTy::U32)) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::WorkgroupDimension(
                axis,
            ))
        }
        ProductionTerminalExpansionV1::GridDimension(axis)
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && matches!(rust_output.kind(), TyKind::Uint(UintTy::U32)) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::GridDimension(axis))
        }
        ProductionTerminalExpansionV1::WorkgroupLdsScopeCurrent
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && abi.canon_abi() == SemanticCanonAbiV1::Rust
                && abi.extern_abi() == SemanticExternAbiV1::Rust
                && !abi.c_variadic()
                && rust_is_trusted_adt_v1(
                    tcx,
                    rust_output,
                    TrustedDeviceItem::WorkgroupLdsScope,
                )
                && output == abi.return_value().ty()
                && matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
                && abi.return_value().adjusted().is_none()
                && abi.return_value().pointee_override().is_none()
                && abi.arguments().is_empty()
                && abi.hidden_arguments().is_empty()
                && semantic_exact_inhabited_aggregate_zst_v1(types, output) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::WorkgroupLdsScopeCurrent { scope: output })
        }
        ProductionTerminalExpansionV1::DynamicLdsExactCurrent
            if inputs.len() == 1
                && rust_inputs.len() == 1
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::WorkgroupLdsScope)
                })
                && rust_dynamic_lds_scalar_element_v1(tcx, rust_output).is_some() =>
        {
            let elements = single_const_u64_v1(instance)
                .filter(|elements| *elements != 0 && *elements <= u64::from(u32::MAX))
                .ok_or_else(|| body_owner_table_mismatch_v1("exact LDS element count"))?;
            let scope = pointer_pointee_v1(types, inputs[0])?;
            let element_storage = dynamic_lds_element_storage_v1(types, output)?;
            let storage = types
                .get(element_storage.index() as usize)
                .ok_or_else(|| body_owner_table_mismatch_v1("exact LDS storage type"))?;
            let alignment = storage.layout().alignment_bytes();
            if !storage.layout().size_bytes().is_some_and(|size| size != 0)
                || alignment == 0
                || alignment > 16
                || !alignment.is_power_of_two()
            {
                return Err(body_owner_table_mismatch_v1(
                    "exact LDS storage size or alignment",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::DynamicLdsExactCurrent {
                    scope,
                    dynamic_lds: output,
                    element_storage,
                    elements,
                },
            )
        }
        ProductionTerminalExpansionV1::DynamicLdsIntoCollectiveRawParts
            if inputs.len() == 1
                && rust_inputs.len() == 1
                && rust_dynamic_lds_scalar_element_v1(tcx, rust_inputs[0])
                    .is_some_and(|element| rust_dynamic_lds_raw_parts_v1(rust_output, element)) =>
        {
            let dynamic_lds = inputs[0];
            let element_storage = dynamic_lds_element_storage_v1(types, dynamic_lds)?;
            let raw_pointer = tuple_field_v1(types, output, 0)?;
            let element = pointer_pointee_v1(types, raw_pointer)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::DynamicLdsIntoCollectiveRawParts {
                    dynamic_lds,
                    raw_parts: output,
                    element_storage,
                    element,
                },
            )
        }
        ProductionTerminalExpansionV1::WorkgroupPipelineCurrent
            if inputs.len() == 1
                && rust_inputs.len() == 1
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::WorkgroupLdsScope)
                })
                && rust_workgroup_pipeline_contract_v1(tcx, rust_output).is_some() =>
        {
            let (_, buffers, elements, prefetch_distance) =
                rust_workgroup_pipeline_contract_v1(tcx, rust_output)
                    .ok_or_else(|| body_owner_table_mismatch_v1("workgroup pipeline contract"))?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
                    scope: pointer_pointee_v1(types, inputs[0])?,
                    pipeline: output,
                    buffers,
                    elements,
                    prefetch_distance,
                },
            )
        }
        ProductionTerminalExpansionV1::WorkgroupPipelineStage
        | ProductionTerminalExpansionV1::WorkgroupPipelineCommit
        | ProductionTerminalExpansionV1::WorkgroupPipelineWait
        | ProductionTerminalExpansionV1::WorkgroupPipelineConsume
        | ProductionTerminalExpansionV1::WorkgroupPipelineDiscard
        | ProductionTerminalExpansionV1::WorkgroupPipelineRelease
            if inputs.len() == 2
                && rust_inputs.len() == 2
                && rust_reference_pointee_v1(rust_inputs[0])
                    .and_then(|ty| rust_workgroup_pipeline_contract_v1(tcx, ty))
                    .is_some()
                && matches!(rust_inputs[1].kind(), TyKind::Uint(UintTy::Usize))
                && matches!(rust_output.kind(), TyKind::Tuple(fields) if fields.is_empty()) =>
        {
            let event = match expansion {
                ProductionTerminalExpansionV1::WorkgroupPipelineStage => {
                    SemanticWorkgroupPipelineEventV1::Stage
                }
                ProductionTerminalExpansionV1::WorkgroupPipelineCommit => {
                    SemanticWorkgroupPipelineEventV1::Commit
                }
                ProductionTerminalExpansionV1::WorkgroupPipelineWait => {
                    SemanticWorkgroupPipelineEventV1::Wait
                }
                ProductionTerminalExpansionV1::WorkgroupPipelineConsume => {
                    SemanticWorkgroupPipelineEventV1::Consume
                }
                ProductionTerminalExpansionV1::WorkgroupPipelineDiscard => {
                    SemanticWorkgroupPipelineEventV1::Discard
                }
                ProductionTerminalExpansionV1::WorkgroupPipelineRelease => {
                    SemanticWorkgroupPipelineEventV1::Release
                }
                _ => unreachable!("guarded pipeline event expansion"),
            };
            Ok(
                SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineEvent {
                    pipeline: pointer_pointee_v1(types, inputs[0])?,
                    event,
                },
            )
        }
        ProductionTerminalExpansionV1::WorkgroupPipelineWrite
            if inputs.len() == 4
                && rust_inputs.len() == 4
                && rust_reference_pointee_v1(rust_inputs[0])
                    .and_then(|ty| rust_workgroup_pipeline_contract_v1(tcx, ty))
                    .is_some_and(|(element, ..)| element == rust_inputs[3])
                && matches!(rust_inputs[1].kind(), TyKind::Uint(UintTy::Usize))
                && matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize))
                && matches!(rust_output.kind(), TyKind::Tuple(fields) if fields.is_empty()) =>
        {
            Ok(
                SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite {
                    pipeline: pointer_pointee_v1(types, inputs[0])?,
                    element: inputs[3],
                },
            )
        }
        ProductionTerminalExpansionV1::WorkgroupPipelineRead
            if inputs.len() == 3
                && rust_inputs.len() == 3
                && rust_reference_pointee_v1(rust_inputs[0])
                    .and_then(|ty| rust_workgroup_pipeline_contract_v1(tcx, ty))
                    .is_some_and(|(element, ..)| element == rust_output)
                && matches!(rust_inputs[1].kind(), TyKind::Uint(UintTy::Usize))
                && matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize)) =>
        {
            Ok(
                SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead {
                    pipeline: pointer_pointee_v1(types, inputs[0])?,
                    element: output,
                },
            )
        }
        ProductionTerminalExpansionV1::Trap
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && matches!(rust_output.kind(), TyKind::Never) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::Trap)
        }
        ProductionTerminalExpansionV1::WorkgroupBarrier
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && matches!(rust_output.kind(), TyKind::Tuple(fields) if fields.is_empty()) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier)
        }
        ProductionTerminalExpansionV1::ColdPath
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && matches!(rust_output.kind(), TyKind::Tuple(fields) if fields.is_empty()) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::ColdPath)
        }
        ProductionTerminalExpansionV1::MathContextCurrent
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && rust_is_trusted_adt_v1(
                    tcx,
                    rust_output,
                    TrustedDeviceItem::DeviceMath(
                        dialect_amdgcn::DeviceMathDiagnosticItem::Context,
                    ),
                ) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::MathContextCurrent { context: output })
        }
        ProductionTerminalExpansionV1::MathF32(function)
            if inputs.len() == function.arity() + 1
                && rust_inputs.len() == function.arity() + 1
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(
                        tcx,
                        ty,
                        TrustedDeviceItem::DeviceMath(
                            dialect_amdgcn::DeviceMathDiagnosticItem::Context,
                        ),
                    )
                })
                && rust_inputs[1..]
                    .iter()
                    .all(|ty| matches!(ty.kind(), TyKind::Float(FloatTy::F32)))
                && matches!(rust_output.kind(), TyKind::Float(FloatTy::F32)) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::MathF32 {
                context: pointer_pointee_v1(types, inputs[0])?,
                function: semantic_f32_math_function_v1(function),
            })
        }
        ProductionTerminalExpansionV1::RustcFabsF32
            if inputs.len() == 1
                && rust_inputs.len() == 1
                && inputs[0] == output
                && semantic_f32_type_v1(types, output)
                && matches!(rust_inputs[0].kind(), TyKind::Float(FloatTy::F32))
                && matches!(rust_output.kind(), TyKind::Float(FloatTy::F32)) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::FabsF32)
        }
        ProductionTerminalExpansionV1::MemoryVolatileLoad
            if inputs.len() == 2
                && rust_inputs.len() == 2
                && rust_shared_slice_element_v1(rust_inputs[0]) == Some(rust_output)
                && matches!(rust_inputs[1].kind(), TyKind::Uint(UintTy::Usize))
                && rust_supported_volatile_load_scalar_v1(rust_output) =>
        {
            let slice = pointer_pointee_v1(types, inputs[0])?;
            let SemanticTypeShapeV1::Slice { element } = types
                .get(slice.index() as usize)
                .ok_or_else(|| body_owner_table_mismatch_v1("volatile-load slice type"))?
                .shape()
            else {
                return Err(body_owner_table_mismatch_v1("volatile-load slice type"));
            };
            if *element != output
                || !types
                    .get(inputs[1].index() as usize)
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape(),
                            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                                signed: false,
                                bits: 64,
                            })
                        )
                    })
            {
                return Err(body_owner_table_mismatch_v1(
                    "volatile-load element or index type",
                ));
            }
            Ok(SemanticCompilerIntrinsicOperationV1::MemoryVolatileLoad { element: *element })
        }
        ProductionTerminalExpansionV1::Bf16Conversion(conversion) => {
            if inputs.len() != 1 || rust_inputs.len() != 1 {
                return Err(body_owner_table_mismatch_v1("BF16 conversion arity"));
            }
            let input = inputs[0];
            let rust_input = rust_inputs[0];
            let rust_is_bf16 = |ty| {
                rust_is_trusted_adt_v1(
                    tcx,
                    ty,
                    TrustedDeviceItem::DeviceValue(DeviceValueDiagnosticItem::Bf16),
                )
            };
            let rust_is_u16 = |ty: Ty<'tcx>| matches!(ty.kind(), TyKind::Uint(UintTy::U16));
            let rust_is_f32 = |ty: Ty<'tcx>| matches!(ty.kind(), TyKind::Float(FloatTy::F32));
            let (kind, valid) = match conversion {
                ProductionBf16ConversionV1::FromBits => (
                    SemanticBf16ConversionKindV1::FromBits,
                    rust_is_u16(rust_input)
                        && rust_is_bf16(rust_output)
                        && semantic_u16_type_v1(types, input)
                        && semantic_bf16_storage_type_v1(types, output),
                ),
                ProductionBf16ConversionV1::ToBits => (
                    SemanticBf16ConversionKindV1::ToBits,
                    rust_is_bf16(rust_input)
                        && rust_is_u16(rust_output)
                        && semantic_bf16_storage_type_v1(types, input)
                        && semantic_u16_type_v1(types, output),
                ),
                ProductionBf16ConversionV1::FromF32RoundTiesEven => (
                    SemanticBf16ConversionKindV1::FromF32RoundTiesEven,
                    rust_is_f32(rust_input)
                        && rust_is_bf16(rust_output)
                        && semantic_f32_type_v1(types, input)
                        && semantic_bf16_storage_type_v1(types, output),
                ),
                ProductionBf16ConversionV1::ToF32 => (
                    SemanticBf16ConversionKindV1::ToF32,
                    rust_is_bf16(rust_input)
                        && rust_is_f32(rust_output)
                        && semantic_bf16_storage_type_v1(types, input)
                        && semantic_f32_type_v1(types, output),
                ),
            };
            if !valid {
                return Err(body_owner_table_mismatch_v1(
                    "authenticated BF16 conversion ABI",
                ));
            }
            Ok(SemanticCompilerIntrinsicOperationV1::Bf16Conversion {
                kind,
                input,
                output,
            })
        }
        ProductionTerminalExpansionV1::CollectiveContextCurrent
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && rust_is_trusted_adt_v1(
                    tcx,
                    rust_output,
                    TrustedDeviceItem::Gfx942CollectivesContext,
                ) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::CollectiveContextCurrent { context: output })
        }
        ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && rust_is_trusted_adt_v1(
                    tcx,
                    rust_output,
                    TrustedDeviceItem::WorkgroupCollectivesContext,
                ) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::CollectiveContextCurrent { context: output })
        }
        ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum
            if inputs.len() == 3
                && rust_inputs.len() == 3
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::WorkgroupCollectivesContext)
                })
                && rust_dynamic_lds_uninitialized_scalar_element_v1(tcx, rust_inputs[1])
                    == Some(rust_output)
                && rust_inputs[2] == rust_output
                && matches!(
                    rust_output.kind(),
                    TyKind::Int(IntTy::I32)
                        | TyKind::Uint(UintTy::U32)
                        | TyKind::Float(FloatTy::F32)
                ) =>
        {
            let context = pointer_pointee_v1(types, inputs[0])?;
            let dynamic_lds = inputs[1];
            let element_storage = dynamic_lds_element_storage_v1(types, dynamic_lds)?;
            if inputs[2] != output {
                return Err(body_owner_table_mismatch_v1(
                    "target-neutral workgroup reduction element",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupReduceSum {
                    context,
                    dynamic_lds,
                    element_storage,
                    element: output,
                },
            )
        }
        ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum
        | ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum
            if inputs.len() == 3
                && rust_inputs.len() == 3
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::WorkgroupCollectivesContext)
                })
                && rust_dynamic_lds_uninitialized_scalar_element_v1(tcx, rust_inputs[1])
                    == Some(rust_output)
                && rust_inputs[2] == rust_output
                && matches!(
                    rust_output.kind(),
                    TyKind::Int(IntTy::I32)
                        | TyKind::Uint(UintTy::U32)
                        | TyKind::Float(FloatTy::F32)
                ) =>
        {
            let context = pointer_pointee_v1(types, inputs[0])?;
            let dynamic_lds = inputs[1];
            let element_storage = dynamic_lds_element_storage_v1(types, dynamic_lds)?;
            if inputs[2] != output {
                return Err(body_owner_table_mismatch_v1(
                    "target-neutral workgroup scan element",
                ));
            }
            let kind = match expansion {
                ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum => {
                    SemanticWorkgroupScanKindV1::Inclusive
                }
                ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum => {
                    SemanticWorkgroupScanKindV1::Exclusive
                }
                _ => unreachable!("match arm admits only target-neutral workgroup scans"),
            };
            Ok(
                SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupScanSum {
                    context,
                    dynamic_lds,
                    element_storage,
                    element: output,
                    kind,
                },
            )
        }
        ProductionTerminalExpansionV1::WorkgroupReduceSum
            if inputs.len() == 4
                && rust_inputs.len() == 4
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::WorkgroupGroup)
                })
                && rust_reference_pointee_v1(rust_inputs[1]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::Gfx942CollectivesContext)
                })
                && matches!(
                    rust_inputs[2].kind(),
                    TyKind::Ref(_, _, rustc_hir::Mutability::Mut)
                )
                && rust_reference_pointee_v1(rust_inputs[2])
                    .and_then(|ty| rust_workgroup_collective_scratch_element_v1(tcx, ty))
                    == Some(rust_output)
                && rust_inputs[3] == rust_output
                && matches!(
                    rust_output.kind(),
                    TyKind::Int(IntTy::I32)
                        | TyKind::Uint(UintTy::U32)
                        | TyKind::Float(FloatTy::F32)
                ) =>
        {
            let workgroup = pointer_pointee_v1(types, inputs[0])?;
            let context = pointer_pointee_v1(types, inputs[1])?;
            let scratch = pointer_pointee_v1(types, inputs[2])?;
            let base = aggregate_field_v1(types, scratch, 0)?;
            if pointer_pointee_v1(types, base)? != output || inputs[3] != output {
                return Err(body_owner_table_mismatch_v1(
                    "workgroup reduction scratch element",
                ));
            }
            Ok(SemanticCompilerIntrinsicOperationV1::WorkgroupReduceSum {
                workgroup,
                context,
                scratch,
                element: output,
            })
        }
        ProductionTerminalExpansionV1::SubgroupReduceSumF32
        | ProductionTerminalExpansionV1::SubgroupReduceMaxF32
            if inputs.len() == 2
                && rust_inputs.len() == 2
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::Gfx942CollectivesContext)
                })
                && matches!(rust_inputs[1].kind(), TyKind::Float(FloatTy::F32))
                && matches!(rust_output.kind(), TyKind::Float(FloatTy::F32)) =>
        {
            let width = single_const_u32_v1(instance)
                .ok_or_else(|| body_owner_table_mismatch_v1("subgroup reduction width"))?;
            let kind = match expansion {
                ProductionTerminalExpansionV1::SubgroupReduceSumF32 => {
                    SemanticSubgroupReductionKindV1::Sum
                }
                ProductionTerminalExpansionV1::SubgroupReduceMaxF32 => {
                    SemanticSubgroupReductionKindV1::Maximum
                }
                _ => unreachable!("matched subgroup reduction expansion"),
            };
            Ok(SemanticCompilerIntrinsicOperationV1::SubgroupReduceF32 {
                context: pointer_pointee_v1(types, inputs[0])?,
                width,
                kind,
            })
        }
        ProductionTerminalExpansionV1::Gfx950SubgroupCurrent
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && rust_is_trusted_adt_v1(
                    tcx,
                    rust_output,
                    TrustedDeviceItem::Gfx950SubgroupContext,
                ) =>
        {
            Ok(
                SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupContextCurrent {
                    context: output,
                },
            )
        }
        expansion @ (ProductionTerminalExpansionV1::Gfx950SubgroupReduceSumF32
        | ProductionTerminalExpansionV1::Gfx950SubgroupReduceMaxF32)
            if inputs.len() == 2
                && rust_inputs.len() == 2
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::Gfx950SubgroupContext)
                })
                && matches!(rust_inputs[1].kind(), TyKind::Float(FloatTy::F32))
                && matches!(rust_output.kind(), TyKind::Float(FloatTy::F32)) =>
        {
            let width = single_const_u32_v1(instance)
                .filter(|width| *width != 0 && width.is_power_of_two() && *width <= 64)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 subgroup reduction width"))?;
            let kind = match expansion {
                ProductionTerminalExpansionV1::Gfx950SubgroupReduceSumF32 => {
                    SemanticSubgroupReductionKindV1::Sum
                }
                ProductionTerminalExpansionV1::Gfx950SubgroupReduceMaxF32 => {
                    SemanticSubgroupReductionKindV1::Maximum
                }
                _ => unreachable!("matched gfx950 subgroup reduction expansion"),
            };
            Ok(
                SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupReduceF32 {
                    context: pointer_pointee_v1(types, inputs[0])?,
                    width,
                    kind,
                },
            )
        }
        ProductionTerminalExpansionV1::Gfx950SubgroupBroadcastF32
            if inputs.len() == 3
                && rust_inputs.len() == 3
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::Gfx950SubgroupContext)
                })
                && matches!(rust_inputs[1].kind(), TyKind::Float(FloatTy::F32))
                && matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::U32))
                && matches!(rust_output.kind(), TyKind::Float(FloatTy::F32)) =>
        {
            let width = single_const_u32_v1(instance)
                .filter(|width| *width != 0 && width.is_power_of_two() && *width <= 64)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 subgroup broadcast width"))?;
            Ok(SemanticCompilerIntrinsicOperationV1::SubgroupBroadcastF32 {
                context: pointer_pointee_v1(types, inputs[0])?,
                width,
            })
        }
        ProductionTerminalExpansionV1::MatrixContextCurrent
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && rust_is_trusted_adt_v1(tcx, rust_output, TrustedDeviceItem::DeviceMatrix) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { context: output })
        }
        ProductionTerminalExpansionV1::Gfx950MatrixContextCurrent
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && rust_is_trusted_adt_v1(tcx, rust_output, TrustedDeviceItem::Gfx950Matrix) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { context: output })
        }
        ProductionTerminalExpansionV1::WaveLaneCurrent
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && rust_wave_lane64_v1(tcx, rust_output) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
                lane: output,
                wave_width: 64,
            })
        }
        expansion @ (ProductionTerminalExpansionV1::Bf16MatrixARowMajor
        | ProductionTerminalExpansionV1::Bf16MatrixBRowMajor)
            if inputs.len() == 5
                && rust_inputs.len() == 5
                && rust_shared_u16_slice_v1(rust_inputs[0])
                && rust_inputs[1..]
                    .iter()
                    .all(|ty| matches!(ty.kind(), TyKind::Uint(UintTy::Usize))) =>
        {
            let (rust_view, rust_error) = rust_result_payloads_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed MFMA row-major result"))?;
            let role = rust_mfma_matrix_role_v1(tcx, rust_view)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed MFMA row-major view"))?;
            let expected_role = match expansion {
                ProductionTerminalExpansionV1::Bf16MatrixARowMajor => SemanticMfmaOperandRoleV1::A,
                ProductionTerminalExpansionV1::Bf16MatrixBRowMajor => SemanticMfmaOperandRoleV1::B,
                _ => unreachable!("matched row-major expansion"),
            };
            if role != expected_role
                || !rust_is_trusted_adt_v1(
                    tcx,
                    rust_error,
                    TrustedDeviceItem::Bf16MfmaMatrixViewError,
                )
            {
                return Err(body_owner_table_mismatch_v1(
                    "typed MFMA row-major role or error",
                ));
            }
            let (view, error) = semantic_result_payloads_v1(types, output)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor {
                    result: output,
                    view,
                    error,
                    role,
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                },
            )
        }
        expansion @ (ProductionTerminalExpansionV1::Gfx950Fp4MatrixARowMajor
        | ProductionTerminalExpansionV1::Gfx950Fp4MatrixBRowMajor)
            if inputs.len() == 5
                && rust_inputs.len() == 5
                && rust_shared_u8_slice_v1(rust_inputs[0])
                && rust_inputs[1..]
                    .iter()
                    .all(|ty| matches!(ty.kind(), TyKind::Uint(UintTy::Usize))) =>
        {
            let (rust_view, rust_error) = rust_result_payloads_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP4 row-major result"))?;
            let role = rust_gfx950_fp4_matrix_role_v1(tcx, rust_view)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP4 row-major view"))?;
            let expected_role = match expansion {
                ProductionTerminalExpansionV1::Gfx950Fp4MatrixARowMajor => {
                    SemanticMfmaOperandRoleV1::A
                }
                ProductionTerminalExpansionV1::Gfx950Fp4MatrixBRowMajor => {
                    SemanticMfmaOperandRoleV1::B
                }
                _ => unreachable!("matched gfx950 FP4 row-major expansion"),
            };
            if role != expected_role
                || !rust_is_trusted_adt_v1(
                    tcx,
                    rust_error,
                    TrustedDeviceItem::Gfx950MfmaMatrixViewError,
                )
            {
                return Err(body_owner_table_mismatch_v1(
                    "gfx950 FP4 row-major role or error",
                ));
            }
            let (view, error) = semantic_result_payloads_v1(types, output)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::Gfx950Fp4MatrixViewRowMajor {
                    result: output,
                    view,
                    error,
                    role,
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                },
            )
        }
        expansion @ (ProductionTerminalExpansionV1::Gfx950Fp8MatrixARowMajor
        | ProductionTerminalExpansionV1::Gfx950Fp8MatrixBRowMajor)
            if inputs.len() == 5
                && rust_inputs.len() == 5
                && rust_shared_u8_slice_v1(rust_inputs[0])
                && rust_inputs[1..]
                    .iter()
                    .all(|ty| matches!(ty.kind(), TyKind::Uint(UintTy::Usize))) =>
        {
            let (rust_view, rust_error) = rust_result_payloads_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP8 row-major result"))?;
            let role = rust_gfx950_fp8_matrix_role_v1(tcx, rust_view)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP8 row-major view"))?;
            let expected_role = match expansion {
                ProductionTerminalExpansionV1::Gfx950Fp8MatrixARowMajor => {
                    SemanticMfmaOperandRoleV1::A
                }
                ProductionTerminalExpansionV1::Gfx950Fp8MatrixBRowMajor => {
                    SemanticMfmaOperandRoleV1::B
                }
                _ => unreachable!("matched gfx950 row-major expansion"),
            };
            if role != expected_role
                || !rust_is_trusted_adt_v1(
                    tcx,
                    rust_error,
                    TrustedDeviceItem::Gfx950MfmaMatrixViewError,
                )
            {
                return Err(body_owner_table_mismatch_v1(
                    "gfx950 FP8 row-major role or error",
                ));
            }
            let (view, error) = semantic_result_payloads_v1(types, output)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::Gfx950Fp8MatrixViewRowMajor {
                    result: output,
                    view,
                    error,
                    role,
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                },
            )
        }
        ProductionTerminalExpansionV1::StridedReadView2DFromSharedSlice
            if inputs.len() == 5
                && rust_inputs.len() == 5
                && rust_shared_slice_element_v1(rust_inputs[0])
                    .is_some_and(rust_supported_read_view_scalar_v1)
                && rust_inputs[1..]
                    .iter()
                    .all(|ty| matches!(ty.kind(), TyKind::Uint(UintTy::Usize))) =>
        {
            let Some(rust_element) = rust_shared_slice_element_v1(rust_inputs[0]) else {
                return Err(body_owner_table_mismatch_v1(
                    "strided read view slice element",
                ));
            };
            let (rust_view, rust_error) = rust_result_payloads_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("strided read view result"))?;
            let view_arguments = rust_trusted_adt_type_arguments_v1(
                tcx,
                rust_view,
                TrustedDeviceItem::StridedReadView2D,
            )
            .ok_or_else(|| body_owner_table_mismatch_v1("strided read view type"))?;
            if view_arguments.as_slice() != [rust_element]
                || !rust_is_exact_trusted_marker_v1(
                    tcx,
                    rust_error,
                    TrustedDeviceItem::StridedReadView2DError,
                )
            {
                return Err(body_owner_table_mismatch_v1(
                    "strided read view element or error",
                ));
            }
            let (view, error) = semantic_result_payloads_v1(types, output)?;
            let slice = pointer_pointee_v1(types, inputs[0])?;
            let SemanticTypeShapeV1::Slice { element } = types
                .get(slice.index() as usize)
                .ok_or_else(|| body_owner_table_mismatch_v1("strided read view slice"))?
                .shape()
            else {
                return Err(body_owner_table_mismatch_v1("strided read view slice"));
            };
            Ok(
                SemanticCompilerIntrinsicOperationV1::StridedReadView2DFromSharedSlice {
                    result: output,
                    view,
                    error,
                    element: *element,
                },
            )
        }
        ProductionTerminalExpansionV1::StridedReadView2DLoadOr
            if inputs.len() == 4
                && rust_inputs.len() == 4
                && matches!(rust_inputs[1].kind(), TyKind::Uint(UintTy::Usize))
                && matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize))
                && rust_inputs[3] == rust_output
                && rust_supported_read_view_scalar_v1(rust_output) =>
        {
            let rust_view = rust_reference_pointee_v1(rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("strided read view borrow"))?;
            let view_arguments = rust_trusted_adt_type_arguments_v1(
                tcx,
                rust_view,
                TrustedDeviceItem::StridedReadView2D,
            )
            .ok_or_else(|| body_owner_table_mismatch_v1("strided read view type"))?;
            if view_arguments.as_slice() != [rust_output] || inputs[3] != output {
                return Err(body_owner_table_mismatch_v1(
                    "strided read view load element",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::StridedReadView2DLoadOr {
                    view: pointer_pointee_v1(types, inputs[0])?,
                    element: output,
                },
            )
        }
        expansion @ (ProductionTerminalExpansionV1::Bf16MatrixALoadZeroFilledV2
        | ProductionTerminalExpansionV1::Bf16MatrixBLoadZeroFilledV2)
            if inputs.len() == 4
                && rust_inputs.len() == 4
                && matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize))
                && matches!(rust_inputs[3].kind(), TyKind::Uint(UintTy::Usize)) =>
        {
            let rust_view = rust_reference_pointee_v1(rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("typed MFMA load view borrow"))?;
            let role = rust_mfma_matrix_role_v1(tcx, rust_view)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed MFMA load view"))?;
            let rust_lane = rust_reference_pointee_v1(rust_inputs[1])
                .ok_or_else(|| body_owner_table_mismatch_v1("typed MFMA load lane borrow"))?;
            let contract = rust_mfma_fragment_contract_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed MFMA load fragment"))?;
            let expected_role = match expansion {
                ProductionTerminalExpansionV1::Bf16MatrixALoadZeroFilledV2 => {
                    SemanticMfmaOperandRoleV1::A
                }
                ProductionTerminalExpansionV1::Bf16MatrixBLoadZeroFilledV2 => {
                    SemanticMfmaOperandRoleV1::B
                }
                _ => unreachable!("matched MFMA load expansion"),
            };
            if role != expected_role
                || contract.role != expected_role
                || !rust_wave_lane64_v1(tcx, rust_lane)
            {
                return Err(body_owner_table_mismatch_v1("typed MFMA load contract"));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
                    fragment: output,
                    view: pointer_pointee_v1(types, inputs[0])?,
                    lane: pointer_pointee_v1(types, inputs[1])?,
                    contract,
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                },
            )
        }
        expansion @ (ProductionTerminalExpansionV1::Gfx950Fp4MatrixALoadM16K128
        | ProductionTerminalExpansionV1::Gfx950Fp4MatrixBLoadK128N16)
            if inputs.len() == 4
                && rust_inputs.len() == 4
                && matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize))
                && matches!(rust_inputs[3].kind(), TyKind::Uint(UintTy::Usize)) =>
        {
            let rust_view = rust_reference_pointee_v1(rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP4 load view borrow"))?;
            let role = rust_gfx950_fp4_matrix_role_v1(tcx, rust_view)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP4 load view"))?;
            let rust_lane = rust_reference_pointee_v1(rust_inputs[1])
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP4 load lane borrow"))?;
            let contract = rust_gfx950_fp4_fragment_contract_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP4 load fragment"))?;
            let expected_role = match expansion {
                ProductionTerminalExpansionV1::Gfx950Fp4MatrixALoadM16K128 => {
                    SemanticMfmaOperandRoleV1::A
                }
                ProductionTerminalExpansionV1::Gfx950Fp4MatrixBLoadK128N16 => {
                    SemanticMfmaOperandRoleV1::B
                }
                _ => unreachable!("matched gfx950 FP4 load expansion"),
            };
            if role != expected_role
                || contract.role != expected_role
                || !rust_wave_lane64_v1(tcx, rust_lane)
            {
                return Err(body_owner_table_mismatch_v1("gfx950 FP4 load contract"));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::Gfx950Fp4MatrixLoadM16K128 {
                    fragment: output,
                    view: pointer_pointee_v1(types, inputs[0])?,
                    lane: pointer_pointee_v1(types, inputs[1])?,
                    contract,
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                },
            )
        }
        expansion @ (ProductionTerminalExpansionV1::Gfx950Fp8MatrixALoadM16K128
        | ProductionTerminalExpansionV1::Gfx950Fp8MatrixBLoadK128N16)
            if inputs.len() == 4
                && rust_inputs.len() == 4
                && matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize))
                && matches!(rust_inputs[3].kind(), TyKind::Uint(UintTy::Usize)) =>
        {
            let rust_view = rust_reference_pointee_v1(rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP8 load view borrow"))?;
            let role = rust_gfx950_fp8_matrix_role_v1(tcx, rust_view)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP8 load view"))?;
            let rust_lane = rust_reference_pointee_v1(rust_inputs[1])
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP8 load lane borrow"))?;
            let contract = rust_gfx950_fp8_fragment_contract_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP8 load fragment"))?;
            let expected_role = match expansion {
                ProductionTerminalExpansionV1::Gfx950Fp8MatrixALoadM16K128 => {
                    SemanticMfmaOperandRoleV1::A
                }
                ProductionTerminalExpansionV1::Gfx950Fp8MatrixBLoadK128N16 => {
                    SemanticMfmaOperandRoleV1::B
                }
                _ => unreachable!("matched gfx950 FP8 load expansion"),
            };
            if role != expected_role
                || contract.role != expected_role
                || !rust_wave_lane64_v1(tcx, rust_lane)
            {
                return Err(body_owner_table_mismatch_v1("gfx950 FP8 load contract"));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::Gfx950Fp8MatrixLoadM16K128 {
                    fragment: output,
                    view: pointer_pointee_v1(types, inputs[0])?,
                    lane: pointer_pointee_v1(types, inputs[1])?,
                    contract,
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                },
            )
        }
        ProductionTerminalExpansionV1::Gfx950LdsTransposeTileCurrent
            if inputs.len() == 1
                && rust_inputs.len() == 1
                && rust_reference_pointee_v1(rust_inputs[0])
                    .is_some_and(|ty| rust_wave_lane64_v1(tcx, ty)) =>
        {
            let format = rust_gfx950_lds_transpose_format_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 transpose current tile"))?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeCurrent {
                    tile: output,
                    lane: pointer_pointee_v1(types, inputs[0])?,
                    format,
                },
            )
        }
        expansion @ (ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB4
        | ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB8)
            if inputs.len() == 4
                && rust_inputs.len() == 4
                && matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize))
                && matches!(rust_inputs[3].kind(), TyKind::Uint(UintTy::Usize)) =>
        {
            let input_format = rust_gfx950_lds_transpose_format_v1(tcx, rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 transpose stage input"))?;
            let output_format = rust_gfx950_lds_transpose_format_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 transpose stage output"))?;
            let rust_view = rust_reference_pointee_v1(rust_inputs[1])
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 transpose stage view"))?;
            let expected_format = match expansion {
                ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB4 => {
                    (rust_gfx950_fp4_matrix_role_v1(tcx, rust_view)
                        == Some(SemanticMfmaOperandRoleV1::A))
                    .then_some(SemanticGfx950LdsTransposeFormatV1::Fp4E2M1)
                }
                ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB8 => {
                    (rust_gfx950_fp8_matrix_role_v1(tcx, rust_view)
                        == Some(SemanticMfmaOperandRoleV1::A))
                    .then_some(SemanticGfx950LdsTransposeFormatV1::Fp8E4M3)
                }
                _ => unreachable!("matched gfx950 transpose stage expansion"),
            }
            .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 transpose stage A view"))?;
            if input_format != expected_format || output_format != expected_format {
                return Err(body_owner_table_mismatch_v1(
                    "gfx950 transpose stage format transition",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeStage {
                    input_tile: inputs[0],
                    output_tile: output,
                    view: pointer_pointee_v1(types, inputs[1])?,
                    format: expected_format,
                },
            )
        }
        ProductionTerminalExpansionV1::Gfx950LdsTransposePublish
            if inputs.len() == 1 && rust_inputs.len() == 1 =>
        {
            let input_format = rust_gfx950_lds_transpose_format_v1(tcx, rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 transpose publish input"))?;
            let output_format = rust_gfx950_lds_transpose_format_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 transpose publish output"))?;
            if input_format != output_format {
                return Err(body_owner_table_mismatch_v1(
                    "gfx950 transpose publish format transition",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposePublish {
                    input_tile: inputs[0],
                    output_tile: output,
                    format: input_format,
                },
            )
        }
        expansion @ (ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB4
        | ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB8)
            if inputs.len() == 1 && rust_inputs.len() == 1 =>
        {
            let input_format = rust_gfx950_lds_transpose_format_v1(tcx, rust_inputs[0])
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 transpose read tile"))?;
            let (expected_format, contract) = match expansion {
                ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB4 => (
                    SemanticGfx950LdsTransposeFormatV1::Fp4E2M1,
                    rust_gfx950_fp4_fragment_contract_v1(tcx, rust_output),
                ),
                ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB8 => (
                    SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
                    rust_gfx950_fp8_fragment_contract_v1(tcx, rust_output),
                ),
                _ => unreachable!("matched gfx950 transpose read expansion"),
            };
            let contract = contract
                .filter(|contract| contract.role == SemanticMfmaOperandRoleV1::B)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 transpose B fragment"))?;
            if input_format != expected_format {
                return Err(body_owner_table_mismatch_v1("gfx950 transpose read format"));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeRead {
                    tile: inputs[0],
                    fragment: output,
                    contract,
                    format: expected_format,
                },
            )
        }
        ProductionTerminalExpansionV1::F32MatrixAccumulatorZero
            if inputs.len() == 1
                && rust_inputs.len() == 1
                && rust_reference_pointee_v1(rust_inputs[0])
                    .is_some_and(|ty| rust_wave_lane64_v1(tcx, ty)) =>
        {
            let contract = rust_mfma_accumulator_contract_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("typed MFMA zero accumulator"))?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
                    lane: pointer_pointee_v1(types, inputs[0])?,
                    fragment: output,
                    contract,
                },
            )
        }
        ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorZero
            if inputs.len() == 1
                && rust_inputs.len() == 1
                && rust_reference_pointee_v1(rust_inputs[0])
                    .is_some_and(|ty| rust_wave_lane64_v1(tcx, ty)) =>
        {
            let contract = rust_gfx950_fp4_accumulator_contract_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP4 zero accumulator"))?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
                    lane: pointer_pointee_v1(types, inputs[0])?,
                    fragment: output,
                    contract,
                },
            )
        }
        ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorZero
            if inputs.len() == 1
                && rust_inputs.len() == 1
                && rust_reference_pointee_v1(rust_inputs[0])
                    .is_some_and(|ty| rust_wave_lane64_v1(tcx, ty)) =>
        {
            let contract = rust_gfx950_fp8_accumulator_contract_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("gfx950 FP8 zero accumulator"))?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
                    lane: pointer_pointee_v1(types, inputs[0])?,
                    fragment: output,
                    contract,
                },
            )
        }
        ProductionTerminalExpansionV1::F32MatrixAccumulatorIntoValues
            if inputs.len() == 1
                && rust_inputs.len() == 1
                && rust_mfma_accumulator_contract_v1(tcx, rust_inputs[0]).is_some()
                && rust_f32_array_v1(tcx, rust_output, 4) =>
        {
            Ok(
                SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                    fragment: inputs[0],
                    values: output,
                },
            )
        }
        ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorIntoValues
            if inputs.len() == 1
                && rust_inputs.len() == 1
                && rust_gfx950_fp4_accumulator_contract_v1(tcx, rust_inputs[0]).is_some()
                && rust_f32_array_v1(tcx, rust_output, 4) =>
        {
            Ok(
                SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                    fragment: inputs[0],
                    values: output,
                },
            )
        }
        ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorIntoValues
            if inputs.len() == 1
                && rust_inputs.len() == 1
                && rust_gfx950_fp8_accumulator_contract_v1(tcx, rust_inputs[0]).is_some()
                && rust_f32_array_v1(tcx, rust_output, 4) =>
        {
            Ok(
                SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                    fragment: inputs[0],
                    values: output,
                },
            )
        }
        ProductionTerminalExpansionV1::MatrixMultiplyAccumulate
            if inputs.len() == 4
                && rust_inputs.len() == 4
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::DeviceMatrix)
                })
                && rust_mfma_fragment_contract_v1(tcx, rust_inputs[1]).is_some()
                && rust_mfma_fragment_contract_v1(tcx, rust_inputs[2]).is_some()
                && rust_mfma_accumulator_contract_v1(tcx, rust_inputs[3]).is_some()
                && rust_mfma_accumulator_contract_v1(tcx, rust_output).is_some() =>
        {
            let (Some(lhs), Some(rhs), Some(accumulator)) = (
                rust_mfma_fragment_contract_v1(tcx, rust_inputs[1]),
                rust_mfma_fragment_contract_v1(tcx, rust_inputs[2]),
                rust_mfma_accumulator_contract_v1(tcx, rust_inputs[3]),
            ) else {
                return Err(body_owner_table_mismatch_v1("typed MFMA argument contract"));
            };
            if lhs.role != SemanticMfmaOperandRoleV1::A
                || rhs.role != SemanticMfmaOperandRoleV1::B
                || Some(accumulator) != rust_mfma_accumulator_contract_v1(tcx, rust_output)
            {
                return Err(body_owner_table_mismatch_v1("typed MFMA argument contract"));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                    context: pointer_pointee_v1(types, inputs[0])?,
                    lhs_fragment: inputs[1],
                    rhs_fragment: inputs[2],
                    accumulator_fragment: inputs[3],
                    lhs,
                    rhs,
                    accumulator,
                },
            )
        }
        ProductionTerminalExpansionV1::Gfx950Fp4MultiplyAccumulate
            if inputs.len() == 4
                && rust_inputs.len() == 4
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::Gfx950Matrix)
                })
                && rust_gfx950_fp4_fragment_contract_v1(tcx, rust_inputs[1]).is_some()
                && rust_gfx950_fp4_fragment_contract_v1(tcx, rust_inputs[2]).is_some()
                && rust_gfx950_fp4_accumulator_contract_v1(tcx, rust_inputs[3]).is_some()
                && rust_gfx950_fp4_accumulator_contract_v1(tcx, rust_output).is_some() =>
        {
            let (Some(lhs), Some(rhs), Some(accumulator)) = (
                rust_gfx950_fp4_fragment_contract_v1(tcx, rust_inputs[1]),
                rust_gfx950_fp4_fragment_contract_v1(tcx, rust_inputs[2]),
                rust_gfx950_fp4_accumulator_contract_v1(tcx, rust_inputs[3]),
            ) else {
                return Err(body_owner_table_mismatch_v1("gfx950 FP4 MFMA contract"));
            };
            if lhs.role != SemanticMfmaOperandRoleV1::A
                || rhs.role != SemanticMfmaOperandRoleV1::B
                || Some(accumulator) != rust_gfx950_fp4_accumulator_contract_v1(tcx, rust_output)
            {
                return Err(body_owner_table_mismatch_v1("gfx950 FP4 MFMA contract"));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                    context: pointer_pointee_v1(types, inputs[0])?,
                    lhs_fragment: inputs[1],
                    rhs_fragment: inputs[2],
                    accumulator_fragment: inputs[3],
                    lhs,
                    rhs,
                    accumulator,
                },
            )
        }
        ProductionTerminalExpansionV1::Gfx950Fp4Fp8MultiplyAccumulate
            if inputs.len() == 4
                && rust_inputs.len() == 4
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::Gfx950Matrix)
                })
                && rust_gfx950_fp4_fragment_contract_v1(tcx, rust_inputs[1]).is_some()
                && rust_gfx950_fp8_fragment_contract_v1(tcx, rust_inputs[2]).is_some()
                && rust_gfx950_fp4_accumulator_contract_v1(tcx, rust_inputs[3]).is_some()
                && rust_gfx950_fp4_accumulator_contract_v1(tcx, rust_output).is_some() =>
        {
            let (Some(lhs), Some(rhs), Some(accumulator)) = (
                rust_gfx950_fp4_fragment_contract_v1(tcx, rust_inputs[1]),
                rust_gfx950_fp8_fragment_contract_v1(tcx, rust_inputs[2]),
                rust_gfx950_fp4_accumulator_contract_v1(tcx, rust_inputs[3]),
            ) else {
                return Err(body_owner_table_mismatch_v1(
                    "gfx950 mixed FP4xFP8 MFMA contract",
                ));
            };
            if lhs.role != SemanticMfmaOperandRoleV1::A
                || rhs.role != SemanticMfmaOperandRoleV1::B
                || Some(accumulator) != rust_gfx950_fp4_accumulator_contract_v1(tcx, rust_output)
            {
                return Err(body_owner_table_mismatch_v1(
                    "gfx950 mixed FP4xFP8 MFMA contract",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                    context: pointer_pointee_v1(types, inputs[0])?,
                    lhs_fragment: inputs[1],
                    rhs_fragment: inputs[2],
                    accumulator_fragment: inputs[3],
                    lhs,
                    rhs,
                    accumulator,
                },
            )
        }
        ProductionTerminalExpansionV1::Gfx950Fp8MultiplyAccumulate
            if inputs.len() == 4
                && rust_inputs.len() == 4
                && rust_reference_pointee_v1(rust_inputs[0]).is_some_and(|ty| {
                    rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::Gfx950Matrix)
                })
                && rust_gfx950_fp8_fragment_contract_v1(tcx, rust_inputs[1]).is_some()
                && rust_gfx950_fp8_fragment_contract_v1(tcx, rust_inputs[2]).is_some()
                && rust_gfx950_fp8_accumulator_contract_v1(tcx, rust_inputs[3]).is_some()
                && rust_gfx950_fp8_accumulator_contract_v1(tcx, rust_output).is_some() =>
        {
            let (Some(lhs), Some(rhs), Some(accumulator)) = (
                rust_gfx950_fp8_fragment_contract_v1(tcx, rust_inputs[1]),
                rust_gfx950_fp8_fragment_contract_v1(tcx, rust_inputs[2]),
                rust_gfx950_fp8_accumulator_contract_v1(tcx, rust_inputs[3]),
            ) else {
                return Err(body_owner_table_mismatch_v1("gfx950 FP8 MFMA contract"));
            };
            if lhs.role != SemanticMfmaOperandRoleV1::A
                || rhs.role != SemanticMfmaOperandRoleV1::B
                || Some(accumulator) != rust_gfx950_fp8_accumulator_contract_v1(tcx, rust_output)
            {
                return Err(body_owner_table_mismatch_v1("gfx950 FP8 MFMA contract"));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                    context: pointer_pointee_v1(types, inputs[0])?,
                    lhs_fragment: inputs[1],
                    rhs_fragment: inputs[2],
                    accumulator_fragment: inputs[3],
                    lhs,
                    rhs,
                    accumulator,
                },
            )
        }
        ProductionTerminalExpansionV1::ThreadIndex1d
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && rust_branded_index_v1(tcx, rust_output, TrustedDeviceItem::ThreadIndex)
                    .is_some_and(|(index_space, brand)| {
                        rust_disjoint_index_space_v1(tcx, index_space)
                            == Some(SemanticDisjointIndexSpaceV1::Index1d)
                            && rust_is_unbranded_capability_v1(tcx, brand)
                    }) =>
        {
            let raw_index = aggregate_field_v1(types, output, 0)?;
            Ok(SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: output,
                raw_index,
            })
        }
        ProductionTerminalExpansionV1::Invocation3DIndex1D
            if inputs.len() == 1 && rust_inputs.len() == 1 =>
        {
            let root = capability_root.ok_or(
                ProductionSemanticImportErrorV1::KernelContextBinding(
                    "invocation index lacks authenticated root custody",
                ),
            )?;
            let invocation_brand = rust_inputs
                .first()
                .and_then(|ty| rust_reference_pointee_v1(*ty))
                .and_then(|ty| rust_invocation_brand_v1(tcx, ty))
                .and_then(|brand| rust_kernel_brand_v1(tcx, brand))
                .ok_or_else(|| body_owner_table_mismatch_v1("branded invocation index input"))?;
            let (index_space, index_brand_ty) =
                rust_branded_index_v1(tcx, rust_output, TrustedDeviceItem::ThreadIndex)
                    .ok_or_else(|| body_owner_table_mismatch_v1("branded invocation index output"))?;
            let index_brand = rust_kernel_brand_v1(tcx, index_brand_ty)
                .ok_or_else(|| body_owner_table_mismatch_v1("branded invocation index output"))?;
            if !trusted_device_items::is_authenticated_index_space_1d_v1(tcx, index_space)
                || invocation_brand.ty != index_brand.ty
                || *rustc_type_identity_v1(tcx, index_brand.kernel).as_bytes()
                    != root.kernel_marker_identity
            {
                return Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                    "invocation index does not retain its root's exact kernel, target, launch, and Index1D brand",
                ));
            }
            let raw_index = aggregate_field_v1(types, output, 0)?;
            Ok(SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: output,
                raw_index,
            })
        }
        ProductionTerminalExpansionV1::ThreadIndexGet
            if inputs.len() == 1
                && rust_inputs.len() == 1
                && rust_reference_pointee_v1(rust_inputs[0])
                    .and_then(|ty| {
                        rust_index_witness_space_v1(tcx, ty, TrustedDeviceItem::ThreadIndex)
                    })
                    .is_some() =>
        {
            let index_witness = pointer_pointee_v1(types, inputs[0])?;
            Ok(SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                index_witness,
                raw_index: output,
            })
        }
        ProductionTerminalExpansionV1::ThreadIndexIntoDisjoint
            if inputs.len() == 1 && rust_inputs.len() == 1 =>
        {
            let (Some((input_space, input_brand)), Some((output_space, output_brand))) = (
                rust_index_witness_contract_v1(tcx, rust_inputs[0], TrustedDeviceItem::ThreadIndex),
                rust_index_witness_contract_v1(tcx, rust_output, TrustedDeviceItem::DisjointIndex),
            ) else {
                return Err(body_owner_table_mismatch_v1("terminal disjoint mapping"));
            };
            if input_space != output_space || input_brand != output_brand {
                return Err(body_owner_table_mismatch_v1("terminal disjoint mapping"));
            }
            let raw_index = aggregate_field_v1(types, inputs[0], 0)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
                    input_witness: inputs[0],
                    output_witness: output,
                    raw_index,
                    index_space: input_space,
                },
            )
        }
        ProductionTerminalExpansionV1::ThreadIndexCheckedShift
            if inputs.len() == 1 && rust_inputs.len() == 1 =>
        {
            checked_shift_operation_v1(
                tcx,
                rust_inputs[0],
                rust_output,
                inputs[0],
                output,
                types,
                TrustedDeviceItem::ThreadIndex,
                true,
            )
        }
        ProductionTerminalExpansionV1::ThreadIndexCheckedBlock
            if inputs.len() == 1 && rust_inputs.len() == 1 =>
        {
            let (input_space, input_brand) =
                rust_index_witness_contract_v1(tcx, rust_inputs[0], TrustedDeviceItem::ThreadIndex)
                    .ok_or_else(|| body_owner_table_mismatch_v1("terminal checked-block input"))?;
            let rust_output_block = rust_option_payload_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("terminal checked-block result"))?;
            let (output_space, lanes_per_block, elements_per_lane, output_brand) =
                rust_disjoint_block_contract_v1(tcx, rust_output_block).ok_or_else(|| {
                    body_owner_table_mismatch_v1("terminal checked-block witness")
                })?;
            if input_space != SemanticDisjointIndexSpaceV1::Index1d || input_brand != output_brand {
                return Err(body_owner_table_mismatch_v1(
                    "terminal checked-block input mapping or brand",
                ));
            }
            let output_block = option_payload_v1(types, output)?;
            let raw_index = aggregate_field_v1(types, inputs[0], 0)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock {
                    input_witness: inputs[0],
                    output_block,
                    raw_index,
                    input_space,
                    output_space,
                    lanes_per_block,
                    elements_per_lane,
                },
            )
        }
        ProductionTerminalExpansionV1::ThreadIndexCheckedTiled2d
            if inputs.len() == 1 && rust_inputs.len() == 1 =>
        {
            let (input_space, input_brand) = rust_index_witness_contract_v1(
                tcx,
                rust_inputs[0],
                TrustedDeviceItem::ThreadIndex,
            )
            .ok_or_else(|| body_owner_table_mismatch_v1("terminal checked-tiled-2d input"))?;
            let rust_output_tile = rust_option_payload_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("terminal checked-tiled-2d result"))?;
            let (
                output_space,
                lanes_per_tile,
                tile_rows,
                tile_columns,
                elements_per_lane,
                output_brand,
            ) = rust_disjoint_tile_2d_contract_v1(tcx, rust_output_tile).ok_or_else(|| {
                    body_owner_table_mismatch_v1("terminal checked-tiled-2d witness")
                })?;
            if input_space != SemanticDisjointIndexSpaceV1::Index1d
                || input_brand != output_brand
            {
                return Err(body_owner_table_mismatch_v1(
                    "terminal checked-tiled-2d input mapping or brand",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedTiled2d {
                    input_witness: inputs[0],
                    output_tile: option_payload_v1(types, output)?,
                    raw_index: aggregate_field_v1(types, inputs[0], 0)?,
                    input_space,
                    output_space,
                    lanes_per_tile,
                    tile_rows,
                    tile_columns,
                    elements_per_lane,
                },
            )
        }
        ProductionTerminalExpansionV1::ThreadIndexCheckedRowStriped2d
            if inputs.len() == 1 && rust_inputs.len() == 1 =>
        {
            let (input_space, input_brand) = rust_index_witness_contract_v1(
                tcx,
                rust_inputs[0],
                TrustedDeviceItem::ThreadIndex,
            )
            .ok_or_else(|| {
                body_owner_table_mismatch_v1("terminal checked-row-striped-2d input")
            })?;
            let rust_output_stripe = rust_option_payload_v1(tcx, rust_output).ok_or_else(|| {
                body_owner_table_mismatch_v1("terminal checked-row-striped-2d result")
            })?;
            let (output_space, lanes_per_row, elements_per_lane, output_brand) =
                rust_disjoint_row_stripe_2d_contract_v1(tcx, rust_output_stripe).ok_or_else(|| {
                    body_owner_table_mismatch_v1("terminal checked-row-striped-2d witness")
                })?;
            if input_space != SemanticDisjointIndexSpaceV1::Index1d
                || input_brand != output_brand
            {
                return Err(body_owner_table_mismatch_v1(
                    "terminal checked-row-striped-2d input mapping or brand",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedRowStriped2d {
                    input_witness: inputs[0],
                    output_stripe: option_payload_v1(types, output)?,
                    raw_index: aggregate_field_v1(types, inputs[0], 0)?,
                    input_space,
                    output_space,
                    lanes_per_row,
                    elements_per_lane,
                },
            )
        }
        ProductionTerminalExpansionV1::DisjointIndexGet
            if inputs.len() == 1 && rust_inputs.len() == 1 =>
        {
            let Some(rust_witness) = rust_reference_pointee_v1(rust_inputs[0]) else {
                return Err(body_owner_table_mismatch_v1(
                    "terminal disjoint-index receiver",
                ));
            };
            let Some(index_space) =
                rust_index_witness_space_v1(tcx, rust_witness, TrustedDeviceItem::DisjointIndex)
            else {
                return Err(body_owner_table_mismatch_v1("terminal disjoint mapping"));
            };
            let index_witness = pointer_pointee_v1(types, inputs[0])?;
            Ok(SemanticCompilerIntrinsicOperationV1::DisjointIndexGet {
                index_witness,
                raw_index: output,
                index_space,
            })
        }
        ProductionTerminalExpansionV1::DisjointBlockComponentIndex
            if inputs.len() == 2 && rust_inputs.len() == 2 =>
        {
            let (receiver_is_shared, rust_block) = match *rust_inputs[0].kind() {
                TyKind::Ref(_, rust_block, mutability) => {
                    (mutability == rustc_hir::Mutability::Not, Some(rust_block))
                }
                _ => (false, None),
            };
            let (index_space, lanes_per_block, elements_per_lane) =
                require_rust_disjoint_block_component_contract_v1(
                    receiver_is_shared,
                    rust_block.and_then(|rust_block| rust_disjoint_block_v1(tcx, rust_block)),
                    matches!(rust_inputs[1].kind(), TyKind::Uint(UintTy::Usize)),
                    rust_option_payload_v1(tcx, rust_output)
                        .is_some_and(|ty| matches!(ty.kind(), TyKind::Uint(UintTy::Usize))),
                )?;
            let block_witness = pointer_pointee_v1(types, inputs[0])?;
            let raw_index = option_payload_v1(types, output)?;
            if inputs[1] != raw_index {
                return Err(body_owner_table_mismatch_v1(
                    "terminal disjoint-block raw index type",
                ));
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::DisjointBlockComponentIndex {
                    block_witness,
                    raw_index,
                    index_space,
                    lanes_per_block,
                    elements_per_lane,
                },
            )
        }
        ProductionTerminalExpansionV1::DisjointIndexCheckedShift
            if inputs.len() == 1 && rust_inputs.len() == 1 =>
        {
            checked_shift_operation_v1(
                tcx,
                rust_inputs[0],
                rust_output,
                inputs[0],
                output,
                types,
                TrustedDeviceItem::DisjointIndex,
                false,
            )
        }
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceLen
            if inputs.len() == 1 && rust_inputs.len() == 1 =>
        {
            let (_, index_space) = rust_reference_pointee_v1(rust_inputs[0])
                .and_then(|ty| rust_write_only_disjoint_slice_v1(tcx, ty))
                .ok_or_else(|| {
                    body_owner_table_mismatch_v1("terminal write-only disjoint-slice len")
                })?;
            let disjoint_slice = pointer_pointee_v1(types, inputs[0])?;
            let element_pointer = aggregate_field_v1(types, disjoint_slice, 0)?;
            let element = pointer_pointee_v1(types, element_pointer)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceLen {
                    disjoint_slice,
                    element,
                    raw_index: output,
                    index_space,
                },
            )
        }
        expansion @ (ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWrite
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteDisjoint
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteExclusive
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteBlock
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteTiled2d
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteRowStriped2d) => {
            write_only_disjoint_operation_v1(
                tcx,
                expansion,
                inputs,
                rust_inputs,
                rust_output,
                types,
            )
        }
        ProductionTerminalExpansionV1::DisjointSliceLen
            if inputs.len() == 1 && rust_inputs.len() == 1 =>
        {
            let (_, index_space) = rust_reference_pointee_v1(rust_inputs[0])
                .and_then(|ty| rust_disjoint_slice_v1(tcx, ty))
                .ok_or_else(|| body_owner_table_mismatch_v1("terminal disjoint-slice len"))?;
            let disjoint_slice = pointer_pointee_v1(types, inputs[0])?;
            let element_pointer = aggregate_field_v1(types, disjoint_slice, 0)?;
            let element = pointer_pointee_v1(types, element_pointer)?;
            Ok(SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
                disjoint_slice,
                element,
                raw_index: output,
                index_space,
            })
        }
        ProductionTerminalExpansionV1::DisjointSliceGetMut
            if inputs.len() == 2 && rust_inputs.len() == 2 =>
        {
            let rust_slice = rust_reference_pointee_v1(rust_inputs[0])
                .and_then(|ty| rust_disjoint_slice_v1(tcx, ty));
            let rust_index =
                rust_index_witness_space_v1(tcx, rust_inputs[1], TrustedDeviceItem::ThreadIndex);
            if rust_slice.map(|(_, space)| space) != rust_index {
                return Err(body_owner_table_mismatch_v1("terminal disjoint mapping"));
            }
            let disjoint_slice = pointer_pointee_v1(types, inputs[0])?;
            let index_witness = inputs[1];
            let raw_index = aggregate_field_v1(types, index_witness, 0)?;
            let element_pointer = aggregate_field_v1(types, disjoint_slice, 0)?;
            let element = pointer_pointee_v1(types, element_pointer)?;
            Ok(SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                disjoint_slice,
                index_witness,
                element,
                raw_index,
            })
        }
        ProductionTerminalExpansionV1::DisjointSliceGetDisjointMut
            if inputs.len() == 2 && rust_inputs.len() == 2 =>
        {
            let rust_slice = rust_reference_pointee_v1(rust_inputs[0])
                .and_then(|ty| rust_disjoint_slice_v1(tcx, ty));
            let rust_index =
                rust_index_witness_space_v1(tcx, rust_inputs[1], TrustedDeviceItem::DisjointIndex);
            let Some(index_space) = rust_index else {
                return Err(body_owner_table_mismatch_v1("terminal disjoint mapping"));
            };
            if rust_slice.map(|(_, space)| space) != Some(index_space) {
                return Err(body_owner_table_mismatch_v1("terminal disjoint mapping"));
            }
            let disjoint_slice = pointer_pointee_v1(types, inputs[0])?;
            let index_witness = inputs[1];
            let raw_index = aggregate_field_v1(types, index_witness, 0)?;
            let element_pointer = aggregate_field_v1(types, disjoint_slice, 0)?;
            let element = pointer_pointee_v1(types, element_pointer)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                    disjoint_slice,
                    index_witness,
                    element,
                    raw_index,
                    index_space,
                },
            )
        }
        ProductionTerminalExpansionV1::GridLeaderCurrent
            if inputs.is_empty() && rust_inputs.is_empty() =>
        {
            let Some(rust_leader) = rust_option_payload_v1(tcx, rust_output) else {
                return Err(body_owner_table_mismatch_v1("terminal grid-leader result"));
            };
            if !rust_is_trusted_adt_v1(tcx, rust_leader, TrustedDeviceItem::GridLeader) {
                return Err(body_owner_table_mismatch_v1(
                    "terminal grid-leader identity",
                ));
            }
            Ok(SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent {
                grid_leader: option_payload_v1(types, output)?,
            })
        }
        ProductionTerminalExpansionV1::DisjointSliceGetMutExclusive
            if inputs.len() == 3 && rust_inputs.len() == 3 =>
        {
            let rust_slice = rust_reference_pointee_v1(rust_inputs[0])
                .and_then(|ty| rust_disjoint_slice_v1(tcx, ty));
            let rust_leader = rust_reference_pointee_v1(rust_inputs[1]);
            if rust_slice.map(|(_, space)| space)
                != Some(SemanticDisjointIndexSpaceV1::GridExclusive)
                || rust_leader.is_none_or(|ty| {
                    !rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::GridLeader)
                })
            {
                return Err(body_owner_table_mismatch_v1(
                    "terminal grid-exclusive mapping",
                ));
            }
            let disjoint_slice = pointer_pointee_v1(types, inputs[0])?;
            let grid_leader = pointer_pointee_v1(types, inputs[1])?;
            let element_pointer = aggregate_field_v1(types, disjoint_slice, 0)?;
            let element = pointer_pointee_v1(types, element_pointer)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                    disjoint_slice,
                    grid_leader,
                    element,
                    raw_index: inputs[2],
                },
            )
        }
        ProductionTerminalExpansionV1::DisjointSliceGetBlockMut
            if inputs.len() == 3 && rust_inputs.len() == 3 =>
        {
            let rust_slice = rust_reference_pointee_v1(rust_inputs[0])
                .and_then(|ty| rust_disjoint_slice_v1(tcx, ty));
            let rust_block = rust_reference_pointee_v1(rust_inputs[1])
                .and_then(|ty| rust_disjoint_block_v1(tcx, ty));
            let Some((index_space, lanes_per_block, elements_per_lane)) = rust_block else {
                return Err(body_owner_table_mismatch_v1(
                    "terminal blocked witness identity",
                ));
            };
            if rust_slice.map(|(_, space)| space) != Some(index_space) {
                return Err(body_owner_table_mismatch_v1(
                    "terminal blocked mapping identity",
                ));
            }
            let disjoint_slice = pointer_pointee_v1(types, inputs[0])?;
            let block_witness = pointer_pointee_v1(types, inputs[1])?;
            let element_pointer = aggregate_field_v1(types, disjoint_slice, 0)?;
            let element = pointer_pointee_v1(types, element_pointer)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
                    disjoint_slice,
                    block_witness,
                    element,
                    raw_index: inputs[2],
                    index_space,
                    lanes_per_block,
                    elements_per_lane,
                },
            )
        }
        ProductionTerminalExpansionV1::DisjointSliceGetTiled2dMut
            if inputs.len() == 6 && rust_inputs.len() == 6 =>
        {
            let rust_slice = rust_reference_pointee_v1(rust_inputs[0])
                .and_then(|ty| rust_disjoint_slice_v1(tcx, ty));
            let rust_tile = rust_reference_pointee_v1(rust_inputs[1])
                .and_then(|ty| rust_disjoint_tile_2d_v1(tcx, ty));
            let Some((index_space, lanes_per_tile, tile_rows, tile_columns, elements_per_lane)) =
                rust_tile
            else {
                return Err(body_owner_table_mismatch_v1(
                    "terminal tiled-2d witness identity",
                ));
            };
            if rust_slice.map(|(_, space)| space) != Some(index_space) {
                return Err(body_owner_table_mismatch_v1(
                    "terminal tiled-2d mapping identity",
                ));
            }
            let disjoint_slice = pointer_pointee_v1(types, inputs[0])?;
            let tile_witness = pointer_pointee_v1(types, inputs[1])?;
            let element_pointer = aggregate_field_v1(types, disjoint_slice, 0)?;
            let element = pointer_pointee_v1(types, element_pointer)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
                    disjoint_slice,
                    tile_witness,
                    element,
                    raw_index: inputs[2],
                    index_space,
                    lanes_per_tile,
                    tile_rows,
                    tile_columns,
                    elements_per_lane,
                },
            )
        }
        ProductionTerminalExpansionV1::DisjointSliceGetRowStriped2dMut
            if inputs.len() == 6 && rust_inputs.len() == 6 =>
        {
            let rust_slice = rust_reference_pointee_v1(rust_inputs[0])
                .and_then(|ty| rust_disjoint_slice_v1(tcx, ty));
            let rust_stripe = rust_reference_pointee_v1(rust_inputs[1])
                .and_then(|ty| rust_disjoint_row_stripe_2d_v1(tcx, ty));
            let Some((index_space, lanes_per_row, elements_per_lane)) = rust_stripe else {
                return Err(body_owner_table_mismatch_v1(
                    "terminal row-striped-2d witness identity",
                ));
            };
            if rust_slice.map(|(_, space)| space) != Some(index_space) {
                return Err(body_owner_table_mismatch_v1(
                    "terminal row-striped-2d mapping identity",
                ));
            }
            let disjoint_slice = pointer_pointee_v1(types, inputs[0])?;
            let stripe_witness = pointer_pointee_v1(types, inputs[1])?;
            let element_pointer = aggregate_field_v1(types, disjoint_slice, 0)?;
            let element = pointer_pointee_v1(types, element_pointer)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut {
                    disjoint_slice,
                    stripe_witness,
                    element,
                    raw_index: inputs[2],
                    index_space,
                    lanes_per_row,
                    elements_per_lane,
                },
            )
        }
        ProductionTerminalExpansionV1::KernelContextIssue
        | ProductionTerminalExpansionV1::ThreadIndex(_)
        | ProductionTerminalExpansionV1::WorkgroupIndex(_)
        | ProductionTerminalExpansionV1::WorkgroupDimension(_)
        | ProductionTerminalExpansionV1::GridDimension(_)
        | ProductionTerminalExpansionV1::ThreadIndex1d
        | ProductionTerminalExpansionV1::Invocation3DIndex1D
        | ProductionTerminalExpansionV1::ThreadIndexGet
        | ProductionTerminalExpansionV1::ThreadIndexIntoDisjoint
        | ProductionTerminalExpansionV1::ThreadIndexCheckedShift
        | ProductionTerminalExpansionV1::DisjointIndexGet
        | ProductionTerminalExpansionV1::DisjointBlockComponentIndex
        | ProductionTerminalExpansionV1::DisjointIndexCheckedShift
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceLen
        | ProductionTerminalExpansionV1::DisjointSliceLen
        | ProductionTerminalExpansionV1::DisjointSliceGetMut
        | ProductionTerminalExpansionV1::DisjointSliceGetDisjointMut
        | ProductionTerminalExpansionV1::GridLeaderCurrent
        | ProductionTerminalExpansionV1::DisjointSliceGetMutExclusive
        | ProductionTerminalExpansionV1::ThreadIndexCheckedBlock
        | ProductionTerminalExpansionV1::ThreadIndexCheckedTiled2d
        | ProductionTerminalExpansionV1::ThreadIndexCheckedRowStriped2d
        | ProductionTerminalExpansionV1::DisjointSliceGetBlockMut
        | ProductionTerminalExpansionV1::DisjointSliceGetTiled2dMut
        | ProductionTerminalExpansionV1::DisjointSliceGetRowStriped2dMut
        | ProductionTerminalExpansionV1::MathContextCurrent
        | ProductionTerminalExpansionV1::MathF32(_)
        | ProductionTerminalExpansionV1::RustcFabsF32
        | ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent
        | ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum
        | ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum
        | ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum
        | ProductionTerminalExpansionV1::CollectiveContextCurrent
        | ProductionTerminalExpansionV1::WorkgroupReduceSum
        | ProductionTerminalExpansionV1::SubgroupReduceSumF32
        | ProductionTerminalExpansionV1::SubgroupReduceMaxF32
        | ProductionTerminalExpansionV1::Gfx950SubgroupCurrent
        | ProductionTerminalExpansionV1::Gfx950SubgroupReduceMaxF32
        | ProductionTerminalExpansionV1::Gfx950SubgroupReduceSumF32
        | ProductionTerminalExpansionV1::Gfx950SubgroupBroadcastF32
        | ProductionTerminalExpansionV1::WaveLaneCurrent
        | ProductionTerminalExpansionV1::MatrixContextCurrent
        | ProductionTerminalExpansionV1::Bf16MatrixARowMajor
        | ProductionTerminalExpansionV1::Bf16MatrixBRowMajor
        | ProductionTerminalExpansionV1::Bf16MatrixALoadZeroFilledV2
        | ProductionTerminalExpansionV1::Bf16MatrixBLoadZeroFilledV2
        | ProductionTerminalExpansionV1::StridedReadView2DFromSharedSlice
        | ProductionTerminalExpansionV1::StridedReadView2DLoadOr
        | ProductionTerminalExpansionV1::WorkgroupLdsScopeCurrent
        | ProductionTerminalExpansionV1::DynamicLdsExactCurrent
        | ProductionTerminalExpansionV1::DynamicLdsIntoCollectiveRawParts
        | ProductionTerminalExpansionV1::WorkgroupPipelineCurrent
        | ProductionTerminalExpansionV1::WorkgroupPipelineStage
        | ProductionTerminalExpansionV1::WorkgroupPipelineWrite
        | ProductionTerminalExpansionV1::WorkgroupPipelineCommit
        | ProductionTerminalExpansionV1::WorkgroupPipelineWait
        | ProductionTerminalExpansionV1::WorkgroupPipelineConsume
        | ProductionTerminalExpansionV1::WorkgroupPipelineRead
        | ProductionTerminalExpansionV1::WorkgroupPipelineDiscard
        | ProductionTerminalExpansionV1::WorkgroupPipelineRelease
        | ProductionTerminalExpansionV1::F32MatrixAccumulatorZero
        | ProductionTerminalExpansionV1::F32MatrixAccumulatorIntoValues
        | ProductionTerminalExpansionV1::MatrixMultiplyAccumulate
        | ProductionTerminalExpansionV1::Gfx950MatrixContextCurrent
        | ProductionTerminalExpansionV1::Gfx950Fp4MatrixARowMajor
        | ProductionTerminalExpansionV1::Gfx950Fp4MatrixBRowMajor
        | ProductionTerminalExpansionV1::Gfx950Fp4MatrixALoadM16K128
        | ProductionTerminalExpansionV1::Gfx950Fp4MatrixBLoadK128N16
        | ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorZero
        | ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorIntoValues
        | ProductionTerminalExpansionV1::Gfx950Fp4MultiplyAccumulate
        | ProductionTerminalExpansionV1::Gfx950Fp4Fp8MultiplyAccumulate
        | ProductionTerminalExpansionV1::Gfx950Fp8MatrixARowMajor
        | ProductionTerminalExpansionV1::Gfx950Fp8MatrixBRowMajor
        | ProductionTerminalExpansionV1::Gfx950Fp8MatrixALoadM16K128
        | ProductionTerminalExpansionV1::Gfx950Fp8MatrixBLoadK128N16
        | ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorZero
        | ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorIntoValues
        | ProductionTerminalExpansionV1::Gfx950Fp8MultiplyAccumulate
        | ProductionTerminalExpansionV1::Gfx950LdsTransposeTileCurrent
        | ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB4
        | ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB8
        | ProductionTerminalExpansionV1::Gfx950LdsTransposePublish
        | ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB4
        | ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB8
        | ProductionTerminalExpansionV1::MemoryVolatileLoad
        | ProductionTerminalExpansionV1::Trap
        | ProductionTerminalExpansionV1::ColdPath
        | ProductionTerminalExpansionV1::WorkgroupBarrier
        | ProductionTerminalExpansionV1::Execution(_) => {
            Err(body_owner_table_mismatch_v1("terminal callable ABI"))
        }
    }
}

fn write_only_disjoint_operation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    expansion: crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1,
    inputs: &[SemanticTypeIdV1],
    rust_inputs: &[Ty<'tcx>],
    rust_output: Ty<'tcx>,
    types: &[SemanticTypeDeclV1],
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;

    if !matches!(rust_output.kind(), TyKind::Bool) || inputs.is_empty() || rust_inputs.is_empty() {
        return Err(body_owner_table_mismatch_v1(
            "write-only disjoint terminal ABI",
        ));
    }
    let (rust_element, index_space) = rust_reference_pointee_v1(rust_inputs[0])
        .and_then(|ty| rust_write_only_disjoint_slice_v1(tcx, ty))
        .ok_or_else(|| body_owner_table_mismatch_v1("write-only disjoint receiver"))?;
    let disjoint_slice = pointer_pointee_v1(types, inputs[0])?;
    let element_pointer = aggregate_field_v1(types, disjoint_slice, 0)?;
    let element = pointer_pointee_v1(types, element_pointer)?;

    let (witness, raw_index, kind, value_argument) = match expansion {
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWrite
        | ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteDisjoint => {
            if inputs.len() != 3 || rust_inputs.len() != 3 || rust_inputs[2] != rust_element {
                return Err(body_owner_table_mismatch_v1(
                    "write-only direct write signature",
                ));
            }
            let disjoint =
                expansion == ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteDisjoint;
            let trusted = if disjoint {
                TrustedDeviceItem::DisjointIndex
            } else {
                TrustedDeviceItem::ThreadIndex
            };
            if rust_index_witness_space_v1(tcx, rust_inputs[1], trusted) != Some(index_space) {
                return Err(body_owner_table_mismatch_v1(
                    "write-only direct write mapping",
                ));
            }
            (
                inputs[1],
                aggregate_field_v1(types, inputs[1], 0)?,
                SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint },
                2,
            )
        }
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteExclusive => {
            if inputs.len() != 4 || rust_inputs.len() != 4 || rust_inputs[3] != rust_element {
                return Err(body_owner_table_mismatch_v1(
                    "write-only exclusive write signature",
                ));
            }
            let rust_leader = rust_reference_pointee_v1(rust_inputs[1]);
            if index_space != SemanticDisjointIndexSpaceV1::GridExclusive
                || rust_leader.is_none_or(|ty| {
                    !rust_is_trusted_adt_v1(tcx, ty, TrustedDeviceItem::GridLeader)
                })
            {
                return Err(body_owner_table_mismatch_v1("write-only exclusive mapping"));
            }
            (
                pointer_pointee_v1(types, inputs[1])?,
                inputs[2],
                SemanticWriteOnlyDisjointWriteKindV1::GridExclusive,
                3,
            )
        }
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteBlock => {
            if inputs.len() != 4 || rust_inputs.len() != 4 || rust_inputs[3] != rust_element {
                return Err(body_owner_table_mismatch_v1(
                    "write-only blocked write signature",
                ));
            }
            let Some((expected, lanes_per_block, elements_per_lane)) =
                rust_reference_pointee_v1(rust_inputs[1])
                    .and_then(|ty| rust_disjoint_block_v1(tcx, ty))
            else {
                return Err(body_owner_table_mismatch_v1("write-only blocked witness"));
            };
            if index_space != expected {
                return Err(body_owner_table_mismatch_v1("write-only blocked mapping"));
            }
            (
                pointer_pointee_v1(types, inputs[1])?,
                inputs[2],
                SemanticWriteOnlyDisjointWriteKindV1::Block {
                    lanes_per_block,
                    elements_per_lane,
                },
                3,
            )
        }
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteTiled2d => {
            if inputs.len() != 7 || rust_inputs.len() != 7 || rust_inputs[6] != rust_element {
                return Err(body_owner_table_mismatch_v1(
                    "write-only tiled write signature",
                ));
            }
            let Some((expected, lanes_per_tile, tile_rows, tile_columns, elements_per_lane)) =
                rust_reference_pointee_v1(rust_inputs[1])
                    .and_then(|ty| rust_disjoint_tile_2d_v1(tcx, ty))
            else {
                return Err(body_owner_table_mismatch_v1("write-only tiled witness"));
            };
            if index_space != expected {
                return Err(body_owner_table_mismatch_v1("write-only tiled mapping"));
            }
            (
                pointer_pointee_v1(types, inputs[1])?,
                inputs[2],
                SemanticWriteOnlyDisjointWriteKindV1::Tiled2d {
                    lanes_per_tile,
                    tile_rows,
                    tile_columns,
                    elements_per_lane,
                },
                6,
            )
        }
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteRowStriped2d => {
            if inputs.len() != 7 || rust_inputs.len() != 7 || rust_inputs[6] != rust_element {
                return Err(body_owner_table_mismatch_v1(
                    "write-only row-striped write signature",
                ));
            }
            let Some((expected, lanes_per_row, elements_per_lane)) =
                rust_reference_pointee_v1(rust_inputs[1])
                    .and_then(|ty| rust_disjoint_row_stripe_2d_v1(tcx, ty))
            else {
                return Err(body_owner_table_mismatch_v1(
                    "write-only row-striped witness",
                ));
            };
            if index_space != expected {
                return Err(body_owner_table_mismatch_v1(
                    "write-only row-striped mapping",
                ));
            }
            (
                pointer_pointee_v1(types, inputs[1])?,
                inputs[2],
                SemanticWriteOnlyDisjointWriteKindV1::RowStriped2d {
                    lanes_per_row,
                    elements_per_lane,
                },
                6,
            )
        }
        _ => {
            return Err(body_owner_table_mismatch_v1(
                "write-only disjoint terminal expansion",
            ));
        }
    };
    if inputs[value_argument] != element {
        return Err(body_owner_table_mismatch_v1(
            "write-only disjoint semantic element",
        ));
    }
    Ok(
        SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
            disjoint_slice,
            witness,
            element,
            raw_index,
            index_space,
            kind,
        },
    )
}

fn semantic_f32_math_function_v1(
    function: fe2o3_kernel_ir::F32MathFunction,
) -> SemanticF32MathFunctionV1 {
    use fe2o3_kernel_ir::F32MathFunction as Kernel;
    match function {
        Kernel::Sqrt => SemanticF32MathFunctionV1::Sqrt,
        Kernel::FusedMultiplyAdd => SemanticF32MathFunctionV1::FusedMultiplyAdd,
        Kernel::Floor => SemanticF32MathFunctionV1::Floor,
        Kernel::Ceil => SemanticF32MathFunctionV1::Ceil,
        Kernel::Truncate => SemanticF32MathFunctionV1::Truncate,
        Kernel::RoundTiesEven => SemanticF32MathFunctionV1::RoundTiesEven,
        Kernel::Sin => SemanticF32MathFunctionV1::Sin,
        Kernel::Cos => SemanticF32MathFunctionV1::Cos,
        Kernel::Exp => SemanticF32MathFunctionV1::Exp,
        Kernel::Exp2 => SemanticF32MathFunctionV1::Exp2,
        Kernel::Ln => SemanticF32MathFunctionV1::Ln,
        Kernel::Log2 => SemanticF32MathFunctionV1::Log2,
        Kernel::Log10 => SemanticF32MathFunctionV1::Log10,
        Kernel::Abs => unreachable!("rustc fabs has a distinct semantic intrinsic"),
    }
}

fn single_const_u32_v1(instance: Instance<'_>) -> Option<u32> {
    let mut values = instance
        .args
        .iter()
        .filter_map(|argument| argument.as_const())
        .filter_map(|value| value.try_to_leaf())
        .map(|value| value.to_bits(value.size()));
    let value = u32::try_from(values.next()?).ok()?;
    values.next().is_none().then_some(value)
}

fn semantic_u16_type_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    types.get(ty.index() as usize).is_some_and(|declaration| {
        matches!(
            declaration.shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 16,
            })
        )
    })
}

fn semantic_f32_type_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    types.get(ty.index() as usize).is_some_and(|declaration| {
        matches!(
            declaration.shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 })
        )
    })
}

fn semantic_exact_inhabited_aggregate_zst_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> bool {
    let Some(declaration) = types.get(ty.index() as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Aggregate(aggregate) = declaration.shape() else {
        return false;
    };
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
        return false;
    };
    declaration.layout().size_bytes() == Some(0)
        && !declaration.layout().is_uninhabited()
        && matches!(
            declaration.layout().backend_repr(),
            SemanticBackendReprV1::Memory { sized: true }
        )
        && aggregate.fields().len() == layout.field_offsets().len()
        && layout.field_offsets().iter().all(|offset| *offset == 0)
        && layout.padding().is_empty()
        && aggregate.fields().iter().all(|field| {
            types.get(field.index() as usize).is_some_and(|field| {
                field.layout().size_bytes() == Some(0) && !field.layout().is_uninhabited()
            })
        })
}

fn semantic_bf16_storage_type_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    let Some(declaration) = types.get(ty.index() as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Aggregate(aggregate) = declaration.shape() else {
        return false;
    };
    let [bits] = aggregate.fields() else {
        return false;
    };
    let Some(bits_declaration) = types.get(bits.index() as usize) else {
        return false;
    };
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
        return false;
    };
    semantic_u16_type_v1(types, *bits)
        && declaration.layout().size_bytes() == Some(2)
        && declaration.layout().alignment_bytes() == 2
        && !declaration.layout().is_uninhabited()
        && bits_declaration.layout().size_bytes() == Some(2)
        && bits_declaration.layout().alignment_bytes() == 2
        && !bits_declaration.layout().is_uninhabited()
        && declaration.layout().backend_repr() == bits_declaration.layout().backend_repr()
        && layout.field_offsets() == [0]
        && layout.padding().is_empty()
}

fn single_const_u64_v1(instance: Instance<'_>) -> Option<u64> {
    let mut values = instance
        .args
        .iter()
        .filter_map(|argument| argument.as_const())
        .filter_map(|value| value.try_to_leaf())
        .map(|value| value.to_bits(value.size()));
    let value = u64::try_from(values.next()?).ok()?;
    values.next().is_none().then_some(value)
}

fn rust_workgroup_pipeline_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(Ty<'tcx>, u32, u64, u32)> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if !tcx.is_diagnostic_item(
        Symbol::intern("fe2o3_device_workgroup_pipeline_v1"),
        definition.did(),
    ) {
        return None;
    }
    let type_arguments = arguments.types().collect::<Vec<_>>();
    let [element] = type_arguments.as_slice() else {
        return None;
    };
    let constants = arguments
        .iter()
        .filter_map(|argument| argument.as_const())
        .map(|constant| constant.try_to_target_usize(tcx))
        .collect::<Option<Vec<_>>>()?;
    let [buffers, elements, prefetch_distance] = constants.as_slice() else {
        return None;
    };
    Some((
        *element,
        u32::try_from(*buffers).ok()?,
        u64::try_from(*elements).ok()?,
        u32::try_from(*prefetch_distance).ok()?,
    ))
}

fn rust_dynamic_lds_scalar_element_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if !tcx.is_diagnostic_item(Symbol::intern("fe2o3_device_dynamic_lds"), definition.did()) {
        return None;
    }
    let arguments = arguments.types().collect::<Vec<_>>();
    let [element, _state] = arguments.as_slice() else {
        return None;
    };
    matches!(
        element.kind(),
        TyKind::Uint(UintTy::U8 | UintTy::U16 | UintTy::U32 | UintTy::U64 | UintTy::Usize)
            | TyKind::Int(IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64 | IntTy::Isize)
            | TyKind::Float(FloatTy::F32 | FloatTy::F64)
    )
    .then_some(*element)
}

fn rust_dynamic_lds_uninitialized_scalar_element_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if !tcx.is_diagnostic_item(Symbol::intern("fe2o3_device_dynamic_lds"), definition.did()) {
        return None;
    }
    let arguments = arguments.types().collect::<Vec<_>>();
    let [element, state] = arguments.as_slice() else {
        return None;
    };
    if !rust_is_trusted_adt_v1(tcx, *state, TrustedDeviceItem::LdsUninitialized)
        || !matches!(
            element.kind(),
            TyKind::Int(IntTy::I32) | TyKind::Uint(UintTy::U32) | TyKind::Float(FloatTy::F32)
        )
    {
        return None;
    }
    Some(*element)
}

fn rust_dynamic_lds_raw_parts_v1<'tcx>(output: Ty<'tcx>, element: Ty<'tcx>) -> bool {
    let TyKind::Tuple(fields) = output.kind() else {
        return false;
    };
    fields.len() == 2
        && matches!(
            fields[0].kind(),
            TyKind::RawPtr(pointee, rustc_hir::Mutability::Mut) if *pointee == element
        )
        && matches!(fields[1].kind(), TyKind::Uint(UintTy::Usize))
}

#[allow(clippy::too_many_arguments)]
fn checked_shift_operation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    rust_input: Ty<'tcx>,
    rust_output: Ty<'tcx>,
    input_witness: SemanticTypeIdV1,
    output: SemanticTypeIdV1,
    types: &[SemanticTypeDeclV1],
    input_kind: TrustedDeviceItem,
    thread_index: bool,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    let (input_space, input_brand) = rust_index_witness_contract_v1(tcx, rust_input, input_kind)
        .ok_or_else(|| body_owner_table_mismatch_v1("terminal checked-shift input"))?;
    let rust_output_witness = rust_option_payload_v1(tcx, rust_output)
        .ok_or_else(|| body_owner_table_mismatch_v1("terminal checked-shift result"))?;
    let (output_space, output_brand) =
        rust_index_witness_contract_v1(tcx, rust_output_witness, TrustedDeviceItem::DisjointIndex)
            .ok_or_else(|| body_owner_table_mismatch_v1("terminal checked-shift output"))?;
    if input_brand != output_brand {
        return Err(body_owner_table_mismatch_v1("terminal checked-shift brand"));
    }
    let offset = match (input_space, output_space) {
        (
            SemanticDisjointIndexSpaceV1::Index1d,
            SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset },
        ) => offset,
        _ => {
            return Err(body_owner_table_mismatch_v1(
                "terminal checked-shift mapping",
            ));
        }
    };
    let output_witness = option_payload_v1(types, output)?;
    let raw_index = aggregate_field_v1(types, input_witness, 0)?;
    Ok(if thread_index {
        SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
            input_witness,
            output_witness,
            raw_index,
            input_space,
            output_space,
            offset,
        }
    } else {
        SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
            input_witness,
            output_witness,
            raw_index,
            input_space,
            output_space,
            offset,
        }
    })
}

fn rust_reference_pointee_v1(ty: Ty<'_>) -> Option<Ty<'_>> {
    match *ty.kind() {
        TyKind::Ref(_, pointee, _) => Some(pointee),
        _ => None,
    }
}

fn rust_workgroup_collective_scratch_element_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::WorkgroupCollectiveScratch)
    {
        return None;
    }
    let type_arguments = arguments.types().collect::<Vec<_>>();
    let [element] = type_arguments.as_slice() else {
        return None;
    };
    Some(*element)
}

fn rust_shared_u16_slice_v1(ty: Ty<'_>) -> bool {
    let Some(pointee) = rust_reference_pointee_v1(ty) else {
        return false;
    };
    matches!(*pointee.kind(), TyKind::Slice(element)
        if matches!(element.kind(), TyKind::Uint(UintTy::U16)))
}

fn rust_shared_u8_slice_v1(ty: Ty<'_>) -> bool {
    let Some(pointee) = rust_reference_pointee_v1(ty) else {
        return false;
    };
    matches!(*pointee.kind(), TyKind::Slice(element)
        if matches!(element.kind(), TyKind::Uint(UintTy::U8)))
}

fn rust_shared_slice_element_v1(ty: Ty<'_>) -> Option<Ty<'_>> {
    let TyKind::Ref(_, pointee, rustc_hir::Mutability::Not) = *ty.kind() else {
        return None;
    };
    match *pointee.kind() {
        TyKind::Slice(element) => Some(element),
        _ => None,
    }
}

fn rust_mutable_slice_element_v1(ty: Ty<'_>) -> Option<Ty<'_>> {
    let TyKind::Ref(_, pointee, rustc_hir::Mutability::Mut) = *ty.kind() else {
        return None;
    };
    match *pointee.kind() {
        TyKind::Slice(element) => Some(element),
        _ => None,
    }
}

fn rust_supported_read_view_scalar_v1(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Bool
            | TyKind::Char
            | TyKind::Int(IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64)
            | TyKind::Uint(UintTy::U8 | UintTy::U16 | UintTy::U32 | UintTy::U64)
            | TyKind::Float(FloatTy::F32 | FloatTy::F64)
    )
}

fn rust_supported_volatile_load_scalar_v1(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Bool
            | TyKind::Int(IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64)
            | TyKind::Uint(UintTy::U8 | UintTy::U16 | UintTy::U32 | UintTy::U64)
            | TyKind::Float(FloatTy::F32 | FloatTy::F64)
    )
}

pub(crate) fn rust_option_payload_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    (tcx.is_diagnostic_item(sym::Option, definition.did()) && arguments.len() == 1)
        .then(|| arguments[0].as_type())
        .flatten()
}

fn rust_result_payloads_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<(Ty<'tcx>, Ty<'tcx>)> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    (tcx.is_diagnostic_item(sym::Result, definition.did()) && arguments.len() == 2)
        .then(|| Some((arguments[0].as_type()?, arguments[1].as_type()?)))
        .flatten()
}

fn rust_is_trusted_adt_v1(tcx: TyCtxt<'_>, ty: Ty<'_>, item: TrustedDeviceItem) -> bool {
    matches!(*ty.kind(), TyKind::Adt(definition, arguments)
        if arguments
            .iter()
            .all(|argument| matches!(argument.kind(), GenericArgKind::Lifetime(_)))
            && trusted_device_items::classify(tcx, definition.did()) == Some(item))
}

fn rust_trusted_adt_type_arguments_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    item: TrustedDeviceItem,
) -> Option<Vec<Ty<'tcx>>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    (trusted_device_items::classify(tcx, definition.did()) == Some(item))
        .then(|| arguments.types().collect())
}

fn rust_is_exact_trusted_marker_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    item: TrustedDeviceItem,
) -> bool {
    rust_trusted_adt_type_arguments_v1(tcx, ty, item).is_some_and(|arguments| arguments.is_empty())
}

fn rust_wave_lane64_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::WaveLane).is_some_and(
        |arguments| {
            matches!(arguments.as_slice(), [width]
                if rust_is_exact_trusted_marker_v1(tcx, *width, TrustedDeviceItem::Wave64))
        },
    )
}

fn rust_mfma_fragment_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticMfmaOperandContractV1> {
    let arguments =
        rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::Bf16MfmaFragment)?;
    let [role, profile, distribution, width] = arguments.as_slice() else {
        return None;
    };
    let role = if rust_is_exact_trusted_marker_v1(tcx, *role, TrustedDeviceItem::MfmaOperandA) {
        SemanticMfmaOperandRoleV1::A
    } else if rust_is_exact_trusted_marker_v1(tcx, *role, TrustedDeviceItem::MfmaOperandB) {
        SemanticMfmaOperandRoleV1::B
    } else {
        return None;
    };
    rust_is_exact_trusted_marker_v1(tcx, *profile, TrustedDeviceItem::Bf16MfmaProfile)
        .then_some(())?;
    rust_is_exact_trusted_marker_v1(tcx, *distribution, TrustedDeviceItem::MfmaRegisterTile16x16)
        .then_some(())?;
    rust_is_exact_trusted_marker_v1(tcx, *width, TrustedDeviceItem::Wave64).then_some(())?;
    Some(SemanticMfmaOperandContractV1 {
        role,
        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
        register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
        wave_width: 64,
    })
}

fn rust_mfma_accumulator_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticMfmaAccumulatorContractV1> {
    let arguments =
        rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::F32AccumulatorFragment)?;
    let [profile, distribution, width] = arguments.as_slice() else {
        return None;
    };
    rust_is_exact_trusted_marker_v1(tcx, *profile, TrustedDeviceItem::Bf16MfmaProfile)
        .then_some(())?;
    rust_is_exact_trusted_marker_v1(
        tcx,
        *distribution,
        TrustedDeviceItem::MfmaAccumulatorRowMajor,
    )
    .then_some(())?;
    rust_is_exact_trusted_marker_v1(tcx, *width, TrustedDeviceItem::Wave64).then_some(())?;
    Some(SemanticMfmaAccumulatorContractV1 {
        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
        distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
        wave_width: 64,
    })
}

fn rust_mfma_matrix_role_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticMfmaOperandRoleV1> {
    let arguments =
        rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::Bf16MfmaMatrixView)?;
    let [role] = arguments.as_slice() else {
        return None;
    };
    if rust_is_exact_trusted_marker_v1(tcx, *role, TrustedDeviceItem::MfmaOperandA) {
        Some(SemanticMfmaOperandRoleV1::A)
    } else if rust_is_exact_trusted_marker_v1(tcx, *role, TrustedDeviceItem::MfmaOperandB) {
        Some(SemanticMfmaOperandRoleV1::B)
    } else {
        None
    }
}

fn rust_gfx950_fp8_fragment_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticMfmaOperandContractV1> {
    let arguments =
        rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::Gfx950MfmaFragment)?;
    let [format, role] = arguments.as_slice() else {
        return None;
    };
    rust_is_exact_trusted_marker_v1(tcx, *format, TrustedDeviceItem::Gfx950Fp8E4M3Format)
        .then_some(())?;
    let role = if rust_is_exact_trusted_marker_v1(tcx, *role, TrustedDeviceItem::Gfx950MfmaOperandA)
    {
        SemanticMfmaOperandRoleV1::A
    } else if rust_is_exact_trusted_marker_v1(tcx, *role, TrustedDeviceItem::Gfx950MfmaOperandB) {
        SemanticMfmaOperandRoleV1::B
    } else {
        return None;
    };
    Some(SemanticMfmaOperandContractV1 {
        role,
        profile: SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
        register_distribution: SemanticMfmaRegisterDistributionV1::Gfx950M16N16K128,
        wave_width: 64,
    })
}

fn rust_gfx950_fp4_fragment_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticMfmaOperandContractV1> {
    let arguments =
        rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::Gfx950MfmaFragment)?;
    let [format, role] = arguments.as_slice() else {
        return None;
    };
    rust_is_exact_trusted_marker_v1(tcx, *format, TrustedDeviceItem::Gfx950Fp4E2M1Format)
        .then_some(())?;
    let role = if rust_is_exact_trusted_marker_v1(tcx, *role, TrustedDeviceItem::Gfx950MfmaOperandA)
    {
        SemanticMfmaOperandRoleV1::A
    } else if rust_is_exact_trusted_marker_v1(tcx, *role, TrustedDeviceItem::Gfx950MfmaOperandB) {
        SemanticMfmaOperandRoleV1::B
    } else {
        return None;
    };
    Some(SemanticMfmaOperandContractV1 {
        role,
        profile: SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128,
        register_distribution: SemanticMfmaRegisterDistributionV1::Gfx950M16N16K128,
        wave_width: 64,
    })
}

fn rust_gfx950_fp8_accumulator_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticMfmaAccumulatorContractV1> {
    let arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        ty,
        TrustedDeviceItem::Gfx950F32AccumulatorFragment,
    )?;
    let [format] = arguments.as_slice() else {
        return None;
    };
    rust_is_exact_trusted_marker_v1(tcx, *format, TrustedDeviceItem::Gfx950Fp8E4M3Format)
        .then_some(())?;
    Some(SemanticMfmaAccumulatorContractV1 {
        profile: SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
        distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
        wave_width: 64,
    })
}

fn rust_gfx950_fp4_accumulator_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticMfmaAccumulatorContractV1> {
    let arguments = rust_trusted_adt_type_arguments_v1(
        tcx,
        ty,
        TrustedDeviceItem::Gfx950F32AccumulatorFragment,
    )?;
    let [format] = arguments.as_slice() else {
        return None;
    };
    rust_is_exact_trusted_marker_v1(tcx, *format, TrustedDeviceItem::Gfx950Fp4E2M1Format)
        .then_some(())?;
    Some(SemanticMfmaAccumulatorContractV1 {
        profile: SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128,
        distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
        wave_width: 64,
    })
}

fn rust_gfx950_fp8_matrix_role_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticMfmaOperandRoleV1> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    let role = match trusted_device_items::classify(tcx, definition.did())? {
        TrustedDeviceItem::Gfx950MfmaMatrixAView => SemanticMfmaOperandRoleV1::A,
        TrustedDeviceItem::Gfx950MfmaMatrixBView => SemanticMfmaOperandRoleV1::B,
        _ => return None,
    };
    let mut formats = arguments.types();
    let format = formats.next()?;
    if formats.next().is_some()
        || !rust_is_exact_trusted_marker_v1(tcx, format, TrustedDeviceItem::Gfx950Fp8E4M3Format)
    {
        return None;
    }
    Some(role)
}

fn rust_gfx950_fp4_matrix_role_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticMfmaOperandRoleV1> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    let role = match trusted_device_items::classify(tcx, definition.did())? {
        TrustedDeviceItem::Gfx950MfmaMatrixAView => SemanticMfmaOperandRoleV1::A,
        TrustedDeviceItem::Gfx950MfmaMatrixBView => SemanticMfmaOperandRoleV1::B,
        _ => return None,
    };
    let mut formats = arguments.types();
    let format = formats.next()?;
    if formats.next().is_some()
        || !rust_is_exact_trusted_marker_v1(tcx, format, TrustedDeviceItem::Gfx950Fp4E2M1Format)
    {
        return None;
    }
    Some(role)
}

fn rust_gfx950_lds_transpose_format_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticGfx950LdsTransposeFormatV1> {
    let arguments =
        rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::Gfx950LdsTransposeTile)?;
    let [format, _state] = arguments.as_slice() else {
        return None;
    };
    if rust_is_exact_trusted_marker_v1(tcx, *format, TrustedDeviceItem::Gfx950Fp4E2M1Format) {
        Some(SemanticGfx950LdsTransposeFormatV1::Fp4E2M1)
    } else if rust_is_exact_trusted_marker_v1(tcx, *format, TrustedDeviceItem::Gfx950Fp8E4M3Format)
    {
        Some(SemanticGfx950LdsTransposeFormatV1::Fp8E4M3)
    } else {
        None
    }
}

fn rust_f32_array_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>, expected_length: u64) -> bool {
    let TyKind::Array(element, length) = *ty.kind() else {
        return false;
    };
    matches!(element.kind(), TyKind::Float(FloatTy::F32))
        && length.try_to_target_usize(tcx) == Some(expected_length)
}

fn rust_disjoint_slice_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(Ty<'tcx>, SemanticDisjointIndexSpaceV1)> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::DisjointSlice)
        || arguments.len() != 2
    {
        return None;
    }
    Some((
        arguments[0].as_type()?,
        rust_disjoint_index_space_v1(tcx, arguments[1].as_type()?)?,
    ))
}

fn rust_write_only_disjoint_slice_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(Ty<'tcx>, SemanticDisjointIndexSpaceV1)> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::WriteOnlyDisjointSlice)
        || arguments.len() != 2
    {
        return None;
    }
    Some((
        arguments[0].as_type()?,
        rust_disjoint_index_space_v1(tcx, arguments[1].as_type()?)?,
    ))
}

pub(crate) fn rust_index_witness_space_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    item: TrustedDeviceItem,
) -> Option<SemanticDisjointIndexSpaceV1> {
    rust_index_witness_contract_v1(tcx, ty, item).map(|(space, _)| space)
}

fn rust_index_witness_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    item: TrustedDeviceItem,
) -> Option<(SemanticDisjointIndexSpaceV1, Ty<'tcx>)> {
    let (index_space, brand) = rust_branded_index_v1(tcx, ty, item)?;
    Some((rust_disjoint_index_space_v1(tcx, index_space)?, brand))
}

fn rust_is_unbranded_capability_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    matches!(
        *ty.kind(),
        TyKind::Adt(definition, arguments)
            if arguments.is_empty()
                && tcx.def_path_str(definition.did()) == "fe2o3_device::UnbrandedCapability"
    )
}

fn rust_disjoint_index_space_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<SemanticDisjointIndexSpaceV1> {
    if ty == trusted_index1d_type_v1(tcx)? {
        return Some(SemanticDisjointIndexSpaceV1::Index1d);
    }
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    match trusted_device_items::classify(tcx, definition.did()) {
        Some(TrustedDeviceItem::ShiftedIndexSpace) if arguments.len() == 2 => {
            let base = arguments[0].as_type()?;
            if base != trusted_index1d_type_v1(tcx)? {
                return None;
            }
            let offset = arguments[1].as_const()?.try_to_target_usize(tcx)?;
            Some(SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset })
        }
        Some(TrustedDeviceItem::GridExclusiveIndexSpace) if arguments.is_empty() => {
            Some(SemanticDisjointIndexSpaceV1::GridExclusive)
        }
        Some(TrustedDeviceItem::BlockedIndexSpace) if arguments.len() == 3 => {
            if arguments[0].as_type()? != trusted_index1d_type_v1(tcx)? {
                return None;
            }
            let lanes_per_block = arguments[1].as_const()?.try_to_target_usize(tcx)?;
            let elements_per_lane = arguments[2].as_const()?.try_to_target_usize(tcx)?;
            if lanes_per_block == 0
                || elements_per_lane == 0
                || lanes_per_block.checked_mul(elements_per_lane).is_none()
            {
                return None;
            }
            Some(SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                lanes_per_block,
                elements_per_lane,
            })
        }
        Some(TrustedDeviceItem::Tiled2DIndexSpace) if arguments.len() == 5 => {
            if arguments[0].as_type()? != trusted_index1d_type_v1(tcx)? {
                return None;
            }
            let lanes_per_tile = arguments[1].as_const()?.try_to_target_usize(tcx)?;
            let tile_rows = arguments[2].as_const()?.try_to_target_usize(tcx)?;
            let tile_columns = arguments[3].as_const()?.try_to_target_usize(tcx)?;
            let elements_per_lane = arguments[4].as_const()?.try_to_target_usize(tcx)?;
            rust_tiled_2d_geometry_valid_v1(
                lanes_per_tile,
                tile_rows,
                tile_columns,
                elements_per_lane,
            )
            .then_some(SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
                lanes_per_tile,
                tile_rows,
                tile_columns,
                elements_per_lane,
            })
        }
        Some(TrustedDeviceItem::RowStriped2DIndexSpace) if arguments.len() == 3 => {
            if arguments[0].as_type()? != trusted_index1d_type_v1(tcx)? {
                return None;
            }
            let lanes_per_row = arguments[1].as_const()?.try_to_target_usize(tcx)?;
            let elements_per_lane = arguments[2].as_const()?.try_to_target_usize(tcx)?;
            rust_row_striped_2d_geometry_valid_v1(lanes_per_row, elements_per_lane).then_some(
                SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
                    lanes_per_row,
                    elements_per_lane,
                },
            )
        }
        _ => None,
    }
}

fn rust_tiled_2d_geometry_valid_v1(
    lanes_per_tile: u64,
    tile_rows: u64,
    tile_columns: u64,
    elements_per_lane: u64,
) -> bool {
    lanes_per_tile != 0
        && tile_rows != 0
        && tile_columns != 0
        && elements_per_lane != 0
        && lanes_per_tile.is_multiple_of(tile_columns)
        && lanes_per_tile.checked_mul(elements_per_lane) == tile_rows.checked_mul(tile_columns)
        && (lanes_per_tile / tile_columns).checked_mul(elements_per_lane) == Some(tile_rows)
}

fn rust_row_striped_2d_geometry_valid_v1(lanes_per_row: u64, elements_per_lane: u64) -> bool {
    lanes_per_row != 0
        && elements_per_lane != 0
        && (elements_per_lane - 1)
            .checked_mul(lanes_per_row)
            .and_then(|base| base.checked_add(lanes_per_row - 1))
            .is_some()
}

fn rust_disjoint_block_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(SemanticDisjointIndexSpaceV1, u64, u64)> {
    rust_disjoint_block_contract_v1(tcx, ty).map(
        |(index_space, lanes_per_block, elements_per_lane, _)| {
            (index_space, lanes_per_block, elements_per_lane)
        },
    )
}

fn rust_disjoint_block_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(SemanticDisjointIndexSpaceV1, u64, u64, Ty<'tcx>)> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::DisjointBlock)
        || arguments.len() != 4
    {
        return None;
    }
    let base = arguments[0].as_type()?;
    if base != trusted_index1d_type_v1(tcx)? {
        return None;
    }
    let lanes_per_block = arguments[1].as_const()?.try_to_target_usize(tcx)?;
    let elements_per_lane = arguments[2].as_const()?.try_to_target_usize(tcx)?;
    if lanes_per_block == 0
        || elements_per_lane == 0
        || lanes_per_block.checked_mul(elements_per_lane).is_none()
    {
        return None;
    }
    Some((
        SemanticDisjointIndexSpaceV1::BlockedIndex1d {
            lanes_per_block,
            elements_per_lane,
        },
        lanes_per_block,
        elements_per_lane,
        arguments[3].as_type()?,
    ))
}

fn require_rust_disjoint_block_component_contract_v1(
    receiver_is_shared: bool,
    mapping: Option<(SemanticDisjointIndexSpaceV1, u64, u64)>,
    component_is_usize: bool,
    output_is_option_usize: bool,
) -> Result<(SemanticDisjointIndexSpaceV1, u64, u64), ProductionSemanticImportErrorV1> {
    if !receiver_is_shared {
        return Err(body_owner_table_mismatch_v1(
            "terminal disjoint-block receiver",
        ));
    }
    let mapping =
        mapping.ok_or_else(|| body_owner_table_mismatch_v1("terminal disjoint-block mapping"))?;
    if !component_is_usize || !output_is_option_usize {
        return Err(body_owner_table_mismatch_v1(
            "terminal disjoint-block component or output",
        ));
    }
    Ok(mapping)
}

pub(crate) fn rust_disjoint_tile_2d_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(SemanticDisjointIndexSpaceV1, u64, u64, u64, u64)> {
    rust_disjoint_tile_2d_contract_v1(tcx, ty).map(
        |(mapping, lanes_per_tile, tile_rows, tile_columns, elements_per_lane, _)| {
            (
                mapping,
                lanes_per_tile,
                tile_rows,
                tile_columns,
                elements_per_lane,
            )
        },
    )
}

fn rust_disjoint_tile_2d_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(SemanticDisjointIndexSpaceV1, u64, u64, u64, u64, Ty<'tcx>)> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::DisjointTile2D)
        || arguments.len() != 6
        || arguments[0].as_type()? != trusted_index1d_type_v1(tcx)?
    {
        return None;
    }
    let lanes_per_tile = arguments[1].as_const()?.try_to_target_usize(tcx)?;
    let tile_rows = arguments[2].as_const()?.try_to_target_usize(tcx)?;
    let tile_columns = arguments[3].as_const()?.try_to_target_usize(tcx)?;
    let elements_per_lane = arguments[4].as_const()?.try_to_target_usize(tcx)?;
    if !rust_tiled_2d_geometry_valid_v1(lanes_per_tile, tile_rows, tile_columns, elements_per_lane)
    {
        return None;
    }
    Some((
        SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
            lanes_per_tile,
            tile_rows,
            tile_columns,
            elements_per_lane,
        },
        lanes_per_tile,
        tile_rows,
        tile_columns,
        elements_per_lane,
        arguments[5].as_type()?,
    ))
}

fn rust_disjoint_row_stripe_2d_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(SemanticDisjointIndexSpaceV1, u64, u64)> {
    rust_disjoint_row_stripe_2d_contract_v1(tcx, ty).map(
        |(mapping, lanes_per_row, elements_per_lane, _)| {
            (mapping, lanes_per_row, elements_per_lane)
        },
    )
}

fn rust_disjoint_row_stripe_2d_contract_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(SemanticDisjointIndexSpaceV1, u64, u64, Ty<'tcx>)> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::DisjointRowStripe2D)
        || arguments.len() != 4
        || arguments[0].as_type()? != trusted_index1d_type_v1(tcx)?
    {
        return None;
    }
    let lanes_per_row = arguments[1].as_const()?.try_to_target_usize(tcx)?;
    let elements_per_lane = arguments[2].as_const()?.try_to_target_usize(tcx)?;
    if !rust_row_striped_2d_geometry_valid_v1(lanes_per_row, elements_per_lane) {
        return None;
    }
    Some((
        SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
            lanes_per_row,
            elements_per_lane,
        },
        lanes_per_row,
        elements_per_lane,
        arguments[3].as_type()?,
    ))
}

fn trusted_index1d_type_v1<'tcx>(tcx: TyCtxt<'tcx>) -> Option<Ty<'tcx>> {
    let function = trusted_device_items::definition(tcx, TrustedDeviceItem::ThreadIndex1d)?;
    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(function).instantiate_identity());
    let TyKind::Adt(definition, arguments) = *signature.output().kind() else {
        return None;
    };
    (trusted_device_items::classify(tcx, definition.did()) == Some(TrustedDeviceItem::ThreadIndex)
        && arguments.len() == 2
        && arguments
            .get(1)
            .and_then(|argument| argument.as_type())
            .is_some_and(|brand| rust_is_unbranded_capability_v1(tcx, brand)))
    .then(|| arguments[0].as_type())
    .flatten()
}

fn option_payload_v1(
    types: &[SemanticTypeDeclV1],
    option: SemanticTypeIdV1,
) -> Result<SemanticTypeIdV1, ProductionSemanticImportErrorV1> {
    let declaration = types
        .get(option.index() as usize)
        .ok_or_else(|| body_owner_table_mismatch_v1("terminal option type"))?;
    let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
        return Err(body_owner_table_mismatch_v1("terminal option type"));
    };
    variants
        .get(1)
        .and_then(|variant| variant.fields().fields().first())
        .copied()
        .ok_or_else(|| body_owner_table_mismatch_v1("terminal option payload"))
}

fn aggregate_field_v1(
    types: &[SemanticTypeDeclV1],
    aggregate: SemanticTypeIdV1,
    field: usize,
) -> Result<SemanticTypeIdV1, ProductionSemanticImportErrorV1> {
    let declaration = types
        .get(aggregate.index() as usize)
        .ok_or_else(|| body_owner_table_mismatch_v1("terminal aggregate type"))?;
    let SemanticTypeShapeV1::Aggregate(fields) = declaration.shape() else {
        return Err(body_owner_table_mismatch_v1("terminal aggregate type"));
    };
    fields
        .fields()
        .get(field)
        .copied()
        .ok_or_else(|| body_owner_table_mismatch_v1("terminal aggregate field"))
}

fn tuple_field_v1(
    types: &[SemanticTypeDeclV1],
    tuple: SemanticTypeIdV1,
    field: usize,
) -> Result<SemanticTypeIdV1, ProductionSemanticImportErrorV1> {
    let declaration = types
        .get(tuple.index() as usize)
        .ok_or_else(|| body_owner_table_mismatch_v1("terminal tuple type"))?;
    let SemanticTypeShapeV1::Tuple(fields) = declaration.shape() else {
        return Err(body_owner_table_mismatch_v1("terminal tuple type"));
    };
    fields
        .fields()
        .get(field)
        .copied()
        .ok_or_else(|| body_owner_table_mismatch_v1("terminal tuple field"))
}

fn dynamic_lds_element_storage_v1(
    types: &[SemanticTypeDeclV1],
    dynamic_lds: SemanticTypeIdV1,
) -> Result<SemanticTypeIdV1, ProductionSemanticImportErrorV1> {
    let mut current = aggregate_field_v1(types, dynamic_lds, 0)?;
    for _ in 0..4 {
        let declaration = types
            .get(current.index() as usize)
            .ok_or_else(|| body_owner_table_mismatch_v1("exact LDS pointer wrapper"))?;
        match declaration.shape() {
            SemanticTypeShapeV1::Aggregate(wrapper) | SemanticTypeShapeV1::Union(wrapper)
                if wrapper.fields().len() == 1 =>
            {
                current = wrapper.fields()[0];
            }
            SemanticTypeShapeV1::Pointer(pointer) => return Ok(pointer.pointee()),
            _ => return Err(body_owner_table_mismatch_v1("exact LDS pointer wrapper")),
        }
    }
    Err(body_owner_table_mismatch_v1("exact LDS pointer depth"))
}

fn semantic_result_payloads_v1(
    types: &[SemanticTypeDeclV1],
    result: SemanticTypeIdV1,
) -> Result<(SemanticTypeIdV1, SemanticTypeIdV1), ProductionSemanticImportErrorV1> {
    let Some(declaration) = types.get(result.index() as usize) else {
        return Err(body_owner_table_mismatch_v1("semantic Result shape"));
    };
    let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
        return Err(body_owner_table_mismatch_v1("semantic Result shape"));
    };
    if variants.len() != 2
        || variants[0].discriminant() != 0
        || variants[0].fields().fields().len() != 1
        || variants[1].discriminant() != 1
        || variants[1].fields().fields().len() != 1
    {
        return Err(body_owner_table_mismatch_v1("semantic Result variants"));
    }
    Ok((
        variants[0].fields().fields()[0],
        variants[1].fields().fields()[0],
    ))
}

fn pointer_pointee_v1(
    types: &[SemanticTypeDeclV1],
    pointer: SemanticTypeIdV1,
) -> Result<SemanticTypeIdV1, ProductionSemanticImportErrorV1> {
    let declaration = types
        .get(pointer.index() as usize)
        .ok_or_else(|| body_owner_table_mismatch_v1("terminal pointer type"))?;
    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
        return Err(body_owner_table_mismatch_v1("terminal pointer type"));
    };
    Ok(pointer.pointee())
}

#[cfg(test)]
// Frozen calculator for the independently published BF16 and pipeline V1 histories.
const fn terminal_operation_tag_v1(
    expansion: crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1,
) -> u8 {
    terminal_operation_tag_for_schema_v1(expansion, TerminalIdentitySchemaV1::IndependentV1)
}

const fn terminal_operation_tag_for_schema_v1(
    expansion: crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1,
    schema: TerminalIdentitySchemaV1,
) -> u8 {
    use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
    match expansion {
        ProductionTerminalExpansionV1::ThreadIndex(
            fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1::X,
        ) => 13,
        ProductionTerminalExpansionV1::ThreadIndex(
            fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1::Y,
        ) => 14,
        ProductionTerminalExpansionV1::ThreadIndex(
            fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1::Z,
        ) => 15,
        ProductionTerminalExpansionV1::WorkgroupIndex(
            fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1::X,
        ) => 16,
        ProductionTerminalExpansionV1::WorkgroupIndex(
            fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1::Y,
        ) => 17,
        ProductionTerminalExpansionV1::WorkgroupIndex(
            fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1::Z,
        ) => 18,
        ProductionTerminalExpansionV1::WorkgroupDimension(
            fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1::X,
        ) => 19,
        ProductionTerminalExpansionV1::WorkgroupDimension(
            fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1::Y,
        ) => 20,
        ProductionTerminalExpansionV1::WorkgroupDimension(
            fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1::Z,
        ) => 21,
        ProductionTerminalExpansionV1::GridDimension(
            fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1::X,
        ) => 22,
        ProductionTerminalExpansionV1::GridDimension(
            fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1::Y,
        ) => 23,
        ProductionTerminalExpansionV1::GridDimension(
            fe2o3_mir_model::semantic_mir_v1::SemanticAxisV1::Z,
        ) => 24,
        ProductionTerminalExpansionV1::DisjointSliceLen => 25,
        ProductionTerminalExpansionV1::ThreadIndex1d => 0,
        ProductionTerminalExpansionV1::Invocation3DIndex1D => 167,
        ProductionTerminalExpansionV1::ThreadIndexGet => 1,
        ProductionTerminalExpansionV1::DisjointSliceGetMut => 2,
        ProductionTerminalExpansionV1::ThreadIndexIntoDisjoint => 3,
        ProductionTerminalExpansionV1::ThreadIndexCheckedShift => 4,
        ProductionTerminalExpansionV1::DisjointIndexGet => 5,
        ProductionTerminalExpansionV1::DisjointIndexCheckedShift => 6,
        ProductionTerminalExpansionV1::DisjointSliceGetDisjointMut => 7,
        ProductionTerminalExpansionV1::GridLeaderCurrent => 8,
        ProductionTerminalExpansionV1::DisjointSliceGetMutExclusive => 9,
        ProductionTerminalExpansionV1::ThreadIndexCheckedBlock => 10,
        ProductionTerminalExpansionV1::DisjointSliceGetBlockMut => 11,
        ProductionTerminalExpansionV1::WorkgroupBarrier => 12,
        ProductionTerminalExpansionV1::MatrixContextCurrent => 26,
        ProductionTerminalExpansionV1::F32MatrixAccumulatorIntoValues => 29,
        ProductionTerminalExpansionV1::MatrixMultiplyAccumulate => 30,
        ProductionTerminalExpansionV1::ThreadIndexCheckedTiled2d => 31,
        ProductionTerminalExpansionV1::DisjointSliceGetTiled2dMut => 32,
        ProductionTerminalExpansionV1::CollectiveContextCurrent => 33,
        ProductionTerminalExpansionV1::SubgroupReduceSumF32 => 34,
        ProductionTerminalExpansionV1::SubgroupReduceMaxF32 => 35,
        ProductionTerminalExpansionV1::MathContextCurrent => 36,
        ProductionTerminalExpansionV1::MathF32(function) => 37 + f32_math_tag_v1(function),
        ProductionTerminalExpansionV1::ColdPath => 50,
        ProductionTerminalExpansionV1::WaveLaneCurrent => 51,
        ProductionTerminalExpansionV1::Bf16MatrixARowMajor => 52,
        ProductionTerminalExpansionV1::Bf16MatrixBRowMajor => 53,
        ProductionTerminalExpansionV1::Bf16MatrixALoadZeroFilledV2 => 54,
        ProductionTerminalExpansionV1::Bf16MatrixBLoadZeroFilledV2 => 55,
        ProductionTerminalExpansionV1::F32MatrixAccumulatorZero => 56,
        ProductionTerminalExpansionV1::StridedReadView2DFromSharedSlice => 57,
        ProductionTerminalExpansionV1::StridedReadView2DLoadOr => 58,
        ProductionTerminalExpansionV1::ThreadIndexCheckedRowStriped2d => 59,
        ProductionTerminalExpansionV1::DisjointSliceGetRowStriped2dMut => 60,
        ProductionTerminalExpansionV1::Gfx950MatrixContextCurrent => 61,
        ProductionTerminalExpansionV1::Gfx950Fp8MatrixARowMajor => 62,
        ProductionTerminalExpansionV1::Gfx950Fp8MatrixBRowMajor => 63,
        ProductionTerminalExpansionV1::Gfx950Fp8MatrixALoadM16K128 => 64,
        ProductionTerminalExpansionV1::Gfx950Fp8MatrixBLoadK128N16 => 65,
        ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorZero => 66,
        ProductionTerminalExpansionV1::Gfx950Fp8AccumulatorIntoValues => 67,
        ProductionTerminalExpansionV1::Gfx950Fp8MultiplyAccumulate => 68,
        ProductionTerminalExpansionV1::Gfx950Fp4MatrixARowMajor => 69,
        ProductionTerminalExpansionV1::Gfx950Fp4MatrixBRowMajor => 70,
        ProductionTerminalExpansionV1::Gfx950Fp4MatrixALoadM16K128 => 71,
        ProductionTerminalExpansionV1::Gfx950Fp4MatrixBLoadK128N16 => 72,
        ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorZero => 73,
        ProductionTerminalExpansionV1::Gfx950Fp4AccumulatorIntoValues => 74,
        ProductionTerminalExpansionV1::Gfx950Fp4MultiplyAccumulate => 75,
        ProductionTerminalExpansionV1::Gfx950SubgroupCurrent => 76,
        ProductionTerminalExpansionV1::Gfx950SubgroupReduceMaxF32 => 77,
        ProductionTerminalExpansionV1::Gfx950SubgroupReduceSumF32 => 78,
        ProductionTerminalExpansionV1::Gfx950SubgroupBroadcastF32 => 79,
        ProductionTerminalExpansionV1::Gfx950LdsTransposeTileCurrent => 80,
        ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB4 => 81,
        ProductionTerminalExpansionV1::Gfx950LdsTransposeStageB8 => 82,
        ProductionTerminalExpansionV1::Gfx950LdsTransposePublish => 83,
        ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB4 => 84,
        ProductionTerminalExpansionV1::Gfx950LdsTransposeReadB8 => 85,
        ProductionTerminalExpansionV1::Trap => 86,
        ProductionTerminalExpansionV1::Gfx950Fp4Fp8MultiplyAccumulate => 87,
        ProductionTerminalExpansionV1::DynamicLdsExactCurrent => 88,
        ProductionTerminalExpansionV1::WorkgroupReduceSum => 89,
        ProductionTerminalExpansionV1::DynamicLdsIntoCollectiveRawParts => 90,
        ProductionTerminalExpansionV1::WorkgroupPipelineCurrent => 91,
        ProductionTerminalExpansionV1::WorkgroupPipelineStage => 92,
        ProductionTerminalExpansionV1::WorkgroupPipelineWrite => 93,
        ProductionTerminalExpansionV1::WorkgroupPipelineCommit => 94,
        ProductionTerminalExpansionV1::WorkgroupPipelineWait => 95,
        ProductionTerminalExpansionV1::WorkgroupPipelineConsume => 96,
        ProductionTerminalExpansionV1::WorkgroupPipelineRead => 97,
        ProductionTerminalExpansionV1::WorkgroupPipelineDiscard => 98,
        ProductionTerminalExpansionV1::WorkgroupPipelineRelease => 99,
        ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent => match schema {
            #[cfg(test)]
            TerminalIdentitySchemaV1::IndependentV1 | TerminalIdentitySchemaV1::CombinedV2 => 104,
            TerminalIdentitySchemaV1::CombinedV3 | TerminalIdentitySchemaV1::CombinedV4 => 111,
        },
        ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum => match schema {
            #[cfg(test)]
            TerminalIdentitySchemaV1::IndependentV1 | TerminalIdentitySchemaV1::CombinedV2 => 105,
            TerminalIdentitySchemaV1::CombinedV3 | TerminalIdentitySchemaV1::CombinedV4 => 112,
        },
        ProductionTerminalExpansionV1::RustcFabsF32 => 113,
        ProductionTerminalExpansionV1::MemoryVolatileLoad => 115,
        ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum => match schema {
            #[cfg(test)]
            TerminalIdentitySchemaV1::IndependentV1 | TerminalIdentitySchemaV1::CombinedV2 => 106,
            TerminalIdentitySchemaV1::CombinedV3 => 113,
            TerminalIdentitySchemaV1::CombinedV4 => 116,
        },
        ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum => match schema {
            #[cfg(test)]
            TerminalIdentitySchemaV1::IndependentV1 | TerminalIdentitySchemaV1::CombinedV2 => 107,
            TerminalIdentitySchemaV1::CombinedV3 => 114,
            TerminalIdentitySchemaV1::CombinedV4 => 117,
        },
        ProductionTerminalExpansionV1::WorkgroupLdsScopeCurrent => 118,
        ProductionTerminalExpansionV1::DisjointBlockComponentIndex => 119,
        ProductionTerminalExpansionV1::Bf16Conversion(conversion) => {
            let base = match schema {
                #[cfg(test)]
                TerminalIdentitySchemaV1::IndependentV1 => 91,
                #[cfg(test)]
                TerminalIdentitySchemaV1::CombinedV2 => 100,
                TerminalIdentitySchemaV1::CombinedV3 | TerminalIdentitySchemaV1::CombinedV4 => 100,
            };
            base + match conversion {
                crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::FromBits => 0,
                crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::ToBits => 1,
                crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::FromF32RoundTiesEven => 2,
                crate::production_semantic_terminal_v1::ProductionBf16ConversionV1::ToF32 => 3,
            }
        }
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceLen => 104,
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWrite => 105,
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteDisjoint => 106,
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteExclusive => 107,
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteBlock => 108,
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteTiled2d => 109,
        ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteRowStriped2d => 110,
        ProductionTerminalExpansionV1::KernelContextIssue => 120,
        ProductionTerminalExpansionV1::CapabilityGlobalBindReadOnly => 121,
        ProductionTerminalExpansionV1::CapabilityGlobalBindDisjointWrite => 122,
        ProductionTerminalExpansionV1::CapabilityGlobalLoad => 123,
        ProductionTerminalExpansionV1::CapabilityGlobalStore => 124,
        ProductionTerminalExpansionV1::Execution(terminal) => 125 + terminal.identity_tag(),
    }
}

const fn f32_math_tag_v1(function: fe2o3_kernel_ir::F32MathFunction) -> u8 {
    use fe2o3_kernel_ir::F32MathFunction as Function;
    match function {
        Function::Sqrt => 0,
        Function::FusedMultiplyAdd => 1,
        Function::Floor => 2,
        Function::Ceil => 3,
        Function::Truncate => 4,
        Function::RoundTiesEven => 5,
        Function::Sin => 6,
        Function::Cos => 7,
        Function::Exp => 8,
        Function::Exp2 => 9,
        Function::Ln => 10,
        Function::Log2 => 11,
        Function::Log10 => 12,
        Function::Abs => 77,
    }
}

fn build_identity_inventory_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
    collection: &CollectionResult<'tcx>,
    roots: &[AuthenticatedProductionRootV1<'tcx>],
) -> Result<ProductionSemanticIdentityInventoryV1<'tcx>, ProductionSemanticImportErrorV1> {
    require_count_within_limit_v1(
        SemanticMirResourceV1::Functions,
        collection.functions.len(),
        HARD_MAX_FUNCTIONS_V1,
    )?;
    require_count_within_limit_v1(SemanticMirResourceV1::Roots, roots.len(), HARD_MAX_ROOTS_V1)?;

    let target = canonical_target_layout_v1(target.rustc_layout());
    let mut functions = Vec::with_capacity(collection.functions.len());
    for function in &collection.functions {
        functions.push(RetainedSemanticFunctionProducerV1 {
            identities: canonical_function_identities_v1(tcx, function.instance),
            instance: function.instance,
            role: function.role,
            export_name: matches!(
                function.role,
                CollectedFunctionRole::KernelEntry | CollectedFunctionRole::DeviceFfiExport
            )
            .then(|| function.export_name.clone()),
            kernel_binding: function.kernel_binding,
            frontend_contract: function.frontend_contract.clone(),
        });
    }
    functions.sort_unstable_by_key(|entry| entry.identities.function());
    if functions
        .windows(2)
        .any(|pair| pair[0].identities.function() == pair[1].identities.function())
    {
        return Err(ProductionSemanticImportErrorV1::FunctionIdentityCollision);
    }

    let mut function_ids = BTreeMap::<SemanticFunctionIdentityV1, SemanticFunctionIdV1>::new();
    for (index, function) in functions.iter().enumerate() {
        let index =
            u32::try_from(index).map_err(|_| ProductionSemanticImportErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::Functions,
                actual: u64::MAX,
                maximum: HARD_MAX_FUNCTIONS_V1,
            })?;
        function_ids.insert(
            function.identities.function(),
            SemanticFunctionIdV1::from_index(index),
        );
    }

    let mut canonical_roots = Vec::with_capacity(roots.len());
    for root in roots {
        let identity = canonical_function_identities_v1(tcx, root.instance).function();
        let Some(function_id) = function_ids.get(&identity).copied() else {
            return Err(ProductionSemanticImportErrorV1::RootIdentityMismatch);
        };
        canonical_roots.push(function_id);
    }
    canonical_roots.sort_unstable();
    if canonical_roots.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ProductionSemanticImportErrorV1::RootIdentityMismatch);
    }

    let (sha256, canonical_transcript) =
        identity_inventory_identity_and_transcript_v1(target, &functions, &canonical_roots);
    Ok(ProductionSemanticIdentityInventoryV1 {
        functions: functions.into_boxed_slice(),
        roots: canonical_roots.into_boxed_slice(),
        sha256,
        canonical_transcript,
    })
}

fn identity_inventory_identity_and_transcript_v1(
    target: SemanticTargetDataLayoutV1,
    functions: &[RetainedSemanticFunctionProducerV1<'_>],
    roots: &[SemanticFunctionIdV1],
) -> ([u8; 32], Box<[u8]>) {
    let mut digest =
        SemanticIdentityDigestV1::new_with_canonical_transcript(IDENTITY_INVENTORY_DOMAIN_V1);
    digest.field(target.identity().as_bytes());
    for function in functions {
        digest.field(function.identities.function().as_bytes());
        digest.field(function.identities.item_definition().as_bytes());
        digest.field(function.identities.monomorphization().as_bytes());
        digest.field(function.identities.generic_type_arguments().as_bytes());
        digest.field(function.identities.const_generic_arguments().as_bytes());
        digest.field(&[function_role_tag_v1(function.role)]);
        match &function.export_name {
            Some(symbol) => {
                digest.field(&[1]);
                digest.field(symbol.as_bytes());
            }
            None => digest.field(&[0]),
        }
        match function.kernel_binding {
            Some(binding) => {
                digest.field(&[1]);
                digest.field(&binding.as_bytes());
            }
            None => digest.field(&[0]),
        }
        match &function.frontend_contract {
            Some(contract) => {
                digest.field(&[1]);
                digest.field(contract.canonical_bytes());
                if let Some(bytes) = contract.resource_canonical_bytes() {
                    digest.field(&[1]);
                    digest.field(bytes);
                }
                let reachable = contract.reachable_assembly();
                digest.field(&reachable.blocks().to_le_bytes());
                digest.field(&reachable.operand_bits().to_le_bytes());
                digest.field(&reachable.option_bits().to_le_bytes());
            }
            None => digest.field(&[0]),
        }
    }
    for root in roots {
        digest.field(&root.index().to_le_bytes());
    }
    digest.finish_with_canonical_transcript()
}

const fn function_role_tag_v1(role: CollectedFunctionRole) -> u8 {
    match role {
        CollectedFunctionRole::KernelEntry => 0,
        CollectedFunctionRole::InternalHelper => 1,
        CollectedFunctionRole::DeviceFfiExport => 2,
    }
}

fn require_count_within_limit_v1(
    resource: SemanticMirResourceV1,
    actual: usize,
    maximum: u64,
) -> Result<(), ProductionSemanticImportErrorV1> {
    let actual = u64::try_from(actual).unwrap_or(u64::MAX);
    if actual > maximum {
        Err(ProductionSemanticImportErrorV1::LimitExceeded {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn exact_ordered_axes_match<T: PartialEq>(
    expected: impl IntoIterator<Item = T>,
    observed: impl IntoIterator<Item = T>,
) -> bool {
    expected.into_iter().eq(observed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_backend_contract_produces_a_target_neutral_context_brand() {
        let contract = crate::production_backend_v1::ProductionBackendTargetContractV1::synthetic_test_contract_v1();
        let rustc_layout =
            crate::semantic_layout_bridge::SemanticLayoutTargetV1::new_with_codegen_profile(
                contract.rustc_target(),
                contract.rustc_data_layout(),
                contract.pointer_width_bits(),
                contract.cpu(),
                "",
                contract.rustc_features(),
            )
            .unwrap();
        let layout = canonical_target_layout_v1(&rustc_layout);
        let brand = kernel_context_target_brand_identity_from_contract_v1(layout, contract);
        assert_ne!(brand, [0; 32]);
        assert_eq!(
            brand,
            kernel_context_target_brand_identity_from_contract_v1(layout, contract),
        );
    }

    fn context_root(
        selected_root: u32,
        root_identity: u8,
        launch_identity: u8,
        physical_argument_count: u32,
        logical_argument_count: u32,
    ) -> AuthenticatedProductionKernelContextRootV1 {
        AuthenticatedProductionKernelContextRootV1 {
            selected_root: SemanticFunctionIdV1::from_index(selected_root),
            root_function_identity: [root_identity; 32],
            kernel_binding: [root_identity.wrapping_add(1); 32],
            kernel_marker_identity: [root_identity.wrapping_add(2); 32],
            launch_brand_identity: [launch_identity; 32],
            issuance_identity: [root_identity.wrapping_add(3); 32],
            physical_argument_count,
            logical_argument_count,
        }
    }

    fn context_custody(
        expected_roots: Vec<SemanticFunctionIdV1>,
        roots: Vec<AuthenticatedProductionKernelContextRootV1>,
        frontend: u8,
        target: u8,
    ) -> AuthenticatedProductionKernelContextsV1 {
        let frontend_unit_identity = [frontend; 32];
        let target_brand_identity = [target; 32];
        let custody_identity = kernel_context_custody_identity_v1(
            frontend_unit_identity,
            target_brand_identity,
            &expected_roots,
            &roots,
        );
        AuthenticatedProductionKernelContextsV1 {
            frontend_unit_identity,
            target_brand_identity,
            expected_roots: expected_roots.into_boxed_slice(),
            roots: roots.into_boxed_slice(),
            custody_identity,
        }
    }

    fn context_observation(
        selected_root: u32,
        root_identity: u8,
        launch_identity: u8,
    ) -> ProductionKernelContextRootObservationV1 {
        ProductionKernelContextRootObservationV1 {
            selected_root: SemanticFunctionIdV1::from_index(selected_root),
            root_function_identity: [root_identity; 32],
            kernel_binding: [root_identity.wrapping_add(1); 32],
            launch_brand_identity: [launch_identity; 32],
        }
    }

    fn context_error<T: std::fmt::Debug>(
        result: Result<T, ProductionSemanticImportErrorV1>,
    ) -> &'static str {
        match result {
            Err(ProductionSemanticImportErrorV1::KernelContextBinding(detail)) => detail,
            other => panic!("expected context custody rejection, found {other:?}"),
        }
    }

    #[test]
    fn context_carriage_rejects_missing_and_duplicate_roots() {
        let root = SemanticFunctionIdV1::from_index(0);
        let missing = context_custody(vec![root], vec![], 1, 2);
        assert_eq!(
            context_error(missing.validate_carriage(
                [1; 32],
                [2; 32],
                &[context_observation(0, 10, 20)],
            )),
            "context custody is missing, duplicated, or reordered"
        );

        let duplicate = context_custody(
            vec![root, root],
            vec![context_root(0, 10, 20, 3, 4), context_root(0, 10, 20, 3, 4)],
            1,
            2,
        );
        assert_eq!(
            context_error(duplicate.validate_carriage(
                [1; 32],
                [2; 32],
                &[context_observation(0, 10, 20)],
            )),
            "context custody contains duplicate or noncanonical roots"
        );
    }

    #[test]
    fn context_carriage_rejects_cross_root_target_and_launch_substitution() {
        let expected = vec![SemanticFunctionIdV1::from_index(0)];
        let cross_root =
            context_custody(expected.clone(), vec![context_root(0, 10, 20, 3, 4)], 1, 2);
        assert_eq!(
            context_error(cross_root.validate_carriage(
                [1; 32],
                [2; 32],
                &[context_observation(0, 11, 20)],
            )),
            "context custody was substituted across kernel roots"
        );

        let cross_target =
            context_custody(expected.clone(), vec![context_root(0, 10, 20, 3, 4)], 1, 2);
        assert_eq!(
            context_error(cross_target.validate_carriage(
                [1; 32],
                [3; 32],
                &[context_observation(0, 10, 20)],
            )),
            "context custody belongs to a different selected target"
        );

        let cross_launch = context_custody(expected, vec![context_root(0, 10, 20, 3, 4)], 1, 2);
        assert_eq!(
            context_error(cross_launch.validate_carriage(
                [1; 32],
                [2; 32],
                &[context_observation(0, 10, 21)],
            )),
            "context custody was substituted across launch contracts"
        );
    }

    #[test]
    fn context_carriage_rejects_stale_identity_and_preserves_physical_kernarg_count() {
        let expected = vec![SemanticFunctionIdV1::from_index(0)];
        let mut stale =
            context_custody(expected.clone(), vec![context_root(0, 10, 20, 3, 4)], 1, 2);
        stale.custody_identity[0] ^= 1;
        assert_eq!(
            context_error(stale.validate_carriage(
                [1; 32],
                [2; 32],
                &[context_observation(0, 10, 20)],
            )),
            "context custody identity is stale"
        );

        let exact = context_custody(expected.clone(), vec![context_root(0, 10, 20, 3, 4)], 1, 2);
        assert!(
            exact
                .validate_carriage([1; 32], [2; 32], &[context_observation(0, 10, 20)],)
                .is_ok()
        );

        let changed_physical_abi =
            context_custody(expected, vec![context_root(0, 10, 20, 3, 5)], 1, 2);
        assert_eq!(
            context_error(changed_physical_abi.validate_carriage(
                [1; 32],
                [2; 32],
                &[context_observation(0, 10, 20)],
            )),
            "logical context changed the physical kernel argument count"
        );
    }

    #[test]
    fn root_custody_comparison_rejects_every_sequence_substitution() {
        assert!(exact_ordered_axes_match([1, 2], [1, 2]));
        for substituted in [vec![1], vec![1, 2, 2], vec![2, 1], vec![1, 3]] {
            assert!(!exact_ordered_axes_match(vec![1, 2], substituted));
        }
    }

    #[test]
    fn capability_diagnostics_are_stable_and_distinguish_rejected_from_incomplete() {
        let rejected = ProductionSemanticImportErrorV1::CapabilityTerminalRejected {
            root: "root-id".to_owned(),
            span: "kernel.rs:7:9".to_owned(),
            helper_chain: "root-id->helper-id->terminal-id".to_owned(),
            stage: "terminal-authentication",
            detail: "wrong epoch".to_owned(),
        };
        assert_eq!(
            rejected.to_string(),
            "FE2O3-CAP Rejected root=root-id span=kernel.rs:7:9 helper_chain=root-id->helper-id->terminal-id stage=terminal-authentication: wrong epoch",
        );

        let incomplete = ProductionSemanticImportErrorV1::TargetNeutralLoweringPending {
            functions: 1,
            callables: 2,
            rustc_identity_inventory_sha256: [1; 32],
            rustc_preflight_plan_sha256: [2; 32],
            semantic_sha256: [3; 32],
        }
        .to_string();
        assert!(incomplete.starts_with(
            "FE2O3-CAP Incomplete root=authenticated-set span=authenticated-set helper_chain=authenticated-closure stage=target-neutral-lowering:"
        ));
    }

    #[test]
    fn capability_helper_paths_are_deterministic_bounded_and_cycle_safe() {
        let function = SemanticFunctionIdV1::from_index;
        let edges = [
            (function(0), function(2)),
            (function(0), function(1)),
            (function(1), function(3)),
            (function(2), function(3)),
            (function(3), function(0)),
        ];
        assert_eq!(
            semantic_function_path_v1(function(0), function(3), &edges),
            Some(vec![function(0), function(1), function(3)]),
        );
        assert_eq!(
            semantic_function_path_v1(function(0), function(4), &edges),
            None,
        );
    }

    #[test]
    fn typed_global_terminal_custody_rejects_orphans_and_cross_root_substitution() {
        let function = SemanticFunctionIdV1::from_index;
        let contexts = context_custody(
            vec![function(0), function(1)],
            vec![context_root(0, 10, 20, 2, 3), context_root(1, 11, 21, 2, 3)],
            1,
            2,
        );
        let helper_callers = BTreeSet::from([function(2)]);

        let exact = authenticate_capability_memory_root_v1(
            &contexts,
            &helper_callers,
            &[(function(0), function(2))],
            false,
        )
        .unwrap();
        assert_eq!(exact.selected_root, function(0));

        assert_eq!(
            context_error(authenticate_capability_memory_root_v1(
                &contexts,
                &helper_callers,
                &[],
                false,
            )),
            "typed-global terminal is not owned by exactly one authenticated kernel root"
        );
        assert_eq!(
            context_error(authenticate_capability_memory_root_v1(
                &contexts,
                &helper_callers,
                &[(function(0), function(2)), (function(1), function(2))],
                false,
            )),
            "typed-global terminal is not owned by exactly one authenticated kernel root"
        );

        let substituted_callers = BTreeSet::from([function(0), function(1)]);
        assert_eq!(
            context_error(authenticate_capability_memory_root_v1(
                &contexts,
                &substituted_callers,
                &[],
                false,
            )),
            "typed-global terminal is shared across authenticated kernel roots"
        );
    }

    #[test]
    fn typed_global_bind_requires_direct_root_issuance_and_reachability_is_cycle_safe() {
        let function = SemanticFunctionIdV1::from_index;
        let contexts =
            context_custody(vec![function(0)], vec![context_root(0, 10, 20, 2, 3)], 1, 2);
        let helper_callers = BTreeSet::from([function(2)]);
        let cyclic_edges = [
            (function(0), function(3)),
            (function(3), function(0)),
            (function(3), function(2)),
        ];
        assert!(semantic_function_reaches_v1(
            function(0),
            function(2),
            &cyclic_edges,
        ));
        assert!(!semantic_function_reaches_v1(
            function(0),
            function(4),
            &cyclic_edges,
        ));
        assert_eq!(
            context_error(authenticate_capability_memory_root_v1(
                &contexts,
                &helper_callers,
                &cyclic_edges,
                true,
            )),
            "typed-global binding is not issued directly by its physical kernel root"
        );

        let root_callers = BTreeSet::from([function(0)]);
        assert_eq!(
            authenticate_capability_memory_root_v1(&contexts, &root_callers, &cyclic_edges, true)
                .unwrap()
                .selected_root,
            function(0),
        );
    }

    #[test]
    fn count_preflight_rejects_before_semantic_record_allocation() {
        assert!(require_count_within_limit_v1(SemanticMirResourceV1::Functions, 4, 4).is_ok());
        assert!(matches!(
            require_count_within_limit_v1(SemanticMirResourceV1::Functions, 5, 4),
            Err(ProductionSemanticImportErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::Functions,
                actual: 5,
                maximum: 4,
            })
        ));
    }

    #[test]
    fn disjoint_block_component_terminal_rejects_unrepresentable_rust_signatures() {
        let mapping = (
            SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                lanes_per_block: 16,
                elements_per_lane: 4,
            },
            16,
            4,
        );
        assert_eq!(
            require_rust_disjoint_block_component_contract_v1(true, Some(mapping), true, true,)
                .unwrap(),
            mapping,
        );

        let rejected_table = |error: ProductionSemanticImportErrorV1| match error {
            ProductionSemanticImportErrorV1::BodyConstruction(error) => match *error {
                ProductionSemanticBodyErrorV1::IdentityTableMismatch { table } => table,
                other => panic!("unexpected body error: {other:?}"),
            },
            other => panic!("unexpected importer error: {other:?}"),
        };
        assert_eq!(
            rejected_table(
                require_rust_disjoint_block_component_contract_v1(
                    false,
                    Some(mapping),
                    true,
                    true,
                )
                .unwrap_err(),
            ),
            "terminal disjoint-block receiver",
        );
        assert_eq!(
            rejected_table(
                require_rust_disjoint_block_component_contract_v1(true, None, true, true)
                    .unwrap_err(),
            ),
            "terminal disjoint-block mapping",
        );
        for (component_is_usize, output_is_option_usize) in
            [(false, true), (true, false), (false, false)]
        {
            assert_eq!(
                rejected_table(
                    require_rust_disjoint_block_component_contract_v1(
                        true,
                        Some(mapping),
                        component_is_usize,
                        output_is_option_usize,
                    )
                    .unwrap_err(),
                ),
                "terminal disjoint-block component or output",
            );
        }
    }

    #[test]
    fn terminal_operation_identity_preserves_independent_histories_and_versions_combined_use() {
        use crate::production_semantic_terminal_v1::{
            ProductionBf16ConversionV1, ProductionTerminalExpansionV1,
        };

        let pipeline = [
            ProductionTerminalExpansionV1::WorkgroupPipelineCurrent,
            ProductionTerminalExpansionV1::WorkgroupPipelineStage,
            ProductionTerminalExpansionV1::WorkgroupPipelineWrite,
            ProductionTerminalExpansionV1::WorkgroupPipelineCommit,
            ProductionTerminalExpansionV1::WorkgroupPipelineWait,
            ProductionTerminalExpansionV1::WorkgroupPipelineConsume,
            ProductionTerminalExpansionV1::WorkgroupPipelineRead,
            ProductionTerminalExpansionV1::WorkgroupPipelineDiscard,
            ProductionTerminalExpansionV1::WorkgroupPipelineRelease,
        ];
        assert_eq!(
            pipeline.map(terminal_operation_tag_v1),
            [91, 92, 93, 94, 95, 96, 97, 98, 99]
        );
        let bf16 = [
            ProductionBf16ConversionV1::FromBits,
            ProductionBf16ConversionV1::ToBits,
            ProductionBf16ConversionV1::FromF32RoundTiesEven,
            ProductionBf16ConversionV1::ToF32,
        ];
        assert_eq!(
            bf16.map(|conversion| terminal_operation_tag_v1(
                ProductionTerminalExpansionV1::Bf16Conversion(conversion),
            )),
            [91, 92, 93, 94]
        );

        let combined_schema = TerminalIdentitySchemaV1::CombinedV3;
        assert_eq!(combined_schema, TerminalIdentitySchemaV1::CombinedV3);
        assert_ne!(
            PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V1,
            PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V2
        );
        assert_ne!(
            PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V2,
            PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V3
        );
        assert_ne!(
            PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V3,
            PRODUCTION_COMPILER_INTRINSIC_DOMAIN_V4
        );
        assert_eq!(
            pipeline.map(|expansion| {
                terminal_operation_tag_for_schema_v1(expansion, combined_schema)
            }),
            [91, 92, 93, 94, 95, 96, 97, 98, 99]
        );
        assert_eq!(
            [
                ProductionTerminalExpansionV1::RustcFabsF32,
                ProductionTerminalExpansionV1::MathF32(fe2o3_kernel_ir::F32MathFunction::Abs),
                ProductionTerminalExpansionV1::MemoryVolatileLoad,
            ]
            .map(|expansion| terminal_operation_tag_for_schema_v1(
                expansion,
                TerminalIdentitySchemaV1::CombinedV4,
            )),
            [113, 114, 115],
        );
        assert_eq!(
            bf16.map(|conversion| terminal_operation_tag_for_schema_v1(
                ProductionTerminalExpansionV1::Bf16Conversion(conversion),
                combined_schema,
            )),
            [100, 101, 102, 103]
        );
        assert_eq!(
            [
                ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent,
                ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum,
            ]
            .map(|expansion| terminal_operation_tag_for_schema_v1(expansion, combined_schema)),
            [111, 112, 113, 114]
        );
        assert_eq!(
            [
                ProductionTerminalExpansionV1::RustcFabsF32,
                ProductionTerminalExpansionV1::MathF32(fe2o3_kernel_ir::F32MathFunction::Abs,),
                ProductionTerminalExpansionV1::MemoryVolatileLoad,
                ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum,
                ProductionTerminalExpansionV1::WorkgroupLdsScopeCurrent,
                ProductionTerminalExpansionV1::DisjointBlockComponentIndex,
            ]
            .map(|expansion| terminal_operation_tag_for_schema_v1(
                expansion,
                TerminalIdentitySchemaV1::CombinedV4,
            )),
            [113, 114, 115, 116, 117, 118, 119],
        );
        assert_eq!(
            [
                ProductionTerminalExpansionV1::Bf16Conversion(
                    ProductionBf16ConversionV1::FromBits,
                ),
                ProductionTerminalExpansionV1::Bf16Conversion(
                    ProductionBf16ConversionV1::ToBits,
                ),
                ProductionTerminalExpansionV1::Bf16Conversion(
                    ProductionBf16ConversionV1::FromF32RoundTiesEven,
                ),
                ProductionTerminalExpansionV1::Bf16Conversion(
                    ProductionBf16ConversionV1::ToF32,
                ),
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceLen,
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWrite,
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteDisjoint,
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteExclusive,
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteBlock,
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteTiled2d,
                ProductionTerminalExpansionV1::WriteOnlyDisjointSliceWriteRowStriped2d,
                ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent,
                ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupInclusiveScanSum,
                ProductionTerminalExpansionV1::NeutralWorkgroupExclusiveScanSum,
            ]
            .map(|expansion| terminal_operation_tag_for_schema_v1(expansion, combined_schema)),
            [
                100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114,
            ]
        );
        assert_eq!(
            [
                ProductionTerminalExpansionV1::WorkgroupCollectiveContextCurrent,
                ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum,
            ]
            .map(|expansion| terminal_operation_tag_for_schema_v1(
                expansion,
                TerminalIdentitySchemaV1::CombinedV2,
            )),
            [104, 105]
        );
        assert_ne!(
            terminal_operation_tag_for_schema_v1(
                ProductionTerminalExpansionV1::NeutralWorkgroupReduceSum,
                combined_schema,
            ),
            terminal_operation_tag_for_schema_v1(
                ProductionTerminalExpansionV1::WorkgroupReduceSum,
                combined_schema,
            )
        );
    }
}
