//! Sole consuming boundary from production rustc collection to semantic MIR.

use std::collections::BTreeMap;
use std::fmt;

use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, HARD_MAX_FUNCTIONS_V1, HARD_MAX_ROOTS_V1,
    InertSemanticMirRequestV1, SemanticCallableDeclV1, SemanticCallableIdV1,
    SemanticCompilerIntrinsicIdentityV1, SemanticCompilerIntrinsicOperationV1,
    SemanticDisjointIndexSpaceV1, SemanticFunctionAbiV1, SemanticFunctionIdV1,
    SemanticFunctionIdentityV1, SemanticFunctionRoleV1, SemanticKernelBindingIdentityV1,
    SemanticKernelEntryV1, SemanticKernelLaunchBoundsV1, SemanticKernelSourceContractV1,
    SemanticLinkSymbolV1, SemanticMirErrorV1, SemanticMirLimitsV1, SemanticMirResourceV1,
    SemanticNonBodyCallableBindingV1, SemanticReachableAssemblyV1, SemanticTargetDataLayoutV1,
    SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeShapeV1, SemanticUnsafeAssemblyDeclarationV1,
    SemanticUnsafeAssemblyTargetV1, SemanticWorkgroupDimensionsV1,
};
use rustc_middle::ty::{Instance, Ty, TyCtxt, TyKind};
use rustc_span::sym;

use super::{
    AuthenticatedCollectedKernelClosureV1, AuthenticatedProductionRootV1, CollectedFunctionRole,
    CollectionResult,
};
use crate::production_semantic_body_v1::{
    ProductionSemanticBlockBindingV1, ProductionSemanticBodyErrorV1, ProductionSemanticBodyInputV1,
    ProductionSemanticBodyRequestOwnerV1, ProductionSemanticCallableOwnerEntryV1,
    ProductionSemanticDirectCallBindingV1, ProductionSemanticFunctionExportV1,
    ProductionSemanticFunctionIdentitiesV1, ProductionSemanticLocalBindingV1,
    ProductionSemanticTerminalExpansionRecipeV1, ProductionSemanticTypeBindingV1,
    construct_production_semantic_body_v1,
};
use crate::production_semantic_fn_abi_v1::{
    ConstructedSemanticFunctionAbisV1, ProductionSemanticFnAbiErrorV1,
    construct_production_semantic_fn_abis_v1,
};
use crate::production_semantic_types_v1::{
    ProductionSemanticTypeErrorV1, construct_production_semantic_types_v1,
};
use crate::production_target_v1::ProductionTargetErrorV1;
use crate::rustc_semantic_adapter_v1::{
    SemanticIdentityDigestV1, canonical_function_identities_v1, canonical_target_layout_v1,
};
use crate::rustc_semantic_plan_v1::{
    ProductionSemanticPreflightErrorV1, ProductionSemanticPreflightPlanV1,
    RetainedSemanticFunctionProducerV1, build_production_semantic_preflight_plan_v1,
};
use crate::trusted_device_items::{self, TrustedDeviceItem};

const IDENTITY_INVENTORY_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-identity-inventory/v1";

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
                "semantic importer authenticated rustc identity inventory {} and bounded preflight plan {}, then admitted one complete semantic MIR request with {functions} function(s), {callables} callable(s), and canonical identity {}; an owner-held Pliron locator graph was recursively verified for exact semantic equivalence; target-neutral lowering remains pending; no fallback or artifact emission was entered",
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
            | Self::TargetNeutralLoweringPending { .. } => None,
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

pub(crate) fn construct_production_semantic_mir_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
) -> Result<
    (
        AdmittedInertSemanticMirV1,
        AuthenticatedRustcIdentityInventoryV3,
        AuthenticatedRustcPreflightPlanV3,
        crate::production_target_v1::AuthenticatedProductionTargetV1,
    ),
    ProductionSemanticImportErrorV1,
