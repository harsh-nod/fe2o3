//! Deterministic, transactional optimization of verified Kernel IR.
//!
//! The optimizer deliberately exposes a closed pass order. One hard-bounded
//! private clone is transformed in place and the complete result is published
//! only after all pass budgets and verifier checks succeed. Consequently an
//! error never returns a partially optimized module.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;

use fe2o3_kernel_ir::{
    BinaryOp, CastKind, Gfx950LdsTransposeOperationKindV1, KernelIrEncodeError,
    MAX_MODULE_BYTES_V1, MatrixOperationKind, MemoryIntrinsicOperation, Module, Operation,
    OperationKind, Terminator, UnaryOp, ValueId, VerificationErrors, WaveOperationKind,
    encode_module_v9, verify_module,
};

pub const MAX_KERNEL_IR_OPTIMIZATION_WORK_UNITS_V1: u64 = 16_777_216;
pub const MAX_KERNEL_IR_OPTIMIZATION_MUTATIONS_V1: u64 = 65_536;
pub const MAX_KERNEL_IR_OPTIMIZATION_STORAGE_ITEMS_V1: u64 = 16_777_216;
pub const MAX_KERNEL_IR_OPTIMIZATION_MODULE_BYTES_V1: usize = MAX_MODULE_BYTES_V1;

/// Closed, deterministic order of the first Kernel IR optimization pipeline.
pub const KERNEL_IR_OPTIMIZATION_PASS_ORDER_V1: [KernelIrOptimizationPassV1; 2] = [
    KernelIrOptimizationPassV1::RemoveUnreachableBlocks,
    KernelIrOptimizationPassV1::EliminateDeadPureOperations,
];

/// One target-neutral transformation in the V1 optimization pipeline.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelIrOptimizationPassV1 {
    RemoveUnreachableBlocks,
    EliminateDeadPureOperations,
}

impl fmt::Display for KernelIrOptimizationPassV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::RemoveUnreachableBlocks => "remove-unreachable-blocks",
            Self::EliminateDeadPureOperations => "eliminate-dead-pure-operations",
        })
    }
}

/// Independent deterministic resource limits for one optimization pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrPassBudgetV1 {
    pub max_work_units: u64,
    pub max_mutations: u64,
    pub max_storage_items: u64,
}

impl KernelIrPassBudgetV1 {
    pub const fn new(max_work_units: u64, max_mutations: u64) -> Self {
        Self {
            max_work_units,
            max_mutations,
            max_storage_items: MAX_KERNEL_IR_OPTIMIZATION_STORAGE_ITEMS_V1,
        }
    }

    pub const fn with_storage(
        max_work_units: u64,
        max_mutations: u64,
        max_storage_items: u64,
    ) -> Self {
        Self {
            max_work_units,
            max_mutations,
            max_storage_items,
        }
    }
}

/// Resource limits for the closed V1 pass pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrOptimizationLimitsV1 {
    /// Maximum accepted size of the fully materialized canonical V9 input.
    ///
    /// [`encode_module_v9`] applies the fixed
    /// [`MAX_KERNEL_IR_OPTIMIZATION_MODULE_BYTES_V1`] hard bound while it
    /// encodes. This configurable cap is an encoded-output admission policy;
    /// it cannot prevent the preceding hard-bounded encoding allocation.
    pub max_module_bytes: usize,
    pub remove_unreachable_blocks: KernelIrPassBudgetV1,
    pub eliminate_dead_pure_operations: KernelIrPassBudgetV1,
}

impl KernelIrOptimizationLimitsV1 {
    pub const DEFAULT: Self = Self {
        max_module_bytes: MAX_KERNEL_IR_OPTIMIZATION_MODULE_BYTES_V1,
        remove_unreachable_blocks: KernelIrPassBudgetV1::new(
            MAX_KERNEL_IR_OPTIMIZATION_WORK_UNITS_V1,
            MAX_KERNEL_IR_OPTIMIZATION_MUTATIONS_V1,
        ),
        eliminate_dead_pure_operations: KernelIrPassBudgetV1::new(
            MAX_KERNEL_IR_OPTIMIZATION_WORK_UNITS_V1,
            MAX_KERNEL_IR_OPTIMIZATION_MUTATIONS_V1,
        ),
    };

