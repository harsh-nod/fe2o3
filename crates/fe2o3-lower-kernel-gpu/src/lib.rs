#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

use std::{
    error::Error,
    fmt,
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

use dialect_gpu::{
    AddressSpaceAttr, BarrierOp, HierarchyAttr, HierarchyIdOp, MemoryOrderAttr, MemoryScopeAttr,
    MemorySpaceOp,
};
use dialect_kernel::AlgorithmOp;
use pliron::{
    context::{Context, Ptr},
    dialect::DialectName,
    irbuild::IRStatus,
    location::Located,
    op::Op,
    operation::{Operation, verify_operation},
    pass::{AnalysisManager, Pass, PassResult},
};

/// Stable Pliron pass name.
pub const PASS_NAME: &str = "fe2o3-lower-kernel-gpu";

/// Context marker used by [`register_pass`].
pub const PASS_REGISTRATION_MARKER_KEY: &str = "fe2o3_lower_kernel_gpu_pass_registration_v1";

/// Largest GPU execution rank supported by this lowering shell.
pub const MAX_GPU_RANK: usize = 3;

/// Largest admitted extent of one abstract workgroup axis.
pub const MAX_WORKGROUP_AXIS: u32 = 1_024;

/// Largest admitted product of active workgroup dimensions.
pub const MAX_WORKGROUP_THREADS: u32 = 1_024;

/// Hard bound on logical regions in one request.
pub const MAX_LOGICAL_REGIONS: u16 = 64;

/// Hard bound on distinct target-neutral memory spaces in one request.
pub const MAX_MEMORY_SPACES: usize = 4;

/// Hard bound on GPU operations emitted by one lowering request.
pub const MAX_REWRITES: u16 = 64;

static NEXT_CONTEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct PassRegistrationMarker {
    context_id: NonZeroU64,
}

/// Result of explicitly registering the pass and its source/target dialects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PassRegistrationOutcome {
    /// All registration hooks completed on this call.
    Registered,
    /// The complete registration had already completed in this context.
    AlreadyRegistered,
}

/// Terminal failure while explicitly registering the lowering pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PassRegistrationError {
    /// A foreign value claimed this crate's context marker.
    MarkerCollision,
    /// The marker map points at absent auxiliary data.
    CorruptMarker,
    /// The process exhausted the private context-identity space.
    ContextIdentityExhausted,
    /// The kernel dialect rejected its explicit registration.
    KernelDialect(dialect_kernel::RegistrationError),
    /// The GPU dialect rejected its explicit registration.
    GpuDialect(dialect_gpu::RegistrationError),
}

impl fmt::Display for PassRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MarkerCollision => {
                formatter.write_str("kernel-to-GPU pass registration marker collision")
            }
            Self::CorruptMarker => {
                formatter.write_str("kernel-to-GPU pass registration marker is corrupt")
            }
            Self::ContextIdentityExhausted => {
                formatter.write_str("kernel-to-GPU context identity space is exhausted")
            }
            Self::KernelDialect(error) => write!(formatter, "kernel dialect registration: {error}"),
            Self::GpuDialect(error) => write!(formatter, "GPU dialect registration: {error}"),
        }
    }
}

impl Error for PassRegistrationError {}

/// Explicitly registers both dialect prerequisites and this pass marker.
///
/// Repeated calls are side-effect free. Marker corruption and collisions are
/// rejected before any dialect registration is attempted.
pub fn register_pass(
    context: &mut Context,
) -> Result<PassRegistrationOutcome, PassRegistrationError> {
    match registration_state(context)? {
        RegistrationState::Registered(_) => {
            return Ok(PassRegistrationOutcome::AlreadyRegistered);
        }
        RegistrationState::Absent => {}
    }

    let kernel_name = DialectName::try_new(dialect_kernel::DIALECT_NAME)
        .expect("static kernel dialect name is valid");
    dialect_kernel::register_dialect(context, &kernel_name)
        .map_err(PassRegistrationError::KernelDialect)?;
    dialect_gpu::register_dialect(context).map_err(PassRegistrationError::GpuDialect)?;

    let marker = context.aux_data.insert(Box::new(PassRegistrationMarker {
        context_id: next_context_id()?,
    }));
    context
        .aux_data_map
        .insert(registration_marker_key(), marker);
    Ok(PassRegistrationOutcome::Registered)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegistrationState {
    Absent,
    Registered(NonZeroU64),
}

fn registration_state(context: &Context) -> Result<RegistrationState, PassRegistrationError> {
    let key = registration_marker_key();
    let Some(index) = context.aux_data_map.get(&key).copied() else {
        return Ok(RegistrationState::Absent);
    };
    match context.aux_data.get(index) {
        Some(marker) => marker
            .downcast_ref::<PassRegistrationMarker>()
            .map(|marker| RegistrationState::Registered(marker.context_id))
            .ok_or(PassRegistrationError::MarkerCollision),
        None => Err(PassRegistrationError::CorruptMarker),
    }
}

fn next_context_id() -> Result<NonZeroU64, PassRegistrationError> {
    let value = NEXT_CONTEXT_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| PassRegistrationError::ContextIdentityExhausted)?;
    NonZeroU64::new(value).ok_or(PassRegistrationError::ContextIdentityExhausted)
}

