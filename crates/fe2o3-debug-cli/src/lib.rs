#![deny(unsafe_code, unsafe_op_in_unsafe_fn)]
#![doc = include_str!("../README.md")]

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[allow(unsafe_code)]
mod hardware_linux_v2;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[allow(unsafe_code)]
mod hardware_linux_v3;
mod hardware_v2;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod live_gpu_backend_v3;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod live_kfd_v3;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod live_rocgdb_kfd_v4;
#[cfg(target_os = "linux")]
mod live_rocgdb_v3;
pub mod reference_archive_v1;
#[cfg(target_os = "linux")]
mod rocgdb_mi_parser_v3;
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
pub mod rocgdb_mi_v3;
#[cfg(target_os = "linux")]
pub mod rocgdb_mi_v4;

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::{OsStr, OsString};
#[cfg(target_os = "linux")]
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, BorrowedFd, RawFd};
#[cfg(target_os = "linux")]
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::process::ExitCode;

use fe2o3_debug_protocol::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, DebugSourceMapDocumentV1, DebugSourceMapDocumentV2,
    DebugSourceMapKirSiteV1, DebugSourceMapSpanV1, DebugSourceVariableBindingV2,
    DebugSourceVariableFallbackV2, IndexedControlFlow, MemoryOrdering, OperationKind, ScalarType,
    SynchronizationScope, Type, ValueId, analyze_control_flow, simulation_debug_map_identity_v1,
    simulation_debug_map_identity_v2,
};
use fe2o3_kir_debugger::{
    DebugBreakpointV1, DebugHitConditionV1, DebugInspectionUnavailableV1, DebugInspectionV1,
    DebugKirIdentityV1, DebugNavigationV1, DebugPredicateV1, DebugScopeSelectorV1, DebugSessionV1,
    DebugSiteSelectorV1, DebugSourceCatalogV1, DebugSourceFileV1, DebugSourceResolutionV1,
    DebugSourceSiteV1, DebugSourceSpanV1, DebugStopReasonV1, DebugStopV1,
    DebugTerminalDetailStateV2, DebugTerminalDetailV2, DebugTerminalFaultV1,
    DebugTranscriptCompletenessV1, DebugTranscriptTruncationV1, DebugWatchAccessV1,
    DebugWatchpointV1, DebugWaveWidthV1, DebuggerLimitsV1, capture_debugger_replayed_run_v1,
    capture_debugger_run_v1, hierarchy_for_invocation_v1,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, IndexWidthV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationDebugBarrierActionV1, SimulationDebugBindingV1, SimulationDebugCaptureLimitsV1,
    SimulationDebugCheckpointPhaseV1, SimulationDebugCollectionV1, SimulationDebugFrameV1,
    SimulationDebugRecordKindV1, SimulationDebugRecordV1, SimulationDebugSiteV1,
    SimulationDebugValueV1, SimulationErrorV1, SimulationInvocationV1, SimulationScheduleRecordV1,
    SimulationTargetV1,
};
use fe2o3_kir_sim_cli::{
    AdmittedSimulationInputV1, SimulationInputErrorV1, load_debug_sidecar_v1,
    load_debug_simulation_bundle_v1, load_debug_simulation_bundle_v2,
    load_debug_simulation_input_bytes_v1, load_debug_simulation_input_v1,
    load_debug_simulation_schedule_v1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const USAGE: &str = "usage: fe2o3-debug sim ((--kir-v7 PATH | --bundle PATH | --bundle-v2 PATH) --request PATH | --kir-v7-fd FD --request-fd FD) [--replay-schedule PATH] [--source-map PATH --source-bundle-subject ID] [--protocol jsonl] [--wave-width 32|64]\n       fe2o3-debug live-kfd --bundle-v2 PATH --request PATH --hsaco PATH [--protocol jsonl] [--wave-width 32|64] -- PROGRAM [ARG...]\n       fe2o3-debug live-rocgdb --rocgdb PATH --authorization ID [--protocol jsonl] [--wave-width 32|64] [--timeout-ms N] (--attach PID | -- PROGRAM [ARG...])\n       fe2o3-debug live-rocgdb-kfd-v4 --rocgdb PATH --authorization ID --hsaco PATH --load-base 0xHEX --kernel NAME [--device-unique-id DECIMAL] [--protocol jsonl] [--wave-width 32|64] [--timeout-ms N] -- PROGRAM [ARG...]\n       fe2o3-debug hardware -- PROGRAM [ARG...]";
const MAX_SESSION_COMMANDS_V1: u64 = 1_000_000;
#[cfg(target_os = "linux")]
const MAX_SEALED_DEBUG_INPUT_BYTES_V1: usize = 16 * 1024 * 1024;
const TRACE_HEADER_SCHEMA_V1: &str = "fe2o3-debug-trace-v1";
pub const SOURCE_MAP_SCHEMA_V1: &str = fe2o3_kernel_ir::DEBUG_SOURCE_MAP_SCHEMA_V1;
pub const MAX_SOURCE_MAP_BYTES_V1: usize = fe2o3_kernel_ir::MAX_SIMULATION_DEBUG_MAP_BYTES_V1;

#[derive(Debug)]
struct OptionsV1 {
    program: ProgramInputV1,
    request: RequestInputV1,
    source_map: Option<PathBuf>,
    source_bundle_subject: Option<OpaqueIdentityV1>,
    replay_schedule: Option<PathBuf>,
    wave_width: DebugWaveWidthV1,
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[derive(Debug)]
struct LiveKfdOptionsV3 {
    bundle_v2: PathBuf,
    request: PathBuf,
    hsaco: PathBuf,
    wave_width: DebugWaveWidthV1,
    program: PathBuf,
    program_arguments: Vec<OsString>,
}

#[derive(Debug)]
enum ProgramInputV1 {
    KirV7(PathBuf),
    SealedKirV7Fd(i32),
    Bundle(PathBuf),
    BundleV2(PathBuf),
}

#[derive(Debug)]
enum RequestInputV1 {
    Path(PathBuf),
    SealedFd(i32),
}

#[derive(Debug)]
struct AdmittedSourceMapV1 {
    identity: OpaqueIdentityV1,
    bundle_subject_identity: OpaqueIdentityV1,
    configuration_identity: OpaqueIdentityV1,
    provenance: SourceMapProvenanceV1,
    catalog: DebugSourceCatalogV1,
}

#[derive(Clone, Copy, Debug)]
enum AdmittedKirValueDefinitionV2 {
    FunctionParameter,
    BlockParameter(fe2o3_kernel_ir::BlockId),
    OperationResult {
        block: fe2o3_kernel_ir::BlockId,
        operation: u32,
    },
}

#[derive(Debug)]
struct AdmittedFunctionIndexV2 {
    control_flow: IndexedControlFlow,
    definitions: BTreeMap<ValueId, AdmittedKirValueDefinitionV2>,
}

#[derive(Clone, Copy, Debug)]
struct AdmittedSourceVariableLocationV2 {
    block: fe2o3_kernel_ir::BlockId,
    next_operation: u32,
    generation: u64,
    binding: DebugSourceVariableBindingV2,
}

#[derive(Debug)]
struct AdmittedSourceVariableV2 {
    identity: OpaqueIdentityV1,
    name: String,
    function_ordinal: usize,
    scope_identity: OpaqueIdentityV1,
    scope_depth: u32,
    fallback: DebugSourceVariableFallbackV2,
    function_binding: Option<(u64, ValueId)>,
    locations: Vec<AdmittedSourceVariableLocationV2>,
}

#[derive(Debug, Default)]
struct AdmittedSourceVariablesV2 {
    variables: Vec<AdmittedSourceVariableV2>,
    by_identity: BTreeMap<OpaqueIdentityV1, usize>,
    by_name: BTreeMap<(usize, String), Vec<usize>>,
    by_function: BTreeMap<usize, Vec<usize>>,
    scope_parents: BTreeMap<OpaqueIdentityV1, Option<OpaqueIdentityV1>>,
}

#[derive(Debug)]
struct AdmittedSourceMapV2 {
    identity: OpaqueIdentityV1,
    bundle_subject_identity: OpaqueIdentityV1,
    configuration_identity: OpaqueIdentityV1,
    provenance: SourceMapProvenanceV1,
    catalog: DebugSourceCatalogV1,
    variables: AdmittedSourceVariablesV2,
    diagnosis_operation_members: Vec<AdmittedDiagnosisSourceMemberV2>,
    diagnosis_operation_root: OpaqueIdentityV1,
    diagnosis_operation_count: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AdmittedDiagnosisSourceMemberV2 {
    kir_site: KirSiteV1,
    location: SourceLocationV1,
    identity: OpaqueIdentityV1,
}

#[derive(Clone, Copy, Debug)]
enum ConvertErrorV1 {
    Unavailable(
        DebugCapabilityNameV1,
        CapabilityUnavailableReasonV1,
        &'static str,
    ),
    Invalid(&'static str),
}

fn simulator_capabilities(
    source_map_bound: bool,
    source_variables_v2: Option<bool>,
) -> Vec<CapabilityViewV1> {
    use CapabilityAvailabilityV1::{Available, Unavailable};
    use CapabilityUnavailableReasonV1::{
        LogicalVisualizationOnly, NotCaptured, NotExposedByBackend, NotRepresented,
        RequiresAuthenticatedMap,
    };
    use DebugCapabilityNameV1::*;
    vec![
        capability(HierarchyInspection, Available, None),
        capability(KirSites, Available, None),
        capability(
            SourceSites,
            if source_map_bound {
                Available
            } else {
                Unavailable
            },
            if source_map_bound {
                None
            } else {
                Some(RequiresAuthenticatedMap)
            },
        ),
        capability(CallStack, Available, None),
        capability(Breakpoints, Available, None),
        capability(Watchpoints, Available, None),
        capability(ForwardStep, Available, None),
        capability(ReverseStep, Available, None),
        capability(Pause, Unavailable, Some(NotExposedByBackend)),
        capability(DeterministicReplay, Available, None),
        capability(KirSsaValues, Available, None),
        match source_variables_v2 {
            Some(true) => capability(SourceVariableValues, Available, None),
            Some(false) => capability(SourceVariableValues, Unavailable, Some(NotCaptured)),
            None => capability(
                SourceVariableValues,
                Unavailable,
                Some(RequiresAuthenticatedMap),
            ),
        },
        capability(RegisterValues, Unavailable, Some(NotRepresented)),
        capability(AllocationRelativeMemory, Available, None),
        capability(SemanticTrace, Available, None),
        capability(
            HardwareWaveState,
            Unavailable,
            Some(LogicalVisualizationOnly),
        ),
        capability(KfdDispatchControl, Unavailable, Some(NotExposedByBackend)),
    ]
}

const fn capability(
    name: DebugCapabilityNameV1,
    availability: CapabilityAvailabilityV1,
    reason: Option<CapabilityUnavailableReasonV1>,
) -> CapabilityViewV1 {
    CapabilityViewV1 {
        name,
        availability,
        reason,
    }
}

fn admit_source_map_v1(
    bytes: &[u8],
    input: &AdmittedSimulationInputV1,
    configuration_identity: OpaqueIdentityV1,
    expected_bundle_subject: OpaqueIdentityV1,
) -> Result<AdmittedSourceMapV1, String> {
    admit_source_map_with_provenance_v1(
        bytes,
        input,
        configuration_identity,
        expected_bundle_subject,
        None,
        SourceMapProvenanceV1::CallerBound,
    )
}

fn admit_source_map_with_provenance_v1(
    bytes: &[u8],
    input: &AdmittedSimulationInputV1,
    configuration_identity: OpaqueIdentityV1,
    expected_bundle_subject: OpaqueIdentityV1,
    expected_map_identity: Option<OpaqueIdentityV1>,
    provenance: SourceMapProvenanceV1,
) -> Result<AdmittedSourceMapV1, String> {
    if bytes.is_empty() || bytes.len() > MAX_SOURCE_MAP_BYTES_V1 {
        return Err(format!(
            "source map must contain 1 to {MAX_SOURCE_MAP_BYTES_V1} bytes"
        ));
    }
    let map_identity = debug_source_map_identity_v1(bytes)?;
    if expected_map_identity.is_some_and(|expected| expected != map_identity) {
        return Err("source map identity does not match the compiler bundle commitment".to_owned());
    }
    let document =
        DebugSourceMapDocumentV1::from_json_bytes(bytes).map_err(|error| error.to_string())?;
    let binding = document.binding();
    if binding.bundle_subject_identity() != expected_bundle_subject.as_bytes() {
        return Err(
            "source map bundle subject identity does not match the expected subject".to_owned(),
        );
    }
    let canonical_len = input.module.identity().canonical_length();
    if binding.canonical_kir().digest() != input.kir_sha256
        || binding.canonical_kir().canonical_bytes() != canonical_len
    {
        return Err(format!(
            "source map canonical KIR identity does not match admitted digest {} length {canonical_len}",
            hex_bytes(&input.kir_sha256)
        ));
    }
    let files = document
        .files()
        .iter()
        .map(|file| DebugSourceFileV1 {
            identity: file.identity(),
            byte_len: file.byte_len(),
            display_path: file.display_path().to_owned(),
        })
        .collect();
    let mut sites = Vec::new();
    sites
        .try_reserve_exact(document.sites().len())
        .map_err(|_| "source map site allocation failed".to_owned())?;
    for site in document.sites() {
        sites.push(DebugSourceSiteV1 {
            site: source_map_wire_site(&input.module, site.site())?,
            spans: site.spans().iter().copied().map(source_map_span).collect(),
        });
    }
    let catalog = DebugSourceCatalogV1::new_with_eliminated(
        DebugKirIdentityV1 {
            digest: input.kir_sha256,
            canonical_len,
        },
        files,
        sites,
        document
            .eliminated()
            .iter()
            .copied()
            .map(source_map_span)
            .collect(),
    )
    .map_err(|error| format!("source map catalog is invalid: {error}"))?;
    Ok(AdmittedSourceMapV1 {
        identity: map_identity,
        bundle_subject_identity: nonzero_identity(binding.bundle_subject_identity()),
        configuration_identity,
        provenance,
        catalog,
    })
}

fn admit_source_map_v2(
    bytes: &[u8],
    input: &AdmittedSimulationInputV1,
    configuration_identity: OpaqueIdentityV1,
    expected_bundle_subject: OpaqueIdentityV1,
    expected_map_identity: OpaqueIdentityV1,
) -> Result<AdmittedSourceMapV2, String> {
    admit_source_map_v2_with_provenance(
        bytes,
        input,
        configuration_identity,
        expected_bundle_subject,
        Some(expected_map_identity),
        SourceMapProvenanceV1::CompilerBundleBound,
    )
}

fn admit_caller_source_map_v2(
    bytes: &[u8],
    input: &AdmittedSimulationInputV1,
    configuration_identity: OpaqueIdentityV1,
    expected_bundle_subject: OpaqueIdentityV1,
) -> Result<AdmittedSourceMapV2, String> {
    admit_source_map_v2_with_provenance(
        bytes,
        input,
        configuration_identity,
        expected_bundle_subject,
        None,
        SourceMapProvenanceV1::CallerBound,
    )
}

fn admit_source_map_v2_with_provenance(
    bytes: &[u8],
    input: &AdmittedSimulationInputV1,
    configuration_identity: OpaqueIdentityV1,
    expected_bundle_subject: OpaqueIdentityV1,
    expected_map_identity: Option<OpaqueIdentityV1>,
    provenance: SourceMapProvenanceV1,
) -> Result<AdmittedSourceMapV2, String> {
    if bytes.is_empty() || bytes.len() > MAX_SOURCE_MAP_BYTES_V1 {
        return Err(format!(
            "source map V2 must contain 1 to {MAX_SOURCE_MAP_BYTES_V1} bytes"
        ));
    }
    let identity = nonzero_identity(simulation_debug_map_identity_v2(bytes));
    if expected_map_identity.is_some_and(|expected| identity != expected) {
        return Err("source map V2 identity does not match its bundle commitment".to_owned());
    }
    let document = DebugSourceMapDocumentV2::from_canonical_json_bytes(bytes)
        .map_err(|error| error.to_string())?;
    let binding = document.binding();
    if binding.bundle_subject_identity() != expected_bundle_subject.as_bytes() {
        return Err("source map V2 bundle subject identity does not match the bundle".to_owned());
    }
    let canonical_len = input.module.identity().canonical_length();
    if binding.canonical_kir().digest() != input.kir_sha256
        || binding.canonical_kir().canonical_bytes() != canonical_len
    {
        return Err(
            "source map V2 canonical KIR binding does not match the admitted input".to_owned(),
        );
    }
    let files = document
        .files()
        .iter()
        .map(|file| DebugSourceFileV1 {
            identity: file.identity(),
            byte_len: file.byte_len(),
            display_path: file.display_path().to_owned(),
        })
        .collect();
    let mut sites = Vec::new();
    sites
        .try_reserve_exact(document.sites().len())
        .map_err(|_| "source map V2 site allocation failed".to_owned())?;
    let mut diagnosis_operation_members = Vec::new();
    for site in document.sites() {
        let map_site = site.site();
        let kir_site = KirSiteV1 {
            function_ordinal: map_site.function_ordinal(),
            block_ordinal: map_site.block_ordinal(),
            point: KirSitePointV1::Operation {
                operation_ordinal: map_site.operation_ordinal(),
            },
        };
        diagnosis_operation_members
            .try_reserve_exact(site.spans().len())
            .map_err(|_| "source map V2 diagnosis membership allocation failed".to_owned())?;
        for span in site.spans().iter().copied().map(source_map_span) {
            let location = SourceLocationV1 {
                map_identity: identity,
                provenance,
                file_identity: nonzero_identity(span.file),
                byte_start: span.byte_start,
                byte_end: span.byte_end,
            };
            let member_identity = diagnosis_source_map_member_identity_v2(
                expected_bundle_subject,
                kir_site,
                location,
            )
            .map_err(|error| format!("source map V2 diagnosis member is invalid: {error}"))?;
            diagnosis_operation_members.push(AdmittedDiagnosisSourceMemberV2 {
                kir_site,
                location,
                identity: member_identity,
            });
        }
        sites.push(DebugSourceSiteV1 {
            site: source_map_wire_site(&input.module, map_site)?,
            spans: site.spans().iter().copied().map(source_map_span).collect(),
        });
    }
    diagnosis_operation_members.sort_unstable_by_key(|member| member.identity.as_bytes());
    if diagnosis_operation_members
        .windows(2)
        .any(|pair| pair[0].identity == pair[1].identity)
    {
        return Err("source map V2 diagnosis operation membership is duplicated".to_owned());
    }
    let mut diagnosis_operation_identities = Vec::new();
    diagnosis_operation_identities
        .try_reserve_exact(diagnosis_operation_members.len())
        .map_err(|_| "source map V2 diagnosis identity allocation failed".to_owned())?;
    diagnosis_operation_identities.extend(
        diagnosis_operation_members
            .iter()
            .map(|member| member.identity),
    );
    let diagnosis_operation_root =
        diagnosis_source_map_membership_root_v2(&diagnosis_operation_identities)
            .map_err(|error| format!("source map V2 diagnosis inventory is invalid: {error}"))?;
    let diagnosis_operation_count = u32::try_from(diagnosis_operation_members.len())
        .map_err(|_| "source map V2 diagnosis inventory is too large".to_owned())?;
    let catalog = DebugSourceCatalogV1::new_with_eliminated(
        DebugKirIdentityV1 {
            digest: input.kir_sha256,
            canonical_len,
        },
        files,
        sites,
        document
            .eliminated()
            .iter()
            .copied()
            .map(source_map_span)
            .collect(),
    )
    .map_err(|error| format!("source map V2 source catalog is invalid: {error}"))?;

    let mut referenced_functions = BTreeSet::new();
    for variable in document.variables() {
        referenced_functions.insert(variable.function_ordinal());
    }
    let mut function_indices = BTreeMap::new();
    for function_ordinal in referenced_functions {
        let ordinal = usize::try_from(function_ordinal)
            .map_err(|_| "source variable function ordinal does not fit this host".to_owned())?;
        let function = input
            .module
            .module()
            .functions
            .get(ordinal)
            .ok_or_else(|| "source variable function ordinal is unknown".to_owned())?;
        function_indices.insert(ordinal, admit_function_index_v2(function)?);
    }
    for scope in document.scopes() {
        let function_ordinal = usize::try_from(scope.function_ordinal())
            .map_err(|_| "source scope function ordinal does not fit this host".to_owned())?;
        input
            .module
            .module()
            .functions
            .get(function_ordinal)
            .and_then(|function| function.body.as_ref())
            .ok_or_else(|| "source scope function has no admitted KIR body".to_owned())?;
    }
    let scope_parents = document
        .scopes()
        .iter()
        .map(|scope| {
            (
                nonzero_identity(scope.identity()),
                scope.parent_identity().map(nonzero_identity),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut variables = Vec::new();
    variables
        .try_reserve_exact(document.variables().len())
        .map_err(|_| "source variable index allocation failed".to_owned())?;
    for variable in document.variables() {
        let function_ordinal = usize::try_from(variable.function_ordinal())
            .map_err(|_| "source variable function ordinal does not fit this host".to_owned())?;
        let function = input
            .module
            .module()
            .functions
            .get(function_ordinal)
            .ok_or_else(|| "source variable function ordinal is unknown".to_owned())?;
        let body = function
            .body
            .as_ref()
            .ok_or_else(|| "source variable function has no KIR body".to_owned())?;
        let index = function_indices
            .get(&function_ordinal)
            .ok_or_else(|| "source variable function index is missing".to_owned())?;
        let function_binding = variable
            .function_binding()
            .map(|binding| {
                let value = ValueId(
                    u32::try_from(binding.value_ordinal())
                        .map_err(|_| "source variable value does not fit KIR V7".to_owned())?,
                );
                if !matches!(
                    index.definitions.get(&value),
                    Some(AdmittedKirValueDefinitionV2::FunctionParameter)
                ) {
                    return Err(
                        "source variable function binding is not a KIR function parameter"
                            .to_owned(),
                    );
                }
                Ok((binding.generation(), value))
            })
            .transpose()?;
        let scope = document
            .scopes()
            .binary_search_by_key(&variable.scope_identity(), |scope| scope.identity())
            .ok()
            .map(|scope| document.scopes()[scope])
            .ok_or_else(|| "source variable scope is missing after validation".to_owned())?;
        let mut locations = Vec::new();
        locations
            .try_reserve_exact(variable.locations().len())
            .map_err(|_| "source variable location allocation failed".to_owned())?;
        for location in variable.locations() {
            let block_ordinal = usize::try_from(location.block_ordinal())
                .map_err(|_| "source variable block ordinal does not fit this host".to_owned())?;
            let block = body
                .blocks
                .get(block_ordinal)
                .ok_or_else(|| "source variable block ordinal is unknown".to_owned())?;
            let next_operation = u32::try_from(location.next_operation())
                .map_err(|_| "source variable checkpoint does not fit KIR V7".to_owned())?;
            if next_operation as usize > block.operations.len() {
                return Err("source variable checkpoint is outside its KIR block".to_owned());
            }
            if let DebugSourceVariableBindingV2::Captured { value_ordinal } = location.binding() {
                let value = ValueId(
                    u32::try_from(value_ordinal)
                        .map_err(|_| "source variable value does not fit KIR V7".to_owned())?,
                );
                let available = match index.definitions.get(&value).copied() {
                    Some(AdmittedKirValueDefinitionV2::FunctionParameter) => true,
                    Some(AdmittedKirValueDefinitionV2::BlockParameter(definition)) => {
                        index.control_flow.dominates(definition, block.id)
                    }
                    Some(AdmittedKirValueDefinitionV2::OperationResult {
                        block: definition,
                        operation,
                    }) if definition == block.id => operation < next_operation,
                    Some(AdmittedKirValueDefinitionV2::OperationResult {
                        block: definition,
                        ..
                    }) => index.control_flow.dominates(definition, block.id),
                    None => false,
                };
                if !available {
                    return Err(
                        "source variable references a KIR value unavailable at its checkpoint"
                            .to_owned(),
                    );
                }
            }
            locations.push(AdmittedSourceVariableLocationV2 {
                block: block.id,
                next_operation,
                generation: location.generation(),
                binding: location.binding(),
            });
        }
        locations.sort_unstable_by_key(|location| (location.block, location.next_operation));
        variables.push(AdmittedSourceVariableV2 {
            identity: nonzero_identity(variable.identity()),
            name: variable.name().to_owned(),
            function_ordinal,
            scope_identity: nonzero_identity(variable.scope_identity()),
            scope_depth: scope.depth(),
            fallback: variable.fallback(),
            function_binding,
            locations,
        });
    }
    let variables = index_source_variables_v2(variables, scope_parents)?;
    Ok(AdmittedSourceMapV2 {
        identity,
        bundle_subject_identity: expected_bundle_subject,
        configuration_identity,
        provenance,
        catalog,
        variables,
        diagnosis_operation_members,
        diagnosis_operation_root,
        diagnosis_operation_count,
    })
}

fn admit_function_index_v2(
    function: &fe2o3_kernel_ir::Function,
) -> Result<AdmittedFunctionIndexV2, String> {
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| "source variable function has no KIR body".to_owned())?;
    let control_flow = analyze_control_flow(function)
        .map_err(|_| "source variable function control flow is invalid".to_owned())?;
    let mut definitions = BTreeMap::new();
    for value in &body.parameters {
        if definitions
            .insert(*value, AdmittedKirValueDefinitionV2::FunctionParameter)
            .is_some()
        {
            return Err("source variable function has duplicate KIR values".to_owned());
        }
    }
    for block in &body.blocks {
        for parameter in &block.parameters {
            if definitions
                .insert(
                    parameter.id,
                    AdmittedKirValueDefinitionV2::BlockParameter(block.id),
                )
                .is_some()
            {
                return Err("source variable function has duplicate KIR values".to_owned());
            }
        }
        for (operation, definition) in block.operations.iter().enumerate() {
            let operation = u32::try_from(operation)
                .map_err(|_| "source variable operation ordinal is too large".to_owned())?;
            for result in &definition.results {
                if definitions
                    .insert(
                        result.id,
                        AdmittedKirValueDefinitionV2::OperationResult {
                            block: block.id,
                            operation,
                        },
                    )
                    .is_some()
                {
                    return Err("source variable function has duplicate KIR values".to_owned());
                }
            }
        }
    }
    Ok(AdmittedFunctionIndexV2 {
        control_flow,
        definitions,
    })
}

fn index_source_variables_v2(
    mut variables: Vec<AdmittedSourceVariableV2>,
    scope_parents: BTreeMap<OpaqueIdentityV1, Option<OpaqueIdentityV1>>,
) -> Result<AdmittedSourceVariablesV2, String> {
    variables.sort_unstable_by_key(|variable| variable.identity);
    let mut by_identity = BTreeMap::new();
    let mut by_name = BTreeMap::new();
    let mut by_function = BTreeMap::new();
    for (index, variable) in variables.iter().enumerate() {
        if by_identity.insert(variable.identity, index).is_some() {
            return Err("source variable identity is duplicated".to_owned());
        }
        by_name
            .entry((variable.function_ordinal, variable.name.clone()))
            .or_insert_with(Vec::new)
            .push(index);
        by_function
            .entry(variable.function_ordinal)
            .or_insert_with(Vec::new)
            .push(index);
    }
    Ok(AdmittedSourceVariablesV2 {
        variables,
        by_identity,
        by_name,
        by_function,
        scope_parents,
    })
}

/// Computes the exact identity committed by a compiler simulation bundle for
/// one canonical source-map payload.
pub fn debug_source_map_identity_v1(bytes: &[u8]) -> Result<OpaqueIdentityV1, String> {
    if bytes.is_empty() || bytes.len() > MAX_SOURCE_MAP_BYTES_V1 {
        return Err(format!(
            "source map must contain 1 to {MAX_SOURCE_MAP_BYTES_V1} bytes"
        ));
    }
    Ok(nonzero_identity(simulation_debug_map_identity_v1(bytes)))
}

fn source_map_wire_site(
    module: &AdmittedSimulationModuleV1,
    site: DebugSourceMapKirSiteV1,
) -> Result<fe2o3_kir_sim::SimulationDebugSiteV1, String> {
    source_map_operation_site(
        module,
        site.function_ordinal(),
        site.block_ordinal(),
        site.operation_ordinal(),
    )
}

fn source_map_site(
    module: &AdmittedSimulationModuleV1,
    site: KirSiteV1,
) -> Result<fe2o3_kir_sim::SimulationDebugSiteV1, String> {
    let KirSitePointV1::Operation { operation_ordinal } = site.point else {
        return Err("source map sites must identify KIR operations".to_owned());
    };
    source_map_operation_site(
        module,
        site.function_ordinal,
        site.block_ordinal,
        operation_ordinal,
    )
}

fn source_map_operation_site(
    module: &AdmittedSimulationModuleV1,
    function_ordinal: u64,
    block_ordinal: u64,
    operation_ordinal: u64,
) -> Result<fe2o3_kir_sim::SimulationDebugSiteV1, String> {
    let function_ordinal = usize::try_from(function_ordinal)
        .map_err(|_| "source map function ordinal does not fit this host".to_owned())?;
    let function = module
        .module()
        .functions
        .get(function_ordinal)
        .ok_or_else(|| "source map function ordinal is unknown".to_owned())?;
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| "source map function has no body".to_owned())?;
    let block_ordinal = usize::try_from(block_ordinal)
        .map_err(|_| "source map block ordinal does not fit this host".to_owned())?;
    let block = body
        .blocks
        .get(block_ordinal)
        .ok_or_else(|| "source map block ordinal is unknown".to_owned())?;
    let operation = u32::try_from(operation_ordinal)
        .map_err(|_| "source map operation ordinal exceeds KIR V7".to_owned())?;
    if block.operations.get(operation as usize).is_none() {
        return Err("source map operation ordinal is unknown".to_owned());
    }
    Ok(fe2o3_kir_sim::SimulationDebugSiteV1 {
        function_ordinal,
        block: block.id,
        operation,
    })
}

fn source_map_span(span: DebugSourceMapSpanV1) -> DebugSourceSpanV1 {
    DebugSourceSpanV1 {
        file: span.file_identity(),
        byte_start: span.byte_start(),
        byte_end: span.byte_end(),
        line: span.line(),
        column: span.column(),
    }
}

fn convert_scope_selector(scope: ExecutionScopeSelectorV1) -> DebugScopeSelectorV1 {
    match scope {
        ExecutionScopeSelectorV1::Dispatch => DebugScopeSelectorV1::Dispatch,
        ExecutionScopeSelectorV1::Workgroup { workgroup } => {
            DebugScopeSelectorV1::Workgroup(workgroup.map(u64::from))
        }
        ExecutionScopeSelectorV1::Wave { workgroup, wave } => DebugScopeSelectorV1::Wave {
            workgroup: workgroup.map(u64::from),
            wave,
        },
        ExecutionScopeSelectorV1::Lane {
            workgroup,
            wave,
            lane,
        } => DebugScopeSelectorV1::Lane {
            workgroup: workgroup.map(u64::from),
            wave,
            lane,
        },
    }
}

fn convert_hit_condition(condition: HitConditionV1) -> Result<DebugHitConditionV1, ConvertErrorV1> {
    match condition.comparison {
        IntegerComparisonV1::Equal => Ok(DebugHitConditionV1::Equal(condition.count)),
        IntegerComparisonV1::GreaterOrEqual => Ok(DebugHitConditionV1::AtLeast(condition.count)),
        IntegerComparisonV1::GreaterThan => condition
            .count
            .checked_add(1)
            .map(DebugHitConditionV1::AtLeast)
            .ok_or(ConvertErrorV1::Invalid("hit condition count overflows")),
        _ => Err(ConvertErrorV1::Unavailable(
            DebugCapabilityNameV1::Breakpoints,
            CapabilityUnavailableReasonV1::NotRepresented,
            "V1 replay hit conditions support equal, greater-than, and greater-or-equal counts",
        )),
    }
}

fn convert_predicate(
    predicate: &PredicateV1,
) -> Result<(DebugPredicateV1, Option<usize>), ConvertErrorV1> {
    match predicate {
        PredicateV1::Compare {
            left,
            comparison,
            right,
        } => {
            if !matches!(
                comparison,
                IntegerComparisonV1::Equal | IntegerComparisonV1::NotEqual
            ) {
                return Err(ConvertErrorV1::Unavailable(
                    DebugCapabilityNameV1::Breakpoints,
                    CapabilityUnavailableReasonV1::NotRepresented,
                    "V1 replay value predicates support exact equality and inequality",
                ));
            }
            let (path, constant) = match (predicate_path(left), predicate_constant(right)) {
                (Ok(path), Ok(constant)) => (path, constant),
                _ => match (predicate_path(right), predicate_constant(left)) {
                    (Ok(path), Ok(constant)) => (path, constant),
                    _ => {
                        return Err(ConvertErrorV1::Unavailable(
                            DebugCapabilityNameV1::Breakpoints,
                            CapabilityUnavailableReasonV1::NotRepresented,
                            "value predicates require one scalar SSA path and one typed constant",
                        ));
                    }
                },
            };
            let value = match comparison {
                IntegerComparisonV1::Equal => DebugPredicateV1::ScalarEquals {
                    frame_depth: path.frame_depth,
                    value: path.value,
                    expected: constant,
                },
                IntegerComparisonV1::NotEqual => DebugPredicateV1::ScalarNotEquals {
                    frame_depth: path.frame_depth,
                    value: path.value,
                    expected: constant,
                },
                _ => unreachable!(),
            };
            Ok((value, Some(path.function_ordinal)))
        }
        PredicateV1::All { predicates } | PredicateV1::Any { predicates } => {
            let mut converted = Vec::with_capacity(predicates.len());
            let mut function = None;
            for child in predicates {
                let (child, child_function) = convert_predicate(child)?;
                function = merge_function(function, child_function)?;
                converted.push(child);
            }
            let predicate = if matches!(predicate, PredicateV1::All { .. }) {
                DebugPredicateV1::And(converted)
            } else {
                DebugPredicateV1::Or(converted)
            };
            Ok((predicate, function))
        }
        PredicateV1::Not { predicate_value } => {
            let (child, function) = convert_predicate(predicate_value)?;
            Ok((DebugPredicateV1::Not(Box::new(child)), function))
        }
    }
}

#[derive(Clone, Copy)]
struct PredicatePathV1 {
    function_ordinal: usize,
    frame_depth: u32,
    value: ValueId,
}

fn predicate_path(operand: &PredicateOperandV1) -> Result<PredicatePathV1, ConvertErrorV1> {
    let PredicateOperandV1::Value { path } = operand else {
        return Err(ConvertErrorV1::Invalid(
            "predicate operand is not a value path",
        ));
    };
    if !path.components.is_empty() {
        return Err(ConvertErrorV1::Unavailable(
            DebugCapabilityNameV1::Breakpoints,
            CapabilityUnavailableReasonV1::NotRepresented,
            "aggregate predicate paths are not represented by scalar SSA replay predicates",
        ));
    }
    let ValueRootV1::Ssa {
        function_ordinal,
        frame,
        value_ordinal,
    } = &path.root
    else {
        return Err(ConvertErrorV1::Unavailable(
            DebugCapabilityNameV1::KirSsaValues,
            CapabilityUnavailableReasonV1::NotRepresented,
            "replay predicates require a KIR SSA value root",
        ));
    };
    Ok(PredicatePathV1 {
        function_ordinal: usize::try_from(*function_ordinal)
            .map_err(|_| ConvertErrorV1::Invalid("predicate function ordinal does not fit host"))?,
        frame_depth: u32::try_from(frame - 1)
            .map_err(|_| ConvertErrorV1::Invalid("predicate frame does not fit host"))?,
        value: ValueId(
            u32::try_from(*value_ordinal)
                .map_err(|_| ConvertErrorV1::Invalid("predicate value ordinal exceeds KIR V7"))?,
        ),
    })
}

fn predicate_constant(operand: &PredicateOperandV1) -> Result<ScalarBitsV1, ConvertErrorV1> {
    match operand {
        PredicateOperandV1::Bool { value } => Ok(ScalarBitsV1::boolean(*value)),
        PredicateOperandV1::Integer {
            signed,
            bits,
            value,
        } => {
            let ty = integer_scalar_type(*signed, *bits).ok_or(ConvertErrorV1::Unavailable(
                DebugCapabilityNameV1::Breakpoints,
                CapabilityUnavailableReasonV1::NotRepresented,
                "predicate integer constants must have a KIR scalar width from 8 through 128 bits",
            ))?;
            let raw = u128::from_str_radix(value.trim_start_matches("0x"), 16).map_err(|_| {
                ConvertErrorV1::Invalid("predicate constant is not bounded hexadecimal")
            })?;
            ScalarBitsV1::new(ty, raw, SimulationTargetV1::amdgpu_64())
                .map_err(|_| ConvertErrorV1::Invalid("predicate constant is outside its type"))
        }
        PredicateOperandV1::Value { .. } => Err(ConvertErrorV1::Invalid(
            "predicate operand is not a constant",
        )),
    }
}

fn merge_function(
    left: Option<usize>,
    right: Option<usize>,
) -> Result<Option<usize>, ConvertErrorV1> {
    match (left, right) {
        (Some(left), Some(right)) if left != right => Err(ConvertErrorV1::Unavailable(
            DebugCapabilityNameV1::Breakpoints,
            CapabilityUnavailableReasonV1::NotRepresented,
            "one replay value predicate cannot span multiple KIR functions",
        )),
        (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

fn integer_scalar_type(signed: bool, bits: u16) -> Option<ScalarType> {
    match (signed, bits) {
        (true, 8) => Some(ScalarType::I8),
        (true, 16) => Some(ScalarType::I16),
        (true, 32) => Some(ScalarType::I32),
        (true, 64) => Some(ScalarType::I64),
        (true, 128) => Some(ScalarType::I128),
        (false, 8) => Some(ScalarType::U8),
        (false, 16) => Some(ScalarType::U16),
        (false, 32) => Some(ScalarType::U32),
        (false, 64) => Some(ScalarType::U64),
        (false, 128) => Some(ScalarType::U128),
        _ => None,
    }
}

fn page_bounds(
    page: PageRequestV1,
    query: OpaqueIdentityV1,
    total: usize,
) -> Result<(usize, usize, Option<PageCursorV1>), &'static str> {
    let (start, limit) = page_window(page, query)?;
    if start > total {
        return Err("page cursor is outside the result set");
    }
    let end = start.saturating_add(limit).min(total);
    let next = page_next(start, end - start, total, query)?;
    Ok((start, end, next))
}

fn page_window(
    page: PageRequestV1,
    query: OpaqueIdentityV1,
) -> Result<(usize, usize), &'static str> {
    let start = match page.cursor {
        Some(cursor) => {
            if cursor.query_identity != query {
                return Err("page cursor does not match this query and session revision");
            }
            usize::try_from(cursor.position).map_err(|_| "page cursor does not fit this host")?
        }
        None => 0,
    };
    Ok((start, usize::from(page.limit)))
}

fn page_next(
    start: usize,
    returned: usize,
    total: usize,
    query: OpaqueIdentityV1,
) -> Result<Option<PageCursorV1>, &'static str> {
    if start > total {
        return Err("page cursor is outside the result set");
    }
    let end = start
        .checked_add(returned)
        .ok_or("page result position overflow")?;
    Ok((end < total).then(|| PageCursorV1 {
        query_identity: query,
        position: u64::try_from(end).unwrap_or(u64::MAX),
    }))
}

fn restore_cursor(session: &mut DebugSessionV1, cursor: Option<usize>) {
    match cursor {
        Some(index) => {
            let _ = session.seek_record_index(index);
        }
        None => {
            let _ = session.seek_entry();
        }
    }
}

fn record_matches_step(
    record: &SimulationDebugRecordV1,
    granularity: StepGranularityV1,
    scope: &DebugScopeSelectorV1,
    width: DebugWaveWidthV1,
    baseline: Option<fe2o3_kir_debugger::DebugHierarchyV1>,
) -> bool {
    if !scope.matches(record.invocation, width) {
        return false;
    }
    let hierarchy = hierarchy_for_invocation_v1(record.invocation, width);
    match granularity {
        StepGranularityV1::Event => true,
        StepGranularityV1::Operation => {
            matches!(record.kind, SimulationDebugRecordKindV1::Checkpoint { .. })
        }
        StepGranularityV1::MemoryAccess => {
            matches!(record.kind, SimulationDebugRecordKindV1::Memory { .. })
        }
        StepGranularityV1::BarrierPhase => {
            matches!(
                record.kind,
                SimulationDebugRecordKindV1::WorkgroupBarrier { .. }
            )
        }
        StepGranularityV1::Lane => baseline.is_none_or(|baseline| {
            (baseline.workgroup, baseline.wave, baseline.lane)
                != (hierarchy.workgroup, hierarchy.wave, hierarchy.lane)
        }),
        StepGranularityV1::Wave => baseline.is_none_or(|baseline| {
            (baseline.workgroup, baseline.wave) != (hierarchy.workgroup, hierarchy.wave)
        }),
        StepGranularityV1::Workgroup => {
            baseline.is_none_or(|baseline| baseline.workgroup != hierarchy.workgroup)
        }
        StepGranularityV1::Over | StepGranularityV1::Out | StepGranularityV1::Source => false,
    }
}

fn navigation_stop(navigation: DebugNavigationV1, failed_execution: bool) -> StopViewV1 {
    match navigation {
        DebugNavigationV1::Stopped(stop) => stop_view(stop),
        DebugNavigationV1::Beginning => StopViewV1 {
            reason: StopReasonV1::Entry,
            breakpoint_id: None,
            watchpoint_id: None,
            outcome: ExecutionOutcomeV1::Active,
            exact: true,
        },
        DebugNavigationV1::End => StopViewV1 {
            reason: StopReasonV1::Completed,
            breakpoint_id: None,
            watchpoint_id: None,
            outcome: if failed_execution {
                ExecutionOutcomeV1::Failed
            } else {
                ExecutionOutcomeV1::Completed
            },
            exact: true,
        },
        DebugNavigationV1::BudgetExhausted(_) => StopViewV1 {
            reason: StopReasonV1::ResourceExhaustion,
            breakpoint_id: None,
            watchpoint_id: None,
            outcome: ExecutionOutcomeV1::Active,
            exact: false,
        },
        DebugNavigationV1::TranscriptTruncated(_) => StopViewV1 {
            reason: StopReasonV1::ResourceExhaustion,
            breakpoint_id: None,
            watchpoint_id: None,
            outcome: ExecutionOutcomeV1::Active,
            exact: false,
        },
        DebugNavigationV1::Unavailable(_) => StopViewV1 {
            reason: StopReasonV1::ResourceExhaustion,
            breakpoint_id: None,
            watchpoint_id: None,
            outcome: ExecutionOutcomeV1::Active,
            exact: false,
        },
    }
}

fn stop_view(stop: DebugStopV1) -> StopViewV1 {
    let (reason, breakpoint_id, watchpoint_id, outcome) = match stop.reason {
        DebugStopReasonV1::Step => (StopReasonV1::Step, None, None, ExecutionOutcomeV1::Active),
        DebugStopReasonV1::Breakpoint(id) => (
            StopReasonV1::Breakpoint,
            Some(id),
            None,
            ExecutionOutcomeV1::Active,
        ),
        DebugStopReasonV1::Watchpoint(id) => (
            StopReasonV1::Watchpoint,
            None,
            Some(id),
            ExecutionOutcomeV1::Active,
        ),
        DebugStopReasonV1::Fault => (StopReasonV1::Fault, None, None, ExecutionOutcomeV1::Failed),
    };
    StopViewV1 {
        reason,
        breakpoint_id,
        watchpoint_id,
        outcome,
        exact: true,
    }
}

fn workgroup_u32(workgroup: [u64; 3]) -> Option<[u32; 3]> {
    Some([
        u32::try_from(workgroup[0]).ok()?,
        u32::try_from(workgroup[1]).ok()?,
        u32::try_from(workgroup[2]).ok()?,
    ])
}

fn protocol_scope_for_invocation(
    invocation: SimulationInvocationV1,
    width: DebugWaveWidthV1,
) -> Option<ExecutionScopeV1> {
    let hierarchy = hierarchy_for_invocation_v1(invocation, width);
    Some(ExecutionScopeV1::Lane {
        workgroup: workgroup_u32(hierarchy.workgroup)?,
        wave: hierarchy.wave,
        lane: hierarchy.lane,
        logical_workitem: hierarchy.global,
        active_mask: hierarchy.active_mask,
        wave_width: width.lanes(),
        interpretation: WaveInterpretationV1::LogicalVisualization,
    })
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ScopeKeyV1 {
    Dispatch,
    Workgroup([u32; 3]),
    Wave([u32; 3], u32),
    Lane([u32; 3], u32, u16),
}

impl ScopeKeyV1 {
    const fn from_scope(scope: ExecutionScopeV1) -> Self {
        match scope {
            ExecutionScopeV1::Dispatch => Self::Dispatch,
            ExecutionScopeV1::Workgroup { workgroup } => Self::Workgroup(workgroup),
            ExecutionScopeV1::Wave {
                workgroup, wave, ..
            } => Self::Wave(workgroup, wave),
            ExecutionScopeV1::Lane {
                workgroup,
                wave,
                lane,
                ..
            } => Self::Lane(workgroup, wave, lane),
        }
    }

    fn matches_invocation(
        self,
        invocation: SimulationInvocationV1,
        width: DebugWaveWidthV1,
    ) -> bool {
        if self == Self::Dispatch {
            return true;
        }
        let hierarchy = hierarchy_for_invocation_v1(invocation, width);
        let Some(workgroup) = workgroup_u32(hierarchy.workgroup) else {
            return false;
        };
        match self {
            Self::Dispatch => true,
            Self::Workgroup(expected) => workgroup == expected,
            Self::Wave(expected, wave) => workgroup == expected && hierarchy.wave == wave,
            Self::Lane(expected, wave, lane) => {
                workgroup == expected && hierarchy.wave == wave && hierarchy.lane == lane
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ScopeObservationV1 {
    first: Option<usize>,
    last: Option<usize>,
    contains_cursor: bool,
    representative: Option<SimulationInvocationV1>,
    barrier_waiting: u32,
}

fn update_scope_observation(
    observation: &mut ScopeObservationV1,
    record_index: usize,
    cursor: usize,
) {
    observation.first.get_or_insert(record_index);
    observation.last = Some(record_index);
    observation.contains_cursor |= record_index == cursor;
}

fn active_workgroup_participants(invocation: SimulationInvocationV1) -> u32 {
    invocation
        .workgroup
        .into_iter()
        .zip(invocation.workgroup_size)
        .zip(invocation.launch_extent)
        .try_fold(1_u64, |product, ((group, size), extent)| {
            let start = group.checked_mul(u64::from(size))?;
            let live = extent.saturating_sub(start).min(u64::from(size));
            product.checked_mul(live)
        })
        .and_then(|participants| u32::try_from(participants).ok())
        .unwrap_or(0)
}

fn scope_barrier_population(scope: ExecutionScopeV1, observation: ScopeObservationV1) -> u32 {
    match scope {
        ExecutionScopeV1::Dispatch | ExecutionScopeV1::Workgroup { .. } => observation
            .representative
            .map(active_workgroup_participants)
            .unwrap_or(0),
        ExecutionScopeV1::Wave { active_mask, .. } => active_mask.count_ones(),
        ExecutionScopeV1::Lane { .. } => 1,
    }
}

fn scope_states_for_records(
    records: &[SimulationDebugRecordV1],
    terminal_fault: Option<&DebugTerminalFaultV1>,
    cursor: Option<usize>,
    scopes: &[ExecutionScopeV1],
    wave_width: DebugWaveWidthV1,
) -> Vec<ScopeStateV1> {
    let Some(cursor) = cursor else {
        return vec![ScopeStateV1::NotStarted; scopes.len()];
    };
    let mut observations = vec![ScopeObservationV1::default(); scopes.len()];
    let mut indices = BTreeMap::new();
    let mut group_indices: BTreeMap<[u32; 3], Vec<usize>> = BTreeMap::new();
    for (index, scope) in scopes.iter().copied().enumerate() {
        let key = ScopeKeyV1::from_scope(scope);
        indices.insert(key, index);
        let group = match key {
            ScopeKeyV1::Workgroup(group)
            | ScopeKeyV1::Wave(group, _)
            | ScopeKeyV1::Lane(group, _, _) => Some(group),
            ScopeKeyV1::Dispatch => None,
        };
        if let Some(group) = group {
            group_indices.entry(group).or_default().push(index);
        }
    }
    for (record_index, record) in records.iter().enumerate() {
        let hierarchy = hierarchy_for_invocation_v1(record.invocation, wave_width);
        let Some(workgroup) = workgroup_u32(hierarchy.workgroup) else {
            if let Some(index) = indices.get(&ScopeKeyV1::Dispatch).copied() {
                let observation = &mut observations[index];
                update_scope_observation(observation, record_index, cursor);
                if record_index <= cursor {
                    observation.representative = Some(record.invocation);
                }
            }
            continue;
        };
        for key in [
            ScopeKeyV1::Dispatch,
            ScopeKeyV1::Workgroup(workgroup),
            ScopeKeyV1::Wave(workgroup, hierarchy.wave),
            ScopeKeyV1::Lane(workgroup, hierarchy.wave, hierarchy.lane),
        ] {
            if let Some(index) = indices.get(&key).copied() {
                let observation = &mut observations[index];
                update_scope_observation(observation, record_index, cursor);
                if record_index <= cursor {
                    observation.representative = Some(record.invocation);
                }
            }
        }
        if record_index > cursor {
            continue;
        }
        match record.kind {
            SimulationDebugRecordKindV1::WorkgroupBarrier {
                action: SimulationDebugBarrierActionV1::Arrive,
                ..
            } => {
                for key in [
                    ScopeKeyV1::Dispatch,
                    ScopeKeyV1::Workgroup(workgroup),
                    ScopeKeyV1::Wave(workgroup, hierarchy.wave),
                    ScopeKeyV1::Lane(workgroup, hierarchy.wave, hierarchy.lane),
                ] {
                    if let Some(index) = indices.get(&key).copied() {
                        observations[index].barrier_waiting =
                            observations[index].barrier_waiting.saturating_add(1);
                    }
                }
            }
            SimulationDebugRecordKindV1::WorkgroupBarrier {
                action: SimulationDebugBarrierActionV1::Release,
                ..
            } => {
                if let Some(index) = indices.get(&ScopeKeyV1::Dispatch).copied() {
                    observations[index].barrier_waiting = 0;
                }
                if let Some(group) = group_indices.get(&workgroup) {
                    for index in group.iter().copied() {
                        observations[index].barrier_waiting = 0;
                        observations[index].last = Some(record_index);
                    }
                }
            }
            _ => {}
        }
    }
    let terminal_fault = (cursor == records.len())
        .then_some(terminal_fault)
        .flatten();
    scopes
        .iter()
        .copied()
        .zip(observations)
        .map(|(scope, observed)| {
            let key = ScopeKeyV1::from_scope(scope);
            if terminal_fault.is_some_and(|fault| {
                key == ScopeKeyV1::Dispatch
                    || fault
                        .invocation
                        .is_some_and(|invocation| key.matches_invocation(invocation, wave_width))
            }) {
                return ScopeStateV1::Failed;
            }
            let (Some(first), Some(last)) = (observed.first, observed.last) else {
                return ScopeStateV1::Unavailable;
            };
            if cursor < first {
                return ScopeStateV1::NotStarted;
            }
            if cursor > last || cursor == records.len() {
                return ScopeStateV1::Completed;
            }
            let population = scope_barrier_population(scope, observed);
            if population != 0 && observed.barrier_waiting >= population {
                return ScopeStateV1::BarrierBlocked;
            }
            if observed.barrier_waiting != 0 {
                return ScopeStateV1::Runnable;
            }
            if observed.contains_cursor {
                ScopeStateV1::Running
            } else {
                ScopeStateV1::Runnable
            }
        })
        .collect()
}

fn selector_requests_unsupported_root(
    selector: &ValueSelectorV1,
    requested: ValueRootClassV1,
) -> bool {
    match selector {
        ValueSelectorV1::All => false,
        ValueSelectorV1::Roots { roots } => roots.contains(&requested),
        ValueSelectorV1::Paths { paths } => paths.iter().any(|path| {
            matches!(
                (&path.root, requested),
                (ValueRootV1::Register { .. }, ValueRootClassV1::Register)
                    | (
                        ValueRootV1::SourceVariable { .. },
                        ValueRootClassV1::SourceVariable
                    )
            )
        }),
    }
}

fn value_for_path(path: ValuePathV1, frames: &[SimulationDebugFrameV1]) -> DebugValueV1 {
    let unavailable = |path| DebugValueV1 {
        path,
        availability: ValueAvailabilityV1::Unavailable {
            reason: ValueUnavailableReasonV1::NotInScope,
        },
    };
    if !path.components.is_empty() {
        return DebugValueV1 {
            path,
            availability: ValueAvailabilityV1::Unavailable {
                reason: ValueUnavailableReasonV1::NotRepresented,
            },
        };
    }
    let non_ssa_reason = match &path.root {
        ValueRootV1::SourceVariable { .. } => ValueUnavailableReasonV1::RequiresAuthenticatedMap,
        ValueRootV1::Register { .. } => ValueUnavailableReasonV1::UnsupportedByBackend,
        ValueRootV1::Argument { .. } => ValueUnavailableReasonV1::NotRepresented,
        ValueRootV1::Ssa { .. } => ValueUnavailableReasonV1::NotInScope,
    };
    let ValueRootV1::Ssa {
        function_ordinal,
        frame,
        value_ordinal,
    } = path.root
    else {
        return DebugValueV1 {
            path,
            availability: ValueAvailabilityV1::Unavailable {
                reason: non_ssa_reason,
            },
        };
    };
    let (Ok(function), Ok(depth), Ok(value)) = (
        usize::try_from(function_ordinal),
        u32::try_from(frame - 1),
        u32::try_from(value_ordinal),
    ) else {
        return unavailable(path);
    };
    let Some(frame) = frames
        .iter()
        .find(|candidate| candidate.function_ordinal == function && candidate.depth == depth)
    else {
        return unavailable(path);
    };
    let SimulationDebugCollectionV1::Captured(bindings) = &frame.values else {
        return DebugValueV1 {
            path,
            availability: ValueAvailabilityV1::Unavailable {
                reason: ValueUnavailableReasonV1::Truncated,
            },
        };
    };
    let Some(binding) = bindings
        .iter()
        .find(|binding| binding.value == ValueId(value))
    else {
        return unavailable(path);
    };
    DebugValueV1 {
        path,
        availability: availability_for_observed(&binding.observed),
    }
}

fn value_for_binding(
    frame: &SimulationDebugFrameV1,
    binding: &SimulationDebugBindingV1,
) -> DebugValueV1 {
    DebugValueV1 {
        path: ValuePathV1 {
            root: ValueRootV1::Ssa {
                function_ordinal: u64::try_from(frame.function_ordinal).unwrap_or(u64::MAX),
                frame: u64::from(frame.depth).saturating_add(1),
                value_ordinal: u64::from(binding.value.0),
            },
            components: Vec::new(),
        },
        availability: availability_for_observed(&binding.observed),
    }
}

fn availability_for_observed(observed: &SimulationDebugValueV1) -> ValueAvailabilityV1 {
    match observed {
        SimulationDebugValueV1::Scalar(value) => {
            let (value_type, width) = protocol_scalar_type(*value);
            ValueAvailabilityV1::Captured {
                value_type,
                value: CapturedValueV1::Bits {
                    bits: fixed_width_bits(value.bits(), width),
                },
                provenance: ValueProvenanceV1::SimulatedObservation,
            }
        }
        SimulationDebugValueV1::Pointer {
            allocation,
            byte_offset,
            address_space,
            ..
        }
        | SimulationDebugValueV1::Slice {
            allocation,
            byte_offset,
            address_space,
            ..
        } => ValueAvailabilityV1::Captured {
            value_type: DebugValueTypeV1::Pointer {
                address_space: protocol_address_space(*address_space),
            },
            value: CapturedValueV1::AllocationRelativePointer {
                allocation: AllocationIdentityV1 {
                    ordinal: *allocation,
                    generation: 0,
                },
                byte_offset: u64::try_from(*byte_offset).unwrap_or(u64::MAX),
            },
            provenance: ValueProvenanceV1::SimulatedObservation,
        },
    }
}

fn source_variable_binding_at_v2(
    variable: &AdmittedSourceVariableV2,
    block: fe2o3_kernel_ir::BlockId,
    next_operation: u32,
) -> Option<(u64, DebugSourceVariableBindingV2)> {
    let end = variable
        .locations
        .partition_point(|location| location.block <= block);
    let start = variable.locations[..end].partition_point(|location| location.block < block);
    variable.locations[start..end]
        .iter()
        .rev()
        .find(|location| location.next_operation <= next_operation)
        .map(|location| (location.generation, location.binding))
}

fn source_variable_effective_binding_at_v2(
    variable: &AdmittedSourceVariableV2,
    block: fe2o3_kernel_ir::BlockId,
    next_operation: u32,
) -> (u64, DebugSourceVariableBindingV2) {
    source_variable_binding_at_v2(variable, block, next_operation)
        .or_else(|| {
            variable.function_binding.map(|(generation, value)| {
                (
                    generation,
                    DebugSourceVariableBindingV2::Captured {
                        value_ordinal: u64::from(value.0),
                    },
                )
            })
        })
        .unwrap_or((
            0,
            match variable.fallback {
                DebugSourceVariableFallbackV2::NotInScope => {
                    DebugSourceVariableBindingV2::NotInScope
                }
                DebugSourceVariableFallbackV2::OptimizedOut => {
                    DebugSourceVariableBindingV2::OptimizedOut
                }
                DebugSourceVariableFallbackV2::Unrepresented => {
                    DebugSourceVariableBindingV2::Unrepresented
                }
                DebugSourceVariableFallbackV2::NotCaptured => {
                    DebugSourceVariableBindingV2::NotCaptured
                }
            },
        ))
}

fn source_variable_scopes_form_chain_v2(
    index: &AdmittedSourceVariablesV2,
    candidates: &[usize],
    deepest_scope: OpaqueIdentityV1,
) -> Result<bool, ()> {
    let maximum_depth = candidates
        .iter()
        .map(|candidate| index.variables[*candidate].scope_depth)
        .max()
        .unwrap_or(0);
    let capacity = usize::try_from(maximum_depth)
        .ok()
        .and_then(|depth| depth.checked_add(1))
        .ok_or(())?;
    let mut ancestors = Vec::new();
    ancestors.try_reserve_exact(capacity).map_err(|_| ())?;
    let mut current = Some(deepest_scope);
    while let Some(scope) = current {
        ancestors.push(scope);
        current = *index.scope_parents.get(&scope).ok_or(())?;
        if ancestors.len() > capacity {
            return Ok(false);
        }
    }
    ancestors.sort_unstable();
    Ok(candidates.iter().all(|candidate| {
        ancestors
            .binary_search(&index.variables[*candidate].scope_identity)
            .is_ok()
    }))
}

fn source_variable_value_v2(
    variable: &AdmittedSourceVariableV2,
    frame: &SimulationDebugFrameV1,
    next_operation: u32,
    force_ambiguous: bool,
) -> SourceVariableValueV2 {
    let (generation, binding) =
        source_variable_effective_binding_at_v2(variable, frame.block, next_operation);
    let availability =
        if force_ambiguous || matches!(binding, DebugSourceVariableBindingV2::Ambiguous) {
            SourceVariableValueAvailabilityV2::Ambiguous
        } else {
            let value = match binding {
                DebugSourceVariableBindingV2::NotInScope => ValueAvailabilityV1::Unavailable {
                    reason: ValueUnavailableReasonV1::NotInScope,
                },
                DebugSourceVariableBindingV2::Uninitialized => ValueAvailabilityV1::Unavailable {
                    reason: ValueUnavailableReasonV1::Uninitialized,
                },
                DebugSourceVariableBindingV2::NotCaptured => ValueAvailabilityV1::Unavailable {
                    reason: ValueUnavailableReasonV1::NotCaptured,
                },
                DebugSourceVariableBindingV2::OptimizedOut => ValueAvailabilityV1::Unavailable {
                    reason: ValueUnavailableReasonV1::OptimizedOut,
                },
                DebugSourceVariableBindingV2::Unrepresented => ValueAvailabilityV1::Unavailable {
                    reason: ValueUnavailableReasonV1::NotRepresented,
                },
                DebugSourceVariableBindingV2::Captured { value_ordinal } => {
                    let value = u32::try_from(value_ordinal).ok().map(ValueId);
                    match (&frame.values, value) {
                        (SimulationDebugCollectionV1::Captured(bindings), Some(value)) => bindings
                            .iter()
                            .find(|binding| binding.value == value)
                            .map(|binding| availability_for_observed(&binding.observed))
                            .unwrap_or(ValueAvailabilityV1::Unavailable {
                                reason: ValueUnavailableReasonV1::NotLive,
                            }),
                        (SimulationDebugCollectionV1::Unavailable { .. }, _) => {
                            ValueAvailabilityV1::Unavailable {
                                reason: ValueUnavailableReasonV1::Truncated,
                            }
                        }
                        (_, None) => ValueAvailabilityV1::Unavailable {
                            reason: ValueUnavailableReasonV1::NotRepresented,
                        },
                    }
                }
                DebugSourceVariableBindingV2::Ambiguous => unreachable!("handled above"),
            };
            SourceVariableValueAvailabilityV2::Value { value }
        };
    SourceVariableValueV2 {
        variable_identity: variable.identity,
        name: variable.name.clone(),
        function_ordinal: variable.function_ordinal as u64,
        scope_identity: variable.scope_identity,
        scope_depth: variable.scope_depth,
        generation,
        availability,
    }
}

fn protocol_scalar_type(value: ScalarBitsV1) -> (DebugValueTypeV1, u16) {
    match value.ty() {
        ScalarType::Bool => (DebugValueTypeV1::Bool, 1),
        ScalarType::Index => (DebugValueTypeV1::Index { bits: 64 }, 64),
        ty if ty.is_float() => {
            let width = ty.bit_width().unwrap_or(64);
            (DebugValueTypeV1::Float { bits: width }, width)
        }
        ty => {
            let width = ty.bit_width().unwrap_or(128);
            (
                DebugValueTypeV1::Integer {
                    signed: ty.is_signed_integer(),
                    bits: width,
                },
                width,
            )
        }
    }
}

fn fixed_width_bits(value: u128, bits: u16) -> String {
    format!("0x{value:0width$x}", width = usize::from(bits).div_ceil(4))
}

fn current_ssa_values(session: &DebugSessionV1, maximum: usize) -> Vec<DebugValueV1> {
    let DebugInspectionV1::Available(frames) = session.stack() else {
        return Vec::new();
    };
    frames
        .iter()
        .flat_map(|frame| {
            let SimulationDebugCollectionV1::Captured(bindings) = &frame.values else {
                return Vec::new().into_iter();
            };
            bindings
                .iter()
                .map(|binding| value_for_binding(frame, binding))
                .collect::<Vec<_>>()
                .into_iter()
        })
        .take(maximum)
        .collect()
}

const fn protocol_address_space(address_space: AddressSpace) -> AddressSpaceV1 {
    match address_space {
        AddressSpace::Private => AddressSpaceV1::Private,
        AddressSpace::Workgroup => AddressSpaceV1::Workgroup,
        AddressSpace::Global => AddressSpaceV1::Global,
        AddressSpace::Constant => AddressSpaceV1::Constant,
        AddressSpace::Generic => AddressSpaceV1::Generic,
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(2 + bytes.len().saturating_mul(2));
    output.push_str("0x");
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn initialization_bits(initialized: &[bool]) -> String {
    let mut bytes = vec![0_u8; initialized.len().div_ceil(8)];
    for (index, initialized) in initialized.iter().copied().enumerate() {
        if initialized {
            bytes[index / 8] |= 1 << (index % 8);
        }
    }
    hex_bytes(&bytes)
}

const fn inspection_unavailable_value_reason(
    reason: DebugInspectionUnavailableV1,
) -> ValueUnavailableReasonV1 {
    match reason {
        DebugInspectionUnavailableV1::SourceNotBound => {
            ValueUnavailableReasonV1::RequiresAuthenticatedMap
        }
        DebugInspectionUnavailableV1::Stack(_)
        | DebugInspectionUnavailableV1::Values(_)
        | DebugInspectionUnavailableV1::Memory(_) => ValueUnavailableReasonV1::Truncated,
        DebugInspectionUnavailableV1::UnknownValue | DebugInspectionUnavailableV1::UnknownFrame => {
            ValueUnavailableReasonV1::NotInScope
        }
        DebugInspectionUnavailableV1::NoCurrentRecord
        | DebugInspectionUnavailableV1::NotCheckpoint => ValueUnavailableReasonV1::NotCaptured,
        DebugInspectionUnavailableV1::UnknownAllocation
        | DebugInspectionUnavailableV1::RangeOverflow
        | DebugInspectionUnavailableV1::OutOfBounds
        | DebugInspectionUnavailableV1::NonScalarValue
        | DebugInspectionUnavailableV1::SourceUnavailable => {
            ValueUnavailableReasonV1::NotRepresented
        }
    }
}

const fn transcript_truncation_reason(
    reason: DebugTranscriptTruncationV1,
) -> CaptureTruncationReasonV1 {
    match reason {
        DebugTranscriptTruncationV1::RecordLimit => CaptureTruncationReasonV1::EventLimit,
        DebugTranscriptTruncationV1::ValueLimit | DebugTranscriptTruncationV1::MemoryByteLimit => {
            CaptureTruncationReasonV1::ResidentLimit
        }
        DebugTranscriptTruncationV1::AllocationFailure => {
            CaptureTruncationReasonV1::ProducerFailure
        }
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct TraceHeaderV1 {
    schema: &'static str,
    configuration_identity: OpaqueIdentityV1,
    backend: DebugBackendV1,
    simulated: bool,
    hardware_observed: bool,
    performance_prediction: bool,
    wave_interpretation: WaveInterpretationV1,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapErrorV1<'a> {
    schema: &'static str,
    status: &'static str,
    stage: &'a str,
    code: &'a str,
    message: &'a str,
}

pub fn main() -> ExitCode {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|value| value == OsStr::new("live-rocgdb-kfd-v4"))
    {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            return live_rocgdb_kfd_v4::run(arguments);
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            write_bootstrap_error(
                "arguments",
                "native_rocgdb_kfd_v4_unavailable",
                "native ROCgdb/KFD V4 correlation requires Linux x86_64",
            );
            return ExitCode::FAILURE;
        }
    }
    if arguments
        .first()
        .is_some_and(|value| value == OsStr::new("live-rocgdb"))
    {
        #[cfg(target_os = "linux")]
        {
            return live_rocgdb_v3::run(arguments);
        }
        #[cfg(not(target_os = "linux"))]
        {
            write_bootstrap_error(
                "arguments",
                "live_rocgdb_debugger_unavailable",
                "live ROCgdb debugging requires Linux",
            );
            return ExitCode::FAILURE;
        }
    }
    if arguments
        .first()
        .is_some_and(|value| value == OsStr::new("live-kfd"))
    {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            let options = match parse_live_kfd_options_v3(arguments) {
                Ok(options) => options,
                Err(message) => {
                    write_bootstrap_error("arguments", "invalid_command_line", &message);
                    return ExitCode::FAILURE;
                }
            };
            let plan = match live_kfd_v3::LiveKfdSemanticSessionPlanV3::try_new(
                options.bundle_v2,
                options.request,
                Some(options.hsaco),
                &options.program,
                live_kfd_v3::LiveKfdBindingLimitsV3::default(),
            ) {
                Ok(plan) => plan,
                Err(error) => {
                    write_bootstrap_error("binding", "live_kfd_plan_rejected", error.message());
                    return ExitCode::FAILURE;
                }
            };
            let admitted = match live_kfd_v3::admit_live_kfd_semantic_session_v3(plan) {
                Ok(admitted) => admitted,
                Err(error) => {
                    write_bootstrap_error("binding", "live_kfd_inputs_rejected", error.message());
                    return ExitCode::FAILURE;
                }
            };
            let protocol_binding = live_gpu_artifact_binding_v3(&admitted, options.wave_width);
            return hardware_linux_v3::run(admitted, options.program_arguments, protocol_binding);
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            write_bootstrap_error(
                "arguments",
                "live_kfd_debugger_unavailable",
                "live KFD debugging requires Linux x86_64 and the KFD UAPI",
            );
            return ExitCode::FAILURE;
        }
    }
    if arguments
        .first()
        .is_some_and(|value| value == OsStr::new("hardware"))
    {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            return hardware_linux_v2::run(arguments.into_iter().skip(1));
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            write_bootstrap_error(
                "arguments",
                "hardware_debugger_unavailable",
                "hardware debugging requires Linux x86_64 and the KFD UAPI",
            );
            return ExitCode::FAILURE;
        }
    }
    let options = match parse_options(arguments.into_iter()) {
        Ok(options) => options,
        Err(message) => {
            write_bootstrap_error("arguments", "invalid_command_line", &message);
            return ExitCode::FAILURE;
        }
    };
    let (admitted, bundle, bundle_v2) = match (&options.program, &options.request) {
        (ProgramInputV1::KirV7(path), RequestInputV1::Path(request)) => {
            match load_debug_simulation_input_v1(path, request) {
                Ok(input) => (input, None, None),
                Err(error) => {
                    write_input_error(&error);
                    return ExitCode::FAILURE;
                }
            }
        }
        (ProgramInputV1::SealedKirV7Fd(kir_fd), RequestInputV1::SealedFd(request_fd)) => {
            #[cfg(target_os = "linux")]
            {
                let (kir, request) = match read_sealed_debug_input_pair_v1(
                    *kir_fd,
                    *request_fd,
                    MAX_SEALED_DEBUG_INPUT_BYTES_V1,
                ) {
                    Ok(inputs) => inputs,
                    Err(message) => {
                        write_bootstrap_error("input", "sealed_input_rejected", &message);
                        return ExitCode::FAILURE;
                    }
                };
                match load_debug_simulation_input_bytes_v1(&kir, &request) {
                    Ok(input) => (input, None, None),
                    Err(error) => {
                        write_input_error(&error);
                        return ExitCode::FAILURE;
                    }
                }
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = (kir_fd, request_fd);
                write_bootstrap_error(
                    "input",
                    "sealed_input_unavailable",
                    "sealed debugger input descriptors require Linux",
                );
                return ExitCode::FAILURE;
            }
        }
        (ProgramInputV1::Bundle(path), RequestInputV1::Path(request)) => {
            match load_debug_simulation_bundle_v1(path, request) {
                Ok(admitted) => {
                    let (input, bundle) = admitted.into_parts();
                    (input, Some(bundle), None)
                }
                Err(error) => {
                    write_input_error(&error);
                    return ExitCode::FAILURE;
                }
            }
        }
        (ProgramInputV1::BundleV2(path), RequestInputV1::Path(request)) => {
            match load_debug_simulation_bundle_v2(path, request) {
                Ok(admitted) => {
                    let (input, bundle) = admitted.into_parts();
                    (input, None, Some(bundle))
                }
                Err(error) => {
                    write_input_error(&error);
                    return ExitCode::FAILURE;
                }
            }
        }
        _ => unreachable!("argument parser pairs program and request input custody"),
    };
    let (source_map, caller_source_map_v2) =
        match (&options.source_map, options.source_bundle_subject) {
            (Some(path), Some(expected_subject)) => {
                let bytes = match load_debug_sidecar_v1(path, MAX_SOURCE_MAP_BYTES_V1) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        write_input_error(&error);
                        return ExitCode::FAILURE;
                    }
                };
                let configuration = configuration_identity_for_input(&admitted, options.wave_width);
                let admitted_map = if DebugSourceMapDocumentV2::from_canonical_json_bytes(&bytes)
                    .is_ok()
                {
                    admit_caller_source_map_v2(&bytes, &admitted, configuration, expected_subject)
                        .map(|map| (None, Some(map)))
                } else {
                    admit_source_map_v1(&bytes, &admitted, configuration, expected_subject)
                        .map(|map| (Some(map), None))
                };
                match admitted_map {
                    Ok(maps) => maps,
                    Err(message) => {
                        write_bootstrap_error("source_map", "source_map_rejected", &message);
                        return ExitCode::FAILURE;
                    }
                }
            }
            (None, None) => (None, None),
            _ => unreachable!("argument parser requires source-map options as a pair"),
        };
    let replay_schedule = match options.replay_schedule.as_ref() {
        Some(path) => match load_debug_simulation_schedule_v1(path, &admitted) {
            Ok(schedule) => Some(schedule),
            Err(error) => {
                write_input_error(&error);
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };
    if let Some(bundle) = bundle_v2.as_ref() {
        let subject = nonzero_identity(*bundle.subject_identity());
        let map_identity = nonzero_identity(*bundle.debug_map_identity());
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut reader = BufReader::new(stdin.lock());
        let mut writer = BufWriter::new(stdout.lock());
        return match run_admitted_jsonl_with_bundle_source_map_v2(
            admitted,
            options.wave_width,
            bundle.debug_map(),
            subject,
            map_identity,
            replay_schedule.as_ref().map(|schedule| schedule.record()),
            &mut reader,
            &mut writer,
        ) {
            Ok(()) => ExitCode::SUCCESS,
            Err(CompilerBundleDebugRunErrorV1::SourceMap(message)) => {
                write_bootstrap_error("source_map", "bundle_source_map_v2_rejected", &message);
                ExitCode::FAILURE
            }
            Err(CompilerBundleDebugRunErrorV1::Backend(message)) => {
                write_bootstrap_error("backend", "simulation_capture_failed", &message);
                ExitCode::FAILURE
            }
            Err(CompilerBundleDebugRunErrorV1::ProtocolStream(message)) => {
                write_bootstrap_error("output", "protocol_stream_failed", &message);
                ExitCode::FAILURE
            }
        };
    }
    if let Some(bundle) = bundle.as_ref()
        && let Some(map) = bundle.debug_map()
    {
        let subject = OpaqueIdentityV1::new(*bundle.subject_identity())
            .expect("a verified simulation bundle subject is nonzero");
        let map_identity = OpaqueIdentityV1::new(
            bundle
                .debug_map_identity()
                .expect("a verified present debug map has an identity"),
        )
        .expect("a verified present debug map identity is nonzero");
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut reader = BufReader::new(stdin.lock());
        let mut writer = BufWriter::new(stdout.lock());
        return match run_admitted_jsonl_with_compiler_source_map_and_schedule_v1(
            admitted,
            options.wave_width,
            map,
            subject,
            map_identity,
            replay_schedule.as_ref().map(|schedule| schedule.record()),
            &mut reader,
            &mut writer,
        ) {
            Ok(()) => ExitCode::SUCCESS,
            Err(CompilerBundleDebugRunErrorV1::SourceMap(message)) => {
                write_bootstrap_error("source_map", "bundle_source_map_rejected", &message);
                ExitCode::FAILURE
            }
            Err(CompilerBundleDebugRunErrorV1::Backend(message)) => {
                write_bootstrap_error("backend", "simulation_capture_failed", &message);
                ExitCode::FAILURE
            }
            Err(CompilerBundleDebugRunErrorV1::ProtocolStream(message)) => {
                write_bootstrap_error("output", "protocol_stream_failed", &message);
                ExitCode::FAILURE
            }
        };
    }
    let backend = match caller_source_map_v2 {
        Some(source_map_v2) => SimulatorBackendV1::new_with_source_map_v2_and_schedule(
            admitted,
            options.wave_width,
            source_map_v2,
            replay_schedule.as_ref().map(|schedule| schedule.record()),
        ),
        None => SimulatorBackendV1::new_with_source_map_and_schedule(
            admitted,
            options.wave_width,
            source_map,
            replay_schedule.as_ref().map(|schedule| schedule.record()),
        ),
    };
    let backend = match backend {
        Ok(backend) => backend,
        Err(message) => {
            write_bootstrap_error("backend", "simulation_capture_failed", &message);
            return ExitCode::FAILURE;
        }
    };
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());
    match run_jsonl_v1(backend, &mut reader, &mut writer) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            write_bootstrap_error("output", "protocol_stream_failed", &message);
            ExitCode::FAILURE
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn parse_live_kfd_options_v3(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<LiveKfdOptionsV3, String> {
    let mut arguments = arguments.into_iter();
    if arguments.next().as_deref() != Some(OsStr::new("live-kfd")) {
        return Err(USAGE.to_owned());
    }
    let mut bundle_v2 = None;
    let mut request = None;
    let mut hsaco = None;
    let mut protocol_seen = false;
    let mut wave_width = DebugWaveWidthV1::Wave64;
    let mut program = None;
    let mut program_arguments = Vec::new();
    while let Some(option) = arguments.next() {
        if option == OsStr::new("--") {
            program = arguments.next().map(PathBuf::from);
            program_arguments.extend(arguments);
            break;
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("option {option:?} requires a value; {USAGE}"))?;
        if option == OsStr::new("--bundle-v2") {
            set_once(&mut bundle_v2, PathBuf::from(value), "--bundle-v2")?;
        } else if option == OsStr::new("--request") {
            set_once(&mut request, PathBuf::from(value), "--request")?;
        } else if option == OsStr::new("--hsaco") {
            set_once(&mut hsaco, PathBuf::from(value), "--hsaco")?;
        } else if option == OsStr::new("--protocol") {
            if protocol_seen || value != OsStr::new("jsonl") {
                return Err(format!(
                    "--protocol must appear at most once and equal jsonl; {USAGE}"
                ));
            }
            protocol_seen = true;
        } else if option == OsStr::new("--wave-width") {
            wave_width = match value.to_str() {
                Some("32") => DebugWaveWidthV1::Wave32,
                Some("64") => DebugWaveWidthV1::Wave64,
                _ => return Err(format!("--wave-width must be 32 or 64; {USAGE}")),
            };
        } else {
            return Err(format!("unknown option {option:?}; {USAGE}"));
        }
    }
    Ok(LiveKfdOptionsV3 {
        bundle_v2: bundle_v2.ok_or_else(|| format!("--bundle-v2 is required; {USAGE}"))?,
        request: request.ok_or_else(|| format!("--request is required; {USAGE}"))?,
        hsaco: hsaco.ok_or_else(|| format!("--hsaco is required; {USAGE}"))?,
        wave_width,
        program: program.ok_or_else(|| format!("-- PROGRAM is required; {USAGE}"))?,
        program_arguments,
    })
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn live_gpu_artifact_binding_v3(
    admitted: &live_kfd_v3::LiveKfdSemanticSessionBindingV3,
    wave_width: DebugWaveWidthV1,
) -> LiveGpuArtifactBindingV3 {
    let declared = admitted
        .declared_hsaco()
        .expect("live-kfd command admission requires a declared HSACO");
    let unavailable_truth = || LiveGpuTruthV3 {
        origin: LiveGpuTruthOriginV3::Unavailable,
        evidence: Vec::new(),
    };
    let binding_identity = nonzero_identity(admitted.session_identity());
    let declaration = LiveGpuTruthV3 {
        origin: LiveGpuTruthOriginV3::Declared,
        evidence: vec![LiveGpuEvidenceRefV3 {
            kind: LiveGpuEvidenceKindV3::Declaration,
            identity: binding_identity,
        }],
    };
    let content = |identity: live_kfd_v3::LiveKfdContentIdentityV3| LiveGpuContentIdentityV3 {
        digest: nonzero_identity(identity.sha256()),
        canonical_bytes: identity.length(),
    };
    let input = admitted.admitted_input();
    let bundle = admitted.bundle();
    let binding = LiveGpuArtifactBindingV3 {
        binding_identity,
        code_object_version: u16::from(declared.code_object_version()),
        declared_code_object: content(declared.content()),
        declaration,
        target_declared_code_object: LiveGpuAvailabilityV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::NotObserved,
            truth: unavailable_truth(),
        },
        target_telemetry: LiveGpuAvailabilityV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::NotObserved,
            truth: unavailable_truth(),
        },
        execution_code_object: LiveGpuAvailabilityV3::Unavailable {
            reason: LiveGpuUnavailableReasonV3::NotObserved,
            truth: unavailable_truth(),
        },
        kernel_ir_v7: LiveGpuContentIdentityV3 {
            digest: nonzero_identity(input.kir_sha256),
            canonical_bytes: input.module.identity().canonical_length(),
        },
        source_map_v2: LiveGpuContentIdentityV3 {
            digest: nonzero_identity(*bundle.debug_map_identity()),
            canonical_bytes: u64::try_from(bundle.debug_map().len())
                .expect("bounded source-map length fits u64"),
        },
        isa_map_v1: None,
        cpu_reference: LiveGpuCpuReferenceBindingV3 {
            bundle_identity: nonzero_identity(admitted.bundle_identity()),
            request_identity: nonzero_identity(admitted.request_content_identity().sha256()),
            configuration_identity: configuration_identity_for_input(input, wave_width),
            deterministic_evidence: LiveGpuCpuReferenceEvidenceV3::Unavailable {
                reason: LiveGpuUnavailableReasonV3::NotCaptured,
            },
        },
    };
    binding
        .validate(LiveGpuProtocolLimitsV3::default())
        .expect("exactly admitted live KFD inputs form a valid V3 binding");
    binding
}

fn parse_options(arguments: impl Iterator<Item = OsString>) -> Result<OptionsV1, String> {
    let mut arguments = arguments.peekable();
    if arguments.next().as_deref() != Some(OsStr::new("sim")) {
        return Err(USAGE.to_owned());
    }
    let mut kir_v7 = None;
    let mut kir_v7_fd = None;
    let mut bundle = None;
    let mut bundle_v2 = None;
    let mut request = None;
    let mut request_fd = None;
    let mut source_map = None;
    let mut source_bundle_subject = None;
    let mut replay_schedule = None;
    let mut protocol_seen = false;
    let mut wave_width = DebugWaveWidthV1::Wave64;
    while let Some(option) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("option {option:?} requires a value; {USAGE}"))?;
        if option == OsStr::new("--kir-v7") {
            set_once(&mut kir_v7, PathBuf::from(value), "--kir-v7")?;
        } else if option == OsStr::new("--kir-v7-fd") {
            set_once(
                &mut kir_v7_fd,
                parse_sealed_input_fd_v1(&value, "--kir-v7-fd")?,
                "--kir-v7-fd",
            )?;
        } else if option == OsStr::new("--bundle") {
            set_once(&mut bundle, PathBuf::from(value), "--bundle")?;
        } else if option == OsStr::new("--bundle-v2") {
            set_once(&mut bundle_v2, PathBuf::from(value), "--bundle-v2")?;
        } else if option == OsStr::new("--request") {
            set_once(&mut request, PathBuf::from(value), "--request")?;
        } else if option == OsStr::new("--request-fd") {
            set_once(
                &mut request_fd,
                parse_sealed_input_fd_v1(&value, "--request-fd")?,
                "--request-fd",
            )?;
        } else if option == OsStr::new("--source-map") {
            set_once(&mut source_map, PathBuf::from(value), "--source-map")?;
        } else if option == OsStr::new("--replay-schedule") {
            set_once(
                &mut replay_schedule,
                PathBuf::from(value),
                "--replay-schedule",
            )?;
        } else if option == OsStr::new("--source-bundle-subject") {
            let value = value
                .to_str()
                .ok_or_else(|| format!("--source-bundle-subject must be UTF-8; {USAGE}"))?;
            let quoted = serde_json::to_string(value)
                .map_err(|_| format!("invalid --source-bundle-subject; {USAGE}"))?;
            let identity: OpaqueIdentityV1 = serde_json::from_str(&quoted).map_err(|_| {
                format!("--source-bundle-subject must be 64 lowercase hex digits; {USAGE}")
            })?;
            set_once(
                &mut source_bundle_subject,
                identity,
                "--source-bundle-subject",
            )?;
        } else if option == OsStr::new("--protocol") {
            if protocol_seen || value != OsStr::new("jsonl") {
                return Err(format!(
                    "--protocol must appear at most once and equal jsonl; {USAGE}"
                ));
            }
            protocol_seen = true;
        } else if option == OsStr::new("--wave-width") {
            wave_width = match value.to_str() {
                Some("32") => DebugWaveWidthV1::Wave32,
                Some("64") => DebugWaveWidthV1::Wave64,
                _ => return Err(format!("--wave-width must be 32 or 64; {USAGE}")),
            };
        } else {
            return Err(format!("unknown option {option:?}; {USAGE}"));
        }
    }
    if source_map.is_some() != source_bundle_subject.is_some() {
        return Err(format!(
            "--source-map and --source-bundle-subject must be supplied together; {USAGE}"
        ));
    }
    let program = match (kir_v7, kir_v7_fd, bundle, bundle_v2) {
        (Some(path), None, None, None) => ProgramInputV1::KirV7(path),
        (None, Some(fd), None, None) => ProgramInputV1::SealedKirV7Fd(fd),
        (None, None, Some(path), None) => ProgramInputV1::Bundle(path),
        (None, None, None, Some(path)) => ProgramInputV1::BundleV2(path),
        (None, None, None, None) => {
            return Err(format!("exactly one program input is required; {USAGE}"));
        }
        _ => {
            return Err(format!("program inputs are mutually exclusive; {USAGE}"));
        }
    };
    if matches!(
        program,
        ProgramInputV1::Bundle(_) | ProgramInputV1::BundleV2(_)
    ) && source_map.is_some()
    {
        return Err(format!(
            "--source-map and --source-bundle-subject cannot override an admitted bundle; {USAGE}"
        ));
    }
    let request = match (&program, request, request_fd) {
        (ProgramInputV1::SealedKirV7Fd(kir_fd), None, Some(request_fd))
            if *kir_fd != request_fd =>
        {
            RequestInputV1::SealedFd(request_fd)
        }
        (ProgramInputV1::SealedKirV7Fd(_), None, Some(_)) => {
            return Err(format!(
                "--kir-v7-fd and --request-fd must name distinct descriptors; {USAGE}"
            ));
        }
        (ProgramInputV1::SealedKirV7Fd(_), _, _) => {
            return Err(format!(
                "--kir-v7-fd requires exactly one --request-fd; {USAGE}"
            ));
        }
        (_, Some(path), None) => RequestInputV1::Path(path),
        (_, _, _) => {
            return Err(format!(
                "path program inputs require exactly one --request and no --request-fd; {USAGE}"
            ));
        }
    };
    Ok(OptionsV1 {
        program,
        request,
        source_map,
        source_bundle_subject,
        replay_schedule,
        wave_width,
    })
}

fn parse_sealed_input_fd_v1(value: &OsStr, option: &str) -> Result<i32, String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{option} must be a canonical decimal descriptor; {USAGE}"))?;
    let descriptor = value
        .parse::<i32>()
        .ok()
        .filter(|descriptor| *descriptor >= 3)
        .ok_or_else(|| format!("{option} must be a descriptor at least 3; {USAGE}"))?;
    if descriptor.to_string() != value {
        return Err(format!(
            "{option} must be a canonical decimal descriptor; {USAGE}"
        ));
    }
    Ok(descriptor)
}

#[cfg(target_os = "linux")]
const REQUIRED_SEALED_DEBUG_INPUT_SEALS_V1: rustix::fs::SealFlags = rustix::fs::SealFlags::WRITE
    .union(rustix::fs::SealFlags::GROW)
    .union(rustix::fs::SealFlags::SHRINK)
    .union(rustix::fs::SealFlags::SEAL);

#[cfg(target_os = "linux")]
struct AdmittedSealedDebugInputV1 {
    bytes: Vec<u8>,
    object: (u64, u64),
}

#[cfg(target_os = "linux")]
fn read_sealed_debug_input_pair_v1(
    kir_descriptor: RawFd,
    request_descriptor: RawFd,
    maximum: usize,
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let kir = read_sealed_debug_input_fd_v1(kir_descriptor, maximum, "canonical KIR V7")?;
    let request = read_sealed_debug_input_fd_v1(request_descriptor, maximum, "simulation request")?;
    if kir.object == request.object {
        return Err(
            "canonical KIR V7 and simulation request descriptors alias the same sealed memfd"
                .to_owned(),
        );
    }
    Ok((kir.bytes, request.bytes))
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
fn read_sealed_debug_input_fd_v1(
    descriptor: RawFd,
    maximum: usize,
    label: &str,
) -> Result<AdmittedSealedDebugInputV1, String> {
    // SAFETY: the numeric descriptor is not consumed; every operation first
    // validates it and the owned duplicate is the only descriptor converted
    // into `File`.
    let inherited = unsafe { BorrowedFd::borrow_raw(descriptor) };
    rustix::io::fcntl_getfd(inherited).map_err(|_| format!("{label} descriptor is unavailable"))?;
    rustix::io::fcntl_setfd(inherited, rustix::io::FdFlags::CLOEXEC)
        .map_err(|_| format!("could not protect inherited {label} descriptor"))?;
    let duplicate = rustix::io::fcntl_dupfd_cloexec(inherited, 3)
        .map_err(|_| format!("could not duplicate inherited {label} descriptor"))?;
    let file = File::from(duplicate);
    let before = rustix::fs::fstat(&file)
        .map_err(|_| format!("could not inspect inherited {label} descriptor"))?;
    let status = rustix::fs::fcntl_getfl(&file)
        .map_err(|_| format!("could not inspect inherited {label} status"))?;
    let seals = rustix::fs::fcntl_get_seals(&file)
        .map_err(|_| format!("could not inspect inherited {label} seals"))?;
    let length = usize::try_from(before.st_size)
        .ok()
        .filter(|length| *length > 0 && *length <= maximum)
        .ok_or_else(|| format!("inherited {label} exceeds its {maximum}-byte bound"))?;
    if rustix::fs::FileType::from_raw_mode(before.st_mode) != rustix::fs::FileType::RegularFile
        || before.st_nlink != 0
        || before.st_mode & 0o7777 != 0o400
        || status & rustix::fs::OFlags::ACCMODE != rustix::fs::OFlags::RDONLY
        || status.contains(rustix::fs::OFlags::PATH)
        || seals != REQUIRED_SEALED_DEBUG_INPUT_SEALS_V1
    {
        return Err(format!(
            "inherited {label} is not an exact immutable read-only input memfd"
        ));
    }
    let proc_link = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))
        .map_err(|_| format!("could not inspect inherited {label} descriptor link"))?;
    let proc_link = proc_link.as_os_str().as_encoded_bytes();
    if !(proc_link.starts_with(b"/memfd:") || proc_link.starts_with(b"memfd:"))
        || !proc_link.ends_with(b" (deleted)")
    {
        return Err(format!("inherited {label} is not a sealed memfd"));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| format!("could not reserve bounded inherited {label}"))?;
    bytes.resize(length, 0);
    file.read_exact_at(&mut bytes, 0)
        .map_err(|_| format!("could not read inherited {label}"))?;
    let after =
        rustix::fs::fstat(&file).map_err(|_| format!("could not revalidate inherited {label}"))?;
    let after_seals = rustix::fs::fcntl_get_seals(&file)
        .map_err(|_| format!("could not revalidate inherited {label} seals"))?;
    if before.st_dev != after.st_dev
        || before.st_ino != after.st_ino
        || before.st_mode != after.st_mode
        || before.st_nlink != after.st_nlink
        || before.st_size != after.st_size
        || before.st_mtime != after.st_mtime
        || before.st_mtime_nsec != after.st_mtime_nsec
        || before.st_ctime != after.st_ctime
        || before.st_ctime_nsec != after.st_ctime_nsec
        || seals != after_seals
    {
        return Err(format!("inherited {label} changed during admission"));
    }
    Ok(AdmittedSealedDebugInputV1 {
        bytes,
        object: (before.st_dev, before.st_ino),
    })
}

fn set_once<T>(slot: &mut Option<T>, value: T, name: &str) -> Result<(), String> {
    if slot.replace(value).is_some() {
        return Err(format!("{name} may appear only once; {USAGE}"));
    }
    Ok(())
}

fn write_input_error(error: &SimulationInputErrorV1) {
    write_bootstrap_error(&error.stage, &error.code, &error.message);
}

fn write_bootstrap_error(stage: &str, code: &str, message: &str) {
    let document = BootstrapErrorV1 {
        schema: "fe2o3-debug-bootstrap-error-v1",
        status: "error",
        stage,
        code,
        message,
    };
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    let _ = serde_json::to_writer(&mut stderr, &document);
    let _ = stderr.write_all(b"\n");
}

pub fn run_admitted_jsonl_v1<R: BufRead, W: Write>(
    input: AdmittedSimulationInputV1,
    wave_width: DebugWaveWidthV1,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), String> {
    let backend = SimulatorBackendV1::new(input, wave_width)?;
    run_jsonl_v1(backend, reader, writer)
}

pub fn run_admitted_jsonl_with_source_map_v1<R: BufRead, W: Write>(
    input: AdmittedSimulationInputV1,
    wave_width: DebugWaveWidthV1,
    source_map_bytes: &[u8],
    expected_bundle_subject: OpaqueIdentityV1,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), String> {
    let configuration = configuration_identity_for_input(&input, wave_width);
    let source_map = admit_source_map_v1(
        source_map_bytes,
        &input,
        configuration,
        expected_bundle_subject,
    )?;
    let backend = SimulatorBackendV1::new_with_source_map(input, wave_width, Some(source_map))?;
    run_jsonl_v1(backend, reader, writer)
}

/// Typed failure from a compiler-bundle-bound debugger session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompilerBundleDebugRunErrorV1 {
    /// The exact embedded map failed its bundle/KIR/content admission.
    SourceMap(String),
    /// Deterministic simulator capture or source-catalog binding failed.
    Backend(String),
    /// The bounded JSONL request/response stream failed after admission.
    ProtocolStream(String),
}

impl std::fmt::Display for CompilerBundleDebugRunErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::SourceMap(message) | Self::Backend(message) | Self::ProtocolStream(message) => {
                message
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CompilerBundleDebugRunErrorV1 {}

/// Runs with map bytes obtained from a compiler-bundle decode transaction.
/// `verified_bundle_subject` and `committed_map_identity` must come from that
/// transaction, not from the map document or command-line input.
pub fn run_admitted_jsonl_with_compiler_source_map_v1<R: BufRead, W: Write>(
    input: AdmittedSimulationInputV1,
    wave_width: DebugWaveWidthV1,
    source_map_bytes: &[u8],
    verified_bundle_subject: OpaqueIdentityV1,
    committed_map_identity: OpaqueIdentityV1,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), CompilerBundleDebugRunErrorV1> {
    run_admitted_jsonl_with_compiler_source_map_and_schedule_v1(
        input,
        wave_width,
        source_map_bytes,
        verified_bundle_subject,
        committed_map_identity,
        None,
        reader,
        writer,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_admitted_jsonl_with_compiler_source_map_and_schedule_v1<R: BufRead, W: Write>(
    input: AdmittedSimulationInputV1,
    wave_width: DebugWaveWidthV1,
    source_map_bytes: &[u8],
    verified_bundle_subject: OpaqueIdentityV1,
    committed_map_identity: OpaqueIdentityV1,
    replay_schedule: Option<&SimulationScheduleRecordV1>,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), CompilerBundleDebugRunErrorV1> {
    if input.simulation_bundle_subject() != Some(verified_bundle_subject.as_bytes()) {
        return Err(CompilerBundleDebugRunErrorV1::SourceMap(
            "compiler-bundle source map subject is not retained by the admitted bundle input"
                .to_owned(),
        ));
    }
    let configuration = configuration_identity_for_input(&input, wave_width);
    let source_map = admit_source_map_with_provenance_v1(
        source_map_bytes,
        &input,
        configuration,
        verified_bundle_subject,
        Some(committed_map_identity),
        SourceMapProvenanceV1::CompilerBundleBound,
    )
    .map_err(CompilerBundleDebugRunErrorV1::SourceMap)?;
    let backend = SimulatorBackendV1::new_with_source_map_and_schedule(
        input,
        wave_width,
        Some(source_map),
        replay_schedule,
    )
    .map_err(CompilerBundleDebugRunErrorV1::Backend)?;
    run_jsonl_v1(backend, reader, writer).map_err(CompilerBundleDebugRunErrorV1::ProtocolStream)
}

#[allow(clippy::too_many_arguments)]
fn run_admitted_jsonl_with_bundle_source_map_v2<R: BufRead, W: Write>(
    input: AdmittedSimulationInputV1,
    wave_width: DebugWaveWidthV1,
    source_map_bytes: &[u8],
    verified_bundle_subject: OpaqueIdentityV1,
    committed_map_identity: OpaqueIdentityV1,
    replay_schedule: Option<&SimulationScheduleRecordV1>,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), CompilerBundleDebugRunErrorV1> {
    if input.simulation_bundle_subject() != Some(verified_bundle_subject.as_bytes()) {
        return Err(CompilerBundleDebugRunErrorV1::SourceMap(
            "compiler-bundle V2 source map subject is not retained by the admitted input"
                .to_owned(),
        ));
    }
    let configuration = configuration_identity_for_input(&input, wave_width);
    let source_map = admit_source_map_v2(
        source_map_bytes,
        &input,
        configuration,
        verified_bundle_subject,
        committed_map_identity,
    )
    .map_err(CompilerBundleDebugRunErrorV1::SourceMap)?;
    let backend = SimulatorBackendV1::new_with_source_map_v2_and_schedule(
        input,
        wave_width,
        source_map,
        replay_schedule,
    )
    .map_err(CompilerBundleDebugRunErrorV1::Backend)?;
    run_jsonl_v1(backend, reader, writer).map_err(CompilerBundleDebugRunErrorV1::ProtocolStream)
}

fn run_jsonl_v1<R: BufRead, W: Write>(
    mut backend: SimulatorBackendV1,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), String> {
    let limits = backend.protocol_limits;
    loop {
        let request = match read_request_line_any_v2(reader, limits) {
            Ok(Some(request)) => request,
            Ok(None) => break,
            Err(error) => {
                let response = backend.codec_error(error);
                write_response(writer, &response, limits)?;
                break;
            }
        };
        match request {
            DebugRequestAnyV2::V1(request) => {
                let terminate = matches!(request, DebugRequestV1::Terminate { .. });
                let response = backend.handle(request);
                write_response(writer, &response, limits)?;
                if terminate && matches!(response, DebugResponseV1::Ok { .. }) {
                    break;
                }
            }
            DebugRequestAnyV2::SourceVariablesV2(request) => {
                let response = backend.handle_source_variables_v2(request);
                write_source_variable_response_v2(writer, &response, limits)?;
            }
            DebugRequestAnyV2::DiagnosisV2(request) => {
                let response = backend.handle_diagnosis_v2(request);
                write_diagnosis_response_v2(writer, &response, limits)?;
            }
        }
    }
    writer
        .flush()
        .map_err(|_| "failed to flush debugger responses".to_owned())
}

fn write_diagnosis_response_v2<W: Write>(
    writer: &mut W,
    response: &DiagnosisResponseV2,
    limits: ProtocolLimitsV1,
) -> Result<(), String> {
    match encode_diagnosis_response_line_v2(response, limits) {
        Ok(bytes) => writer
            .write_all(&bytes)
            .map_err(|_| "failed to write diagnosis V2 response".to_owned()),
        Err(ProtocolCodecErrorV1::ResponseTooLarge) => {
            let (request_id, operation, session) = match response {
                DiagnosisResponseV2::Ok {
                    request_id,
                    operation,
                    session,
                    ..
                } => (Some(*request_id), *operation, *session),
                DiagnosisResponseV2::Error {
                    request_id,
                    operation,
                    session,
                    ..
                } => (*request_id, *operation, *session),
            };
            let fallback = DiagnosisResponseV2::Error {
                schema: DiagnosisResponseSchemaV2::V2,
                request_id,
                operation,
                session,
                error: DebugErrorV1 {
                    stage: DebugErrorStageV1::Output,
                    code: DebugErrorCodeV1::ResponseTooLarge,
                    message: "response exceeds the configured JSONL bound".to_owned(),
                    state_changed: false,
                },
            };
            let bytes = encode_diagnosis_response_line_v2(&fallback, limits).map_err(|error| {
                format!("failed to encode bounded diagnosis V2 fallback: {error}")
            })?;
            writer
                .write_all(&bytes)
                .map_err(|_| "failed to write bounded diagnosis V2 fallback response".to_owned())
        }
        Err(error) => Err(format!("failed to encode diagnosis V2 response: {error}")),
    }?;
    writer
        .flush()
        .map_err(|_| "failed to flush diagnosis V2 response".to_owned())
}

fn write_source_variable_response_v2<W: Write>(
    writer: &mut W,
    response: &SourceVariableResponseV2,
    limits: ProtocolLimitsV1,
) -> Result<(), String> {
    match encode_source_variable_response_line_v2(response, limits) {
        Ok(bytes) => writer
            .write_all(&bytes)
            .map_err(|_| "failed to write source-variable V2 response".to_owned()),
        Err(ProtocolCodecErrorV1::ResponseTooLarge) => {
            let fallback = SourceVariableResponseV2::Error {
                schema: SourceVariableResponseSchemaV2::V2,
                request_id: source_variable_response_request_id_v2(response),
                operation: source_variable_response_operation_v2(response),
                session: source_variable_response_session_v2(response),
                error: DebugErrorV1 {
                    stage: DebugErrorStageV1::Output,
                    code: DebugErrorCodeV1::ResponseTooLarge,
                    message: "response exceeds the configured JSONL bound".to_owned(),
                    state_changed: false,
                },
            };
            let bytes =
                encode_source_variable_response_line_v2(&fallback, limits).map_err(|error| {
                    format!("failed to encode bounded source-variable V2 fallback: {error}")
                })?;
            writer.write_all(&bytes).map_err(|_| {
                "failed to write bounded source-variable V2 fallback response".to_owned()
            })
        }
        Err(error) => Err(format!(
            "failed to encode source-variable V2 response: {error}"
        )),
    }?;
    writer
        .flush()
        .map_err(|_| "failed to flush source-variable V2 response".to_owned())
}

fn source_variable_response_request_id_v2(response: &SourceVariableResponseV2) -> Option<u64> {
    match response {
        SourceVariableResponseV2::Ok { request_id, .. }
        | SourceVariableResponseV2::Unavailable { request_id, .. } => Some(*request_id),
        SourceVariableResponseV2::Error { request_id, .. } => *request_id,
    }
}

fn source_variable_response_operation_v2(
    response: &SourceVariableResponseV2,
) -> SourceVariableOperationV2 {
    match response {
        SourceVariableResponseV2::Ok { operation, .. }
        | SourceVariableResponseV2::Unavailable { operation, .. }
        | SourceVariableResponseV2::Error { operation, .. } => *operation,
    }
}

fn source_variable_response_session_v2(response: &SourceVariableResponseV2) -> SessionViewV1 {
    match response {
        SourceVariableResponseV2::Ok { session, .. }
        | SourceVariableResponseV2::Unavailable { session, .. }
        | SourceVariableResponseV2::Error { session, .. } => *session,
    }
}

fn write_response<W: Write>(
    writer: &mut W,
    response: &DebugResponseV1,
    limits: ProtocolLimitsV1,
) -> Result<(), String> {
    match encode_response_line_v1(response, limits) {
        Ok(bytes) => writer
            .write_all(&bytes)
            .map_err(|_| "failed to write debugger response".to_owned()),
        Err(ProtocolCodecErrorV1::ResponseTooLarge) => {
            let fallback = DebugResponseV1::Error {
                schema: ResponseSchemaV1::V1,
                request_id: response_request_id(response),
                operation: response_operation(response),
                session: response_session(response),
                error: DebugErrorV1 {
                    stage: DebugErrorStageV1::Output,
                    code: DebugErrorCodeV1::ResponseTooLarge,
                    message: "response exceeds the configured JSONL bound".to_owned(),
                    state_changed: false,
                },
            };
            let bytes = encode_response_line_v1(&fallback, limits)
                .map_err(|error| format!("failed to encode bounded fallback: {error}"))?;
            writer
                .write_all(&bytes)
                .map_err(|_| "failed to write bounded fallback response".to_owned())
        }
        Err(error) => Err(format!("failed to encode debugger response: {error}")),
    }?;
    writer
        .flush()
        .map_err(|_| "failed to flush debugger response".to_owned())
}

fn response_request_id(response: &DebugResponseV1) -> Option<u64> {
    match response {
        DebugResponseV1::Ok { request_id, .. }
        | DebugResponseV1::Unavailable { request_id, .. } => Some(*request_id),
        DebugResponseV1::Error { request_id, .. } => *request_id,
    }
}

fn response_operation(response: &DebugResponseV1) -> Option<DebugOperationNameV1> {
    match response {
        DebugResponseV1::Ok { operation, .. } | DebugResponseV1::Unavailable { operation, .. } => {
            Some(*operation)
        }
        DebugResponseV1::Error { operation, .. } => *operation,
    }
}

fn response_session(response: &DebugResponseV1) -> Option<SessionViewV1> {
    match response {
        DebugResponseV1::Ok { session, .. } | DebugResponseV1::Unavailable { session, .. } => {
            Some(*session)
        }
        DebugResponseV1::Error { session, .. } => *session,
    }
}

struct SimulatorBackendV1 {
    module: AdmittedSimulationModuleV1,
    session: DebugSessionV1,
    wave_width: DebugWaveWidthV1,
    configuration_identity: OpaqueIdentityV1,
    diagnosis_dispatch: DiagnosisDispatchV2,
    diagnosis_input: DiagnosisInputEvidenceV2,
    diagnosis_allocations: BTreeMap<u64, DiagnosisAllocationContractV2>,
    diagnosis_source_members: Vec<AdmittedDiagnosisSourceMemberV2>,
    source_map_identity: Option<OpaqueIdentityV1>,
    source_map_provenance: Option<SourceMapProvenanceV1>,
    source_variables_v2: Option<AdmittedSourceVariablesV2>,
    revision: u64,
    command_count: u64,
    terminated: bool,
    failed_execution: bool,
    last_stop: Option<StopViewV1>,
    next_breakpoint_id: u64,
    next_watchpoint_id: u64,
    breakpoint_specs: BTreeMap<u64, BreakpointSpecV1>,
    watchpoint_specs: BTreeMap<u64, WatchpointSpecV1>,
    protocol_limits: ProtocolLimitsV1,
}

impl SimulatorBackendV1 {
    fn new(input: AdmittedSimulationInputV1, wave_width: DebugWaveWidthV1) -> Result<Self, String> {
        Self::new_with_source_map(input, wave_width, None)
    }

    fn new_with_source_map(
        input: AdmittedSimulationInputV1,
        wave_width: DebugWaveWidthV1,
        source_map: Option<AdmittedSourceMapV1>,
    ) -> Result<Self, String> {
        Self::new_with_source_map_and_schedule(input, wave_width, source_map, None)
    }

    fn new_with_source_map_and_schedule(
        input: AdmittedSimulationInputV1,
        wave_width: DebugWaveWidthV1,
        source_map: Option<AdmittedSourceMapV1>,
        replay_schedule: Option<&SimulationScheduleRecordV1>,
    ) -> Result<Self, String> {
        Self::new_with_maps_and_schedule(input, wave_width, source_map, None, replay_schedule)
    }

    fn new_with_source_map_v2_and_schedule(
        input: AdmittedSimulationInputV1,
        wave_width: DebugWaveWidthV1,
        source_map: AdmittedSourceMapV2,
        replay_schedule: Option<&SimulationScheduleRecordV1>,
    ) -> Result<Self, String> {
        Self::new_with_maps_and_schedule(input, wave_width, None, Some(source_map), replay_schedule)
    }

    fn new_with_maps_and_schedule(
        input: AdmittedSimulationInputV1,
        wave_width: DebugWaveWidthV1,
        source_map: Option<AdmittedSourceMapV1>,
        source_map_v2: Option<AdmittedSourceMapV2>,
        replay_schedule: Option<&SimulationScheduleRecordV1>,
    ) -> Result<Self, String> {
        let diagnosis_dispatch = DiagnosisDispatchV2 {
            launch_extent: input.request.grid.0,
            workgroup_size: input.request.workgroup.0,
        };
        let diagnosis_allocations = diagnosis_initial_allocations_v2(&input)?;
        let capture_limits =
            SimulationDebugCaptureLimitsV1::new(64, 4_096, 16_384, 16 * 1024 * 1024)
                .map_err(|error| error.to_string())?;
        let debugger_limits = DebuggerLimitsV1::new(1_000_000, 16_000_000, 256 * 1024 * 1024)
            .map_err(|error| error.to_string())?;
        let base_configuration_identity = configuration_identity_for_input(&input, wave_width);
        let run = match replay_schedule {
            Some(schedule) => capture_debugger_replayed_run_v1(
                &input.module,
                &input.request,
                input.simulation_target(),
                input.simulation_limits,
                capture_limits,
                debugger_limits,
                wave_width,
                schedule,
            ),
            None => capture_debugger_run_v1(
                &input.module,
                &input.request,
                input.simulation_target(),
                input.simulation_limits,
                capture_limits,
                debugger_limits,
                wave_width,
            ),
        };
        if let Err(SimulationErrorV1::Preflight(error)) = &run.execution {
            return Err(error.to_string());
        }
        if replay_schedule.is_some()
            && let Err(SimulationErrorV1::Execution(error)) = &run.execution
            && matches!(
                &error.kind,
                fe2o3_kir_sim::SimulationExecutionErrorKindV1::ScheduleDecisionLimit { .. }
                    | fe2o3_kir_sim::SimulationExecutionErrorKindV1::ScheduleResidentLimit { .. }
                    | fe2o3_kir_sim::SimulationExecutionErrorKindV1::ScheduleReplay(_)
            )
        {
            return Err(format!(
                "persisted semantic schedule replay failed: {error}"
            ));
        }
        let failed_execution = matches!(run.execution, Err(SimulationErrorV1::Execution(_)));
        let configuration_identity = replay_schedule
            .map_or(base_configuration_identity, |schedule| {
                configuration_identity_for_replay(base_configuration_identity, schedule)
            });
        let configuration_identity =
            source_map_v2
                .as_ref()
                .map_or(configuration_identity, |source_map| {
                    configuration_identity_for_source_map_v2(
                        configuration_identity,
                        source_map.identity,
                        source_map.bundle_subject_identity,
                        source_map.diagnosis_operation_root,
                        source_map.diagnosis_operation_count,
                    )
                });
        let request_reference = DiagnosisContentReferenceV2 {
            sha256: nonzero_identity(input.request_sha256),
            canonical_bytes: input.request_bytes(),
        };
        let kir_reference = DiagnosisContentReferenceV2 {
            sha256: nonzero_identity(input.kir_sha256),
            canonical_bytes: input.module.identity().canonical_length(),
        };
        let dispatch_identity =
            diagnosis_dispatch_input_identity_v2(request_reference, kir_reference)
                .map_err(|_| "simulated dispatch input identity is zero".to_owned())?;
        let (simulation_bundle, production_kir, kernel_abi_identity, source_lineage) =
            input.simulation_bundle_evidence().map_or_else(
                || {
                    (
                        DiagnosisFactV2::Unavailable {
                            reason: DiagnosisUnavailableReasonV2::InputNotProvided,
                        },
                        DiagnosisFactV2::Unavailable {
                            reason: DiagnosisUnavailableReasonV2::InputNotProvided,
                        },
                        DiagnosisFactV2::Unavailable {
                            reason: DiagnosisUnavailableReasonV2::InputNotProvided,
                        },
                        DiagnosisFactV2::Unavailable {
                            reason: DiagnosisUnavailableReasonV2::InputNotProvided,
                        },
                    )
                },
                |bundle| {
                    (
                        DiagnosisFactV2::Declared {
                            value: DiagnosisBundleReferenceV2 {
                                envelope_version: bundle.envelope_version,
                                identity: nonzero_identity(bundle.envelope_identity),
                                subject_identity: nonzero_identity(bundle.subject_identity),
                            },
                        },
                        DiagnosisFactV2::Declared {
                            value: DiagnosisVersionedContentReferenceV2 {
                                version: bundle.production_kir_version,
                                content: DiagnosisContentReferenceV2 {
                                    sha256: nonzero_identity(bundle.production_kir_sha256),
                                    canonical_bytes: bundle.production_kir_bytes,
                                },
                            },
                        },
                        DiagnosisFactV2::Declared {
                            value: nonzero_identity(bundle.kernel_abi_identity),
                        },
                        DiagnosisFactV2::Declared {
                            value: DiagnosisSourceLineageV2 {
                                identity_inventory_receipt: DiagnosisContentReferenceV2 {
                                    sha256: nonzero_identity(
                                        bundle.identity_inventory_receipt_sha256,
                                    ),
                                    canonical_bytes: bundle.identity_inventory_receipt_bytes,
                                },
                                preflight_plan_receipt: DiagnosisContentReferenceV2 {
                                    sha256: nonzero_identity(bundle.preflight_plan_receipt_sha256),
                                    canonical_bytes: bundle.preflight_plan_receipt_bytes,
                                },
                            },
                        },
                    )
                },
            );
        let source_map_v2_reference =
            source_map_v2
                .as_ref()
                .map(|source_map| DiagnosisSourceMapReferenceV2 {
                    identity: source_map.identity,
                    bundle_subject_identity: source_map.bundle_subject_identity,
                    provenance: source_map.provenance,
                    operation_membership_root: source_map.diagnosis_operation_root,
                    operation_members: source_map.diagnosis_operation_count,
                });
        let mut diagnosis_source_members = Vec::new();
        if let Some(source_map) = source_map_v2.as_ref() {
            diagnosis_source_members
                .try_reserve_exact(source_map.diagnosis_operation_members.len())
                .map_err(|_| "diagnosis source member allocation failed".to_owned())?;
            diagnosis_source_members.extend_from_slice(&source_map.diagnosis_operation_members);
        }
        let diagnosis_input = DiagnosisInputEvidenceV2 {
            configuration_identity,
            dispatch_identity,
            dispatch_request: DiagnosisFactV2::Declared {
                value: request_reference,
            },
            canonical_kir_v7: DiagnosisFactV2::Declared {
                value: kir_reference,
            },
            simulation_bundle,
            production_kir,
            kernel_abi_identity,
            source_lineage,
            source_map_v2: source_map_v2_reference.map_or(
                DiagnosisFactV2::Unavailable {
                    reason: if source_map.is_some() {
                        DiagnosisUnavailableReasonV2::RequiresSourceMapV2
                    } else {
                        DiagnosisUnavailableReasonV2::InputNotProvided
                    },
                },
                |value| DiagnosisFactV2::Declared { value },
            ),
            finalized_artifact: DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::NoArtifactAuthority,
            },
            property_proof: DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::NoProofAuthority,
            },
        };
        let mut session = DebugSessionV1::new(run.transcript);
        let mut source_map_provenance = None;
        let source_map_identity = if let Some(source_map) = source_map {
            if source_map.configuration_identity != base_configuration_identity {
                return Err("source map configuration identity changed before binding".to_owned());
            }
            let _externally_bound_subject = source_map.bundle_subject_identity;
            let identity = source_map.identity;
            source_map_provenance = Some(source_map.provenance);
            session
                .bind_source_catalog(&input.module, source_map.catalog)
                .map_err(|error| format!("source map binding failed: {error}"))?;
            Some(identity)
        } else {
            source_map_v2.as_ref().map(|source_map| source_map.identity)
        };
        let source_variables_v2 = if let Some(source_map) = source_map_v2 {
            if source_map.configuration_identity != base_configuration_identity {
                return Err(
                    "source map V2 configuration identity changed before binding".to_owned(),
                );
            }
            let _bundle_subject = source_map.bundle_subject_identity;
            source_map_provenance = Some(source_map.provenance);
            session
                .bind_source_catalog(&input.module, source_map.catalog)
                .map_err(|error| format!("source map V2 binding failed: {error}"))?;
            Some(source_map.variables)
        } else {
            None
        };
        Ok(Self {
            module: input.module,
            session,
            wave_width,
            configuration_identity,
            diagnosis_dispatch,
            diagnosis_input,
            diagnosis_allocations,
            diagnosis_source_members,
            source_map_identity,
            source_map_provenance,
            source_variables_v2,
            revision: 0,
            command_count: 0,
            terminated: false,
            failed_execution,
            last_stop: None,
            next_breakpoint_id: 1,
            next_watchpoint_id: 1,
            breakpoint_specs: BTreeMap::new(),
            watchpoint_specs: BTreeMap::new(),
            protocol_limits: ProtocolLimitsV1::default(),
        })
    }

    fn codec_error(&self, error: ProtocolCodecErrorV1) -> DebugResponseV1 {
        DebugResponseV1::Error {
            schema: ResponseSchemaV1::V1,
            request_id: None,
            operation: None,
            session: Some(self.session_view()),
            error: DebugErrorV1 {
                stage: DebugErrorStageV1::Framing,
                code: if error == ProtocolCodecErrorV1::InvalidJson {
                    DebugErrorCodeV1::InvalidJson
                } else {
                    DebugErrorCodeV1::InvalidRequest
                },
                message: bounded_message(error.code()),
                state_changed: false,
            },
        }
    }

    fn session_view(&self) -> SessionViewV1 {
        let state = if self.terminated {
            SessionStateV1::Terminated
        } else {
            SessionStateV1::Stopped
        };
        SessionViewV1 {
            backend: DebugBackendV1::CpuKirSimulator,
            execution_kind: ExecutionKindV1::CpuKirSimulation,
            state,
            revision: self.revision,
            configuration_identity: self.configuration_identity,
            cursor: DebugCursorV1 {
                configuration_identity: self.configuration_identity,
                event_sequence: self.cursor_sequence(),
                state_revision: self.revision,
            },
            simulated: true,
            hardware_observed: false,
            performance_prediction: false,
        }
    }

    fn cursor_sequence(&self) -> u64 {
        self.session
            .cursor_record_index()
            .and_then(|index| u64::try_from(index).ok())
            .map_or(0, |index| index.saturating_add(1))
    }

    fn handle(&mut self, request: DebugRequestV1) -> DebugResponseV1 {
        let id = request.request_id();
        let operation = request.operation();
        if request.expected_revision() != self.revision {
            return self.error(
                Some(id),
                Some(operation),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::StaleRevision,
                "expected_revision does not match the current session revision",
            );
        }
        if self.terminated {
            return self.error(
                Some(id),
                Some(operation),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::InvalidState,
                "debug session is terminated",
            );
        }
        if self.command_count == MAX_SESSION_COMMANDS_V1 {
            return self.error(
                Some(id),
                Some(operation),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "debug session command budget is exhausted",
            );
        }
        self.command_count += 1;
        self.handle_admitted(request)
    }

    fn handle_source_variables_v2(
        &mut self,
        request: SourceVariableRequestV2,
    ) -> SourceVariableResponseV2 {
        let request_id = request.request_id();
        if request.expected_revision() != self.revision {
            return self.source_variable_error_v2(
                Some(request_id),
                DebugErrorCodeV1::StaleRevision,
                "expected_revision does not match the current session revision",
            );
        }
        if self.terminated {
            return self.source_variable_error_v2(
                Some(request_id),
                DebugErrorCodeV1::InvalidState,
                "debug session is terminated",
            );
        }
        if self.command_count == MAX_SESSION_COMMANDS_V1 {
            return self.source_variable_error_v2(
                Some(request_id),
                DebugErrorCodeV1::ResourceLimit,
                "debug session command budget is exhausted",
            );
        }
        self.command_count += 1;
        let SourceVariableRequestV2::InspectSourceVariables {
            scope,
            frame,
            selector,
            page,
            ..
        } = request;
        let Some(index) = self.source_variables_v2.as_ref() else {
            return self.source_variable_unavailable_v2(
                request_id,
                SourceVariableQueryUnavailableReasonV2::SourceMapV2Required,
            );
        };
        if index.variables.is_empty() {
            return self.source_variable_unavailable_v2(
                request_id,
                SourceVariableQueryUnavailableReasonV2::VariablesNotCaptured,
            );
        }
        let Some(record) = self.session.current() else {
            return self.source_variable_unavailable_v2(
                request_id,
                SourceVariableQueryUnavailableReasonV2::CheckpointNotCaptured,
            );
        };
        if !convert_scope_selector(scope).matches(record.invocation, self.wave_width) {
            return self.source_variable_unavailable_v2(
                request_id,
                SourceVariableQueryUnavailableReasonV2::OutsideCaptureScope,
            );
        }
        let SimulationDebugRecordKindV1::Checkpoint { stack, .. } = &record.kind else {
            return self.source_variable_unavailable_v2(
                request_id,
                SourceVariableQueryUnavailableReasonV2::CheckpointNotCaptured,
            );
        };
        let SimulationDebugCollectionV1::Captured(frames) = stack else {
            return self.source_variable_unavailable_v2(
                request_id,
                SourceVariableQueryUnavailableReasonV2::CheckpointNotCaptured,
            );
        };
        let frame_identity = frame.unwrap_or(1);
        let Ok(depth) = u32::try_from(frame_identity - 1) else {
            return self.source_variable_error_v2(
                Some(request_id),
                DebugErrorCodeV1::InvalidRequest,
                "frame identity exceeds the simulator frame range",
            );
        };
        let Some(selected_frame) = frames.iter().find(|candidate| candidate.depth == depth) else {
            return self.source_variable_unavailable_v2(
                request_id,
                SourceVariableQueryUnavailableReasonV2::FrameUnavailable,
            );
        };
        let Some(next_operation) = selected_frame.next_operation else {
            return self.source_variable_unavailable_v2(
                request_id,
                SourceVariableQueryUnavailableReasonV2::CheckpointNotCaptured,
            );
        };

        let mut ambiguous_name = false;
        let mut retained_name_candidates = Vec::new();
        let candidates: &[usize] = match &selector {
            SourceVariableSelectorV2::All => index
                .by_function
                .get(&selected_frame.function_ordinal)
                .map_or(&[][..], Vec::as_slice),
            SourceVariableSelectorV2::Identity { variable_identity } => {
                let Some(candidate) = index.by_identity.get(variable_identity) else {
                    return self.source_variable_unavailable_v2(
                        request_id,
                        SourceVariableQueryUnavailableReasonV2::OutsideCaptureScope,
                    );
                };
                if index.variables[*candidate].function_ordinal != selected_frame.function_ordinal {
                    return self.source_variable_unavailable_v2(
                        request_id,
                        SourceVariableQueryUnavailableReasonV2::OutsideCaptureScope,
                    );
                }
                std::slice::from_ref(candidate)
            }
            SourceVariableSelectorV2::Name { name } => {
                let named = index
                    .by_name
                    .get(&(selected_frame.function_ordinal, name.clone()))
                    .map_or(&[][..], Vec::as_slice);
                if retained_name_candidates
                    .try_reserve_exact(named.len())
                    .is_err()
                {
                    return self.source_variable_error_v2(
                        Some(request_id),
                        DebugErrorCodeV1::ResourceLimit,
                        "source variable name resolution allocation failed",
                    );
                }
                retained_name_candidates.extend(named.iter().copied().filter(|candidate| {
                    !matches!(
                        source_variable_effective_binding_at_v2(
                            &index.variables[*candidate],
                            selected_frame.block,
                            next_operation,
                        )
                        .1,
                        DebugSourceVariableBindingV2::NotInScope
                    )
                }));
                let Some(deepest) = retained_name_candidates
                    .iter()
                    .copied()
                    .max_by_key(|candidate| index.variables[*candidate].scope_depth)
                else {
                    return self.source_variable_unavailable_v2(
                        request_id,
                        SourceVariableQueryUnavailableReasonV2::NameNotInScope,
                    );
                };
                let deepest_scope = index.variables[deepest].scope_identity;
                let scopes_form_chain = match source_variable_scopes_form_chain_v2(
                    index,
                    &retained_name_candidates,
                    deepest_scope,
                ) {
                    Ok(result) => result,
                    Err(()) => {
                        return self.source_variable_error_v2(
                            Some(request_id),
                            DebugErrorCodeV1::ResourceLimit,
                            "source variable scope resolution allocation failed",
                        );
                    }
                };
                if scopes_form_chain {
                    let maximum_depth = index.variables[deepest].scope_depth;
                    retained_name_candidates.retain(|candidate| {
                        index.variables[*candidate].scope_depth == maximum_depth
                    });
                    ambiguous_name = retained_name_candidates.len() > 1;
                } else {
                    ambiguous_name = true;
                }
                &retained_name_candidates
            }
        };
        let query_bytes =
            match serde_json::to_vec(&(scope, frame_identity, &selector, self.cursor_sequence())) {
                Ok(bytes) => bytes,
                Err(_) => {
                    return self.source_variable_error_v2(
                        Some(request_id),
                        DebugErrorCodeV1::ResourceLimit,
                        "source variable query binding allocation failed",
                    );
                }
            };
        let query = self.query_identity(b"inspect-source-variables-v2", &query_bytes);
        let (start, end, next_cursor) = match page_bounds(page, query, candidates.len()) {
            Ok(bounds) => bounds,
            Err(message) => {
                return self.source_variable_error_v2(
                    Some(request_id),
                    DebugErrorCodeV1::InvalidCursor,
                    message,
                );
            }
        };
        let mut values = Vec::new();
        if values.try_reserve_exact(end - start).is_err() {
            return self.source_variable_error_v2(
                Some(request_id),
                DebugErrorCodeV1::ResourceLimit,
                "source variable response allocation failed",
            );
        }
        for candidate in &candidates[start..end] {
            values.push(source_variable_value_v2(
                &index.variables[*candidate],
                selected_frame,
                next_operation,
                ambiguous_name,
            ));
        }
        let Some(snapshot) = self.current_anchor(Some(frame_identity)) else {
            return self.source_variable_unavailable_v2(
                request_id,
                SourceVariableQueryUnavailableReasonV2::CheckpointNotCaptured,
            );
        };
        SourceVariableResponseV2::Ok {
            schema: SourceVariableResponseSchemaV2::V2,
            request_id,
            operation: SourceVariableOperationV2::InspectSourceVariables,
            session: self.session_view(),
            snapshot: Box::new(snapshot),
            values,
            next_cursor,
        }
    }

    fn handle_diagnosis_v2(&mut self, request: DiagnosisRequestV2) -> DiagnosisResponseV2 {
        let request_id = request.request_id();
        if request.expected_revision() != self.revision {
            return self.diagnosis_error_v2(
                Some(request_id),
                DebugErrorCodeV1::StaleRevision,
                "expected_revision does not match the current session revision",
            );
        }
        if self.terminated {
            return self.diagnosis_error_v2(
                Some(request_id),
                DebugErrorCodeV1::InvalidState,
                "debug session is terminated",
            );
        }
        if self.command_count == MAX_SESSION_COMMANDS_V1 {
            return self.diagnosis_error_v2(
                Some(request_id),
                DebugErrorCodeV1::ResourceLimit,
                "debug session command budget is exhausted",
            );
        }
        self.command_count += 1;
        let DiagnosisRequestV2::Diagnose { filter, page, .. } = request;
        self.diagnose_v2(request_id, filter, page)
    }

    fn diagnose_v2(
        &self,
        request_id: u64,
        filter: DiagnosisFilterV2,
        page: PageRequestV1,
    ) -> DiagnosisResponseV2 {
        let query_bytes = match serde_json::to_vec(&filter) {
            Ok(bytes) => bytes,
            Err(_) => {
                return self.diagnosis_error_v2(
                    Some(request_id),
                    DebugErrorCodeV1::ResourceLimit,
                    "diagnosis query binding allocation failed",
                );
            }
        };
        let query = self.query_identity(b"diagnose-v2", &query_bytes);
        let terminal_fault = self.session.transcript().terminal_fault();
        let expected_retained =
            terminal_fault.and_then(|fault| self.diagnosis_retained_evidence_v2(fault));
        let candidate = terminal_fault
            .and_then(|fault| self.diagnosis_view_v2(fault))
            .filter(|diagnosis| {
                filter.class.is_none_or(|class| diagnosis.class == class)
                    && filter.scope.is_none_or(|scope| {
                        self.session
                            .transcript()
                            .terminal_fault()
                            .is_some_and(|fault| {
                                self.diagnosis_fault_matches_scope_v2(fault, scope)
                            })
                    })
            });
        let total = usize::from(candidate.is_some());
        let (start, end, next_cursor) = match page_bounds(page, query, total) {
            Ok(bounds) => bounds,
            Err(message) => {
                return self.diagnosis_error_v2(
                    Some(request_id),
                    DebugErrorCodeV1::InvalidCursor,
                    message,
                );
            }
        };
        let diagnoses = if start < end {
            candidate.into_iter().collect()
        } else {
            Vec::new()
        };
        let emitted_events =
            u64::try_from(self.session.transcript().records().len()).unwrap_or(u64::MAX);
        let completeness = match self.session.transcript().completeness() {
            DebugTranscriptCompletenessV1::Complete => CaptureCompletenessV1::Complete,
            DebugTranscriptCompletenessV1::Truncated(reason) => CaptureCompletenessV1::Truncated {
                reason: transcript_truncation_reason(reason),
                emitted_events,
                dropped_events: None,
            },
        };
        let session = self.session_view();
        let response_envelope = DiagnosisResponseEnvelopeBindingV2 {
            schema: DiagnosisResponseSchemaV2::V2,
            request_id,
            operation: DiagnosisOperationV2::Diagnose,
            next_cursor,
        };
        let expected_capture = expected_retained.and_then(|retained| {
            retained
                .capture_binding_v2(
                    &self.diagnosis_input,
                    session,
                    completeness,
                    response_envelope,
                )
                .ok()
        });
        let response = DiagnosisResponseV2::Ok {
            schema: DiagnosisResponseSchemaV2::V2,
            request_id,
            operation: DiagnosisOperationV2::Diagnose,
            session,
            completeness,
            diagnoses,
            next_cursor,
        };
        if !matches!(&response, DiagnosisResponseV2::Ok { diagnoses, .. } if diagnoses.is_empty())
            && expected_capture.is_none_or(|expected| {
                response
                    .validate_against_capture_v2(self.protocol_limits, expected)
                    .is_err()
            })
        {
            return self.diagnosis_error_v2(
                Some(request_id),
                DebugErrorCodeV1::InvalidState,
                "diagnosis evidence does not match the retained simulator capture",
            );
        }
        response
    }

    fn diagnosis_error_v2(
        &self,
        request_id: Option<u64>,
        code: DebugErrorCodeV1,
        message: &str,
    ) -> DiagnosisResponseV2 {
        DiagnosisResponseV2::Error {
            schema: DiagnosisResponseSchemaV2::V2,
            request_id,
            operation: DiagnosisOperationV2::Diagnose,
            session: self.session_view(),
            error: DebugErrorV1 {
                stage: DebugErrorStageV1::Session,
                code,
                message: bounded_message(message),
                state_changed: false,
            },
        }
    }

    fn source_variable_unavailable_v2(
        &self,
        request_id: u64,
        reason: SourceVariableQueryUnavailableReasonV2,
    ) -> SourceVariableResponseV2 {
        SourceVariableResponseV2::Unavailable {
            schema: SourceVariableResponseSchemaV2::V2,
            request_id,
            operation: SourceVariableOperationV2::InspectSourceVariables,
            session: self.session_view(),
            reason,
        }
    }

    fn source_variable_error_v2(
        &self,
        request_id: Option<u64>,
        code: DebugErrorCodeV1,
        message: &str,
    ) -> SourceVariableResponseV2 {
        SourceVariableResponseV2::Error {
            schema: SourceVariableResponseSchemaV2::V2,
            request_id,
            operation: SourceVariableOperationV2::InspectSourceVariables,
            session: self.session_view(),
            error: DebugErrorV1 {
                stage: DebugErrorStageV1::Session,
                code,
                message: bounded_message(message),
                state_changed: false,
            },
        }
    }

    fn ok(
        &self,
        request_id: u64,
        operation: DebugOperationNameV1,
        result: DebugResultV1,
    ) -> DebugResponseV1 {
        DebugResponseV1::Ok {
            schema: ResponseSchemaV1::V1,
            request_id,
            operation,
            session: self.session_view(),
            result: Box::new(result),
        }
    }

    fn unavailable(
        &self,
        request_id: u64,
        operation: DebugOperationNameV1,
        capability: DebugCapabilityNameV1,
        reason: CapabilityUnavailableReasonV1,
        detail: &str,
    ) -> DebugResponseV1 {
        DebugResponseV1::Unavailable {
            schema: ResponseSchemaV1::V1,
            request_id,
            operation,
            session: self.session_view(),
            unavailable: CapabilityUnavailableV1 {
                capability,
                reason,
                state_changed: false,
                detail: bounded_message(detail),
            },
        }
    }

    fn error(
        &self,
        request_id: Option<u64>,
        operation: Option<DebugOperationNameV1>,
        stage: DebugErrorStageV1,
        code: DebugErrorCodeV1,
        message: &str,
    ) -> DebugResponseV1 {
        DebugResponseV1::Error {
            schema: ResponseSchemaV1::V1,
            request_id,
            operation,
            session: Some(self.session_view()),
            error: DebugErrorV1 {
                stage,
                code,
                message: bounded_message(message),
                state_changed: false,
            },
        }
    }

    fn bump_revision(&mut self) -> Result<(), ()> {
        self.revision = self.revision.checked_add(1).ok_or(())?;
        Ok(())
    }

    fn handle_admitted(&mut self, request: DebugRequestV1) -> DebugResponseV1 {
        match request {
            DebugRequestV1::DiscoverCapabilities { request_id, .. } => self.ok(
                request_id,
                DebugOperationNameV1::DiscoverCapabilities,
                DebugResultV1::Capabilities {
                    capabilities: simulator_capabilities(
                        self.source_map_identity.is_some(),
                        self.source_variables_v2
                            .as_ref()
                            .map(|variables| !variables.variables.is_empty()),
                    ),
                },
            ),
            DebugRequestV1::GetState { request_id, .. } => self.ok(
                request_id,
                DebugOperationNameV1::GetState,
                DebugResultV1::State {
                    snapshot: self.snapshot_availability(),
                },
            ),
            DebugRequestV1::SetBreakpoints {
                request_id,
                breakpoints,
                ..
            } => self.set_breakpoints(request_id, breakpoints),
            DebugRequestV1::RemoveBreakpoints {
                request_id,
                breakpoint_ids,
                ..
            } => self.remove_breakpoints(request_id, &breakpoint_ids),
            DebugRequestV1::ListBreakpoints {
                request_id, page, ..
            } => self.list_breakpoints(request_id, page),
            DebugRequestV1::SetWatchpoints {
                request_id,
                watchpoints,
                ..
            } => self.set_watchpoints(request_id, watchpoints),
            DebugRequestV1::RemoveWatchpoints {
                request_id,
                watchpoint_ids,
                ..
            } => self.remove_watchpoints(request_id, &watchpoint_ids),
            DebugRequestV1::ListWatchpoints {
                request_id, page, ..
            } => self.list_watchpoints(request_id, page),
            DebugRequestV1::Continue {
                request_id,
                max_events,
                ..
            } => self.continue_forward(request_id, max_events),
            DebugRequestV1::Pause { request_id, .. } => self.unavailable(
                request_id,
                DebugOperationNameV1::Pause,
                DebugCapabilityNameV1::Pause,
                CapabilityUnavailableReasonV1::NotExposedByBackend,
                "the deterministic simulator is synchronously stopped between JSONL requests",
            ),
            DebugRequestV1::Step {
                request_id,
                direction,
                granularity,
                count,
                focus,
                ..
            } => self.step(request_id, direction, granularity, count, focus),
            DebugRequestV1::Seek {
                request_id, cursor, ..
            } => self.seek(request_id, cursor),
            DebugRequestV1::InspectScope {
                request_id,
                scope,
                include_children,
                page,
                ..
            } => self.inspect_scope(request_id, scope, include_children, page),
            DebugRequestV1::ResolveSource {
                request_id, site, ..
            } => self.resolve_source(request_id, site),
            DebugRequestV1::InspectStack {
                request_id,
                scope,
                page,
                ..
            } => self.inspect_stack(request_id, scope, page),
            DebugRequestV1::InspectValues {
                request_id,
                scope,
                frame,
                selector,
                page,
                ..
            } => self.inspect_values(request_id, scope, frame, selector, page),
            DebugRequestV1::ReadMemory {
                request_id,
                allocation,
                byte_offset,
                byte_len,
                ..
            } => self.read_memory(request_id, allocation, byte_offset, byte_len),
            DebugRequestV1::QueryEvents {
                request_id,
                filter,
                page,
                ..
            } => self.query_events(request_id, filter, page),
            DebugRequestV1::ExportTrace {
                request_id,
                max_bytes,
                ..
            } => self.export_trace(request_id, max_bytes),
            DebugRequestV1::Terminate { request_id, .. } => {
                if self.bump_revision().is_err() {
                    return self.error(
                        Some(request_id),
                        Some(DebugOperationNameV1::Terminate),
                        DebugErrorStageV1::Session,
                        DebugErrorCodeV1::ResourceLimit,
                        "session revision is exhausted",
                    );
                }
                self.terminated = true;
                self.last_stop = Some(StopViewV1 {
                    reason: StopReasonV1::Terminated,
                    breakpoint_id: None,
                    watchpoint_id: None,
                    outcome: ExecutionOutcomeV1::Cancelled,
                    exact: true,
                });
                self.ok(
                    request_id,
                    DebugOperationNameV1::Terminate,
                    DebugResultV1::Terminated,
                )
            }
        }
    }

    fn set_breakpoints(
        &mut self,
        request_id: u64,
        specs: Vec<BreakpointSpecV1>,
    ) -> DebugResponseV1 {
        let count = specs.len();
        if self.breakpoint_specs.len().saturating_add(specs.len())
            > self.protocol_limits.max_breakpoints
        {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::SetBreakpoints),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "breakpoint session bound would be exceeded",
            );
        }
        let Some(end_id) = self
            .next_breakpoint_id
            .checked_add(u64::try_from(specs.len()).unwrap_or(u64::MAX))
        else {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::SetBreakpoints),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "breakpoint identity space is exhausted",
            );
        };
        let mut converted = Vec::new();
        if converted.try_reserve_exact(specs.len()).is_err() {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::SetBreakpoints),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "breakpoint conversion allocation failed",
            );
        }
        for (offset, spec) in specs.iter().enumerate() {
            let id = self.next_breakpoint_id + u64::try_from(offset).unwrap_or(u64::MAX);
            match self.convert_breakpoint(id, spec) {
                Ok(value) => converted.push(value),
                Err(ConvertErrorV1::Unavailable(capability, reason, detail)) => {
                    return self.unavailable(
                        request_id,
                        DebugOperationNameV1::SetBreakpoints,
                        capability,
                        reason,
                        detail,
                    );
                }
                Err(ConvertErrorV1::Invalid(detail)) => {
                    return self.error(
                        Some(request_id),
                        Some(DebugOperationNameV1::SetBreakpoints),
                        DebugErrorStageV1::Protocol,
                        DebugErrorCodeV1::InvalidRequest,
                        detail,
                    );
                }
            }
        }
        if self.session.add_breakpoints_atomic(converted).is_err() {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::SetBreakpoints),
                DebugErrorStageV1::Backend,
                DebugErrorCodeV1::BackendFailure,
                "replay debugger rejected the atomic breakpoint set",
            );
        }
        if self.bump_revision().is_err() {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::SetBreakpoints),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "session revision is exhausted",
            );
        }
        for (offset, spec) in specs.into_iter().enumerate() {
            let id = self.next_breakpoint_id + u64::try_from(offset).unwrap_or(u64::MAX);
            self.breakpoint_specs.insert(id, spec);
        }
        self.next_breakpoint_id = end_id;
        self.ok(
            request_id,
            DebugOperationNameV1::SetBreakpoints,
            DebugResultV1::Acknowledged {
                accepted: u32::try_from(count).unwrap_or(u32::MAX),
            },
        )
    }

    fn remove_breakpoints(&mut self, request_id: u64, ids: &[u64]) -> DebugResponseV1 {
        if ids.iter().any(|id| !self.breakpoint_specs.contains_key(id)) {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::RemoveBreakpoints),
                DebugErrorStageV1::Protocol,
                DebugErrorCodeV1::InvalidRequest,
                "remove_breakpoints contains an unknown breakpoint identity",
            );
        }
        for id in ids {
            let removed = self.session.remove_breakpoint(*id);
            debug_assert!(removed);
            self.breakpoint_specs.remove(id);
        }
        if self.bump_revision().is_err() {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::RemoveBreakpoints),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "session revision is exhausted",
            );
        }
        self.ok(
            request_id,
            DebugOperationNameV1::RemoveBreakpoints,
            DebugResultV1::Acknowledged {
                accepted: u32::try_from(ids.len()).unwrap_or(u32::MAX),
            },
        )
    }

    fn list_breakpoints(&self, request_id: u64, page: PageRequestV1) -> DebugResponseV1 {
        let query = self.query_identity(b"list-breakpoints", &[]);
        let (start, end, next_cursor) = match page_bounds(page, query, self.breakpoint_specs.len())
        {
            Ok(bounds) => bounds,
            Err(message) => {
                return self.error(
                    Some(request_id),
                    Some(DebugOperationNameV1::ListBreakpoints),
                    DebugErrorStageV1::Protocol,
                    DebugErrorCodeV1::InvalidCursor,
                    message,
                );
            }
        };
        let breakpoints = self
            .breakpoint_specs
            .iter()
            .skip(start)
            .take(end - start)
            .map(|(id, spec)| BreakpointViewV1 {
                breakpoint_id: *id,
                spec: spec.clone(),
                hit_count: self.session.breakpoint_hit_count(*id).unwrap_or(0),
            })
            .collect();
        self.ok(
            request_id,
            DebugOperationNameV1::ListBreakpoints,
            DebugResultV1::Breakpoints {
                breakpoints,
                next_cursor,
            },
        )
    }

    fn convert_breakpoint(
        &self,
        id: u64,
        spec: &BreakpointSpecV1,
    ) -> Result<DebugBreakpointV1, ConvertErrorV1> {
        let scope =
            convert_scope_selector(spec.scope.unwrap_or(ExecutionScopeSelectorV1::Dispatch));
        let hit_condition = spec.hit_condition.map(convert_hit_condition).transpose()?;
        let (site, predicate) = match &spec.kind {
            BreakpointKindV1::Site { site, phase } => {
                let point = match site.point {
                    KirSitePointV1::Operation { operation_ordinal } => operation_ordinal,
                    KirSitePointV1::BlockEntry | KirSitePointV1::Terminator => {
                        return Err(ConvertErrorV1::Unavailable(
                            DebugCapabilityNameV1::KirSites,
                            CapabilityUnavailableReasonV1::NotRepresented,
                            "V1 simulator checkpoints are operation sites, not block-entry or terminator sites",
                        ));
                    }
                };
                let function_ordinal = usize::try_from(site.function_ordinal).map_err(|_| {
                    ConvertErrorV1::Invalid("function ordinal does not fit this host")
                })?;
                let operation = u32::try_from(point).map_err(|_| {
                    ConvertErrorV1::Invalid("operation ordinal is outside KIR V7 bounds")
                })?;
                let function = self.module.module().functions.get(function_ordinal).ok_or(
                    ConvertErrorV1::Invalid("breakpoint function ordinal is unknown"),
                )?;
                let body = function
                    .body
                    .as_ref()
                    .ok_or(ConvertErrorV1::Invalid("breakpoint function has no body"))?;
                let block_index = usize::try_from(site.block_ordinal)
                    .map_err(|_| ConvertErrorV1::Invalid("block ordinal does not fit this host"))?;
                let block = body.blocks.get(block_index).ok_or(ConvertErrorV1::Invalid(
                    "breakpoint block ordinal is unknown",
                ))?;
                if block.operations.get(operation as usize).is_none() {
                    return Err(ConvertErrorV1::Invalid(
                        "breakpoint operation ordinal is unknown",
                    ));
                }
                (
                    DebugSiteSelectorV1 {
                        function_ordinal: Some(function_ordinal),
                        block: Some(block.id),
                        operation: Some(operation),
                        phase: Some(match phase {
                            OperationStopPhaseV1::BeforeOperation => {
                                SimulationDebugCheckpointPhaseV1::BeforeOperation
                            }
                            OperationStopPhaseV1::AfterOperation => {
                                SimulationDebugCheckpointPhaseV1::AfterOperation
                            }
                        }),
                    },
                    DebugPredicateV1::True,
                )
            }
            BreakpointKindV1::Value { predicate } => {
                let (predicate, function_ordinal) = convert_predicate(predicate)?;
                (
                    DebugSiteSelectorV1 {
                        function_ordinal,
                        block: None,
                        operation: None,
                        phase: None,
                    },
                    predicate,
                )
            }
            BreakpointKindV1::Source { source } => {
                let Some(map_identity) = self.source_map_identity else {
                    return Err(ConvertErrorV1::Unavailable(
                        DebugCapabilityNameV1::SourceSites,
                        CapabilityUnavailableReasonV1::RequiresAuthenticatedMap,
                        "source breakpoints require an exact-KIR bound source map",
                    ));
                };
                if source.map_identity != map_identity {
                    return Err(ConvertErrorV1::Unavailable(
                        DebugCapabilityNameV1::SourceSites,
                        CapabilityUnavailableReasonV1::RequiresAuthenticatedMap,
                        "source breakpoint map identity is stale or belongs to another map",
                    ));
                }
                if Some(source.provenance) != self.source_map_provenance {
                    return Err(ConvertErrorV1::Unavailable(
                        DebugCapabilityNameV1::SourceSites,
                        CapabilityUnavailableReasonV1::RequiresAuthenticatedMap,
                        "source breakpoint provenance does not match the bound map",
                    ));
                }
                let resolution = self.session.resolve_source_location(
                    source.file_identity.as_bytes(),
                    source.byte_start,
                    source.byte_end,
                );
                let DebugInspectionV1::Available(resolution) = resolution else {
                    return Err(ConvertErrorV1::Unavailable(
                        DebugCapabilityNameV1::SourceSites,
                        CapabilityUnavailableReasonV1::RequiresAuthenticatedMap,
                        "source map is not bound to this debugger session",
                    ));
                };
                let DebugSourceResolutionV1::Resolved { site, .. } = resolution else {
                    let (reason, detail) = match resolution {
                        DebugSourceResolutionV1::Absent => (
                            CapabilityUnavailableReasonV1::Absent,
                            "source location is absent from the bound map",
                        ),
                        DebugSourceResolutionV1::Eliminated => (
                            CapabilityUnavailableReasonV1::OptimizedOut,
                            "source location was eliminated before canonical KIR",
                        ),
                        DebugSourceResolutionV1::ManyToOne => (
                            CapabilityUnavailableReasonV1::ManyToOne,
                            "source location does not resolve to one exact KIR operation",
                        ),
                        DebugSourceResolutionV1::Resolved { .. } => unreachable!(),
                    };
                    return Err(ConvertErrorV1::Unavailable(
                        DebugCapabilityNameV1::SourceSites,
                        reason,
                        detail,
                    ));
                };
                (
                    DebugSiteSelectorV1 {
                        function_ordinal: Some(site.function_ordinal),
                        block: Some(site.block),
                        operation: Some(site.operation),
                        phase: Some(SimulationDebugCheckpointPhaseV1::BeforeOperation),
                    },
                    DebugPredicateV1::True,
                )
            }
            BreakpointKindV1::Barrier { .. } => {
                return Err(ConvertErrorV1::Unavailable(
                    DebugCapabilityNameV1::Breakpoints,
                    CapabilityUnavailableReasonV1::NotRepresented,
                    "barrier records can be stepped and queried, but V1 replay breakpoint filters are operation/value based",
                ));
            }
            BreakpointKindV1::Diagnostic { .. } => {
                return Err(ConvertErrorV1::Unavailable(
                    DebugCapabilityNameV1::Breakpoints,
                    CapabilityUnavailableReasonV1::NotRepresented,
                    "diagnostic-class breakpoints are not represented by the V1 replay breakpoint engine",
                ));
            }
        };
        Ok(DebugBreakpointV1 {
            id,
            site,
            scope,
            predicate,
            hit_condition,
            enabled: spec.enabled,
        })
    }

    fn set_watchpoints(
        &mut self,
        request_id: u64,
        specs: Vec<WatchpointSpecV1>,
    ) -> DebugResponseV1 {
        if self.watchpoint_specs.len().saturating_add(specs.len())
            > self.protocol_limits.max_watchpoints
        {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::SetWatchpoints),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "watchpoint session bound would be exceeded",
            );
        }
        let count = specs.len();
        let Some(end_id) = self
            .next_watchpoint_id
            .checked_add(u64::try_from(count).unwrap_or(u64::MAX))
        else {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::SetWatchpoints),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "watchpoint identity space is exhausted",
            );
        };
        let mut converted = Vec::new();
        if converted.try_reserve_exact(count).is_err() {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::SetWatchpoints),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "watchpoint conversion allocation failed",
            );
        }
        for (offset, spec) in specs.iter().enumerate() {
            if spec.allocation.generation != 0 {
                return self.unavailable(
                    request_id,
                    DebugOperationNameV1::SetWatchpoints,
                    DebugCapabilityNameV1::Watchpoints,
                    CapabilityUnavailableReasonV1::NotRepresented,
                    "CPU simulation allocations have generation zero",
                );
            }
            if spec.timing != MemoryStopPhaseV1::AfterCommit {
                return self.unavailable(
                    request_id,
                    DebugOperationNameV1::SetWatchpoints,
                    DebugCapabilityNameV1::Watchpoints,
                    CapabilityUnavailableReasonV1::NotCaptured,
                    "the replay transcript exposes successful reads and committed writes, not pre-commit writes",
                );
            }
            let access = match spec.access {
                WatchAccessV1::Read => DebugWatchAccessV1::Read,
                WatchAccessV1::Write => DebugWatchAccessV1::Write,
                WatchAccessV1::Any => DebugWatchAccessV1::ReadWrite,
                WatchAccessV1::Atomic => DebugWatchAccessV1::Atomic,
            };
            let byte_offset = match usize::try_from(spec.byte_offset) {
                Ok(value) => value,
                Err(_) => {
                    return self.error(
                        Some(request_id),
                        Some(DebugOperationNameV1::SetWatchpoints),
                        DebugErrorStageV1::Protocol,
                        DebugErrorCodeV1::InvalidRequest,
                        "watchpoint byte offset does not fit this host",
                    );
                }
            };
            let byte_len = match usize::try_from(spec.byte_len) {
                Ok(value) => value,
                Err(_) => {
                    return self.error(
                        Some(request_id),
                        Some(DebugOperationNameV1::SetWatchpoints),
                        DebugErrorStageV1::Protocol,
                        DebugErrorCodeV1::InvalidRequest,
                        "watchpoint byte length does not fit this host",
                    );
                }
            };
            converted.push(DebugWatchpointV1 {
                id: self.next_watchpoint_id + u64::try_from(offset).unwrap_or(u64::MAX),
                allocation: spec.allocation.ordinal,
                byte_offset,
                byte_len,
                access,
                scope: convert_scope_selector(
                    spec.scope.unwrap_or(ExecutionScopeSelectorV1::Dispatch),
                ),
                value_equals: None,
                enabled: spec.enabled,
            });
        }
        if self.session.add_watchpoints_atomic(converted).is_err() {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::SetWatchpoints),
                DebugErrorStageV1::Backend,
                DebugErrorCodeV1::BackendFailure,
                "replay debugger rejected the atomic watchpoint set",
            );
        }
        if self.bump_revision().is_err() {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::SetWatchpoints),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "session revision is exhausted",
            );
        }
        for (offset, spec) in specs.into_iter().enumerate() {
            self.watchpoint_specs.insert(
                self.next_watchpoint_id + u64::try_from(offset).unwrap_or(u64::MAX),
                spec,
            );
        }
        self.next_watchpoint_id = end_id;
        self.ok(
            request_id,
            DebugOperationNameV1::SetWatchpoints,
            DebugResultV1::Acknowledged {
                accepted: u32::try_from(count).unwrap_or(u32::MAX),
            },
        )
    }

    fn remove_watchpoints(&mut self, request_id: u64, ids: &[u64]) -> DebugResponseV1 {
        if ids.iter().any(|id| !self.watchpoint_specs.contains_key(id)) {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::RemoveWatchpoints),
                DebugErrorStageV1::Protocol,
                DebugErrorCodeV1::InvalidRequest,
                "remove_watchpoints contains an unknown watchpoint identity",
            );
        }
        for id in ids {
            let removed = self.session.remove_watchpoint(*id);
            debug_assert!(removed);
            self.watchpoint_specs.remove(id);
        }
        if self.bump_revision().is_err() {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::RemoveWatchpoints),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "session revision is exhausted",
            );
        }
        self.ok(
            request_id,
            DebugOperationNameV1::RemoveWatchpoints,
            DebugResultV1::Acknowledged {
                accepted: u32::try_from(ids.len()).unwrap_or(u32::MAX),
            },
        )
    }

    fn list_watchpoints(&self, request_id: u64, page: PageRequestV1) -> DebugResponseV1 {
        let query = self.query_identity(b"list-watchpoints", &[]);
        let (start, end, next_cursor) = match page_bounds(page, query, self.watchpoint_specs.len())
        {
            Ok(bounds) => bounds,
            Err(message) => {
                return self.error(
                    Some(request_id),
                    Some(DebugOperationNameV1::ListWatchpoints),
                    DebugErrorStageV1::Protocol,
                    DebugErrorCodeV1::InvalidCursor,
                    message,
                );
            }
        };
        let watchpoints = self
            .watchpoint_specs
            .iter()
            .skip(start)
            .take(end - start)
            .map(|(id, spec)| WatchpointViewV1 {
                watchpoint_id: *id,
                spec: spec.clone(),
                hit_count: self.session.watchpoint_hit_count(*id).unwrap_or(0),
            })
            .collect();
        self.ok(
            request_id,
            DebugOperationNameV1::ListWatchpoints,
            DebugResultV1::Watchpoints {
                watchpoints,
                next_cursor,
            },
        )
    }

    fn continue_forward(&mut self, request_id: u64, max_events: u64) -> DebugResponseV1 {
        let Ok(max_events) = usize::try_from(max_events) else {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::Continue),
                DebugErrorStageV1::Protocol,
                DebugErrorCodeV1::ResourceLimit,
                "max_events does not fit this host",
            );
        };
        let before = self.cursor_sequence();
        let navigation = self.session.continue_forward_bounded(max_events);
        self.finish_control(
            request_id,
            DebugOperationNameV1::Continue,
            before,
            navigation,
        )
    }

    fn step(
        &mut self,
        request_id: u64,
        direction: StepDirectionV1,
        granularity: StepGranularityV1,
        count: u32,
        focus: Option<ExecutionScopeSelectorV1>,
    ) -> DebugResponseV1 {
        if granularity == StepGranularityV1::Source && self.source_map_identity.is_none() {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::Step,
                DebugCapabilityNameV1::SourceSites,
                CapabilityUnavailableReasonV1::RequiresAuthenticatedMap,
                "source stepping requires an exact-KIR bound source map",
            );
        }
        if direction == StepDirectionV1::Reverse
            && matches!(
                granularity,
                StepGranularityV1::Over | StepGranularityV1::Out
            )
        {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::Step,
                DebugCapabilityNameV1::ReverseStep,
                CapabilityUnavailableReasonV1::NotExposedByBackend,
                "reverse frame-aware over/out is not exposed by the V1 replay engine",
            );
        }
        let before = self.cursor_sequence();
        let saved = self.session.cursor_record_index();
        let scope = convert_scope_selector(focus.unwrap_or(ExecutionScopeSelectorV1::Dispatch));
        let mut navigation = DebugNavigationV1::Beginning;
        for _ in 0..count {
            navigation = match (direction, granularity) {
                (_, StepGranularityV1::Source) => match self.step_source(direction, &scope) {
                    Ok(navigation) => navigation,
                    Err((reason, detail)) => {
                        restore_cursor(&mut self.session, saved);
                        return self.unavailable(
                            request_id,
                            DebugOperationNameV1::Step,
                            DebugCapabilityNameV1::SourceSites,
                            reason,
                            detail,
                        );
                    }
                },
                (StepDirectionV1::Forward, StepGranularityV1::Over) => {
                    self.session.step_over(&scope)
                }
                (StepDirectionV1::Forward, StepGranularityV1::Out) => self.session.step_out(&scope),
                _ => self.step_filtered(direction, granularity, &scope),
            };
            if matches!(navigation, DebugNavigationV1::Unavailable(_)) {
                restore_cursor(&mut self.session, saved);
                return self.unavailable(
                    request_id,
                    DebugOperationNameV1::Step,
                    DebugCapabilityNameV1::ForwardStep,
                    CapabilityUnavailableReasonV1::NotCaptured,
                    "the requested replay step has no captured frame state at the current cursor",
                );
            }
            if matches!(
                navigation,
                DebugNavigationV1::Beginning | DebugNavigationV1::End
            ) {
                break;
            }
        }
        self.finish_control(request_id, DebugOperationNameV1::Step, before, navigation)
    }

    fn step_source(
        &mut self,
        direction: StepDirectionV1,
        scope: &DebugScopeSelectorV1,
    ) -> Result<DebugNavigationV1, (CapabilityUnavailableReasonV1, &'static str)> {
        let baseline = self
            .session
            .current()
            .map(|current| {
                source_span_for_step(self.session.resolve_source_site(current.site), true)
            })
            .transpose()?
            .flatten();
        let records = self.session.transcript().records();
        let cursor = self.session.cursor_record_index();
        let inspect = |index: usize| -> Result<bool, _> {
            let record = &records[index];
            if !scope.matches(record.invocation, self.wave_width) {
                return Ok(false);
            }
            match source_span_for_step(self.session.resolve_source_site(record.site), false)? {
                Some(span) => Ok(baseline.is_none_or(|baseline| span != baseline)),
                None => Ok(false),
            }
        };
        let candidate = match direction {
            StepDirectionV1::Forward => {
                let start = cursor.map_or(0, |index| index.saturating_add(1));
                let mut found = None;
                for index in start..records.len() {
                    if inspect(index)? {
                        found = Some(index);
                        break;
                    }
                }
                found
            }
            StepDirectionV1::Reverse => {
                let Some(start) = cursor.and_then(|index| index.checked_sub(1)) else {
                    return Ok(self.session.seek_entry());
                };
                let mut found = None;
                for index in (0..=start).rev() {
                    if inspect(index)? {
                        found = Some(index);
                        break;
                    }
                }
                found
            }
        };
        Ok(if let Some(index) = candidate {
            self.session.seek_record_index(index)
        } else {
            match direction {
                StepDirectionV1::Forward => self.session.seek_record_index(records.len()),
                StepDirectionV1::Reverse => self.session.seek_entry(),
            }
        })
    }

    fn step_filtered(
        &mut self,
        direction: StepDirectionV1,
        granularity: StepGranularityV1,
        scope: &DebugScopeSelectorV1,
    ) -> DebugNavigationV1 {
        let records = self.session.transcript().records();
        let cursor = self.session.cursor_record_index();
        let baseline = self.session.current_hierarchy();
        let candidate = match direction {
            StepDirectionV1::Forward => {
                let start = cursor.map_or(0, |index| index.saturating_add(1));
                (start..records.len()).find(|index| {
                    record_matches_step(
                        &records[*index],
                        granularity,
                        scope,
                        self.wave_width,
                        baseline,
                    )
                })
            }
            StepDirectionV1::Reverse => {
                let Some(start) = cursor.and_then(|index| index.checked_sub(1)) else {
                    return self.session.seek_entry();
                };
                (0..=start).rev().find(|index| {
                    record_matches_step(
                        &records[*index],
                        granularity,
                        scope,
                        self.wave_width,
                        baseline,
                    )
                })
            }
        };
        if let Some(index) = candidate {
            self.session.seek_record_index(index)
        } else {
            match direction {
                StepDirectionV1::Forward => self
                    .session
                    .seek_record_index(self.session.transcript().records().len()),
                StepDirectionV1::Reverse => self.session.seek_entry(),
            }
        }
    }

    fn seek(&mut self, request_id: u64, cursor: DebugCursorV1) -> DebugResponseV1 {
        if cursor.configuration_identity != self.configuration_identity
            || cursor.state_revision != self.revision
        {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::Seek),
                DebugErrorStageV1::Protocol,
                DebugErrorCodeV1::InvalidCursor,
                "seek cursor is not bound to the current configuration and revision",
            );
        }
        let record_count = self.session.transcript().records().len();
        let maximum = u64::try_from(record_count)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        if cursor.event_sequence > maximum {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::Seek),
                DebugErrorStageV1::Protocol,
                DebugErrorCodeV1::InvalidCursor,
                "seek cursor is outside the captured transcript",
            );
        }
        let before = self.cursor_sequence();
        let navigation = if cursor.event_sequence == 0 {
            self.session.seek_entry()
        } else {
            self.session.seek_record_index(
                usize::try_from(cursor.event_sequence - 1).expect("bounded by record count"),
            )
        };
        self.finish_control(request_id, DebugOperationNameV1::Seek, before, navigation)
    }

    fn finish_control(
        &mut self,
        request_id: u64,
        operation: DebugOperationNameV1,
        before: u64,
        navigation: DebugNavigationV1,
    ) -> DebugResponseV1 {
        let after = self.cursor_sequence();
        let stop = navigation_stop(navigation, self.failed_execution);
        let visible_stop_changed = self.last_stop.as_ref() != Some(&stop);
        if (after != before || visible_stop_changed) && self.bump_revision().is_err() {
            return self.error(
                Some(request_id),
                Some(operation),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "session revision is exhausted",
            );
        }
        self.last_stop = Some(stop.clone());
        self.ok(
            request_id,
            operation,
            DebugResultV1::Control {
                stop: Some(stop),
                snapshot: self.snapshot_availability(),
                events_advanced: before.abs_diff(after),
            },
        )
    }

    fn inspect_scope(
        &self,
        request_id: u64,
        scope: ExecutionScopeSelectorV1,
        include_children: bool,
        page: PageRequestV1,
    ) -> DebugResponseV1 {
        let query_bytes = serde_json::to_vec(&(scope, include_children)).unwrap_or_default();
        let query = self.query_identity(b"inspect-scope", &query_bytes);
        let (start, limit) = match page_window(page, query) {
            Ok(window) => window,
            Err(message) => {
                return self.error(
                    Some(request_id),
                    Some(DebugOperationNameV1::InspectScope),
                    DebugErrorStageV1::Protocol,
                    DebugErrorCodeV1::InvalidCursor,
                    message,
                );
            }
        };
        let mut scopes = Vec::new();
        if scopes.try_reserve_exact(limit).is_err() {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::InspectScope),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "scope page allocation failed",
            );
        }
        let records = self.session.transcript().records();
        let mut total = 0_usize;
        {
            let mut emit = |candidate: ExecutionScopeV1| {
                if total >= start && scopes.len() < limit {
                    scopes.push(candidate);
                }
                total = total.saturating_add(1);
            };
            match scope {
                ExecutionScopeSelectorV1::Dispatch => {
                    emit(ExecutionScopeV1::Dispatch);
                    if include_children {
                        // Simulator transcripts are workgroup-major, so adjacent deduplication
                        // enumerates observed workgroups with constant scratch space.
                        let mut previous = None;
                        for record in records {
                            let group = record.invocation.workgroup;
                            if previous == Some(group) {
                                continue;
                            }
                            previous = Some(group);
                            if let Some(workgroup) = workgroup_u32(group) {
                                emit(ExecutionScopeV1::Workgroup { workgroup });
                            }
                        }
                    }
                }
                ExecutionScopeSelectorV1::Workgroup { workgroup } => {
                    emit(ExecutionScopeV1::Workgroup { workgroup });
                    if include_children {
                        let group64 = workgroup.map(u64::from);
                        let mut waves = [None; 32];
                        for record in records
                            .iter()
                            .filter(|record| record.invocation.workgroup == group64)
                        {
                            let hierarchy =
                                hierarchy_for_invocation_v1(record.invocation, self.wave_width);
                            if let Some(slot) = waves.get_mut(hierarchy.wave as usize) {
                                slot.get_or_insert(record.invocation);
                            }
                        }
                        for (wave, invocation) in waves.into_iter().enumerate() {
                            let Some(invocation) = invocation else {
                                continue;
                            };
                            let hierarchy =
                                hierarchy_for_invocation_v1(invocation, self.wave_width);
                            emit(ExecutionScopeV1::Wave {
                                workgroup,
                                wave: u32::try_from(wave).expect("wave table index fits u32"),
                                active_mask: hierarchy.active_mask,
                                wave_width: self.wave_width.lanes(),
                                interpretation: WaveInterpretationV1::LogicalVisualization,
                            });
                        }
                    }
                }
                ExecutionScopeSelectorV1::Wave { workgroup, wave } => {
                    let group64 = workgroup.map(u64::from);
                    if let Some(invocation) = records
                        .iter()
                        .find(|record| {
                            let hierarchy =
                                hierarchy_for_invocation_v1(record.invocation, self.wave_width);
                            hierarchy.workgroup == group64 && hierarchy.wave == wave
                        })
                        .map(|record| record.invocation)
                    {
                        let hierarchy = hierarchy_for_invocation_v1(invocation, self.wave_width);
                        emit(ExecutionScopeV1::Wave {
                            workgroup,
                            wave,
                            active_mask: hierarchy.active_mask,
                            wave_width: self.wave_width.lanes(),
                            interpretation: WaveInterpretationV1::LogicalVisualization,
                        });
                        if include_children {
                            let mut lanes = [None; 64];
                            for record in records {
                                let candidate =
                                    hierarchy_for_invocation_v1(record.invocation, self.wave_width);
                                if candidate.workgroup == group64 && candidate.wave == wave {
                                    lanes[usize::from(candidate.lane)]
                                        .get_or_insert(record.invocation);
                                }
                            }
                            for (lane, invocation) in lanes.into_iter().enumerate() {
                                let Some(invocation) = invocation else {
                                    continue;
                                };
                                emit(ExecutionScopeV1::Lane {
                                    workgroup,
                                    wave,
                                    lane: u16::try_from(lane).expect("lane table index fits u16"),
                                    logical_workitem: invocation.global,
                                    active_mask: hierarchy.active_mask,
                                    wave_width: self.wave_width.lanes(),
                                    interpretation: WaveInterpretationV1::LogicalVisualization,
                                });
                            }
                        }
                    }
                }
                ExecutionScopeSelectorV1::Lane {
                    workgroup,
                    wave,
                    lane,
                } => {
                    let group64 = workgroup.map(u64::from);
                    if let Some(invocation) = records
                        .iter()
                        .find(|record| {
                            let hierarchy =
                                hierarchy_for_invocation_v1(record.invocation, self.wave_width);
                            hierarchy.workgroup == group64
                                && hierarchy.wave == wave
                                && hierarchy.lane == lane
                        })
                        .map(|record| record.invocation)
                    {
                        let hierarchy = hierarchy_for_invocation_v1(invocation, self.wave_width);
                        emit(ExecutionScopeV1::Lane {
                            workgroup,
                            wave,
                            lane,
                            logical_workitem: invocation.global,
                            active_mask: hierarchy.active_mask,
                            wave_width: self.wave_width.lanes(),
                            interpretation: WaveInterpretationV1::LogicalVisualization,
                        });
                    }
                }
            }
        }
        let next_cursor = match page_next(start, scopes.len(), total, query) {
            Ok(next) => next,
            Err(message) => {
                return self.error(
                    Some(request_id),
                    Some(DebugOperationNameV1::InspectScope),
                    DebugErrorStageV1::Protocol,
                    DebugErrorCodeV1::InvalidCursor,
                    message,
                );
            }
        };
        let states = self.scope_states(&scopes);
        let views: Vec<_> = scopes
            .into_iter()
            .zip(states)
            .map(|(scope, state)| ScopeViewV1 { state, scope })
            .collect();
        self.ok(
            request_id,
            DebugOperationNameV1::InspectScope,
            DebugResultV1::Scopes {
                scopes: views,
                next_cursor,
            },
        )
    }

    fn scope_states(&self, scopes: &[ExecutionScopeV1]) -> Vec<ScopeStateV1> {
        scope_states_for_records(
            self.session.transcript().records(),
            self.session.transcript().terminal_fault(),
            self.session.cursor_record_index(),
            scopes,
            self.wave_width,
        )
    }

    fn resolve_source(&self, request_id: u64, kir: KirSiteV1) -> DebugResponseV1 {
        let site = match source_map_site(&self.module, kir) {
            Ok(site) => site,
            Err(message) => {
                return self.error(
                    Some(request_id),
                    Some(DebugOperationNameV1::ResolveSource),
                    DebugErrorStageV1::Protocol,
                    DebugErrorCodeV1::InvalidRequest,
                    &message,
                );
            }
        };
        self.ok(
            request_id,
            DebugOperationNameV1::ResolveSource,
            DebugResultV1::Source {
                site: SemanticSiteViewV1 {
                    kir,
                    source: self.source_availability(site),
                },
            },
        )
    }

    fn inspect_stack(
        &self,
        request_id: u64,
        scope: ExecutionScopeSelectorV1,
        page: PageRequestV1,
    ) -> DebugResponseV1 {
        let Some(record) = self.session.current() else {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::InspectStack,
                DebugCapabilityNameV1::CallStack,
                CapabilityUnavailableReasonV1::NotCaptured,
                "the current cursor has no captured checkpoint stack",
            );
        };
        if !convert_scope_selector(scope).matches(record.invocation, self.wave_width) {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::InspectStack,
                DebugCapabilityNameV1::CallStack,
                CapabilityUnavailableReasonV1::OutsideCaptureScope,
                "the requested scope is not the current replay invocation",
            );
        }
        let frames = match self.session.stack() {
            DebugInspectionV1::Available(frames) => frames,
            DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::Stack(_)) => {
                return self.unavailable(
                    request_id,
                    DebugOperationNameV1::InspectStack,
                    DebugCapabilityNameV1::CallStack,
                    CapabilityUnavailableReasonV1::Truncated,
                    "the checkpoint stack exceeded its capture bound",
                );
            }
            DebugInspectionV1::Unavailable(_) => {
                return self.unavailable(
                    request_id,
                    DebugOperationNameV1::InspectStack,
                    DebugCapabilityNameV1::CallStack,
                    CapabilityUnavailableReasonV1::NotCaptured,
                    "call stacks are captured only at operation checkpoints",
                );
            }
        };
        let query_bytes = serde_json::to_vec(&(scope, self.cursor_sequence())).unwrap_or_default();
        let query = self.query_identity(b"inspect-stack", &query_bytes);
        let (start, end, next_cursor) = match page_bounds(page, query, frames.len()) {
            Ok(bounds) => bounds,
            Err(message) => {
                return self.error(
                    Some(request_id),
                    Some(DebugOperationNameV1::InspectStack),
                    DebugErrorStageV1::Protocol,
                    DebugErrorCodeV1::InvalidCursor,
                    message,
                );
            }
        };
        let Some(views) = frames[start..end]
            .iter()
            .map(|frame| self.stack_frame_view(frame))
            .collect::<Option<Vec<_>>>()
        else {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::InspectStack),
                DebugErrorStageV1::Backend,
                DebugErrorCodeV1::BackendFailure,
                "a captured stack frame does not belong to the admitted KIR module",
            );
        };
        let Some(snapshot) = self.current_anchor(None) else {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::InspectStack,
                DebugCapabilityNameV1::CallStack,
                CapabilityUnavailableReasonV1::NotCaptured,
                "the current cursor has no captured checkpoint anchor",
            );
        };
        self.ok(
            request_id,
            DebugOperationNameV1::InspectStack,
            DebugResultV1::Stack {
                snapshot,
                frames: views,
                next_cursor,
            },
        )
    }

    fn stack_frame_view(&self, frame: &SimulationDebugFrameV1) -> Option<StackFrameV1> {
        let function = self.module.module().functions.get(frame.function_ordinal)?;
        let block_ordinal = function
            .body
            .as_ref()?
            .blocks
            .iter()
            .position(|block| block.id == frame.block)?;
        let values = match &frame.values {
            SimulationDebugCollectionV1::Captured(values) => StackValuesAvailabilityV1::Captured {
                value_count: u64::try_from(values.len()).ok()?,
            },
            SimulationDebugCollectionV1::Unavailable { reason, .. } => {
                StackValuesAvailabilityV1::Unavailable {
                    reason: match reason {
                        fe2o3_kir_sim::SimulationDebugUnavailableReasonV1::NotCaptured => {
                            ValueUnavailableReasonV1::NotCaptured
                        }
                        _ => ValueUnavailableReasonV1::Truncated,
                    },
                }
            }
        };
        Some(StackFrameV1 {
            frame: u64::from(frame.depth).checked_add(1)?,
            function_ordinal: u64::try_from(frame.function_ordinal).ok()?,
            block_ordinal: u64::try_from(block_ordinal).ok()?,
            next_operation: frame.next_operation.map(u64::from),
            values,
        })
    }

    fn inspect_values(
        &self,
        request_id: u64,
        scope: ExecutionScopeSelectorV1,
        frame: Option<u64>,
        selector: ValueSelectorV1,
        page: PageRequestV1,
    ) -> DebugResponseV1 {
        if frame == Some(0) {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::InspectValues),
                DebugErrorStageV1::Protocol,
                DebugErrorCodeV1::InvalidRequest,
                "frame identities are one-based",
            );
        }
        if selector_requests_unsupported_root(&selector, ValueRootClassV1::Register) {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::InspectValues,
                DebugCapabilityNameV1::RegisterValues,
                CapabilityUnavailableReasonV1::NotRepresented,
                "CPU KIR simulation does not expose hardware registers",
            );
        }
        if selector_requests_unsupported_root(&selector, ValueRootClassV1::SourceVariable) {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::InspectValues,
                DebugCapabilityNameV1::SourceVariableValues,
                CapabilityUnavailableReasonV1::RequiresAuthenticatedMap,
                "source variables are not represented by the V1 source map",
            );
        }
        let Some(record) = self.session.current() else {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::InspectValues,
                DebugCapabilityNameV1::KirSsaValues,
                CapabilityUnavailableReasonV1::NotCaptured,
                "the current cursor has no operation checkpoint",
            );
        };
        let selected_scope = convert_scope_selector(scope);
        if !selected_scope.matches(record.invocation, self.wave_width) {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::InspectValues,
                DebugCapabilityNameV1::KirSsaValues,
                CapabilityUnavailableReasonV1::OutsideCaptureScope,
                "the requested scope is not the current replay invocation",
            );
        }
        let SimulationDebugRecordKindV1::Checkpoint { stack, .. } = &record.kind else {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::InspectValues,
                DebugCapabilityNameV1::KirSsaValues,
                CapabilityUnavailableReasonV1::NotCaptured,
                "SSA values are captured only at operation checkpoints",
            );
        };
        let SimulationDebugCollectionV1::Captured(frames) = stack else {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::InspectValues,
                DebugCapabilityNameV1::KirSsaValues,
                CapabilityUnavailableReasonV1::Truncated,
                "the checkpoint stack or values exceeded its capture bound",
            );
        };
        let requested_depth = match frame {
            Some(value) => match u32::try_from(value - 1) {
                Ok(depth) => Some(depth),
                Err(_) => {
                    return self.error(
                        Some(request_id),
                        Some(DebugOperationNameV1::InspectValues),
                        DebugErrorStageV1::Protocol,
                        DebugErrorCodeV1::InvalidRequest,
                        "frame identity exceeds the simulator frame range",
                    );
                }
            },
            None => None,
        };
        let mut values: Vec<DebugValueV1> = match &selector {
            ValueSelectorV1::Paths { paths } => paths
                .iter()
                .map(|path| value_for_path(path.clone(), frames))
                .collect(),
            ValueSelectorV1::All | ValueSelectorV1::Roots { .. } => frames
                .iter()
                .filter(|candidate| requested_depth.is_none_or(|depth| candidate.depth == depth))
                .flat_map(|candidate| {
                    let SimulationDebugCollectionV1::Captured(bindings) = &candidate.values else {
                        return Vec::new().into_iter();
                    };
                    bindings
                        .iter()
                        .map(|binding| value_for_binding(candidate, binding))
                        .collect::<Vec<_>>()
                        .into_iter()
                })
                .collect(),
        };
        let query_bytes = serde_json::to_vec(&(scope, frame, &selector, self.cursor_sequence()))
            .unwrap_or_default();
        let query = self.query_identity(b"inspect-values", &query_bytes);
        let (start, end, next_cursor) = match page_bounds(page, query, values.len()) {
            Ok(bounds) => bounds,
            Err(message) => {
                return self.error(
                    Some(request_id),
                    Some(DebugOperationNameV1::InspectValues),
                    DebugErrorStageV1::Protocol,
                    DebugErrorCodeV1::InvalidCursor,
                    message,
                );
            }
        };
        values = values.drain(start..end).collect();
        let anchor = self
            .current_anchor(frame)
            .expect("current checkpoint has a representable anchor");
        self.ok(
            request_id,
            DebugOperationNameV1::InspectValues,
            DebugResultV1::Values {
                snapshot: anchor,
                values,
                next_cursor,
            },
        )
    }

    fn read_memory(
        &self,
        request_id: u64,
        allocation: AllocationIdentityV1,
        byte_offset: u64,
        byte_len: u64,
    ) -> DebugResponseV1 {
        let Some(snapshot) = self.current_anchor(None) else {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::ReadMemory,
                DebugCapabilityNameV1::AllocationRelativeMemory,
                CapabilityUnavailableReasonV1::NotCaptured,
                "the current cursor has no captured operation memory snapshot",
            );
        };
        if allocation.generation != 0 {
            return self.unavailable(
                request_id,
                DebugOperationNameV1::ReadMemory,
                DebugCapabilityNameV1::AllocationRelativeMemory,
                CapabilityUnavailableReasonV1::NotRepresented,
                "CPU simulation allocations have generation zero",
            );
        }
        let (Ok(offset), Ok(length)) = (usize::try_from(byte_offset), usize::try_from(byte_len))
        else {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::ReadMemory),
                DebugErrorStageV1::Protocol,
                DebugErrorCodeV1::InvalidRequest,
                "memory read range does not fit this host",
            );
        };
        let memory = match self.session.memory(allocation.ordinal, offset, length) {
            DebugInspectionV1::Available(slice) => MemoryReadV1 {
                allocation,
                byte_offset,
                requested_bytes: byte_len,
                returned_bytes: byte_len,
                availability: MemoryAvailabilityV1::Captured {
                    address_space: protocol_address_space(slice.address_space),
                    bytes: hex_bytes(&slice.bytes),
                    initialized: initialization_bits(&slice.initialized),
                    truncated: false,
                },
            },
            DebugInspectionV1::Unavailable(reason) => MemoryReadV1 {
                allocation,
                byte_offset,
                requested_bytes: byte_len,
                returned_bytes: 0,
                availability: MemoryAvailabilityV1::Unavailable {
                    reason: inspection_unavailable_value_reason(reason),
                },
            },
        };
        self.ok(
            request_id,
            DebugOperationNameV1::ReadMemory,
            DebugResultV1::Memory { snapshot, memory },
        )
    }

    fn query_events(
        &self,
        request_id: u64,
        filter: EventFilterV1,
        page: PageRequestV1,
    ) -> DebugResponseV1 {
        let query_bytes = serde_json::to_vec(&filter).unwrap_or_default();
        let query = self.query_identity(b"query-events", &query_bytes);
        let (start, limit) = match page_window(page, query) {
            Ok(window) => window,
            Err(message) => {
                return self.error(
                    Some(request_id),
                    Some(DebugOperationNameV1::QueryEvents),
                    DebugErrorStageV1::Protocol,
                    DebugErrorCodeV1::InvalidCursor,
                    message,
                );
            }
        };
        let mut events = Vec::new();
        if events.try_reserve_exact(limit).is_err() {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::QueryEvents),
                DebugErrorStageV1::Session,
                DebugErrorCodeV1::ResourceLimit,
                "event page allocation failed",
            );
        }
        let mut total = 0_usize;
        for event in self
            .session
            .transcript()
            .records()
            .iter()
            .filter(|record| self.record_matches_event_filter(record, &filter))
            .filter_map(|record| self.event_view(record))
        {
            if total >= start && events.len() < limit {
                events.push(event);
            }
            total = total.saturating_add(1);
        }
        let next_cursor = match page_next(start, events.len(), total, query) {
            Ok(next) => next,
            Err(message) => {
                return self.error(
                    Some(request_id),
                    Some(DebugOperationNameV1::QueryEvents),
                    DebugErrorStageV1::Protocol,
                    DebugErrorCodeV1::InvalidCursor,
                    message,
                );
            }
        };
        self.ok(
            request_id,
            DebugOperationNameV1::QueryEvents,
            DebugResultV1::Events {
                events,
                next_cursor,
            },
        )
    }

    fn export_trace(&self, request_id: u64, max_bytes: u64) -> DebugResponseV1 {
        let response_bound = self
            .protocol_limits
            .max_response_line_bytes
            .saturating_sub(8 * 1024)
            / 2;
        let requested = usize::try_from(max_bytes).unwrap_or(usize::MAX);
        let byte_bound = requested.min(response_bound);
        let header = TraceHeaderV1 {
            schema: TRACE_HEADER_SCHEMA_V1,
            configuration_identity: self.configuration_identity,
            backend: DebugBackendV1::CpuKirSimulator,
            simulated: true,
            hardware_observed: false,
            performance_prediction: false,
            wave_interpretation: WaveInterpretationV1::LogicalVisualization,
        };
        let mut bytes = match serde_json::to_vec(&header) {
            Ok(bytes) => bytes,
            Err(_) => {
                return self.error(
                    Some(request_id),
                    Some(DebugOperationNameV1::ExportTrace),
                    DebugErrorStageV1::Output,
                    DebugErrorCodeV1::OutputFailure,
                    "failed to serialize the trace header",
                );
            }
        };
        bytes.push(b'\n');
        if bytes.len() > byte_bound {
            return self.error(
                Some(request_id),
                Some(DebugOperationNameV1::ExportTrace),
                DebugErrorStageV1::Protocol,
                DebugErrorCodeV1::ResourceLimit,
                "max_bytes is too small for the trace header",
            );
        }
        let mut emitted = 0_u64;
        let mut byte_truncated = false;
        for record in self.session.transcript().records() {
            let Some(event) = self.event_view(record) else {
                continue;
            };
            let line = match serde_json::to_vec(&event) {
                Ok(line) => line,
                Err(_) => {
                    return self.error(
                        Some(request_id),
                        Some(DebugOperationNameV1::ExportTrace),
                        DebugErrorStageV1::Output,
                        DebugErrorCodeV1::OutputFailure,
                        "failed to serialize a trace event",
                    );
                }
            };
            if bytes
                .len()
                .checked_add(line.len().saturating_add(1))
                .is_none_or(|total| total > byte_bound)
            {
                byte_truncated = true;
                break;
            }
            bytes.extend_from_slice(&line);
            bytes.push(b'\n');
            emitted = emitted.saturating_add(1);
        }
        let completeness = if byte_truncated {
            CaptureCompletenessV1::Truncated {
                reason: CaptureTruncationReasonV1::ByteLimit,
                emitted_events: emitted,
                dropped_events: u64::try_from(self.session.transcript().records().len())
                    .ok()
                    .map(|total| total.saturating_sub(emitted)),
            }
        } else {
            match self.session.transcript().completeness() {
                DebugTranscriptCompletenessV1::Complete => CaptureCompletenessV1::Complete,
                DebugTranscriptCompletenessV1::Truncated(reason) => {
                    CaptureCompletenessV1::Truncated {
                        reason: transcript_truncation_reason(reason),
                        emitted_events: emitted,
                        dropped_events: None,
                    }
                }
            }
        };
        let trace_identity = nonzero_identity(Sha256::digest(&bytes).into());
        self.ok(
            request_id,
            DebugOperationNameV1::ExportTrace,
            DebugResultV1::Trace {
                trace_identity,
                canonical_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                bytes: hex_bytes(&bytes),
                completeness,
            },
        )
    }

    fn event_view(&self, record: &SimulationDebugRecordV1) -> Option<DebugEventViewV1> {
        Some(DebugEventViewV1 {
            sequence: record.ordinal.saturating_add(1),
            scope: protocol_scope_for_invocation(record.invocation, self.wave_width)?,
            site: self.protocol_site(record),
            category: match record.kind {
                SimulationDebugRecordKindV1::Checkpoint { .. } => EventCategoryV1::Operation,
                SimulationDebugRecordKindV1::Memory { .. } => EventCategoryV1::Memory,
                SimulationDebugRecordKindV1::WorkgroupBarrier { .. } => EventCategoryV1::Barrier,
                SimulationDebugRecordKindV1::Fence { .. } => EventCategoryV1::Operation,
            },
            provenance: EventProvenanceV1::SimulatedObservation,
        })
    }

    fn terminal_oob_detail_v2(
        &self,
        allocation: u64,
        offset: usize,
        bytes: usize,
        allocation_bytes: usize,
    ) -> Option<&fe2o3_kir_sim::SimulationOutOfBoundsV2> {
        let DebugTerminalDetailStateV2::Captured(DebugTerminalDetailV2::OutOfBounds(detail)) =
            self.session.transcript().terminal_detail_v2()
        else {
            return None;
        };
        (detail.allocation == allocation
            && detail.offset == offset
            && detail.bytes == bytes
            && detail.allocation_bytes == allocation_bytes)
            .then_some(detail)
    }

    fn terminal_barrier_divergence_v2(
        &self,
        v1: &fe2o3_kir_sim::DivergentWorkgroupBarrierV1,
    ) -> Option<&fe2o3_kir_sim::DivergentWorkgroupBarrierV2> {
        let DebugTerminalDetailStateV2::Captured(DebugTerminalDetailV2::BarrierDivergence(detail)) =
            self.session.transcript().terminal_detail_v2()
        else {
            return None;
        };
        (detail.phase == v1.phase
            && detail
                .waiting
                .iter()
                .any(|participant| participant == &v1.waiting)
            && detail
                .exited
                .iter()
                .any(|participant| participant == &v1.exited))
        .then_some(detail)
    }

    fn diagnosis_view_v2(&self, fault: &DebugTerminalFaultV1) -> Option<DiagnosisViewV2> {
        use fe2o3_kir_sim::SimulationExecutionErrorKindV1 as ErrorKind;

        let context = self.diagnosis_context_v2(fault.invocation);
        let kir_site = fault
            .site
            .as_ref()
            .and_then(|site| self.protocol_execution_site_v2(site));
        let site = kir_site.map_or(
            DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::SiteNotRepresented,
            },
            |value| DiagnosisFactV2::Observed { value },
        );
        let source_operation = self.diagnosis_source_operation_v2(fault.site.as_ref(), kir_site);
        let (class, memory_region, barrier) = match &fault.kind {
            ErrorKind::OutOfBounds {
                allocation,
                offset,
                bytes,
                allocation_bytes,
            } => {
                let memory_region = self
                    .terminal_oob_detail_v2(*allocation, *offset, *bytes, *allocation_bytes)
                    .and_then(|detail| self.diagnosis_oob_region_v2(detail))
                    .map_or(
                        DiagnosisFactV2::Unavailable {
                            reason: DiagnosisUnavailableReasonV2::NotRepresentable,
                        },
                        |value| DiagnosisFactV2::Observed { value },
                    );
                (
                    DiagnosisClassV2::MemoryOutOfBounds,
                    memory_region,
                    DiagnosisFactV2::Unavailable {
                        reason: DiagnosisUnavailableReasonV2::NotApplicable,
                    },
                )
            }
            ErrorKind::DivergentWorkgroupBarrier(detail) => {
                let barrier = fault
                    .invocation
                    .zip(self.terminal_barrier_divergence_v2(detail))
                    .and_then(|(invocation, detail)| {
                        self.divergent_barrier_v2(invocation, detail, kir_site)
                    })
                    .map_or(
                        DiagnosisFactV2::Unavailable {
                            reason: DiagnosisUnavailableReasonV2::NotRepresentable,
                        },
                        |value| DiagnosisFactV2::Observed { value },
                    );
                (
                    DiagnosisClassV2::WorkgroupBarrierDivergence,
                    DiagnosisFactV2::Unavailable {
                        reason: DiagnosisUnavailableReasonV2::NotApplicable,
                    },
                    barrier,
                )
            }
            ErrorKind::MismatchedWorkgroupBarrier(detail) => {
                let mismatch = match detail.mismatch {
                    fe2o3_kir_sim::WorkgroupBarrierMismatchV1::Site => {
                        DiagnosisBarrierMismatchV2::Site
                    }
                    fe2o3_kir_sim::WorkgroupBarrierMismatchV1::Semantics => {
                        DiagnosisBarrierMismatchV2::Semantics
                    }
                    fe2o3_kir_sim::WorkgroupBarrierMismatchV1::SiteAndSemantics => {
                        DiagnosisBarrierMismatchV2::SiteAndSemantics
                    }
                };
                let expected_site = self.protocol_event_site_v2(&detail.expected).map_or(
                    DiagnosisFactV2::Unavailable {
                        reason: DiagnosisUnavailableReasonV2::SiteNotRepresented,
                    },
                    |value| DiagnosisFactV2::Observed { value },
                );
                let actual_semantics =
                    kir_site.and_then(|site| self.diagnosis_barrier_semantics_v2(site));
                let expected_semantics = self
                    .protocol_event_site_v2(&detail.expected)
                    .and_then(|site| self.diagnosis_barrier_semantics_v2(site));
                let expected_participants =
                    fault.invocation.and_then(active_workgroup_participants_v2);
                let expected_participant_set = fault
                    .invocation
                    .and_then(|invocation| self.expected_barrier_participant_set_v2(invocation));
                let (
                    Some(actual_semantics),
                    Some(expected_semantics),
                    Some(expected_participants),
                    Some(expected_participant_set),
                ) = (
                    actual_semantics,
                    expected_semantics,
                    expected_participants,
                    expected_participant_set,
                )
                else {
                    let mut diagnosis = DiagnosisViewV2 {
                        sequence: fault.ordinal.saturating_add(1),
                        class: DiagnosisClassV2::WorkgroupBarrierMismatch,
                        input: self.diagnosis_input.clone(),
                        context,
                        site,
                        source_operation,
                        memory_region: DiagnosisFactV2::Unavailable {
                            reason: DiagnosisUnavailableReasonV2::NotApplicable,
                        },
                        barrier: DiagnosisFactV2::Unavailable {
                            reason: DiagnosisUnavailableReasonV2::NotRepresentable,
                        },
                        evidence: DiagnosisEvidenceManifestV2::unsealed().ok()?,
                    };
                    diagnosis
                        .seal_evidence_v2(
                            self.session_view(),
                            self.diagnosis_completeness_v2(),
                            self.diagnosis_retained_evidence_v2(fault)?,
                        )
                        .ok()?;
                    return Some(diagnosis);
                };
                (
                    DiagnosisClassV2::WorkgroupBarrierMismatch,
                    DiagnosisFactV2::Unavailable {
                        reason: DiagnosisUnavailableReasonV2::NotApplicable,
                    },
                    DiagnosisFactV2::Observed {
                        value: DiagnosisBarrierV2::Mismatch {
                            phase: DiagnosisFactV2::Observed {
                                value: detail.phase,
                            },
                            semantics: DiagnosisFactV2::Declared {
                                value: actual_semantics,
                            },
                            expected_semantics: DiagnosisFactV2::Declared {
                                value: expected_semantics,
                            },
                            lds_epoch: diagnosis_failed_lds_epoch_v2(detail.phase),
                            expected_participants: DiagnosisFactV2::Inferred {
                                value: expected_participants,
                                basis: DiagnosisInferenceBasisV2::LaunchGeometry,
                            },
                            expected_participant_set: DiagnosisFactV2::Inferred {
                                value: expected_participant_set,
                                basis: DiagnosisInferenceBasisV2::LaunchGeometry,
                            },
                            mismatch: DiagnosisFactV2::Observed { value: mismatch },
                            expected_site,
                        },
                    },
                )
            }
            _ => return None,
        };
        let mut diagnosis = DiagnosisViewV2 {
            sequence: fault.ordinal.saturating_add(1),
            class,
            input: self.diagnosis_input.clone(),
            context,
            site,
            source_operation,
            memory_region,
            barrier,
            evidence: DiagnosisEvidenceManifestV2::unsealed().ok()?,
        };
        diagnosis
            .seal_evidence_v2(
                self.session_view(),
                self.diagnosis_completeness_v2(),
                self.diagnosis_retained_evidence_v2(fault)?,
            )
            .ok()?;
        Some(diagnosis)
    }

    fn diagnosis_retained_evidence_v2(
        &self,
        fault: &DebugTerminalFaultV1,
    ) -> Option<DiagnosisRetainedEvidenceV2> {
        use fe2o3_kir_sim::SimulationExecutionErrorKindV1 as ErrorKind;

        let sequence = fault.ordinal.checked_add(1)?;
        let invocation = fault
            .invocation
            .map(|invocation| DiagnosisTerminalInvocationRecordV2 {
                global: invocation.global,
                workgroup: invocation.workgroup,
                local: invocation.local,
                workgroup_size: invocation.workgroup_size,
                launch_extent: invocation.launch_extent,
            });
        let site = fault
            .site
            .as_ref()
            .and_then(|site| self.protocol_execution_site_v2(site));
        let (class, payload, transcript_barrier) = match &fault.kind {
            ErrorKind::OutOfBounds {
                allocation,
                offset,
                bytes,
                allocation_bytes,
            } => {
                let requested_offset = u64::try_from(*offset).ok()?;
                let requested_bytes = u64::try_from(*bytes).ok()?;
                let allocation_bytes_u64 = u64::try_from(*allocation_bytes).ok()?;
                let abi_view = self
                    .terminal_oob_detail_v2(*allocation, *offset, *bytes, *allocation_bytes)
                    .and_then(|detail| {
                        let region = self.diagnosis_oob_region_v2(detail)?;
                        let DiagnosisFactV2::Declared {
                            value: allocation_contract,
                        } = region.allocation_contract
                        else {
                            return None;
                        };
                        let DiagnosisFactV2::Declared {
                            value: abi_argument,
                        } = region.abi_argument
                        else {
                            return None;
                        };
                        Some(DiagnosisTerminalAbiViewRecordV2 {
                            allocation_contract,
                            abi_argument,
                            legal_offset: region.legal_offset,
                            legal_bytes: region.legal_bytes,
                        })
                    });
                (
                    DiagnosisClassV2::MemoryOutOfBounds,
                    DiagnosisTerminalPayloadRecordV2::MemoryOutOfBounds {
                        allocation: AllocationIdentityV1 {
                            ordinal: *allocation,
                            generation: 0,
                        },
                        requested_offset,
                        requested_bytes,
                        allocation_bytes: allocation_bytes_u64,
                        abi_view,
                    },
                    None,
                )
            }
            ErrorKind::DivergentWorkgroupBarrier(detail) => {
                let (waiting, exited) =
                    self.terminal_barrier_divergence_v2(detail)
                        .map_or((None, None), |expanded| {
                            let mut waiting = Vec::new();
                            let mut exited = Vec::new();
                            if expanded.waiting.len() > 1_024
                                || expanded.exited.len() > 1_024
                                || waiting.try_reserve_exact(expanded.waiting.len()).is_err()
                                || exited.try_reserve_exact(expanded.exited.len()).is_err()
                            {
                                return (None, None);
                            }
                            waiting.extend(
                                expanded.waiting.iter().map(|participant| participant.local),
                            );
                            exited.extend(
                                expanded.exited.iter().map(|participant| participant.local),
                            );
                            waiting.sort_unstable();
                            exited.sort_unstable();
                            (Some(waiting), Some(exited))
                        });
                let workgroup = fault.invocation?.workgroup;
                let mut arrivals = Vec::new();
                for record in self.session.transcript().records().iter().filter(|record| {
                    record.invocation.workgroup == workgroup
                        && matches!(
                            record.kind,
                            SimulationDebugRecordKindV1::WorkgroupBarrier {
                                action: SimulationDebugBarrierActionV1::Arrive,
                                phase,
                                ..
                            } if phase == detail.phase
                        )
                }) {
                    if arrivals.len() == 1_024 || arrivals.try_reserve_exact(1).is_err() {
                        return None;
                    }
                    arrivals.push(DiagnosisBarrierArrivalEvidenceRecordV2 {
                        sequence: record.ordinal.checked_add(1)?,
                        local: record.invocation.local,
                        site: self.protocol_site(record),
                    });
                }
                (
                    DiagnosisClassV2::WorkgroupBarrierDivergence,
                    DiagnosisTerminalPayloadRecordV2::WorkgroupBarrierDivergence {
                        phase: detail.phase,
                        waiting_representative: detail.waiting.local,
                        exited_representative: detail.exited.local,
                        waiting,
                        exited,
                    },
                    Some(DiagnosisBarrierTranscriptEvidenceV2 {
                        phase: detail.phase,
                        workgroup,
                        arrivals,
                    }),
                )
            }
            ErrorKind::MismatchedWorkgroupBarrier(detail) => {
                let mismatch = match detail.mismatch {
                    fe2o3_kir_sim::WorkgroupBarrierMismatchV1::Site => {
                        DiagnosisBarrierMismatchV2::Site
                    }
                    fe2o3_kir_sim::WorkgroupBarrierMismatchV1::Semantics => {
                        DiagnosisBarrierMismatchV2::Semantics
                    }
                    fe2o3_kir_sim::WorkgroupBarrierMismatchV1::SiteAndSemantics => {
                        DiagnosisBarrierMismatchV2::SiteAndSemantics
                    }
                };
                (
                    DiagnosisClassV2::WorkgroupBarrierMismatch,
                    DiagnosisTerminalPayloadRecordV2::WorkgroupBarrierMismatch {
                        phase: detail.phase,
                        mismatch,
                        expected_site: self.protocol_event_site_v2(&detail.expected),
                    },
                    None,
                )
            }
            _ => return None,
        };
        Some(DiagnosisRetainedEvidenceV2 {
            terminal: DiagnosisTerminalEvidenceRecordV2 {
                sequence,
                class,
                invocation,
                site,
                payload,
            },
            transcript: DiagnosisTranscriptEvidenceRecordV2 {
                completeness: self.diagnosis_completeness_v2(),
                barrier: transcript_barrier,
            },
        })
    }

    fn diagnosis_context_v2(
        &self,
        invocation: Option<SimulationInvocationV1>,
    ) -> DiagnosisExecutionContextV2 {
        let dispatch = DiagnosisFactV2::Declared {
            value: self.diagnosis_dispatch,
        };
        let Some(invocation) = invocation else {
            return DiagnosisExecutionContextV2 {
                dispatch,
                workgroup: DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::MissingInvocation,
                },
                workitem: DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::MissingInvocation,
                },
                wave: DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::MissingInvocation,
                },
                lane: DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::MissingInvocation,
                },
            };
        };
        let hierarchy = hierarchy_for_invocation_v1(invocation, self.wave_width);
        DiagnosisExecutionContextV2 {
            dispatch,
            workgroup: DiagnosisFactV2::Observed {
                value: invocation.workgroup,
            },
            workitem: DiagnosisFactV2::Observed {
                value: DiagnosisWorkitemV2 {
                    global: invocation.global,
                    local: invocation.local,
                },
            },
            wave: DiagnosisFactV2::Inferred {
                value: DiagnosisLogicalWaveV2 {
                    wave: hierarchy.wave,
                    width: self.wave_width.lanes(),
                    active_mask: hierarchy.active_mask,
                },
                basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
            },
            lane: DiagnosisFactV2::Inferred {
                value: hierarchy.lane,
                basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
            },
        }
    }

    fn diagnosis_fault_matches_scope_v2(
        &self,
        fault: &DebugTerminalFaultV1,
        scope: ExecutionScopeSelectorV1,
    ) -> bool {
        if matches!(scope, ExecutionScopeSelectorV1::Dispatch) {
            return true;
        }
        let Some(invocation) = fault.invocation else {
            return false;
        };
        let selector = convert_scope_selector(scope);
        let fe2o3_kir_sim::SimulationExecutionErrorKindV1::DivergentWorkgroupBarrier(detail) =
            &fault.kind
        else {
            return selector.matches(invocation, self.wave_width);
        };
        let Some(detail) = self.terminal_barrier_divergence_v2(detail) else {
            return false;
        };
        detail
            .waiting
            .iter()
            .chain(&detail.exited)
            .map(|participant| participant.local)
            .filter_map(|local| invocation_at_local_v2(invocation, local))
            .any(|participant| selector.matches(participant, self.wave_width))
    }

    fn divergent_barrier_v2(
        &self,
        invocation: SimulationInvocationV1,
        detail: &fe2o3_kir_sim::DivergentWorkgroupBarrierV2,
        site: Option<KirSiteV1>,
    ) -> Option<DiagnosisBarrierV2> {
        let expected = active_workgroup_participants_v2(invocation)?;
        let semantics = self.diagnosis_barrier_semantics_v2(site?)?;
        let expected_participant_set = self.expected_barrier_participant_set_v2(invocation)?;
        let waiting_participants = self.barrier_participant_set_v2(
            invocation,
            detail.waiting.iter().map(|participant| participant.local),
            true,
        )?;
        let exited_participants = self.barrier_participant_set_v2(
            invocation,
            detail.exited.iter().map(|participant| participant.local),
            true,
        )?;
        let mut arrived_participants = Vec::new();
        arrived_participants
            .try_reserve_exact(waiting_participants.len())
            .ok()?;
        arrived_participants.extend_from_slice(&waiting_participants);
        let observed_arrivals = match self.session.transcript().completeness() {
            DebugTranscriptCompletenessV1::Complete => {
                let arrivals = self
                    .session
                    .transcript()
                    .records()
                    .iter()
                    .filter(|record| {
                        record.invocation.workgroup == invocation.workgroup
                            && matches!(
                                record.kind,
                                SimulationDebugRecordKindV1::WorkgroupBarrier {
                                    action: SimulationDebugBarrierActionV1::Arrive,
                                    phase,
                                    ..
                                } if phase == detail.phase
                            )
                    })
                    .count();
                match u32::try_from(arrivals) {
                    Ok(arrivals) if arrivals != 0 => DiagnosisFactV2::Observed { value: arrivals },
                    _ => DiagnosisFactV2::Unavailable {
                        reason: DiagnosisUnavailableReasonV2::NotCaptured,
                    },
                }
            }
            DebugTranscriptCompletenessV1::Truncated(_) => DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::TranscriptTruncated,
            },
        };
        Some(DiagnosisBarrierV2::Divergence {
            phase: DiagnosisFactV2::Observed {
                value: detail.phase,
            },
            semantics: DiagnosisFactV2::Declared { value: semantics },
            lds_epoch: diagnosis_failed_lds_epoch_v2(detail.phase),
            observed_arrivals,
            expected_participants: DiagnosisFactV2::Inferred {
                value: expected,
                basis: DiagnosisInferenceBasisV2::LaunchGeometry,
            },
            expected_participant_set: DiagnosisFactV2::Inferred {
                value: expected_participant_set,
                basis: DiagnosisInferenceBasisV2::LaunchGeometry,
            },
            arrived_participants: DiagnosisFactV2::Observed {
                value: arrived_participants,
            },
            waiting_participants: DiagnosisFactV2::Observed {
                value: waiting_participants,
            },
            exited_participants: DiagnosisFactV2::Observed {
                value: exited_participants,
            },
        })
    }

    fn diagnosis_oob_region_v2(
        &self,
        detail: &fe2o3_kir_sim::SimulationOutOfBoundsV2,
    ) -> Option<DiagnosisMemoryRegionV2> {
        let abi_view = detail.abi_view?;
        let stored_contract = self.diagnosis_allocations.get(&detail.allocation)?;
        let mut abi_arguments = Vec::new();
        abi_arguments
            .try_reserve_exact(stored_contract.abi_arguments.len())
            .ok()?;
        abi_arguments.extend_from_slice(&stored_contract.abi_arguments);
        let contract = DiagnosisAllocationContractV2 {
            address_space: stored_contract.address_space,
            access: stored_contract.access,
            alignment: stored_contract.alignment,
            allocation_bytes: stored_contract.allocation_bytes,
            abi_arguments,
        };
        let argument = *contract
            .abi_arguments
            .iter()
            .find(|argument| argument.ordinal == abi_view.argument_ordinal)?;
        let requested_offset = u64::try_from(detail.offset).ok()?;
        let requested_bytes = u64::try_from(detail.bytes).ok()?;
        let allocation_bytes = u64::try_from(detail.allocation_bytes).ok()?;
        let legal_offset = u64::try_from(detail.legal_lower_bound).ok()?;
        let legal_end = u64::try_from(detail.legal_upper_bound).ok()?;
        let legal_bytes = legal_end.checked_sub(legal_offset)?;
        let view_offset = u64::try_from(abi_view.byte_offset).ok()?;
        let view_bytes = u64::try_from(abi_view.byte_len).ok()?;
        if detail.allocation == 0
            || requested_bytes == 0
            || contract.allocation_bytes != allocation_bytes
            || legal_offset != view_offset
            || legal_bytes != view_bytes
            || argument.view_offset != view_offset
            || argument.view_bytes != view_bytes
            || argument.address_space != contract.address_space
            || !diagnosis_access_satisfies_v2(argument.access, argument.supplied_access)
            || (argument.backing.is_none() && argument.supplied_access != contract.access)
            || (argument.backing.is_some()
                && !diagnosis_access_satisfies_v2(argument.supplied_access, contract.access))
        {
            return None;
        }
        let requested_end = requested_offset.checked_add(requested_bytes)?;
        if requested_offset >= legal_offset && requested_end <= legal_end {
            return None;
        }
        let relative = requested_offset.checked_sub(legal_offset)?;
        let element_bytes = diagnosis_protocol_scalar_bytes_v2(argument.element);
        if element_bytes == 0
            || requested_bytes != u64::from(element_bytes)
            || relative % u64::from(element_bytes) != 0
            || !legal_offset.is_multiple_of(u64::from(element_bytes))
            || !legal_bytes.is_multiple_of(u64::from(element_bytes))
        {
            return None;
        }
        Some(DiagnosisMemoryRegionV2 {
            allocation: AllocationIdentityV1 {
                ordinal: detail.allocation,
                generation: 0,
            },
            requested_offset,
            requested_bytes,
            legal_offset,
            legal_bytes,
            allocation_bytes,
            allocation_contract: DiagnosisFactV2::Declared { value: contract },
            abi_argument: DiagnosisFactV2::Declared { value: argument },
            logical_element: DiagnosisFactV2::Inferred {
                value: DiagnosisLogicalElementV2 {
                    argument_ordinal: argument.ordinal,
                    element: argument.element,
                    element_bytes,
                    element_index: relative / u64::from(element_bytes),
                },
                basis: DiagnosisInferenceBasisV2::AbiViewBounds,
            },
            legal_bounds: DiagnosisFactV2::Inferred {
                value: DiagnosisLegalBoundsPropertyV2 {
                    argument_ordinal: argument.ordinal,
                    legal_offset,
                    legal_bytes,
                    requested_offset,
                    requested_bytes,
                    satisfied: false,
                },
                basis: DiagnosisInferenceBasisV2::AbiViewBounds,
            },
        })
    }

    fn diagnosis_barrier_semantics_v2(
        &self,
        site: KirSiteV1,
    ) -> Option<DiagnosisBarrierSemanticsV2> {
        let function = self
            .module
            .module()
            .functions
            .get(usize::try_from(site.function_ordinal).ok()?)?;
        let body = function.body.as_ref()?;
        let block = body.blocks.get(usize::try_from(site.block_ordinal).ok()?)?;
        let KirSitePointV1::Operation { operation_ordinal } = site.point else {
            return None;
        };
        let operation = block
            .operations
            .get(usize::try_from(operation_ordinal).ok()?)?;
        let OperationKind::WorkgroupBarrier(barrier) = &operation.kind else {
            return None;
        };
        Some(DiagnosisBarrierSemanticsV2 {
            memory_scope: diagnosis_synchronization_scope_v2(barrier.memory_scope),
            ordering: diagnosis_memory_ordering_v2(barrier.semantics.ordering),
            address_spaces: barrier
                .semantics
                .address_spaces
                .iter()
                .copied()
                .map(diagnosis_address_space_v2)
                .collect(),
        })
    }

    fn diagnosis_source_operation_v2(
        &self,
        execution_site: Option<&fe2o3_kir_sim::SimulationSiteV1>,
        kir_site: Option<KirSiteV1>,
    ) -> DiagnosisFactV2<DiagnosisSourceOperationV2> {
        let map = match &self.diagnosis_input.source_map_v2 {
            DiagnosisFactV2::Declared { value } => *value,
            DiagnosisFactV2::Unavailable { reason } => {
                return DiagnosisFactV2::Unavailable { reason: *reason };
            }
            _ => {
                return DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::RequiresSourceMapV2,
                };
            }
        };
        let (Some(execution_site), Some(kir_site)) = (execution_site, kir_site) else {
            return DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::SiteNotRepresented,
            };
        };
        let KirSitePointV1::Operation { operation_ordinal } = kir_site.point else {
            return DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::SourceSiteAbsent,
            };
        };
        let Ok(function_ordinal) = usize::try_from(kir_site.function_ordinal) else {
            return DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::SiteNotRepresented,
            };
        };
        let Ok(operation) = u32::try_from(operation_ordinal) else {
            return DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::SiteNotRepresented,
            };
        };
        match self.session.resolve_source_site(SimulationDebugSiteV1 {
            function_ordinal,
            block: execution_site.block,
            operation,
        }) {
            DebugInspectionV1::Available(DebugSourceResolutionV1::Resolved { span, .. }) => {
                let location = SourceLocationV1 {
                    map_identity: map.identity,
                    provenance: map.provenance,
                    file_identity: nonzero_identity(span.file),
                    byte_start: span.byte_start,
                    byte_end: span.byte_end,
                };
                let Some((member_index, _member)) = self
                    .diagnosis_source_members
                    .iter()
                    .enumerate()
                    .find(|(_, member)| member.kir_site == kir_site && member.location == location)
                else {
                    return DiagnosisFactV2::Unavailable {
                        reason: DiagnosisUnavailableReasonV2::SourceSiteAbsent,
                    };
                };
                let mut identities = Vec::new();
                if identities
                    .try_reserve_exact(self.diagnosis_source_members.len())
                    .is_err()
                {
                    return DiagnosisFactV2::Unavailable {
                        reason: DiagnosisUnavailableReasonV2::NotRepresentable,
                    };
                }
                identities.extend(
                    self.diagnosis_source_members
                        .iter()
                        .map(|member| member.identity),
                );
                let Ok(membership) =
                    diagnosis_source_map_membership_proof_v2(&identities, member_index)
                else {
                    return DiagnosisFactV2::Unavailable {
                        reason: DiagnosisUnavailableReasonV2::SourceSiteAbsent,
                    };
                };
                DiagnosisFactV2::Declared {
                    value: DiagnosisSourceOperationV2 {
                        bundle_subject_identity: map.bundle_subject_identity,
                        kir_site,
                        location,
                        membership,
                    },
                }
            }
            DebugInspectionV1::Available(DebugSourceResolutionV1::ManyToOne) => {
                DiagnosisFactV2::Unavailable {
                    reason: DiagnosisUnavailableReasonV2::SourceSiteAmbiguous,
                }
            }
            DebugInspectionV1::Available(
                DebugSourceResolutionV1::Absent | DebugSourceResolutionV1::Eliminated,
            )
            | DebugInspectionV1::Unavailable(_) => DiagnosisFactV2::Unavailable {
                reason: DiagnosisUnavailableReasonV2::SourceSiteAbsent,
            },
        }
    }

    fn barrier_participant_v2(
        &self,
        invocation: SimulationInvocationV1,
        local: [u32; 3],
        observed_local: bool,
    ) -> Option<DiagnosisBarrierParticipantV2> {
        let participant = invocation_at_local_v2(invocation, local)?;
        let linear = u64::from(local[0]).checked_add(
            u64::from(invocation.workgroup_size[0]).checked_mul(
                u64::from(local[1]).checked_add(
                    u64::from(invocation.workgroup_size[1]).checked_mul(u64::from(local[2]))?,
                )?,
            )?,
        )?;
        let width = u64::from(self.wave_width.lanes());
        Some(DiagnosisBarrierParticipantV2 {
            local_workitem: if observed_local {
                DiagnosisFactV2::Observed { value: local }
            } else {
                DiagnosisFactV2::Inferred {
                    value: local,
                    basis: DiagnosisInferenceBasisV2::LaunchGeometry,
                }
            },
            global_workitem: DiagnosisFactV2::Inferred {
                value: participant.global,
                basis: DiagnosisInferenceBasisV2::LaunchGeometry,
            },
            wave: DiagnosisFactV2::Inferred {
                value: u32::try_from(linear / width).ok()?,
                basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
            },
            lane: DiagnosisFactV2::Inferred {
                value: u16::try_from(linear % width).ok()?,
                basis: DiagnosisInferenceBasisV2::LogicalWavePartition,
            },
        })
    }

    fn barrier_participant_set_v2(
        &self,
        invocation: SimulationInvocationV1,
        locals: impl IntoIterator<Item = [u32; 3]>,
        observed_local: bool,
    ) -> Option<Vec<DiagnosisBarrierParticipantV2>> {
        let mut retained = Vec::new();
        for local in locals {
            if retained.len() == 1_024 || retained.try_reserve_exact(1).is_err() {
                return None;
            }
            retained.push(local);
        }
        retained.sort_unstable();
        if retained.windows(2).any(|pair| pair[0] == pair[1]) {
            return None;
        }
        let mut participants = Vec::new();
        participants.try_reserve_exact(retained.len()).ok()?;
        for local in retained {
            participants.push(self.barrier_participant_v2(invocation, local, observed_local)?);
        }
        Some(participants)
    }

    fn expected_barrier_participant_set_v2(
        &self,
        invocation: SimulationInvocationV1,
    ) -> Option<Vec<DiagnosisBarrierParticipantV2>> {
        let expected = active_workgroup_participants_v2(invocation)?;
        let expected = usize::try_from(expected).ok()?;
        if expected > 1_024 {
            return None;
        }
        let mut locals = Vec::new();
        locals.try_reserve_exact(expected).ok()?;
        let active_extent = active_workgroup_extent_v2(invocation)?;
        for z in 0..active_extent[2] {
            for y in 0..active_extent[1] {
                for x in 0..active_extent[0] {
                    locals.push([x, y, z]);
                }
            }
        }
        self.barrier_participant_set_v2(invocation, locals, false)
    }

    fn diagnosis_completeness_v2(&self) -> CaptureCompletenessV1 {
        let emitted_events =
            u64::try_from(self.session.transcript().records().len()).unwrap_or(u64::MAX);
        match self.session.transcript().completeness() {
            DebugTranscriptCompletenessV1::Complete => CaptureCompletenessV1::Complete,
            DebugTranscriptCompletenessV1::Truncated(reason) => CaptureCompletenessV1::Truncated {
                reason: transcript_truncation_reason(reason),
                emitted_events,
                dropped_events: None,
            },
        }
    }

    fn protocol_execution_site_v2(
        &self,
        site: &fe2o3_kir_sim::SimulationSiteV1,
    ) -> Option<KirSiteV1> {
        let function_ordinal = self
            .module
            .module()
            .functions
            .iter()
            .position(|function| function.id == site.function)?;
        let body = self.module.module().functions[function_ordinal]
            .body
            .as_ref()?;
        let block_ordinal = body
            .blocks
            .iter()
            .position(|block| block.id == site.block)?;
        Some(KirSiteV1 {
            function_ordinal: u64::try_from(function_ordinal).ok()?,
            block_ordinal: u64::try_from(block_ordinal).ok()?,
            point: site
                .operation
                .map_or(KirSitePointV1::Terminator, |operation| {
                    KirSitePointV1::Operation {
                        operation_ordinal: u64::from(operation),
                    }
                }),
        })
    }

    fn protocol_event_site_v2(
        &self,
        site: &fe2o3_kir_sim::SimulationEventSiteV1,
    ) -> Option<KirSiteV1> {
        let function = self.module.module().functions.get(site.function_ordinal)?;
        let body = function.body.as_ref()?;
        let block_ordinal = body
            .blocks
            .iter()
            .position(|block| block.id == site.block)?;
        Some(KirSiteV1 {
            function_ordinal: u64::try_from(site.function_ordinal).ok()?,
            block_ordinal: u64::try_from(block_ordinal).ok()?,
            point: site
                .operation
                .map_or(KirSitePointV1::Terminator, |operation| {
                    KirSitePointV1::Operation {
                        operation_ordinal: u64::from(operation),
                    }
                }),
        })
    }

    fn record_matches_event_filter(
        &self,
        record: &SimulationDebugRecordV1,
        filter: &EventFilterV1,
    ) -> bool {
        let sequence = record.ordinal.saturating_add(1);
        if filter.sequence_start.is_some_and(|start| sequence < start)
            || filter.sequence_end.is_some_and(|end| sequence > end)
        {
            return false;
        }
        if filter.scope.is_some_and(|scope| {
            !convert_scope_selector(scope).matches(record.invocation, self.wave_width)
        }) {
            return false;
        }
        if filter
            .site
            .is_some_and(|site| self.protocol_site(record) != Some(site))
        {
            return false;
        }
        if let Some(allocation) = filter.allocation
            && (allocation.generation != 0
                || !matches!(
                    record.kind,
                    SimulationDebugRecordKindV1::Memory {
                        allocation: actual,
                        ..
                    } if actual == allocation.ordinal
                ))
        {
            return false;
        }
        if filter.category.is_some_and(|category| {
            category
                != match record.kind {
                    SimulationDebugRecordKindV1::Checkpoint { .. } => EventCategoryV1::Operation,
                    SimulationDebugRecordKindV1::Memory { .. } => EventCategoryV1::Memory,
                    SimulationDebugRecordKindV1::WorkgroupBarrier { .. } => {
                        EventCategoryV1::Barrier
                    }
                    SimulationDebugRecordKindV1::Fence { .. } => EventCategoryV1::Operation,
                }
        }) {
            return false;
        }
        true
    }

    fn protocol_site(&self, record: &SimulationDebugRecordV1) -> Option<KirSiteV1> {
        let function = self
            .module
            .module()
            .functions
            .get(record.site.function_ordinal)?;
        let body = function.body.as_ref()?;
        let block_ordinal = body
            .blocks
            .iter()
            .position(|block| block.id == record.site.block)?;
        Some(KirSiteV1 {
            function_ordinal: u64::try_from(record.site.function_ordinal).ok()?,
            block_ordinal: u64::try_from(block_ordinal).ok()?,
            point: KirSitePointV1::Operation {
                operation_ordinal: u64::from(record.site.operation),
            },
        })
    }

    fn source_availability(
        &self,
        site: fe2o3_kir_sim::SimulationDebugSiteV1,
    ) -> SourceSiteAvailabilityV1 {
        let Some(map_identity) = self.source_map_identity else {
            return SourceSiteAvailabilityV1::Unavailable {
                reason: SourceSiteUnavailableReasonV1::RequiresAuthenticatedMap,
            };
        };
        let Some(provenance) = self.source_map_provenance else {
            return SourceSiteAvailabilityV1::Unavailable {
                reason: SourceSiteUnavailableReasonV1::RequiresAuthenticatedMap,
            };
        };
        match self.session.resolve_source_site(site) {
            DebugInspectionV1::Available(DebugSourceResolutionV1::Resolved { span, .. }) => {
                SourceSiteAvailabilityV1::Resolved {
                    location: SourceLocationV1 {
                        map_identity,
                        provenance,
                        file_identity: nonzero_identity(span.file),
                        byte_start: span.byte_start,
                        byte_end: span.byte_end,
                    },
                }
            }
            DebugInspectionV1::Available(DebugSourceResolutionV1::Absent) => {
                SourceSiteAvailabilityV1::Unavailable {
                    reason: SourceSiteUnavailableReasonV1::Absent,
                }
            }
            DebugInspectionV1::Available(DebugSourceResolutionV1::Eliminated) => {
                SourceSiteAvailabilityV1::Unavailable {
                    reason: SourceSiteUnavailableReasonV1::OptimizedOut,
                }
            }
            DebugInspectionV1::Available(DebugSourceResolutionV1::ManyToOne) => {
                SourceSiteAvailabilityV1::Unavailable {
                    reason: SourceSiteUnavailableReasonV1::ManyToOne,
                }
            }
            DebugInspectionV1::Unavailable(_) => SourceSiteAvailabilityV1::Unavailable {
                reason: SourceSiteUnavailableReasonV1::RequiresAuthenticatedMap,
            },
        }
    }

    fn current_anchor(&self, frame: Option<u64>) -> Option<DebugSnapshotAnchorV1> {
        let record = self.session.current()?;
        if !matches!(record.kind, SimulationDebugRecordKindV1::Checkpoint { .. }) {
            return None;
        }
        Some(DebugSnapshotAnchorV1 {
            cursor: self.session_view().cursor,
            scope: protocol_scope_for_invocation(record.invocation, self.wave_width)?,
            site: self.protocol_site(record).map(|kir| SemanticSiteViewV1 {
                kir,
                source: self.source_availability(record.site),
            }),
            frame,
            occurrence: frame.map(|_| 1),
        })
    }

    fn snapshot_availability(&self) -> SnapshotAvailabilityV1 {
        let Some(anchor) = self.current_anchor(None) else {
            return SnapshotAvailabilityV1::Unavailable {
                reason: SnapshotUnavailableReasonV1::NotCaptured,
            };
        };
        let values = current_ssa_values(&self.session, self.protocol_limits.max_response_items);
        SnapshotAvailabilityV1::Captured {
            snapshot: Box::new(DebugSnapshotV1 {
                anchor,
                stop: self.last_stop.clone().unwrap_or(StopViewV1 {
                    reason: StopReasonV1::Step,
                    breakpoint_id: None,
                    watchpoint_id: None,
                    outcome: ExecutionOutcomeV1::Active,
                    exact: true,
                }),
                values,
            }),
        }
    }

    fn query_identity(&self, label: &[u8], query: &[u8]) -> OpaqueIdentityV1 {
        let mut digest = Sha256::new();
        digest.update(b"fe2o3-debug-page-v1\0");
        digest.update(label);
        digest.update(self.configuration_identity.as_bytes());
        digest.update(self.revision.to_le_bytes());
        digest.update(query);
        nonzero_identity(digest.finalize().into())
    }
}

