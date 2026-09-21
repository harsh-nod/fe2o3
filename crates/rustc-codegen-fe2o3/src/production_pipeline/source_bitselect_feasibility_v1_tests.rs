//! Private first feasibility gate, not a replacement/planner admission API.
//! Captures typed HIR before consuming the sealed collected transaction, then
//! joins it to the unchanged normal semantic/KIR owners in that same callback.

use super::{CollectedRustStage, ProductionCompilation};
use crate::collector::source_census_v1::bitselect_feasibility::{
    CapturedBitselect, ScanMeter, capture,
};
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, Function, OperationKind, ScalarType, Type, ValueId, WorkgroupSize,
};
use fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssignmentV1, SemanticBinaryOpV1, SemanticFunctionDeclV1, SemanticFunctionIdV1,
    SemanticFunctionRoleV1, SemanticLocalIdV1, SemanticLocalRoleV1, SemanticOperandV1,
    SemanticRvalueKindV1, SemanticScalarTypeV1, SemanticStatementKindV1, SemanticTypeShapeV1,
};
use serde_json::{Value, json};

#[cfg(target_os = "linux")]
#[path = "source_bitselect_candidate_v1_tests.rs"]
mod candidate;

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_source_bitselect_feasibility(self) -> Result<Value, String> {
        let tcx = self.stage.tcx;
        let mut captured = capture(tcx, &self.stage.closure)?;
        // No special source importer, MIR rewrite, ranked bypass or owner
        // constructor: precisely the ordinary V6 extraction stage sequence.
        let admitted = self
            .import_semantic_mir()
            .map_err(|e| format!("source-boundary import: {e}"))?;
        let middle = admitted
            .construct_semantic_middle_end()
            .map_err(|e| format!("source-boundary middle: {e}"))?;
        let ssa = middle
            .construct_semantic_ssa()
            .map_err(|e| format!("source-boundary SSA: {e}"))?;
        let materialized = ssa
            .materialize_target_neutral()
            .map_err(|e| format!("source-boundary materialization: {e}"))?;
        let ranked = materialized
            .verify_general_kernel_checks()
            .map_err(|e| format!("source-boundary ranked: {e}"))?;
        let neutral = ranked
            .attach_target_neutral_checks()
            .map_err(|e| format!("source-boundary attachment: {e}"))?;
        captured.recheck_original(tcx)?;
        let observation = exact_join(&mut captured, &neutral.lowered)?.observation(&captured)?;
        // Keep authenticated source/target/descriptor bindings and the
        // semantic/KIR owner alive through the complete join; return only an
        // inert diagnostic. No graph or source-edit authority escapes.
        drop(neutral);
        Ok(observation)
    }
}

fn plain_operand(operand: &SemanticOperandV1) -> Result<SemanticLocalIdV1, String> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            Ok(place.local())
        }
        _ => Err("source-boundary semantic operand is not a plain local".into()),
    }
}

fn binary(
    assignment: &SemanticAssignmentV1,
) -> Result<
    (
        SemanticBinaryOpV1,
        SemanticLocalIdV1,
        SemanticLocalIdV1,
        SemanticLocalIdV1,
    ),
    String,
> {
    if !assignment.destination().projections().is_empty() {
        return Err("source-boundary semantic destination is projected".into());
    }
    let SemanticRvalueKindV1::Binary {
        operation,
        left,
        right,
    } = assignment.value().kind()
    else {
        return Err("source-boundary statement is not a binary assignment".into());
    };
    Ok((
        *operation,
        plain_operand(left)?,
        plain_operand(right)?,
        assignment.destination().local(),
    ))
}

fn unique<'a, T>(
    rows: &'a [T],
    meter: &mut ScanMeter,
    mut matches: impl FnMut(&T) -> bool,
    error: &'static str,
) -> Result<&'a T, String> {
    meter.rows(rows.len())?;
    let mut found = None;
    for row in rows {
        if matches(row) && found.replace(row).is_some() {
            return Err(format!("{error}: ambiguous"));
        }
    }
    found.ok_or_else(|| format!("{error}: missing"))
}