fn registration_marker_key() -> pliron::identifier::Identifier {
    PASS_REGISTRATION_MARKER_KEY
        .try_into()
        .expect("static pass registration key is valid")
}

/// Bounded abstract dimensions for one workgroup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkgroupShape {
    rank: u8,
    dimensions: [u32; MAX_GPU_RANK],
    threads: u32,
}

impl WorkgroupShape {
    /// Creates a workgroup shape with one to three non-zero dimensions.
    pub fn new(dimensions: &[u32]) -> Result<Self, ConfigError> {
        if dimensions.is_empty() || dimensions.len() > MAX_GPU_RANK {
            return Err(ConfigError::RankOutOfBounds(dimensions.len()));
        }

        let mut bounded = [1; MAX_GPU_RANK];
        let mut threads = 1_u32;
        for (axis, extent) in dimensions.iter().copied().enumerate() {
            if extent == 0 || extent > MAX_WORKGROUP_AXIS {
                return Err(ConfigError::WorkgroupAxisOutOfBounds { axis, extent });
            }
            threads = threads
                .checked_mul(extent)
                .ok_or(ConfigError::WorkgroupTooLarge(u64::MAX))?;
            bounded[axis] = extent;
        }
        if threads > MAX_WORKGROUP_THREADS {
            return Err(ConfigError::WorkgroupTooLarge(u64::from(threads)));
        }

        Ok(Self {
            rank: dimensions.len() as u8,
            dimensions: bounded,
            threads,
        })
    }

    /// Returns the number of active workgroup dimensions.
    pub const fn rank(self) -> u8 {
        self.rank
    }

    /// Returns all three dimensions, with inactive dimensions set to one.
    pub const fn dimensions(self) -> [u32; MAX_GPU_RANK] {
        self.dimensions
    }

    /// Returns the bounded product of active dimensions.
    pub const fn threads(self) -> u32 {
        self.threads
    }
}

/// Synchronization semantics requested from this shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SynchronizationMode {
    /// Emit no synchronization operation.
    None,
    /// Emit one workgroup-memory barrier for each logical region.
    WorkgroupBarrier,
}

