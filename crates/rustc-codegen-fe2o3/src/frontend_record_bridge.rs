//! Extraction of bounded frontend records from the active rustc session.
//!
//! This bridge is an observational preflight gate. Its output does not grant
//! code-generation, artifact, proof, or launch authority.

use crate::collector::{
    AuthenticatedKernelFrontendContractV1, CollectedFunction, CollectionResult,
    ReachableAssemblySummaryV1,
};
use fe2o3_artifacts::DigestAlgorithm;
use fe2o3_rustc_front::{
    BasicBlockV1, BlockIdV1, DecodeError, FrontendUnitV1, FunctionIdentityV1, FunctionRoleV1,
    MAX_BLOCKS_PER_FUNCTION_V1, MAX_FUNCTIONS_V1, MAX_PARAMETERS_PER_FUNCTION_V1,
    MAX_SUCCESSORS_PER_BLOCK_V1, MAX_TOTAL_BLOCKS_V1, MonomorphizedFunctionV1,
    SourceFileIdentityV1, SourceLocationV1, StableTypeIdentityV1, TypedSignatureV1,
    ValidationError, decode_frontend_unit_v1, encode_frontend_unit_v1,
};
use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_middle::mir::Body;
use rustc_middle::ty::print::with_no_trimmed_paths;
use rustc_middle::ty::{InstanceKind, Ty, TyCtxt, TyKind, TypeVisitableExt, TypingEnv};
use rustc_span::Span;
use std::collections::BTreeSet;
use std::fmt;

const MAX_IDENTITY_PREIMAGE_BYTES: usize = 256 * 1024;
const FUNCTION_IDENTITY_DOMAIN: &[u8] = b"fe2o3-rustc-front/function-identity/v1";
const TYPE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-rustc-front/type-identity/v1";
const SOURCE_FILE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-rustc-front/source-file-identity/v1";

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FrontendRecordBridgeError {
    BoundExceeded {
        field: &'static str,
        actual: usize,
        max: usize,
    },
    MissingMir {
        function: String,
    },
    MissingTerminator {
        function: String,
        block: usize,
    },
    UnsupportedInstance {
        function: String,
        detail: String,
    },
    NotFullyMonomorphized {
        function: String,
        rust_type: String,
    },
    Normalization {
        function: String,
        rust_type: String,
        detail: String,
    },
    InvalidSpan {
        function: String,
    },
    IntegerOverflow {
        field: &'static str,
    },
    IdentityPreimageTooLarge,
    Validation(ValidationError),
    Decode(DecodeError),
    NonCanonicalRoundTrip,
    SameSessionRustcCustody,
}

impl fmt::Display for FrontendRecordBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundExceeded { field, actual, max } => {
                write!(formatter, "{field} bound exceeded: {actual} > {max}")
            }
            Self::MissingMir { function } => {
                write!(formatter, "collected function `{function}` has no MIR")
            }
            Self::MissingTerminator { function, block } => {
                write!(
                    formatter,
                    "MIR block {block} in `{function}` has no terminator"
                )
            }
            Self::UnsupportedInstance { function, detail } => {
                write!(formatter, "unsupported instance `{function}`: {detail}")
            }
            Self::NotFullyMonomorphized {
                function,
                rust_type,
            } => write!(
                formatter,
                "function `{function}` contains non-monomorphized type `{rust_type}`"
            ),
            Self::Normalization {
                function,
                rust_type,
                detail,
            } => write!(
                formatter,
                "failed to normalize type `{rust_type}` in `{function}`: {detail}"
            ),
            Self::InvalidSpan { function } => {
                write!(formatter, "function `{function}` has no usable source span")
            }
            Self::IntegerOverflow { field } => {
                write!(formatter, "{field} does not fit its record field")
            }
            Self::IdentityPreimageTooLarge => write!(
                formatter,
                "frontend identity preimage exceeds {MAX_IDENTITY_PREIMAGE_BYTES} bytes"
            ),
            Self::Validation(error) => write!(formatter, "invalid frontend record: {error}"),
            Self::Decode(error) => write!(
                formatter,
                "frontend record decoder rejected compiler output: {error}"
            ),
            Self::NonCanonicalRoundTrip => {
                formatter.write_str("frontend record changed across canonical decode")
            }
            Self::SameSessionRustcCustody => formatter
                .write_str("same-session ordinary-Rust custody rejected the typed rustc closure"),
        }
    }
}

