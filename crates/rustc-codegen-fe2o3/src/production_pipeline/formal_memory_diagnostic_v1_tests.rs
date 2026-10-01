//! Same-run observations before the normal formal-memory gate consumes its owner.
//! This test-only report is not a compiler receipt or an optimized-graph witness.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticSourceOriginV1, SemanticSourceProvenanceV1};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;

#[path = "../production_source_census_coordinates_v1.rs"]
mod coordinates;

const ROW_LIMIT: usize = 65_536;
const BYTE_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Owner {
    semantic_root: u32,
    semantic_function: u32,
    role: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicalOperation {
    function_ordinal: usize,
    function: String,
    block_ordinal: usize,
    block_id: u32,
    operation_ordinal: usize,
    owners: Vec<Owner>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn fresh_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > BYTE_LIMIT {
        return Err("diagnostic byte bound exceeded".into());
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn source_origin(origin: Option<SemanticSourceOriginV1>) -> Value {
    origin.map_or(Value::Null, |origin| {
        json!({
            "fileIdentity": hex(origin.file().as_bytes()),
            "byteRange": origin.byte_range(),
            "startCoordinate": origin.start_coordinate(),
            "endCoordinate": origin.end_coordinate(),
        })
    })
}

fn source_provenance(source: SemanticSourceProvenanceV1) -> Value {
    json!({
        "expansion": source_origin(source.expansion()),
        "callSite": source_origin(source.call_site()),
        "v1ProjectionUses": "callSite",
    })
}

fn span_json(span: fe2o3_kernel_ir::DebugSourceMapSpanV1) -> Value {
    json!({
        "fileIdentity": hex(&span.file_identity()),
        "byteStart": span.byte_start(), "byteEnd": span.byte_end(),
        "line": span.line(), "column": span.column(),
    })
}

fn physical_operations(
    owner: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
) -> Result<Vec<PhysicalOperation>, String> {
    let mut rows = Vec::new();
    let mut charged = 0_usize;
    for (function_ordinal, function) in owner.module().functions.iter().enumerate() {
        let Some(body) = &function.body else {
            continue;
        };
        let owners: Vec<_> = owner
            .correspondence()
            .lowered_functions()
            .iter()
            .filter(|row| row.kernel_ir_function() == &function.id)
            .map(|row| Owner {
                semantic_root: row.correspondence_owner().index(),
                semantic_function: row.semantic_function().index(),
                role: format!("{:?}", row.role()),
            })
            .collect();
        if owners.len() > ROW_LIMIT {
            return Err("diagnostic owner row bound exceeded".into());
        }
        for (block_ordinal, block) in body.blocks.iter().enumerate() {
            for operation_ordinal in 0..block.operations.len() {
                charged = charged.saturating_add(owners.len().saturating_add(1));
                if charged > ROW_LIMIT {
                    return Err("diagnostic operation row bound exceeded".into());
                }
                rows.push(PhysicalOperation {
                    function_ordinal,
                    function: function.id.as_str().to_owned(),
                    block_ordinal,
                    block_id: block.id.0,
                    operation_ordinal,
                    owners: owners.clone(),
                });
            }
        }
    }
    Ok(rows)
}

fn kernel_roster(
    owner: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    layout_is_exact: bool,
) -> Value {
    Value::Array(owner.module().kernels.iter().enumerate().map(|(ordinal, kernel)| {
        let functions: Vec<_> = owner.module().functions.iter().enumerate()
            .filter(|(_, function)| function.id == kernel.entry)
            .map(|(ordinal, _)| ordinal).collect();
        let roots: Vec<_> = owner.correspondence().lowered_functions().iter()
            .filter(|row| row.kernel_ir_function() == &kernel.entry
                && row.role() == fe2o3_lower_mir_kernel::SemanticKirFunctionRoleV1::KernelEntry)
            .map(|row| Owner {
                semantic_root: row.correspondence_owner().index(),
                semantic_function: row.semantic_function().index(),
                role: format!("{:?}", row.role()),
            }).collect();
        let unique = match (functions.as_slice(), roots.as_slice()) {
            ([function], [root]) if layout_is_exact => Some(json!({
                "functionOrdinal": function, "rootCorrespondence": root,
            })),
            _ => None,
        };
        json!({
            "kernelOrdinal": ordinal, "kernelId": kernel.id.as_str(),
            "entryFunctionId": kernel.entry.as_str(),
            "entryFunctionOrdinalCandidates": functions,
            "rootCorrespondenceCandidates": roots,
            "ownerStatus": if unique.is_some() { "unique-live-owner" } else { "unavailable-or-ambiguous" },
            "uniqueEntry": unique,
        })
    }).collect())
}

fn raw_correspondence(
    owner: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
) -> Result<Value, String> {
    let correspondence = owner.correspondence();
    let count = correspondence
        .statement_operation_spans()
        .len()
        .checked_add(correspondence.terminator_operation_spans().len())
        .and_then(|count| count.checked_add(correspondence.synthetic_operation_spans().len()))
        .ok_or("diagnostic correspondence row count overflow")?;
    if count > ROW_LIMIT {
        return Err("diagnostic correspondence row bound exceeded".into());
    }
    let mut rows = Vec::with_capacity(count);
    for row in correspondence.statement_operation_spans() {
        let source = owner.semantic().resolve_statement(
            row.semantic_function(),
            row.semantic_block(),
            row.statement_ordinal(),
        );
        rows.push(json!({
            "kind": "statement", "semanticRoot": row.correspondence_owner().index(),
            "semanticFunction": row.semantic_function().index(),
            "semanticBlock": row.semantic_block().index(),
            "statementOrdinal": row.statement_ordinal(),
            "kirBlockId": row.kernel_ir_block().0,
            "firstOperationOrdinal": row.first_operation_ordinal(),
            "operationCount": row.operation_count(),
            "resolved": source.is_some(),
            "source": source.map(|statement| source_provenance(statement.source())),
        }));
    }
    for row in correspondence.terminator_operation_spans() {
        let source = owner
            .semantic()
            .resolve_terminator(row.semantic_function(), row.semantic_block());
        rows.push(json!({
            "kind": "terminator", "semanticRoot": row.correspondence_owner().index(),
            "semanticFunction": row.semantic_function().index(),
            "semanticBlock": row.semantic_block().index(),
            "kirBlockId": row.kernel_ir_block().0,
            "firstOperationOrdinal": row.first_operation_ordinal(),
            "operationCount": row.operation_count(),
            "resolved": source.is_some(),
            "source": source.map(|terminator| source_provenance(terminator.source())),
        }));
    }
    for row in correspondence.synthetic_operation_spans() {
        rows.push(json!({
            "kind": "synthetic", "semanticRoot": row.correspondence_owner().index(),
            "semanticFunction": row.semantic_function().index(),
            "kirBlockId": row.kernel_ir_block().0,
            "firstOperationOrdinal": row.first_operation_ordinal(),
            "operationCount": row.operation_count(), "rule": format!("{:?}", row.rule()),
            "source": null,
        }));
    }
    Ok(Value::Array(rows))
}

fn projection_json(
    owner: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    files: &[fe2o3_kernel_ir::DebugSourceMapFileV1],
) -> Value {
    match compiler_debug_source_projection_v1(ExactDebugSourceOwnerV1::Normal(owner), files) {
        Ok(projection) => json!({
            "status": "available",
            "sites": projection.sites.iter().map(|row| {
                let site = row.site();
                json!({
                    "functionOrdinal": site.function_ordinal(),
                    "blockOrdinal": site.block_ordinal(),
                    "operationOrdinal": site.operation_ordinal(),
                    "spans": row.spans().iter().copied().map(span_json).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
            "eliminated": projection.eliminated.into_iter().map(span_json).collect::<Vec<_>>(),
        }),
        Err(error) => json!({"status": "unavailable", "reason": format!("{error:?}")}),
    }
}

fn live_source_files(
    tcx: rustc_middle::ty::TyCtxt<'_>,
    owner: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    directory: &Path,
) -> Result<Value, String> {
    use rustc_span::{BytePos, FileName, Span};
    let statements = owner
        .correspondence()
        .statement_operation_spans()
        .iter()
        .filter_map(|row| {
            owner
                .semantic()
                .resolve_statement(
                    row.semantic_function(),
                    row.semantic_block(),
                    row.statement_ordinal(),
                )
                .map(|statement| statement.source())
        });
    let terminators = owner
        .correspondence()
        .terminator_operation_spans()
        .iter()
        .filter_map(|row| {
            owner
                .semantic()
                .resolve_terminator(row.semantic_function(), row.semantic_block())
                .map(|terminator| terminator.source())
        });
    let mut origins = BTreeMap::new();
    for origin in statements
        .chain(terminators)
        .flat_map(|source| [source.expansion(), source.call_site()])
        .flatten()
    {
        let (start, end) = origin.byte_range();
        origins.insert((*origin.file().as_bytes(), start, end), origin);
        if origins.len() > ROW_LIMIT {
            return Err("diagnostic source-origin row bound exceeded".into());
        }
    }
    let mut pending: BTreeSet<_> = origins.keys().map(|(identity, _, _)| *identity).collect();
    let files = tcx.sess.source_map().files();
    if files.len() > 4096 {
        return Err("diagnostic live source-file roster bound exceeded".into());
    }
    let files = files.iter().cloned().collect::<Vec<_>>();
    let source_directory = directory.join("live-source");
    std::fs::create_dir(&source_directory).map_err(|error| error.to_string())?;
    let mut rows = Vec::new();
    let mut charged_bytes = 0_usize;
    for file in files {
        let span = Span::with_root_ctxt(file.start_pos, file.end_position());
        let Ok((_, Some(metadata))) =
            crate::rustc_semantic_adapter_v1::canonical_source_provenance_and_debug_files_v1(
                tcx, span, 0,
            )
        else {
            continue;
        };
        if !pending.remove(&metadata.identity()) {
            continue;
        }
        let identity = hex(&metadata.identity());
        let coordinates = origins.iter().filter(|((id, _, _), _)| *id == metadata.identity()).map(|((_, start, end), _)| {
            let mapped = u32::try_from(*start).ok().zip(u32::try_from(*end).ok())
                .and_then(|(start, end)| file.start_pos.0.checked_add(start).zip(file.start_pos.0.checked_add(end)))
                .ok_or("source origin overflows rustc byte positions")
                .and_then(|(start, end)| coordinates::source_coordinates_v1(
                    &file, Span::with_root_ctxt(BytePos(start), BytePos(end)),
                ));
            json!({
                "normalizedRange": [start, end],
                "originalCoordinates": match mapped { Ok(value) => json!(value), Err(error) => json!({"unavailable": error}) },
            })
        }).collect::<Vec<_>>();
        let normalized = match file.src.as_deref() {
            Some(source) if source.len() == metadata.byte_len() as usize => {
                charged_bytes = charged_bytes.saturating_add(source.len());
                if charged_bytes > BYTE_LIMIT {
                    return Err("diagnostic source byte bound exceeded".into());
                }
                let name = format!("{identity}.normalized.rs");
                match fresh_bytes(&source_directory.join(&name), source.as_bytes()) {
                    Ok(()) => {
                        json!({"path": name, "sha256": hex(&Sha256::digest(source.as_bytes())), "byteLength": source.len()})
                    }
                    Err(error) => json!({"unavailable": error}),
                }
            }
            _ => {
                json!({"unavailable": "live normalized source text is absent or has inconsistent length"})
            }
        };
        let physical = (|| -> Result<Value, String> {
            let FileName::Real(name) = &file.name else {
                return Err("source file has no physical name".into());
            };
            let path = name.local_path().ok_or("source file has no local path")?;
            let mut opened = std::fs::File::open(path).map_err(|error| error.to_string())?;
            let metadata = opened.metadata().map_err(|error| error.to_string())?;
            if !metadata.is_file() || metadata.len() != u64::from(file.unnormalized_source_len) {
                return Err("physical source is not a regular file of the captured length".into());
            }
            let remaining = BYTE_LIMIT.saturating_sub(charged_bytes);
            let mut bytes = Vec::new();
            (&mut opened)
                .take(remaining as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            if bytes.len() > remaining {
                return Err("diagnostic source byte bound exceeded".into());
            }
            let text = std::str::from_utf8(&bytes).map_err(|error| error.to_string())?;
            if !file.src_hash.matches(text) {
                return Err("physical bytes differ from the live rustc source hash".into());
            }
            charged_bytes += bytes.len();
            let name = format!("{identity}.original.rs");
            fresh_bytes(&source_directory.join(&name), &bytes)?;
            Ok(
                json!({"path": name, "sha256": hex(&Sha256::digest(&bytes)), "byteLength": bytes.len(), "localPath": path}),
            )
        })();
        rows.push(json!({
            "identity": identity, "displayPath": metadata.display_path(),
            "normalized": normalized,
            "physical": match physical { Ok(value) => value, Err(error) => json!({"unavailable": error}) },
            "coordinates": coordinates,
        }));
    }
    Ok(json!({
        "files": rows,
        "unavailableIdentities": pending.iter().map(|identity| hex(identity)).collect::<Vec<_>>(),
        "originOffsetSpace": "rustc normalized UTF-8; original offsets are separately mapped",
    }))
}

fn candidates(
    rows: &[PhysicalOperation],
    location: fe2o3_kernel_ir::FunctionOperationLocation,
) -> Vec<&PhysicalOperation> {
    rows.iter()
        .filter(|row| {
            row.block_id == location.block.0 && row.operation_ordinal == location.operation_index
        })
        .collect()
}

fn conflict_json(
    rows: &[PhysicalOperation],
    layout_is_exact: bool,
    left: fe2o3_kernel_ir::FunctionOperationLocation,
    right: fe2o3_kernel_ir::FunctionOperationLocation,
    allocation_parameter: u32,
) -> Value {
    let left_candidates = candidates(rows, left);
    let right_candidates = candidates(rows, right);
    let unique = match (left_candidates.as_slice(), right_candidates.as_slice()) {
        ([left], [right])
            if layout_is_exact
                && left.function_ordinal == right.function_ordinal
                && left.owners.len() == 1
                && left.owners == right.owners =>
        {
            Some(json!({
                "functionOrdinal": left.function_ordinal,
                "function": left.function,
                "owner": left.owners[0],
            }))
        }
        _ => None,
    };
    json!({
        "left": {"blockId": left.block.0, "operationOrdinal": left.operation_index},
        "right": {"blockId": right.block.0, "operationOrdinal": right.operation_index},
        "allocationParameter": allocation_parameter,
        "locationScope": "function-local; formal error omits function identity",
        "ownershipStatus": if unique.is_some() { "unique-live-owner" } else { "unavailable-or-ambiguous" },
        "uniqueOwner": unique,
        "leftCandidates": left_candidates,
        "rightCandidates": right_candidates,
    })
}

fn execution_witness_json(
    witness: fe2o3_lower_mir_kernel::ProductionFormalMemoryExecutionWitnessV1,
) -> Value {
    let path = match witness.path() {
        fe2o3_kernel_ir::FormalGuardedPathV1::ExplicitPredicate => {
            json!({"kind": "explicitPredicate"})
        }
        fe2o3_kernel_ir::FormalGuardedPathV1::TrueEdge {
            source,
            ordinal,
            target,
        } => {
            json!({"kind": "trueEdge", "sourceBlock": source.0,
                "successorOrdinal": ordinal, "targetBlock": target.0})
        }
    };
    json!({"invocation": witness.invocation(), "index": witness.index().0,
        "threshold": witness.threshold().0, "predicate": witness.predicate().0,
        "path": path})
}

fn admitted_execution_json(
    owner: &fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
    operations: &[PhysicalOperation],
    layout_is_exact: bool,
) -> Value {
    let mut rows = Vec::new();
    let mut charged = 0_usize;
    let operation_work = operations.iter().fold(0_usize, |work, operation| {
        work.saturating_add(1)
            .saturating_add(operation.owners.len())
    });
    for kernel in owner.kernels() {
        let obligations = kernel.obligations();
        let conflicts = obligations.inter_invocation_conflicts();
        let discharges = kernel.execution_discharges();
        charged = charged
            .saturating_add(1)
            .saturating_add(
                conflicts
                    .len()
                    .saturating_mul(operation_work.saturating_mul(2)),
            )
            .saturating_add(discharges.len());
        if charged > ROW_LIMIT {
            return json!({"unavailable": "diagnostic execution-discharge row bound exceeded"});
        }
        let location = |value: fe2o3_kernel_ir::FunctionOperationLocation| json!({"blockId": value.block.0, "operationOrdinal": value.operation_index});
        rows.push(json!({
            "kernel": obligations.kernel().as_str(), "entry": obligations.entry().as_str(),
            "rawConflicts": conflicts.iter().map(|conflict| conflict_json(
                operations, layout_is_exact, conflict.left(), conflict.right(),
                conflict.allocation().parameter_index(),
            )).collect::<Vec<_>>(),
            "executionDischarges": discharges.iter().map(|discharge| json!({
                "conflictOrdinal": discharge.conflict_ordinal(),
                "left": location(discharge.left()), "right": location(discharge.right()),
                "allocationParameter": discharge.allocation_parameter(),
                "leftWitness": execution_witness_json(discharge.left_witness()),
                "rightWitness": execution_witness_json(discharge.right_witness()),
            })).collect::<Vec<_>>(),
            "rankedDischargedReasonCount": kernel.ranked_discharged_reasons().len(),
            "compilerDischargedReasonCount": kernel.compiler_discharged_reasons().len(),
        }));
    }
    json!({"kernels": rows, "observationGrantsAuthority": false})
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_pre_formal_memory_v1(
        self,
        directory: &Path,
        target_budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Value, ProductionPipelineError> {
        let tcx = self.stage.tcx;
        let ranked = self
            .import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .materialize_target_neutral()?
            .verify_general_kernel_checks()?;
        if ranked.has_direct_conditional_roots_v2() {
            return Err(ranked.conditional_production_finalizer_refusal_v5(target_budget));
        }
        let neutral = ranked.attach_target_neutral_checks()?;
        let owner = &neutral.lowered;
        let identity = owner.canonical_kernel_ir_identity();
        let bytes = owner.canonical_kernel_ir_bytes();
        let kir_capture = fresh_bytes(&directory.join("pre-formal-owner.kir"), bytes);
        let layouts =
            exact_debug_map_functions_from_owner_v1(ExactDebugSourceOwnerV1::Normal(owner))
                .map(|_| ());
        let operations = physical_operations(owner);
        let correspondence = raw_correspondence(owner);
        let bounded_roster = operations.is_ok() && correspondence.is_ok();
        let mut report = json!({
            "schema": "fe2o3-test-pre-formal-diagnostic-v1",
            "authority": "none; test observation only",
            "phase": "same-run target-neutral owner immediately before formal admission",
            "optimized": {"status": "unavailable_before_formal_admission", "graph": null},
            "canonicalOwner": {
                "version": format!("{:?}", identity.version()),
                "identity": hex(identity.digest()), "canonicalLength": identity.canonical_length(),
                "bytesSha256": hex(&Sha256::digest(bytes)),
                "capture": match kir_capture { Ok(()) => json!({"path": "pre-formal-owner.kir"}), Err(error) => json!({"error": error}) },
            },
            "kernels": owner.module().kernels.iter().map(|kernel| kernel.id.as_str()).collect::<Vec<_>>(),
            "kernelEntries": kernel_roster(owner, layouts.is_ok()),
            "loweredFunctions": owner.correspondence().lowered_functions().iter().map(|row| json!({
                "semanticRoot": row.correspondence_owner().index(),
                "semanticFunction": row.semantic_function().index(),
                "kernelIrFunction": row.kernel_ir_function().as_str(),
                "role": format!("{:?}", row.role()),
            })).collect::<Vec<_>>(),
            "functionLayouts": match &layouts { Ok(_) => json!({"status": "exact"}), Err(error) => json!({"status": "unavailable", "reason": format!("{error:?}")}) },
            "operations": match &operations { Ok(rows) => json!(rows), Err(error) => json!({"unavailable": error}) },
            "correspondence": match correspondence { Ok(rows) => rows, Err(error) => json!({"unavailable": error}) },
            "sourceFiles": neutral.bindings.debug_source_files.iter().map(|file| json!({
                "identity": hex(&file.identity()), "byteLength": file.byte_len(),
                "displayPath": file.display_path(),
            })).collect::<Vec<_>>(),
            "sourceCaptureGap": neutral.bindings.debug_capture_gap.map(|gap| format!("{gap:?}")),
            "callSiteProjection": if bounded_roster {
                projection_json(owner, &neutral.bindings.debug_source_files)
            } else {
                json!({"status": "unavailable", "reason": "diagnostic roster bound exceeded"})
            },
            "liveSource": if bounded_roster {
                match live_source_files(tcx, owner, directory) {
                    Ok(value) => value, Err(error) => json!({"unavailable": error}),
                }
            } else {
                json!({"unavailable": "diagnostic roster bound exceeded"})
            },
        });
        // No diagnostic limitation turns a genuine refusal into a different
        // compiler result, and no second owner or optimizer is constructed.
        report["formalAdmission"] = match neutral.admit_formal_memory() {
            Ok(admitted) => json!({
                "status": "admitted", "nativeLowering": "not attempted",
                "execution": admitted_execution_json(
                    &admitted.admitted, operations.as_deref().unwrap_or(&[]), layouts.is_ok(),
                ),
            }),
            Err(error) => {
                let conflicts = match &error {
                    ProductionPipelineError::FormalMemoryAdmission(
                        fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::InterInvocationConflicts { conflicts },
                    ) => conflicts.iter().map(|conflict| conflict_json(
                        operations.as_deref().unwrap_or(&[]), layouts.is_ok(),
                        conflict.left(), conflict.right(), conflict.allocation().parameter_index(),
                    )).collect::<Vec<_>>(),
                    _ => Vec::new(),
                };
                json!({
                    "status": "refused", "error": format!("{error:?}"),
                    "display": error.to_string(), "conflicts": conflicts,
                })
            }
        };
        if report["formalAdmission"]["status"] == "admitted" {
            report["optimized"]["status"] = json!("unavailable_not_executed_by_diagnostic");
        }
        Ok(report)
    }
}

fn test_row(function: usize, root: u32, block: u32) -> PhysicalOperation {
    PhysicalOperation {
        function_ordinal: function,
        function: format!("function-{function}"),
        block_ordinal: 0,
        block_id: block,
        operation_ordinal: 0,
        owners: vec![Owner {
            semantic_root: root,
            semantic_function: root,
            role: "KernelEntry".into(),
        }],
    }
}

fn location(block: u32) -> fe2o3_kernel_ir::FunctionOperationLocation {
    fe2o3_kernel_ir::FunctionOperationLocation {
        block: fe2o3_kernel_ir::BlockId(block),
        operation_index: 0,
    }
}

#[test]
fn diagnostic_conflict_requires_one_actual_function_and_root() {
    let rows = [test_row(0, 4, 21), test_row(0, 4, 22)];
    let report = conflict_json(&rows, true, location(21), location(22), 1);
    assert_eq!(report["ownershipStatus"], "unique-live-owner");
    assert_eq!(report["uniqueOwner"]["owner"]["semanticRoot"], 4);
    assert_eq!(report["allocationParameter"], 1);
    assert_eq!(report["left"]["blockId"], 21);
}

#[test]
fn diagnostic_conflict_never_guesses_ambiguous_cross_owner_or_missing_locations() {
    for (rows, exact) in [
        (vec![test_row(0, 4, 21), test_row(1, 5, 21)], true),
        (vec![test_row(0, 4, 21), test_row(1, 4, 22)], true),
        (vec![test_row(0, 4, 21), test_row(0, 5, 22)], true),
        (vec![test_row(0, 4, 21)], true),
        (vec![test_row(0, 4, 21), test_row(0, 4, 22)], false),
    ] {
        let report = conflict_json(&rows, exact, location(21), location(22), 1);
        assert_eq!(report["ownershipStatus"], "unavailable-or-ambiguous");
        assert!(report["uniqueOwner"].is_null());
    }
    let mut shared = test_row(0, 4, 21);
    shared.owners.push(test_row(0, 5, 21).owners.remove(0));
    let report = conflict_json(&[shared], true, location(21), location(21), 1);
    assert!(report["uniqueOwner"].is_null());
}

#[test]
fn diagnostic_keeps_expansion_and_call_site_independent() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticSourceFileIdentityV1;
    let origin = SemanticSourceOriginV1::new(
        SemanticSourceFileIdentityV1::from_sha256([7; 32]),
        2,
        8,
        1,
        2,
        1,
        8,
    )
    .unwrap();
    let unavailable = source_provenance(SemanticSourceProvenanceV1::unavailable());
    assert!(unavailable["expansion"].is_null());
    assert!(unavailable["callSite"].is_null());
    let expansion = source_provenance(SemanticSourceProvenanceV1::new(Some(origin), None));
    assert!(!expansion["expansion"].is_null());
    assert!(expansion["callSite"].is_null());
    let call_site = source_provenance(SemanticSourceProvenanceV1::new(None, Some(origin)));
    assert!(call_site["expansion"].is_null());
    assert_eq!(call_site["callSite"]["byteRange"], json!([2, 8]));
}

#[test]
fn diagnostic_canonical_handoff_never_clobbers_existing_bytes() {
    let scratch = crate::test_temp_dir::TestTempDir::create("formal-owner-no-clobber");
    let path = scratch.path().join("owner.kir");
    fresh_bytes(&path, b"first").unwrap();
    assert!(fresh_bytes(&path, b"second").is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"first");
}