/// Invalid bounded lowering configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    /// Workgroup rank was zero or exceeded [`MAX_GPU_RANK`].
    RankOutOfBounds(usize),
    /// One workgroup axis was zero or exceeded [`MAX_WORKGROUP_AXIS`].
    WorkgroupAxisOutOfBounds {
        /// Zero-based axis in the input shape.
        axis: usize,
        /// Rejected extent.
        extent: u32,
    },
    /// The workgroup dimension product exceeded [`MAX_WORKGROUP_THREADS`].
    WorkgroupTooLarge(u64),
    /// Logical region count was zero or exceeded [`MAX_LOGICAL_REGIONS`].
    RegionCountOutOfBounds(u16),
    /// The memory-space list was empty or exceeded [`MAX_MEMORY_SPACES`].
    MemorySpaceCountOutOfBounds(usize),
    /// A memory space occurred more than once.
    DuplicateMemorySpace(AddressSpaceAttr),
    /// Workgroup synchronization was requested without workgroup memory.
    WorkgroupBarrierWithoutWorkgroupMemory,
    /// Rewrite limit was zero or exceeded [`MAX_REWRITES`].
    RewriteLimitOutOfBounds(u16),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RankOutOfBounds(rank) => write!(
                formatter,
                "workgroup rank {rank} is outside 1..={MAX_GPU_RANK}"
            ),
            Self::WorkgroupAxisOutOfBounds { axis, extent } => write!(
                formatter,
                "workgroup axis {axis} extent {extent} is outside 1..={MAX_WORKGROUP_AXIS}"
            ),
            Self::WorkgroupTooLarge(threads) => write!(
                formatter,
                "workgroup size {threads} exceeds {MAX_WORKGROUP_THREADS}"
            ),
            Self::RegionCountOutOfBounds(count) => write!(
                formatter,
                "logical region count {count} is outside 1..={MAX_LOGICAL_REGIONS}"
            ),
            Self::MemorySpaceCountOutOfBounds(count) => write!(
                formatter,
                "memory-space count {count} is outside 1..={MAX_MEMORY_SPACES}"
            ),
            Self::DuplicateMemorySpace(space) => {
                write!(formatter, "duplicate memory space {space:?}")
            }
            Self::WorkgroupBarrierWithoutWorkgroupMemory => {
                formatter.write_str("workgroup barrier requires the workgroup memory space")
            }
            Self::RewriteLimitOutOfBounds(limit) => write!(
                formatter,
                "rewrite limit {limit} is outside 1..={MAX_REWRITES}"
            ),
        }
    }
}

impl Error for ConfigError {}

/// Immutable, bounded configuration for one lowering invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweringConfig {
    workgroup: WorkgroupShape,
    logical_regions: u16,
    memory_spaces: Vec<AddressSpaceAttr>,
    synchronization: SynchronizationMode,
    rewrite_limit: u16,
}

impl LoweringConfig {
    /// Validates and canonicalizes a target-neutral lowering request.
    pub fn new(
        workgroup: WorkgroupShape,
        logical_regions: u16,
        memory_spaces: &[AddressSpaceAttr],
        synchronization: SynchronizationMode,
        rewrite_limit: u16,
    ) -> Result<Self, ConfigError> {
        if logical_regions == 0 || logical_regions > MAX_LOGICAL_REGIONS {
            return Err(ConfigError::RegionCountOutOfBounds(logical_regions));
        }
        if memory_spaces.is_empty() || memory_spaces.len() > MAX_MEMORY_SPACES {
            return Err(ConfigError::MemorySpaceCountOutOfBounds(
                memory_spaces.len(),
            ));
        }
        if rewrite_limit == 0 || rewrite_limit > MAX_REWRITES {
            return Err(ConfigError::RewriteLimitOutOfBounds(rewrite_limit));
        }

        let mut memory_spaces = memory_spaces.to_vec();
        memory_spaces.sort_by_key(|space| memory_space_order(*space));
        if let Some(duplicate) = memory_spaces
            .windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0])
        {
            return Err(ConfigError::DuplicateMemorySpace(duplicate));
        }
        if synchronization == SynchronizationMode::WorkgroupBarrier
            && !memory_spaces.contains(&AddressSpaceAttr::Workgroup)
        {
            return Err(ConfigError::WorkgroupBarrierWithoutWorkgroupMemory);
        }

        Ok(Self {
            workgroup,
            logical_regions,
            memory_spaces,
            synchronization,
            rewrite_limit,
        })
    }

    /// Returns the abstract workgroup shape.
    pub const fn workgroup(&self) -> WorkgroupShape {
        self.workgroup
    }

    /// Returns the bounded logical region count.
    pub const fn logical_regions(&self) -> u16 {
        self.logical_regions
    }

    /// Returns memory spaces in canonical target-neutral order.
    pub fn memory_spaces(&self) -> &[AddressSpaceAttr] {
        &self.memory_spaces
    }

    /// Returns the requested synchronization mode.
    pub const fn synchronization(&self) -> SynchronizationMode {
        self.synchronization
    }

    /// Returns the maximum number of emitted GPU operations.
    pub const fn rewrite_limit(&self) -> u16 {
        self.rewrite_limit
    }
}

const fn memory_space_order(space: AddressSpaceAttr) -> u8 {
    match space {
        AddressSpaceAttr::Private => 0,
        AddressSpaceAttr::Workgroup => 1,
        AddressSpaceAttr::Global => 2,
        AddressSpaceAttr::Constant => 3,
    }
}