    pub const fn budget_for(self, pass: KernelIrOptimizationPassV1) -> KernelIrPassBudgetV1 {
        match pass {
            KernelIrOptimizationPassV1::RemoveUnreachableBlocks => self.remove_unreachable_blocks,
            KernelIrOptimizationPassV1::EliminateDeadPureOperations => {
                self.eliminate_dead_pure_operations
            }
        }
    }
}

impl Default for KernelIrOptimizationLimitsV1 {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Resource whose deterministic pass budget was exhausted.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelIrOptimizationResourceV1 {
    /// Fully materialized canonical V9 bytes admitted after fixed-hard-bounded
    /// encoding, not a caller-selected encoder allocation limit.
    CanonicalBytes,
    WorkUnits,
    Mutations,
    StorageItems,
}

impl fmt::Display for KernelIrOptimizationResourceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CanonicalBytes => "encoded module bytes",
            Self::WorkUnits => "work units",
            Self::Mutations => "mutations",
            Self::StorageItems => "storage items",
        })
    }
}

/// Whether a verifier failure occurred before or after a pass candidate ran.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelIrOptimizationVerificationPhaseV1 {
    BeforePass,
    AfterPass,
}

/// Fail-closed optimization failure. No variant contains a candidate module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelIrOptimizationErrorV1 {
    InvalidLimit {
        pass: Option<KernelIrOptimizationPassV1>,
        resource: KernelIrOptimizationResourceV1,
        requested: u64,
        hard_maximum: u64,
    },
    InputEncoding(KernelIrEncodeError),
    Verification {
        pass: KernelIrOptimizationPassV1,
        phase: KernelIrOptimizationVerificationPhaseV1,
        epoch: u64,
        errors: VerificationErrors,
    },
    BudgetExceeded {
        pass: KernelIrOptimizationPassV1,
        resource: KernelIrOptimizationResourceV1,
        limit: u64,
        attempted: u64,
    },
    CounterOverflow {
        pass: KernelIrOptimizationPassV1,
        resource: KernelIrOptimizationResourceV1,
    },
    MutationEpochOverflow {
        pass: KernelIrOptimizationPassV1,
        epoch: u64,
    },
}

impl fmt::Display for KernelIrOptimizationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimit {
                pass,
                resource,
                requested,
                hard_maximum,
            } => write!(
                formatter,
                "configured {}{resource} limit {requested} exceeds the hard maximum {hard_maximum}",
                pass.map_or(String::new(), |pass| format!("{pass} "))
            ),
            Self::InputEncoding(error) => {
                write!(
                    formatter,
                    "Kernel IR optimization input is not bounded V9: {error}"
                )
            }
            Self::Verification {
                pass,
                phase,
                epoch,
                errors,
            } => write!(
                formatter,
                "Kernel IR verification failed {phase:?} {pass} at mutation epoch {epoch}: {errors}"
            ),
            Self::BudgetExceeded {
                pass,
                resource,
                limit,
                attempted,
            } => write!(
                formatter,
                "{pass} exceeded its deterministic {resource} budget {limit}: attempted {attempted}"
            ),
            Self::CounterOverflow { pass, resource } => {
                write!(formatter, "{pass} overflowed its {resource} counter")
            }
            Self::MutationEpochOverflow { pass, epoch } => {
                write!(formatter, "{pass} cannot advance mutation epoch {epoch}")
            }
        }
    }
}

impl Error for KernelIrOptimizationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InputEncoding(error) => Some(error),
            Self::Verification { errors, .. } => Some(errors),
            _ => None,
        }
    }
}

