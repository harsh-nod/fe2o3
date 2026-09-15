//! Bounded observations of the existing pass; never read-classification facts.
use super::*;

#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)] // Fields are rendered only by the bounded rejection diagnostic.
pub(in crate::production_ranked_projection_v1) struct Observation {
    pub(super) completed: bool,
    pub(super) remaining_work: usize,
    pub(super) work_exhausted: bool,
    pub(super) locals: usize,
    pub(super) blocks: usize,
    pub(super) admitted_reads: usize,
    pub(super) last_block: Option<usize>,
    pub(super) last_statement: Option<usize>,
    pub(super) phase: flow_failure::Phase,
    pub(super) failure: Option<flow_failure::Failure>,
    pub(super) empty_goto_census: Option<source_cfg_diagnostic_v1::empty_goto_census_v1::Observation>,
    pub(super) terminator: Option<&'static str>,
    pub(super) edges: [Option<Edge>; 2],
    pub(super) edges_truncated: bool,
    pub(super) successor: Option<(usize, Option<u32>)>,
    pub(super) candidates: [Option<Candidate>; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(super) struct Edge {
    target: usize,
    role: SemanticEdgeRoleV1,
    returned: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(super) struct Candidate {
    block: usize,
    statement: usize,
    local: u32,
    value: Kind,
    target: Option<u32>,
    target_value: Kind,
    target_dead: bool,
    target_escaped: bool,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
enum Kind {
    Missing,
    Opaque,
    SharedReference,
    MutableReference,
    Fields {
        count: usize,
        initialized_children: usize,
    },
}

fn kind(value: Option<&Value>) -> Kind {
    match value {
        None => Kind::Missing,
        Some(Value::Opaque | Value::Variants { .. }) => Kind::Opaque,
        Some(Value::Reference(reference)) if reference.mutable => Kind::MutableReference,
        Some(Value::Reference(_)) => Kind::SharedReference,
        Some(Value::Fields(fields)) => Kind::Fields {
            count: fields.len(),
            initialized_children: fields
                .iter()
                .take(MAX_FIELDS)
                .filter(|value| value.is_some())
                .count(),
        },
    }
}

impl Observation {
    pub(super) fn block(&mut self, block: usize) {
        self.last_block = Some(block);
        self.last_statement = None;
        self.terminator = None;
        self.edges = [None; 2];
        self.edges_truncated = false;
        self.successor = None;
    }

    pub(super) fn edge(&mut self, target: usize, role: SemanticEdgeRoleV1, returned: Option<u32>) {
        let edge = Edge { target, role, returned };
        if self.edges.contains(&Some(edge)) {
            return;
        }
        if let Some(slot) = self.edges.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(edge);
        } else {
            self.edges_truncated = true;
        }
    }

    pub(super) fn reject<T>(&mut self, failure: flow_failure::Failure) -> Result<T> {
        self.failure.get_or_insert(failure);
        Err(())
    }

    #[cfg(test)]
    pub(in crate::production_ranked_projection_v1) fn completed_without_exhaustion(&self) -> bool {
        self.completed && !self.work_exhausted
    }

    pub(super) fn candidate(&mut self, site: Site, place: &SemanticPlaceV1, flow: &Flow) {
        let Some(slot) = self.candidates.iter_mut().find(|slot| slot.is_none()) else {
            return;
        };
        let value = flow.values.get(&place.local().index());
        let target = match value {
            Some(Value::Reference(reference)) => Some(reference.target),
            _ => None,
        };
        *slot = Some(Candidate {
            block: site.block,
            statement: site.statement,
            local: place.local().index(),
            value: kind(value),
            target,
            target_value: kind(target.and_then(|target| flow.values.get(&target))),
            target_dead: target.is_some_and(|target| flow.dead.contains(&target)),
            target_escaped: target.is_some_and(|target| flow.escaped.contains(&target)),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{SemanticLocalIdV1, SemanticTypeIdV1};

    #[test]
    fn failure_edges_are_bounded_and_reset_for_the_current_block() {
        let mut observation = Observation::default();
        observation.block(499);
        observation.edge(500, SemanticEdgeRoleV1::CallReturn, Some(7));
        observation.edge(501, SemanticEdgeRoleV1::CallUnwind, None);
        observation.edge(500, SemanticEdgeRoleV1::CallReturn, Some(7));
        assert!(!observation.edges_truncated);
        observation.edge(502, SemanticEdgeRoleV1::Goto, None);
        assert!(observation.edges_truncated);
        assert_eq!(observation.edges.iter().flatten().count(), 2);
        observation.successor = Some((501, None));
        observation.block(500);
        assert_eq!(observation.last_block, Some(500));
        assert_eq!(observation.edges, [None; 2]);
        assert!(!observation.edges_truncated);
        assert_eq!(observation.successor, None);
    }

    #[test]
    fn observation_is_bounded_and_does_not_mutate_custody() {
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![],
            SemanticTypeIdV1::from_index(0),
        )
        .unwrap();
        let flow = Flow {
            values: BTreeMap::from([
                (0, Value::Fields(vec![Some(Value::Opaque), None])),
                (
                    1,
                    Value::Reference(Reference {
                        target: 0,
                        mutable: false,
                        borrow: Site {
                            block: 0,
                            statement: 0,
                        },
                    }),
                ),
            ])
            .into(),
            escaped: BTreeSet::from([0]),
            dead: BTreeSet::from([0]),
        };
        let original = flow.clone();
        let mut observed = Observation::default();
        for statement in 0..100 {
            observed.candidate(
                Site {
                    block: 1,
                    statement,
                },
                &place,
                &flow,
            );
        }
        assert_eq!(flow, original);
        assert_eq!(observed.candidates.iter().flatten().count(), 8);
        for (index, row) in observed.candidates.into_iter().flatten().enumerate() {
            assert_eq!((row.block, row.statement, row.local), (1, index, 1));
            assert_eq!(row.target, Some(0));
            assert!(matches!(row.value, Kind::SharedReference));
            assert!(matches!(
                row.target_value,
                Kind::Fields {
                    count: 2,
                    initialized_children: 1
                }
            ));
            assert!(row.target_dead && row.target_escaped);
        }
    }

    #[test]
    fn work_failure_is_observed_without_changing_the_limit() {
        let mut budget = Budget::new(2);
        assert!(!budget.work_exhausted);
        budget.charge(2).unwrap();
        assert_eq!(budget.remaining, 0);
        assert!(!budget.work_exhausted);
        assert!(budget.charge(1).is_err());
        assert!(budget.work_exhausted);
        assert_eq!(budget.remaining, 0);
    }
}
