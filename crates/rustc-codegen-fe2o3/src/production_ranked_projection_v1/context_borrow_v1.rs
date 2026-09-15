//! Transport an existing Context origin; never create a root or borrow proof.

use super::{SemanticBorrowKindV1, SemanticPlaceV1, SemanticProjectionKindV1, SemanticTypeIdV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Borrow {
    Owned,
    Shared,
    Exclusive,
}

pub(super) fn reborrow(
    context: SemanticTypeIdV1,
    current: Borrow,
    place: &SemanticPlaceV1,
    kind: SemanticBorrowKindV1,
) -> Option<Borrow> {
    if place.ty() != context {
        return None;
    }
    let exact_place = match current {
        Borrow::Owned => place.projections().is_empty(),
        Borrow::Shared | Borrow::Exclusive => matches!(place.projections(), [projection]
            if matches!(projection.kind(), SemanticProjectionKindV1::Dereference)),
    };
    if !exact_place {
        return None;
    }
    match kind {
        SemanticBorrowKindV1::Shared => Some(Borrow::Shared),
        SemanticBorrowKindV1::Mutable if current != Borrow::Shared => Some(Borrow::Exclusive),
        SemanticBorrowKindV1::Mutable | SemanticBorrowKindV1::Fake => None,
    }
}