/// Auditable outcome of one committed or no-op pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrOptimizationPassReportV1 {
    pub pass: KernelIrOptimizationPassV1,
    pub input_epoch: u64,
    pub output_epoch: u64,
    pub changed: bool,
    pub work_units: u64,
    pub mutations: u64,
    pub peak_storage_items: u64,
}

/// Deterministic report for the complete fixed-order pipeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelIrOptimizationReportV1 {
    /// Size of the fully materialized, fixed-hard-bounded canonical V9 input.
    pub input_canonical_bytes: usize,
    pub initial_epoch: u64,
    pub final_epoch: u64,
    pub passes: Vec<KernelIrOptimizationPassReportV1>,
}

/// Fully verified result of the complete V1 optimization pipeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptimizedKernelIrModuleV1 {
    module: Module,
    report: KernelIrOptimizationReportV1,
}

impl OptimizedKernelIrModuleV1 {
    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn report(&self) -> &KernelIrOptimizationReportV1 {
        &self.report
    }

    pub fn into_module(self) -> Module {
        self.module
    }

    pub fn into_parts(self) -> (Module, KernelIrOptimizationReportV1) {
        (self.module, self.report)
    }
}

/// Runs the fixed-order V1 pipeline transactionally from mutation epoch zero.
///
/// The input is borrowed and never mutated. The bounded verified input is
/// cloned once into a private transaction. A failure drops that transaction, so
/// callers can only observe either the unchanged input or a fully verified
/// [`OptimizedKernelIrModuleV1`].
pub fn optimize_kernel_ir_module_v1(
    input: &Module,
    limits: KernelIrOptimizationLimitsV1,
) -> Result<OptimizedKernelIrModuleV1, KernelIrOptimizationErrorV1> {
    optimize_kernel_ir_module_at_epoch_v1(input, 0, limits)
}

/// Runs the fixed-order V1 pipeline from an existing mutation epoch.
///
/// This entry point lets an owning compiler transaction preserve monotonic
/// epochs across several optimization sessions. The epoch advances once for
/// every pass that commits one or more mutations.
///
/// Canonical encoding hard-bounds the module to 16 MiB before cloning. The
/// configurable `max_module_bytes` admission cap is checked against that
/// already materialized, fixed-hard-bounded encoding. The pipeline makes one
/// `O(M)` private clone. It verifies the input once and each changed checkpoint,
/// for at most `P + 1` full verifier traversals, where V1 fixes `P = 2`.
/// Reachability and DCE use ordered maps, so their current bound is `O(M log M)`
/// time with explicitly metered auxiliary storage.
pub fn optimize_kernel_ir_module_at_epoch_v1(
    input: &Module,
    initial_epoch: u64,
    limits: KernelIrOptimizationLimitsV1,
) -> Result<OptimizedKernelIrModuleV1, KernelIrOptimizationErrorV1> {
    validate_limits(limits)?;
    // Encoding is fixed-hard-bounded by Kernel IR and runs before cloning. The
    // configurable cap below is deliberately a post-encoding admission check:
    // exact encoded size is unavailable until the bounded encoding exists.
    let canonical = encode_module_v9(input).map_err(KernelIrOptimizationErrorV1::InputEncoding)?;
    check_budget(
        None,
        KernelIrOptimizationResourceV1::CanonicalBytes,
        u64::try_from(canonical.len()).unwrap_or(u64::MAX),
        u64::try_from(limits.max_module_bytes).unwrap_or(u64::MAX),
    )?;
    let first_pass = KERNEL_IR_OPTIMIZATION_PASS_ORDER_V1[0];
    verify_module(input).map_err(|errors| KernelIrOptimizationErrorV1::Verification {
        pass: first_pass,
        phase: KernelIrOptimizationVerificationPhaseV1::BeforePass,
        epoch: initial_epoch,
        errors,
    })?;

    let input_canonical_bytes = canonical.len();
    drop(canonical);
    let mut current = input.clone();
    let mut epoch = initial_epoch;
    let mut reports = Vec::with_capacity(KERNEL_IR_OPTIMIZATION_PASS_ORDER_V1.len());

    for pass in KERNEL_IR_OPTIMIZATION_PASS_ORDER_V1 {
        let mut meter = PassMeter::new(pass, limits.budget_for(pass));
        match pass {
            KernelIrOptimizationPassV1::RemoveUnreachableBlocks => {
                remove_unreachable_blocks(&mut current, &mut meter)?;
            }
            KernelIrOptimizationPassV1::EliminateDeadPureOperations => {
                eliminate_dead_pure_operations(&mut current, &mut meter)?;
            }
        }

        let input_epoch = epoch;
        if meter.mutations != 0 {
            epoch = epoch
                .checked_add(1)
                .ok_or(KernelIrOptimizationErrorV1::MutationEpochOverflow { pass, epoch })?;
        }
        if meter.mutations != 0 {
            verify_module(&current).map_err(|errors| {
                KernelIrOptimizationErrorV1::Verification {
                    pass,
                    phase: KernelIrOptimizationVerificationPhaseV1::AfterPass,
                    epoch,
                    errors,
                }
            })?;
        }

        reports.push(KernelIrOptimizationPassReportV1 {
            pass,
            input_epoch,
            output_epoch: epoch,
            changed: meter.mutations != 0,
            work_units: meter.work_units,
            mutations: meter.mutations,
            peak_storage_items: meter.peak_storage_items,
        });
    }

    Ok(OptimizedKernelIrModuleV1 {
        module: current,
        report: KernelIrOptimizationReportV1 {
            input_canonical_bytes,
            initial_epoch,
            final_epoch: epoch,
            passes: reports,
        },
    })
}