/// Borrows the live compiler owner, never reconstructed from a diagnostic.
struct JoinedBitselect<'a> {
    owner: &'a ProductionSemanticKirOwnerV1,
    root: SemanticFunctionIdV1,
    function: &'a SemanticFunctionDeclV1,
    kir: &'a Function,
    kir_block: &'a BasicBlock,
    locals: [SemanticLocalIdV1; 3],
    inputs: [ValueId; 3],
    results: [ValueId; 3],
    statements: [u32; 3],
    operations: [u32; 3],
}

fn exact_join<'a>(
    captured: &mut CapturedBitselect<'_>,
    owner: &'a ProductionSemanticKirOwnerV1,
) -> Result<JoinedBitselect<'a>, String> {
    // This existing replay keeps its production limits/ledger. The separate
    // ScanMeter below accounts only for the feasibility leaf's own scans.
    owner
        .verify_equivalence()
        .map_err(|e| format!("source-boundary equivalence: {e}"))?;
    let semantic = owner.semantic().semantic();
    let [root] = semantic.roots() else {
        return Err("source-boundary semantic root roster changed".into());
    };
    let selection = semantic
        .select_kernel_body_v1()
        .ok_or("source-boundary selected semantic body unavailable")?;
    if selection.root() != *root || selection.body() != *root {
        return Err("source-boundary root/body wrapper is outside profile".into());
    }
    let function = semantic
        .functions()
        .get(root.index() as usize)
        .ok_or("source-boundary semantic function index unavailable")?;
    let identities = captured.identities;
    if function.identity() != identities.function()
        || function.item_definition_identity() != identities.item_definition()
        || function.monomorphization_identity() != identities.monomorphization()
        || function.generic_type_arguments_identity() != identities.generic_type_arguments()
        || function.const_generic_arguments_identity() != identities.const_generic_arguments()
        || function.role() != SemanticFunctionRoleV1::KernelRoot
    {
        return Err("source-boundary sealed Instance/semantic identity mismatch".into());
    }
    let correspondence = owner.correspondence();
    let mapped = unique(
        correspondence.lowered_functions(),
        &mut captured.meter,
        |row| row.correspondence_owner() == *root && row.semantic_function() == *root,
        "source-boundary function correspondence",
    )?;
    let kir = unique(
        &owner.module().functions,
        &mut captured.meter,
        |row| &row.id == mapped.kernel_ir_function(),
        "source-boundary KIR function",
    )?;
    let body = kir.body.as_ref().ok_or("source-boundary KIR body absent")?;
    let [kernel] = owner.module().kernels.as_slice() else {
        return Err("source-boundary KIR kernel roster changed".into());
    };
    if kernel.entry != kir.id || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1)) {
        return Err("source-boundary requires exact 64x1x1 workgroup".into());
    }
    let mut locals = [SemanticLocalIdV1::from_index(0); 3];
    let mut inputs = [ValueId(0); 3];
    for (index, parameter) in captured.parameters.iter().enumerate() {
        captured.meter.rows(function.locals().len())?;
        let mut selected = None;
        for (local_index, local) in function.locals().iter().enumerate() {
            if local.role() == SemanticLocalRoleV1::Argument(parameter.ordinal) {
                if selected.replace((local_index, local)).is_some() {
                    return Err("source-boundary ambiguous semantic parameter".into());
                }
            }
        }
        let (local_index, local) = selected.ok_or("source-boundary semantic parameter absent")?;
        if semantic
            .types()
            .get(local.ty().index() as usize)
            .map(|ty| ty.shape())
            != Some(&SemanticTypeShapeV1::Scalar(
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ))
        {
            return Err("source-boundary semantic parameter is not u32".into());
        }
        let local_id = SemanticLocalIdV1::from_index(
            u32::try_from(local_index).map_err(|_| "source-boundary semantic local conversion")?,
        );
        locals[index] = local_id;
        let binding = unique(
            correspondence.parameter_bindings(),
            &mut captured.meter,
            |row| {
                row.correspondence_owner() == *root
                    && row.semantic_function() == *root
                    && row.semantic_local() == local_id
            },
            "source-boundary exact parameter/KIR correspondence",
        )?;
        inputs[index] = binding.kernel_ir_value();
        captured.meter.rows(body.parameters.len())?;
        let mut matches = 0;
        for (ordinal, value) in body.parameters.iter().enumerate() {
            if *value == inputs[index] {
                if kir.signature.parameters.get(ordinal) != Some(&Type::Scalar(ScalarType::U32)) {
                    return Err("source-boundary KIR parameter type mismatch".into());
                }
                matches += 1;
            }
        }
        if matches != 1 {
            return Err("source-boundary parameter binding is not one function input".into());
        }
    }
    let entry = function
        .blocks()
        .get(function.entry().index() as usize)
        .ok_or("source-boundary semantic entry absent")?;
    let mapped_entry = unique(
        correspondence.blocks(),
        &mut captured.meter,
        |row| {
            row.correspondence_owner() == *root
                && row.semantic_function() == *root
                && row.semantic_block() == function.entry()
        },
        "source-boundary entry-block correspondence",
    )?;
    let kir_block = unique(
        &body.blocks,
        &mut captured.meter,
        |block| block.id == mapped_entry.kernel_ir_block(),
        "source-boundary KIR entry",
    )?;
    let mut destinations = [SemanticLocalIdV1::from_index(0); 3];
    let mut results = [ValueId(0); 3];
    let mut statements = [0u32; 3];
    let mut operations = [0u32; 3];
    for index in 0..3 {
        captured.meter.rows(entry.statements().len())?;
        let mut selected = None;
        for (ordinal, statement) in entry.statements().iter().enumerate() {
            if statement.source() == captured.operators[index]
                && let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                && matches!(
                    assignment.value().kind(),
                    SemanticRvalueKindV1::Binary { .. }
                )
            {
                if selected.replace((ordinal, assignment)).is_some() {
                    return Err("source-boundary ambiguous operator/semantic span".into());
                }
            }
        }
        let (ordinal, assignment) =
            selected.ok_or("source-boundary exact HIR operator/semantic span absent")?;
        let expected_semantic = match index {
            0 => (SemanticBinaryOpV1::BitXor, locals[0], locals[1]),
            1 => (SemanticBinaryOpV1::BitAnd, destinations[0], locals[2]),
            _ => (SemanticBinaryOpV1::BitXor, locals[1], destinations[1]),
        };
        let (op, left, right, destination) = binary(&assignment)?;
        if (op, left, right) != expected_semantic {
            return Err("source-boundary semantic bitselect graph mismatch".into());
        }
        destinations[index] = destination;
        statements[index] =
            u32::try_from(ordinal).map_err(|_| "source-boundary statement ordinal")?;
        let span = unique(
            correspondence.statement_operation_spans(),
            &mut captured.meter,
            |row| {
                row.correspondence_owner() == *root
                    && row.semantic_function() == *root
                    && row.semantic_block() == function.entry()
                    && row.statement_ordinal() == statements[index]
            },
            "source-boundary exact statement/KIR correspondence",
        )?;
        if span.kernel_ir_block() != kir_block.id || span.operation_count() != 1 {
            return Err("source-boundary operator must emit exactly one KIR operation".into());
        }
        operations[index] = span.first_operation_ordinal();
        let operation = kir_block
            .operations
            .get(operations[index] as usize)
            .ok_or("source-boundary KIR operation absent")?;
        let expected_kir = match index {
            0 => (BinaryOp::BitXor, inputs[0], inputs[1]),
            1 => (BinaryOp::BitAnd, results[0], inputs[2]),
            _ => (BinaryOp::BitXor, inputs[1], results[1]),
        };
        let OperationKind::Binary { op, lhs, rhs } = operation.kind else {
            return Err("source-boundary mapped KIR operation is not binary".into());
        };
        let [result] = operation.results.as_slice() else {
            return Err("source-boundary mapped KIR result arity".into());
        };
        if (op, lhs, rhs) != expected_kir || result.ty != Type::Scalar(ScalarType::U32) {
            return Err("source-boundary exact KIR bitselect graph mismatch".into());
        }
        results[index] = result.id;
    }
    if operations[0].checked_add(1) != Some(operations[1])
        || operations[1].checked_add(1) != Some(operations[2])
        || !(statements[0] < statements[1] && statements[1] < statements[2])
    {
        return Err("source-boundary operators are not one ordered KIR boundary".into());
    }
    let identities = [
        inputs[0], inputs[1], inputs[2], results[0], results[1], results[2],
    ];
    for (index, value) in identities.iter().enumerate() {
        captured.meter.scan(identities.len() - index - 1)?;
        if identities[index + 1..].contains(value) {
            return Err("source-boundary live-ins/results are not six distinct SSA values".into());
        }
    }
    Ok(JoinedBitselect {
        owner,
        root: *root,
        function,
        kir,
        kir_block,
        locals,
        inputs,
        results,
        statements,
        operations,
    })
}