> {
    let AuthenticatedCollectedKernelClosureV1 {
        target,
        collection,
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
    let plan = match build_production_semantic_preflight_plan_v1(
        tcx,
        canonical_target_layout_v1(target.rustc_layout()),
        functions,
        roots,
        rustc_identity_inventory_sha256,
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
        semantic_types.into_records(),
        semantic_function_abis,
        semantic_terminal_abis,
    )?;
    let (rustc_preflight_plan_sha256, rustc_preflight_plan_transcript) =
        plan.into_identity_and_canonical_transcript();
    drop(collection);
    Ok((
        semantic_mir,
        AuthenticatedRustcIdentityInventoryV3 {
            sha256: rustc_identity_inventory_sha256,
            canonical_transcript: rustc_identity_inventory_transcript,
        },
        AuthenticatedRustcPreflightPlanV3 {
            sha256: rustc_preflight_plan_sha256,
            rustc_identity_inventory_sha256,
            canonical_transcript: rustc_preflight_plan_transcript,
        },
        target,
    ))
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

fn construct_complete_request_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    target: SemanticTargetDataLayoutV1,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
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
        let operation =
            terminal_operation_v1(tcx, terminal.instance, terminal.expansion, abi, &types)?;
        let mut digest =
            SemanticIdentityDigestV1::new(b"fe2o3/semantic-mir/production-compiler-intrinsic/v1");
        digest.field(terminal.identities.function().as_bytes());
        digest.field(abi.identity().as_bytes());
        digest.field(&[terminal_operation_tag_v1(terminal.expansion)]);
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
                },
                &mut body_owner,
            )
            .map_err(|error| ProductionSemanticImportErrorV1::BodyConstruction(Box::new(error)))?,
        );
    }

    InertSemanticMirRequestV1::new_with_callables(
        target,
        types,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        functions,
        callables,
        plan.roots().to_vec(),
    )
    .and_then(|request| request.admit(SemanticMirLimitsV1::default()))
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)
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
    SemanticKernelSourceContractV1::new(launch, unsafe_assembly, reachable_assembly)
        .map_err(ProductionSemanticImportErrorV1::SemanticSchema)
}

fn terminal_operation_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    expansion: crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
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
        ProductionTerminalExpansionV1::WorkgroupBarrier
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && matches!(rust_output.kind(), TyKind::Tuple(fields) if fields.is_empty()) =>
        {
            Ok(SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier)
        }
        ProductionTerminalExpansionV1::ThreadIndex1d
            if inputs.is_empty()
                && rust_inputs.is_empty()
                && rust_index_witness_space_v1(
                    tcx,
                    rust_output,
                    TrustedDeviceItem::ThreadIndex,
                ) == Some(SemanticDisjointIndexSpaceV1::Index1d) =>
        {
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
            let input_space =
                rust_index_witness_space_v1(tcx, rust_inputs[0], TrustedDeviceItem::ThreadIndex);
            let output_space =
                rust_index_witness_space_v1(tcx, rust_output, TrustedDeviceItem::DisjointIndex);
            if input_space.is_none() || input_space != output_space {
                return Err(body_owner_table_mismatch_v1("terminal disjoint mapping"));
            }
            let raw_index = aggregate_field_v1(types, inputs[0], 0)?;
            Ok(
                SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
                    input_witness: inputs[0],
                    output_witness: output,
                    raw_index,
                    index_space: input_space.expect("checked mapping"),
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
            let input_space =
                rust_index_witness_space_v1(tcx, rust_inputs[0], TrustedDeviceItem::ThreadIndex)
                    .ok_or_else(|| body_owner_table_mismatch_v1("terminal checked-block input"))?;
            let rust_output_block = rust_option_payload_v1(tcx, rust_output)
                .ok_or_else(|| body_owner_table_mismatch_v1("terminal checked-block result"))?;
            let (output_space, lanes_per_block, elements_per_lane) =
                rust_disjoint_block_v1(tcx, rust_output_block).ok_or_else(|| {
                    body_owner_table_mismatch_v1("terminal checked-block witness")
                })?;
            if input_space != SemanticDisjointIndexSpaceV1::Index1d {
                return Err(body_owner_table_mismatch_v1(
                    "terminal checked-block input mapping",
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
        ProductionTerminalExpansionV1::ThreadIndex1d
        | ProductionTerminalExpansionV1::ThreadIndexGet
        | ProductionTerminalExpansionV1::ThreadIndexIntoDisjoint
        | ProductionTerminalExpansionV1::ThreadIndexCheckedShift
        | ProductionTerminalExpansionV1::DisjointIndexGet
        | ProductionTerminalExpansionV1::DisjointIndexCheckedShift
        | ProductionTerminalExpansionV1::DisjointSliceGetMut
        | ProductionTerminalExpansionV1::DisjointSliceGetDisjointMut
        | ProductionTerminalExpansionV1::GridLeaderCurrent
        | ProductionTerminalExpansionV1::DisjointSliceGetMutExclusive
        | ProductionTerminalExpansionV1::ThreadIndexCheckedBlock
        | ProductionTerminalExpansionV1::DisjointSliceGetBlockMut
        | ProductionTerminalExpansionV1::WorkgroupBarrier => {
            Err(body_owner_table_mismatch_v1("terminal callable ABI"))
        }
    }
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
    let input_space = rust_index_witness_space_v1(tcx, rust_input, input_kind)
        .ok_or_else(|| body_owner_table_mismatch_v1("terminal checked-shift input"))?;
    let rust_output_witness = rust_option_payload_v1(tcx, rust_output)
        .ok_or_else(|| body_owner_table_mismatch_v1("terminal checked-shift result"))?;
    let output_space =
        rust_index_witness_space_v1(tcx, rust_output_witness, TrustedDeviceItem::DisjointIndex)
            .ok_or_else(|| body_owner_table_mismatch_v1("terminal checked-shift output"))?;
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

fn rust_option_payload_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    (tcx.is_diagnostic_item(sym::Option, definition.did()) && arguments.len() == 1)
        .then(|| arguments[0].as_type())
        .flatten()
}

fn rust_is_trusted_adt_v1(tcx: TyCtxt<'_>, ty: Ty<'_>, item: TrustedDeviceItem) -> bool {
    matches!(*ty.kind(), TyKind::Adt(definition, arguments)
        if arguments.is_empty() && trusted_device_items::classify(tcx, definition.did()) == Some(item))
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

fn rust_index_witness_space_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    item: TrustedDeviceItem,
) -> Option<SemanticDisjointIndexSpaceV1> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did()) != Some(item) || arguments.len() != 1 {
        return None;
    }
    rust_disjoint_index_space_v1(tcx, arguments[0].as_type()?)
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
        _ => None,
    }
}

