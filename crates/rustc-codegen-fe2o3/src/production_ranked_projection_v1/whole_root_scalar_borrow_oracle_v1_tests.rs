//! Test-only complete DATA/resolve comparison; no construction or authority.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticBasicBlockV1, SemanticStatementV1};
pub(in crate::production_ranked_projection_v1) fn compare_retained_original_v1(
    expected: Option<&ScalarPrivateBorrowsV1<'_>>,
    actual: Option<&ScalarPrivateBorrowsV1<'_>>,
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> R<()> {
    let (expected, actual) = match (expected, actual) {
        (None, None) => return Ok(()),
        (Some(expected), Some(actual)) => (expected, actual),
        _ => return Err(malformed()),
    };
    let count = expected
        .locals
        .len()
        .checked_add(expected.starts.len())
        .and_then(|n| n.checked_add(expected.reads.len()))
        .and_then(|n| n.checked_mul(8))
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    charge(facts, count)?;
    if !std::ptr::eq(expected.function, actual.function)
        || !std::ptr::eq(expected.types, actual.types)
        || expected.target != actual.target
        || expected.ledger != actual.ledger
        || expected.locals.len() != actual.locals.len()
        || expected.starts != actual.starts
        || expected.reads.len() != actual.reads.len()
    {
        return Err(malformed());
    }
    for (left, right) in expected.locals.iter().zip(&actual.locals) {
        let candidate =
            |value: Option<Candidate>| value.map(|c| (c.root, c.alias, c.block, c.statement));
        if left.root_alias != right.root_alias
            || candidate(left.candidate) != candidate(right.candidate)
            || left.initialized != right.initialized
            || left.ever_initialized != right.ever_initialized
            || left.alias_alive != right.alias_alive
            || left.blocked != right.blocked
        {
            return Err(malformed());
        }
    }
    for (left, right) in expected.reads.iter().zip(&actual.reads) {
        match (left, right) {
            (None, None) => {}
            (Some(left), Some(right))
                if std::ptr::eq(left.place, right.place) && left.alias == right.alias => {}
            _ => return Err(malformed()),
        }
    }
    // Resolve at each genuine source statement, using the original result's
    // source/root provenance. No candidate result becomes an expected answer.
    for (block, body) in expected.function.blocks().iter().enumerate() {
        for statement in 0..body.statements().len() {
            let ordinal = expected.starts[block]
                .checked_add(statement)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            let Some(read) = expected.reads[ordinal] else {
                continue;
            };
            let candidate = expected.locals[read.alias]
                .candidate
                .ok_or_else(malformed)?;
            let root = SemanticLocalIdV1::from_index(
                u32::try_from(candidate.root).map_err(|_| resource(Resource::Arithmetic))?,
            );
            let site = Site {
                block,
                statement: Some(statement),
            };
            for access in [AccessKindAttr::Read, AccessKindAttr::Write] {
                let left = expected.resolve(
                    expected.function,
                    expected.types,
                    expected.target,
                    site,
                    read.place,
                    access,
                    None,
                    Some(LocalAllocationProvenanceV1::Private(root)),
                    facts,
                )?;
                let right = actual.resolve(
                    expected.function,
                    expected.types,
                    expected.target,
                    site,
                    read.place,
                    access,
                    None,
                    Some(LocalAllocationProvenanceV1::Private(root)),
                    facts,
                )?;
                if left != right {
                    return Err(malformed());
                }
            }
        }
    }
    Ok(())
}
pub(in crate::production_ranked_projection_v1) fn comparison_frame_v1() -> usize {
    std::mem::size_of::<(
        Option<&ScalarPrivateBorrowsV1<'static>>,
        &ScalarPrivateBorrowsV1<'static>,
        &mut dyn ProjectedAssertionFactsV1,
        R<()>,
        R<Option<SemanticLocalIdV1>>,
        &SemanticFunctionDeclV1,
        &[SemanticTypeDeclV1],
        &[SemanticBasicBlockV1],
        &[SemanticStatementV1],
        &Vec<usize>,
        &usize,
        SemanticTargetDataLayoutV1,
        &Local,
        Local,
        &Option<Read<'static>>,
        Option<Read<'static>>,
        Read<'static>,
        Option<Candidate>,
        Candidate,
        Option<(usize, usize, usize, usize)>,
        usize,
        usize,
        usize,
        Option<usize>,
        u32,
        bool,
        Site,
        SemanticLocalIdV1,
        AccessKindAttr,
        [AccessKindAttr; 2],
        std::array::IntoIter<AccessKindAttr, 2>,
        Option<LocalAllocationProvenanceV1>,
        std::slice::Iter<'static, Local>,
        std::slice::Iter<'static, Option<Read<'static>>>,
        std::iter::Zip<std::slice::Iter<'static, Local>, std::slice::Iter<'static, Local>>,
        std::iter::Zip<
            std::slice::Iter<'static, Option<Read<'static>>>,
            std::slice::Iter<'static, Option<Read<'static>>>,
        >,
        std::iter::Enumerate<std::slice::Iter<'static, SemanticBasicBlockV1>>,
        &SemanticBasicBlockV1,
        std::ops::Range<usize>,
        &mut dyn FnMut(Option<Candidate>) -> Option<(usize, usize, usize, usize)>,
        Resource,
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1,
        ProductionRankedProjectionErrorV1,
    )>()
}
