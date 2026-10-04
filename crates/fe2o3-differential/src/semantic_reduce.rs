use core::fmt;

use crate::{
    AtomicOperation, AtomicScope, CorpusError, LayoutSpec, MemoryOrdering, ObligationSpec,
    ScalarLayout, SemanticCase, SemanticCaseIdentityV1, SemanticModelError, SemanticSpec,
    VolatileOperation, encode_semantic_case_v1, semantic_case_identity_v1,
};

pub const MAX_SEMANTIC_REDUCTION_ATTEMPTS: usize = 16_384;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticCaseComplexity {
    pub structural_items: usize,
    pub semantic_weight: u64,
    pub canonical_bytes: usize,
}

impl SemanticCaseComplexity {
    pub fn measure(case: &SemanticCase) -> Self {
        let (structural_items, semantic_weight) = specification_complexity(case.specification());
        let canonical_bytes = encode_semantic_case_v1(case)
            .expect("SemanticCase invariants produce bounded canonical bytes")
            .len();
        Self {
            structural_items,
            semantic_weight,
            canonical_bytes,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticReductionResult {
    pub case: SemanticCase,
    pub source_identity: SemanticCaseIdentityV1,
    pub reduced_identity: SemanticCaseIdentityV1,
    pub initial_complexity: SemanticCaseComplexity,
    pub final_complexity: SemanticCaseComplexity,
    pub predicate_evaluations: usize,
    pub accepted_reductions: usize,
}

/// Deterministically reduces a semantic case while `interesting` remains true.
///
/// The predicate defines what must be preserved, typically an exact mismatch or
/// compile-rejection class. Candidates are bounded and visited in stable order.
pub fn reduce_semantic_case<F>(
    case: &SemanticCase,
    mut interesting: F,
) -> Result<SemanticReductionResult, SemanticReduceError>
where
    F: FnMut(&SemanticCase) -> bool,
{
    case.validate().map_err(SemanticReduceError::InvalidCase)?;
    if !interesting(case) {
        return Err(SemanticReduceError::InitialPredicateAbsent);
    }

    let source_identity = semantic_case_identity_v1(case).map_err(SemanticReduceError::Identity)?;
    let initial_complexity = SemanticCaseComplexity::measure(case);
    let mut current = case.clone();
    let mut predicate_evaluations = 1;
    let mut accepted_reductions = 0;

    loop {
        let candidates = reduction_candidates(&current);
        let mut accepted = None;
        for candidate in candidates {
            if predicate_evaluations == MAX_SEMANTIC_REDUCTION_ATTEMPTS {
                return Err(SemanticReduceError::AttemptLimitExceeded);
            }
            predicate_evaluations += 1;
            if interesting(&candidate) {
                accepted = Some(candidate);
                accepted_reductions += 1;
                break;
            }
        }
        match accepted {
            Some(candidate) => current = candidate,
            None => break,
        }
    }

    Ok(SemanticReductionResult {
        reduced_identity: semantic_case_identity_v1(&current)
            .map_err(SemanticReduceError::Identity)?,
        final_complexity: SemanticCaseComplexity::measure(&current),
        case: current,
        source_identity,
        initial_complexity,
        predicate_evaluations,
        accepted_reductions,
    })
}

fn reduction_candidates(case: &SemanticCase) -> Vec<SemanticCase> {
    let current = SemanticCaseComplexity::measure(case);
    let specifications = match case.specification() {
        SemanticSpec::PointerDistance(value) => {
            let mut candidates = Vec::new();
            for (field, replacement) in [(0, 0), (1, 0), (2, 0), (3, 1), (4, 1), (5, 1)] {
                let mut reduced = value.clone();
                match field {
                    0 => reduced.allocation_bytes = replacement,
                    1 => reduced.from_offset = replacement,
                    2 => reduced.to_offset = replacement,
                    3 => reduced.element_bytes = replacement as u8,
                    4 => reduced.same_allocation = replacement != 0,
                    _ => reduced.signed = replacement != 0,
                }
                candidates.push(SemanticSpec::PointerDistance(reduced));
            }
            candidates
        }
        SemanticSpec::Volatile(value) => {
            let mut candidates = Vec::new();
            if value.words.len() > 1 {
                let mut reduced = value.clone();
                reduced.words.pop();
                candidates.push(SemanticSpec::Volatile(reduced));
            }
            let mut zero_words = value.clone();
            zero_words.words.fill(0);
            candidates.push(SemanticSpec::Volatile(zero_words));
            let mut zero_index = value.clone();
            zero_index.index = 0;
            candidates.push(SemanticSpec::Volatile(zero_index));
            let mut minimum_alignment = value.clone();
            minimum_alignment.byte_alignment = 1;
            candidates.push(SemanticSpec::Volatile(minimum_alignment));
            if let VolatileOperation::Store(stored) = value.operation
                && stored != 0
            {
                let mut zero_store = value.clone();
                zero_store.operation = VolatileOperation::Store(0);
                candidates.push(SemanticSpec::Volatile(zero_store));
            }
            candidates
        }
        SemanticSpec::CopyNonoverlapping(value) => {
            let mut candidates = Vec::new();
            if value.words.len() > 1 {
                let mut reduced = value.clone();
                reduced.words.pop();
                candidates.push(SemanticSpec::CopyNonoverlapping(reduced));
            }
            let mut zero_words = value.clone();
            zero_words.words.fill(0);
            candidates.push(SemanticSpec::CopyNonoverlapping(zero_words));
            for field in 0..3 {
                let mut reduced = value.clone();
                match field {
                    0 => reduced.source = 0,
                    1 => reduced.destination = 0,
                    _ => reduced.count = reduced.count.min(1),
                }
                candidates.push(SemanticSpec::CopyNonoverlapping(reduced));
            }
            candidates
        }
        SemanticSpec::Layout(value) => layout_candidates(value),
        SemanticSpec::IntegerSwitch(value) => {
            let mut candidates = Vec::new();
            if !value.arms.is_empty() {
                let mut reduced = value.clone();
                reduced.arms.pop();
                candidates.push(SemanticSpec::IntegerSwitch(reduced));
            }
            let mut selector = value.clone();
            selector.selector = 0;
            candidates.push(SemanticSpec::IntegerSwitch(selector));
            let mut default = value.clone();
            default.default = 0;
            candidates.push(SemanticSpec::IntegerSwitch(default));
            for index in 0..value.arms.len() {
                for component in 0..2 {
                    let mut reduced = value.clone();
                    if component == 0 {
                        reduced.arms[index].0 = 0;
                    } else {
                        reduced.arms[index].1 = 0;
                    }
                    candidates.push(SemanticSpec::IntegerSwitch(reduced));
                }
            }
            candidates
        }
        SemanticSpec::Atomics(value) => {
            let mut candidates = Vec::new();
            if value.operations.len() > 1 {
                let mut reduced = value.clone();
                reduced.operations.pop();
                candidates.push(SemanticSpec::Atomics(reduced));
            }
            let mut initial = value.clone();
            initial.initial = 0;
            candidates.push(SemanticSpec::Atomics(initial));
            let mut scope = value.clone();
            scope.scope = AtomicScope::Workgroup;
            candidates.push(SemanticSpec::Atomics(scope));
            for index in 0..value.operations.len() {
                let mut reduced = value.clone();
                reduced.operations[index] = reduce_atomic(value.operations[index]);
                candidates.push(SemanticSpec::Atomics(reduced));
            }
            candidates
        }
        SemanticSpec::Obligation(value) => obligation_candidates(value),
    };

    let mut candidates = Vec::new();
    for specification in specifications {
        let Some(candidate) = case.rebuild(specification) else {
            continue;
        };
        if SemanticCaseComplexity::measure(&candidate) < current && !candidates.contains(&candidate)
        {
            candidates.push(candidate);
        }
    }
    candidates
}

fn layout_candidates(layout: &LayoutSpec) -> Vec<SemanticSpec> {
    let mut candidates = Vec::new();
    match layout {
        LayoutSpec::Aggregate { fields } => {
            if fields.len() > 1 {
                let mut reduced = fields.clone();
                reduced.pop();
                candidates.push(SemanticSpec::Layout(LayoutSpec::Aggregate {
                    fields: reduced,
                }));
            }
            for index in 0..fields.len() {
                let mut reduced = fields.clone();
                reduced[index] = reduce_layout(reduced[index]);
                candidates.push(SemanticSpec::Layout(LayoutSpec::Aggregate {
                    fields: reduced,
                }));
            }
        }
        LayoutSpec::TaggedEnum { tag, payloads } => {
            if payloads.len() > 1 {
                let mut reduced = payloads.clone();
                reduced.pop();
                candidates.push(SemanticSpec::Layout(LayoutSpec::TaggedEnum {
                    tag: *tag,
                    payloads: reduced,
                }));
            }
            let reduced_tag = reduce_layout(*tag);
            candidates.push(SemanticSpec::Layout(LayoutSpec::TaggedEnum {
                tag: reduced_tag,
                payloads: payloads.clone(),
            }));
            for index in 0..payloads.len() {
                let mut reduced = payloads.clone();
                reduced[index] = reduce_layout(reduced[index]);
                candidates.push(SemanticSpec::Layout(LayoutSpec::TaggedEnum {
                    tag: *tag,
                    payloads: reduced,
                }));
            }
        }
        LayoutSpec::NicheEnum { payload } => {
            candidates.push(SemanticSpec::Layout(LayoutSpec::NicheEnum {
                payload: reduce_layout(*payload),
            }));
        }
    }
    candidates
}

fn obligation_candidates(obligation: &ObligationSpec) -> Vec<SemanticSpec> {
    let mut candidates = Vec::new();
    match obligation {
        ObligationSpec::Bounds { length, index } => {
            candidates.push(SemanticSpec::Obligation(ObligationSpec::Bounds {
                length: 1,
                index: *index,
            }));
            candidates.push(SemanticSpec::Obligation(ObligationSpec::Bounds {
                length: *length,
                index: 0,
            }));
        }
        ObligationSpec::Race {
            allocation_words,
            accesses,
        } => {
            if accesses.len() > 1 {
                let mut reduced = accesses.clone();
                reduced.pop();
                candidates.push(SemanticSpec::Obligation(ObligationSpec::Race {
                    allocation_words: *allocation_words,
                    accesses: reduced,
                }));
            }
            candidates.push(SemanticSpec::Obligation(ObligationSpec::Race {
                allocation_words: 1,
                accesses: accesses.clone(),
            }));
            for index in 0..accesses.len() {
                let mut reduced = accesses.clone();
                reduced[index].lane = 0;
                reduced[index].index = 0;
                candidates.push(SemanticSpec::Obligation(ObligationSpec::Race {
                    allocation_words: *allocation_words,
                    accesses: reduced,
                }));
            }
        }
    }
    candidates
}

fn reduce_layout(_layout: ScalarLayout) -> ScalarLayout {
    ScalarLayout {
        size: 1,
        alignment: 1,
    }
}

fn reduce_atomic(operation: AtomicOperation) -> AtomicOperation {
    match operation {
        AtomicOperation::Load { .. } => AtomicOperation::Load {
            ordering: MemoryOrdering::Relaxed,
        },
        AtomicOperation::Store { .. } => AtomicOperation::Store {
            value: 0,
            ordering: MemoryOrdering::Relaxed,
        },
        AtomicOperation::FetchAdd { .. } => AtomicOperation::FetchAdd {
            value: 0,
            ordering: MemoryOrdering::Relaxed,
        },
        AtomicOperation::CompareExchange { .. } => AtomicOperation::CompareExchange {
            current: 0,
            new: 0,
            success: MemoryOrdering::Relaxed,
            failure: MemoryOrdering::Relaxed,
        },
    }
}

fn specification_complexity(specification: &SemanticSpec) -> (usize, u64) {
    match specification {
        SemanticSpec::PointerDistance(value) => (
            1,
            u64::from(value.allocation_bytes)
                + u64::from(value.from_offset)
                + u64::from(value.to_offset)
                + u64::from(value.element_bytes)
                + u64::from(value.same_allocation)
                + u64::from(value.signed),
        ),
        SemanticSpec::Volatile(value) => (
            value.words.len(),
            words_weight(&value.words)
                + u64::from(value.index)
                + u64::from(value.byte_alignment)
                + u64::from(value.readable)
                + u64::from(value.writable)
                + match value.operation {
                    VolatileOperation::Load => 0,
                    VolatileOperation::Store(stored) => 1 + u64::from(stored.unsigned_abs()),
                },
        ),
        SemanticSpec::CopyNonoverlapping(value) => (
            value.words.len(),
            words_weight(&value.words)
                + u64::from(value.source)
                + u64::from(value.destination)
                + u64::from(value.count),
        ),
        SemanticSpec::Layout(value) => match value {
            LayoutSpec::Aggregate { fields } => (fields.len(), layouts_weight(fields)),
            LayoutSpec::TaggedEnum { tag, payloads } => (
                payloads.len() + 1,
                layout_weight(*tag) + layouts_weight(payloads) + 1,
            ),
            LayoutSpec::NicheEnum { payload } => (1, layout_weight(*payload) + 2),
        },
        SemanticSpec::IntegerSwitch(value) => (
            value.arms.len(),
            u64::from(value.selector.unsigned_abs())
                + u64::from(value.default.unsigned_abs())
                + value
                    .arms
                    .iter()
                    .map(|(selector, result)| {
                        u64::from(selector.unsigned_abs()) + u64::from(result.unsigned_abs())
                    })
                    .sum::<u64>(),
        ),
        SemanticSpec::Atomics(value) => (
            value.operations.len(),
            u64::from(value.initial.unsigned_abs())
                + scope_weight(value.scope)
                + value
                    .operations
                    .iter()
                    .copied()
                    .map(atomic_weight)
                    .sum::<u64>(),
        ),
        SemanticSpec::Obligation(value) => match value {
            ObligationSpec::Bounds { length, index } => (1, u64::from(*length) + u64::from(*index)),
            ObligationSpec::Race {
                allocation_words,
                accesses,
            } => (
                accesses.len(),
                u64::from(*allocation_words)
                    + accesses
                        .iter()
                        .map(|access| {
                            u64::from(access.lane)
                                + u64::from(access.index)
                                + u64::from(access.atomic)
                                + 1
                        })
                        .sum::<u64>(),
            ),
        },
    }
}

fn words_weight(words: &[i32]) -> u64 {
    words.iter().fold(0_u64, |weight, value| {
        weight.saturating_add(u64::from(value.unsigned_abs()))
    })
}

fn layouts_weight(layouts: &[ScalarLayout]) -> u64 {
    layouts.iter().copied().map(layout_weight).sum()
}

fn layout_weight(layout: ScalarLayout) -> u64 {
    u64::from(layout.size) + u64::from(layout.alignment)
}

fn scope_weight(scope: AtomicScope) -> u64 {
    match scope {
        AtomicScope::Workgroup => 0,
        AtomicScope::Device => 1,
        AtomicScope::System => 2,
    }
}

fn atomic_weight(operation: AtomicOperation) -> u64 {
    match operation {
        AtomicOperation::Load { ordering } => ordering_weight(ordering),
        AtomicOperation::Store { value, ordering }
        | AtomicOperation::FetchAdd { value, ordering } => {
            1 + u64::from(value.unsigned_abs()) + ordering_weight(ordering)
        }
        AtomicOperation::CompareExchange {
            current,
            new,
            success,
            failure,
        } => {
            2 + u64::from(current.unsigned_abs())
                + u64::from(new.unsigned_abs())
                + ordering_weight(success)
                + ordering_weight(failure)
        }
    }
}

fn ordering_weight(ordering: MemoryOrdering) -> u64 {
    match ordering {
        MemoryOrdering::Relaxed => 0,
        MemoryOrdering::Acquire | MemoryOrdering::Release => 1,
        MemoryOrdering::AcquireRelease => 2,
        MemoryOrdering::SequentiallyConsistent => 3,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticReduceError {
    InvalidCase(SemanticModelError),
    InitialPredicateAbsent,
    AttemptLimitExceeded,
    Identity(CorpusError),
}

impl fmt::Display for SemanticReduceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCase(error) => write!(formatter, "cannot reduce invalid case: {error}"),
            Self::InitialPredicateAbsent => {
                formatter.write_str("initial case does not satisfy the reduction predicate")
            }
            Self::AttemptLimitExceeded => {
                formatter.write_str("semantic reduction attempt limit was exceeded")
            }
            Self::Identity(error) => write!(formatter, "cannot identify semantic case: {error}"),
        }
    }
}

impl std::error::Error for SemanticReduceError {}
