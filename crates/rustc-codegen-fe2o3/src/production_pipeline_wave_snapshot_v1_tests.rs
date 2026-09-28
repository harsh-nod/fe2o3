//! Bounded typed diagnostic for actual P4 endpoints; no admission or error change.
use super::Owner;
use fe2o3_kernel_ir::{
    OperationKind, SynchronizationScope, Type, WaveF32ReductionKindV1, WaveOperationKind,
};
use serde::{Deserialize, Serialize};

const MAX_FUNCTIONS: usize = 64;
const MAX_BLOCKS: usize = 4096;
const MAX_OPERATIONS: usize = 16384;
const MAX_ROWS: usize = 4;

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Schema {
    #[serde(rename = "fe2o3-p4-wave-observation-v1")]
    V1,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    LaneId,
    Ballot,
    Any,
    All,
    ShuffleIndex,
    ReduceSumF32,
    ReduceMaximumF32,
    BroadcastF32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    function: usize,
    block: usize,
    operation: usize,
    kind: Kind,
    width: u32,
    active_lanes: u32,
    subgroup_scope: bool,
    tile: Option<u32>,
    operands: [Option<u32>; 2],
    results: usize,
    result: Option<u32>,
    result_is_f32: bool,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Summary {
    schema: Schema,
    owner_sha256: [u8; 32],
    identifies_failing_endpoint: bool,
    grants_authority: bool,
    complete: bool,
    visited_blocks: usize,
    visited_operations: usize,
    rows: Vec<Row>,
}

pub(super) fn observe(owner: &Owner) -> Summary {
    let mut result = Summary {
        schema: Schema::V1,
        owner_sha256: *owner.canonical().identity().digest(),
        identifies_failing_endpoint: false,
        grants_authority: false,
        complete: true,
        visited_blocks: 0,
        visited_operations: 0,
        rows: Vec::with_capacity(MAX_ROWS),
    };
    for (function, f) in owner.module().functions.iter().enumerate() {
        if function == MAX_FUNCTIONS {
            result.complete = false;
            break;
        }
        let Some(body) = &f.body else { continue };
        for (block, b) in body.blocks.iter().enumerate() {
            if result.visited_blocks == MAX_BLOCKS {
                result.complete = false;
                return result;
            }
            result.visited_blocks += 1;
            for (operation, op) in b.operations.iter().enumerate() {
                if result.visited_operations == MAX_OPERATIONS {
                    result.complete = false;
                    return result;
                }
                result.visited_operations += 1;
                let OperationKind::Wave(wave) = &op.kind else {
                    continue;
                };
                if result.rows.len() == MAX_ROWS {
                    result.complete = false;
                    return result;
                }
                let (kind, tile, operands) = match wave.kind {
                    WaveOperationKind::LaneId => (Kind::LaneId, None, [None, None]),
                    WaveOperationKind::Ballot { predicate } => {
                        (Kind::Ballot, None, [Some(predicate.0), None])
                    }
                    WaveOperationKind::Any { predicate } => {
                        (Kind::Any, None, [Some(predicate.0), None])
                    }
                    WaveOperationKind::All { predicate } => {
                        (Kind::All, None, [Some(predicate.0), None])
                    }
                    WaveOperationKind::ShuffleIndex {
                        value,
                        source_lane,
                        tile_width,
                    } => (
                        Kind::ShuffleIndex,
                        Some(tile_width),
                        [Some(value.0), Some(source_lane.0)],
                    ),
                    WaveOperationKind::ReduceF32 {
                        value,
                        tile_width,
                        kind,
                    } => (
                        match kind {
                            WaveF32ReductionKindV1::Sum => Kind::ReduceSumF32,
                            WaveF32ReductionKindV1::Maximum => Kind::ReduceMaximumF32,
                        },
                        Some(tile_width),
                        [Some(value.0), None],
                    ),
                    WaveOperationKind::BroadcastF32 {
                        value,
                        source_lane,
                        tile_width,
                    } => (
                        Kind::BroadcastF32,
                        Some(tile_width),
                        [Some(value.0), Some(source_lane.0)],
                    ),
                };
                result.rows.push(Row {
                    function,
                    block,
                    operation,
                    kind,
                    width: wave.width.lanes(),
                    active_lanes: wave.active_lanes,
                    subgroup_scope: wave.convergence.scope() == SynchronizationScope::Subgroup,
                    tile,
                    operands,
                    results: op.results.len(),
                    result: op.results.first().map(|result| result.id.0),
                    result_is_f32: op
                        .results
                        .first()
                        .is_some_and(|result| result.ty == Type::F32),
                });
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::*;
    fn owner(count: usize) -> Owner {
        let mut module = Module::new("typed-wave-diagnostic");
        let mut block = BasicBlock::new(BlockId(97));
        for index in 0..count {
            block.operations.push(Operation::new(
                vec![ValueDef::new(ValueId(100 + index as u32), Type::F32)],
                OperationKind::Wave(WaveOperation::full(
                    WaveOperationKind::ReduceF32 {
                        value: ValueId(17),
                        tile_width: 16,
                        kind: if index % 2 == 0 {
                            WaveF32ReductionKindV1::Sum
                        } else {
                            WaveF32ReductionKindV1::Maximum
                        },
                    },
                    WaveWidth::Wave64,
                )),
            ));
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            "wave",
            Signature::new(vec![Type::F32], vec![]),
            vec![ValueId(17)],
            vec![block],
        ));
        module.required_capabilities =
            WaveOperation::full(WaveOperationKind::LaneId, WaveWidth::Wave64)
                .required_capabilities();
        module.kernels.push(Kernel::new(
            "wave",
            "wave",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget)
            .unwrap()
            .0
    }
    #[test]
    fn wave_snapshot_exact_typed_fields_bind_actual_owner_not_failing_endpoint() {
        let owner = owner(2);
        let result = observe(&owner);
        assert_eq!(result.owner_sha256, *owner.canonical().identity().digest());
        assert!(result.complete);
        assert!(!result.identifies_failing_endpoint && !result.grants_authority);
        assert_eq!(result.rows.len(), 2);
        assert_eq!(
            (
                result.rows[0].function,
                result.rows[0].block,
                result.rows[0].operation
            ),
            (0, 0, 0)
        );
        assert_eq!(
            (result.rows[0].kind, result.rows[1].kind),
            (Kind::ReduceSumF32, Kind::ReduceMaximumF32)
        );
        for (index, row) in result.rows.iter().enumerate() {
            assert_eq!((row.width, row.active_lanes, row.tile), (64, 64, Some(16)));
            assert_eq!(row.operands, [Some(17), None]);
            assert_eq!((row.results, row.result), (1, Some(100 + index as u32)));
            assert!(row.result_is_f32 && row.subgroup_scope);
        }
        let bytes = serde_json::to_vec(&result).unwrap();
        assert_eq!(serde_json::from_slice::<Summary>(&bytes).unwrap(), result);
        let mut value = serde_json::to_value(&result).unwrap();
        value["rows"][0]["unknown_authority"] = true.into();
        assert!(serde_json::from_value::<Summary>(value).is_err());
        let mut value = serde_json::to_value(&result).unwrap();
        value["schema"] = "future-unchecked-version".into();
        assert!(serde_json::from_value::<Summary>(value).is_err());
        let mut value = serde_json::to_value(&result).unwrap();
        value["rows"][0]["kind"] = "unknown_wave_family".into();
        assert!(serde_json::from_value::<Summary>(value).is_err());
    }
    #[test]
    fn wave_snapshot_caps_rows_and_reports_empty_unchanged_owner() {
        let empty = owner(0);
        assert!(observe(&empty).rows.is_empty());
        let many = owner(MAX_ROWS + 1);
        let before = *many.canonical().identity().digest();
        let observed = observe(&many);
        assert_eq!(observed.rows.len(), MAX_ROWS);
        assert!(!observed.complete);
        assert_eq!(many.canonical().identity().digest(), &before);
        assert_ne!(observed.owner_sha256, observe(&empty).owner_sha256);
    }

    #[test]
    fn wave_snapshot_maximal_summaries_fit_the_existing_endpoint_index_cap() {
        use super::super::{CANONICAL_LIMIT, Endpoint, GRAPH_LIMIT, INDEX_LIMIT, Index};
        let owner = owner(MAX_ROWS);
        let index = Index {
            schema: "fe2o3-policy4-endpoint-snapshot-v1",
            point: "after-checked-reservation-before-ranked-receipt-and-admission",
            identifies_failing_endpoint: false,
            canonical_limit: CANONICAL_LIMIT,
            graph_limit: GRAPH_LIMIT,
            endpoints: ["B", "C", "O"].map(|role| Endpoint {
                role,
                identity_sha256: "f".repeat(64),
                canonical_bytes: CANONICAL_LIMIT,
                state: "complete",
                diagnostic: None,
                wave_details: observe(&owner),
            }),
        };
        assert!(serde_json::to_vec_pretty(&index).unwrap().len() < INDEX_LIMIT);
    }
}
