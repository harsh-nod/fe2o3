//! Diagnostic-only bounded expectations for the separate native test observer.
//! This text is not a canonical/source decoder or authority-bearing manifest.
use super::*;
use fe2o3_kernel_ir::{
    Gfx942PhysicalEntryBranchEncodingVNext as Encoding, OperationKind, Terminator,
    gfx942_physical_entry_declaration_v20,
};
use sha2::{Digest, Sha256};
const NATIVE_OBSERVATION_TEXT_BYTES_V20: usize = 16 * 1024;
#[cfg(test)]
#[path = "gfx942_physical_entry_native_observation_v20_tests.rs"]
mod tests;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalEntryNativeObservationStorageV20 {
    retained: usize,
}
impl Gfx942PhysicalEntryNativeObservationStorageV20 {
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
/// Produces inert expectation text from the exact retained canonical owner and
/// its unchanged emitted LLVM. The external test observer never admits source
/// or canonical ownership from this text. Keep actual source/checked owners
/// alive independently; an identity-only sidecar cannot replace their custody.
///
/// Prepays bounded hashing/rendering on the same ledger. The returned text's
/// logical receipt is unreserved; retain it only with normal caller accounting.
pub fn physical_entry_native_observation_input_v20(
    owner: &VerifiedCanonicalKernelIrModuleV20,
    emission: &Gfx942PhysicalEntryCanonicalEmissionV20,
    budget: &mut Budget<'_>,
) -> Result<(String, Gfx942PhysicalEntryNativeObservationStorageV20)> {
    let retained = std::mem::size_of::<String>()
        .checked_add(NATIVE_OBSERVATION_TEXT_BYTES_V20)
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(budget.storage(), 1, 131_072, retained + 4096, |_| {
        if owner.identity() != emission.canonical_identity() {
            return Err(Gfx942PhysicalEntryCanonicalEmissionErrorV20::Profile(
                "native observation canonical content identity mismatch",
            ));
        }
        let declaration = gfx942_physical_entry_declaration_v20(owner).ok_or(
            Gfx942PhysicalEntryCanonicalEmissionErrorV20::Profile(
                "native observation physical profile",
            ),
        )?;
        let function = &owner.module().functions[0];
        let body =
            function
                .body
                .as_ref()
                .ok_or(Gfx942PhysicalEntryCanonicalEmissionErrorV20::Profile(
                    "native observation body",
                ))?;
        let mut text = emit::Text::new(NATIVE_OBSERVATION_TEXT_BYTES_V20)?;
        text.format(format_args!(
            "FE2O3_PHYSICAL_ENTRY_V20_NATIVE_OBSERVATION_INPUT_V1\ncanonical "
        ))?;
        hex(&mut text, owner.identity().digest())?;
        text.format(format_args!(
            " {}\nllvm ",
            owner.identity().canonical_length()
        ))?;
        hex(&mut text, &Sha256::digest(emission.llvm_ir().as_bytes()))?;
        text.format(format_args!(
            " {}\nentry {}\nlaunch 64 1 1 2 1 1\nblocks {}\n",
            emission.llvm_ir().len(),
            function.id.as_str(),
            body.blocks.len()
        ))?;
        let mut native = 0usize;
        let mut steps = 0usize;
        for (index, block) in body.blocks.iter().enumerate() {
            let contract = declaration.blocks[index];
            let count = block
                .operations
                .iter()
                .filter(|op| matches!(op.kind, OperationKind::Gfx942PhysicalEntryStep(_)))
                .count();
            steps += count;
            let native_count = count + usize::from(contract.encoding != Encoding::Fallthrough);
            let edges = match &block.terminator {
                Some(Terminator::ConditionalBranch {
                    then_target,
                    else_target,
                    ..
                }) => [
                    then_target.0.min(else_target.0),
                    then_target.0.max(else_target.0),
                ],
                Some(Terminator::Branch { target, .. }) => [target.0, 255],
                Some(Terminator::Return { .. }) => [255, 255],
                _ => {
                    return Err(Gfx942PhysicalEntryCanonicalEmissionErrorV20::Profile(
                        "native observation CFG",
                    ));
                }
            };
            text.format(format_args!(
                "block {} {} {native} {native_count} {} {}\n",
                block.id.0, contract.encoding as u8, edges[0], edges[1]
            ))?;
            native += native_count;
        }
        if native != usize::from(declaration.native_instruction_count) {
            return Err(Gfx942PhysicalEntryCanonicalEmissionErrorV20::Profile(
                "native observation native census",
            ));
        }
        text.format(format_args!("steps {steps}\n"))?;
        for block in &body.blocks {
            for operation in &block.operations {
                if let OperationKind::Gfx942PhysicalEntryStep(step) = &operation.kind {
                    text.format(format_args!("step {} {}", block.id.0, step.native_ordinal))?;
                    for byte in step.instruction.descriptor() {
                        text.format(format_args!(" {byte}"))?;
                    }
                    text.format(format_args!("\n"))?;
                }
            }
        }
        text.format(format_args!("end\n"))?;
        Ok((
            text.value,
            Gfx942PhysicalEntryNativeObservationStorageV20 { retained },
        ))
    })
}