/// Deterministic semantic step emitted by the lowering shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoweringStep {
    /// Materialize one abstract execution-hierarchy coordinate.
    Hierarchy(HierarchyAttr),
    /// Materialize one canonical target-neutral memory space.
    MemorySpace(AddressSpaceAttr),
    /// Synchronize workgroup memory after one logical region.
    WorkgroupBarrier {
        /// Zero-based logical region index.
        region: u16,
    },
}

/// Pointer-independent deterministic record of one successful lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweringRecord {
    source_rank: u32,
    workgroup: WorkgroupShape,
    logical_regions: u16,
    memory_spaces: Vec<AddressSpaceAttr>,
    steps: Vec<LoweringStep>,
}

impl LoweringRecord {
    /// Returns the verified source algorithm rank.
    pub const fn source_rank(&self) -> u32 {
        self.source_rank
    }

    /// Returns the abstract target-neutral workgroup shape.
    pub const fn workgroup(&self) -> WorkgroupShape {
        self.workgroup
    }

    /// Returns the logical source-region count.
    pub const fn logical_regions(&self) -> u16 {
        self.logical_regions
    }

    /// Returns canonical memory spaces.
    pub fn memory_spaces(&self) -> &[AddressSpaceAttr] {
        &self.memory_spaces
    }

    /// Returns emitted semantic steps in stable order.
    pub fn steps(&self) -> &[LoweringStep] {
        &self.steps
    }

    /// Returns the exact number of GPU operations emitted.
    pub fn rewrite_count(&self) -> u16 {
        self.steps
            .len()
            .try_into()
            .expect("bounded lowering step count fits u16")
    }
}

/// Successful bounded transformation output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweringResult {
    record: LoweringRecord,
    operations: Vec<Ptr<Operation>>,
    context_id: NonZeroU64,
}

impl LoweringResult {
    /// Returns the pointer-independent deterministic semantic record.
    pub const fn record(&self) -> &LoweringRecord {
        &self.record
    }

    /// Returns unlinked Pliron roots for the emitted `gpu.*` operations.
    pub fn operations(&self) -> &[Ptr<Operation>] {
        &self.operations
    }

    /// This representation never grants proof or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Revalidates all bounded structural and typed postconditions.
    pub fn validate(&self, context: &Context) -> Result<(), PostconditionError> {
        validate_postconditions(context, self)
    }
}

/// A failed output invariant, indicating malformed or externally mutated IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostconditionError {
    /// The supplied context does not own this result's arena pointers.
    ContextMismatch,
    /// Semantic-step and operation counts differ.
    OperationCountMismatch,
    /// The result escaped the hard rewrite bound.
    RewriteBoundExceeded,
    /// An emitted operation failed its dialect verifier.
    InvalidGpuOperation {
        /// Zero-based emitted operation index.
        index: usize,
    },
    /// An operation kind disagreed with its deterministic semantic step.
    UnexpectedGpuOperation {
        /// Zero-based emitted operation index.
        index: usize,
    },
}

impl fmt::Display for PostconditionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextMismatch => {
                formatter.write_str("lowering result belongs to a different Pliron context")
            }
            Self::OperationCountMismatch => {
                formatter.write_str("lowering step and GPU operation counts differ")
            }
            Self::RewriteBoundExceeded => {
                formatter.write_str("lowering result exceeds the hard rewrite bound")
            }
            Self::InvalidGpuOperation { index } => {
                write!(
                    formatter,
                    "emitted GPU operation {index} failed verification"
                )
            }
            Self::UnexpectedGpuOperation { index } => write!(
                formatter,
                "emitted GPU operation {index} does not match its lowering step"
            ),
        }
    }
}

impl Error for PostconditionError {}

/// Terminal checked-lowering failure. No variant permits fallback execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoweringError {
    /// [`register_pass`] did not complete in this context.
    PassNotRegistered,
    /// The context registration marker was foreign or corrupt.
    RegistrationCorrupt,
    /// The source operation is not `kernel.algorithm_root`.
    UnsupportedSourceOperation,
    /// The source operation failed its kernel-dialect verifier.
    SourceVerificationFailed,
    /// The source rank exceeds the currently supported GPU rank.
    UnsupportedSourceRank(u32),
    /// Source rank and configured workgroup rank disagree.
    RankMismatch {
        /// Verified source rank.
        source: u32,
        /// Configured workgroup rank.
        workgroup: u8,
    },
    /// The bounded region count is valid but not implemented by this shell.
    UnsupportedRegionCount(u16),
    /// Required deterministic rewrites exceed the request limit.
    RewriteLimitExceeded {
        /// Number of rewrites required by the deterministic plan.
        required: u16,
        /// Caller-provided rewrite limit.
        limit: u16,
    },
    /// Emitted target-neutral GPU IR failed postcondition validation.
    Postcondition(PostconditionError),
}

