//! Closed construction of direct LLVM worker requests from validated plans.

use std::fmt;

use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, LinkPlanIdentityV1, MultiInputLinkPlanV1, WorkerInputKindV1, WorkerInputV1,
    WorkerOptimizationLevelV1, WorkerOptionsV1, WorkerOutputConstraintsV1, WorkerProtocolError,
    WorkerRequestV1, worker_protocol::validate_symbols,
};

const INPUT_KIND_CLOSURE_DOMAIN_V1: &[u8] = b"FE2O3/DEVICE-LINK-INPUT-KIND-CLOSURE/V1\0";
const SYMBOL_CLOSURE_DOMAIN_V1: &[u8] = b"FE2O3/DEVICE-LINK-SYMBOL-CLOSURE/V1\0";
const PLAN_REQUEST_DOMAIN_V1: &[u8] = b"FE2O3/PLAN-BOUND-WORKER-REQUEST/V1\0";

/// Stable identity of a canonical required/import/export symbol closure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LinkSymbolClosureIdentityV1([u8; 32]);

impl LinkSymbolClosureIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable identity of the plan-bound input-role sequence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LinkInputKindClosureIdentityV1([u8; 32]);

impl LinkInputKindClosureIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Independent source of truth for each canonical link input's file kind.
///
/// `MultiInputLinkPlanV1` predates typed inputs, so changing its V1 canonical
/// bytes would be a wire-format break. This companion closure binds one kind to
/// each plan input in canonical identity order. It is inert data and grants no
/// link, load, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkInputKindClosureV1 {
    plan_identity: LinkPlanIdentityV1,
    kinds: Vec<WorkerInputKindV1>,
    identity: LinkInputKindClosureIdentityV1,
}

impl LinkInputKindClosureV1 {
    pub fn new(
        plan: &MultiInputLinkPlanV1,
        kinds: Vec<WorkerInputKindV1>,
    ) -> Result<Self, WorkerRequestConstructionError> {
        if kinds.len() != plan.inputs().len() {
            return Err(WorkerRequestConstructionError::InputKindCountMismatch {
                planned: plan.inputs().len(),
                declared: kinds.len(),
            });
        }
        let plan_identity = plan.identity();
        let identity = calculate_input_kind_closure_identity(plan, &kinds);
        Ok(Self {
            plan_identity,
            kinds,
            identity,
        })
    }

    pub const fn plan_identity(&self) -> LinkPlanIdentityV1 {
        self.plan_identity
    }

    pub fn kinds(&self) -> &[WorkerInputKindV1] {
        &self.kinds
    }

    pub const fn identity(&self) -> LinkInputKindClosureIdentityV1 {
        self.identity
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Exact externally visible symbol closure expected from a native device link.
///
/// `required_symbols` is the complete final defined-symbol set. Imports and
/// exports are disjoint directional annotations and must each be subsets of
/// that set. The closure is inert data and grants no link, load, or launch
/// authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkSymbolClosureV1 {
    required_symbols: Vec<String>,
    import_symbols: Vec<String>,
    export_symbols: Vec<String>,
    identity: LinkSymbolClosureIdentityV1,
}

impl LinkSymbolClosureV1 {
    pub fn new(
        required_symbols: Vec<String>,
        import_symbols: Vec<String>,
        export_symbols: Vec<String>,
    ) -> Result<Self, WorkerRequestConstructionError> {
        if required_symbols.is_empty() {
            return Err(WorkerRequestConstructionError::EmptySymbolClosure);
        }
        validate_symbols(&required_symbols)
            .map_err(WorkerRequestConstructionError::InvalidRequiredSymbols)?;
        validate_symbols(&import_symbols)
            .map_err(WorkerRequestConstructionError::InvalidImportSymbols)?;
        validate_symbols(&export_symbols)
            .map_err(WorkerRequestConstructionError::InvalidExportSymbols)?;

        for symbol in &import_symbols {
            if required_symbols.binary_search(symbol).is_err() {
                return Err(WorkerRequestConstructionError::UnreferencedImport(
                    symbol.clone(),
                ));
            }
            if export_symbols.binary_search(symbol).is_ok() {
                return Err(WorkerRequestConstructionError::ConflictingSymbolRole(
                    symbol.clone(),
                ));
            }
        }
        for symbol in &export_symbols {
            if required_symbols.binary_search(symbol).is_err() {
                return Err(WorkerRequestConstructionError::UnreferencedExport(
                    symbol.clone(),
                ));
            }
        }

        let identity =
            calculate_closure_identity(&required_symbols, &import_symbols, &export_symbols);
        Ok(Self {
            required_symbols,
            import_symbols,
            export_symbols,
            identity,
        })
    }

