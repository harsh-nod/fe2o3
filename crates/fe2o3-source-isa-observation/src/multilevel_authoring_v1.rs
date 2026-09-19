//! Bounded, authority-free authoring observations over the existing V6 bundle.
//!
//! The only decoded program is the bundle's exact canonical V11 module. Reports
//! and generated Rust drafts are inert projections: neither admits an edited
//! intermediate, authenticates source, nor establishes a source insertion site.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::io;

#[path = "ordered_program_materialization_v1.rs"]
pub mod ordered_program_materialization_v1;

use fe2o3_kernel_ir::{
    AssemblyOption, BinaryOp, DebugSourceMapDocumentV2, DebugSourceMapSpanV1, Function, Module,
    Operation, OperationKind, ScalarType, Terminator, Type, ValueId, VerifiedSimulationBundleV6,
    decode_module_v11, validate_gfx942_inline_assembly_v1,
};
use serde::{Deserialize, Serialize};

pub const MAX_AUTHORING_PAGE_ITEMS_V1: u32 = 64;
pub const MAX_AUTHORING_REGION_OPERATIONS_V1: usize = 64;
pub const MAX_AUTHORING_OPERATIONS_V1: usize = 65_536;
pub const MAX_AUTHORING_DEFINITIONS_V1: usize = 65_536;
pub const MAX_AUTHORING_SCAN_ITEMS_V1: usize = 1_048_576;
pub const MAX_AUTHORING_REGION_VALUES_V1: usize = 256;
pub const MAX_AUTHORING_SOURCE_BYTES_V1: usize = 64 * 1024;
pub const MAX_AUTHORING_REPORT_BYTES_V1: usize = 256 * 1024;
const MAX_TEXT_BYTES: usize = 4096;