impl fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PassNotRegistered => formatter.write_str("lowering pass is not registered"),
            Self::RegistrationCorrupt => {
                formatter.write_str("lowering pass registration is corrupt")
            }
            Self::UnsupportedSourceOperation => {
                formatter.write_str("source operation is not kernel.algorithm_root")
            }
            Self::SourceVerificationFailed => {
                formatter.write_str("source kernel operation failed verification")
            }
            Self::UnsupportedSourceRank(rank) => {
                write!(
                    formatter,
                    "source rank {rank} exceeds supported GPU rank {MAX_GPU_RANK}"
                )
            }
            Self::RankMismatch { source, workgroup } => write!(
                formatter,
                "source rank {source} differs from workgroup rank {workgroup}"
            ),
            Self::UnsupportedRegionCount(count) => write!(
                formatter,
                "logical region count {count} is not implemented; only one is supported"
            ),
            Self::RewriteLimitExceeded { required, limit } => write!(
                formatter,
                "lowering requires {required} rewrites but the limit is {limit}"
            ),
            Self::Postcondition(error) => write!(formatter, "lowering postcondition: {error}"),
        }
    }
}

impl Error for LoweringError {}

/// Bounded target-neutral kernel-to-GPU Pliron transformation pass.
#[derive(Clone, Debug)]
pub struct KernelGpuLoweringPass {
    config: LoweringConfig,
    last_result: Option<LoweringResult>,
}

impl KernelGpuLoweringPass {
    /// Creates a pass with an already validated immutable configuration.
    pub const fn new(config: LoweringConfig) -> Self {
        Self {
            config,
            last_result: None,
        }
    }

    /// Returns this pass's immutable bounded configuration.
    pub const fn config(&self) -> &LoweringConfig {
        &self.config
    }

    /// Returns the most recent successful structured result.
    pub const fn last_result(&self) -> Option<&LoweringResult> {
        self.last_result.as_ref()
    }

    /// Takes ownership of the most recent successful structured result.
    pub fn take_result(&mut self) -> Option<LoweringResult> {
        self.last_result.take()
    }

    /// Runs all preconditions, transformation steps, and postconditions.
    ///
    /// Failure is terminal and clears any prior result. This method never
    /// invokes another lowering path.
    pub fn run_checked(
        &mut self,
        source: Ptr<Operation>,
        context: &mut Context,
    ) -> Result<&LoweringResult, LoweringError> {
        self.last_result = None;
        let (source_rank, context_id) = validate_preconditions(context, source, &self.config)?;
        let steps = build_steps(&self.config)?;
        let operations = materialize_steps(context, &steps);
        let result = LoweringResult {
            record: LoweringRecord {
                source_rank,
                workgroup: self.config.workgroup,
                logical_regions: self.config.logical_regions,
                memory_spaces: self.config.memory_spaces.clone(),
                steps,
            },
            operations,
            context_id,
        };
        result
            .validate(context)
            .map_err(LoweringError::Postcondition)?;
        self.last_result = Some(result);
        Ok(self
            .last_result
            .as_ref()
            .expect("successful lowering stores a result"))
    }
}

impl Pass for KernelGpuLoweringPass {
    fn name(&self) -> &str {
        PASS_NAME
    }

    fn run(
        &mut self,
        source: Ptr<Operation>,
        context: &mut Context,
        _analyses: &mut AnalysisManager,
    ) -> pliron::result::Result<PassResult> {
        let location = source.deref(context).loc();
        self.run_checked(source, context)
            .map_err(|error| pliron::verify_error!(location.clone(), "{error}"))?;
        let mut result = PassResult::default();
        result.ir_changed = IRStatus::Unchanged;
        Ok(result)
    }
}

