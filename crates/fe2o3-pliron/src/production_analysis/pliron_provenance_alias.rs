//! Workload-neutral provenance and alias facts for ranked PLIRON memory.
//!
//! The analysis derives its subjects from ranked views, accesses, and
//! whole-allocation effects already present in IR. Allocation-origin and
//! no-alias attributes constrain the relation, but inconsistent or incomplete
//! contracts fail closed and never manufacture a proof.

use std::collections::{HashMap, HashSet};
use std::fmt;

use dialect_kernel::{AllocationEffectOp, MemorySpaceAttr, RankedAccessOp, RankedViewOp};
#[cfg(test)]
use pliron::builtin::ops::FuncOp;
use pliron::{common_traits::Named, context::Context, operation::Operation, value::Value};

use crate::production_analysis::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
use crate::production_analysis::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
};

pub const MAX_PLIRON_PROVENANCE_SUBJECTS_V1: usize = 65_536;
pub(crate) const MAX_PLIRON_PROVENANCE_DIAGNOSTIC_BYTES_V1: usize = 1_024;
const MAX_PLIRON_PROVENANCE_DIAGNOSTIC_NAME_BYTES_V1: usize = 256;
const PROVENANCE_RETAINED_FIXED_ITEMS_PER_SUBJECT_V1: usize = 8;
const PROVENANCE_CONSTRUCTION_FIXED_ITEMS_PER_SUBJECT_V1: usize = 4;
// During validation these structures coexist: relevant-subject roster (1),
// origin->class map (2), distinct-subject set (2), writable-class set (2),
// class->origins outer map and inner sets (4), signature map (2), and the
// returned origin roster on the rejecting path (1).
// Two further units cover hash-table capacity slack while all six validation
// collections are live.
const PROVENANCE_VALIDATION_FIXED_ITEMS_PER_SUBJECT_V1: usize = 16;
const PROVENANCE_REJECTION_FIXED_ITEMS_V1: usize = 8;
// Pinned Value::id owns `v` plus at most 20 u64 digits. Allocation-site labels
// own 31 literal bytes and two usize decimal values (at most 71 bytes on the
// supported 64-bit host). The pinned formatter requests at most 124 bytes for
// either label, including growth. This excludes allocator-internal rounding.
const PROVENANCE_SUBJECT_LABEL_PAYLOAD_BYTES_V1: usize = 128;

fn provenance_resource_overflow_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::ProvenanceAlias,
        resource: "provenance/alias resource upper bound",
    }
}

fn checked_provenance_mul_v1(
    lhs: usize,
    rhs: usize,
) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs)
        .ok_or_else(provenance_resource_overflow_v1)
}

fn checked_provenance_sum_v1(items: &[usize]) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    items.iter().try_fold(0_usize, |total, item| {
        total
            .checked_add(*item)
            .ok_or_else(provenance_resource_overflow_v1)
    })
}