    pub fn required_symbols(&self) -> &[String] {
        &self.required_symbols
    }

    pub fn import_symbols(&self) -> &[String] {
        &self.import_symbols
    }

    pub fn export_symbols(&self) -> &[String] {
        &self.export_symbols
    }

    pub const fn identity(&self) -> LinkSymbolClosureIdentityV1 {
        self.identity
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Builds one deterministic worker request from a fully validated link plan.
///
/// The caller must supply inputs in the plan's canonical identity order. The
/// request target, code-object version, structured worker options, exact input
/// bytes, symbol closure, and output bound are checked before a request can be
/// returned for execution.
#[allow(clippy::too_many_arguments)]
pub fn construct_worker_request_v1(
    plan: &MultiInputLinkPlanV1,
    llvm_build_identity: impl Into<String>,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    options: WorkerOptionsV1,
    inputs: Vec<WorkerInputV1>,
    input_kinds: &LinkInputKindClosureV1,
    symbols: &LinkSymbolClosureV1,
    output: WorkerOutputConstraintsV1,
) -> Result<WorkerRequestV1, WorkerRequestConstructionError> {
    if target != plan.target() {
        return Err(WorkerRequestConstructionError::TargetMismatch);
    }
    let (planned_code_object_version, planned_options) = decode_plan_options(plan)?;
    if code_object_version != planned_code_object_version {
        return Err(WorkerRequestConstructionError::CodeObjectVersionMismatch {
            planned: planned_code_object_version,
            requested: code_object_version,
        });
    }
    if options != planned_options {
        return Err(WorkerRequestConstructionError::OptionsMismatch {
            planned: planned_options,
            requested: options,
        });
    }
    validate_inputs(plan, input_kinds, &inputs)?;

    let expected_output_bytes = plan.output().identity().byte_len();
    if output.max_bytes() != expected_output_bytes {
        return Err(WorkerRequestConstructionError::OutputBoundMismatch {
            planned: expected_output_bytes,
            requested: output.max_bytes(),
        });
    }

    let llvm_build_identity = llvm_build_identity.into();
    let request_id = calculate_request_id(
        plan,
        &llvm_build_identity,
        target,
        code_object_version,
        options,
        &inputs,
        input_kinds,
        symbols,
        &output,
    );
    if request_id == [0; 32] {
        return Err(WorkerRequestConstructionError::ReservedRequestId);
    }

    WorkerRequestV1::new(
        request_id,
        llvm_build_identity,
        target,
        code_object_version,
        options,
        inputs,
        symbols.required_symbols.clone(),
        symbols.required_symbols.clone(),
        output,
    )
    .map_err(WorkerRequestConstructionError::WorkerProtocol)
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerRequestConstructionError {
    EmptySymbolClosure,
    InvalidRequiredSymbols(WorkerProtocolError),
    InvalidImportSymbols(WorkerProtocolError),
    InvalidExportSymbols(WorkerProtocolError),
    UnreferencedImport(String),
    UnreferencedExport(String),
    ConflictingSymbolRole(String),
    TargetMismatch,
    MissingCodeObjectVersion,
    InvalidCodeObjectVersion(String),
    UnsupportedLinkOption(String),
    InvalidLinkOptionValue {
        name: String,
        value: String,
    },
    CodeObjectVersionMismatch {
        planned: CodeObjectVersion,
        requested: CodeObjectVersion,
    },
    OptionsMismatch {
        planned: WorkerOptionsV1,
        requested: WorkerOptionsV1,
    },
    InputCountMismatch {
        planned: usize,
        provided: usize,
    },
    InputKindCountMismatch {
        planned: usize,
        declared: usize,
    },
    InputKindPlanMismatch {
        planned: LinkPlanIdentityV1,
        declared: LinkPlanIdentityV1,
    },
    InputKindMismatch {
        index: usize,
        planned: WorkerInputKindV1,
        provided: WorkerInputKindV1,
    },
    InputIdentityMismatch {
        index: usize,
        planned: ContentIdentityV1,
        provided: ContentIdentityV1,
    },
    InputBytesMismatch {
        index: usize,
        planned: ContentIdentityV1,
    },
    OutputBoundMismatch {
        planned: u64,
        requested: u64,
    },
    ReservedRequestId,
    WorkerProtocol(WorkerProtocolError),
}

impl fmt::Display for WorkerRequestConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySymbolClosure => formatter.write_str("device link symbol closure is empty"),
            Self::InvalidRequiredSymbols(error) => {
                write!(formatter, "invalid required-symbol set: {error}")
            }
            Self::InvalidImportSymbols(error) => {
                write!(formatter, "invalid import-symbol set: {error}")
            }
            Self::InvalidExportSymbols(error) => {
                write!(formatter, "invalid export-symbol set: {error}")
            }
            Self::UnreferencedImport(symbol) => {
                write!(
                    formatter,
                    "import {symbol} is absent from the required-symbol set"
                )
            }
            Self::UnreferencedExport(symbol) => {
                write!(
                    formatter,
                    "export {symbol} is absent from the required-symbol set"
                )
            }
            Self::ConflictingSymbolRole(symbol) => {
                write!(formatter, "symbol {symbol} is both imported and exported")
            }
            Self::TargetMismatch => {
                formatter.write_str("worker request target does not match link plan")
            }
            Self::MissingCodeObjectVersion => {
                formatter.write_str("link plan has no code-object-version option")
            }
            Self::InvalidCodeObjectVersion(value) => {
                write!(formatter, "unsupported code-object-version value {value}")
            }
            Self::UnsupportedLinkOption(name) => {
                write!(formatter, "unsupported direct-link option {name}")
            }
            Self::InvalidLinkOptionValue { name, value } => {
                write!(
                    formatter,
                    "invalid value {value} for direct-link option {name}"
                )
            }
            Self::CodeObjectVersionMismatch { planned, requested } => write!(
                formatter,
                "requested code-object version {requested:?} does not match plan {planned:?}"
            ),
            Self::OptionsMismatch { planned, requested } => write!(
                formatter,
                "requested worker options {requested:?} do not match plan {planned:?}"
            ),
            Self::InputCountMismatch { planned, provided } => write!(
                formatter,
                "provided input count {provided} does not match plan count {planned}"
            ),
            Self::InputKindCountMismatch { planned, declared } => write!(
                formatter,
                "declared input-kind count {declared} does not match plan count {planned}"
            ),
            Self::InputKindPlanMismatch { planned, declared } => write!(
                formatter,
                "input-kind closure plan identity {declared:?} does not match plan {planned:?}"
            ),
            Self::InputKindMismatch {
                index,
                planned,
                provided,
            } => write!(
                formatter,
                "provided input {index} kind {provided:?} does not match declared kind {planned:?}"
            ),
            Self::InputIdentityMismatch {
                index,
                planned,
                provided,
            } => write!(
                formatter,
                "provided input {index} identity {provided} does not match plan {planned}"
            ),
            Self::InputBytesMismatch { index, planned } => write!(
                formatter,
                "provided input {index} bytes do not match plan identity {planned}"
            ),
            Self::OutputBoundMismatch { planned, requested } => write!(
                formatter,
                "requested output bound {requested} does not match planned length {planned}"
            ),
            Self::ReservedRequestId => {
                formatter.write_str("derived worker request ID is the reserved zero value")
            }
            Self::WorkerProtocol(error) => {
                write!(formatter, "worker request validation failed: {error}")
            }
        }
    }
}

impl std::error::Error for WorkerRequestConstructionError {}

fn validate_inputs(
    plan: &MultiInputLinkPlanV1,
    input_kinds: &LinkInputKindClosureV1,
    inputs: &[WorkerInputV1],
) -> Result<(), WorkerRequestConstructionError> {
    if input_kinds.plan_identity != plan.identity() {
        return Err(WorkerRequestConstructionError::InputKindPlanMismatch {
            planned: plan.identity(),
            declared: input_kinds.plan_identity,
        });
    }
    if inputs.len() != plan.inputs().len() {
        return Err(WorkerRequestConstructionError::InputCountMismatch {
            planned: plan.inputs().len(),
            provided: inputs.len(),
        });
    }
    for (index, ((planned, planned_kind), provided)) in plan
        .inputs()
        .iter()
        .zip(&input_kinds.kinds)
        .zip(inputs)
        .enumerate()
    {
        if provided.identity() != planned.identity() {
            return Err(WorkerRequestConstructionError::InputIdentityMismatch {
                index,
                planned: planned.identity(),
                provided: provided.identity(),
            });
        }
        if !planned.identity().matches(provided.bytes()) {
            return Err(WorkerRequestConstructionError::InputBytesMismatch {
                index,
                planned: planned.identity(),
            });
        }
        if *planned_kind != provided.kind() {
            return Err(WorkerRequestConstructionError::InputKindMismatch {
                index,
                planned: *planned_kind,
                provided: provided.kind(),
            });
        }
    }
    Ok(())
}

fn decode_plan_options(
    plan: &MultiInputLinkPlanV1,
) -> Result<(CodeObjectVersion, WorkerOptionsV1), WorkerRequestConstructionError> {
    let mut code_object_version = None;
    let mut optimization = WorkerOptimizationLevelV1::O0;
    let mut strip_debug = false;
    let mut verify_each = false;

    for option in plan.options() {
        match option.name() {
            "code-object-version" => {
                code_object_version = Some(match option.value() {
                    "4" => CodeObjectVersion::V4,
                    "5" => CodeObjectVersion::V5,
                    "6" => CodeObjectVersion::V6,
                    value => {
                        return Err(WorkerRequestConstructionError::InvalidCodeObjectVersion(
                            value.to_owned(),
                        ));
                    }
                });
            }
            "opt-level" => {
                optimization = match option.value() {
                    "0" => WorkerOptimizationLevelV1::O0,
                    "1" => WorkerOptimizationLevelV1::O1,
                    "2" => WorkerOptimizationLevelV1::O2,
                    "3" => WorkerOptimizationLevelV1::O3,
                    value => {
                        return Err(WorkerRequestConstructionError::InvalidLinkOptionValue {
                            name: option.name().to_owned(),
                            value: value.to_owned(),
                        });
                    }
                };
            }
            "strip-debug" => strip_debug = decode_bool_option(option.name(), option.value())?,
            "verify-each" => verify_each = decode_bool_option(option.name(), option.value())?,
            name => {
                return Err(WorkerRequestConstructionError::UnsupportedLinkOption(
                    name.to_owned(),
                ));
            }
        }
    }
    let code_object_version =
        code_object_version.ok_or(WorkerRequestConstructionError::MissingCodeObjectVersion)?;
    Ok((
        code_object_version,
        WorkerOptionsV1::new(optimization, strip_debug, verify_each),
    ))
}

fn decode_bool_option(name: &str, value: &str) -> Result<bool, WorkerRequestConstructionError> {
    match value {
        "false" => Ok(false),
        "true" => Ok(true),
        _ => Err(WorkerRequestConstructionError::InvalidLinkOptionValue {
            name: name.to_owned(),
            value: value.to_owned(),
        }),
    }
}

fn calculate_closure_identity(
    required_symbols: &[String],
    import_symbols: &[String],
    export_symbols: &[String],
) -> LinkSymbolClosureIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(SYMBOL_CLOSURE_DOMAIN_V1);
    hash_strings(&mut hasher, required_symbols);
    hash_strings(&mut hasher, import_symbols);
    hash_strings(&mut hasher, export_symbols);
    LinkSymbolClosureIdentityV1(hasher.finalize().into())
}