impl std::error::Error for FrontendRecordBridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ValidationError> for FrontendRecordBridgeError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}

impl From<DecodeError> for FrontendRecordBridgeError {
    fn from(value: DecodeError) -> Self {
        Self::Decode(value)
    }
}

pub(crate) struct CompilerFrontendRecordV1 {
    unit: FrontendUnitV1,
    canonical_bytes: Vec<u8>,
    kernel_contracts: Vec<CompilerKernelFrontendContractRecordV1>,
    ordinary_rust_scalar: Option<crate::same_session_rustc_v1::SameSessionRustKernelOutcomeV1>,
}

impl fmt::Debug for CompilerFrontendRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerFrontendRecordV1")
            .field("unit", &self.unit)
            .field("canonical_bytes", &self.canonical_bytes.len())
            .field("kernel_contracts", &self.kernel_contracts.len())
            .field("ordinary_rust_scalar", &self.ordinary_rust_scalar.is_some())
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CompilerKernelFrontendContractRecordV1 {
    function_identity: FunctionIdentityV1,
    registration_path: String,
    target_def_path_hash: [u8; 16],
    target_symbol: String,
    canonical_bytes: Vec<u8>,
    contract: fe2o3_rustc_front::KernelFrontendContractV1,
    reachable_assembly: ReachableAssemblySummaryV1,
}

impl CompilerFrontendRecordV1 {
    pub(crate) const fn unit(&self) -> &FrontendUnitV1 {
        &self.unit
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub(crate) fn kernel_contracts(&self) -> &[CompilerKernelFrontendContractRecordV1] {
        &self.kernel_contracts
    }

    pub(crate) fn ordinary_rust_scalar(
        &self,
    ) -> Option<&crate::same_session_rustc_v1::OwnerControlledRustKernelImportV1> {
        match self.ordinary_rust_scalar.as_ref()? {
            crate::same_session_rustc_v1::SameSessionRustKernelOutcomeV1::Supported(imported) => {
                Some(imported)
            }
            crate::same_session_rustc_v1::SameSessionRustKernelOutcomeV1::Unsupported(_) => None,
        }
    }

    pub(crate) fn ordinary_rust_rejection(
        &self,
    ) -> Option<&crate::same_session_rustc_v1::UnsupportedRustMirDiagnosticV1> {
        match self.ordinary_rust_scalar.as_ref()? {
            crate::same_session_rustc_v1::SameSessionRustKernelOutcomeV1::Supported(_) => None,
            crate::same_session_rustc_v1::SameSessionRustKernelOutcomeV1::Unsupported(
                diagnostic,
            ) => Some(diagnostic),
        }
    }
}

impl CompilerKernelFrontendContractRecordV1 {
    fn from_authenticated(
        function_identity: FunctionIdentityV1,
        authenticated: &AuthenticatedKernelFrontendContractV1,
    ) -> Self {
        Self {
            function_identity,
            registration_path: authenticated.registration_path().to_owned(),
            target_def_path_hash: authenticated.target_def_path_hash(),
            target_symbol: authenticated.target_symbol().to_owned(),
            canonical_bytes: authenticated.canonical_bytes().to_vec(),
            contract: authenticated.contract(),
            reachable_assembly: authenticated.reachable_assembly(),
        }
    }

    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub(crate) const fn contract(&self) -> fe2o3_rustc_front::KernelFrontendContractV1 {
        self.contract
    }

    pub(crate) const fn reachable_assembly(&self) -> ReachableAssemblySummaryV1 {
        self.reachable_assembly
    }
}

pub(crate) fn extract_frontend_record_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
) -> Result<CompilerFrontendRecordV1, FrontendRecordBridgeError> {
    with_no_trimmed_paths!(extract_frontend_record_untrimmed_v1(tcx, collection))
}

