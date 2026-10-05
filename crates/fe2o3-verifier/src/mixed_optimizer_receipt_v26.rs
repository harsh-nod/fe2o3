//! A dedicated signed block-simulation boundary. This is not a V2 functional
//! refinement receipt: no CPU reference subjects or MIR->N claim are invented.

use super::{Budget, Error, PreparedMixedOptimizerRefinementV26 as Prepared, Resource, Result};
use crate::functional_refinement_receipt_v2::{
    MAX_FUNCTIONAL_REFINEMENT_VERUS_OUTPUT_BYTES_V2 as OUTPUT_LIMIT,
    MAX_FUNCTIONAL_REFINEMENT_VERUS_TIMEOUT_SECONDS_V2 as TIMEOUT_LIMIT, validate_proved_output,
};
use crate::{
    FunctionalRefinementVerusRuntimeLeaseV1 as Runtime,
    functional_refinement_verus_toolchain_identity_v2,
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use fe2o3_functional_proof::VerusToolchainIdentityV2;
use rand_core::OsRng;
use sha2::{Digest as _, Sha256};
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    time::{Duration, Instant},
};

const MAGIC: &[u8] = b"FE2O3/MIXED/BLOCK/V26\0";
const UNSIGNED: usize = MAGIC.len() + 4 + 2 * (32 + 8) + 32 + 32 + 5 * 32 + 32 + 32 + 8;
const WIRE: usize = UNSIGNED + 64;
const EXECUTION_DOMAIN: &[u8] = b"FE2O3/V18/POLICY9/BLOCK-EXECUTION/V26\0";

#[derive(Clone, Copy)]
struct Expected {
    input: [u8; 32],
    input_length: u64,
    output: [u8; 32],
    output_length: u64,
    statement: [u8; 32],
    generated: [u8; 32],
    toolchain: [[u8; 32]; 5],
    execution: [u8; 32],
    key: [u8; 32],
    blocks: u64,
}
impl Expected {
    fn for_request(
        request: &Prepared<'_, '_, '_>,
        toolchain: VerusToolchainIdentityV2,
        execution: [u8; 32],
        key: [u8; 32],
    ) -> Result<Self> {
        let subject = request.subject;
        Ok(Self {
            input: *subject.input.digest(),
            input_length: subject.input.canonical_length(),
            output: *subject.output.digest(),
            output_length: subject.output.canonical_length(),
            statement: subject.statement,
            generated: request.generated.identity().as_bytes(),
            toolchain: [
                *toolchain.verus_executable().as_bytes(),
                *toolchain.verus_configuration().as_bytes(),
                *toolchain.solver_executable().as_bytes(),
                *toolchain.solver_configuration().as_bytes(),
                *toolchain.runtime_closure().as_bytes(),
            ],
            execution,
            key,
            blocks: subject
                .blocks
                .try_into()
                .map_err(|_| Resource::Arithmetic)?,
        })
    }
    fn unsigned(&self) -> [u8; UNSIGNED] {
        let mut bytes = [0u8; UNSIGNED];
        let mut cursor = 0;
        let mut put = |value: &[u8]| {
            bytes[cursor..cursor + value.len()].copy_from_slice(value);
            cursor += value.len();
        };
        put(MAGIC);
        put(&26u16.to_le_bytes());
        put(&[1, 1]); // Boundary: shared-operator block simulation. Result: proved.
        put(&self.input);
        put(&self.input_length.to_le_bytes());
        put(&self.output);
        put(&self.output_length.to_le_bytes());
        put(&self.statement);
        put(&self.generated);
        for digest in &self.toolchain {
            put(digest);
        }
        put(&self.execution);
        put(&self.key);
        put(&self.blocks.to_le_bytes());
        debug_assert_eq!(cursor, UNSIGNED);
        bytes
    }
}

// The complete expected statement is re-derived from retained producers. The
// embedded key does not choose an admitted signer, and a valid foreign-bound
// signature is still rejected before a proof owner can be constructed.
fn import(expected: &Expected, wire: &[u8]) -> Result<()> {
    if wire.len() != WIRE {
        return Err(Error::Receipt("exact V26 block receipt length"));
    }
    if expected.blocks == 0 || expected.input_length == 0 || expected.output_length == 0 {
        return Err(Error::Receipt("nonempty exact graph subjects"));
    }
    if wire[..UNSIGNED] != expected.unsigned() {
        return Err(Error::Receipt(
            "boundary, result, subjects, generated source, execution or signer",
        ));
    }
    let key = VerifyingKey::from_bytes(&expected.key).map_err(|_| Error::Receipt("signer key"))?;
    if key.is_weak() {
        return Err(Error::Receipt("weak signer key"));
    }
    let signature = Signature::from_slice(&wire[UNSIGNED..])
        .map_err(|_| Error::Receipt("signature framing"))?;
    key.verify_strict(&wire[..UNSIGNED], &signature)
        .map_err(|_| Error::Receipt("strict Ed25519 signature"))
}