/// Bounds collection and all three address-space validation sweeps before the
/// provenance cache is allocated. The authenticated ranked-access and
/// allocation-effect census bounds subjects; every operation is still scanned.
/// The fixed per-subject terms cover the subject record and capacity,
/// view-map entry/contract, origin/class maps, and validation sets. A unique
/// view retains its rank-sized signature both in the view contract and in its
/// subject. Construction briefly owns the source signature plus the contract
/// and map-entry copies, so a third rank-sized roster is temporary. Each subject
/// owns a bounded numeric value or allocation-site label, independent of both
/// authored identifiers and unauthenticated debug aliases.
pub(crate) fn preflight_provenance_alias_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let subjects = census
        .ranked_accesses
        .checked_add(census.allocation_effects)
        .ok_or_else(provenance_resource_overflow_v1)?
        .min(MAX_PLIRON_PROVENANCE_SUBJECTS_V1);
    let subject_label_bytes =
        checked_provenance_mul_v1(subjects, PROVENANCE_SUBJECT_LABEL_PAYLOAD_BYTES_V1)?;
    let retained_subject_items =
        checked_provenance_mul_v1(subjects, PROVENANCE_RETAINED_FIXED_ITEMS_PER_SUBJECT_V1)?;
    let retained_signature_items = checked_provenance_mul_v1(
        checked_provenance_mul_v1(subjects, dialect_kernel::MAX_RANKED_MEMORY_RANK)?,
        2,
    )?;
    let retained = checked_provenance_sum_v1(&[
        retained_subject_items,
        retained_signature_items,
        subject_label_bytes,
        3,
    ])?;
    let possible_rejection = usize::from(subjects != 0);
    // Collection can own the locally rendered name and the clone installed in
    // the returned failure at once. The bounded projection may subsequently
    // coexist with that failure.
    let rejection_temporary = checked_provenance_mul_v1(
        possible_rejection,
        checked_provenance_sum_v1(&[
            checked_provenance_mul_v1(PROVENANCE_SUBJECT_LABEL_PAYLOAD_BYTES_V1, 2)?,
            MAX_PLIRON_PROVENANCE_DIAGNOSTIC_BYTES_V1,
            PROVENANCE_REJECTION_FIXED_ITEMS_V1,
        ])?,
    )?;
    let validation_temporary = checked_provenance_sum_v1(&[
        checked_provenance_mul_v1(
            subjects,
            dialect_kernel::MAX_RANKED_MEMORY_RANK
                .checked_add(PROVENANCE_VALIDATION_FIXED_ITEMS_PER_SUBJECT_V1)
                .ok_or_else(provenance_resource_overflow_v1)?,
        )?,
        rejection_temporary,
    ])?;
    let construction_temporary = checked_provenance_mul_v1(
        subjects,
        dialect_kernel::MAX_RANKED_MEMORY_RANK
            .checked_add(PROVENANCE_CONSTRUCTION_FIXED_ITEMS_PER_SUBJECT_V1)
            .ok_or_else(provenance_resource_overflow_v1)?,
    )?;
    let temporary = validation_temporary.max(checked_provenance_sum_v1(&[
        construction_temporary,
        rejection_temporary,
    ])?);
    let work = checked_provenance_sum_v1(&[
        checked_provenance_mul_v1(census.operations, 8)?,
        checked_provenance_mul_v1(subjects, 32)?,
        subject_label_bytes,
    ])?;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::ProvenanceAlias,
        work,
        retained,
        temporary,
    )?;
    limits.require(ProductionAnalysisResourcePhaseV1::ProvenanceAlias, bound)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironProvenanceFailureV1 {
    ResourceLimit {
        limit: usize,
        actual: usize,
    },
    MissingViewDefinition {
        view: String,
    },
    ForeignViewDefinition {
        view: String,
    },
    MissingMemorySpace {
        view: String,
    },
    ClaimedNoAliasWithoutOrigin {
        subject: String,
        class: u64,
    },
    InconsistentClassForOrigin {
        origin: u64,
        first: u64,
        second: u64,
    },
    UnknownWritableAlias {
        memory_space: MemorySpaceAttr,
    },
    MissingRelativeOffset {
        memory_space: MemorySpaceAttr,
        class: u64,
        origins: Vec<u64>,
    },
    IncompatibleViewSignature {
        memory_space: MemorySpaceAttr,
        class: u64,
    },
}

impl fmt::Display for PlironProvenanceFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit { limit, actual } => write!(
                formatter,
                "ranked provenance subject count {actual} exceeds analysis limit {limit}",
            ),
            Self::MissingViewDefinition { view } => {
                write!(
                    formatter,
                    "ranked view value {view} has no defining operation"
                )
            }
            Self::ForeignViewDefinition { view } => write!(
                formatter,
                "ranked view value {view} is not defined by kernel.ranked_view",
            ),
            Self::MissingMemorySpace { view } => {
                write!(formatter, "ranked view {view} has no memory space")
            }
            Self::ClaimedNoAliasWithoutOrigin { subject, class } => write!(
                formatter,
                "{subject} claims no-alias class {class} without a compiler-derived allocation origin",
            ),
            Self::InconsistentClassForOrigin {
                origin,
                first,
                second,
            } => write!(
                formatter,
                "allocation origin {origin} is assigned inconsistent no-alias classes {first} and {second}",
            ),
            Self::UnknownWritableAlias { memory_space } => write!(
                formatter,
                "an unknown-alias {memory_space:?} view may overlap another subject and at least one subject is writable; relative base offsets are unavailable",
            ),
            Self::MissingRelativeOffset {
                memory_space,
                class,
                origins,
            } => write!(
                formatter,
                "potentially aliasing class {class} in {memory_space:?} memory contains writable views from distinct allocation origins {origins:?}, but ranked IR does not retain their relative base offsets",
            ),
            Self::IncompatibleViewSignature {
                memory_space,
                class,
            } => write!(
                formatter,
                "potentially aliasing view class {class} in {memory_space:?} memory has incompatible element widths or rank/shapes",
            ),
        }
    }
}

