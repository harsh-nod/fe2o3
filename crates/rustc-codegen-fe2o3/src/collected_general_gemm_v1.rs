//! Authenticated MIR adapter for the safe general tiled-GEMM surfaces.
//!
//! This first adapter recognizes only exact diagnostic-item `DefId`s from the
//! reviewed standalone companion crate. It derives call multiplicity and
//! ordering from the collected kernel root's optimized MIR CFG. The concrete
//! KIR plan below is deliberately a non-authoritative witness: runtime ABI
//! values are not yet bound to checked planner fields. Consequently even a
//! verified witness stops before lowering or artifact publication.

use std::collections::{BTreeSet, VecDeque};
use std::fmt;

use fe2o3_kernel_ir::{
    GeneralGemmBarrierRoleV1, GeneralGemmKirDiagnosticV1, GeneralGemmKirV1,
    GeneralGemmPhaseEventV1, GeneralGemmPlanFieldsV1, GeneralGemmPlanSnapshotV1,
    GeneralGemmStoreCardinalityV1, verify_general_gemm_kir_v1,
};
use rustc_middle::mir::{BasicBlock, Body, Operand, START_BLOCK, TerminatorKind};
use rustc_middle::ty::{TyCtxt, TyKind};

use crate::AmdGpuTarget;
use crate::collector::CollectionResult;
use crate::trusted_device_items::{
    self, TrustedDeviceItem, TrustedGeneralGemmOperationV1, TrustedGeneralGemmSurfaceV1,
};

