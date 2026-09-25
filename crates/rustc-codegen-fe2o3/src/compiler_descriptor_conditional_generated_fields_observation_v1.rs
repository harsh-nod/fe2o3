//! Passive, test-only copies from the actual descriptor/proof replay callback.
//! Observed rows and receipt digests are inert; completion is not finalization.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticLocalRoleV1;
use fe2o3_verifier::ProductionConditionalFormulaExecutionV1 as Execution;
use serde::Serialize;
use std::cell::RefCell;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct Argument {
    pub(crate) canonical_parameter: u32,
    pub(crate) canonical_value: u32,
    pub(crate) source_argument: u32,
    pub(crate) adjusted_argument: u32,
    pub(crate) semantic_local: u32,
    pub(crate) semantic_type: u32,
    pub(crate) generated_field: u16,
    pub(crate) output: bool,
    pub(crate) source_type_identity: [u8; 32],
    pub(crate) device_layout_identity: [u8; 32],
    /// Actual retained typed descriptor values, not recomputed from the row.
    pub(crate) generated_offset: u32,
    pub(crate) generated_semantic_type_identity: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct Read {
    pub(crate) argument: u16,
    pub(crate) canonical_parameter: u32,
    pub(crate) source_argument: u32,
    pub(crate) adjusted_argument: u32,
    pub(crate) semantic_local: u32,
    pub(crate) semantic_type: u32,
    pub(crate) canonical_block: u32,
    pub(crate) canonical_operation: usize,
    pub(crate) ranked_block: u32,
    pub(crate) ranked_operation: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct BodyArgument {
    pub(crate) source_argument: u32,
    pub(crate) semantic_local: u32,
    pub(crate) semantic_type: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct Projection {
    pub(crate) root: u32,
    pub(crate) body: u32,
    pub(crate) kernel_binding: [u8; 32],
    /// Independent source-body roles, not copied from generated-field rows.
    pub(crate) body_arguments: Vec<BodyArgument>,
    pub(crate) arguments: Vec<Argument>,
    pub(crate) output_argument: u16,
    pub(crate) reads: Vec<Read>,
    pub(crate) canonical_store_block: u32,
    pub(crate) canonical_store_operation: usize,
    pub(crate) statement: [u8; 32],
    pub(crate) receipt: [u8; 32],
    pub(crate) work: usize,
    pub(crate) storage: usize,
}

#[derive(Debug, Serialize)]
pub(crate) enum Event {
    /// Descriptor callback only; verifier/lower/phase postchecks are still pending.
    ProjectionCallback(Projection),
    /// The same ranked program returned after ALL enclosing replay postchecks.
    ReplayCompleted,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct Observation {
    pub(crate) events: Vec<Event>,
}
thread_local! {
    static ACTIVE: RefCell<Option<Observation>> = const { RefCell::new(None) };
}
struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        ACTIVE.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

/// Surround one real target entrypoint. This cannot install owners, receipts,
/// a budget or a consumer callback; it only copies test diagnostics.
pub(crate) fn observe<R>(run: impl FnOnce() -> R) -> (R, Observation) {
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        assert!(slot.is_none(), "generated-field observer must not nest");
        *slot = Some(Observation::default());
    });
    let restore = Restore;
    let result = run();
    let observed = ACTIVE.with(|slot| slot.borrow_mut().take().unwrap());
    drop(restore);
    (result, observed)
}

fn record(event: impl FnOnce() -> Event) {
    ACTIVE.with(|slot| {
        if let Some(observation) = slot.borrow_mut().as_mut() {
            // One callback per root, then the whole-roster completion marker.
            assert!(observation.events.len() < MAX_CONDITIONAL_ROOTS_V1 + 1);
            observation.events.push(event());
        }
    });
}

pub(crate) fn projection_callback(
    fields: &ConditionalGeneratedFieldsV1<'_>,
    execution: &Execution,
    budget: &Budget<'_>,
) {
    record(|| {
        let input = fields.request().pliron_input();
        assert!(fields.arguments().len() <= MAX_CONDITIONAL_ARGUMENTS_V1);
        assert!(input.reads().len() <= MAX_CONDITIONAL_READS_V1);
        assert_eq!(fields.read_arguments().len(), input.reads().len());
        let semantic = fields.request().source().semantic_ssa().source_semantic();
        assert_eq!(
            semantic
                .select_kernel_body_for_root_v1(fields.semantic_root())
                .unwrap()
                .body(),
            fields.semantic_body()
        );
        let body_arguments = semantic.functions()[fields.semantic_body().index() as usize]
            .locals()
            .iter()
            .enumerate()
            .filter_map(|(index, local)| {
                let SemanticLocalRoleV1::Argument(source_argument) = local.role() else {
                    return None;
                };
                Some(BodyArgument {
                    source_argument,
                    semantic_local: u32::try_from(index).unwrap(),
                    semantic_type: local.ty().index(),
                })
            })
            .collect();
        let arguments = fields
            .arguments()
            .iter()
            .map(|argument| {
                let row = argument.projection();
                let generated =
                    &fields.typed_root().arguments.as_slice()[usize::from(row.generated_field)];
                Argument {
                    canonical_parameter: row.canonical_parameter,
                    canonical_value: argument.canonical_value().0,
                    source_argument: row.source_argument,
                    adjusted_argument: row.adjusted_argument,
                    semantic_local: row.semantic_local,
                    semantic_type: row.semantic_type,
                    generated_field: row.generated_field,
                    output: row.role == Role::Output,
                    source_type_identity: row.source_type_identity,
                    device_layout_identity: row.device_layout_identity,
                    generated_offset: generated.offset,
                    generated_semantic_type_identity: *generated.semantic_type_identity.as_bytes(),
                }
            })
            .collect();
        let reads = input
            .reads()
            .iter()
            .zip(fields.read_arguments())
            .map(|(read, argument)| {
                let source = read.source();
                Read {
                    argument: *argument,
                    canonical_parameter: source.canonical_parameter(),
                    source_argument: source.source_argument(),
                    adjusted_argument: source.adjusted_argument(),
                    semantic_local: source.semantic_local().index(),
                    semantic_type: source.semantic_type().index(),
                    canonical_block: read.canonical().location().block.0,
                    canonical_operation: read.canonical().location().operation_index,
                    ranked_block: read.site().block,
                    ranked_operation: read.site().operation,
                }
            })
            .collect();
        let store = input.canonical_output_store_location_v1();
        Event::ProjectionCallback(Projection {
            root: fields.semantic_root().index(),
            body: fields.semantic_body().index(),
            kernel_binding: fields.typed_root().kernel_binding_bytes(),
            body_arguments,
            arguments,
            output_argument: fields.output_argument(),
            reads,
            canonical_store_block: store.block.0,
            canonical_store_operation: store.operation_index,
            statement: *execution.report().statement_identity().as_bytes(),
            receipt: *execution.report().receipt_identity().as_bytes(),
            work: budget.work(),
            storage: budget.storage(),
        })
    });
}

pub(crate) fn replay_completed() {
    record(|| Event::ReplayCompleted);
}

#[path = "compiler_descriptor_conditional_generated_fields_observation_v1_tests.rs"]
mod tests;
pub(crate) use tests::check;