fn invocation_at_local_v2(
    invocation: SimulationInvocationV1,
    local: [u32; 3],
) -> Option<SimulationInvocationV1> {
    let mut global = [0_u64; 3];
    for (axis, coordinate) in global.iter_mut().enumerate() {
        if local[axis] >= invocation.workgroup_size[axis] {
            return None;
        }
        *coordinate = invocation.workgroup[axis]
            .checked_mul(u64::from(invocation.workgroup_size[axis]))?
            .checked_add(u64::from(local[axis]))?;
        if *coordinate >= invocation.launch_extent[axis] {
            return None;
        }
    }
    Some(SimulationInvocationV1 {
        global,
        local,
        ..invocation
    })
}

fn active_workgroup_participants_v2(invocation: SimulationInvocationV1) -> Option<u32> {
    let declared_volume = invocation
        .workgroup_size
        .into_iter()
        .try_fold(1_u64, |volume, size| volume.checked_mul(u64::from(size)))?;
    if declared_volume > 1_024 {
        return None;
    }
    let active_extent = active_workgroup_extent_v2(invocation)?;
    let mut participants = 1_u64;
    for active in active_extent {
        participants = participants.checked_mul(u64::from(active))?;
    }
    u32::try_from(participants).ok()
}

fn active_workgroup_extent_v2(invocation: SimulationInvocationV1) -> Option<[u32; 3]> {
    let mut active_extent = [0_u32; 3];
    for (axis, coordinate) in invocation.workgroup.into_iter().enumerate() {
        let start = coordinate.checked_mul(u64::from(invocation.workgroup_size[axis]))?;
        let remaining = invocation.launch_extent[axis].checked_sub(start)?;
        let active = remaining.min(u64::from(invocation.workgroup_size[axis]));
        if active == 0 {
            return None;
        }
        active_extent[axis] = u32::try_from(active).ok()?;
    }
    Some(active_extent)
}