fn extract_frontend_record_untrimmed_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
) -> Result<CompilerFrontendRecordV1, FrontendRecordBridgeError> {
    check_bound(
        "collected functions",
        collection.functions.len(),
        MAX_FUNCTIONS_V1,
    )?;

    let mut total_blocks = 0_usize;
    let mut functions = Vec::with_capacity(collection.functions.len());
    let mut kernel_contracts = Vec::new();
    for function in &collection.functions {
        let name = function_name(tcx, function);
        if !matches!(function.instance.def, InstanceKind::Item(_)) {
            return Err(FrontendRecordBridgeError::UnsupportedInstance {
                function: name,
                detail: format!(
                    "expected an ordinary item, found {:?}",
                    function.instance.def
                ),
            });
        }
        if !tcx.is_mir_available(function.instance.def_id()) {
            return Err(FrontendRecordBridgeError::MissingMir { function: name });
        }
        let body = tcx.instance_mir(function.instance.def);
        check_bound(
            "function CFG blocks",
            body.basic_blocks.len(),
            MAX_BLOCKS_PER_FUNCTION_V1,
        )?;
        total_blocks = total_blocks.checked_add(body.basic_blocks.len()).ok_or(
            FrontendRecordBridgeError::IntegerOverflow {
                field: "total CFG block count",
            },
        )?;
        check_bound("frontend CFG blocks", total_blocks, MAX_TOTAL_BLOCKS_V1)?;
        let extracted = extract_function(tcx, function, body)?;
        if let Some(contract) = &function.frontend_contract {
            kernel_contracts.push(CompilerKernelFrontendContractRecordV1::from_authenticated(
                extracted.identity(),
                contract,
            ));
        }
        functions.push(extracted);
    }

    kernel_contracts.sort_by(|lhs, rhs| {
        lhs.target_symbol
            .cmp(&rhs.target_symbol)
            .then_with(|| lhs.registration_path.cmp(&rhs.registration_path))
            .then_with(|| lhs.target_def_path_hash.cmp(&rhs.target_def_path_hash))
    });

    let extracted = FrontendUnitV1::new(functions)?;
    let canonical_bytes = encode_frontend_unit_v1(&extracted)?;
    let decoded = decode_frontend_unit_v1(&canonical_bytes)?;
    if decoded != extracted {
        return Err(FrontendRecordBridgeError::NonCanonicalRoundTrip);
    }
    let ordinary_rust_scalar = if is_ordinary_scalar_candidate(collection) {
        let outcome = crate::same_session_rustc_v1::import_ordinary_rust_kernel_same_session_v1(
            tcx, collection,
        )
        .map_err(|_| FrontendRecordBridgeError::SameSessionRustcCustody)?;
        match &outcome {
            crate::same_session_rustc_v1::SameSessionRustKernelOutcomeV1::Supported(imported) => {
                debug_assert_eq!(
                    imported.function_count(),
                    imported.imported().functions().len()
                );
                debug_assert_eq!(
                    imported.mir_closure(),
                    imported.imported().mir_closure_identity()
                );
                debug_assert_ne!(imported.abi_closure(), &[0; 32]);
                debug_assert_ne!(imported.custody_binding(), &[0; 32]);
                debug_assert!(imported.retained_graph_is_live());
                debug_assert!(imported.source_identity_join_is_exact());
                debug_assert!(!imported.grants_compiler_authority());
            }
            crate::same_session_rustc_v1::SameSessionRustKernelOutcomeV1::Unsupported(
                diagnostic,
            ) => {
                debug_assert!(diagnostic.is_terminal());
                debug_assert_eq!(
                    diagnostic.code(),
                    crate::same_session_rustc_v1::UnsupportedRustMirDiagnosticCodeV1::UnsupportedSemanticOperation
                );
                debug_assert_ne!(
                    diagnostic.kind(),
                    dialect_mir::pliron::MirSemanticOperationKind::TerminatorReturn
                );
                debug_assert_ne!(diagnostic.provenance().call_site().file_identity(), [0; 4]);
                debug_assert_ne!(diagnostic.custody_binding(), &[0; 32]);
            }
        }
        Some(outcome)
    } else {
        None
    };
    Ok(CompilerFrontendRecordV1 {
        unit: decoded,
        canonical_bytes,
        kernel_contracts,
        ordinary_rust_scalar,
    })
}

