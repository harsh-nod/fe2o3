//! Structured content commitment, separate from deterministic node ID seeds.

use super::*;

struct ContentHash<'a> {
    hash: Sha256,
    budget: &'a mut Budget,
}
impl ContentHash<'_> {
    fn bytes(&mut self, value: &[u8]) -> Result<()> {
        self.budget.work(value.len())?;
        self.hash.update(value);
        Ok(())
    }
    fn u32(&mut self, value: u32) -> Result<()> {
        self.bytes(&value.to_le_bytes())
    }
    fn count(&mut self, value: usize) -> Result<()> {
        let value = u32::try_from(value).map_err(|_| {
            SemanticCallExpansionErrorV1::Limit(SemanticCallExpansionResourceV1::Work)
        })?;
        self.u32(value)
    }
    fn optional(&mut self, value: Option<u32>) -> Result<()> {
        self.bytes(&[u8::from(value.is_some())])?;
        if let Some(value) = value {
            self.u32(value)?;
        }
        Ok(())
    }
}

pub(super) fn root_content_identity(
    source: &AdmittedInertSemanticMirV1,
    root: &SemanticExpandedRootV1,
    budget: &mut Budget,
) -> Result<[u8; 32]> {
    // Reserve both encoding and hashing work before the fragment encoder can
    // allocate. Its output is temporary and never becomes an admitted document.
    budget.work(b"FE2O3/SEMANTIC-FUNCTION-FRAGMENT/V1\0".len())?;
    let remaining = budget.limits.work - budget.used[5];
    let (function_digest, length) = canonical_semantic_function_fragment_sha256_v1(
        &root.body,
        source.wire_version(),
        (remaining / 2) as u64,
    )
    .map_err(|error| match error {
        SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        } => SemanticCallExpansionErrorV1::Limit(SemanticCallExpansionResourceV1::Work),
        error => SemanticCallExpansionErrorV1::Model(error),
    })?;
    budget.work(length * 2)?;
    let mut content = ContentHash {
        hash: Sha256::new(),
        budget,
    };
    content.bytes(b"FE2O3/EXPANDED-ROOT-CONTENT/V1\0")?;
    content.bytes(source.semantic_sha256().as_bytes())?;
    content.u32(root.root.index())?;
    content.u32(root.source_body.index())?;
    content.count(length)?;
    content.bytes(&function_digest)?;
    content.count(root.instances.len())?;
    for instance in &root.instances {
        content.u32(instance.function.index())?;
        content.bytes(instance.function_identity.as_bytes())?;
        content.optional(instance.parent.map(|id| id.0))?;
        content.optional(instance.call_block.map(SemanticBlockIdV1::index))?;
        for value in [
            instance.local_start,
            instance.local_count,
            instance.block_start,
            instance.block_count,
        ] {
            content.u32(value)?;
        }
        content.count(instance.depth)?;
    }
    content.count(root.local_origins.len())?;
    for origin in &root.local_origins {
        content.u32(origin.instance.0)?;
        content.u32(origin.function.index())?;
        content.u32(origin.local.index())?;
    }
    content.count(root.block_origins.len())?;
    for origin in &root.block_origins {
        content.u32(origin.instance.0)?;
        content.u32(origin.function.index())?;
        content.u32(origin.block.index())?;
        content.count(origin.statements.len())?;
        for statement in &origin.statements {
            match *statement {
                SemanticExpandedStatementOriginV1::Source { statement } => {
                    content.bytes(&[0])?;
                    content.u32(statement)?;
                }
                SemanticExpandedStatementOriginV1::ParameterTransfer { callee, argument } => {
                    content.bytes(&[1])?;
                    content.u32(callee.0)?;
                    content.u32(argument)?;
                }
                SemanticExpandedStatementOriginV1::ReturnTransfer { callee } => {
                    content.bytes(&[2])?;
                    content.u32(callee.0)?;
                }
                SemanticExpandedStatementOriginV1::FrameStorageLive { callee, local } => {
                    content.bytes(&[3])?;
                    content.u32(callee.0)?;
                    content.u32(local.index())?;
                }
                SemanticExpandedStatementOriginV1::FrameStorageDead { callee, local } => {
                    content.bytes(&[4])?;
                    content.u32(callee.0)?;
                    content.u32(local.index())?;
                }
            }
        }
        match origin.terminator {
            SemanticExpandedTerminatorOriginV1::Source => content.bytes(&[0])?,
            SemanticExpandedTerminatorOriginV1::CallEntry { callee } => {
                content.bytes(&[1])?;
                content.u32(callee.0)?;
            }
            SemanticExpandedTerminatorOriginV1::CallReturn { callee } => {
                content.bytes(&[2])?;
                content.u32(callee.0)?;
            }
        }
    }
    Ok(content.hash.finalize().into())
}