fn calculate_input_kind_closure_identity(
    plan: &MultiInputLinkPlanV1,
    kinds: &[WorkerInputKindV1],
) -> LinkInputKindClosureIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(INPUT_KIND_CLOSURE_DOMAIN_V1);
    hasher.update(plan.identity().as_bytes());
    hasher.update((kinds.len() as u64).to_le_bytes());
    for (input, kind) in plan.inputs().iter().zip(kinds) {
        hasher.update(input.identity().sha256());
        hasher.update(input.identity().byte_len().to_le_bytes());
        hasher.update([*kind as u8]);
    }
    LinkInputKindClosureIdentityV1(hasher.finalize().into())
}

#[allow(clippy::too_many_arguments)]
fn calculate_request_id(
    plan: &MultiInputLinkPlanV1,
    llvm_build_identity: &str,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    options: WorkerOptionsV1,
    inputs: &[WorkerInputV1],
    input_kinds: &LinkInputKindClosureV1,
    symbols: &LinkSymbolClosureV1,
    output: &WorkerOutputConstraintsV1,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PLAN_REQUEST_DOMAIN_V1);
    hasher.update(plan.identity().as_bytes());
    hasher.update(input_kinds.identity.as_bytes());
    hasher.update(symbols.identity.as_bytes());
    hash_text(&mut hasher, llvm_build_identity);
    hash_text(&mut hasher, &target.to_string());
    hasher.update([code_object_version_byte(code_object_version)]);
    hasher.update([
        options.optimization() as u8,
        u8::from(options.strip_debug()),
        u8::from(options.verify_each()),
    ]);
    hasher.update((inputs.len() as u64).to_le_bytes());
    for input in inputs {
        hasher.update([input.kind() as u8]);
        hasher.update(input.identity().sha256());
        hasher.update(input.identity().byte_len().to_le_bytes());
    }
    hasher.update(output.max_bytes().to_le_bytes());
    hasher.finalize().into()
}

fn hash_strings(hasher: &mut Sha256, strings: &[String]) {
    hasher.update((strings.len() as u64).to_le_bytes());
    for string in strings {
        hash_text(hasher, string);
    }
}

fn hash_text(hasher: &mut Sha256, text: &str) {
    hasher.update((text.len() as u64).to_le_bytes());
    hasher.update(text.as_bytes());
}

const fn code_object_version_byte(version: CodeObjectVersion) -> u8 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}