#[derive(Debug)]
struct PendingDiagnosisAllocationV2 {
    address_space: AddressSpaceV1,
    access: DiagnosisAccessModeV2,
    alignment: u32,
    allocation_bytes: u64,
    abi_arguments: Vec<DiagnosisAbiArgumentV2>,
}

fn diagnosis_initial_allocations_v2(
    input: &AdmittedSimulationInputV1,
) -> Result<BTreeMap<u64, DiagnosisAllocationContractV2>, String> {
    let module = input.module.module();
    let kernel = module
        .kernels
        .iter()
        .find(|kernel| kernel.id == input.request.kernel)
        .ok_or_else(|| "admitted diagnosis kernel is missing".to_owned())?;
    let entry = module
        .function(&kernel.entry)
        .ok_or_else(|| "admitted diagnosis kernel entry is missing".to_owned())?;
    if entry.signature.parameters.len() != input.request.arguments.len() {
        return Err("admitted diagnosis ABI argument count changed".to_owned());
    }

    let mut pending = BTreeMap::new();
    let mut shared_allocations = BTreeMap::new();
    let mut next_allocation = 1_u64;
    for shared in &input.request.shared_buffers {
        let allocation = next_allocation;
        next_allocation = next_allocation
            .checked_add(1)
            .ok_or_else(|| "diagnosis allocation identity overflow".to_owned())?;
        if shared_allocations.insert(shared.id, allocation).is_some() {
            return Err("admitted diagnosis shared allocation is duplicated".to_owned());
        }
        pending.insert(
            allocation,
            PendingDiagnosisAllocationV2 {
                address_space: AddressSpaceV1::Global,
                access: diagnosis_access_v2(shared.buffer.access())?,
                alignment: shared.buffer.alignment(),
                allocation_bytes: u64::try_from(shared.buffer.bytes().len())
                    .map_err(|_| "diagnosis allocation length does not fit u64".to_owned())?,
                abi_arguments: Vec::new(),
            },
        );
    }

    for (ordinal, (argument, ty)) in input
        .request
        .arguments
        .iter()
        .zip(&entry.signature.parameters)
        .enumerate()
    {
        let (allocation, backing, view_offset, view_bytes, element, argument_access) =
            match argument {
                SimulationArgumentV1::Scalar(_) => continue,
                SimulationArgumentV1::Buffer(buffer) => {
                    let allocation = next_allocation;
                    next_allocation = next_allocation
                        .checked_add(1)
                        .ok_or_else(|| "diagnosis allocation identity overflow".to_owned())?;
                    let bytes = u64::try_from(buffer.bytes().len())
                        .map_err(|_| "diagnosis allocation length does not fit u64".to_owned())?;
                    pending.insert(
                        allocation,
                        PendingDiagnosisAllocationV2 {
                            address_space: AddressSpaceV1::Global,
                            access: diagnosis_access_v2(buffer.access())?,
                            alignment: buffer.alignment(),
                            allocation_bytes: bytes,
                            abi_arguments: Vec::new(),
                        },
                    );
                    (
                        allocation,
                        None,
                        0,
                        bytes,
                        buffer.element(),
                        buffer.access(),
                    )
                }
                SimulationArgumentV1::BufferView(view) => {
                    let allocation = *shared_allocations.get(&view.backing()).ok_or_else(|| {
                        "admitted diagnosis buffer view backing is missing".to_owned()
                    })?;
                    let element_bytes =
                        diagnosis_scalar_bytes_v2(view.element(), input.simulation_target())?;
                    let bytes = u64::try_from(view.elements())
                        .ok()
                        .and_then(|elements| elements.checked_mul(element_bytes))
                        .ok_or_else(|| "diagnosis buffer view length overflow".to_owned())?;
                    (
                        allocation,
                        Some(view.backing().0),
                        u64::try_from(view.byte_offset()).map_err(|_| {
                            "diagnosis buffer view offset does not fit u64".to_owned()
                        })?,
                        bytes,
                        view.element(),
                        view.access(),
                    )
                }
            };
        let (kind, abi_element, address_space, abi_access) = match ty {
            Type::Pointer(pointer) => (
                DiagnosisAbiArgumentKindV2::Pointer,
                pointer.pointee.as_scalar(),
                pointer.address_space,
                pointer.access,
            ),
            Type::Slice(slice) => (
                DiagnosisAbiArgumentKindV2::Slice,
                slice.element.as_scalar(),
                slice.address_space,
                slice.access,
            ),
            _ => return Err("admitted diagnosis buffer ABI type changed".to_owned()),
        };
        if abi_element != Some(element) {
            return Err("admitted diagnosis buffer ABI contract changed".to_owned());
        }
        let allocation_contract = pending
            .get_mut(&allocation)
            .ok_or_else(|| "diagnosis allocation contract is missing".to_owned())?;
        let required_access = diagnosis_access_v2(abi_access)?;
        let supplied_access = diagnosis_access_v2(argument_access)?;
        if !diagnosis_access_satisfies_v2(required_access, supplied_access)
            || (backing.is_none() && supplied_access != allocation_contract.access)
            || (backing.is_some()
                && !diagnosis_access_satisfies_v2(supplied_access, allocation_contract.access))
        {
            return Err("admitted diagnosis buffer access contract changed".to_owned());
        }
        allocation_contract
            .abi_arguments
            .try_reserve_exact(1)
            .map_err(|_| "diagnosis ABI argument allocation failed".to_owned())?;
        allocation_contract
            .abi_arguments
            .push(DiagnosisAbiArgumentV2 {
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| "diagnosis ABI ordinal does not fit u32".to_owned())?,
                backing,
                kind,
                element: diagnosis_scalar_type_v2(element, input.simulation_target()),
                address_space: diagnosis_address_space_v2(address_space),
                access: required_access,
                supplied_access,
                view_offset,
                view_bytes,
            });
    }

    pending
        .into_iter()
        .map(|(allocation, pending)| {
            Ok((
                allocation,
                DiagnosisAllocationContractV2 {
                    address_space: pending.address_space,
                    access: pending.access,
                    alignment: pending.alignment,
                    allocation_bytes: pending.allocation_bytes,
                    abi_arguments: pending.abi_arguments,
                },
            ))
        })
        .collect()
}

