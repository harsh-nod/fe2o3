//! A retained policy reference cannot be interpreted as a bare math context.

use fe2o3_mir_model::semantic_mir_v1::{SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeShapeV1};
use std::collections::BTreeSet;

pub(super) fn policy_free_context(
    types: &[SemanticTypeDeclV1],
    context: SemanticTypeIdV1,
    policies: &[SemanticTypeIdV1],
) -> bool {
    let mut pending = vec![context];
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if policies.contains(&id) {
            return false;
        }
        if !visited.insert(id) {
            continue;
        }
        let Some(ty) = types.get(id.index() as usize) else {
            return false;
        };
        match ty.shape() {
            SemanticTypeShapeV1::Pointer(pointer) => pending.push(pointer.pointee()),
            SemanticTypeShapeV1::Aggregate(aggregate)
            | SemanticTypeShapeV1::Tuple(aggregate)
            | SemanticTypeShapeV1::Union(aggregate) => pending.extend(aggregate.fields()),
            SemanticTypeShapeV1::Array { element, .. } | SemanticTypeShapeV1::Slice { element } => {
                pending.push(*element)
            }
            SemanticTypeShapeV1::Enum {
                discriminant,
                variants,
            } => {
                pending.push(*discriminant);
                for variant in variants {
                    pending.extend(variant.fields().fields());
                }
            }
            _ => {}
        }
    }
    true
}

#[cfg(test)]
#[path = "legacy_math/tests.rs"]
mod tests;