impl JoinedBitselect<'_> {
    fn observation(&self, captured: &CapturedBitselect<'_>) -> Result<Value, String> {
        let Self {
            owner,
            root,
            function,
            kir,
            kir_block,
            locals,
            inputs,
            results,
            statements,
            operations,
        } = self;
        let semantic = owner.semantic().semantic();
        let parameters = captured
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                json!({
                    "name": parameter.name, "hir_binding": format!("{:?}", parameter.hir_id),
                    "source_parameter_ordinal": parameter.ordinal, "source_range": parameter.range,
                    "semantic_local": locals[index].index(), "kernel_ir_parameter": inputs[index].0,
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "stage": "actual_typed_hir_semantic_kir_join",
            "function_identity": captured.identities.function().as_bytes(),
            "definition_identity": captured.identities.item_definition().as_bytes(),
            "monomorphization_identity": captured.identities.monomorphization().as_bytes(),
            "semantic_sha256": semantic.semantic_sha256().as_bytes(),
            "kernel_ir_sha256": owner.canonical_kernel_ir_identity().digest(),
            "kernel_ir_version": format!("{:?}", owner.canonical_kernel_ir_identity().version()),
            "source_file_identity": captured.operators[2].expansion()
                .ok_or("source-boundary original provenance absent")?.file().as_bytes(),
            "original_sha256": captured.original_sha256,
            "initializer": captured.initializer,
            "operators": captured.operator_ranges,
            "parameters": parameters,
            "result_name": captured.result_name,
            "result_hir_binding": format!("{:?}", captured.result_hir_id),
            "semantic_function": root.index(), "semantic_block": function.entry().index(),
            "semantic_statements": statements, "kernel_ir_function": kir.id.as_str(),
            "kernel_ir_block": kir_block.id.0, "kernel_ir_operations": operations,
            "kernel_ir_results": results.map(|value| value.0),
            "normal_stages_and_equivalence_replayed": true,
            "work_accounting": captured.meter,
            "accounting_scope": "own cumulative scans and charged source/name payload bounds only; compiler queries/replay and diagnostic JSON excluded",
            "source_unchanged": true, "candidate_written": false,
            "replacement_admitted": false, "reversible_rust_claimed": false,
            "grants_artifact_or_launch_authority": false,
        }))
    }
}

#[test]
fn source_boundary_join_lookup_refuses_missing_duplicate_and_over_budget_rows() {
    let mut meter = ScanMeter::default();
    assert_eq!(
        *unique(&[7, 9], &mut meter, |row| *row == 9, "join").unwrap(),
        9
    );
    assert_eq!(
        unique(&[7, 9], &mut meter, |row| *row == 1, "join").unwrap_err(),
        "join: missing"
    );
    assert_eq!(
        unique(&[9, 9], &mut meter, |row| *row == 9, "join").unwrap_err(),
        "join: ambiguous"
    );
    let rows = [9; 4097];
    assert_eq!(
        unique(&rows, &mut meter, |_| true, "join").unwrap_err(),
        "source-boundary row limit"
    );
}