struct PassMeter {
    pass: KernelIrOptimizationPassV1,
    budget: KernelIrPassBudgetV1,
    work_units: u64,
    mutations: u64,
    storage_items: u64,
    peak_storage_items: u64,
}

impl PassMeter {
    const fn new(pass: KernelIrOptimizationPassV1, budget: KernelIrPassBudgetV1) -> Self {
        Self {
            pass,
            budget,
            work_units: 0,
            mutations: 0,
            storage_items: 0,
            peak_storage_items: 0,
        }
    }

    fn charge_work(&mut self, amount: usize) -> Result<(), KernelIrOptimizationErrorV1> {
        self.work_units = add_counter(
            self.pass,
            KernelIrOptimizationResourceV1::WorkUnits,
            self.work_units,
            amount,
        )?;
        check_budget(
            Some(self.pass),
            KernelIrOptimizationResourceV1::WorkUnits,
            self.work_units,
            self.budget.max_work_units,
        )
    }

    fn record_mutations(&mut self, amount: usize) -> Result<(), KernelIrOptimizationErrorV1> {
        self.mutations = add_counter(
            self.pass,
            KernelIrOptimizationResourceV1::Mutations,
            self.mutations,
            amount,
        )?;
        check_budget(
            Some(self.pass),
            KernelIrOptimizationResourceV1::Mutations,
            self.mutations,
            self.budget.max_mutations,
        )
    }

    fn reserve_storage(&mut self, amount: usize) -> Result<(), KernelIrOptimizationErrorV1> {
        self.storage_items = add_counter(
            self.pass,
            KernelIrOptimizationResourceV1::StorageItems,
            self.storage_items,
            amount,
        )?;
        check_budget(
            Some(self.pass),
            KernelIrOptimizationResourceV1::StorageItems,
            self.storage_items,
            self.budget.max_storage_items,
        )?;
        self.peak_storage_items = self.peak_storage_items.max(self.storage_items);
        Ok(())
    }