/// Immutable canonical roster coordinate; meaningful only with exact identities.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringOperationCoordinateV1 {
    pub function: u32,
    pub block: u32,
    pub operation: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringRegionSelectorV1 {
    pub bundle_identity: String,
    pub canonical_kir_digest: String,
    pub target: String,
    pub operations: Vec<AuthoringOperationCoordinateV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringAuthorityV1 {
    pub observation_only: bool,
    pub authenticates_compiler_execution: bool,
    pub source_authenticated: bool,
    pub grants_proof_authority: bool,
    pub grants_production_resume: bool,
    pub grants_load_or_launch: bool,
}

const AUTHORITY: AuthoringAuthorityV1 = AuthoringAuthorityV1 {
    observation_only: true,
    authenticates_compiler_execution: false,
    source_authenticated: false,
    grants_proof_authority: false,
    grants_production_resume: false,
    grants_load_or_launch: false,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringCapabilityV1 {
    pub level: &'static str,
    pub read: &'static str,
    pub select: &'static str,
    pub materialize: &'static str,
    pub edit: &'static str,
    pub readmit: &'static str,
    pub simulate: &'static str,
    pub inspect: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringSnapshotSummaryV1 {
    pub schema: &'static str,
    pub authority: AuthoringAuthorityV1,
    pub bundle_identity: String,
    pub bundle_subject_identity: String,
    pub canonical_kir_version: u16,
    pub canonical_kir_digest: String,
    pub canonical_kir_bytes: String,
    pub target: String,
    pub source_map_identity: String,
    pub semantic_mir_identity: String,
    pub rustc_identity_inventory_receipt_sha256: String,
    pub rustc_identity_inventory_receipt_bytes: String,
    pub rustc_preflight_plan_receipt_sha256: String,
    pub rustc_preflight_plan_receipt_bytes: String,
    pub compiler_policy_identity: &'static str,
    pub final_artifact_identity: &'static str,
    pub operation_count: u32,
    pub eliminated_source_span_count: u32,
    pub capabilities: Vec<AuthoringCapabilityV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringValueV1 {
    /// Function-local SSA label, never a cross-build identity or physical register.
    pub value: u32,
    pub ty: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringSourceSpanV1 {
    pub file_identity: String,
    pub display_path: String,
    pub byte_start: String,
    pub byte_end: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringAssemblySourceV1 {
    pub frontend_unit: String,
    pub function: String,
    pub contract: String,
    pub statement: String,
    /// These are exact retained references, not independent source authentication.
    pub authority: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringOperationV1 {
    pub coordinate: AuthoringOperationCoordinateV1,
    pub function_name: String,
    pub kind: &'static str,
    /// Bounded scalar opcode or literal; absent for operation families not projected here.
    pub semantic_detail: Option<String>,
    pub mnemonic: Option<String>,
    pub inline_assembly_source: Option<AuthoringAssemblySourceV1>,
    pub inputs: Vec<AuthoringValueV1>,
    pub results: Vec<AuthoringValueV1>,
    pub local_memory_effects: Vec<String>,
    pub complete_local_effect_summary: bool,
    pub convergence: &'static str,
    pub traps: &'static str,
    pub physical_resources: &'static str,
    pub source_binding: &'static str,
    pub source_spans: Vec<AuthoringSourceSpanV1>,
    pub materialization: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringOperationPageV1 {
    pub authority: AuthoringAuthorityV1,
    pub bundle_identity: String,
    pub canonical_kir_digest: String,
    pub target: String,
    pub start: u32,
    pub next_start: Option<u32>,
    pub total_operations: u32,
    pub operations: Vec<AuthoringOperationV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringRegionV1 {
    pub authority: AuthoringAuthorityV1,
    pub selector: AuthoringRegionSelectorV1,
    pub structural_boundary: &'static str,
    pub source_insertion_boundary: &'static str,
    pub live_in: Vec<AuthoringValueV1>,
    pub live_out: Vec<AuthoringValueV1>,
    pub operations: Vec<AuthoringOperationV1>,
    pub materialization: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AuthoringRustCandidateV1 {
    pub authority: AuthoringAuthorityV1,
    pub selector: AuthoringRegionSelectorV1,
    pub helper_name: String,
    pub source: String,
    pub live_in: Vec<AuthoringValueV1>,
    pub live_out: Vec<AuthoringValueV1>,
    pub status: &'static str,
    pub frontend_readmission: &'static str,
    pub source_application: &'static str,
    pub semantic_equivalence: &'static str,
    pub exact_machine_contract: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoringErrorV1 {
    InvalidCanonicalInput,
    InvalidSourceMap,
    ResourceLimit,
    StaleBundleIdentity,
    StaleCanonicalIdentity,
    IncompatibleTarget,
    InvalidPage,
    EmptySelection,
    InvalidCoordinate,
    NonContiguousSelection,
    InvalidBoundary,
    UnsupportedMaterialization,
    InvalidHelperName,
}

impl fmt::Display for AuthoringErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCanonicalInput => "invalid canonical V11 input in the V6 bundle",
            Self::InvalidSourceMap => {
                "source map does not identify this canonical operation roster"
            }
            Self::ResourceLimit => "authoring observation exceeds its bounded resource profile",
            Self::StaleBundleIdentity => "stale or malformed exact V6 bundle identity",
            Self::StaleCanonicalIdentity => "stale or malformed exact canonical V11 identity",
            Self::IncompatibleTarget => "selector target differs from the exact bundle target",
            Self::InvalidPage => "invalid operation page; limit must be 1..=64 and start in range",
            Self::EmptySelection => "region selection must name at least one operation",
            Self::InvalidCoordinate => "operation coordinate is outside the exact canonical roster",
            Self::NonContiguousSelection => {
                "selection must be strictly consecutive operations in one block"
            }
            Self::InvalidBoundary => {
                "selected region has an unavailable or invalid typed value boundary"
            }
            Self::UnsupportedMaterialization => {
                "selection is outside the diagnostic u32 gfx942 typed-ISA draft profile"
            }
            Self::InvalidHelperName => {
                "helper name must be a non-keyword ASCII Rust identifier of at most 64 bytes"
            }
        })
    }
}

impl std::error::Error for AuthoringErrorV1 {}
type Result<T> = std::result::Result<T, AuthoringErrorV1>;

#[derive(Clone, Copy, Debug)]
enum Definition {
    Parameter(usize),
    BlockParameter(usize, usize),
    Result(usize, usize, usize),
}

/// Owns one immutable decode of the existing canonical program and inert indexes.
/// No mutable program, session handle, compiler receipt, or resumed executable is exposed.
#[derive(Debug)]
pub struct AuthoringSnapshotV1 {
    bundle: VerifiedSimulationBundleV6,
    module: Module,
    source_map: DebugSourceMapDocumentV2,
    operations: Vec<AuthoringOperationCoordinateV1>,
    definitions: Vec<BTreeMap<ValueId, Definition>>,
    source_sites: BTreeMap<AuthoringOperationCoordinateV1, usize>,
    source_files: BTreeMap<[u8; 32], usize>,
}

impl AuthoringSnapshotV1 {
    pub fn from_bundle_v6(bundle: VerifiedSimulationBundleV6) -> Result<Self> {
        // The move-only bundle already verified these exact bytes. This is the
        // canonical decoder, not an importer or a competing executable format.
        let module = decode_module_v11(bundle.canonical_kir_v11())
            .map_err(|_| AuthoringErrorV1::InvalidCanonicalInput)?;
        let source_map = DebugSourceMapDocumentV2::from_canonical_json_bytes(bundle.debug_map())
            .map_err(|_| AuthoringErrorV1::InvalidSourceMap)?;
        let mut operations = Vec::new();
        let mut definitions = Vec::new();
        let mut definition_count = 0;
        let mut scan = 0;
        let mut source_files = BTreeMap::new();
        for (index, file) in source_map.files().iter().enumerate() {
            charge(&mut scan, 1)?;
            if source_files.insert(file.identity(), index).is_some() {
                return Err(AuthoringErrorV1::InvalidSourceMap);
            }
        }
        for (function_index, function) in module.functions.iter().enumerate() {
            charge(&mut scan, 1)?;
            let mut values = BTreeMap::new();
            if let Some(body) = &function.body {
                for (index, value) in body.parameters.iter().enumerate() {
                    insert_definition(
                        &mut values,
                        *value,
                        Definition::Parameter(index),
                        &mut definition_count,
                    )?;
                }
                for (block_index, block) in body.blocks.iter().enumerate() {
                    charge(&mut scan, 1)?;
                    for (index, value) in block.parameters.iter().enumerate() {
                        insert_definition(
                            &mut values,
                            value.id,
                            Definition::BlockParameter(block_index, index),
                            &mut definition_count,
                        )?;
                    }
                    for (operation_index, operation) in block.operations.iter().enumerate() {
                        if operations.len() == MAX_AUTHORING_OPERATIONS_V1 {
                            return Err(AuthoringErrorV1::ResourceLimit);
                        }
                        charge(&mut scan, 1)?;
                        operation
                            .kind
                            .try_visit_operands(|_| charge(&mut scan, 1))?;
                        for (index, value) in operation.results.iter().enumerate() {
                            insert_definition(
                                &mut values,
                                value.id,
                                Definition::Result(block_index, operation_index, index),
                                &mut definition_count,
                            )?;
                        }
                        operations.push(AuthoringOperationCoordinateV1 {
                            function: ordinal(function_index)?,
                            block: ordinal(block_index)?,
                            operation: ordinal(operation_index)?,
                        });
                    }
                    visit_terminator(
                        block
                            .terminator
                            .as_ref()
                            .ok_or(AuthoringErrorV1::InvalidCanonicalInput)?,
                        |_, count| charge(&mut scan, count),
                    )?;
                }
            }
            definitions.push(values);
        }
        let mut source_sites = BTreeMap::new();
        for (index, site) in source_map.sites().iter().enumerate() {
            charge(&mut scan, site.spans().len().saturating_add(1))?;
            let position = site.site();
            let coordinate = AuthoringOperationCoordinateV1 {
                function: u32::try_from(position.function_ordinal())
                    .map_err(|_| AuthoringErrorV1::InvalidSourceMap)?,
                block: u32::try_from(position.block_ordinal())
                    .map_err(|_| AuthoringErrorV1::InvalidSourceMap)?,
                operation: u32::try_from(position.operation_ordinal())
                    .map_err(|_| AuthoringErrorV1::InvalidSourceMap)?,
            };
            if operations.binary_search(&coordinate).is_err()
                || source_sites.insert(coordinate, index).is_some()
            {
                return Err(AuthoringErrorV1::InvalidSourceMap);
            }
        }
        Ok(Self {
            bundle,
            module,
            source_map,
            operations,
            definitions,
            source_sites,
            source_files,
        })
    }

    pub fn summary(&self) -> AuthoringSnapshotSummaryV1 {
        let lineage = self.bundle.source_lineage();
        let unavailable = |level| AuthoringCapabilityV1 {
            level,
            read: "unavailable",
            select: "unavailable",
            materialize: "unavailable",
            edit: "unavailable",
            readmit: "unavailable",
            simulate: "unavailable",
            inspect: "unavailable",
        };
        AuthoringSnapshotSummaryV1 {
            schema: "fe2o3-multilevel-authoring-observation-v1",
            authority: AUTHORITY,
            bundle_identity: hex(self.bundle.identity().as_bytes()),
            bundle_subject_identity: hex(self.bundle.subject_identity()),
            canonical_kir_version: 11,
            canonical_kir_digest: hex(self.bundle.canonical_kir_v11_digest()),
            canonical_kir_bytes: self.bundle.canonical_kir_v11_length().to_string(),
            target: self.bundle.target().to_owned(),
            source_map_identity: hex(&self.bundle.debug_map_identity()),
            semantic_mir_identity: hex(&self.bundle.semantic_mir_identity()),
            rustc_identity_inventory_receipt_sha256: hex(
                &lineage.rustc_identity_inventory_receipt_sha256()
            ),
            rustc_identity_inventory_receipt_bytes: lineage
                .rustc_identity_inventory_receipt_bytes()
                .to_string(),
            rustc_preflight_plan_receipt_sha256: hex(&lineage.rustc_preflight_plan_receipt_sha256()),
            rustc_preflight_plan_receipt_bytes: lineage
                .rustc_preflight_plan_receipt_bytes()
                .to_string(),
            compiler_policy_identity: "unavailable_in_v6",
            final_artifact_identity: "unavailable_extraction_precedes_final_artifact",
            operation_count: self.operations.len() as u32,
            eliminated_source_span_count: self.source_map.eliminated().len() as u32,
            capabilities: vec![
                AuthoringCapabilityV1 {
                    level: "structured_rust",
                    read: "source_locations_only",
                    ..unavailable("structured_rust")
                },
                unavailable("scheduled_tile_rust"),
                AuthoringCapabilityV1 {
                    level: "canonical_simt_v11",
                    read: "available",
                    select: "bounded_contiguous_block",
                    materialize: "bounded_u32_bitwise_or_validated_inline_isa_draft",
                    edit: "external_source_edit_only",
                    readmit: "requires_fresh_source_compilation_supported_subset",
                    simulate: "separate_existing_simulator_with_coverage_checks",
                    inspect: "available",
                },
                unavailable("physical_register_exact_region"),
            ],
        }
    }

    pub fn operation_page(
        &self,
        expected_bundle_identity: &str,
        start: u32,
        limit: u32,
    ) -> Result<AuthoringOperationPageV1> {
        self.check_bundle(expected_bundle_identity)?;
        if limit == 0
            || limit > MAX_AUTHORING_PAGE_ITEMS_V1
            || start as usize > self.operations.len()
        {
            return Err(AuthoringErrorV1::InvalidPage);
        }
        let end = (start as usize + limit as usize).min(self.operations.len());
        let operations = self.operations[start as usize..end]
            .iter()
            .map(|coordinate| self.operation_view(*coordinate))
            .collect::<Result<Vec<_>>>()?;
        bounded_report(AuthoringOperationPageV1 {
            authority: AUTHORITY,
            bundle_identity: hex(self.bundle.identity().as_bytes()),
            canonical_kir_digest: hex(self.bundle.canonical_kir_v11_digest()),
            target: self.bundle.target().to_owned(),
            start,
            next_start: (end < self.operations.len()).then_some(end as u32),
            total_operations: self.operations.len() as u32,
            operations,
        })
    }

    pub fn select_region(&self, selector: &AuthoringRegionSelectorV1) -> Result<AuthoringRegionV1> {
        self.check_selector(selector)?;
        let first = selector.operations[0];
        let mut produced = BTreeSet::new();
        let mut inputs = BTreeSet::new();
        let mut live_in = Vec::new();
        let mut views = Vec::new();
        for coordinate in &selector.operations {
            let operation = self.operation(*coordinate)?;
            operation.kind.try_visit_operands(|value| {
                if !produced.contains(&value) && inputs.insert(value) {
                    if live_in.len() == MAX_AUTHORING_REGION_VALUES_V1 {
                        return Err(AuthoringErrorV1::ResourceLimit);
                    }
                    live_in.push(self.value_view(first.function, value)?);
                }
                Ok(())
            })?;
            for result in &operation.results {
                if produced.len() == MAX_AUTHORING_REGION_VALUES_V1 {
                    return Err(AuthoringErrorV1::ResourceLimit);
                }
                produced.insert(result.id);
            }
            views.push(self.operation_view(*coordinate)?);
        }
        // A value defined inside the selection cannot be an incoming capture.
        if inputs.iter().any(|value| produced.contains(value)) {
            return Err(AuthoringErrorV1::InvalidBoundary);
        }
        let function = &self.module.functions[first.function as usize];
        let body = function
            .body
            .as_ref()
            .ok_or(AuthoringErrorV1::InvalidBoundary)?;
        let last = selector
            .operations
            .last()
            .ok_or(AuthoringErrorV1::EmptySelection)?
            .operation;
        let mut external_uses = BTreeSet::new();
        for (block_index, block) in body.blocks.iter().enumerate() {
            for (operation_index, operation) in block.operations.iter().enumerate() {
                if block_index == first.block as usize
                    && (first.operation as usize..=last as usize).contains(&operation_index)
                {
                    continue;
                }
                operation.kind.try_visit_operands(|value| {
                    if produced.contains(&value) {
                        external_uses.insert(value);
                    }
                    Ok::<_, AuthoringErrorV1>(())
                })?;
            }
            visit_terminator(
                block
                    .terminator
                    .as_ref()
                    .ok_or(AuthoringErrorV1::InvalidBoundary)?,
                |value, _| {
                    if let Some(value) = value
                        && produced.contains(&value)
                    {
                        external_uses.insert(value);
                    }
                    Ok(())
                },
            )?;
        }
        let mut live_out = Vec::new();
        // Preserve definition order, independently of numeric SSA labels.
        for coordinate in &selector.operations {
            for result in &self.operation(*coordinate)?.results {
                if external_uses.contains(&result.id) {
                    live_out.push(self.value_view(first.function, result.id)?);
                }
            }
        }
        let supported = views
            .iter()
            .all(|view| view.materialization == "diagnostic_rust_draft_available");
        bounded_report(AuthoringRegionV1 {
            authority: AUTHORITY,
            selector: selector.clone(),
            structural_boundary: "contiguous_operations_in_one_block_no_terminator_selected",
            source_insertion_boundary: "unavailable_source_application_not_admitted",
            live_in,
            live_out,
            operations: views,
            materialization: if supported {
                "diagnostic_rust_draft_available"
            } else {
                "unavailable_unsupported_operation_or_contract"
            },
        })
    }

    pub fn materialize_typed_rust(
        &self,
        selector: &AuthoringRegionSelectorV1,
        helper_name: &str,
    ) -> Result<AuthoringRustCandidateV1> {
        if !valid_helper_name(helper_name) {
            return Err(AuthoringErrorV1::InvalidHelperName);
        }
        let region = self.select_region(selector)?;
        if region.materialization != "diagnostic_rust_draft_available" {
            return Err(AuthoringErrorV1::UnsupportedMaterialization);
        }
        let mut text = BoundedText::new(MAX_AUTHORING_SOURCE_BYTES_V1);
        writeln!(text, "// Diagnostic source draft; source authentication and fresh frontend readmission have not been performed.")
            .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        writeln!(
            text,
            "// Exact baseline bundle: {}",
            selector.bundle_identity
        )
        .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        writeln!(text, "// Target: gfx942:xnack-; logical operands; physical allocation remains compiler-owned.").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        write!(text, "pub fn {helper_name}(").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        for (index, value) in region.live_in.iter().enumerate() {
            write!(
                text,
                "{}v{}: u32",
                if index == 0 { "" } else { ", " },
                value.value
            )
            .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        }
        write!(text, ") -> (").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        for _ in &region.live_out {
            write!(text, "u32,").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        }
        writeln!(text, ") {{").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        for coordinate in &selector.operations {
            let instruction = self
                .draft_instruction(*coordinate, self.operation(*coordinate)?)
                .ok_or(AuthoringErrorV1::UnsupportedMaterialization)?;
            write!(
                text,
                "    let v{}: u32 = fe2o3_device::amdgpu_asm!({}(",
                instruction.result.0, instruction.mnemonic
            )
            .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
            for (index, input) in instruction.inputs.iter().enumerate() {
                write!(text, "{}v{}", if index == 0 { "" } else { ", " }, input.0)
                    .map_err(|_| AuthoringErrorV1::ResourceLimit)?;
            }
            writeln!(text, "));").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        }
        write!(text, "    (").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        for value in &region.live_out {
            write!(text, "v{},", value.value).map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        }
        writeln!(text, ")\n}}").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
        bounded_report(AuthoringRustCandidateV1 {
            authority: AUTHORITY,
            selector: selector.clone(),
            helper_name: helper_name.to_owned(),
            source: text.text,
            live_in: region.live_in,
            live_out: region.live_out,
            status: "diagnostic_source_draft_only",
            frontend_readmission: "not_performed_requires_fresh_source_compilation",
            source_application: "unavailable_requires_explicit_new_source_and_normal_frontend",
            semantic_equivalence: "unproved",
            exact_machine_contract: "unproved",
        })
    }

    fn check_bundle(&self, identity: &str) -> Result<()> {
        if identity != hex(self.bundle.identity().as_bytes()) {
            return Err(AuthoringErrorV1::StaleBundleIdentity);
        }
        Ok(())
    }

    fn check_selector(&self, selector: &AuthoringRegionSelectorV1) -> Result<()> {
        self.check_bundle(&selector.bundle_identity)?;
        if selector.canonical_kir_digest != hex(self.bundle.canonical_kir_v11_digest()) {
            return Err(AuthoringErrorV1::StaleCanonicalIdentity);
        }
        if selector.target != self.bundle.target() {
            return Err(AuthoringErrorV1::IncompatibleTarget);
        }
        if selector.operations.is_empty() {
            return Err(AuthoringErrorV1::EmptySelection);
        }
        if selector.operations.len() > MAX_AUTHORING_REGION_OPERATIONS_V1 {
            return Err(AuthoringErrorV1::ResourceLimit);
        }
        let first = selector.operations[0];
        for (index, coordinate) in selector.operations.iter().enumerate() {
            self.operation(*coordinate)?;
            if coordinate.function != first.function
                || coordinate.block != first.block
                || first.operation.checked_add(index as u32) != Some(coordinate.operation)
            {
                return Err(AuthoringErrorV1::NonContiguousSelection);
            }
        }
        Ok(())
    }

    fn operation(&self, coordinate: AuthoringOperationCoordinateV1) -> Result<&Operation> {
        self.module
            .functions
            .get(coordinate.function as usize)
            .and_then(|function| function.body.as_ref())
            .and_then(|body| body.blocks.get(coordinate.block as usize))
            .and_then(|block| block.operations.get(coordinate.operation as usize))
            .ok_or(AuthoringErrorV1::InvalidCoordinate)
    }

    fn value_type(&self, function: u32, value: ValueId) -> Option<&Type> {
        let function_index = function as usize;
        let definition = self.definitions.get(function_index)?.get(&value)?;
        let function = self.module.functions.get(function_index)?;
        let body = function.body.as_ref()?;
        match *definition {
            Definition::Parameter(index) => function.signature.parameters.get(index),
            Definition::BlockParameter(block, index) => body
                .blocks
                .get(block)?
                .parameters
                .get(index)
                .map(|value| &value.ty),
            Definition::Result(block, operation, index) => body
                .blocks
                .get(block)?
                .operations
                .get(operation)?
                .results
                .get(index)
                .map(|value| &value.ty),
        }
    }

    fn value_view(&self, function: u32, value: ValueId) -> Result<AuthoringValueV1> {
        let ty = self
            .value_type(function, value)
            .ok_or(AuthoringErrorV1::InvalidBoundary)?;
        Ok(AuthoringValueV1 {
            value: value.0,
            ty: bounded_debug(ty)?,
        })
    }

    fn operation_view(
        &self,
        coordinate: AuthoringOperationCoordinateV1,
    ) -> Result<AuthoringOperationV1> {
        let operation = self.operation(coordinate)?;
        let Function { id, .. } = &self.module.functions[coordinate.function as usize];
        if id.as_str().len() > MAX_TEXT_BYTES {
            return Err(AuthoringErrorV1::ResourceLimit);
        }
        let mut inputs = Vec::new();
        operation.kind.try_visit_operands(|value| {
            if inputs.len() == MAX_AUTHORING_REGION_VALUES_V1 {
                return Err(AuthoringErrorV1::ResourceLimit);
            }
            inputs.push(self.value_view(coordinate.function, value)?);
            Ok(())
        })?;
        if operation.results.len() > MAX_AUTHORING_REGION_VALUES_V1 {
            return Err(AuthoringErrorV1::ResourceLimit);
        }
        let results = operation
            .results
            .iter()
            .map(|value| self.value_view(coordinate.function, value.id))
            .collect::<Result<Vec<_>>>()?;
        let mut effects = Vec::new();
        operation.try_visit_local_memory_effects_v1(|effect| {
            if effects.len() == MAX_AUTHORING_REGION_VALUES_V1 {
                return Err(AuthoringErrorV1::ResourceLimit);
            }
            effects.push(bounded_debug(&effect)?);
            Ok(())
        })?;
        let mut spans = Vec::new();
        if let Some(index) = self.source_sites.get(&coordinate) {
            for span in self.source_map.sites()[*index].spans() {
                if spans.len() == MAX_AUTHORING_REGION_VALUES_V1 {
                    return Err(AuthoringErrorV1::ResourceLimit);
                }
                spans.push(self.source_span(*span)?);
            }
        }
        let mnemonic = match &operation.kind {
            OperationKind::InlineAssembly(assembly) => {
                if assembly.mnemonic.len() > MAX_TEXT_BYTES {
                    return Err(AuthoringErrorV1::ResourceLimit);
                }
                Some(assembly.mnemonic.clone())
            }
            _ => None,
        };
        Ok(AuthoringOperationV1 {
            coordinate,
            function_name: id.as_str().to_owned(),
            kind: operation_kind(&operation.kind),
            semantic_detail: semantic_detail(&operation.kind)?,
            mnemonic,
            inline_assembly_source: match &operation.kind {
                OperationKind::InlineAssembly(assembly) => Some(AuthoringAssemblySourceV1 {
                    frontend_unit: hex(&assembly.source.frontend_unit),
                    function: hex(&assembly.source.function),
                    contract: hex(&assembly.source.contract),
                    statement: hex(&assembly.source.statement),
                    authority: "inert_references_not_source_authentication",
                }),
                _ => None,
            },
            inputs,
            results,
            local_memory_effects: effects,
            complete_local_effect_summary: operation.has_complete_effect_summary(),
            convergence: "not_analyzed",
            traps: "not_analyzed",
            physical_resources: "unavailable_logical_canonical_stage",
            source_binding: if spans.is_empty() {
                "unavailable_no_source_span"
            } else {
                "bundle_content_bound_not_authenticated"
            },
            source_spans: spans,
            materialization: if self.draft_supported(coordinate, operation) {
                "diagnostic_rust_draft_available"
            } else {
                "unavailable_unsupported_operation_or_contract"
            },
        })
    }

    fn source_span(&self, span: DebugSourceMapSpanV1) -> Result<AuthoringSourceSpanV1> {
        let index = self
            .source_files
            .get(&span.file_identity())
            .ok_or(AuthoringErrorV1::InvalidSourceMap)?;
        let file = &self.source_map.files()[*index];
        if file.display_path().len() > MAX_TEXT_BYTES {
            return Err(AuthoringErrorV1::ResourceLimit);
        }
        Ok(AuthoringSourceSpanV1 {
            file_identity: hex(&span.file_identity()),
            display_path: file.display_path().to_owned(),
            byte_start: span.byte_start().to_string(),
            byte_end: span.byte_end().to_string(),
            line: span.line(),
            column: span.column(),
        })
    }

    fn draft_supported(
        &self,
        coordinate: AuthoringOperationCoordinateV1,
        operation: &Operation,
    ) -> bool {
        self.draft_instruction(coordinate, operation).is_some()
    }

    fn draft_instruction(
        &self,
        coordinate: AuthoringOperationCoordinateV1,
        operation: &Operation,
    ) -> Option<DraftInstructionV1> {
        if self.bundle.target() != "gfx942:xnack-" {
            return None;
        }
        // These three scalar bitwise operations have no overflow, carry,
        // floating-point, memory, convergence, or implicit-state contract to
        // reconstruct. The selected graph remains a Binary operation in all
        // observations: choosing a typed instruction happens only in the inert
        // generated Rust candidate, never by relabeling the retained program.
        if let OperationKind::Binary { op, lhs, rhs } = &operation.kind {
            let mnemonic = match op {
                BinaryOp::BitAnd => "v_and_b32",
                BinaryOp::BitOr => "v_or_b32",
                BinaryOp::BitXor => "v_xor_b32",
                _ => return None,
            };
            let [result] = operation.results.as_slice() else {
                return None;
            };
            if result.ty != Type::Scalar(ScalarType::U32)
                || [lhs, rhs].into_iter().any(|value| {
                    self.value_type(coordinate.function, *value)
                        != Some(&Type::Scalar(ScalarType::U32))
                })
            {
                return None;
            }
            return Some(DraftInstructionV1 {
                result: result.id,
                mnemonic,
                inputs: vec![*lhs, *rhs],
            });
        }
        let OperationKind::InlineAssembly(assembly) = &operation.kind else {
            return None;
        };
        // The source macro has no syntax for stronger caller-selected contracts.
        if assembly.options.len() != 1 || !assembly.options.contains(&AssemblyOption::NoMemory) {
            return None;
        }
        let instruction = validate_gfx942_inline_assembly_v1(operation, |value| {
            self.value_type(coordinate.function, value)
                .and_then(Type::as_scalar)
        })
        .ok()?;
        if instruction.scalar_type() != ScalarType::U32
            || !instruction.instruction().has_source_macro()
        {
            return None;
        }
        Some(DraftInstructionV1 {
            result: instruction.result(),
            mnemonic: instruction.instruction().mnemonic(),
            inputs: instruction.inputs().to_vec(),
        })
    }
}

struct DraftInstructionV1 {
    result: ValueId,
    mnemonic: &'static str,
    inputs: Vec<ValueId>,
}

fn insert_definition(
    values: &mut BTreeMap<ValueId, Definition>,
    value: ValueId,
    definition: Definition,
    count: &mut usize,
) -> Result<()> {
    if *count == MAX_AUTHORING_DEFINITIONS_V1 {
        return Err(AuthoringErrorV1::ResourceLimit);
    }
    *count += 1;
    if values.insert(value, definition).is_some() {
        return Err(AuthoringErrorV1::InvalidCanonicalInput);
    }
    Ok(())
}

fn ordinal(value: usize) -> Result<u32> {
    u32::try_from(value).map_err(|_| AuthoringErrorV1::ResourceLimit)
}

fn charge(scan: &mut usize, count: usize) -> Result<()> {
    *scan = scan
        .checked_add(count)
        .ok_or(AuthoringErrorV1::ResourceLimit)?;
    if *scan > MAX_AUTHORING_SCAN_ITEMS_V1 {
        return Err(AuthoringErrorV1::ResourceLimit);
    }
    Ok(())
}

fn visit_terminator(
    terminator: &Terminator,
    mut visit: impl FnMut(Option<ValueId>, usize) -> Result<()>,
) -> Result<()> {
    visit(None, 1)?;
    match terminator {
        Terminator::ConditionalBranch { condition, .. } => visit(Some(*condition), 1)?,
        Terminator::Switch { selector, .. } | Terminator::IntegerSwitch { selector, .. } => {
            visit(Some(*selector), 1)?
        }
        Terminator::Return { values } => {
            for value in values {
                visit(Some(*value), 1)?;
            }
        }
        _ => {}
    }
    terminator.try_visit_edges_v1(|_, arguments| {
        visit(None, 1)?;
        for value in arguments {
            visit(Some(*value), 1)?;
        }
        Ok(())
    })
}

fn semantic_detail(kind: &OperationKind) -> Result<Option<String>> {
    // These payloads are scalar enums only. Never format the complete nested
    // operation, source identities, operand vectors, or helper bodies.
    Ok(match kind {
        OperationKind::Constant(value) => Some(bounded_debug(value)?),
        OperationKind::Unary { op, .. } => Some(bounded_debug(op)?),
        OperationKind::Binary { op, .. } => Some(bounded_debug(op)?),
        OperationKind::Compare { predicate, .. } => Some(bounded_debug(predicate)?),
        OperationKind::Cast { kind, .. } => Some(bounded_debug(kind)?),
        _ => None,
    })
}

fn operation_kind(kind: &OperationKind) -> &'static str {
    match kind {
        OperationKind::Constant(_) => "constant",
        OperationKind::Intrinsic(_) => "intrinsic",
        OperationKind::MemoryIntrinsic(_) => "memory_intrinsic",
        OperationKind::Unary { .. } => "unary",
        OperationKind::Binary { .. } => "binary",
        OperationKind::Compare { .. } => "compare",
        OperationKind::Cast { .. } => "cast",
        OperationKind::Select { .. } => "select",
        OperationKind::Call { .. } => "call",
        OperationKind::Alloca { .. } => "alloca",
        OperationKind::SliceLength { .. } => "slice_length",
        OperationKind::SliceData { .. } => "slice_data",
        OperationKind::GetElementPointer { .. } => "get_element_pointer",
        OperationKind::Load { .. } => "load",
        OperationKind::GuardedLoad { .. } => "guarded_load",
        OperationKind::Store { .. } => "store",
        OperationKind::GuardedStore { .. } => "guarded_store",
        OperationKind::Barrier(_) => "barrier",
        OperationKind::Atomic(_) => "atomic",
        OperationKind::Fence(_) => "fence",
        OperationKind::WorkgroupBarrier(_) => "workgroup_barrier",
        OperationKind::WorkgroupMemory(_) => "workgroup_memory",
        OperationKind::Matrix(_) => "matrix",
        OperationKind::Gfx950LdsTranspose(_) => "gfx950_lds_transpose",
        OperationKind::Wave(_) => "wave",
        OperationKind::InlineAssembly(_) => "inline_assembly",
        _ => "unavailable_operation_outside_v11_profile",
    }
}

fn valid_helper_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    if name.len() > 64
        || !bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return false;
    }
    !matches!(
        name,
        "_" | "as"
            | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "gen"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
    )
}

fn hex(bytes: &[u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(64);
    for byte in bytes {
        value.push(DIGITS[(byte >> 4) as usize] as char);
        value.push(DIGITS[(byte & 15) as usize] as char);
    }
    value
}

struct BoundedText {
    text: String,
    limit: usize,
}
impl BoundedText {
    fn new(limit: usize) -> Self {
        Self {
            text: String::new(),
            limit,
        }
    }
}
impl fmt::Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if value.len() > self.limit.saturating_sub(self.text.len()) {
            return Err(fmt::Error);
        }
        self.text.push_str(value);
        Ok(())
    }
}
fn bounded_debug(value: &impl fmt::Debug) -> Result<String> {
    let mut text = BoundedText::new(MAX_TEXT_BYTES);
    write!(text, "{value:?}").map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    Ok(text.text)
}

fn bounded_report<T: Serialize>(report: T) -> Result<T> {
    struct Counter(usize);
    impl io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > MAX_AUTHORING_REPORT_BYTES_V1.saturating_sub(self.0) {
                return Err(io::Error::other("authoring report limit"));
            }
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Counter(0), &report).map_err(|_| AuthoringErrorV1::ResourceLimit)?;
    Ok(report)
}

#[cfg(test)]
#[path = "multilevel_authoring_v1_tests.rs"]
mod tests;
