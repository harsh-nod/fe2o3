//! Inert measurements on an already rejected, borrowed semantic function.
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticEdgeRoleV1, SemanticFunctionDeclV1, SemanticTerminatorKindV1,
};

#[path = "empty_goto_census_v1/bounded.rs"]
mod bounded;
use bounded::{Edge, Header, Kind, Limits, Source, Stop};

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // Only bounded diagnostic rendering consumes these fields.
pub(crate) struct Observation {
    source_function_identity: [u8; 32],
    source_only_not_ranked_eligibility: bounded::Census,
}

pub(crate) fn capture(function: &SemanticFunctionDeclV1) -> Observation {
    // Separate error-only fuel, derived from existing limits. No successful
    // admission budget or proof state is changed, replenished or reused here.
    let words = fe2o3_kernel_analysis::MAX_RANKED_BOUNDS_OPERATIONS;
    Observation {
        source_function_identity: *function.identity().as_bytes(),
        source_only_not_ranked_eligibility: bounded::capture(
            &Borrowed(function),
            Limits {
                work: words,
                scratch_bytes: words.checked_mul(std::mem::size_of::<usize>()).unwrap_or(0),
            },
        ),
    }
}

struct Borrowed<'a>(&'a SemanticFunctionDeclV1);
impl Source for Borrowed<'_> {
    fn len(&self) -> usize {
        self.0.blocks().len()
    }
    fn entry(&self) -> usize {
        self.0.entry().index() as usize
    }
    fn header(&self, block: usize) -> Header {
        let source = &self.0.blocks()[block];
        let kind = match source.terminator().kind() {
            SemanticTerminatorKindV1::Goto(_) => Kind::Goto,
            SemanticTerminatorKindV1::SwitchInt { .. } => Kind::Switch,
            SemanticTerminatorKindV1::Call(_) => Kind::Call,
            SemanticTerminatorKindV1::Assert { .. } => Kind::Assert,
            SemanticTerminatorKindV1::Drop { .. } => Kind::Drop,
            SemanticTerminatorKindV1::FalseEdge { .. } => Kind::FalseEdge,
            SemanticTerminatorKindV1::Return => Kind::Return,
            SemanticTerminatorKindV1::TailCall(_) => Kind::TailCall,
            SemanticTerminatorKindV1::UnwindResume => Kind::UnwindResume,
            SemanticTerminatorKindV1::UnwindTerminate => Kind::UnwindTerminate,
            SemanticTerminatorKindV1::Abort => Kind::Abort,
            SemanticTerminatorKindV1::Unreachable => Kind::Unreachable,
        };
        Header {
            empty: source.statements().is_empty(),
            kind,
        }
    }
    fn edges(
        &self,
        block: usize,
        visit: &mut dyn FnMut(Edge) -> Result<(), Stop>,
    ) -> Result<(), Stop> {
        self.0.blocks()[block]
            .terminator()
            .kind()
            .try_for_each_edge(|edge| {
                visit(Edge {
                    target: edge.target().index() as usize,
                    goto: edge.role() == SemanticEdgeRoleV1::Goto,
                })
            })
    }
}