fn diagnosis_scalar_bytes_v2(
    scalar: ScalarType,
    target: SimulationTargetV1,
) -> Result<u64, String> {
    let bits = match scalar {
        ScalarType::Bool => 1,
        ScalarType::Index => match target.index_width() {
            IndexWidthV1::Bits32 => 32,
            IndexWidthV1::Bits64 => 64,
        },
        _ => scalar
            .bit_width()
            .ok_or_else(|| "diagnosis scalar width is unavailable".to_owned())?,
    };
    Ok(if bits == 1 { 1 } else { u64::from(bits / 8) })
}

const fn diagnosis_protocol_scalar_bytes_v2(scalar: DiagnosisScalarTypeV2) -> u16 {
    match scalar {
        DiagnosisScalarTypeV2::Bool | DiagnosisScalarTypeV2::I8 | DiagnosisScalarTypeV2::U8 => 1,
        DiagnosisScalarTypeV2::I16
        | DiagnosisScalarTypeV2::U16
        | DiagnosisScalarTypeV2::F16
        | DiagnosisScalarTypeV2::Bf16 => 2,
        DiagnosisScalarTypeV2::I32
        | DiagnosisScalarTypeV2::U32
        | DiagnosisScalarTypeV2::Index32
        | DiagnosisScalarTypeV2::F32 => 4,
        DiagnosisScalarTypeV2::I64
        | DiagnosisScalarTypeV2::U64
        | DiagnosisScalarTypeV2::Index64
        | DiagnosisScalarTypeV2::F64 => 8,
        DiagnosisScalarTypeV2::I128 | DiagnosisScalarTypeV2::U128 => 16,
    }
}