fn validate_preconditions(
    context: &Context,
    source: Ptr<Operation>,
    config: &LoweringConfig,
) -> Result<(u32, NonZeroU64), LoweringError> {
    let context_id = match registration_state(context) {
        Ok(RegistrationState::Registered(context_id)) => context_id,
        Ok(RegistrationState::Absent) => return Err(LoweringError::PassNotRegistered),
        Err(_) => return Err(LoweringError::RegistrationCorrupt),
    };
    if !Operation::is_op::<AlgorithmOp>(source, context) {
        return Err(LoweringError::UnsupportedSourceOperation);
    }
    verify_operation(source, context).map_err(|_| LoweringError::SourceVerificationFailed)?;

    let source = AlgorithmOp::from_operation(source);
    let source_rank = source
        .iteration_domain(context)
        .ok_or(LoweringError::SourceVerificationFailed)?
        .rank();
    if usize::try_from(source_rank).map_or(true, |rank| rank > MAX_GPU_RANK) {
        return Err(LoweringError::UnsupportedSourceRank(source_rank));
    }
    if source_rank != u32::from(config.workgroup.rank) {
        return Err(LoweringError::RankMismatch {
            source: source_rank,
            workgroup: config.workgroup.rank,
        });
    }
    if config.logical_regions != 1 {
        return Err(LoweringError::UnsupportedRegionCount(
            config.logical_regions,
        ));
    }
    Ok((source_rank, context_id))
}

fn build_steps(config: &LoweringConfig) -> Result<Vec<LoweringStep>, LoweringError> {
    let barrier_count = match config.synchronization {
        SynchronizationMode::None => 0,
        SynchronizationMode::WorkgroupBarrier => config.logical_regions,
    };
    let required = 3_usize
        .checked_add(config.memory_spaces.len())
        .and_then(|count| count.checked_add(usize::from(barrier_count)))
        .and_then(|count| u16::try_from(count).ok())
        .expect("hard configuration bounds keep rewrite count in u16");
    if required > config.rewrite_limit {
        return Err(LoweringError::RewriteLimitExceeded {
            required,
            limit: config.rewrite_limit,
        });
    }

    let mut steps = Vec::with_capacity(usize::from(required));
    steps.push(LoweringStep::Hierarchy(HierarchyAttr::Grid));
    steps.push(LoweringStep::Hierarchy(HierarchyAttr::Workgroup));
    steps.push(LoweringStep::Hierarchy(HierarchyAttr::Lane));
    steps.extend(
        config
            .memory_spaces
            .iter()
            .copied()
            .map(LoweringStep::MemorySpace),
    );
    if config.synchronization == SynchronizationMode::WorkgroupBarrier {
        steps.extend(
            (0..config.logical_regions).map(|region| LoweringStep::WorkgroupBarrier { region }),
        );
    }
    Ok(steps)
}

fn materialize_steps(context: &mut Context, steps: &[LoweringStep]) -> Vec<Ptr<Operation>> {
    steps
        .iter()
        .map(|step| match *step {
            LoweringStep::Hierarchy(hierarchy) => {
                HierarchyIdOp::new(context, hierarchy).get_operation()
            }
            LoweringStep::MemorySpace(space) => MemorySpaceOp::new(context, space).get_operation(),
            LoweringStep::WorkgroupBarrier { .. } => BarrierOp::new(
                context,
                HierarchyAttr::Workgroup,
                MemoryScopeAttr::Workgroup,
                AddressSpaceAttr::Workgroup,
                MemoryOrderAttr::AcquireRelease,
            )
            .get_operation(),
        })
        .collect()
}

fn validate_postconditions(
    context: &Context,
    result: &LoweringResult,
) -> Result<(), PostconditionError> {
    match registration_state(context) {
        Ok(RegistrationState::Registered(context_id)) if context_id == result.context_id => {}
        _ => return Err(PostconditionError::ContextMismatch),
    }
    if result.record.steps.len() != result.operations.len() {
        return Err(PostconditionError::OperationCountMismatch);
    }
    if result.operations.len() > usize::from(MAX_REWRITES) {
        return Err(PostconditionError::RewriteBoundExceeded);
    }

    for (index, (step, operation)) in result
        .record
        .steps
        .iter()
        .zip(&result.operations)
        .enumerate()
    {
        verify_operation(*operation, context)
            .map_err(|_| PostconditionError::InvalidGpuOperation { index })?;
        if !operation_matches_step(context, *operation, step) {
            return Err(PostconditionError::UnexpectedGpuOperation { index });
        }
    }
    Ok(())
}