    fn release_storage(&mut self, amount: usize) {
        self.storage_items = self
            .storage_items
            .checked_sub(u64::try_from(amount).expect("reserved storage count fits u64"))
            .expect("pass storage release matches a prior reservation");
    }
}

fn add_counter(
    pass: KernelIrOptimizationPassV1,
    resource: KernelIrOptimizationResourceV1,
    current: u64,
    amount: usize,
) -> Result<u64, KernelIrOptimizationErrorV1> {
    let amount = u64::try_from(amount)
        .map_err(|_| KernelIrOptimizationErrorV1::CounterOverflow { pass, resource })?;
    current
        .checked_add(amount)
        .ok_or(KernelIrOptimizationErrorV1::CounterOverflow { pass, resource })
}

fn check_budget(
    pass: Option<KernelIrOptimizationPassV1>,
    resource: KernelIrOptimizationResourceV1,
    attempted: u64,
    limit: u64,
) -> Result<(), KernelIrOptimizationErrorV1> {
    if attempted > limit {
        Err(KernelIrOptimizationErrorV1::BudgetExceeded {
            pass: pass.unwrap_or(KERNEL_IR_OPTIMIZATION_PASS_ORDER_V1[0]),
            resource,
            limit,
            attempted,
        })
    } else {
        Ok(())
    }
}

fn validate_limits(
    limits: KernelIrOptimizationLimitsV1,
) -> Result<(), KernelIrOptimizationErrorV1> {
    validate_limit(
        None,
        KernelIrOptimizationResourceV1::CanonicalBytes,
        u64::try_from(limits.max_module_bytes).unwrap_or(u64::MAX),
        u64::try_from(MAX_KERNEL_IR_OPTIMIZATION_MODULE_BYTES_V1).unwrap_or(u64::MAX),
    )?;
    for pass in KERNEL_IR_OPTIMIZATION_PASS_ORDER_V1 {
        let budget = limits.budget_for(pass);
        validate_limit(
            Some(pass),
            KernelIrOptimizationResourceV1::WorkUnits,
            budget.max_work_units,
            MAX_KERNEL_IR_OPTIMIZATION_WORK_UNITS_V1,
        )?;
        validate_limit(
            Some(pass),
            KernelIrOptimizationResourceV1::Mutations,
            budget.max_mutations,
            MAX_KERNEL_IR_OPTIMIZATION_MUTATIONS_V1,
        )?;
        validate_limit(
            Some(pass),
            KernelIrOptimizationResourceV1::StorageItems,
            budget.max_storage_items,
            MAX_KERNEL_IR_OPTIMIZATION_STORAGE_ITEMS_V1,
        )?;
    }
    Ok(())
}

fn validate_limit(
    pass: Option<KernelIrOptimizationPassV1>,
    resource: KernelIrOptimizationResourceV1,
    requested: u64,
    hard_maximum: u64,
) -> Result<(), KernelIrOptimizationErrorV1> {
    if requested > hard_maximum {
        Err(KernelIrOptimizationErrorV1::InvalidLimit {
            pass,
            resource,
            requested,
            hard_maximum,
        })
    } else {
        Ok(())
    }
}