const fn diagnosis_scalar_type_v2(
    scalar: ScalarType,
    target: SimulationTargetV1,
) -> DiagnosisScalarTypeV2 {
    match scalar {
        ScalarType::Bool => DiagnosisScalarTypeV2::Bool,
        ScalarType::I8 => DiagnosisScalarTypeV2::I8,
        ScalarType::I16 => DiagnosisScalarTypeV2::I16,
        ScalarType::I32 => DiagnosisScalarTypeV2::I32,
        ScalarType::I64 => DiagnosisScalarTypeV2::I64,
        ScalarType::I128 => DiagnosisScalarTypeV2::I128,
        ScalarType::U8 => DiagnosisScalarTypeV2::U8,
        ScalarType::U16 => DiagnosisScalarTypeV2::U16,
        ScalarType::U32 => DiagnosisScalarTypeV2::U32,
        ScalarType::U64 => DiagnosisScalarTypeV2::U64,
        ScalarType::U128 => DiagnosisScalarTypeV2::U128,
        ScalarType::Index => match target.index_width() {
            IndexWidthV1::Bits32 => DiagnosisScalarTypeV2::Index32,
            IndexWidthV1::Bits64 => DiagnosisScalarTypeV2::Index64,
        },
        ScalarType::F16 => DiagnosisScalarTypeV2::F16,
        ScalarType::Bf16 => DiagnosisScalarTypeV2::Bf16,
        ScalarType::F32 => DiagnosisScalarTypeV2::F32,
        ScalarType::F64 => DiagnosisScalarTypeV2::F64,
    }
}