fn is_ordinary_scalar_candidate(collection: &CollectionResult<'_>) -> bool {
    let mut roots = collection
        .functions
        .iter()
        .filter(|function| function.is_kernel_entry());
    let Some(root) = roots.next() else {
        return false;
    };
    if roots.next().is_some() {
        return false;
    }
    let Some(contract) = root
        .frontend_contract
        .as_ref()
        .map(|record| record.contract())
    else {
        return false;
    };
    let Some(launch) = contract.launch() else {
        return false;
    };
    contract.unsafe_assembly().is_none()
        && launch.required().map(|value| value.as_array()) == Some([1, 1, 1])
        && launch.maximum().map(|value| value.as_array()) == Some([1, 1, 1])
        && launch.min_workgroups_per_compute_unit().is_none()
}

fn extract_function<'tcx>(
    tcx: TyCtxt<'tcx>,
    function: &CollectedFunction<'tcx>,
    body: &Body<'tcx>,
) -> Result<MonomorphizedFunctionV1, FrontendRecordBridgeError> {
    let function_name = function_name(tcx, function);
    require_monomorphized_args(function, &function_name)?;

    let symbol = tcx.symbol_name(function.instance).name.to_string();
    let identity = FunctionIdentityV1::new(hash_identity(
        FUNCTION_IDENTITY_DOMAIN,
        &[symbol.as_bytes()],
    )?)?;
    let fallback_span = tcx.def_span(function.instance.def_id());
    let location = source_location(tcx, &function_name, fallback_span, None)?;
    let signature = extract_signature(tcx, function, &function_name)?;
    let mut blocks = Vec::with_capacity(body.basic_blocks.len());

    for (block_id, block) in body.basic_blocks.iter_enumerated() {
        let block_index = block_id.as_usize();
        let terminator = block.terminator.as_ref().ok_or_else(|| {
            FrontendRecordBridgeError::MissingTerminator {
                function: function_name.clone(),
                block: block_index,
            }
        })?;
        let primary_span = block
            .statements
            .first()
            .map(|statement| statement.source_info.span)
            .unwrap_or(terminator.source_info.span);
        let block_location =
            source_location(tcx, &function_name, primary_span, Some(fallback_span))?;
        let successors = collect_bounded_cfg_successors(
            terminator
                .successors()
                .map(|successor| successor.as_usize()),
        )?;
        let successors = successors
            .into_iter()
            .map(|successor| {
                u32::try_from(successor).map(BlockIdV1::new).map_err(|_| {
                    FrontendRecordBridgeError::IntegerOverflow {
                        field: "CFG successor block identity",
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let block_id =
            u32::try_from(block_index).map_err(|_| FrontendRecordBridgeError::IntegerOverflow {
                field: "CFG block identity",
            })?;
        blocks.push(BasicBlockV1::new(
            BlockIdV1::new(block_id),
            block_location,
            successors,
        )?);
    }

    MonomorphizedFunctionV1::new(
        identity,
        if function.is_kernel_entry() {
            FunctionRoleV1::Kernel
        } else {
            FunctionRoleV1::Helper
        },
        function_name,
        location,
        signature,
        BlockIdV1::new(0),
        blocks,
    )
    .map_err(Into::into)
}

fn collect_bounded_cfg_successors(
    successors: impl IntoIterator<Item = usize>,
) -> Result<BTreeSet<usize>, FrontendRecordBridgeError> {
    let mut successor_work = 0_usize;
    let mut collected = BTreeSet::new();
    for successor in successors {
        successor_work =
            successor_work
                .checked_add(1)
                .ok_or(FrontendRecordBridgeError::IntegerOverflow {
                    field: "CFG successor work",
                })?;
        check_bound(
            "CFG successor work",
            successor_work,
            MAX_SUCCESSORS_PER_BLOCK_V1,
        )?;
        if !collected.contains(&successor) && collected.len() >= MAX_SUCCESSORS_PER_BLOCK_V1 {
            return Err(FrontendRecordBridgeError::BoundExceeded {
                field: "CFG block successors",
                actual: collected.len().saturating_add(1),
                max: MAX_SUCCESSORS_PER_BLOCK_V1,
            });
        }
        collected.insert(successor);
    }
    Ok(collected)
}

fn extract_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    function: &CollectedFunction<'tcx>,
    function_name: &str,
) -> Result<TypedSignatureV1, FrontendRecordBridgeError> {
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(function.instance.def_id())
            .instantiate(tcx, function.instance.args),
    );
    check_bound(
        "function parameters",
        signature.inputs().len(),
        MAX_PARAMETERS_PER_FUNCTION_V1,
    )?;
    let parameters = signature
        .inputs()
        .iter()
        .map(|ty| stable_type_identity(tcx, function_name, *ty))
        .collect::<Result<Vec<_>, _>>()?;
    let return_type = stable_type_identity(tcx, function_name, signature.output())?;
    TypedSignatureV1::new(parameters, return_type).map_err(Into::into)
}

fn stable_type_identity<'tcx>(
    tcx: TyCtxt<'tcx>,
    function_name: &str,
    ty: Ty<'tcx>,
) -> Result<StableTypeIdentityV1, FrontendRecordBridgeError> {
    require_monomorphized_type(function_name, ty)?;
    let normalized = tcx
        .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), ty)
        .map_err(|error| FrontendRecordBridgeError::Normalization {
            function: function_name.to_owned(),
            rust_type: ty.to_string(),
            detail: format!("{error:?}"),
        })?;
    require_monomorphized_type(function_name, normalized)?;
    if normalized.has_aliases() || matches!(normalized.kind(), TyKind::Error(_)) {
        return Err(FrontendRecordBridgeError::NotFullyMonomorphized {
            function: function_name.to_owned(),
            rust_type: normalized.to_string(),
        });
    }
    StableTypeIdentityV1::new(hash_identity(
        TYPE_IDENTITY_DOMAIN,
        &[&stable_fingerprint!(tcx, normalized)],
    )?)
    .map_err(Into::into)
}

