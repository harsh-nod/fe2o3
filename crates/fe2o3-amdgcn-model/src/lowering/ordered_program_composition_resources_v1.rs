//! Prepaid logical envelope around the existing complete-module engine.
//! Not an allocator/RSS meter. Old renderer routes and their policies are unchanged.
use super::*;

pub(super) struct Envelope {
    pub work: usize,
    pub storage: usize,
}
fn add(a: usize, b: usize) -> Result<usize, E> {
    a.checked_add(b).ok_or(Resource::Arithmetic.into())
}
fn mul(a: usize, b: usize) -> Result<usize, E> {
    a.checked_mul(b).ok_or(Resource::Arithmetic.into())
}

pub(super) fn envelope(
    owner: &VerifiedOrderedProgramCompositionV1,
    budget: &mut Budget<'_>,
) -> Result<Envelope, E> {
    let module = owner.canonical().module();
    let bytes = owner.canonical().canonical_bytes().len();
    // The canonical encoding includes every block/operation/result/operand/type/string.
    // This nonallocating census is prepaid before visiting its collections.
    budget.charge_work(add(bytes, 4)?)?;
    let mut blocks = 0usize;
    let mut operations = 0usize;
    for function in &module.functions {
        if let Some(body) = &function.body {
            blocks = add(blocks, body.blocks.len())?;
            for block in &body.blocks {
                operations = add(operations, block.operations.len())?;
            }
        }
    }
    let square = mul(blocks, blocks)?;
    let cube = mul(square, blocks)?;
    // Existing graph algorithms: at most three function plans, eight call edges;
    // CFG fixed points and indexed dominance are conservatively cubic in blocks,
    // with per-operation lookups charged against all blocks. Canonical bytes cover
    // comparisons/formatting of the actual names/types/operands. The fixed text
    // term bounds generated support text and output writes even for tiny inputs.
    let work = add(
        add(mul(bytes, 256)?, mul(cube, 64)?)?,
        add(
            mul(mul(operations, blocks)?, 128)?,
            MAX_COMPILER_MODULE_TEXT_BYTES,
        )?,
    )?;
    // Exact 16 MiB output is prepaid before allocation. The additional engine
    // envelope covers retained maps/bindings, cloned diagnostic/identifier/type
    // payload, indexed CFG sets and transient formatting. It is deliberately
    // conservative logical payload, not a claim about allocator node headers or
    // whole-process/compiler RSS. Checked arithmetic denies before engine entry.
    let storage = add(
        add(
            MAX_COMPILER_MODULE_TEXT_BYTES,
            mul(size_of::<OrderedProgramCompositionCanonicalEmissionV1>(), 2)?,
        )?,
        add(
            add(1024 * 1024, mul(bytes, 256)?)?,
            add(mul(square, 256)?, mul(operations, 512)?)?,
        )?,
    )?;
    Ok(Envelope { work, storage })
}
