//! Workload-neutral provenance and alias facts for ranked PLIRON memory.
//!
//! The analysis derives its subjects from ranked views, accesses, and
//! whole-allocation effects already present in IR. Allocation-origin and
//! no-alias attributes constrain the relation, but inconsistent or incomplete
//! contracts fail closed and never manufacture a proof.

use std::collections::{HashMap, HashSet};
use std::fmt;

use dialect_kernel::{AllocationEffectOp, MemorySpaceAttr, RankedAccessOp, RankedViewOp};
use pliron::{
    builtin::ops::FuncOp, common_traits::Named, context::Context, operation::Operation,
    value::Value,
};

use crate::pliron_function_inventory::BoundedPlironFunctionInventoryV1;

pub const MAX_PLIRON_PROVENANCE_SUBJECTS_V1: usize = 65_536;

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

pub fn analyze_pliron_provenance_alias_v1(
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
    let analysis = collect_pliron_provenance_alias_with_inventory_v1(context, &inventory)?;
    for memory_space in [
        MemorySpaceAttr::Private,
        MemorySpaceAttr::Workgroup,
        MemorySpaceAttr::Global,
    ] {
        analysis.validate_space(memory_space)?;
    }
    Ok(analysis)
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
        let name = format!("{}", view.unique_name(context));
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