fn require_monomorphized_args(
    function: &CollectedFunction<'_>,
    function_name: &str,
) -> Result<(), FrontendRecordBridgeError> {
    if function.instance.args.has_non_region_param()
        || function.instance.args.has_infer()
        || function.instance.args.has_escaping_bound_vars()
        || function.instance.args.has_placeholders()
    {
        return Err(FrontendRecordBridgeError::NotFullyMonomorphized {
            function: function_name.to_owned(),
            rust_type: format!("{:?}", function.instance.args),
        });
    }
    Ok(())
}

fn require_monomorphized_type(
    function_name: &str,
    ty: Ty<'_>,
) -> Result<(), FrontendRecordBridgeError> {
    if ty.has_non_region_param()
        || ty.has_infer()
        || ty.has_escaping_bound_vars()
        || ty.has_placeholders()
    {
        return Err(FrontendRecordBridgeError::NotFullyMonomorphized {
            function: function_name.to_owned(),
            rust_type: ty.to_string(),
        });
    }
    Ok(())
}

fn source_location(
    tcx: TyCtxt<'_>,
    function_name: &str,
    primary: Span,
    fallback: Option<Span>,
) -> Result<SourceLocationV1, FrontendRecordBridgeError> {
    let span = if !primary.is_dummy() {
        primary
    } else if let Some(fallback) = fallback.filter(|span| !span.is_dummy()) {
        fallback
    } else {
        return Err(FrontendRecordBridgeError::InvalidSpan {
            function: function_name.to_owned(),
        });
    };
    let location = tcx.sess.source_map().lookup_char_pos(span.lo());
    let source_name = location
        .file
        .name
        .prefer_remapped_unconditionally()
        .to_string_lossy()
        .into_owned();
    let file = SourceFileIdentityV1::new(hash_identity(
        SOURCE_FILE_IDENTITY_DOMAIN,
        &[source_name.as_bytes()],
    )?)?;
    let line =
        u32::try_from(location.line).map_err(|_| FrontendRecordBridgeError::IntegerOverflow {
            field: "source line",
        })?;
    let column =
        location
            .col
            .0
            .checked_add(1)
            .ok_or(FrontendRecordBridgeError::IntegerOverflow {
                field: "source column",
            })?;
    let column = u32::try_from(column).map_err(|_| FrontendRecordBridgeError::IntegerOverflow {
        field: "source column",
    })?;
    SourceLocationV1::new(file, line, column).map_err(Into::into)
}

