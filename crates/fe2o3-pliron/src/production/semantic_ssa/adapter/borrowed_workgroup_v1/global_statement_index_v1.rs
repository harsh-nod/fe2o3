//! Necessary-key dispatch only; selected facts still run the complete matcher.
use super::*;

pub(super) struct GlobalStatementIndex<'a> {
    facts: &'a [GlobalBf16BorrowV1],
    captures: BTreeMap<SemanticTypeIdV1, Vec<usize>>,
    metadata: BTreeMap<SemanticTypeIdV1, Vec<usize>>,
}

impl<'a> GlobalStatementIndex<'a> {
    pub(super) fn new(
        facts: &'a [GlobalBf16BorrowV1],
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let mut this = Self {
            facts,
            captures: BTreeMap::new(),
            metadata: BTreeMap::new(),
        };
        for (index, fact) in facts.iter().enumerate() {
            budget.charge(1)?;
            let pairs = fact.pairs();
            for (table, key) in [
                (&mut this.captures, pairs[0].1),
                (&mut this.metadata, pairs[2].1),
            ] {
                // Pessimistic distinct-key comparisons plus entry/append work.
                // This does not assume a particular standard-library tree fanout.
                budget.charge(table.len().saturating_add(2))?;
                table.entry(key).or_default().push(index);
            }
        }
        Ok(this)
    }

    pub(super) fn find<T>(
        &self,
        statement: &SemanticStatementKindV1,
        budget: &mut Budget,
        mut accept: impl FnMut(&GlobalBf16BorrowV1, SemanticLocalIdV1) -> Option<T>,
    ) -> Result<Option<T>, ProductionSemanticSsaErrorV1> {
        if self.facts.is_empty() {
            return Ok(None);
        }
        budget.charge(1)?;
        budget.profile.uses.query(
            statement as *const SemanticStatementKindV1 as usize,
            uses_observation_v1::MatchOwner {
                index: self as *const Self as usize,
                facts: self.facts.as_ptr() as usize,
                fact_count: self.facts.len(),
                keys: [self.captures.len(), self.metadata.len()],
            },
        );
        let SemanticStatementKindV1::Assign(assignment) = statement else {
            return Ok(None);
        };
        let (table, key) = match assignment.value().kind() {
            SemanticRvalueKindV1::Aggregate(_) => (&self.captures, assignment.destination().ty()),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) => {
                let [deref, _field] = place.projections() else {
                    return Ok(None);
                };
                (&self.metadata, deref.result_type())
            }
            _ => return Ok(None),
        };
        budget.charge(table.len().saturating_add(1))?;
        budget.profile.uses.dispatch(table.len().saturating_add(1));
        let Some(indices) = table.get(&key) else {
            return Ok(None);
        };
        budget
            .profile
            .uses
            .bucket(indices.as_ptr() as usize, indices.len());
        // Original callable order and first accepted match are preserved. Keys
        // are only necessary conditions, never substitutes for these checks.
        for &index in indices {
            budget.charge(16)?;
            let fact = &self.facts[index];
            let local = fact
                .captured_reference(statement)
                .or_else(|| fact.metadata_reference(statement));
            budget.profile.uses.matcher(index, local.is_some());
            if let Some(local) = local {
                let value = accept(fact, local);
                budget.profile.uses.callback(index, value.is_some());
                if let Some(value) = value {
                    return Ok(Some(value));
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
#[path = "global_statement_index_v1/tests.rs"]
mod tests;