/// Move-only result of executing exactly the source-owned generated statement.
/// Its common-operator and block-entry assumptions remain explicit. Generic
/// CFG/interprocedural composition, MIR->N refinement, protected compiler
/// origin and concrete launch premise discharge remain separate obligations.
#[must_use = "retain or explicitly settle the executed block-simulation owner"]
pub struct ExecutedMixedOptimizerBlockSimulationV26<'handoff, 'view, 'source> {
    request: Prepared<'handoff, 'view, 'source>,
    expected: Expected,
    wire: [u8; WIRE],
    retained: usize,
    required: usize,
}
impl ExecutedMixedOptimizerBlockSimulationV26<'_, '_, '_> {
    fn check(&self, budget: &Budget<'_>) -> Result<()> {
        self.request.check(budget)?;
        self.request
            .handoff
            .observe_retained_storage_v18(self.required, budget)?;
        Ok(())
    }
    /// The wire and key are inert transport. There is no public constructor
    /// turning either into this retained executed owner.
    pub fn signed_receipt(&self, budget: &Budget<'_>) -> Result<&[u8]> {
        self.check(budget)?;
        Ok(&self.wire)
    }
    pub fn verifying_key(&self, budget: &Budget<'_>) -> Result<&[u8; 32]> {
        self.check(budget)?;
        Ok(&self.expected.key)
    }
    pub fn replay_signed_receipt(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.check(budget)?;
        budget.charge_work(WIRE + UNSIGNED)?;
        import(&self.expected, &self.wire)
    }
    pub const fn proves_mir_to_native_lowering(&self) -> bool {
        false
    }
    pub const fn grants_worker_or_launch_authority(&self) -> bool {
        false
    }
    pub fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
        let checked = self.check(budget);
        let custody = self
            .request
            .handoff
            .observe_retained_storage_v18(self.required, budget);
        let Self {
            request, retained, ..
        } = self;
        let released = custody.and_then(|()| {
            budget
                .release_storage(retained)
                .map_err(|error| request.source.retain_query_resource_error_v18(error))
        });
        let settled = request.discard(budget);
        checked?;
        released?;
        settled
    }
}

