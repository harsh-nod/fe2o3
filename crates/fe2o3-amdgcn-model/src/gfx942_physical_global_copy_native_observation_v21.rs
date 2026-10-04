//! Bounded diagnostic input for the separate native test observer. Not a
//! source/canonical loader and not a host-memory or deployment proof.
use super::*;
use fe2o3_kernel_ir::{
    Gfx942PhysicalEntryBranchEncodingVNext as Encoding, OperationKind, Terminator,
    gfx942_physical_global_copy_declaration_v21,
};
use sha2::{Digest, Sha256};
const NATIVE_OBSERVATION_TEXT_BYTES_V21: usize = 16 * 1024;
#[cfg(test)]
#[path = "gfx942_physical_global_copy_native_observation_v21_tests.rs"]
mod tests;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalGlobalCopyNativeObservationStorageV21 {
    retained: usize,
}
impl Gfx942PhysicalGlobalCopyNativeObservationStorageV21 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}
fn hex(text: &mut emit::Text, bytes: &[u8]) -> Result<()> {
    for byte in bytes {
        text.format(format_args!("{byte:02x}"))?;
    }
    Ok(())
}
/// Joins this immutable owner and its emission, then records actual one-block
/// canonical instruction correspondence. No caller plan, symbol, raw bytes or
/// source identity override is accepted. Retain real source/checked owners
/// independently; these diagnostic bytes cannot replace their custody.
///
/// Uses the same cumulative budget, preserving caller floors and all work/peak
/// and denial history. Reserve the returned receipt while retaining the text.
pub fn physical_global_copy_native_observation_input_v21(
    owner: &VerifiedCanonicalKernelIrModuleV21,
    emission: &Gfx942PhysicalGlobalCopyCanonicalEmissionV21,
    budget: &mut Budget<'_>,
) -> Result<(String, Gfx942PhysicalGlobalCopyNativeObservationStorageV21)> {
    type E = Gfx942PhysicalGlobalCopyCanonicalEmissionErrorV21;
    let retained = std::mem::size_of::<String>()
        .checked_add(NATIVE_OBSERVATION_TEXT_BYTES_V21)
        .ok_or(Resource::Arithmetic)?;
    let scratch = retained.checked_add(4096).ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(budget.storage(), 1, 131_072, scratch, |_| {
        if owner.identity() != emission.canonical_identity() {
            return Err(E::Profile("native observation canonical content identity mismatch"));
        }
        let declaration = gfx942_physical_global_copy_declaration_v21(owner)
            .ok_or(E::Profile("native observation global-copy profile"))?;
        let [function] = owner.module().functions.as_slice() else {
            return Err(E::Profile("native observation function census"));
        };
        let body = function.body.as_ref().ok_or(E::Profile("native observation body"))?;
        let [block] = body.blocks.as_slice() else {
            return Err(E::Profile("native observation block census"));
        };
        if block.id.0 != 0 || declaration.block.encoding != Encoding::Endpgm0
            || !matches!(&block.terminator, Some(Terminator::Return { values }) if values.is_empty())
            || block.operations.is_empty() || block.operations.len() > 32
            || block.operations.len() != usize::from(declaration.native_instruction_count)
        { return Err(E::Profile("native observation actual terminal/native census")); }
        // Also join every immutable relation row to the actual canonical subject.
        let mut correspondence = emission.operations();
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let row = correspondence.next().ok_or(E::Profile("native observation missing correspondence"))?;
            let (site, descriptor, native) = match &operation.kind {
                OperationKind::Gfx942PhysicalGlobalCopyDeclaration(d) if ordinal == 0 =>
                    (d.begin_site, None, None),
                OperationKind::Gfx942PhysicalGlobalCopyStep(s) if ordinal > 0 =>
                    (s.site, Some(s.instruction.descriptor()), Some(s.native_ordinal)),
                _ => return Err(E::Profile("native observation exact operation membership")),
            };
            let mut ids = [None; 5];
            if operation.results.len() > ids.len() { return Err(E::Profile("native observation result census")); }
            for (id, result) in ids.iter_mut().zip(&operation.results) { *id = Some(result.id); }
            if row.block != block.id || usize::from(row.operation_ordinal) != ordinal
                || row.source_site != site || row.results != ids
                || row.instruction_descriptor != descriptor || row.native_ordinal != native
            { return Err(E::Profile("native observation actual operation correspondence")); }
        }
        if correspondence.next().is_some() { return Err(E::Profile("native observation extra correspondence")); }
        let mut text = emit::Text::new(NATIVE_OBSERVATION_TEXT_BYTES_V21)?;
        text.format(format_args!("FE2O3_PHYSICAL_GLOBAL_COPY_V21_NATIVE_OBSERVATION_INPUT_V1\ncanonical "))?;
        hex(&mut text, owner.identity().digest())?;
        text.format(format_args!(" {}\nllvm ", owner.identity().canonical_length()))?;
        hex(&mut text, &Sha256::digest(emission.llvm_ir().as_bytes()))?;
        text.format(format_args!(
            " {}\nentry {}\nlaunch 64 1 1 2 1 1\nblocks 1\nblock 0 4 0 {} 255 255\nsteps {}\n",
            emission.llvm_ir().len(), function.id.as_str(),
            declaration.native_instruction_count, block.operations.len() - 1
        ))?;
        for operation in block.operations.iter().skip(1) {
            let OperationKind::Gfx942PhysicalGlobalCopyStep(step) = &operation.kind else {
                return Err(E::Profile("native observation step"));
            };
            text.format(format_args!("step 0 {}", step.native_ordinal))?;
            for byte in step.instruction.descriptor() { text.format(format_args!(" {byte}"))?; }
            text.format(format_args!("\n"))?;
        }
        text.format(format_args!("end\n"))?;
        Ok((text.value, Gfx942PhysicalGlobalCopyNativeObservationStorageV21 { retained }))
    })
}