fn function_name(tcx: TyCtxt<'_>, function: &CollectedFunction<'_>) -> String {
    tcx.def_path_str(function.instance.def_id())
}

fn hash_identity(domain: &[u8], fields: &[&[u8]]) -> Result<[u8; 32], FrontendRecordBridgeError> {
    let mut preimage = Vec::with_capacity(domain.len() + 32);
    append_identity_field(&mut preimage, domain)?;
    for field in fields {
        append_identity_field(&mut preimage, field)?;
    }
    Ok(*DigestAlgorithm::Sha256
        .calculate(&preimage)
        .bytes()
        .as_bytes())
}

fn append_identity_field(
    preimage: &mut Vec<u8>,
    field: &[u8],
) -> Result<(), FrontendRecordBridgeError> {
    let length =
        u64::try_from(field.len()).map_err(|_| FrontendRecordBridgeError::IntegerOverflow {
            field: "identity field length",
        })?;
    let next_len = preimage
        .len()
        .checked_add(8)
        .and_then(|length| length.checked_add(field.len()))
        .ok_or(FrontendRecordBridgeError::IdentityPreimageTooLarge)?;
    if next_len > MAX_IDENTITY_PREIMAGE_BYTES {
        return Err(FrontendRecordBridgeError::IdentityPreimageTooLarge);
    }
    preimage.extend_from_slice(&length.to_le_bytes());
    preimage.extend_from_slice(field);
    Ok(())
}