/// Execute the authentic generated program with the admitted retained runtime,
/// revalidate the closure, sign only a successful exact result, strictly import
/// this distinct boundary, and complete the original runtime attempt.
/// Missing runtime admission is a real error, never an unsigned success path.
pub fn execute_mixed_optimizer_block_simulation_v26<'handoff, 'view, 'source>(
    request: Prepared<'handoff, 'view, 'source>,
    runtime: &Runtime,
    budget: &mut Budget<'_>,
    timeout_seconds: u32,
) -> Result<ExecutedMixedOptimizerBlockSimulationV26<'handoff, 'view, 'source>> {
    let mut retained = 0;
    let caught = catch_unwind(AssertUnwindSafe(|| {
        request.check(budget)?;
        if timeout_seconds == 0 || timeout_seconds > TIMEOUT_LIMIT {
            return Err(Error::Statement("bounded Verus execution timeout"));
        }
        let headers = 2 * OUTPUT_LIMIT
            + 2 * UNSIGNED
            + size_of::<ExecutedMixedOptimizerBlockSimulationV26<'_, '_, '_>>();
        budget.reserve_storage(headers)?;
        retained = headers;
        budget.charge_work(
            request
                .generated
                .source()
                .len()
                .checked_add(2 * OUTPUT_LIMIT + 2 * WIRE)
                .ok_or(Resource::Arithmetic)?,
        )?;
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(u64::from(timeout_seconds)))
            .ok_or(Error::Statement("Verus execution deadline overflow"))?;
        let mut attempt = runtime.begin_attempt().map_err(Error::Runtime)?;
        let toolchain =
            functional_refinement_verus_toolchain_identity_v2(runtime).map_err(Error::Execution)?;
        let observed = runtime
            .execute_generated_rust_verify(&mut attempt, &request.generated, deadline, OUTPUT_LIMIT)
            .map_err(Error::Runtime)?;
        validate_proved_output(&observed).map_err(Error::Execution)?;
        runtime.revalidate().map_err(Error::Runtime)?;
        if Instant::now() >= deadline {
            return Err(Error::Statement("Verus execution deadline elapsed"));
        }
        let mut digest = Sha256::new();
        digest.update(EXECUTION_DOMAIN);
        for bytes in [
            runtime.identity().as_bytes().as_slice(),
            request.generated.identity().as_bytes().as_slice(),
            request.subject.statement.as_slice(),
            observed.stdout.as_slice(),
            observed.stderr.as_slice(),
        ] {
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        }
        digest.update(observed.exit_code.unwrap_or(-1).to_le_bytes());
        digest.update(observed.signal.unwrap_or(0).to_le_bytes());
        let signing = SigningKey::generate(&mut OsRng);
        let expected = Expected::for_request(
            &request,
            toolchain,
            digest.finalize().into(),
            signing.verifying_key().to_bytes(),
        )?;
        let unsigned = expected.unsigned();
        let signature = signing.sign(&unsigned).to_bytes();
        let mut wire = [0u8; WIRE];
        wire[..UNSIGNED].copy_from_slice(&unsigned);
        wire[UNSIGNED..].copy_from_slice(&signature);
        import(&expected, &wire)?;
        request.check(budget)?;
        attempt.complete().map_err(Error::Runtime)?;
        Ok((expected, wire))
    }));
    match caught {
        Ok(Ok((expected, wire))) => Ok(ExecutedMixedOptimizerBlockSimulationV26 {
            request,
            expected,
            wire,
            retained,
            required: budget.storage(),
        }),
        other => {
            let custody = request.handoff.observe_retained_storage_v18(
                request
                    .required
                    .checked_add(retained)
                    .ok_or(Resource::Arithmetic)?,
                budget,
            );
            if custody.is_ok() {
                let _ = budget.release_storage(retained);
            }
            let source = request.source;
            let settled = request.discard(budget);
            match other {
                Ok(Err(Error::Resource(error))) => {
                    Err(source.retain_query_resource_error_v18(error).into())
                }
                Ok(Err(error)) => {
                    let _ = settled;
                    Err(error)
                }
                Err(payload) => resume_unwind(payload),
                _ => unreachable!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected(key: [u8; 32]) -> Expected {
        Expected {
            input: [1; 32],
            input_length: 123,
            output: [2; 32],
            output_length: 93,
            statement: [3; 32],
            generated: [4; 32],
            toolchain: [[5; 32], [6; 32], [7; 32], [8; 32], [9; 32]],
            execution: [10; 32],
            key,
            blocks: 3,
        }
    }
    fn sign(expected: &Expected, key: &SigningKey) -> [u8; WIRE] {
        let unsigned = expected.unsigned();
        let mut wire = [0; WIRE];
        wire[..UNSIGNED].copy_from_slice(&unsigned);
        wire[UNSIGNED..].copy_from_slice(&key.sign(&unsigned).to_bytes());
        wire
    }
    #[test]
    fn block_receipt_rejects_every_byte_substitution_and_nonexact_length() {
        let key = SigningKey::from_bytes(&[31; 32]);
        let expected = expected(key.verifying_key().to_bytes());
        let wire = sign(&expected, &key);
        import(&expected, &wire).unwrap();
        for i in 0..WIRE {
            let mut wrong = wire;
            wrong[i] ^= 1;
            assert!(import(&expected, &wrong).is_err(), "byte {i}");
        }
        assert!(import(&expected, &wire[..WIRE - 1]).is_err());
        let mut trailing = wire.to_vec();
        trailing.push(0);
        assert!(import(&expected, &trailing).is_err());
    }
    #[test]
    fn valid_signatures_do_not_override_exact_subjects_or_boundary() {
        let key = SigningKey::from_bytes(&[32; 32]);
        let expected = expected(key.verifying_key().to_bytes());
        for i in 0..UNSIGNED {
            let mut wire = sign(&expected, &key);
            wire[i] ^= 1;
            let signature = key.sign(&wire[..UNSIGNED]).to_bytes();
            wire[UNSIGNED..].copy_from_slice(&signature);
            assert!(import(&expected, &wire).is_err(), "resigned byte {i}");
        }
        let foreign = SigningKey::from_bytes(&[33; 32]);
        assert!(import(&expected, &sign(&expected, &foreign)).is_err());
    }
    #[test]
    fn empty_graph_statement_cannot_become_an_executed_block_claim() {
        let key = SigningKey::from_bytes(&[34; 32]);
        let mut value = expected(key.verifying_key().to_bytes());
        value.blocks = 0;
        assert!(import(&value, &sign(&value, &key)).is_err());
        value.blocks = 1;
        value.input_length = 0;
        assert!(import(&value, &sign(&value, &key)).is_err());
    }
}