impl PlironProvenanceFailureV1 {
    /// Produces a bounded projection for reports owned by another analysis.
    /// Large origin rosters are summarized instead of debug-formatted.
    pub(crate) fn bounded_description_v1(&self) -> String {
        match self {
            Self::ResourceLimit { limit, actual } => {
                format!("ranked provenance subject count {actual} exceeds analysis limit {limit}")
            }
            Self::MissingViewDefinition { view } => format!(
                "ranked view value {} has no defining operation",
                bounded_provenance_name_v1(view)
            ),
            Self::ForeignViewDefinition { view } => format!(
                "ranked view value {} is not defined by kernel.ranked_view",
                bounded_provenance_name_v1(view)
            ),
            Self::MissingMemorySpace { view } => format!(
                "ranked view {} has no memory space",
                bounded_provenance_name_v1(view)
            ),
            Self::ClaimedNoAliasWithoutOrigin { subject, class } => format!(
                "{} claims no-alias class {class} without a compiler-derived allocation origin",
                bounded_provenance_name_v1(subject)
            ),
            Self::InconsistentClassForOrigin {
                origin,
                first,
                second,
            } => format!(
                "allocation origin {origin} is assigned inconsistent no-alias classes {first} and {second}"
            ),
            Self::UnknownWritableAlias { memory_space } => format!(
                "an unknown-alias {memory_space:?} view may overlap another writable subject; relative base offsets are unavailable"
            ),
            Self::MissingRelativeOffset {
                memory_space,
                class,
                origins,
            } => format!(
                "potentially aliasing class {class} in {memory_space:?} memory contains {} distinct allocation origins (first {:?}); ranked IR does not retain their relative base offsets",
                origins.len(),
                origins.first().copied()
            ),
            Self::IncompatibleViewSignature {
                memory_space,
                class,
            } => format!(
                "potentially aliasing view class {class} in {memory_space:?} memory has incompatible element widths or rank/shapes"
            ),
        }
    }
}