fn operation_matches_step(
    context: &Context,
    operation: Ptr<Operation>,
    step: &LoweringStep,
) -> bool {
    match step {
        LoweringStep::Hierarchy(expected) => {
            Operation::get_op::<HierarchyIdOp>(operation, context).and_then(|op| {
                op.get_attr_gpu_hierarchy_id_hierarchy(context)
                    .map(|actual| *actual)
            }) == Some(*expected)
        }
        LoweringStep::MemorySpace(expected) => {
            Operation::get_op::<MemorySpaceOp>(operation, context).and_then(|op| {
                op.get_attr_gpu_memory_space_address_space(context)
                    .map(|actual| *actual)
            }) == Some(*expected)
        }
        LoweringStep::WorkgroupBarrier { .. } => {
            let Some(op) = Operation::get_op::<BarrierOp>(operation, context) else {
                return false;
            };
            op.get_attr_gpu_barrier_execution_scope(context)
                .map(|value| *value)
                == Some(HierarchyAttr::Workgroup)
                && op
                    .get_attr_gpu_barrier_memory_scope(context)
                    .map(|value| *value)
                    == Some(MemoryScopeAttr::Workgroup)
                && op
                    .get_attr_gpu_barrier_address_space(context)
                    .map(|value| *value)
                    == Some(AddressSpaceAttr::Workgroup)
                && op.get_attr_gpu_barrier_order(context).map(|value| *value)
                    == Some(MemoryOrderAttr::AcquireRelease)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(spaces: &[AddressSpaceAttr]) -> LoweringConfig {
        LoweringConfig::new(
            WorkgroupShape::new(&[16, 16]).expect("valid shape"),
            1,
            spaces,
            SynchronizationMode::WorkgroupBarrier,
            16,
        )
        .expect("valid config")
    }

    fn lower(spaces: &[AddressSpaceAttr]) -> (Context, LoweringResult) {
        let mut context = Context::new();
        register_pass(&mut context).expect("registration succeeds");
        let source = AlgorithmOp::new(&mut context, 2).expect("valid source");
        let mut pass = KernelGpuLoweringPass::new(config(spaces));
        pass.run_checked(source.get_operation(), &mut context)
            .expect("lowering succeeds");
        let result = pass.take_result().expect("result exists");
        (context, result)
    }

    #[test]
    fn lowers_to_verified_bounded_gpu_operations() {
        let (context, result) = lower(&[AddressSpaceAttr::Global, AddressSpaceAttr::Workgroup]);

        assert_eq!(result.record().source_rank(), 2);
        assert_eq!(result.record().workgroup().dimensions(), [16, 16, 1]);
        assert_eq!(result.record().rewrite_count(), 6);
        assert_eq!(result.operations().len(), 6);
        assert!(!result.grants_authority());
        result.validate(&context).expect("postconditions hold");
    }

    #[test]
    fn canonical_memory_order_makes_records_deterministic() {
        let (_, left) = lower(&[AddressSpaceAttr::Global, AddressSpaceAttr::Workgroup]);
        let (_, right) = lower(&[AddressSpaceAttr::Workgroup, AddressSpaceAttr::Global]);

        assert_eq!(left.record(), right.record());
    }

    #[test]
    fn registration_is_explicit_and_idempotent() {
        let mut context = Context::new();
        assert_eq!(
            register_pass(&mut context),
            Ok(PassRegistrationOutcome::Registered)
        );
        assert_eq!(
            register_pass(&mut context),
            Ok(PassRegistrationOutcome::AlreadyRegistered)
        );
    }

    #[test]
    fn pliron_pass_adapter_reports_detached_output_without_ir_change() {
        let mut context = Context::new();
        register_pass(&mut context).expect("registration succeeds");
        let source = AlgorithmOp::new(&mut context, 2).expect("valid source");
        let mut pass = KernelGpuLoweringPass::new(config(&[AddressSpaceAttr::Workgroup]));
        let mut analyses = AnalysisManager::default();

        let pass_result = Pass::run(
            &mut pass,
            source.get_operation(),
            &mut context,
            &mut analyses,
        )
        .expect("pass succeeds");

        assert_eq!(pass_result.ir_changed, IRStatus::Unchanged);
        assert!(pass.last_result().is_some());
    }
}
