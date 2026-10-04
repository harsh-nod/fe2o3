use core::fmt;

/// Maximum number of words in one modeled memory object.
pub const MAX_SEMANTIC_WORDS: usize = 32;
/// Maximum number of fields or variants in one layout case.
pub const MAX_LAYOUT_MEMBERS: usize = 8;
/// Maximum number of explicit integer switch arms.
pub const MAX_SWITCH_ARMS: usize = 16;
/// Maximum number of atomic operations in one modeled execution.
pub const MAX_ATOMIC_STEPS: usize = 32;
/// Maximum number of accesses in one safety-obligation case.
pub const MAX_OBLIGATION_ACCESSES: usize = 32;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticFeature {
    PointerDistance,
    VolatileMemory,
    CopyNonoverlapping,
    RustLayout,
    IntegerSwitch,
    AtomicScopes,
    BoundsAndRaces,
}

impl SemanticFeature {
    pub const ALL: [Self; 7] = [
        Self::PointerDistance,
        Self::VolatileMemory,
        Self::CopyNonoverlapping,
        Self::RustLayout,
        Self::IntegerSwitch,
        Self::AtomicScopes,
        Self::BoundsAndRaces,
    ];
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticCase {
    seed: u64,
    ordinal: u16,
    feature: SemanticFeature,
    specification: SemanticSpec,
}

impl SemanticCase {
    pub fn new(
        seed: u64,
        ordinal: u16,
        feature: SemanticFeature,
        specification: SemanticSpec,
    ) -> Result<Self, SemanticModelError> {
        let case = Self {
            seed,
            ordinal,
            feature,
            specification,
        };
        case.validate()?;
        Ok(case)
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn ordinal(&self) -> u16 {
        self.ordinal
    }

    pub fn feature(&self) -> SemanticFeature {
        self.feature
    }

    pub fn specification(&self) -> &SemanticSpec {
        &self.specification
    }

    pub fn validate(&self) -> Result<(), SemanticModelError> {
        if self.feature != self.specification.feature() {
            return Err(SemanticModelError::FeatureMismatch {
                declared: self.feature,
                actual: self.specification.feature(),
            });
        }
        self.specification.validate()
    }

    pub(crate) fn rebuild(&self, specification: SemanticSpec) -> Option<Self> {
        Self::new(self.seed, self.ordinal, self.feature, specification).ok()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticSpec {
    PointerDistance(PointerDistanceSpec),
    Volatile(VolatileSpec),
    CopyNonoverlapping(CopyNonoverlappingSpec),
    Layout(LayoutSpec),
    IntegerSwitch(IntegerSwitchSpec),
    Atomics(AtomicSpec),
    Obligation(ObligationSpec),
}

impl SemanticSpec {
    pub const fn feature(&self) -> SemanticFeature {
        match self {
            Self::PointerDistance(_) => SemanticFeature::PointerDistance,
            Self::Volatile(_) => SemanticFeature::VolatileMemory,
            Self::CopyNonoverlapping(_) => SemanticFeature::CopyNonoverlapping,
            Self::Layout(_) => SemanticFeature::RustLayout,
            Self::IntegerSwitch(_) => SemanticFeature::IntegerSwitch,
            Self::Atomics(_) => SemanticFeature::AtomicScopes,
            Self::Obligation(_) => SemanticFeature::BoundsAndRaces,
        }
    }

    fn validate(&self) -> Result<(), SemanticModelError> {
        match self {
            Self::PointerDistance(specification) => specification.validate(),
            Self::Volatile(specification) => specification.validate(),
            Self::CopyNonoverlapping(specification) => specification.validate(),
            Self::Layout(specification) => specification.validate(),
            Self::IntegerSwitch(specification) => specification.validate(),
            Self::Atomics(specification) => specification.validate(),
            Self::Obligation(specification) => specification.validate(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointerDistanceSpec {
    pub allocation_bytes: u16,
    pub from_offset: u16,
    pub to_offset: u16,
    pub element_bytes: u8,
    pub same_allocation: bool,
    pub signed: bool,
}

impl PointerDistanceSpec {
    fn validate(&self) -> Result<(), SemanticModelError> {
        if self.allocation_bytes == 0 || usize::from(self.allocation_bytes) > MAX_SEMANTIC_WORDS * 8
        {
            return Err(SemanticModelError::InvalidAllocationBytes {
                actual: usize::from(self.allocation_bytes),
            });
        }
        if self.element_bytes == 0 || self.element_bytes > 16 {
            return Err(SemanticModelError::InvalidElementBytes {
                actual: self.element_bytes,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VolatileOperation {
    Load,
    Store(i32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolatileSpec {
    pub words: Vec<i32>,
    pub index: u16,
    pub byte_alignment: u8,
    pub readable: bool,
    pub writable: bool,
    pub operation: VolatileOperation,
}

impl VolatileSpec {
    fn validate(&self) -> Result<(), SemanticModelError> {
        validate_words(&self.words)?;
        if self.byte_alignment == 0 || !self.byte_alignment.is_power_of_two() {
            return Err(SemanticModelError::InvalidAlignment {
                actual: self.byte_alignment,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyNonoverlappingSpec {
    pub words: Vec<i32>,
    pub source: u16,
    pub destination: u16,
    pub count: u16,
}

impl CopyNonoverlappingSpec {
    fn validate(&self) -> Result<(), SemanticModelError> {
        validate_words(&self.words)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarLayout {
    pub size: u8,
    pub alignment: u8,
}

impl ScalarLayout {
    fn validate(self) -> Result<(), SemanticModelError> {
        if self.size == 0 || self.size > 16 {
            return Err(SemanticModelError::InvalidLayoutSize { actual: self.size });
        }
        if self.alignment == 0 || !self.alignment.is_power_of_two() || self.alignment > 16 {
            return Err(SemanticModelError::InvalidAlignment {
                actual: self.alignment,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayoutSpec {
    Aggregate {
        fields: Vec<ScalarLayout>,
    },
    TaggedEnum {
        tag: ScalarLayout,
        payloads: Vec<ScalarLayout>,
    },
    /// Niche selection is intentionally outside this corpus's supported layout contract.
    NicheEnum {
        payload: ScalarLayout,
    },
}

impl LayoutSpec {
    fn validate(&self) -> Result<(), SemanticModelError> {
        match self {
            Self::Aggregate { fields } => validate_layout_members(fields),
            Self::TaggedEnum { tag, payloads } => {
                tag.validate()?;
                validate_layout_members(payloads)
            }
            Self::NicheEnum { payload } => payload.validate(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerSwitchSpec {
    pub selector: i32,
    pub arms: Vec<(i32, i32)>,
    pub default: i32,
}

impl IntegerSwitchSpec {
    fn validate(&self) -> Result<(), SemanticModelError> {
        if self.arms.len() > MAX_SWITCH_ARMS {
            return Err(SemanticModelError::TooManySwitchArms {
                actual: self.arms.len(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicScope {
    Workgroup,
    Device,
    System,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryOrdering {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicOperation {
    Load {
        ordering: MemoryOrdering,
    },
    Store {
        value: i32,
        ordering: MemoryOrdering,
    },
    FetchAdd {
        value: i32,
        ordering: MemoryOrdering,
    },
    CompareExchange {
        current: i32,
        new: i32,
        success: MemoryOrdering,
        failure: MemoryOrdering,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AtomicSpec {
    pub initial: i32,
    pub scope: AtomicScope,
    pub operations: Vec<AtomicOperation>,
}

impl AtomicSpec {
    fn validate(&self) -> Result<(), SemanticModelError> {
        if self.operations.is_empty() || self.operations.len() > MAX_ATOMIC_STEPS {
            return Err(SemanticModelError::InvalidAtomicStepCount {
                actual: self.operations.len(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessKind {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryAccess {
    pub lane: u16,
    pub index: u16,
    pub kind: AccessKind,
    pub atomic: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObligationSpec {
    Bounds {
        length: u16,
        index: u16,
    },
    Race {
        allocation_words: u16,
        accesses: Vec<MemoryAccess>,
    },
}

impl ObligationSpec {
    fn validate(&self) -> Result<(), SemanticModelError> {
        match self {
            Self::Bounds { length, .. } => {
                if *length == 0 || usize::from(*length) > MAX_SEMANTIC_WORDS {
                    return Err(SemanticModelError::InvalidWordCount {
                        actual: usize::from(*length),
                    });
                }
            }
            Self::Race {
                allocation_words,
                accesses,
            } => {
                if *allocation_words == 0 || usize::from(*allocation_words) > MAX_SEMANTIC_WORDS {
                    return Err(SemanticModelError::InvalidWordCount {
                        actual: usize::from(*allocation_words),
                    });
                }
                if accesses.is_empty() || accesses.len() > MAX_OBLIGATION_ACCESSES {
                    return Err(SemanticModelError::InvalidAccessCount {
                        actual: accesses.len(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticObservation {
    Scalar(i64),
    Words(Vec<i32>),
    Layout {
        size: u16,
        alignment: u8,
        offsets: Vec<u16>,
    },
    Switch {
        arm: Option<u8>,
        value: i32,
    },
    Atomic {
        observed: Vec<i32>,
        final_value: i32,
    },
    ObligationsSatisfied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileRejection {
    PointerProvenance,
    PointerOutOfBounds,
    PointerMisaligned,
    UnsignedPointerUnderflow,
    VolatileOutOfBounds,
    VolatilePermission,
    VolatileMisaligned,
    CopyOutOfBounds,
    CopyOverlap,
    UnsupportedNicheLayout,
    DuplicateSwitchValue,
    UnsupportedAtomicScope,
    InvalidAtomicOrdering,
    BoundsObligation,
    RaceObligation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReferenceOutcome {
    Execution(SemanticObservation),
    CompileRejection(CompileRejection),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HardwareUnavailable {
    NoCompatibleDevice,
    DriverUnavailable,
    TargetUnsupported,
    DevicePermissionDenied,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendOutcome {
    Execution(SemanticObservation),
    CompileRejection(CompileRejection),
    HardwareUnavailable(HardwareUnavailable),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticMismatch {
    pub expected: ReferenceOutcome,
    pub observed: BackendOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConformanceOutcome {
    SupportedPass,
    ExpectedCompileRejection,
    SemanticMismatch(SemanticMismatch),
    HardwareUnavailable(HardwareUnavailable),
}

/// Evaluates the exact CPU reference contract for one semantic case.
pub fn evaluate_semantic_case(case: &SemanticCase) -> ReferenceOutcome {
    case.validate()
        .expect("SemanticCase invariants are retained");
    match case.specification() {
        SemanticSpec::PointerDistance(specification) => pointer_distance(specification),
        SemanticSpec::Volatile(specification) => volatile(specification),
        SemanticSpec::CopyNonoverlapping(specification) => copy_nonoverlapping(specification),
        SemanticSpec::Layout(specification) => layout(specification),
        SemanticSpec::IntegerSwitch(specification) => integer_switch(specification),
        SemanticSpec::Atomics(specification) => atomics(specification),
        SemanticSpec::Obligation(specification) => obligation(specification),
    }
}

/// Compares a backend report against the CPU reference contract.
pub fn classify_semantic_outcome(
    case: &SemanticCase,
    observed: BackendOutcome,
) -> ConformanceOutcome {
    let expected = evaluate_semantic_case(case);
    match (&expected, &observed) {
        (ReferenceOutcome::Execution(expected), BackendOutcome::Execution(actual))
            if expected == actual =>
        {
            ConformanceOutcome::SupportedPass
        }
        (
            ReferenceOutcome::CompileRejection(expected),
            BackendOutcome::CompileRejection(actual),
        ) if expected == actual => ConformanceOutcome::ExpectedCompileRejection,
        (_, BackendOutcome::HardwareUnavailable(reason)) => {
            ConformanceOutcome::HardwareUnavailable(*reason)
        }
        _ => ConformanceOutcome::SemanticMismatch(SemanticMismatch { expected, observed }),
    }
}

fn pointer_distance(specification: &PointerDistanceSpec) -> ReferenceOutcome {
    if !specification.same_allocation {
        return rejection(CompileRejection::PointerProvenance);
    }
    if specification.from_offset > specification.allocation_bytes
        || specification.to_offset > specification.allocation_bytes
    {
        return rejection(CompileRejection::PointerOutOfBounds);
    }
    let element_bytes = u16::from(specification.element_bytes);
    if !specification.from_offset.is_multiple_of(element_bytes)
        || !specification.to_offset.is_multiple_of(element_bytes)
    {
        return rejection(CompileRejection::PointerMisaligned);
    }
    if !specification.signed && specification.to_offset < specification.from_offset {
        return rejection(CompileRejection::UnsignedPointerUnderflow);
    }
    let distance = i64::from(specification.to_offset) - i64::from(specification.from_offset);
    ReferenceOutcome::Execution(SemanticObservation::Scalar(
        distance / i64::from(element_bytes),
    ))
}

fn volatile(specification: &VolatileSpec) -> ReferenceOutcome {
    let index = usize::from(specification.index);
    if index >= specification.words.len() {
        return rejection(CompileRejection::VolatileOutOfBounds);
    }
    if specification.byte_alignment < 4 {
        return rejection(CompileRejection::VolatileMisaligned);
    }
    match specification.operation {
        VolatileOperation::Load => {
            if !specification.readable {
                rejection(CompileRejection::VolatilePermission)
            } else {
                ReferenceOutcome::Execution(SemanticObservation::Scalar(i64::from(
                    specification.words[index],
                )))
            }
        }
        VolatileOperation::Store(value) => {
            if !specification.writable {
                rejection(CompileRejection::VolatilePermission)
            } else {
                let mut words = specification.words.clone();
                words[index] = value;
                ReferenceOutcome::Execution(SemanticObservation::Words(words))
            }
        }
    }
}

fn copy_nonoverlapping(specification: &CopyNonoverlappingSpec) -> ReferenceOutcome {
    let source = usize::from(specification.source);
    let destination = usize::from(specification.destination);
    let count = usize::from(specification.count);
    let Some(source_end) = source.checked_add(count) else {
        return rejection(CompileRejection::CopyOutOfBounds);
    };
    let Some(destination_end) = destination.checked_add(count) else {
        return rejection(CompileRejection::CopyOutOfBounds);
    };
    if source_end > specification.words.len() || destination_end > specification.words.len() {
        return rejection(CompileRejection::CopyOutOfBounds);
    }
    if count != 0 && source < destination_end && destination < source_end {
        return rejection(CompileRejection::CopyOverlap);
    }
    let mut words = specification.words.clone();
    let copied = words[source..source_end].to_vec();
    words[destination..destination_end].copy_from_slice(&copied);
    ReferenceOutcome::Execution(SemanticObservation::Words(words))
}

fn layout(specification: &LayoutSpec) -> ReferenceOutcome {
    match specification {
        LayoutSpec::Aggregate { fields } => {
            let mut offsets = Vec::with_capacity(fields.len());
            let mut size = 0_u16;
            let mut alignment = 1_u8;
            for field in fields {
                alignment = alignment.max(field.alignment);
                size = align_up(size, u16::from(field.alignment));
                offsets.push(size);
                size += u16::from(field.size);
            }
            size = align_up(size, u16::from(alignment));
            ReferenceOutcome::Execution(SemanticObservation::Layout {
                size,
                alignment,
                offsets,
            })
        }
        LayoutSpec::TaggedEnum { tag, payloads } => {
            let payload_alignment = payloads
                .iter()
                .map(|payload| payload.alignment)
                .max()
                .unwrap_or(1);
            let alignment = tag.alignment.max(payload_alignment);
            let payload_offset = align_up(u16::from(tag.size), u16::from(payload_alignment));
            let payload_size = payloads
                .iter()
                .map(|payload| u16::from(payload.size))
                .max()
                .unwrap_or(0);
            let size = align_up(payload_offset + payload_size, u16::from(alignment));
            ReferenceOutcome::Execution(SemanticObservation::Layout {
                size,
                alignment,
                offsets: vec![0, payload_offset],
            })
        }
        LayoutSpec::NicheEnum { .. } => rejection(CompileRejection::UnsupportedNicheLayout),
    }
}

fn integer_switch(specification: &IntegerSwitchSpec) -> ReferenceOutcome {
    for (index, (value, _)) in specification.arms.iter().enumerate() {
        if specification.arms[..index]
            .iter()
            .any(|(previous, _)| previous == value)
        {
            return rejection(CompileRejection::DuplicateSwitchValue);
        }
    }
    match specification
        .arms
        .iter()
        .position(|(value, _)| *value == specification.selector)
    {
        Some(index) => ReferenceOutcome::Execution(SemanticObservation::Switch {
            arm: Some(u8::try_from(index).expect("switch arm bound fits u8")),
            value: specification.arms[index].1,
        }),
        None => ReferenceOutcome::Execution(SemanticObservation::Switch {
            arm: None,
            value: specification.default,
        }),
    }
}

fn atomics(specification: &AtomicSpec) -> ReferenceOutcome {
    if specification.scope == AtomicScope::System {
        return rejection(CompileRejection::UnsupportedAtomicScope);
    }
    if specification.operations.iter().any(invalid_atomic_ordering) {
        return rejection(CompileRejection::InvalidAtomicOrdering);
    }
    let mut value = specification.initial;
    let mut observed = Vec::new();
    for operation in &specification.operations {
        match *operation {
            AtomicOperation::Load { .. } => observed.push(value),
            AtomicOperation::Store { value: next, .. } => value = next,
            AtomicOperation::FetchAdd { value: addend, .. } => {
                observed.push(value);
                value = value.wrapping_add(addend);
            }
            AtomicOperation::CompareExchange { current, new, .. } => {
                observed.push(value);
                if value == current {
                    value = new;
                }
            }
        }
    }
    ReferenceOutcome::Execution(SemanticObservation::Atomic {
        observed,
        final_value: value,
    })
}

fn invalid_atomic_ordering(operation: &AtomicOperation) -> bool {
    match *operation {
        AtomicOperation::Load { ordering } => {
            matches!(
                ordering,
                MemoryOrdering::Release | MemoryOrdering::AcquireRelease
            )
        }
        AtomicOperation::Store { ordering, .. } => {
            matches!(
                ordering,
                MemoryOrdering::Acquire | MemoryOrdering::AcquireRelease
            )
        }
        AtomicOperation::CompareExchange {
            success, failure, ..
        } => {
            matches!(
                failure,
                MemoryOrdering::Release | MemoryOrdering::AcquireRelease
            ) || !failure_allowed_for_success(success, failure)
        }
        AtomicOperation::FetchAdd { .. } => false,
    }
}

fn failure_allowed_for_success(success: MemoryOrdering, failure: MemoryOrdering) -> bool {
    match success {
        MemoryOrdering::Relaxed => failure == MemoryOrdering::Relaxed,
        MemoryOrdering::Acquire => {
            matches!(failure, MemoryOrdering::Relaxed | MemoryOrdering::Acquire)
        }
        MemoryOrdering::Release => failure == MemoryOrdering::Relaxed,
        MemoryOrdering::AcquireRelease => {
            matches!(failure, MemoryOrdering::Relaxed | MemoryOrdering::Acquire)
        }
        MemoryOrdering::SequentiallyConsistent => matches!(
            failure,
            MemoryOrdering::Relaxed
                | MemoryOrdering::Acquire
                | MemoryOrdering::SequentiallyConsistent
        ),
    }
}

fn obligation(specification: &ObligationSpec) -> ReferenceOutcome {
    match specification {
        ObligationSpec::Bounds { length, index } => {
            if index >= length {
                rejection(CompileRejection::BoundsObligation)
            } else {
                ReferenceOutcome::Execution(SemanticObservation::ObligationsSatisfied)
            }
        }
        ObligationSpec::Race {
            allocation_words,
            accesses,
        } => {
            if accesses
                .iter()
                .any(|access| access.index >= *allocation_words)
            {
                return rejection(CompileRejection::BoundsObligation);
            }
            for (index, left) in accesses.iter().enumerate() {
                for right in &accesses[index + 1..] {
                    let conflicts = left.lane != right.lane
                        && left.index == right.index
                        && (left.kind == AccessKind::Write || right.kind == AccessKind::Write)
                        && !(left.atomic && right.atomic);
                    if conflicts {
                        return rejection(CompileRejection::RaceObligation);
                    }
                }
            }
            ReferenceOutcome::Execution(SemanticObservation::ObligationsSatisfied)
        }
    }
}

fn rejection(reason: CompileRejection) -> ReferenceOutcome {
    ReferenceOutcome::CompileRejection(reason)
}

fn align_up(value: u16, alignment: u16) -> u16 {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

fn validate_words(words: &[i32]) -> Result<(), SemanticModelError> {
    if words.is_empty() || words.len() > MAX_SEMANTIC_WORDS {
        return Err(SemanticModelError::InvalidWordCount {
            actual: words.len(),
        });
    }
    Ok(())
}

fn validate_layout_members(members: &[ScalarLayout]) -> Result<(), SemanticModelError> {
    if members.is_empty() || members.len() > MAX_LAYOUT_MEMBERS {
        return Err(SemanticModelError::InvalidLayoutMemberCount {
            actual: members.len(),
        });
    }
    for member in members {
        member.validate()?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticModelError {
    FeatureMismatch {
        declared: SemanticFeature,
        actual: SemanticFeature,
    },
    InvalidAllocationBytes {
        actual: usize,
    },
    InvalidElementBytes {
        actual: u8,
    },
    InvalidWordCount {
        actual: usize,
    },
    InvalidAlignment {
        actual: u8,
    },
    InvalidLayoutSize {
        actual: u8,
    },
    InvalidLayoutMemberCount {
        actual: usize,
    },
    TooManySwitchArms {
        actual: usize,
    },
    InvalidAtomicStepCount {
        actual: usize,
    },
    InvalidAccessCount {
        actual: usize,
    },
}

impl fmt::Display for SemanticModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FeatureMismatch { declared, actual } => {
                write!(
                    formatter,
                    "declared feature {declared:?} does not match {actual:?}"
                )
            }
            Self::InvalidAllocationBytes { actual } => {
                write!(
                    formatter,
                    "allocation byte count {actual} is outside the bound"
                )
            }
            Self::InvalidElementBytes { actual } => {
                write!(
                    formatter,
                    "element byte count {actual} is outside the bound"
                )
            }
            Self::InvalidWordCount { actual } => {
                write!(formatter, "word count {actual} is outside the bound")
            }
            Self::InvalidAlignment { actual } => {
                write!(formatter, "alignment {actual} is invalid")
            }
            Self::InvalidLayoutSize { actual } => {
                write!(formatter, "layout size {actual} is outside the bound")
            }
            Self::InvalidLayoutMemberCount { actual } => {
                write!(
                    formatter,
                    "layout member count {actual} is outside the bound"
                )
            }
            Self::TooManySwitchArms { actual } => {
                write!(formatter, "switch arm count {actual} exceeds the bound")
            }
            Self::InvalidAtomicStepCount { actual } => {
                write!(formatter, "atomic step count {actual} is outside the bound")
            }
            Self::InvalidAccessCount { actual } => {
                write!(formatter, "access count {actual} is outside the bound")
            }
        }
    }
}

impl std::error::Error for SemanticModelError {}