const EXACT_GENERAL_GEMM_TARGET_V1: &str = "gfx942:xnack-";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GeneralGemmMirImportV1 {
    VerifiedWitness {
        surface: TrustedGeneralGemmSurfaceV1,
        identity: [u8; 32],
    },
    Rejected(GeneralGemmKirDiagnosticV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmMirImportErrorV1 {
    message: String,
}

impl GeneralGemmMirImportErrorV1 {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for GeneralGemmMirImportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GeneralGemmMirImportErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeneralGemmCallV1 {
    surface: TrustedGeneralGemmSurfaceV1,
    operation: TrustedGeneralGemmOperationV1,
    block: BasicBlock,
    return_target: BasicBlock,
}

pub(crate) fn try_import_general_gemm_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    target: &AmdGpuTarget,
) -> Result<Option<GeneralGemmMirImportV1>, GeneralGemmMirImportErrorV1> {
    let mut root = None;
    let mut root_calls = Vec::new();
    let mut saw_general_gemm = false;

    for function in &collection.functions {
        let body = tcx.instance_mir(function.instance.def);
        let calls = general_gemm_calls(tcx, body)?;
        if calls.is_empty() {
            continue;
        }
        saw_general_gemm = true;
        if !function.is_kernel_entry() {
            return Err(GeneralGemmMirImportErrorV1::new(
                "general GEMM terminal remained in a collected helper; the V1 adapter requires direct calls in exactly one kernel root",
            ));
        }
        if root.replace(body).is_some() {
            return Err(GeneralGemmMirImportErrorV1::new(
                "general GEMM terminals occur in more than one kernel root",
            ));
        }
        root_calls = calls;
    }

    if !saw_general_gemm {
        return Ok(None);
    }
    if target.as_str() != EXACT_GENERAL_GEMM_TARGET_V1 {
        return Err(GeneralGemmMirImportErrorV1::new(format!(
            "general GEMM V1 requires exact target `{EXACT_GENERAL_GEMM_TARGET_V1}`, found `{target}`"
        )));
    }
    let body = root.expect("a recognized terminal must belong to one root");
    let surface = unique_surface(&root_calls)?;
    validate_call_shape(body, surface, &root_calls)?;

    let canonical = GeneralGemmKirV1::canonical(non_authoritative_witness_plan()?);
    let mut events = canonical.phase_events().to_vec();
    let mut epilogue = *canonical.epilogue();
    if call_count(&root_calls, TrustedGeneralGemmOperationV1::Publish) == 0 {
        events.retain(|event| {
            !matches!(
                event,
                GeneralGemmPhaseEventV1::Barrier(barrier)
                    if barrier.role == GeneralGemmBarrierRoleV1::Publish
            )
        });
    }
    if call_count(&root_calls, TrustedGeneralGemmOperationV1::Store) == 2 {
        epilogue.store_cardinality = GeneralGemmStoreCardinalityV1::RepeatedPerOwner;
    }
    let kir = GeneralGemmKirV1::checked_from_parts(canonical.plan(), events, epilogue)
        .map_err(|error| GeneralGemmMirImportErrorV1::new(error.to_string()))?;

    Ok(Some(match verify_general_gemm_kir_v1(&kir) {
        Ok(verified) => GeneralGemmMirImportV1::VerifiedWitness {
            surface,
            identity: *verified.kir().identity().as_bytes(),
        },
        Err(diagnostic) => GeneralGemmMirImportV1::Rejected(diagnostic),
    }))
}

fn general_gemm_calls(
    tcx: TyCtxt<'_>,
    body: &Body<'_>,
) -> Result<Vec<GeneralGemmCallV1>, GeneralGemmMirImportErrorV1> {
    let mut calls = Vec::new();
    for (block, data) in body.basic_blocks.iter_enumerated() {
        let Some(terminator) = &data.terminator else {
            return Err(GeneralGemmMirImportErrorV1::new(format!(
                "general GEMM MIR block bb{} has no terminator",
                block.as_usize()
            )));
        };
        let TerminatorKind::Call {
            func,
            target: Some(return_target),
            ..
        } = &terminator.kind
        else {
            continue;
        };
        let Operand::Constant(constant) = func else {
            continue;
        };
        let TyKind::FnDef(def_id, _) = constant.const_.ty().kind() else {
            continue;
        };
        let Some(TrustedDeviceItem::GeneralGemm(surface, operation)) =
            trusted_device_items::classify(tcx, *def_id)
        else {
            continue;
        };
        calls.push(GeneralGemmCallV1 {
            surface,
            operation,
            block,
            return_target: *return_target,
        });
    }
    Ok(calls)
}

fn unique_surface(
    calls: &[GeneralGemmCallV1],
) -> Result<TrustedGeneralGemmSurfaceV1, GeneralGemmMirImportErrorV1> {
    let Some(first) = calls.first() else {
        return Err(GeneralGemmMirImportErrorV1::new(
            "general GEMM adapter was selected without a terminal call",
        ));
    };
    if calls.iter().any(|call| call.surface != first.surface) {
        return Err(GeneralGemmMirImportErrorV1::new(
            "general GEMM kernel mixes typestate and proof-sensitive terminal surfaces",
        ));
    }
    Ok(first.surface)
}

fn validate_call_shape(
    body: &Body<'_>,
    surface: TrustedGeneralGemmSurfaceV1,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    for operation in [
        TrustedGeneralGemmOperationV1::Acquire,
        TrustedGeneralGemmOperationV1::Stage,
        TrustedGeneralGemmOperationV1::Mfma,
        TrustedGeneralGemmOperationV1::Reuse,
    ] {
        require_count(calls, operation, 1, 1)?;
    }
    let publish_minimum = match surface {
        TrustedGeneralGemmSurfaceV1::Typestate => 1,
        TrustedGeneralGemmSurfaceV1::ProofSensitive => 0,
    };
    require_count(
        calls,
        TrustedGeneralGemmOperationV1::Publish,
        publish_minimum,
        1,
    )?;
    let store_maximum = match surface {
        TrustedGeneralGemmSurfaceV1::Typestate => 1,
        TrustedGeneralGemmSurfaceV1::ProofSensitive => 2,
    };
    require_count(
        calls,
        TrustedGeneralGemmOperationV1::Store,
        1,
        store_maximum,
    )?;

    match surface {
        TrustedGeneralGemmSurfaceV1::Typestate => validate_typestate_template_order(body, calls),
        TrustedGeneralGemmSurfaceV1::ProofSensitive => validate_proof_sensitive_order(body, calls),
    }
}

// The move-checked typestate API owns local ordering. This intentionally loose
// template admission remains non-authoritative until runtime plan binding.
fn validate_typestate_template_order(
    body: &Body<'_>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let acquire = unique_call(calls, TrustedGeneralGemmOperationV1::Acquire)?;
    let stage = unique_call(calls, TrustedGeneralGemmOperationV1::Stage)?;
    let mfma = unique_call(calls, TrustedGeneralGemmOperationV1::Mfma)?;
    let reuse = unique_call(calls, TrustedGeneralGemmOperationV1::Reuse)?;
    require_reachable(body, acquire.return_target, stage.block, "acquire", "stage")?;
    if let Some(publish) = optional_call(calls, TrustedGeneralGemmOperationV1::Publish)? {
        require_reachable(body, stage.return_target, publish.block, "stage", "publish")?;
        require_reachable(body, publish.return_target, mfma.block, "publish", "MFMA")?;
    } else {
        require_reachable(body, stage.return_target, mfma.block, "stage", "MFMA")?;
    }
    require_reachable(body, mfma.return_target, reuse.block, "MFMA", "reuse")?;

    let stores = calls
        .iter()
        .filter(|call| call.operation == TrustedGeneralGemmOperationV1::Store)
        .collect::<Vec<_>>();
    require_reachable(body, reuse.return_target, stores[0].block, "reuse", "store")?;
    if stores.len() == 2 {
        require_reachable(
            body,
            stores[0].return_target,
            stores[1].block,
            "first store",
            "second store",
        )?;
    }
    Ok(())
}

fn validate_proof_sensitive_order(
    body: &Body<'_>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let acquire = unique_call(calls, TrustedGeneralGemmOperationV1::Acquire)?;
    let stage = unique_call(calls, TrustedGeneralGemmOperationV1::Stage)?;
    let publish = optional_call(calls, TrustedGeneralGemmOperationV1::Publish)?;
    let mfma = unique_call(calls, TrustedGeneralGemmOperationV1::Mfma)?;
    let reuse = unique_call(calls, TrustedGeneralGemmOperationV1::Reuse)?;
    let stores = calls
        .iter()
        .filter(|call| call.operation == TrustedGeneralGemmOperationV1::Store)
        .collect::<Vec<_>>();

    let mut prefix = vec![acquire, stage];
    prefix.extend(publish);
    prefix.extend([mfma, reuse]);

    let mut ordered = prefix.clone();
    ordered.extend(stores.iter().copied());
    match validate_acyclic_lifecycle(body, calls, &ordered) {
        Ok(()) => Ok(()),
        Err(first_error) if stores.len() == 2 => {
            let mut reversed = prefix;
            reversed.extend([stores[1], stores[0]]);
            validate_acyclic_lifecycle(body, calls, &reversed).map_err(|_| first_error)
        }
        Err(error) => Err(error),
    }
}

fn validate_acyclic_lifecycle(
    body: &Body<'_>,
    calls: &[GeneralGemmCallV1],
    ordered: &[&GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let lifecycle_blocks = calls.iter().map(|call| call.block).collect::<BTreeSet<_>>();
    let mut from = START_BLOCK;
    let mut from_name = "kernel entry";
    for call in ordered {
        require_all_paths_reach_lifecycle(
            body,
            from,
            call.block,
            from_name,
            operation_name(call.operation),
            &lifecycle_blocks,
        )?;
        from = call.return_target;
        from_name = operation_name(call.operation);
    }
    require_all_paths_reach_return(body, from, from_name, &lifecycle_blocks)
}

fn require_all_paths_reach_lifecycle(
    body: &Body<'_>,
    from: BasicBlock,
    to: BasicBlock,
    from_name: &str,
    to_name: &str,
    lifecycle_blocks: &BTreeSet<BasicBlock>,
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let mut pending = VecDeque::from([from]);
    let mut region = BTreeSet::new();
    let mut reached_target = false;
    while let Some(block) = pending.pop_front() {
        if block == to {
            reached_target = true;
            continue;
        }
        if !region.insert(block) {
            continue;
        }
        if lifecycle_blocks.contains(&block) {
            return Err(GeneralGemmMirImportErrorV1::new(format!(
                "proof-sensitive general GEMM reaches a different lifecycle event before {to_name} after {from_name}"
            )));
        }
        let successors = normal_successors(body, block)?;
        if successors.is_empty() {
            return Err(GeneralGemmMirImportErrorV1::new(format!(
                "proof-sensitive general GEMM has a normal path that bypasses {to_name} after {from_name}"
            )));
        }
        pending.extend(successors);
    }
    if !reached_target {
        return Err(GeneralGemmMirImportErrorV1::new(format!(
            "proof-sensitive general GEMM has no normal CFG path from {from_name} to {to_name}"
        )));
    }
    require_acyclic_region(body, &region, Some(to), from_name, to_name)
}

fn require_all_paths_reach_return(
    body: &Body<'_>,
    from: BasicBlock,
    from_name: &str,
    lifecycle_blocks: &BTreeSet<BasicBlock>,
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let mut pending = VecDeque::from([from]);
    let mut region = BTreeSet::new();
    let mut reached_return = false;
    while let Some(block) = pending.pop_front() {
        if !region.insert(block) {
            continue;
        }
        if lifecycle_blocks.contains(&block) {
            return Err(GeneralGemmMirImportErrorV1::new(format!(
                "proof-sensitive general GEMM repeats a lifecycle event after {from_name}"
            )));
        }
        let Some(terminator) = &body.basic_blocks[block].terminator else {
            return Err(missing_terminator(block));
        };
        if matches!(&terminator.kind, TerminatorKind::Return) {
            reached_return = true;
            continue;
        }
        let successors = normal_successors(body, block)?;
        if successors.is_empty() {
            return Err(GeneralGemmMirImportErrorV1::new(format!(
                "proof-sensitive general GEMM has a normal path that does not return after {from_name}"
            )));
        }
        pending.extend(successors);
    }
    if !reached_return {
        return Err(GeneralGemmMirImportErrorV1::new(format!(
            "proof-sensitive general GEMM has no normal return after {from_name}"
        )));
    }
    require_acyclic_region(body, &region, None, from_name, "return")
}

fn require_acyclic_region(
    body: &Body<'_>,
    region: &BTreeSet<BasicBlock>,
    boundary: Option<BasicBlock>,
    from_name: &str,
    to_name: &str,
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let mut indegree = vec![0_usize; body.basic_blocks.len()];
    for &block in region {
        for successor in normal_successors(body, block)? {
            if Some(successor) != boundary && region.contains(&successor) {
                indegree[successor.as_usize()] += 1;
            }
        }
    }
    let mut ready = region
        .iter()
        .copied()
        .filter(|block| indegree[block.as_usize()] == 0)
        .collect::<VecDeque<_>>();
    let mut removed = 0_usize;
    while let Some(block) = ready.pop_front() {
        removed += 1;
        for successor in normal_successors(body, block)? {
            if Some(successor) == boundary || !region.contains(&successor) {
                continue;
            }
            indegree[successor.as_usize()] -= 1;
            if indegree[successor.as_usize()] == 0 {
                ready.push_back(successor);
            }
        }
    }
    if removed != region.len() {
        return Err(GeneralGemmMirImportErrorV1::new(format!(
            "proof-sensitive general GEMM has a cyclic normal CFG between {from_name} and {to_name}"
        )));
    }
    Ok(())
}

fn normal_successors(
    body: &Body<'_>,
    block: BasicBlock,
) -> Result<Vec<BasicBlock>, GeneralGemmMirImportErrorV1> {
    let Some(terminator) = &body.basic_blocks[block].terminator else {
        return Err(missing_terminator(block));
    };
    Ok(terminator
        .successors()
        .filter(|successor| !body.basic_blocks[*successor].is_cleanup)
        .collect())
}

fn missing_terminator(block: BasicBlock) -> GeneralGemmMirImportErrorV1 {
    GeneralGemmMirImportErrorV1::new(format!(
        "general GEMM MIR block bb{} has no terminator",
        block.as_usize()
    ))
}

const fn operation_name(operation: TrustedGeneralGemmOperationV1) -> &'static str {
    match operation {
        TrustedGeneralGemmOperationV1::Acquire => "acquire",
        TrustedGeneralGemmOperationV1::Lane => "lane",
        TrustedGeneralGemmOperationV1::WorkgroupX => "workgroup_x",
        TrustedGeneralGemmOperationV1::WorkgroupY => "workgroup_y",
        TrustedGeneralGemmOperationV1::LoadA => "load_a",
        TrustedGeneralGemmOperationV1::LoadB => "load_b",
        TrustedGeneralGemmOperationV1::LoadC => "load_c",
        TrustedGeneralGemmOperationV1::Stage => "stage",
        TrustedGeneralGemmOperationV1::StageValue => "stage_value",
        TrustedGeneralGemmOperationV1::WaitStage => "wait_stage",
        TrustedGeneralGemmOperationV1::ReadStage => "read_stage",
        TrustedGeneralGemmOperationV1::Publish => "publish",
        TrustedGeneralGemmOperationV1::Mfma => "MFMA",
        TrustedGeneralGemmOperationV1::MfmaValue => "MFMA_value",
        TrustedGeneralGemmOperationV1::Reuse => "reuse",
        TrustedGeneralGemmOperationV1::Store => "store",
        TrustedGeneralGemmOperationV1::StoreEpilogue => "store_epilogue",
    }
}

fn require_count(
    calls: &[GeneralGemmCallV1],
    operation: TrustedGeneralGemmOperationV1,
    minimum: usize,
    maximum: usize,
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let actual = call_count(calls, operation);
    if actual < minimum || actual > maximum {
        return Err(GeneralGemmMirImportErrorV1::new(format!(
            "general GEMM MIR has {actual} {operation:?} terminal call(s); expected {minimum} through {maximum}"
        )));
    }
    Ok(())
}

fn call_count(calls: &[GeneralGemmCallV1], operation: TrustedGeneralGemmOperationV1) -> usize {
    calls
        .iter()
        .filter(|call| call.operation == operation)
        .count()
}

fn unique_call(
    calls: &[GeneralGemmCallV1],
    operation: TrustedGeneralGemmOperationV1,
) -> Result<&GeneralGemmCallV1, GeneralGemmMirImportErrorV1> {
    optional_call(calls, operation)?.ok_or_else(|| {
        GeneralGemmMirImportErrorV1::new(format!(
            "general GEMM MIR omitted required {operation:?} terminal"
        ))
    })
}

fn optional_call(
    calls: &[GeneralGemmCallV1],
    operation: TrustedGeneralGemmOperationV1,
) -> Result<Option<&GeneralGemmCallV1>, GeneralGemmMirImportErrorV1> {
    let mut matches = calls.iter().filter(|call| call.operation == operation);
    let first = matches.next();
    if matches.next().is_some() {
        return Err(GeneralGemmMirImportErrorV1::new(format!(
            "general GEMM MIR has multiple {operation:?} terminals where one was required"
        )));
    }
    Ok(first)
}

fn require_reachable(
    body: &Body<'_>,
    from: BasicBlock,
    to: BasicBlock,
    from_name: &str,
    to_name: &str,
) -> Result<(), GeneralGemmMirImportErrorV1> {
    if reachable(body, from, to) {
        Ok(())
    } else {
        Err(GeneralGemmMirImportErrorV1::new(format!(
            "general GEMM MIR has no CFG path from {from_name} to {to_name}"
        )))
    }
}

fn reachable(body: &Body<'_>, from: BasicBlock, to: BasicBlock) -> bool {
    let mut pending = VecDeque::from([from]);
    let mut visited = BTreeSet::new();
    while let Some(block) = pending.pop_front() {
        if block == to {
            return true;
        }
        if !visited.insert(block) {
            continue;
        }
        let Some(terminator) = &body.basic_blocks[block].terminator else {
            return false;
        };
        pending.extend(terminator.successors());
    }
    false
}

fn non_authoritative_witness_plan() -> Result<GeneralGemmPlanFieldsV1, GeneralGemmMirImportErrorV1>
{
    GeneralGemmPlanFieldsV1::checked(GeneralGemmPlanSnapshotV1 {
        dimensions: [17, 19, 18],
        strides: [23, 29, 31],
        storage_elements: [386, 512, 515],
        block_counts: [2, 2, 1],
        aql_grid_work_items: [128, 2, 1],
        reduction_phases: 2,
        alpha_bits: 2.0_f32.to_bits(),
        beta_bits: (-1.0_f32).to_bits(),
    })
    .map_err(|error| GeneralGemmMirImportErrorV1::new(error.to_string()))
}
