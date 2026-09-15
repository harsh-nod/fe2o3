//! Bounded observations of a rejected checked site, never admission evidence.

use super::*;
use std::fmt::Write as _;

const TYPE_BYTES: usize = 2048;
const STATEMENT_BYTES: usize = 1024;
const MAX_STATEMENTS: usize = 4;

struct BoundedText {
    text: String,
    limit: usize,
    truncated: bool,
}

impl BoundedText {
    fn new(limit: usize) -> Self {
        Self {
            text: String::new(),
            limit,
            truncated: false,
        }
    }

    fn finish(self) -> String {
        if self.truncated {
            self.text + " [truncated]"
        } else {
            self.text
        }
    }
}

impl fmt::Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Err(fmt::Error);
        }
        let available = self.limit.saturating_sub(self.text.len());
        if value.len() <= available {
            self.text.push_str(value);
            return Ok(());
        }
        let mut end = available;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.text.push_str(&value[..end]);
        self.truncated = true;
        Err(fmt::Error)
    }
}

fn observed_types(
    base: u32,
    result_type: u32,
    projections: &[Option<TypedProjection>; MAX_PROJECTIONS],
) -> Vec<u32> {
    let mut result = Vec::with_capacity(MAX_PROJECTIONS + 2);
    for ty in [base, result_type].into_iter().chain(
        projections
            .iter()
            .flatten()
            .map(|projection| projection.output),
    ) {
        if !result.contains(&ty) {
            result.push(ty);
        }
    }
    result
}

fn type_text(
    semantic: &AdmittedInertSemanticMirV1,
    diagnostic: &UnrankedDereferenceDiagnosticV1,
) -> String {
    let mut output = BoundedText::new(TYPE_BYTES);
    for ty in observed_types(
        diagnostic.base_type,
        diagnostic.result_type,
        &diagnostic.projections,
    ) {
        let Some(declaration) = semantic.types().get(ty as usize) else {
            if write!(output, "; retained_type={ty} unavailable").is_err() {
                break;
            }
            continue;
        };
        if write!(
            output,
            "; retained_type={ty} identity={} layout={} size={:?} shape={:?}",
            crate::encode_hex(declaration.identity().as_bytes()),
            crate::encode_hex(declaration.layout_identity().as_bytes()),
            declaration.layout().size_bytes(),
            declaration.shape(),
        )
        .is_err()
        {
            break;
        }
    }
    output.finish()
}

fn statement_text(
    function: &SemanticFunctionDeclV1,
    site: ProjectedSemanticAccessSiteV1,
) -> String {
    let mut output = BoundedText::new(STATEMENT_BYTES);
    let Some((block, start)) = function.blocks().get(site.block).zip(site.statement) else {
        return output.finish();
    };
    if let Some(tail) = block.statements().get(start..) {
        for (offset, statement) in tail.iter().take(MAX_STATEMENTS).enumerate() {
            if write!(
                output,
                "; retained_statement={} kind={:?}",
                start + offset,
                statement.kind()
            )
            .is_err()
            {
                break;
            }
        }
    }
    output.finish()
}

pub(super) fn capture(
    semantic: &AdmittedInertSemanticMirV1,
    view: &SemanticExpandedRootV1,
    site: ProjectedSemanticAccessSiteV1,
    diagnostic: &UnrankedDereferenceDiagnosticV1,
) -> String {
    if diagnostic.function != view.body().identity() || diagnostic.block != site.block {
        return String::new();
    }
    type_text(semantic, diagnostic) + &statement_text(view.body(), site)
}

#[cfg(test)]
mod tests;
