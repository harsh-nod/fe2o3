//! Target-neutral resource estimates for optimizer transforms.
//!
//! Cost decisions are deterministic scheduling advice. They are not semantic
//! evidence and do not authorize target-specific lowering.

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TargetNeutralCostTransformV1 {
    FullLoopUnrolling,
    PartialLoopUnrolling,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetNeutralCostQueryV1 {
    transform: TargetNeutralCostTransformV1,
    original_operations: usize,
    added_operations: usize,
    removed_operations: usize,
    trip_count: Option<u64>,
}

impl TargetNeutralCostQueryV1 {
    pub const fn new(
        transform: TargetNeutralCostTransformV1,
        original_operations: usize,
        added_operations: usize,
        removed_operations: usize,
        trip_count: Option<u64>,
    ) -> Self {
        Self {
            transform,
            original_operations,
            added_operations,
            removed_operations,
            trip_count,
        }
    }

    pub const fn transform(self) -> TargetNeutralCostTransformV1 {
        self.transform
    }

    pub const fn original_operations(self) -> usize {
        self.original_operations
    }

    pub const fn added_operations(self) -> usize {
        self.added_operations
    }

    pub const fn removed_operations(self) -> usize {
        self.removed_operations
    }

    pub const fn trip_count(self) -> Option<u64> {
        self.trip_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetNeutralCostRejectionV1 {
    ArithmeticOverflow,
    AddedOperationLimit { required: usize, limit: usize },
    OutputOperationLimit { required: usize, limit: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetNeutralCostDecisionV1 {
    query: TargetNeutralCostQueryV1,
    estimated_output_operations: Option<usize>,
    rejection: Option<TargetNeutralCostRejectionV1>,
}

impl TargetNeutralCostDecisionV1 {
    pub const fn query(self) -> TargetNeutralCostQueryV1 {
        self.query
    }

    pub const fn estimated_output_operations(self) -> Option<usize> {
        self.estimated_output_operations
    }

    pub const fn rejection(self) -> Option<TargetNeutralCostRejectionV1> {
        self.rejection
    }

    pub const fn is_admitted(self) -> bool {
        self.rejection.is_none()
    }

    pub const fn grants_semantic_authority(self) -> bool {
        false
    }

    pub const fn grants_target_authority(self) -> bool {
        false
    }
}

pub trait TargetNeutralCostModelV1 {
    fn evaluate(&self, query: TargetNeutralCostQueryV1) -> TargetNeutralCostDecisionV1;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundedTargetNeutralCostModelV1 {
    max_added_operations: usize,
    max_output_operations: usize,
}

impl BoundedTargetNeutralCostModelV1 {
    pub const fn new(max_added_operations: usize, max_output_operations: usize) -> Self {
        Self {
            max_added_operations,
            max_output_operations,
        }
    }
}

impl TargetNeutralCostModelV1 for BoundedTargetNeutralCostModelV1 {
    fn evaluate(&self, query: TargetNeutralCostQueryV1) -> TargetNeutralCostDecisionV1 {
        let estimated_output_operations = query
            .original_operations
            .checked_add(query.added_operations)
            .and_then(|operations| operations.checked_sub(query.removed_operations));
        let rejection = if query.added_operations > self.max_added_operations {
            Some(TargetNeutralCostRejectionV1::AddedOperationLimit {
                required: query.added_operations,
                limit: self.max_added_operations,
            })
        } else if let Some(required) = estimated_output_operations {
            (required > self.max_output_operations).then_some(
                TargetNeutralCostRejectionV1::OutputOperationLimit {
                    required,
                    limit: self.max_output_operations,
                },
            )
        } else {
            Some(TargetNeutralCostRejectionV1::ArithmeticOverflow)
        };
        TargetNeutralCostDecisionV1 {
            query,
            estimated_output_operations,
            rejection,
        }
    }
}