fn diagnosis_access_v2(access: AccessMode) -> Result<DiagnosisAccessModeV2, String> {
    match access {
        AccessMode::ReadOnly => Ok(DiagnosisAccessModeV2::ReadOnly),
        AccessMode::ReadWrite => Ok(DiagnosisAccessModeV2::ReadWrite),
        AccessMode::WriteOnly => Err(
            "diagnosis V2 cannot represent write-only buffer access; a newer diagnosis schema is required"
                .to_owned(),
        ),
    }
}

const fn diagnosis_access_satisfies_v2(
    required: DiagnosisAccessModeV2,
    supplied: DiagnosisAccessModeV2,
) -> bool {
    matches!(required, DiagnosisAccessModeV2::ReadOnly)
        || matches!(supplied, DiagnosisAccessModeV2::ReadWrite)
}

const fn diagnosis_address_space_v2(address_space: AddressSpace) -> AddressSpaceV1 {
    match address_space {
        AddressSpace::Private => AddressSpaceV1::Private,
        AddressSpace::Workgroup => AddressSpaceV1::Workgroup,
        AddressSpace::Global => AddressSpaceV1::Global,
        AddressSpace::Constant => AddressSpaceV1::Constant,
        AddressSpace::Generic => AddressSpaceV1::Generic,
    }
}