fn rust_disjoint_block_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<(SemanticDisjointIndexSpaceV1, u64, u64)> {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return None;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::DisjointBlock)
        || arguments.len() != 3
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
        && arguments.len() == 1)
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

const fn terminal_operation_tag_v1(
    expansion: crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1,
) -> u8 {
    use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
    match expansion {
        ProductionTerminalExpansionV1::ThreadIndex1d => 0,
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
    fn terminal_diagnostic_is_bounded_and_workload_neutral() {
        let error = ProductionSemanticImportErrorV1::TargetNeutralLoweringPending {
            functions: 3,
            callables: 6,
            rustc_identity_inventory_sha256: [0xab; 32],
            rustc_preflight_plan_sha256: [0xcd; 32],
            semantic_sha256: [0xef; 32],
        };
        let diagnostic = error.to_string();
        assert!(diagnostic.contains("3 function(s)"));
        assert!(diagnostic.contains("6 callable(s)"));
        assert!(diagnostic.contains(&"ab".repeat(32)));
        assert!(diagnostic.contains(&"cd".repeat(32)));
        assert!(diagnostic.contains(&"ef".repeat(32)));
        assert!(diagnostic.contains("admitted one complete semantic MIR request"));
        assert!(diagnostic.contains("recursively verified for exact semantic equivalence"));
        assert!(diagnostic.contains("target-neutral lowering remains pending"));
        for forbidden in [
            "GEMM",
            "attention",
            "softmax",
            "export name",
            concat!("MIR ", "transcript"),
        ] {
            assert!(!diagnostic.contains(forbidden));
        }
    }

    #[test]
    fn root_custody_comparison_rejects_every_sequence_substitution() {
        assert!(exact_ordered_axes_match([1, 2], [1, 2]));
        for substituted in [vec![1], vec![1, 2, 2], vec![2, 1], vec![1, 3]] {
            assert!(!exact_ordered_axes_match(vec![1, 2], substituted));
        }
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
}
