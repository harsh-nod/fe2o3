use super::*;
use std::io::{self, Write};

#[cfg(test)]
mod tests;

const SELECTOR_ENV: &str = "FE2O3_TRACE_STORAGE_LOCAL";

// Diagnostic selection only: body/local/query-block/query-statement. Nothing
// retained here is returned to promotion, event construction or admission.
pub(super) struct Observation {
    body: [u8; 32],
    local: u32,
    query: SemanticTransparentBorrowSiteV1,
    queried_borrow: Option<(u32, Option<bool>)>,
    first: Option<Demotion>,
}

struct Demotion {
    block: u32,
    statement: Option<u32>,
    kind: &'static str,
    transparent_borrow: Option<bool>,
}

impl Observation {
    pub(super) fn from_env(function: &SemanticFunctionDeclV1) -> Option<Self> {
        Self::selected(function, &std::env::var(SELECTOR_ENV).ok()?)
    }

    pub(super) fn selected(function: &SemanticFunctionDeclV1, raw: &str) -> Option<Self> {
        if raw.len() > 97 {
            return None;
        }
        let mut parts = raw.split('/');
        let identity = parts.next()?;
        if identity.len() != 64 {
            return None;
        }
        let mut body = [0; 32];
        for (out, pair) in body.iter_mut().zip(identity.as_bytes().chunks_exact(2)) {
            *out = hex(pair[0])? * 16 + hex(pair[1])?;
        }
        let local = coordinate(parts.next()?)?;
        let block = coordinate(parts.next()?)?;
        let statement = coordinate(parts.next()?)?;
        if parts.next().is_some()
            || body != *function.identity().as_bytes()
            || local as usize >= function.locals().len()
        {
            return None;
        }
        Some(Self {
            body,
            local,
            query: SemanticTransparentBorrowSiteV1 { block, statement },
            queried_borrow: None,
            first: None,
        })
    }

    pub(super) fn value(&self, promotable: &[bool]) -> Option<bool> {
        promotable.get(self.local as usize).copied()
    }

    pub(super) fn selected_site(
        &self,
        function: &SemanticFunctionDeclV1,
    ) -> Option<(u32, SemanticTransparentBorrowSiteV1)> {
        (self.body == *function.identity().as_bytes()
            && (self.local as usize) < function.locals().len())
            .then_some((self.local, self.query))
    }

    pub(super) fn statement(
        &mut self,
        site: SemanticTransparentBorrowSiteV1,
        kind: &SemanticStatementKindV1,
        transparent: Option<bool>,
        before: Option<bool>,
        promotable: &[bool],
    ) {
        let (name, borrowed) = match kind {
            SemanticStatementKindV1::Assign(assignment) => match assignment.value().kind() {
                SemanticRvalueKindV1::Borrow { place, .. } => {
                    if site == self.query {
                        self.queried_borrow = Some((place.local().index(), transparent));
                    }
                    ("assign-borrow", transparent)
                }
                SemanticRvalueKindV1::AddressOf { .. } => ("assign-address-of", None),
                SemanticRvalueKindV1::Load(_) => ("assign-load", None),
                _ => ("assign", None),
            },
            SemanticStatementKindV1::Store(_) => ("store", None),
            SemanticStatementKindV1::AtomicRmw(_) => ("atomic-rmw", None),
            SemanticStatementKindV1::AtomicCompareExchange(_) => ("atomic-compare-exchange", None),
            SemanticStatementKindV1::SetDiscriminant { .. } => ("set-discriminant", None),
            SemanticStatementKindV1::Deinitialize(_) => ("deinitialize", None),
            SemanticStatementKindV1::Assume(_) => ("assume", None),
            SemanticStatementKindV1::StorageLive(_) => ("storage-live", None),
            SemanticStatementKindV1::StorageDead(_) => ("storage-dead", None),
            SemanticStatementKindV1::Nop => ("nop", None),
        };
        self.record(
            site.block,
            Some(site.statement),
            name,
            borrowed,
            before,
            promotable,
        );
    }

    pub(super) fn terminator(
        &mut self,
        block: u32,
        kind: &SemanticTerminatorKindV1,
        before: Option<bool>,
        promotable: &[bool],
    ) {
        let name = match kind {
            SemanticTerminatorKindV1::Call(_) => "call",
            SemanticTerminatorKindV1::Drop { .. } => "drop",
            _ => "other-terminator",
        };
        self.record(block, None, name, None, before, promotable);
    }

    fn record(
        &mut self,
        block: u32,
        statement: Option<u32>,
        kind: &'static str,
        transparent_borrow: Option<bool>,
        before: Option<bool>,
        promotable: &[bool],
    ) {
        if self.first.is_none() && before == Some(true) && self.value(promotable) == Some(false) {
            self.first = Some(Demotion {
                block,
                statement,
                kind,
                transparent_borrow,
            });
        }
    }

    pub(super) fn emit(&self) {
        let _ = self.write_to(&mut io::stderr().lock());
    }

    fn write_to(&self, out: &mut impl Write) -> io::Result<()> {
        write!(out, "capability-ssa-storage-observation body=")?;
        for byte in &self.body {
            write!(out, "{byte:02x}")?;
        }
        write!(
            out,
            " local={} query={}:{}",
            self.local, self.query.block, self.query.statement
        )?;
        match self.queried_borrow {
            Some((local, transparent)) => {
                write!(
                    out,
                    " queried-local={local} queried-transparent={transparent:?}"
                )?;
            }
            None => write!(out, " queried-borrow=unavailable")?,
        }
        match &self.first {
            Some(first) => write!(
                out,
                " first-block={} first-statement={:?} first-kind={} first-transparent={:?}",
                first.block, first.statement, first.kind, first.transparent_borrow,
            )?,
            None => write!(out, " first-demotion=unavailable")?,
        }
        writeln!(out, " diagnostic-only=true")
    }
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn coordinate(text: &str) -> Option<u32> {
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    text.parse().ok()
}