const fn diagnosis_synchronization_scope_v2(
    scope: SynchronizationScope,
) -> DiagnosisSynchronizationScopeV2 {
    match scope {
        SynchronizationScope::Invocation => DiagnosisSynchronizationScopeV2::Invocation,
        SynchronizationScope::Subgroup => DiagnosisSynchronizationScopeV2::Subgroup,
        SynchronizationScope::Workgroup => DiagnosisSynchronizationScopeV2::Workgroup,
        SynchronizationScope::Device => DiagnosisSynchronizationScopeV2::Device,
        SynchronizationScope::System => DiagnosisSynchronizationScopeV2::System,
    }
}

const fn diagnosis_memory_ordering_v2(ordering: MemoryOrdering) -> DiagnosisMemoryOrderingV2 {
    match ordering {
        MemoryOrdering::Relaxed => DiagnosisMemoryOrderingV2::Relaxed,
        MemoryOrdering::Acquire => DiagnosisMemoryOrderingV2::Acquire,
        MemoryOrdering::Release => DiagnosisMemoryOrderingV2::Release,
        MemoryOrdering::AcquireRelease => DiagnosisMemoryOrderingV2::AcquireRelease,
        MemoryOrdering::SequentiallyConsistent => DiagnosisMemoryOrderingV2::SequentiallyConsistent,
    }
}

fn diagnosis_failed_lds_epoch_v2(phase: u64) -> DiagnosisLdsEpochV2 {
    DiagnosisLdsEpochV2 {
        current: DiagnosisFactV2::Inferred {
            value: phase,
            basis: DiagnosisInferenceBasisV2::BarrierPhase,
        },
        after_release: DiagnosisFactV2::Unavailable {
            reason: DiagnosisUnavailableReasonV2::BarrierNotReleased,
        },
    }
}

fn configuration_identity(
    kir: [u8; 32],
    request: [u8; 32],
    wave_width: DebugWaveWidthV1,
) -> OpaqueIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-debug-sim-config-v1\0");
    digest.update(kir);
    digest.update(request);
    digest.update(wave_width.lanes().to_le_bytes());
    nonzero_identity(digest.finalize().into())
}

fn configuration_identity_for_input(
    input: &AdmittedSimulationInputV1,
    wave_width: DebugWaveWidthV1,
) -> OpaqueIdentityV1 {
    let base = configuration_identity(input.kir_sha256, input.request_sha256, wave_width);
    let Some(bundle) = input.simulation_bundle_evidence() else {
        return base;
    };
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-debug-sim-bundle-config-v1\0");
    digest.update(base.as_bytes());
    digest.update(bundle.envelope_version.to_le_bytes());
    digest.update(bundle.envelope_identity);
    digest.update(bundle.subject_identity);
    digest.update(bundle.production_kir_version.to_le_bytes());
    digest.update(bundle.production_kir_sha256);
    digest.update(bundle.production_kir_bytes.to_le_bytes());
    digest.update(bundle.kernel_abi_identity);
    digest.update(bundle.identity_inventory_receipt_sha256);
    digest.update(bundle.identity_inventory_receipt_bytes.to_le_bytes());
    digest.update(bundle.preflight_plan_receipt_sha256);
    digest.update(bundle.preflight_plan_receipt_bytes.to_le_bytes());
    nonzero_identity(digest.finalize().into())
}

fn configuration_identity_for_replay(
    base: OpaqueIdentityV1,
    schedule: &SimulationScheduleRecordV1,
) -> OpaqueIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-debug-sim-schedule-config-v1\0");
    digest.update(base.as_bytes());
    digest.update(schedule.context_identity());
    digest.update(schedule.transcript_identity());
    digest.update(schedule.record_integrity());
    nonzero_identity(digest.finalize().into())
}

fn configuration_identity_for_source_map_v2(
    base: OpaqueIdentityV1,
    source_map: OpaqueIdentityV1,
    bundle_subject: OpaqueIdentityV1,
    operation_membership_root: OpaqueIdentityV1,
    operation_members: u32,
) -> OpaqueIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-debug-sim-source-map-v2-config-v1\0");
    digest.update(base.as_bytes());
    digest.update(source_map.as_bytes());
    digest.update(bundle_subject.as_bytes());
    digest.update(operation_membership_root.as_bytes());
    digest.update(operation_members.to_le_bytes());
    nonzero_identity(digest.finalize().into())
}

fn source_span_for_step(
    inspection: DebugInspectionV1<DebugSourceResolutionV1>,
    absent_is_error: bool,
) -> Result<Option<DebugSourceSpanV1>, (CapabilityUnavailableReasonV1, &'static str)> {
    match inspection {
        DebugInspectionV1::Available(DebugSourceResolutionV1::Resolved { span, .. }) => {
            Ok(Some(span))
        }
        DebugInspectionV1::Available(DebugSourceResolutionV1::Absent) if !absent_is_error => {
            Ok(None)
        }
        DebugInspectionV1::Available(DebugSourceResolutionV1::Absent) => Err((
            CapabilityUnavailableReasonV1::Absent,
            "the current KIR site is absent from the bound source map",
        )),
        DebugInspectionV1::Available(DebugSourceResolutionV1::Eliminated) => Err((
            CapabilityUnavailableReasonV1::OptimizedOut,
            "source stepping reached a site eliminated before canonical KIR",
        )),
        DebugInspectionV1::Available(DebugSourceResolutionV1::ManyToOne) => Err((
            CapabilityUnavailableReasonV1::ManyToOne,
            "source stepping reached a KIR site with no unique source location",
        )),
        DebugInspectionV1::Unavailable(_) => Err((
            CapabilityUnavailableReasonV1::RequiresAuthenticatedMap,
            "source stepping requires a bound source map",
        )),
    }
}

fn nonzero_identity(mut bytes: [u8; 32]) -> OpaqueIdentityV1 {
    if bytes == [0; 32] {
        bytes[31] = 1;
    }
    OpaqueIdentityV1::new(bytes).expect("identity is made nonzero")
}