fn remove_unreachable_blocks(
    module: &mut Module,
    meter: &mut PassMeter,
) -> Result<(), KernelIrOptimizationErrorV1> {
    for function in &mut module.functions {
        meter.charge_work(1)?;
        let Some(body) = &mut function.body else {
            continue;
        };

        // Count through borrowed terminator fields. No successor vector is
        // materialized until both auxiliary storage and its traversal work
        // have been admitted.
        meter.charge_work(body.blocks.len())?;
        let edge_count = body.blocks.iter().fold(0usize, |count, block| {
            count.saturating_add(terminator_successor_count(
                block
                    .terminator
                    .as_ref()
                    .expect("input was verified before transformation"),
            ))
        });
        let storage_items = body
            .blocks
            .len()
            .saturating_mul(2)
            .saturating_add(edge_count);
        meter.reserve_storage(storage_items)?;

        let mut blocks = BTreeMap::new();
        for (position, block) in body.blocks.iter().enumerate() {
            meter.charge_work(1)?;
            blocks.insert(block.id, position);
        }

        let entry = body.blocks[0].id;
        let mut reachable = BTreeSet::new();
        let mut pending = VecDeque::from([entry]);
        while let Some(block_id) = pending.pop_front() {
            meter.charge_work(1)?;
            if !reachable.insert(block_id) {
                continue;
            }
            let position = blocks[&block_id];
            let terminator = body.blocks[position]
                .terminator
                .as_ref()
                .expect("input was verified before transformation");
            let successor_count = terminator_successor_count(terminator);
            meter.charge_work(successor_count)?;
            let successors = terminator.successors();
            debug_assert_eq!(successors.len(), successor_count);
            for successor in successors {
                if !reachable.contains(&successor) {
                    pending.push_back(successor);
                }
            }
        }

        let removed = body
            .blocks
            .iter()
            .filter(|block| !reachable.contains(&block.id))
            .count();
        meter.charge_work(body.blocks.len())?;
        meter.record_mutations(removed)?;
        if removed != 0 {
            body.blocks.retain(|block| reachable.contains(&block.id));
        }
        meter.release_storage(storage_items);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OperationPosition {
    block: usize,
    operation: usize,
}

fn eliminate_dead_pure_operations(
    module: &mut Module,
    meter: &mut PassMeter,
) -> Result<(), KernelIrOptimizationErrorV1> {
    for function in &mut module.functions {
        meter.charge_work(1)?;
        let Some(body) = &mut function.body else {
            continue;
        };

        let mut operation_count = 0usize;
        let mut result_count = 0usize;
        let mut operand_count = 0usize;
        for block in &body.blocks {
            meter.charge_work(1)?;
            operation_count = operation_count.saturating_add(block.operations.len());
            for operation in &block.operations {
                meter.charge_work(1)?;
                result_count = result_count.saturating_add(operation.results.len());
                operand_count =
                    operand_count.saturating_add(operation_operand_storage_upper_bound(operation));
            }
            let terminator = block
                .terminator
                .as_ref()
                .expect("input was verified before transformation");
            charge_terminator_operand_count_scan(terminator, meter)?;
            operand_count =
                operand_count.saturating_add(terminator_operand_storage_upper_bound(terminator));
        }
        let storage_items = operation_count
            .saturating_add(result_count)
            .saturating_add(operand_count);
        meter.reserve_storage(storage_items)?;

        let mut definitions = BTreeMap::<ValueId, OperationPosition>::new();
        let mut live = body
            .blocks
            .iter()
            .map(|block| vec![false; block.operations.len()])
            .collect::<Vec<_>>();
        let mut pending_values = VecDeque::new();

        for (block_index, block) in body.blocks.iter().enumerate() {
            meter.charge_work(1)?;
            for (operation_index, operation) in block.operations.iter().enumerate() {
                meter.charge_work(1)?;
                for result in &operation.results {
                    meter.charge_work(1)?;
                    definitions.insert(
                        result.id,
                        OperationPosition {
                            block: block_index,
                            operation: operation_index,
                        },
                    );
                }
                if !is_removable_pure_operation(operation) {
                    live[block_index][operation_index] = true;
                    let operand_bound = operation_operand_storage_upper_bound(operation);
                    meter.charge_work(operand_bound)?;
                    let operands = operation.operands();
                    debug_assert!(operands.len() <= operand_bound);
                    pending_values.extend(operands);
                }
            }
            let terminator = block
                .terminator
                .as_ref()
                .expect("input was verified before transformation");
            charge_terminator_operand_count_scan(terminator, meter)?;
            let operand_bound = terminator_operand_storage_upper_bound(terminator);
            meter.charge_work(operand_bound)?;
            let operands = terminator.operands();
            debug_assert!(operands.len() <= operand_bound);
            pending_values.extend(operands);
        }

        while let Some(value) = pending_values.pop_front() {
            meter.charge_work(1)?;
            let Some(position) = definitions.get(&value).copied() else {
                continue;
            };
            if live[position.block][position.operation] {
                continue;
            }
            live[position.block][position.operation] = true;
            let operation = &body.blocks[position.block].operations[position.operation];
            let operand_bound = operation_operand_storage_upper_bound(operation);
            meter.charge_work(operand_bound)?;
            let operands = operation.operands();
            debug_assert!(operands.len() <= operand_bound);
            pending_values.extend(operands);
        }

        let removed = live
            .iter()
            .map(|block| block.iter().filter(|is_live| !**is_live).count())
            .sum::<usize>();
        meter.record_mutations(removed)?;
        if removed != 0 {
            for (block, live_operations) in body.blocks.iter_mut().zip(live) {
                let mut operation_index = 0_usize;
                block.operations.retain(|_| {
                    let retain = live_operations[operation_index];
                    operation_index += 1;
                    retain
                });
            }
        }
        meter.release_storage(storage_items);
    }
    Ok(())
}

fn terminator_successor_count(terminator: &Terminator) -> usize {
    match terminator {
        Terminator::Branch { .. } => 1,
        Terminator::ConditionalBranch { .. } => 2,
        Terminator::Switch { cases, .. } => cases.len().saturating_add(1),
        Terminator::IntegerSwitch { cases, .. } => cases.len().saturating_add(1),
        Terminator::Return { .. } | Terminator::Unreachable => 0,
    }
}

/// Conservative operand-slot count available without materializing an operand
/// vector. Inline assembly counts immediate/output descriptors too; over-count
/// is intentional because this value is a resource-admission upper bound.
fn operation_operand_storage_upper_bound(operation: &Operation) -> usize {
    match &operation.kind {
        OperationKind::Constant(_)
        | OperationKind::Intrinsic(_)
        | OperationKind::Barrier(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::WorkgroupMemory(_)
        | OperationKind::Wave(fe2o3_kernel_ir::WaveOperation {
            kind: WaveOperationKind::LaneId,
            ..
        }) => 0,
        OperationKind::MemoryIntrinsic(intrinsic) => match intrinsic {
            MemoryIntrinsicOperation::PointerDistance { .. }
            | MemoryIntrinsicOperation::VolatileStore { .. } => 2,
            MemoryIntrinsicOperation::VolatileLoad { .. } => 1,
            MemoryIntrinsicOperation::CopyNonOverlapping { .. } => 3,
        },
        OperationKind::Matrix(matrix) => match &matrix.kind {
            MatrixOperationKind::MultiplyAccumulate { .. } => 12,
            MatrixOperationKind::ScaledMultiplyAccumulate { .. } => 20,
            MatrixOperationKind::LdsLoad { .. } => 1,
            MatrixOperationKind::LdsStore { .. } => 5,
        },
        OperationKind::Gfx950LdsTranspose(transpose) => match transpose.kind {
            Gfx950LdsTransposeOperationKindV1::Current { .. } => 0,
            Gfx950LdsTransposeOperationKindV1::Stage { .. } => 8,
            Gfx950LdsTransposeOperationKindV1::Publish { .. }
            | Gfx950LdsTransposeOperationKindV1::Read { .. } => 1,
        },
        OperationKind::Unary { .. }
        | OperationKind::Cast { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::Load { .. } => 1,
        OperationKind::Binary { .. }
        | OperationKind::Compare { .. }
        | OperationKind::GetElementPointer { .. }
        | OperationKind::Store { .. } => 2,
        OperationKind::Select { .. }
        | OperationKind::GuardedLoad { .. }
        | OperationKind::GuardedStore { .. } => 3,
        OperationKind::Call { arguments, .. } => arguments.len(),
        OperationKind::Alloca { count, .. } => usize::from(count.is_some()),
        OperationKind::Atomic(atomic) => 1usize
            .saturating_add(usize::from(atomic.value.is_some()))
            .saturating_add(usize::from(atomic.compare.is_some())),
        OperationKind::Wave(wave) => match wave.kind {
            WaveOperationKind::LaneId => 0,
            WaveOperationKind::Ballot { .. }
            | WaveOperationKind::Any { .. }
            | WaveOperationKind::All { .. }
            | WaveOperationKind::ReduceF32 { .. } => 1,
            WaveOperationKind::ShuffleIndex { .. } | WaveOperationKind::BroadcastF32 { .. } => 2,
        },
        OperationKind::InlineAssembly(assembly) => assembly.operands.len(),
    }
}

fn charge_terminator_operand_count_scan(
    terminator: &Terminator,
    meter: &mut PassMeter,
) -> Result<(), KernelIrOptimizationErrorV1> {
    match terminator {
        Terminator::Switch { cases, .. } => meter.charge_work(cases.len()),
        Terminator::IntegerSwitch { cases, .. } => meter.charge_work(cases.len()),
        _ => Ok(()),
    }
}

fn terminator_operand_storage_upper_bound(terminator: &Terminator) -> usize {
    match terminator {
        Terminator::Branch { arguments, .. } => arguments.len(),
        Terminator::ConditionalBranch {
            then_arguments,
            else_arguments,
            ..
        } => 1usize
            .saturating_add(then_arguments.len())
            .saturating_add(else_arguments.len()),
        Terminator::Switch {
            cases,
            default_arguments,
            ..
        } => cases.iter().fold(
            1usize.saturating_add(default_arguments.len()),
            |count, case| count.saturating_add(case.arguments.len()),
        ),
        Terminator::IntegerSwitch {
            cases,
            default_arguments,
            ..
        } => cases.iter().fold(
            1usize.saturating_add(default_arguments.len()),
            |count, case| count.saturating_add(case.arguments.len()),
        ),
        Terminator::Return { values } => values.len(),
        Terminator::Unreachable => 0,
    }
}

/// Conservative erasure contract for one otherwise-dead operation.
///
/// `ErasableNonTrapping` means the operation has no externally observable
/// effect and is total for every verifier-admitted operand value. Absence of a
/// memory effect alone is intentionally insufficient: KIR overflow, division,
/// shifts, float-to-integer casts, and pointer arithmetic can fail execution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelIrOperationErasabilityV1 {
    ErasableNonTrapping,
    RetainedPotentiallyObservable,
}

pub fn classify_operation_erasability_v1(operation: &Operation) -> KernelIrOperationErasabilityV1 {
    let structurally_total = matches!(
        operation.kind,
        OperationKind::Constant(_)
            | OperationKind::Unary {
                op: UnaryOp::Not,
                ..
            }
            | OperationKind::Binary {
                op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Checked(_),
                ..
            }
            | OperationKind::Compare { .. }
            | OperationKind::Cast {
                kind: CastKind::Truncate
                    | CastKind::ZeroExtend
                    | CastKind::SignExtend
                    | CastKind::Bitcast,
                ..
            }
            | OperationKind::Select { .. }
            | OperationKind::SliceLength { .. }
            | OperationKind::SliceData { .. }
    );
    let erasable = !operation.results.is_empty()
        && structurally_total
        && operation.has_complete_effect_summary()
        && operation.effect_summary().is_pure();
    if erasable {
        KernelIrOperationErasabilityV1::ErasableNonTrapping
    } else {
        KernelIrOperationErasabilityV1::RetainedPotentiallyObservable
    }
}

fn is_removable_pure_operation(operation: &Operation) -> bool {
    classify_operation_erasability_v1(operation)
        == KernelIrOperationErasabilityV1::ErasableNonTrapping
}