fn bounded_provenance_name_v1(name: &str) -> String {
    if name.len() <= MAX_PLIRON_PROVENANCE_DIAGNOSTIC_NAME_BYTES_V1 {
        return name.to_owned();
    }
    let mut end = MAX_PLIRON_PROVENANCE_DIAGNOSTIC_NAME_BYTES_V1.saturating_sub(3);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    let mut bounded = String::with_capacity(MAX_PLIRON_PROVENANCE_DIAGNOSTIC_NAME_BYTES_V1);
    bounded.push_str(&name[..end]);
    bounded.push_str("...");
    bounded
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironProvenanceContractV1 {
    allocation_origin: u64,
    noalias_class: u64,
    memory_space: MemorySpaceAttr,
    signature: (u32, Vec<u64>),
}

impl PlironProvenanceContractV1 {
    pub const fn allocation_origin(&self) -> u64 {
        self.allocation_origin
    }

    pub const fn noalias_class(&self) -> u64 {
        self.noalias_class
    }

    pub const fn memory_space(&self) -> MemorySpaceAttr {
        self.memory_space
    }

    pub fn signature(&self) -> &(u32, Vec<u64>) {
        &self.signature
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlironAliasDecisionV1 {
    SameAllocation,
    Disjoint,
    Incomplete,
}

#[derive(Clone, Debug)]
pub struct PlironProvenanceAliasAnalysisV1 {
    views: HashMap<Value, PlironProvenanceContractV1>,
    unknown_spaces: HashSet<MemorySpaceAttr>,
    subjects: Vec<SubjectV1>,
}

impl PlironProvenanceAliasAnalysisV1 {
    pub fn view(&self, value: Value) -> Option<&PlironProvenanceContractV1> {
        self.views.get(&value)
    }

    pub fn canonical_class(&self, memory_space: MemorySpaceAttr, class: u64) -> u64 {
        if self.unknown_spaces.contains(&memory_space) {
            0
        } else {
            class
        }
    }

    pub fn alias(&self, first: Value, second: Value) -> PlironAliasDecisionV1 {
        if first == second && self.views.contains_key(&first) {
            return PlironAliasDecisionV1::SameAllocation;
        }
        let (Some(first), Some(second)) = (self.views.get(&first), self.views.get(&second)) else {
            return PlironAliasDecisionV1::Incomplete;
        };
        if first.memory_space != second.memory_space {
            return PlironAliasDecisionV1::Disjoint;
        }
        if first.allocation_origin != 0 && first.allocation_origin == second.allocation_origin {
            return PlironAliasDecisionV1::SameAllocation;
        }
        if first.noalias_class != 0
            && second.noalias_class != 0
            && first.noalias_class != second.noalias_class
        {
            return PlironAliasDecisionV1::Disjoint;
        }
        PlironAliasDecisionV1::Incomplete
    }

    pub(crate) fn validate_space(
        &self,
        memory_space: MemorySpaceAttr,
    ) -> Result<(), PlironProvenanceFailureV1> {
        validate_subjects_for_space(&self.subjects, memory_space)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum SubjectIdentityV1 {
    View(Value),
    AllocationOrigin(u64),
    AllocationSite(usize, usize),
}

#[derive(Clone, Debug)]
struct SubjectV1 {
    identity: SubjectIdentityV1,
    label: String,
    allocation_origin: u64,
    noalias_class: u64,
    memory_space: MemorySpaceAttr,
    signature: Option<(u32, Vec<u64>)>,
    writes: bool,
}

#[cfg(test)]
pub(crate) fn analyze_pliron_provenance_alias_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironProvenanceAliasAnalysisV1, PlironProvenanceFailureV1> {
    let inventory =
        BoundedPlironFunctionInventoryV1::collect(context, function).map_err(|failure| {
            PlironProvenanceFailureV1::ResourceLimit {
                limit: failure.limit(),
                actual: failure.actual(),
            }
        })?;
    analyze_pliron_provenance_alias_with_inventory_v1(context, &inventory)
}

pub(crate) fn collect_pliron_provenance_alias_with_inventory_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> Result<PlironProvenanceAliasAnalysisV1, PlironProvenanceFailureV1> {
    let mut subjects = Vec::new();
    let mut views = HashMap::new();
    for site in inventory.operations() {
        let block_index = site.block();
        let operation_index = site.operation();
        let operation = Operation::get_op_dyn(site.pointer(), context);
        if let Some(effect) = operation.downcast_ref::<AllocationEffectOp>() {
            let memory_space = effect.memory_space(context).ok_or_else(|| {
                PlironProvenanceFailureV1::MissingMemorySpace {
                    view: format!("allocation effect at block {block_index} op {operation_index}"),
                }
            })?;
            let allocation_origin = effect.allocation_origin(context).unwrap_or(0);
            push_subject(
                &mut subjects,
                SubjectV1 {
                    identity: if allocation_origin == 0 {
                        SubjectIdentityV1::AllocationSite(block_index, operation_index)
                    } else {
                        SubjectIdentityV1::AllocationOrigin(allocation_origin)
                    },
                    label: format!("allocation effect at block {block_index} op {operation_index}"),
                    allocation_origin,
                    noalias_class: effect.noalias_class(context).unwrap_or(0),
                    memory_space,
                    signature: None,
                    writes: effect
                        .kind(context)
                        .is_some_and(|kind| kind.writes_memory()),
                },
            )?;
            continue;
        }
        let Some(access) = operation.downcast_ref::<RankedAccessOp>() else {
            continue;
        };
        let view = access.view(context);
        let name: String = view.id(context).into();
        let definition =
            view.defining_op()
                .ok_or_else(|| PlironProvenanceFailureV1::MissingViewDefinition {
                    view: name.clone(),
                })?;
        let definition = Operation::get_op_dyn(definition, context);
        let view_op = definition.downcast_ref::<RankedViewOp>().ok_or_else(|| {
            PlironProvenanceFailureV1::ForeignViewDefinition { view: name.clone() }
        })?;
        let memory_space = view_op
            .memory_space(context)
            .ok_or_else(|| PlironProvenanceFailureV1::MissingMemorySpace { view: name.clone() })?;
        let view_type = view_op
            .view_type(context)
            .expect("structurally verified ranked view has a ranked view type");
        let signature = {
            let view_type = view_type.deref(context);
            (view_type.element_width(), view_type.shape().to_vec())
        };
        let contract = PlironProvenanceContractV1 {
            allocation_origin: view_op.allocation_origin(context).unwrap_or(0),
            noalias_class: view_op.noalias_class(context).unwrap_or(0),
            memory_space,
            signature: signature.clone(),
        };
        views.entry(view).or_insert_with(|| contract.clone());
        push_subject(
            &mut subjects,
            SubjectV1 {
                identity: SubjectIdentityV1::View(view),
                label: name,
                allocation_origin: contract.allocation_origin,
                noalias_class: contract.noalias_class,
                memory_space,
                signature: Some(signature),
                writes: access
                    .kind(context)
                    .is_some_and(|kind| kind.writes_memory()),
            },
        )?;
    }

    let unknown_spaces = subjects
        .iter()
        .filter_map(|subject| (subject.noalias_class == 0).then_some(subject.memory_space))
        .collect();
    Ok(PlironProvenanceAliasAnalysisV1 {
        views,
        unknown_spaces,
        subjects,
    })
}

pub(crate) fn analyze_pliron_provenance_alias_with_inventory_v1(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> Result<PlironProvenanceAliasAnalysisV1, PlironProvenanceFailureV1> {
    let analysis = collect_pliron_provenance_alias_with_inventory_v1(context, inventory)?;
    for memory_space in [
        MemorySpaceAttr::Private,
        MemorySpaceAttr::Workgroup,
        MemorySpaceAttr::Global,
    ] {
        analysis.validate_space(memory_space)?;
    }
    Ok(analysis)
}

fn push_subject(
    subjects: &mut Vec<SubjectV1>,
    subject: SubjectV1,
) -> Result<(), PlironProvenanceFailureV1> {
    if subjects.len() == MAX_PLIRON_PROVENANCE_SUBJECTS_V1 {
        return Err(PlironProvenanceFailureV1::ResourceLimit {
            limit: MAX_PLIRON_PROVENANCE_SUBJECTS_V1,
            actual: subjects.len() + 1,
        });
    }
    subjects.push(subject);
    Ok(())
}

fn validate_subjects_for_space(
    subjects: &[SubjectV1],
    memory_space: MemorySpaceAttr,
) -> Result<(), PlironProvenanceFailureV1> {
    let mut classes_by_origin = HashMap::new();
    let relevant = subjects
        .iter()
        .filter(|subject| subject.memory_space == memory_space)
        .collect::<Vec<_>>();
    for subject in &relevant {
        if subject.noalias_class != 0 && subject.allocation_origin == 0 {
            return Err(PlironProvenanceFailureV1::ClaimedNoAliasWithoutOrigin {
                subject: subject.label.clone(),
                class: subject.noalias_class,
            });
        }
        if subject.allocation_origin != 0
            && let Some(first) =
                classes_by_origin.insert(subject.allocation_origin, subject.noalias_class)
            && first != subject.noalias_class
        {
            return Err(PlironProvenanceFailureV1::InconsistentClassForOrigin {
                origin: subject.allocation_origin,
                first,
                second: subject.noalias_class,
            });
        }
    }

    let distinct = relevant
        .iter()
        .map(|subject| &subject.identity)
        .collect::<HashSet<_>>();
    if relevant.iter().any(|subject| subject.noalias_class == 0)
        && relevant.iter().any(|subject| subject.writes)
        && distinct.len() > 1
    {
        return Err(PlironProvenanceFailureV1::UnknownWritableAlias { memory_space });
    }

    let writable_classes = relevant
        .iter()
        .filter_map(|subject| subject.writes.then_some(subject.noalias_class))
        .collect::<HashSet<_>>();
    let mut origins_by_class = HashMap::<u64, HashSet<u64>>::new();
    let mut signatures_by_class = HashMap::new();
    for subject in relevant {
        if subject.noalias_class == 0 || !writable_classes.contains(&subject.noalias_class) {
            continue;
        }
        origins_by_class
            .entry(subject.noalias_class)
            .or_default()
            .insert(subject.allocation_origin);
        if let Some(signature) = &subject.signature
            && signatures_by_class
                .insert(subject.noalias_class, signature.clone())
                .is_some_and(|previous| previous != *signature)
        {
            return Err(PlironProvenanceFailureV1::IncompatibleViewSignature {
                memory_space,
                class: subject.noalias_class,
            });
        }
    }
    if let Some((&class, origins)) = origins_by_class
        .iter()
        .find(|(class, origins)| origins.len() > 1 && writable_classes.contains(class))
    {
        let mut origins = origins.iter().copied().collect::<Vec<_>>();
        origins.sort_unstable();
        return Err(PlironProvenanceFailureV1::MissingRelativeOffset {
            memory_space,
            class,
            origins,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "pliron_provenance_alias/subject_population_v69_tests.rs"]
mod subject_population_v69_tests;

#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;

    fn census() -> ProductionAnalysisInputCensusV1 {
        ProductionAnalysisInputCensusV1 {
            operations: 23,
            ranked_accesses: 23,
            identifier_bytes: 71,
            ..ProductionAnalysisInputCensusV1::default()
        }
    }

    #[test]
    fn provenance_bound_has_exact_and_one_under_admission() {
        let exact = preflight_provenance_alias_resource_upper_bound_v1(
            census(),
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        assert!(exact.work_upper_bound() > 0);
        assert!(exact.retained_storage_upper_bound() > 0);
        assert_eq!(
            preflight_provenance_alias_resource_upper_bound_v1(
                census(),
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound() - 1,
                    exact.peak_storage_upper_bound(),
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::ProvenanceAlias,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_provenance_alias_resource_upper_bound_v1(
                census(),
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound() - 1,
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::ProvenanceAlias,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn provenance_bound_rejects_subject_population_overflow() {
        let overflowing = ProductionAnalysisInputCensusV1 {
            operations: 1,
            ranked_accesses: usize::MAX,
            allocation_effects: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        assert_eq!(
            preflight_provenance_alias_resource_upper_bound_v1(
                overflowing,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(provenance_resource_overflow_v1())
        );
    }

    #[test]
    fn unique_rank_eight_view_accounts_two_retained_and_one_transient_signatures() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 1,
            ranked_accesses: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        // One possible unique view retains 8 fixed subject/map items, two
        // rank-8 signatures, a 128-byte numeric-label allowance, and 3 fixed
        // cache records. Its construction transient is rank 8 + 4 fixed
        // map/capacity items. Validation owns 16 fixed items plus one rank-8
        // signature, while the rejecting path reserves two 128-byte labels, a
        // 1,024-byte bounded diagnostic, and 8 fixed failure items.
        const EXACT_WORK: usize = 168;
        const EXACT_RETAINED: usize = 155;
        const EXACT_PEAK: usize = 1_467;
        let exact = preflight_provenance_alias_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.retained_storage_upper_bound(), EXACT_RETAINED);
        assert_eq!(exact.peak_storage_upper_bound(), EXACT_PEAK);
        assert_eq!(
            preflight_provenance_alias_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK - 1, EXACT_PEAK),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::ProvenanceAlias,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_provenance_alias_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK - 1),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::ProvenanceAlias,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn multi_subject_rank_eight_validation_peak_is_literal_and_one_under() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 4,
            ranked_accesses: 4,
            identifier_bytes: 32,
            ..ProductionAnalysisInputCensusV1::default()
        };
        // Retained: 4*8 + 4*8*2 + 4*128 + 3 = 611.
        // Validation temporary: 4*(8+16) + 2*128 + 1,024 + 8 = 1,384.
        // Construction is smaller; authored identifier bytes do not label subjects.
        // Work: 4*8 + 4*32 + 4*128 = 672.
        const EXACT_WORK: usize = 672;
        const EXACT_RETAINED: usize = 611;
        const EXACT_PEAK: usize = 1_995;
        let exact = preflight_provenance_alias_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.retained_storage_upper_bound(), EXACT_RETAINED);
        assert_eq!(exact.peak_storage_upper_bound(), EXACT_PEAK);
        assert_eq!(
            preflight_provenance_alias_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK - 1),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::ProvenanceAlias,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn projected_diagnostics_bound_names_and_summarize_origin_rosters() {
        let name = "view".repeat(10_000);
        let name_detail = PlironProvenanceFailureV1::MissingViewDefinition { view: name }
            .bounded_description_v1();
        assert!(name_detail.len() <= MAX_PLIRON_PROVENANCE_DIAGNOSTIC_BYTES_V1);
        assert!(name_detail.contains("..."));

        let origins_detail = PlironProvenanceFailureV1::MissingRelativeOffset {
            memory_space: MemorySpaceAttr::Workgroup,
            class: 7,
            origins: (0..10_000).collect(),
        }
        .bounded_description_v1();
        assert!(origins_detail.len() <= MAX_PLIRON_PROVENANCE_DIAGNOSTIC_BYTES_V1);
        assert!(origins_detail.contains("10000 distinct allocation origins"));
        assert!(!origins_detail.contains("9999"));
    }
}