fn check_bound(
    field: &'static str,
    actual: usize,
    max: usize,
) -> Result<(), FrontendRecordBridgeError> {
    if actual > max {
        Err(FrontendRecordBridgeError::BoundExceeded { field, actual, max })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::CollectedFunction;
    use fe2o3_rustc_front::{
        FrontendLaunchBoundsV1, FrontendWorkgroupDimensionsV1, KernelFrontendContractV1,
    };
    use rustc_driver::{Callbacks, Compilation};
    use rustc_hir::def::DefKind;
    use rustc_hir::def_id::LocalDefId;
    use rustc_interface::interface::Compiler;
    use rustc_middle::ty::Instance;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    const FIXTURE_SOURCE: &str = r#"
#![allow(dead_code)]

fn helper(value: u32) -> u32 {
    value
}

fn kernel(value: u32) -> u32 {
    if value > 4 { helper(value) } else { value }
}
"#;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn successor_work_bound_counts_duplicates_before_insertion() {
        let actual = MAX_SUCCESSORS_PER_BLOCK_V1 + 1;
        let error = collect_bounded_cfg_successors(std::iter::repeat_n(7, actual)).unwrap_err();
        assert_eq!(
            error,
            FrontendRecordBridgeError::BoundExceeded {
                field: "CFG successor work",
                actual,
                max: MAX_SUCCESSORS_PER_BLOCK_V1,
            }
        );
    }

    #[derive(Debug)]
    struct DriverResults {
        canonical_equal: bool,
        unit_equal: bool,
        function_count: usize,
        scalar_outcome_present: bool,
        scalar_identity_equal: bool,
        has_kernel: bool,
        cfg_and_locations_valid: bool,
        helper_only: FrontendRecordBridgeError,
        duplicate: FrontendRecordBridgeError,
    }

    #[derive(Default)]
    struct BridgeCallbacks {
        results: Option<DriverResults>,
    }

    impl Callbacks for BridgeCallbacks {
        fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            let kernel = collected(tcx, "kernel", true);
            let helper = collected(tcx, "helper", false);
            let first_collection = CollectionResult {
                functions: vec![kernel.clone(), helper.clone()],
                ..CollectionResult::default()
            };
            let reordered_collection = CollectionResult {
                functions: vec![helper.clone(), kernel.clone()],
                ..CollectionResult::default()
            };
            let helper_only = extract_frontend_record_v1(
                tcx,
                &CollectionResult {
                    functions: vec![helper.clone()],
                    ..CollectionResult::default()
                },
            )
            .unwrap_err();
            let duplicate = extract_frontend_record_v1(
                tcx,
                &CollectionResult {
                    functions: vec![kernel.clone(), kernel],
                    ..CollectionResult::default()
                },
            )
            .unwrap_err();
            let first = extract_frontend_record_v1(tcx, &first_collection).unwrap();
            let reordered = extract_frontend_record_v1(tcx, &reordered_collection).unwrap();
            let cfg_and_locations_valid = first.unit.functions().iter().all(|function| {
                function.location().line() > 0
                    && function.signature().parameters().len() == 1
                    && !function.blocks().is_empty()
                    && function
                        .blocks()
                        .iter()
                        .enumerate()
                        .all(|(expected, block)| {
                            block.id().get() == u32::try_from(expected).unwrap()
                                && block.location().line() > 0
                                && block.successors().iter().all(|successor| {
                                    successor.get() < function.blocks().len() as u32
                                })
                        })
            });
            self.results = Some(DriverResults {
                canonical_equal: first.canonical_bytes == reordered.canonical_bytes,
                unit_equal: first.unit == reordered.unit,
                function_count: first.unit.functions().len(),
                scalar_outcome_present: first.ordinary_rust_scalar.is_some(),
                scalar_identity_equal: match (
                    &first.ordinary_rust_scalar,
                    &reordered.ordinary_rust_scalar,
                ) {
                    (
                        Some(crate::same_session_rustc_v1::SameSessionRustKernelOutcomeV1::Supported(first)),
                        Some(crate::same_session_rustc_v1::SameSessionRustKernelOutcomeV1::Supported(reordered)),
                    ) => first.imported().import_identity() == reordered.imported().import_identity(),
                    (
                        Some(crate::same_session_rustc_v1::SameSessionRustKernelOutcomeV1::Unsupported(first)),
                        Some(crate::same_session_rustc_v1::SameSessionRustKernelOutcomeV1::Unsupported(reordered)),
                    ) => first.mir_import_identity() == reordered.mir_import_identity(),
                    _ => false,
                },
                has_kernel: first
                    .unit
                    .functions()
                    .iter()
                    .any(|function| function.role() == FunctionRoleV1::Kernel),
                cfg_and_locations_valid,
                helper_only,
                duplicate,
            });
            Compilation::Stop
        }
    }

    fn collected<'tcx>(tcx: TyCtxt<'tcx>, name: &str, is_kernel: bool) -> CollectedFunction<'tcx> {
        let definition = local_function(tcx, name);
        CollectedFunction {
            instance: Instance::mono(tcx, definition.to_def_id()),
            role: if is_kernel {
                crate::collector::CollectedFunctionRole::KernelEntry
            } else {
                crate::collector::CollectedFunctionRole::InternalHelper
            },
            export_name: format!("fe2o3_test_{name}"),
            logical_name: is_kernel.then(|| name.to_owned()),
            typed_profile: None,
            kernel_binding: None,
            typed_layout_identities: None,
            general_typed_contract: None,
            frontend_contract: is_kernel
                .then(|| AuthenticatedKernelFrontendContractV1::for_test(scalar_contract())),
            dead_branches: None,
        }
    }

    fn scalar_contract() -> KernelFrontendContractV1 {
        let one = FrontendWorkgroupDimensionsV1::new([1, 1, 1]).unwrap();
        KernelFrontendContractV1::new(
            Some(FrontendLaunchBoundsV1::new(Some(one), Some(one), None).unwrap()),
            None,
        )
        .unwrap()
    }

    fn local_function(tcx: TyCtxt<'_>, name: &str) -> LocalDefId {
        tcx.iter_local_def_id()
            .find(|definition| {
                tcx.def_kind(definition.to_def_id()) == DefKind::Fn
                    && tcx.item_name(definition.to_def_id()).as_str() == name
            })
            .unwrap_or_else(|| panic!("missing fixture function `{name}`"))
    }

    struct CompilerFixture {
        source: PathBuf,
        output: PathBuf,
    }

    impl CompilerFixture {
        fn create() -> Self {
            let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let stem = format!("fe2o3-frontend-record-{}-{sequence}", std::process::id());
            let source = std::env::temp_dir().join(format!("{stem}.rs"));
            let output = std::env::temp_dir().join(format!("{stem}.rmeta"));
            fs::write(&source, FIXTURE_SOURCE).expect("write frontend record fixture");
            Self { source, output }
        }
    }

    impl Drop for CompilerFixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.source);
            let _ = fs::remove_file(&self.output);
        }
    }

    fn compiler_results() -> DriverResults {
        let fixture = CompilerFixture::create();
        let sysroot = Command::new("rustc")
            .args(["--print", "sysroot"])
            .output()
            .expect("query rustc sysroot");
        assert!(sysroot.status.success(), "rustc --print sysroot failed");
        let sysroot = String::from_utf8(sysroot.stdout).expect("UTF-8 rustc sysroot");
        let args = vec![
            "rustc".to_owned(),
            "--crate-name".to_owned(),
            "fe2o3_frontend_record_fixture".to_owned(),
            "--crate-type".to_owned(),
            "lib".to_owned(),
            "--edition".to_owned(),
            "2024".to_owned(),
            "--emit".to_owned(),
            "metadata".to_owned(),
            "-Cpanic=abort".to_owned(),
            "--sysroot".to_owned(),
            sysroot.trim().to_owned(),
            "-o".to_owned(),
            fixture.output.display().to_string(),
            fixture.source.display().to_string(),
        ];
        let mut callbacks = BridgeCallbacks::default();
        rustc_driver::run_compiler(&args, &mut callbacks);
        callbacks
            .results
            .expect("frontend record callback did not run")
    }

    #[test]
    fn rustc_records_are_canonical_and_preserve_cfg_and_locations() {
        let results = compiler_results();
        assert!(results.canonical_equal);
        assert!(results.unit_equal);
        assert_eq!(results.function_count, 2);
        assert!(results.scalar_outcome_present);
        assert!(results.scalar_identity_equal);
        assert!(results.has_kernel);
        assert!(results.cfg_and_locations_valid);
    }

    #[test]
    fn malformed_collections_fail_closed() {
        let results = compiler_results();
        assert!(matches!(
            results.helper_only,
            FrontendRecordBridgeError::Validation(ValidationError::MissingKernel)
        ));
        assert!(matches!(
            results.duplicate,
            FrontendRecordBridgeError::Validation(ValidationError::Duplicate {
                field: "function identities"
            })
        ));
    }

    #[test]
    fn identity_preimages_are_bounded_and_domain_separated() {
        let oversized = vec![0_u8; MAX_IDENTITY_PREIMAGE_BYTES];
        assert_eq!(
            hash_identity(FUNCTION_IDENTITY_DOMAIN, &[&oversized]),
            Err(FrontendRecordBridgeError::IdentityPreimageTooLarge)
        );
        assert_ne!(
            hash_identity(FUNCTION_IDENTITY_DOMAIN, &[b"same"]).unwrap(),
            hash_identity(TYPE_IDENTITY_DOMAIN, &[b"same"]).unwrap()
        );
    }
}