fn bounded_message(message: &str) -> String {
    let mut result = String::new();
    for character in message.chars().filter(|character| !character.is_control()) {
        if result.len() + character.len_utf8() > MAX_TEXT_BYTES_V1 {
            break;
        }
        result.push(character);
    }
    if result.is_empty() {
        result.push_str("debug operation failed");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnosis_v2_access_conversion_rejects_unrepresentable_write_only_access() {
        assert_eq!(
            diagnosis_access_v2(AccessMode::ReadOnly),
            Ok(DiagnosisAccessModeV2::ReadOnly)
        );
        assert_eq!(
            diagnosis_access_v2(AccessMode::ReadWrite),
            Ok(DiagnosisAccessModeV2::ReadWrite)
        );
        assert!(diagnosis_access_v2(AccessMode::WriteOnly).is_err());
    }

    #[test]
    fn simulator_float_values_project_as_exact_protocol_float_bits() {
        for (ty, bits, width) in [
            (ScalarType::F16, 0x7e42, 16),
            (ScalarType::Bf16, 0x7fc2, 16),
            (ScalarType::F32, 0x8000_0000, 32),
            (ScalarType::F64, 1, 64),
        ] {
            let value = ScalarBitsV1::new(ty, bits, SimulationTargetV1::amdgpu_64()).unwrap();
            assert_eq!(
                protocol_scalar_type(value),
                (DebugValueTypeV1::Float { bits: width }, width)
            );
            let availability = availability_for_observed(&SimulationDebugValueV1::Scalar(value));
            assert!(matches!(
                availability,
                ValueAvailabilityV1::Captured {
                    value_type: DebugValueTypeV1::Float { bits: actual },
                    value: CapturedValueV1::Bits { ref bits },
                    ..
                } if actual == width && bits == &fixed_width_bits(value.bits(), width)
            ));
        }
    }

    fn fixture_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("crate is in workspace/crates")
            .join(relative)
    }

    fn fill_input() -> AdmittedSimulationInputV1 {
        load_debug_simulation_input_v1(
            &fixture_path("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir"),
            &fixture_path("crates/fe2o3-kir-sim-cli/tutorial/fill-v1/request.json"),
        )
        .expect("admit fill fixture")
    }

    fn fill_source_map() -> Vec<u8> {
        std::fs::read(fixture_path(
            "crates/fe2o3-debug-cli/tutorial/fill-v1/source-map.json",
        ))
        .expect("read source map fixture")
    }

    fn fixture_subject() -> OpaqueIdentityV1 {
        nonzero_identity([
            0xe5, 0x84, 0x49, 0x7b, 0x14, 0x6b, 0x0d, 0xf9, 0x5a, 0x63, 0xa7, 0x89, 0x0e, 0x00,
            0x3c, 0xd8, 0xed, 0xf2, 0xce, 0x9d, 0xfb, 0x45, 0xdf, 0xda, 0x1c, 0xc6, 0x2c, 0x85,
            0x29, 0x11, 0x99, 0x50,
        ])
    }

    fn test_source_scope_v2(
        identity: [u8; 32],
        parent_identity: Option<[u8; 32]>,
        depth: u32,
    ) -> fe2o3_kernel_ir::DebugSourceScopeV2 {
        let v1 = DebugSourceMapDocumentV1::from_json_bytes(&fill_source_map()).unwrap();
        fe2o3_kernel_ir::DebugSourceScopeV2::new(
            identity,
            0,
            parent_identity,
            depth,
            v1.sites()[0].spans()[0],
        )
        .unwrap()
    }

    fn admitted_test_source_map_v2(
        input: &AdmittedSimulationInputV1,
        scopes: Vec<fe2o3_kernel_ir::DebugSourceScopeV2>,
        variables: Vec<fe2o3_kernel_ir::DebugSourceVariableV2>,
    ) -> AdmittedSourceMapV2 {
        // This is a compiler-shaped test document, not compiler-emission evidence.
        let v1 = DebugSourceMapDocumentV1::from_json_bytes(&fill_source_map()).unwrap();
        let document = DebugSourceMapDocumentV2::new(
            v1.binding(),
            v1.files().to_vec(),
            v1.sites().to_vec(),
            v1.eliminated().to_vec(),
            scopes,
            variables,
        )
        .unwrap();
        let bytes = document.to_canonical_json_bytes().unwrap();
        admit_source_map_v2(
            &bytes,
            input,
            configuration_identity(
                input.kir_sha256,
                input.request_sha256,
                DebugWaveWidthV1::Wave64,
            ),
            fixture_subject(),
            nonzero_identity(simulation_debug_map_identity_v2(&bytes)),
        )
        .unwrap()
    }

    fn test_source_variable_v2(
        identity: [u8; 32],
        name: &str,
        scope_identity: [u8; 32],
        fallback: DebugSourceVariableFallbackV2,
        locations: Vec<fe2o3_kernel_ir::DebugSourceVariableLocationV2>,
    ) -> fe2o3_kernel_ir::DebugSourceVariableV2 {
        fe2o3_kernel_ir::DebugSourceVariableV2::new(
            identity,
            name.to_owned(),
            0,
            scope_identity,
            fallback,
            locations,
        )
        .unwrap()
    }

    fn test_source_location_v2(
        next_operation: u64,
        generation: u64,
        binding: DebugSourceVariableBindingV2,
    ) -> fe2o3_kernel_ir::DebugSourceVariableLocationV2 {
        fe2o3_kernel_ir::DebugSourceVariableLocationV2::new(0, next_operation, generation, binding)
            .unwrap()
    }

    fn backend_with_test_source_map_v2(
        scopes: Vec<fe2o3_kernel_ir::DebugSourceScopeV2>,
        variables: Vec<fe2o3_kernel_ir::DebugSourceVariableV2>,
    ) -> SimulatorBackendV1 {
        let input = fill_input();
        let source_map = admitted_test_source_map_v2(&input, scopes, variables);
        SimulatorBackendV1::new_with_source_map_v2_and_schedule(
            input,
            DebugWaveWidthV1::Wave64,
            source_map,
            None,
        )
        .unwrap()
    }

    fn seek_test_checkpoint(backend: &mut SimulatorBackendV1, next_operation: u32) {
        let record = backend
            .session
            .transcript()
            .records()
            .iter()
            .position(|record| {
                matches!(
                    &record.kind,
                    SimulationDebugRecordKindV1::Checkpoint {
                        stack: SimulationDebugCollectionV1::Captured(frames),
                        ..
                    } if frames.iter().any(|frame| {
                        frame.depth == 0
                            && frame.function_ordinal == 0
                            && frame.block == fe2o3_kernel_ir::BlockId(0)
                            && frame.next_operation == Some(next_operation)
                    })
                )
            })
            .unwrap_or_else(|| panic!("missing test checkpoint before operation {next_operation}"));
        assert!(matches!(
            backend.session.seek_record_index(record),
            DebugNavigationV1::Stopped(_)
        ));
    }

    fn inspect_source_variables_v2(
        backend: &mut SimulatorBackendV1,
        request_id: u64,
        frame: Option<u64>,
        selector: SourceVariableSelectorV2,
        page: PageRequestV1,
    ) -> SourceVariableResponseV2 {
        backend.handle_source_variables_v2(SourceVariableRequestV2::InspectSourceVariables {
            schema: SourceVariableRequestSchemaV2::V2,
            request_id,
            expected_revision: backend.revision,
            scope: ExecutionScopeSelectorV1::Dispatch,
            frame,
            selector,
            page,
        })
    }

    fn barrier_record(
        ordinal: u64,
        lane: u32,
        kind: SimulationDebugRecordKindV1,
    ) -> SimulationDebugRecordV1 {
        SimulationDebugRecordV1 {
            ordinal,
            schedule: fe2o3_kir_sim::SimulationDebugScheduleV1 {
                identity:
                    fe2o3_kir_sim::SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1,
                decision_ordinal: ordinal,
            },
            invocation: SimulationInvocationV1 {
                global: [u64::from(lane), 0, 0],
                workgroup: [0, 0, 0],
                local: [lane, 0, 0],
                workgroup_size: [2, 1, 1],
                workgroup_count: [1, 1, 1],
                launch_extent: [2, 1, 1],
            },
            site: fe2o3_kir_sim::SimulationDebugSiteV1 {
                function_ordinal: if matches!(&kind, SimulationDebugRecordKindV1::Checkpoint { .. })
                {
                    0
                } else {
                    1
                },
                block: fe2o3_kernel_ir::BlockId(0),
                operation: 0,
            },
            kind,
        }
    }

    fn empty_checkpoint() -> SimulationDebugRecordKindV1 {
        SimulationDebugRecordKindV1::Checkpoint {
            phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
            stack: SimulationDebugCollectionV1::Captured(vec![]),
            memory: SimulationDebugCollectionV1::Captured(vec![]),
        }
    }

    #[test]
    fn command_line_is_closed_and_wave_width_is_explicit() {
        let options = parse_options(
            [
                "sim",
                "--kir-v7",
                "kernel.kir",
                "--request",
                "request.json",
                "--protocol",
                "jsonl",
                "--wave-width",
                "32",
            ]
            .into_iter()
            .map(OsString::from),
        )
        .unwrap();
        assert!(matches!(
            options.program,
            ProgramInputV1::KirV7(ref path) if path == &PathBuf::from("kernel.kir")
        ));
        assert!(matches!(
            options.request,
            RequestInputV1::Path(ref path) if path == &PathBuf::from("request.json")
        ));
        assert_eq!(options.wave_width, DebugWaveWidthV1::Wave32);

        let options = parse_options(
            [
                "sim",
                "--kir-v7-fd",
                "7",
                "--request-fd",
                "8",
                "--protocol",
                "jsonl",
            ]
            .into_iter()
            .map(OsString::from),
        )
        .unwrap();
        assert!(matches!(options.program, ProgramInputV1::SealedKirV7Fd(7)));
        assert!(matches!(options.request, RequestInputV1::SealedFd(8)));

        for arguments in [
            vec!["sim", "--kir-v7", "kernel.kir", "--request"],
            vec![
                "sim",
                "--kir-v7",
                "kernel.kir",
                "--request",
                "request.json",
                "--protocol",
                "stdio",
            ],
            vec![
                "sim",
                "--kir-v7",
                "one.kir",
                "--kir-v7",
                "two.kir",
                "--request",
                "request.json",
            ],
            vec!["sim", "--kir-v7-fd", "7", "--request-fd", "7"],
            vec!["sim", "--kir-v7-fd", "07", "--request-fd", "8"],
            vec!["sim", "--kir-v7-fd", "7", "--request", "request.json"],
            vec!["sim", "--kir-v7", "kernel.kir", "--request-fd", "8"],
        ] {
            assert!(parse_options(arguments.into_iter().map(OsString::from)).is_err());
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sealed_fd_input_admission_rejects_alias_named_unsealed_and_oversized_objects() {
        use std::os::unix::fs::OpenOptionsExt;

        fn memfd(bytes: &[u8], seal: bool) -> File {
            let descriptor = rustix::fs::memfd_create(
                "fe2o3-debug-cli-fd-test-v1",
                rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
            )
            .unwrap();
            let writable = File::from(descriptor);
            rustix::fs::fchmod(&writable, rustix::fs::Mode::from_raw_mode(0o400)).unwrap();
            writable.write_all_at(bytes, 0).unwrap();
            if seal {
                rustix::fs::fcntl_add_seals(
                    &writable,
                    rustix::fs::SealFlags::WRITE
                        | rustix::fs::SealFlags::GROW
                        | rustix::fs::SealFlags::SHRINK,
                )
                .unwrap();
                rustix::fs::fcntl_add_seals(&writable, rustix::fs::SealFlags::SEAL).unwrap();
            }
            let path = format!("/proc/self/fd/{}", writable.as_raw_fd());
            let read_only = rustix::fs::open(
                path,
                rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map(File::from)
            .unwrap();
            drop(writable);
            read_only
        }

        let admitted = memfd(b"exact bytes", true);
        assert_eq!(
            read_sealed_debug_input_fd_v1(admitted.as_raw_fd(), 1024, "test input")
                .unwrap()
                .bytes,
            b"exact bytes"
        );
        let alias = rustix::io::fcntl_dupfd_cloexec(&admitted, 3).unwrap();
        assert_ne!(admitted.as_raw_fd(), alias.as_raw_fd());
        assert_eq!(
            read_sealed_debug_input_pair_v1(admitted.as_raw_fd(), alias.as_raw_fd(), 1024)
                .unwrap_err(),
            "canonical KIR V7 and simulation request descriptors alias the same sealed memfd"
        );

        let unsealed = memfd(b"unsealed", false);
        assert!(read_sealed_debug_input_fd_v1(unsealed.as_raw_fd(), 1024, "test input").is_err());

        let root =
            std::env::temp_dir().join(format!("fe2o3-debug-cli-named-fd-{}", std::process::id()));
        let named = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .mode(0o400)
            .open(&root)
            .unwrap();
        named.write_all_at(b"named", 0).unwrap();
        assert!(read_sealed_debug_input_fd_v1(named.as_raw_fd(), 1024, "test input").is_err());
        drop(named);
        std::fs::remove_file(root).unwrap();

        let oversized = memfd(b"oversized", true);
        assert!(read_sealed_debug_input_fd_v1(oversized.as_raw_fd(), 4, "test input").is_err());
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn live_kfd_command_line_requires_every_exact_input_and_separator() {
        let options = parse_live_kfd_options_v3(
            [
                "live-kfd",
                "--bundle-v2",
                "kernel.fe2sim-v2",
                "--request",
                "request.json",
                "--hsaco",
                "kernel.hsaco",
                "--protocol",
                "jsonl",
                "--wave-width",
                "32",
                "--",
                "target",
                "argument",
            ]
            .into_iter()
            .map(OsString::from),
        )
        .unwrap();
        assert_eq!(options.bundle_v2, PathBuf::from("kernel.fe2sim-v2"));
        assert_eq!(options.request, PathBuf::from("request.json"));
        assert_eq!(options.hsaco, PathBuf::from("kernel.hsaco"));
        assert_eq!(options.wave_width, DebugWaveWidthV1::Wave32);
        assert_eq!(options.program, PathBuf::from("target"));
        assert_eq!(options.program_arguments, [OsString::from("argument")]);

        for arguments in [
            vec![
                "live-kfd",
                "--request",
                "request.json",
                "--hsaco",
                "kernel.hsaco",
                "--",
                "target",
            ],
            vec![
                "live-kfd",
                "--bundle-v2",
                "kernel.fe2sim-v2",
                "--request",
                "request.json",
                "--hsaco",
                "kernel.hsaco",
                "target",
            ],
            vec![
                "live-kfd",
                "--bundle-v2",
                "kernel.fe2sim-v2",
                "--request",
                "request.json",
                "--hsaco",
                "kernel.hsaco",
                "--protocol",
                "stdio",
                "--",
                "target",
            ],
        ] {
            assert!(parse_live_kfd_options_v3(arguments.into_iter().map(OsString::from)).is_err());
        }
    }

    #[test]
    fn source_map_is_exactly_bound_and_hostile_documents_fail_closed() {
        let input = fill_input();
        let configuration = configuration_identity(
            input.kir_sha256,
            input.request_sha256,
            DebugWaveWidthV1::Wave64,
        );
        let bytes = fill_source_map();
        let admitted = admit_source_map_v1(&bytes, &input, configuration, fixture_subject())
            .expect("admit exact fixture map");
        assert_eq!(admitted.provenance, SourceMapProvenanceV1::CallerBound);
        assert_eq!(admitted.catalog.sites().len(), 4);

        assert!(
            admit_source_map_v1(&bytes, &input, configuration, nonzero_identity([8; 32]))
                .unwrap_err()
                .contains("bundle subject")
        );

        let text = std::str::from_utf8(&bytes).unwrap();
        let stale = text.replacen(
            "e8f2c794a5dd4aeac63f5c820f9d5785b40b5aaff357e3f6726164fa4425f384",
            "0909090909090909090909090909090909090909090909090909090909090909",
            1,
        );
        assert!(
            admit_source_map_v1(stale.as_bytes(), &input, configuration, fixture_subject())
                .unwrap_err()
                .contains("canonical KIR identity")
        );
        for hostile in [
            text.replacen(
                "\"schema\": \"fe2o3-debug-source-map-v1\"",
                "\"schema\": \"fe2o3-debug-source-map-v1\", \"unknown\": 1",
                1,
            ),
            text.replacen(
                "\"schema\": \"fe2o3-debug-source-map-v1\"",
                "\"schema\": \"fe2o3-debug-source-map-v1\", \"schema\": \"fe2o3-debug-source-map-v1\"",
                1,
            ),
            text.replacen("\"files\": [", "\"files\": null, \"discard\": [", 1),
        ] {
            assert!(admit_source_map_v1(
                hostile.as_bytes(),
                &input,
                configuration,
                fixture_subject(),
            )
            .is_err());
        }
    }

    #[test]
    fn v1_and_empty_v2_maps_have_distinct_source_variable_unavailability() {
        let input = fill_input();
        let configuration = configuration_identity(
            input.kir_sha256,
            input.request_sha256,
            DebugWaveWidthV1::Wave64,
        );
        let source_map =
            admit_source_map_v1(&fill_source_map(), &input, configuration, fixture_subject())
                .unwrap();
        let mut v1_backend = SimulatorBackendV1::new_with_source_map(
            input,
            DebugWaveWidthV1::Wave64,
            Some(source_map),
        )
        .unwrap();
        assert!(matches!(
            inspect_source_variables_v2(
                &mut v1_backend,
                1,
                Some(1),
                SourceVariableSelectorV2::All,
                PageRequestV1 {
                    cursor: None,
                    limit: 8
                },
            ),
            SourceVariableResponseV2::Unavailable {
                reason: SourceVariableQueryUnavailableReasonV2::SourceMapV2Required,
                ..
            }
        ));

        let mut empty_v2_backend = backend_with_test_source_map_v2(Vec::new(), Vec::new());
        assert!(matches!(
            inspect_source_variables_v2(
                &mut empty_v2_backend,
                2,
                Some(1),
                SourceVariableSelectorV2::All,
                PageRequestV1 {
                    cursor: None,
                    limit: 8
                },
            ),
            SourceVariableResponseV2::Unavailable {
                reason: SourceVariableQueryUnavailableReasonV2::VariablesNotCaptured,
                ..
            }
        ));
    }

    #[test]
    fn oversized_max_page_returns_correlated_v2_error_and_keeps_stream_usable() {
        let scope = [0x31; 32];
        let scopes = vec![test_source_scope_v2(scope, None, 0)];
        let mut variables = Vec::new();
        variables
            .try_reserve_exact(MAX_RESPONSE_ITEMS_V1)
            .expect("reserve bounded source-variable fixture");
        for ordinal in 0..MAX_RESPONSE_ITEMS_V1 {
            let mut identity = [0x41; 32];
            identity[24..].copy_from_slice(&(ordinal as u64).to_be_bytes());
            let suffix = format!("{ordinal:04x}");
            let mut name = "v".repeat(MAX_TEXT_BYTES_V1 - suffix.len());
            name.push_str(&suffix);
            variables.push(test_source_variable_v2(
                identity,
                &name,
                scope,
                DebugSourceVariableFallbackV2::OptimizedOut,
                Vec::new(),
            ));
        }
        let mut backend = backend_with_test_source_map_v2(scopes, variables);
        seek_test_checkpoint(&mut backend, 0);
        let session = backend.session_view();
        let requests = [
            SourceVariableRequestV2::InspectSourceVariables {
                schema: SourceVariableRequestSchemaV2::V2,
                request_id: 70,
                expected_revision: backend.revision,
                scope: ExecutionScopeSelectorV1::Dispatch,
                frame: Some(1),
                selector: SourceVariableSelectorV2::All,
                page: PageRequestV1 {
                    cursor: None,
                    limit: MAX_PAGE_ITEMS_V1,
                },
            },
            SourceVariableRequestV2::InspectSourceVariables {
                schema: SourceVariableRequestSchemaV2::V2,
                request_id: 71,
                expected_revision: backend.revision,
                scope: ExecutionScopeSelectorV1::Dispatch,
                frame: Some(1),
                selector: SourceVariableSelectorV2::All,
                page: PageRequestV1 {
                    cursor: None,
                    limit: 1,
                },
            },
        ];
        let mut input = Vec::new();
        for request in requests {
            serde_json::to_writer(&mut input, &request).unwrap();
            input.push(b'\n');
        }
        let mut output = Vec::new();
        run_jsonl_v1(
            backend,
            &mut std::io::BufReader::new(input.as_slice()),
            &mut output,
        )
        .unwrap();
        let responses = output
            .split_inclusive(|byte| *byte == b'\n')
            .map(|line| {
                decode_source_variable_response_line_v2(line, ProtocolLimitsV1::default()).unwrap()
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            &responses[0],
            SourceVariableResponseV2::Error {
                request_id: Some(70),
                operation: SourceVariableOperationV2::InspectSourceVariables,
                session: actual_session,
                error: DebugErrorV1 {
                    stage: DebugErrorStageV1::Output,
                    code: DebugErrorCodeV1::ResponseTooLarge,
                    state_changed: false,
                    ..
                },
                ..
            } if *actual_session == session
        ));
        assert!(matches!(
            &responses[1],
            SourceVariableResponseV2::Ok {
                request_id: 71,
                values,
                next_cursor: Some(PageCursorV1 { position: 1, .. }),
                ..
            } if values.len() == 1
        ));
    }

    #[test]
    fn unrelated_active_scope_trees_are_name_ambiguous_even_at_unequal_depths() {
        let left_root = [0x51; 32];
        let left_scope = [0x52; 32];
        let right_root = [0x61; 32];
        let right_scope = [0x62; 32];
        let right_inner = [0x63; 32];
        let scopes = vec![
            test_source_scope_v2(left_root, None, 0),
            test_source_scope_v2(left_scope, Some(left_root), 1),
            test_source_scope_v2(right_root, None, 0),
            test_source_scope_v2(right_scope, Some(right_root), 1),
            test_source_scope_v2(right_inner, Some(right_scope), 2),
        ];
        let variables = vec![
            test_source_variable_v2(
                [0x71; 32],
                "item",
                left_scope,
                DebugSourceVariableFallbackV2::NotInScope,
                vec![test_source_location_v2(
                    0,
                    1,
                    DebugSourceVariableBindingV2::Captured { value_ordinal: 0 },
                )],
            ),
            test_source_variable_v2(
                [0x72; 32],
                "item",
                right_inner,
                DebugSourceVariableFallbackV2::OptimizedOut,
                Vec::new(),
            ),
        ];
        let mut backend = backend_with_test_source_map_v2(scopes, variables);
        seek_test_checkpoint(&mut backend, 0);
        let SourceVariableResponseV2::Ok { values, .. } = inspect_source_variables_v2(
            &mut backend,
            72,
            Some(1),
            SourceVariableSelectorV2::Name {
                name: "item".into(),
            },
            PageRequestV1 {
                cursor: None,
                limit: 8,
            },
        ) else {
            panic!("unrelated active source scopes must produce typed results")
        };
        assert_eq!(values.len(), 2);
        assert!(values.iter().all(|value| matches!(
            value.availability,
            SourceVariableValueAvailabilityV2::Ambiguous
        )));
    }

    #[test]
    fn test_authored_v2_map_tracks_checkpoint_zero_shadowing_and_generations() {
        let root = [0x11; 32];
        let inner = [0x12; 32];
        let buffer = [0x21; 32];
        let outer_item = [0x22; 32];
        let inner_item = [0x23; 32];
        let state = [0x24; 32];
        let optimized = [0x25; 32];
        let unrepresented = [0x26; 32];
        let not_captured = [0x27; 32];
        let scopes = vec![
            test_source_scope_v2(root, None, 0),
            test_source_scope_v2(inner, Some(root), 1),
        ];
        let variables = vec![
            test_source_variable_v2(
                buffer,
                "buffer",
                root,
                DebugSourceVariableFallbackV2::NotInScope,
                Vec::new(),
            )
            .with_function_binding(
                fe2o3_kernel_ir::DebugSourceVariableFunctionBindingV2::new(1, 0).unwrap(),
            )
            .unwrap(),
            test_source_variable_v2(
                outer_item,
                "item",
                root,
                DebugSourceVariableFallbackV2::NotInScope,
                vec![test_source_location_v2(
                    0,
                    1,
                    DebugSourceVariableBindingV2::Captured { value_ordinal: 0 },
                )],
            ),
            test_source_variable_v2(
                inner_item,
                "item",
                inner,
                DebugSourceVariableFallbackV2::NotInScope,
                vec![
                    test_source_location_v2(0, 0, DebugSourceVariableBindingV2::NotInScope),
                    test_source_location_v2(
                        1,
                        1,
                        DebugSourceVariableBindingV2::Captured { value_ordinal: 1 },
                    ),
                    test_source_location_v2(2, 0, DebugSourceVariableBindingV2::NotInScope),
                ],
            ),
            test_source_variable_v2(
                state,
                "state",
                root,
                DebugSourceVariableFallbackV2::NotInScope,
                vec![
                    test_source_location_v2(0, 1, DebugSourceVariableBindingV2::Uninitialized),
                    test_source_location_v2(
                        1,
                        1,
                        DebugSourceVariableBindingV2::Captured { value_ordinal: 1 },
                    ),
                    test_source_location_v2(2, 0, DebugSourceVariableBindingV2::NotInScope),
                    test_source_location_v2(3, 2, DebugSourceVariableBindingV2::Uninitialized),
                ],
            ),
            test_source_variable_v2(
                optimized,
                "optimized",
                root,
                DebugSourceVariableFallbackV2::OptimizedOut,
                Vec::new(),
            ),
            test_source_variable_v2(
                unrepresented,
                "unrepresented",
                root,
                DebugSourceVariableFallbackV2::Unrepresented,
                Vec::new(),
            ),
            test_source_variable_v2(
                not_captured,
                "not_captured",
                root,
                DebugSourceVariableFallbackV2::NotCaptured,
                Vec::new(),
            ),
            test_source_variable_v2(
                [0x28; 32],
                "shadow_optimized",
                root,
                DebugSourceVariableFallbackV2::NotInScope,
                vec![test_source_location_v2(
                    0,
                    1,
                    DebugSourceVariableBindingV2::Captured { value_ordinal: 0 },
                )],
            ),
            test_source_variable_v2(
                [0x29; 32],
                "shadow_optimized",
                inner,
                DebugSourceVariableFallbackV2::OptimizedOut,
                Vec::new(),
            ),
            test_source_variable_v2(
                [0x2a; 32],
                "shadow_unrepresented",
                root,
                DebugSourceVariableFallbackV2::NotInScope,
                vec![test_source_location_v2(
                    0,
                    1,
                    DebugSourceVariableBindingV2::Captured { value_ordinal: 0 },
                )],
            ),
            test_source_variable_v2(
                [0x2b; 32],
                "shadow_unrepresented",
                inner,
                DebugSourceVariableFallbackV2::Unrepresented,
                Vec::new(),
            ),
            test_source_variable_v2(
                [0x2c; 32],
                "shadow_not_captured",
                root,
                DebugSourceVariableFallbackV2::NotInScope,
                vec![test_source_location_v2(
                    0,
                    1,
                    DebugSourceVariableBindingV2::Captured { value_ordinal: 0 },
                )],
            ),
            test_source_variable_v2(
                [0x2d; 32],
                "shadow_not_captured",
                inner,
                DebugSourceVariableFallbackV2::NotCaptured,
                Vec::new(),
            ),
        ];
        let mut backend = backend_with_test_source_map_v2(scopes, variables);

        seek_test_checkpoint(&mut backend, 0);
        let SourceVariableResponseV2::Ok { values, .. } = inspect_source_variables_v2(
            &mut backend,
            1,
            Some(1),
            SourceVariableSelectorV2::Identity {
                variable_identity: nonzero_identity(buffer),
            },
            PageRequestV1 {
                cursor: None,
                limit: 1,
            },
        ) else {
            panic!("checkpoint-zero parameter query must succeed")
        };
        assert!(matches!(
            values.as_slice(),
            [SourceVariableValueV2 {
                generation: 1,
                availability: SourceVariableValueAvailabilityV2::Value {
                    value: ValueAvailabilityV1::Captured {
                        value_type: DebugValueTypeV1::Pointer { .. },
                        value: CapturedValueV1::AllocationRelativePointer { .. },
                        provenance: ValueProvenanceV1::SimulatedObservation,
                    }
                },
                ..
            }]
        ));
        for (request_id, variable_identity, reason) in [
            (10, optimized, ValueUnavailableReasonV1::OptimizedOut),
            (11, unrepresented, ValueUnavailableReasonV1::NotRepresented),
            (12, not_captured, ValueUnavailableReasonV1::NotCaptured),
        ] {
            let SourceVariableResponseV2::Ok { values, .. } = inspect_source_variables_v2(
                &mut backend,
                request_id,
                Some(1),
                SourceVariableSelectorV2::Identity {
                    variable_identity: nonzero_identity(variable_identity),
                },
                PageRequestV1 {
                    cursor: None,
                    limit: 1,
                },
            ) else {
                panic!("typed unavailable source variable query must succeed")
            };
            assert_eq!(values[0].generation, 0);
            assert!(matches!(
                values[0].availability,
                SourceVariableValueAvailabilityV2::Value {
                    value: ValueAvailabilityV1::Unavailable { reason: actual }
                } if actual == reason
            ));
        }
        assert!(matches!(
            inspect_source_variables_v2(
                &mut backend,
                13,
                Some(1),
                SourceVariableSelectorV2::Name {
                    name: "missing".into(),
                },
                PageRequestV1 {
                    cursor: None,
                    limit: 1,
                },
            ),
            SourceVariableResponseV2::Unavailable {
                reason: SourceVariableQueryUnavailableReasonV2::NameNotInScope,
                ..
            }
        ));
        for (request_id, name, variable_identity, reason) in [
            (
                14,
                "shadow_optimized",
                [0x29; 32],
                ValueUnavailableReasonV1::OptimizedOut,
            ),
            (
                15,
                "shadow_unrepresented",
                [0x2b; 32],
                ValueUnavailableReasonV1::NotRepresented,
            ),
            (
                16,
                "shadow_not_captured",
                [0x2d; 32],
                ValueUnavailableReasonV1::NotCaptured,
            ),
        ] {
            let SourceVariableResponseV2::Ok { values, .. } = inspect_source_variables_v2(
                &mut backend,
                request_id,
                Some(1),
                SourceVariableSelectorV2::Name { name: name.into() },
                PageRequestV1 {
                    cursor: None,
                    limit: 2,
                },
            ) else {
                panic!("inner fallback must shadow an outer captured value")
            };
            assert!(matches!(
                values.as_slice(),
                [SourceVariableValueV2 {
                    variable_identity: actual_identity,
                    scope_depth: 1,
                    generation: 0,
                    availability: SourceVariableValueAvailabilityV2::Value {
                        value: ValueAvailabilityV1::Unavailable { reason: actual_reason }
                    },
                    ..
                }] if *actual_identity == nonzero_identity(variable_identity)
                    && *actual_reason == reason
            ));
        }

        seek_test_checkpoint(&mut backend, 1);
        let SourceVariableResponseV2::Ok { values, .. } = inspect_source_variables_v2(
            &mut backend,
            2,
            Some(1),
            SourceVariableSelectorV2::Name {
                name: "item".into(),
            },
            PageRequestV1 {
                cursor: None,
                limit: 2,
            },
        ) else {
            panic!("inner shadow query must succeed")
        };
        assert!(matches!(
            values.as_slice(),
            [SourceVariableValueV2 {
                variable_identity,
                scope_depth: 1,
                generation: 1,
                availability: SourceVariableValueAvailabilityV2::Value {
                    value: ValueAvailabilityV1::Captured {
                        value_type: DebugValueTypeV1::Index { bits: 64 },
                        ..
                    }
                },
                ..
            }] if *variable_identity == nonzero_identity(inner_item)
        ));

        seek_test_checkpoint(&mut backend, 2);
        let SourceVariableResponseV2::Ok { values, .. } = inspect_source_variables_v2(
            &mut backend,
            3,
            Some(1),
            SourceVariableSelectorV2::Name {
                name: "item".into(),
            },
            PageRequestV1 {
                cursor: None,
                limit: 2,
            },
        ) else {
            panic!("outer shadow query must succeed")
        };
        assert!(matches!(
            values.as_slice(),
            [SourceVariableValueV2 {
                variable_identity,
                scope_depth: 0,
                ..
            }] if *variable_identity == nonzero_identity(outer_item)
        ));
        let SourceVariableResponseV2::Ok { values, .. } = inspect_source_variables_v2(
            &mut backend,
            4,
            Some(1),
            SourceVariableSelectorV2::Identity {
                variable_identity: nonzero_identity(state),
            },
            PageRequestV1 {
                cursor: None,
                limit: 1,
            },
        ) else {
            panic!("lifetime-reset query must succeed")
        };
        assert!(matches!(
            values[0].availability,
            SourceVariableValueAvailabilityV2::Value {
                value: ValueAvailabilityV1::Unavailable {
                    reason: ValueUnavailableReasonV1::NotInScope,
                }
            }
        ));
        assert_eq!(values[0].generation, 0);

        seek_test_checkpoint(&mut backend, 3);
        let SourceVariableResponseV2::Ok { values, .. } = inspect_source_variables_v2(
            &mut backend,
            5,
            Some(1),
            SourceVariableSelectorV2::Identity {
                variable_identity: nonzero_identity(state),
            },
            PageRequestV1 {
                cursor: None,
                limit: 1,
            },
        ) else {
            panic!("second-generation query must succeed")
        };
        assert_eq!(values[0].generation, 2);
        assert!(matches!(
            values[0].availability,
            SourceVariableValueAvailabilityV2::Value {
                value: ValueAvailabilityV1::Unavailable {
                    reason: ValueUnavailableReasonV1::Uninitialized,
                }
            }
        ));

        seek_test_checkpoint(&mut backend, 1);
        let SourceVariableResponseV2::Ok { values, .. } = inspect_source_variables_v2(
            &mut backend,
            6,
            Some(1),
            SourceVariableSelectorV2::Identity {
                variable_identity: nonzero_identity(state),
            },
            PageRequestV1 {
                cursor: None,
                limit: 1,
            },
        ) else {
            panic!("reverse replay query must succeed")
        };
        assert_eq!(values[0].generation, 1);
        assert!(matches!(
            values[0].availability,
            SourceVariableValueAvailabilityV2::Value {
                value: ValueAvailabilityV1::Captured { .. }
            }
        ));
    }

    #[test]
    fn source_variable_admission_and_queries_are_hostile_and_page_bounded() {
        let root = [0x31; 32];
        let mut variables = Vec::new();
        for ordinal in 0_u8..96 {
            variables.push(test_source_variable_v2(
                [0x80_u8.wrapping_add(ordinal); 32],
                &format!("value_{ordinal}"),
                root,
                DebugSourceVariableFallbackV2::NotCaptured,
                Vec::new(),
            ));
        }
        variables.push(test_source_variable_v2(
            [0x70; 32],
            "ambiguous",
            root,
            DebugSourceVariableFallbackV2::NotInScope,
            vec![test_source_location_v2(
                0,
                1,
                DebugSourceVariableBindingV2::Captured { value_ordinal: 0 },
            )],
        ));
        variables.push(test_source_variable_v2(
            [0x71; 32],
            "ambiguous",
            root,
            DebugSourceVariableFallbackV2::NotInScope,
            vec![test_source_location_v2(
                0,
                1,
                DebugSourceVariableBindingV2::Captured { value_ordinal: 0 },
            )],
        ));
        let mut backend =
            backend_with_test_source_map_v2(vec![test_source_scope_v2(root, None, 0)], variables);
        seek_test_checkpoint(&mut backend, 0);

        let SourceVariableResponseV2::Ok {
            values,
            next_cursor: Some(next_cursor),
            ..
        } = inspect_source_variables_v2(
            &mut backend,
            1,
            Some(1),
            SourceVariableSelectorV2::All,
            PageRequestV1 {
                cursor: None,
                limit: 3,
            },
        )
        else {
            panic!("first bounded All page must succeed")
        };
        assert_eq!(values.len(), 3);
        let SourceVariableResponseV2::Ok { values, .. } = inspect_source_variables_v2(
            &mut backend,
            2,
            Some(1),
            SourceVariableSelectorV2::All,
            PageRequestV1 {
                cursor: Some(next_cursor),
                limit: 3,
            },
        ) else {
            panic!("second bounded All page must succeed")
        };
        assert_eq!(values.len(), 3);

        let SourceVariableResponseV2::Ok { values, .. } = inspect_source_variables_v2(
            &mut backend,
            3,
            Some(1),
            SourceVariableSelectorV2::Name {
                name: "ambiguous".into(),
            },
            PageRequestV1 {
                cursor: None,
                limit: 2,
            },
        ) else {
            panic!("ambiguous name query must remain typed")
        };
        assert_eq!(values.len(), 2);
        assert!(values.iter().all(|value| matches!(
            value.availability,
            SourceVariableValueAvailabilityV2::Ambiguous
        )));
        assert!(matches!(
            inspect_source_variables_v2(
                &mut backend,
                4,
                Some(99),
                SourceVariableSelectorV2::All,
                PageRequestV1 {
                    cursor: None,
                    limit: 1
                },
            ),
            SourceVariableResponseV2::Unavailable {
                reason: SourceVariableQueryUnavailableReasonV2::FrameUnavailable,
                ..
            }
        ));

        let input = fill_input();
        let v1 = DebugSourceMapDocumentV1::from_json_bytes(&fill_source_map()).unwrap();
        let future_result = test_source_variable_v2(
            [0x72; 32],
            "future_result",
            root,
            DebugSourceVariableFallbackV2::NotInScope,
            vec![test_source_location_v2(
                0,
                1,
                DebugSourceVariableBindingV2::Captured { value_ordinal: 1 },
            )],
        );
        let hostile = DebugSourceMapDocumentV2::new(
            v1.binding(),
            v1.files().to_vec(),
            v1.sites().to_vec(),
            v1.eliminated().to_vec(),
            vec![test_source_scope_v2(root, None, 0)],
            vec![future_result],
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap();
        assert!(
            admit_source_map_v2(
                &hostile,
                &input,
                configuration_identity(
                    input.kir_sha256,
                    input.request_sha256,
                    DebugWaveWidthV1::Wave64,
                ),
                fixture_subject(),
                nonzero_identity(simulation_debug_map_identity_v2(&hostile)),
            )
            .unwrap_err()
            .contains("unavailable at its checkpoint")
        );

        let unknown_parameter = test_source_variable_v2(
            [0x73; 32],
            "unknown_parameter",
            root,
            DebugSourceVariableFallbackV2::NotInScope,
            Vec::new(),
        )
        .with_function_binding(
            fe2o3_kernel_ir::DebugSourceVariableFunctionBindingV2::new(1, u32::MAX as u64).unwrap(),
        )
        .unwrap();
        let hostile = DebugSourceMapDocumentV2::new(
            v1.binding(),
            v1.files().to_vec(),
            v1.sites().to_vec(),
            v1.eliminated().to_vec(),
            vec![test_source_scope_v2(root, None, 0)],
            vec![unknown_parameter],
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap();
        assert!(
            admit_source_map_v2(
                &hostile,
                &input,
                configuration_identity(
                    input.kir_sha256,
                    input.request_sha256,
                    DebugWaveWidthV1::Wave64,
                ),
                fixture_subject(),
                nonzero_identity(simulation_debug_map_identity_v2(&hostile)),
            )
            .unwrap_err()
            .contains("not a KIR function parameter")
        );

        let indexed = &backend.source_variables_v2.as_ref().unwrap().variables[0];
        let truncated_frame = SimulationDebugFrameV1 {
            depth: 0,
            function_ordinal: 0,
            block: fe2o3_kernel_ir::BlockId(0),
            next_operation: Some(0),
            values: SimulationDebugCollectionV1::Unavailable {
                reason: fe2o3_kir_sim::SimulationDebugUnavailableReasonV1::ValueLimit,
                required: 1,
            },
        };
        let value = source_variable_value_v2(indexed, &truncated_frame, 0, false);
        if matches!(
            indexed.locations.first().map(|location| location.binding),
            Some(DebugSourceVariableBindingV2::Captured { .. })
        ) {
            assert!(matches!(
                value.availability,
                SourceVariableValueAvailabilityV2::Value {
                    value: ValueAvailabilityV1::Unavailable {
                        reason: ValueUnavailableReasonV1::Truncated,
                    }
                }
            ));
        }
    }

    #[test]
    fn compiler_map_handoff_verifies_committed_map_identity() {
        let input = fill_input();
        let configuration = configuration_identity(
            input.kir_sha256,
            input.request_sha256,
            DebugWaveWidthV1::Wave64,
        );
        let bytes = fill_source_map();
        let actual = debug_source_map_identity_v1(&bytes).unwrap();
        let expected: OpaqueIdentityV1 = serde_json::from_str(
            "\"5dd2d368096e36101d762b1b873b74765244af9d1b50b1a2caf27056bec41b4d\"",
        )
        .unwrap();
        assert_eq!(actual, expected);
        let admitted = admit_source_map_with_provenance_v1(
            &bytes,
            &input,
            configuration,
            fixture_subject(),
            Some(actual),
            SourceMapProvenanceV1::CompilerBundleBound,
        )
        .expect("admit bundle-committed map");
        assert_eq!(
            admitted.provenance,
            SourceMapProvenanceV1::CompilerBundleBound
        );
        assert!(
            admit_source_map_with_provenance_v1(
                &bytes,
                &input,
                configuration,
                fixture_subject(),
                Some(nonzero_identity([7; 32])),
                SourceMapProvenanceV1::CompilerBundleBound,
            )
            .unwrap_err()
            .contains("bundle commitment")
        );

        let input = fill_input();
        let wrong_configuration = nonzero_identity([9; 32]);
        let admitted =
            admit_source_map_v1(&bytes, &input, wrong_configuration, fixture_subject()).unwrap();
        assert!(
            SimulatorBackendV1::new_with_source_map(
                input,
                DebugWaveWidthV1::Wave64,
                Some(admitted),
            )
            .err()
            .expect("reject stale configuration")
            .contains("configuration identity")
        );
    }

    #[test]
    fn source_breakpoint_states_are_typed_and_ambiguous_step_is_atomic() {
        let input = fill_input();
        let configuration = configuration_identity(
            input.kir_sha256,
            input.request_sha256,
            DebugWaveWidthV1::Wave64,
        );
        let mut document: serde_json::Value = serde_json::from_slice(&fill_source_map()).unwrap();
        document["eliminated"] = serde_json::json!([{
            "file_identity": "8b9da03723f1c1902bc22d282783d38998ecf3ee4fde126135052b17e050e80b",
            "byte_start": 30,
            "byte_end": 40,
            "line": 1,
            "column": 31
        }]);
        let bytes = serde_json::to_vec(&document).unwrap();
        let source_map =
            admit_source_map_v1(&bytes, &input, configuration, fixture_subject()).unwrap();
        let backend = SimulatorBackendV1::new_with_source_map(
            input,
            DebugWaveWidthV1::Wave64,
            Some(source_map),
        )
        .unwrap();
        let source_spec = |byte_start, byte_end| BreakpointSpecV1 {
            client_label: None,
            enabled: true,
            scope: None,
            hit_condition: None,
            kind: BreakpointKindV1::Source {
                source: SourceLocationV1 {
                    map_identity: backend.source_map_identity.unwrap(),
                    provenance: SourceMapProvenanceV1::CallerBound,
                    file_identity: nonzero_identity([
                        0x8b, 0x9d, 0xa0, 0x37, 0x23, 0xf1, 0xc1, 0x90, 0x2b, 0xc2, 0x2d, 0x28,
                        0x27, 0x83, 0xd3, 0x89, 0x98, 0xec, 0xf3, 0xee, 0x4f, 0xde, 0x12, 0x61,
                        0x35, 0x05, 0x2b, 0x17, 0xe0, 0x50, 0xe8, 0x0b,
                    ]),
                    byte_start,
                    byte_end,
                },
            },
        };
        assert!(matches!(
            backend.convert_breakpoint(1, &source_spec(20, 25)),
            Err(ConvertErrorV1::Unavailable(
                DebugCapabilityNameV1::SourceSites,
                CapabilityUnavailableReasonV1::Absent,
                _
            ))
        ));
        assert!(matches!(
            backend.convert_breakpoint(1, &source_spec(30, 40)),
            Err(ConvertErrorV1::Unavailable(
                DebugCapabilityNameV1::SourceSites,
                CapabilityUnavailableReasonV1::OptimizedOut,
                _
            ))
        ));
        assert!(matches!(
            backend.convert_breakpoint(1, &source_spec(47, 68)),
            Err(ConvertErrorV1::Unavailable(
                DebugCapabilityNameV1::SourceSites,
                CapabilityUnavailableReasonV1::ManyToOne,
                _
            ))
        ));

        document["sites"][0]["spans"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "file_identity": "8b9da03723f1c1902bc22d282783d38998ecf3ee4fde126135052b17e050e80b",
                "byte_start": 20,
                "byte_end": 25,
                "line": 1,
                "column": 21
            }));
        let bytes = serde_json::to_vec(&document).unwrap();
        let input = fill_input();
        let source_map =
            admit_source_map_v1(&bytes, &input, configuration, fixture_subject()).unwrap();
        let mut backend = SimulatorBackendV1::new_with_source_map(
            input,
            DebugWaveWidthV1::Wave64,
            Some(source_map),
        )
        .unwrap();
        let operation_zero = backend
            .session
            .transcript()
            .records()
            .iter()
            .position(|record| record.site.operation == 0)
            .unwrap();
        backend.session.seek_record_index(operation_zero);
        let before = backend.cursor_sequence();
        let response = backend.step(
            1,
            StepDirectionV1::Forward,
            StepGranularityV1::Source,
            1,
            None,
        );
        assert!(matches!(
            response,
            DebugResponseV1::Unavailable {
                unavailable: CapabilityUnavailableV1 {
                    reason: CapabilityUnavailableReasonV1::ManyToOne,
                    state_changed: false,
                    ..
                },
                ..
            }
        ));
        assert_eq!(backend.cursor_sequence(), before);
        assert_eq!(backend.revision, 0);
    }

    #[test]
    fn page_cursor_is_bound_to_query_identity_and_range() {
        let first = nonzero_identity([1; 32]);
        let other = nonzero_identity([2; 32]);
        let page = PageRequestV1 {
            cursor: Some(PageCursorV1 {
                query_identity: first,
                position: 2,
            }),
            limit: 2,
        };
        assert_eq!(
            page_bounds(page, first, 5),
            Ok((
                2,
                4,
                Some(PageCursorV1 {
                    query_identity: first,
                    position: 4,
                })
            ))
        );
        assert!(page_bounds(page, other, 5).is_err());
        assert!(
            page_bounds(
                PageRequestV1 {
                    cursor: Some(PageCursorV1 {
                        query_identity: first,
                        position: 6,
                    }),
                    limit: 1,
                },
                first,
                5,
            )
            .is_err()
        );
    }

    #[test]
    fn error_text_is_control_free_nonempty_and_bounded() {
        let message = bounded_message(&format!("\0{}\n", "x".repeat(MAX_TEXT_BYTES_V1 + 10)));
        assert!(!message.is_empty());
        assert!(message.len() <= MAX_TEXT_BYTES_V1);
        assert!(!message.chars().any(char::is_control));
    }

    #[test]
    fn barrier_residency_persists_and_aggregate_mixed_state_is_deterministic() {
        let records = vec![
            barrier_record(
                0,
                0,
                SimulationDebugRecordKindV1::WorkgroupBarrier {
                    action: SimulationDebugBarrierActionV1::Arrive,
                    phase: 0,
                    participants: 1,
                },
            ),
            barrier_record(1, 0, empty_checkpoint()),
            barrier_record(
                2,
                1,
                SimulationDebugRecordKindV1::WorkgroupBarrier {
                    action: SimulationDebugBarrierActionV1::Arrive,
                    phase: 0,
                    participants: 1,
                },
            ),
            barrier_record(3, 1, empty_checkpoint()),
            barrier_record(
                4,
                0,
                SimulationDebugRecordKindV1::WorkgroupBarrier {
                    action: SimulationDebugBarrierActionV1::Release,
                    phase: 0,
                    participants: 2,
                },
            ),
            barrier_record(5, 1, empty_checkpoint()),
        ];
        let scopes = [
            ExecutionScopeV1::Dispatch,
            ExecutionScopeV1::Workgroup {
                workgroup: [0, 0, 0],
            },
            ExecutionScopeV1::Wave {
                workgroup: [0, 0, 0],
                wave: 0,
                active_mask: 0b11,
                wave_width: 32,
                interpretation: WaveInterpretationV1::LogicalVisualization,
            },
            ExecutionScopeV1::Lane {
                workgroup: [0, 0, 0],
                wave: 0,
                lane: 0,
                logical_workitem: [0, 0, 0],
                active_mask: 0b11,
                wave_width: 32,
                interpretation: WaveInterpretationV1::LogicalVisualization,
            },
            ExecutionScopeV1::Lane {
                workgroup: [0, 0, 0],
                wave: 0,
                lane: 1,
                logical_workitem: [1, 0, 0],
                active_mask: 0b11,
                wave_width: 32,
                interpretation: WaveInterpretationV1::LogicalVisualization,
            },
        ];
        let states = |cursor| {
            scope_states_for_records(
                &records,
                None,
                Some(cursor),
                &scopes,
                DebugWaveWidthV1::Wave32,
            )
        };

        assert_eq!(
            states(0),
            vec![
                ScopeStateV1::Runnable,
                ScopeStateV1::Runnable,
                ScopeStateV1::Runnable,
                ScopeStateV1::BarrierBlocked,
                ScopeStateV1::NotStarted,
            ]
        );
        assert_eq!(states(1)[3], ScopeStateV1::BarrierBlocked);
        assert_eq!(states(2), vec![ScopeStateV1::BarrierBlocked; scopes.len()]);
        assert_eq!(states(3), vec![ScopeStateV1::BarrierBlocked; scopes.len()]);
        assert_eq!(
            states(4),
            vec![
                ScopeStateV1::Running,
                ScopeStateV1::Running,
                ScopeStateV1::Running,
                ScopeStateV1::Running,
                ScopeStateV1::Runnable,
            ]
        );
        assert_eq!(
            states(5),
            vec![
                ScopeStateV1::Running,
                ScopeStateV1::Running,
                ScopeStateV1::Running,
                ScopeStateV1::Completed,
                ScopeStateV1::Running,
            ]
        );
    }
}
