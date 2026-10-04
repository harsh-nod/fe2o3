//! Executed CFG model receipt. Exact source/nominal policy remains retained;
//! receipt bytes and embedded keys cannot independently construct this owner.
//!
//! ```compile_fail
//! use fe2o3_verifier::{ExecutedMixedWorklistCfgRefinementV27, ExecutedMixedPureCseCfgRefinementV27};
//! fn relabel<'h, 'v, 's>(old: ExecutedMixedWorklistCfgRefinementV27<'h, 'v, 's>)
//!     -> ExecutedMixedPureCseCfgRefinementV27<'h, 'v, 's> { old }
//! ```

use super::*;
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
use std::time::{Duration, Instant};

const MAGIC: &[u8] = b"FE2O3/MIXED/CONDITIONAL-CFG/V27\0";
const EXECUTION_DOMAIN: &[u8] = b"FE2O3/V18/CONDITIONAL-CFG/EXECUTION/V27\0";
const UNSIGNED: usize = MAGIC.len() + 6 + 2 * 32 + 2 * (32 + 8) + 3 * 32 + 5 * 32 + 2 * 32 + 2 * 8;
const WIRE: usize = UNSIGNED + 64;

#[derive(Clone, Copy)]
struct Expected {
    policy: u16,
    source_semantic: [u8; 32],
    source_ssa: [u8; 32],
    input: [u8; 32],
    input_length: u64,
    output: [u8; 32],
    output_length: u64,
    optimizer_execution: [u8; 32],
    statement: [u8; 32],
    generated: [u8; 32],
    toolchain: [[u8; 32]; 5],
    proof_execution: [u8; 32],
    key: [u8; 32],
    functions: u64,
    blocks: u64,
}
impl Expected {
    fn for_request(
        request: &Prepared<'_, '_, '_>,
        toolchain: VerusToolchainIdentityV2,
        proof_execution: [u8; 32],
        key: [u8; 32],
    ) -> Result<Self> {
        let subject = request.subject;
        Ok(Self {
            policy: subject.policy,
            source_semantic: subject.source_semantic,
            source_ssa: subject.source_ssa,
            input: *subject.input.digest(),
            input_length: subject.input.canonical_length(),
            output: *subject.output.digest(),
            output_length: subject.output.canonical_length(),
            optimizer_execution: subject.execution,
            statement: subject.statement,
            generated: request.generated.identity().as_bytes(),
            toolchain: [
                *toolchain.verus_executable().as_bytes(),
                *toolchain.verus_configuration().as_bytes(),
                *toolchain.solver_executable().as_bytes(),
                *toolchain.solver_configuration().as_bytes(),
                *toolchain.runtime_closure().as_bytes(),
            ],
            proof_execution,
            key,
            functions: subject
                .functions
                .try_into()
                .map_err(|_| Resource::Arithmetic)?,
            blocks: subject
                .blocks
                .try_into()
                .map_err(|_| Resource::Arithmetic)?,
        })
    }
    fn unsigned(&self) -> [u8; UNSIGNED] {
        let mut bytes = [0; UNSIGNED];
        let mut cursor = 0;
        let mut put = |value: &[u8]| {
            bytes[cursor..cursor + value.len()].copy_from_slice(value);
            cursor += value.len();
        };
        put(MAGIC);
        put(&27u16.to_le_bytes());
        put(&[2, 1]); // Conditional shared-operator CFG boundary; executed proved result.
        put(&self.policy.to_le_bytes());
        put(&self.source_semantic);
        put(&self.source_ssa);
        put(&self.input);
        put(&self.input_length.to_le_bytes());
        put(&self.output);
        put(&self.output_length.to_le_bytes());
        put(&self.optimizer_execution);
        put(&self.statement);
        put(&self.generated);
        for digest in &self.toolchain {
            put(digest);
        }
        put(&self.proof_execution);
        put(&self.key);
        put(&self.functions.to_le_bytes());
        put(&self.blocks.to_le_bytes());
        debug_assert_eq!(cursor, UNSIGNED);
        bytes
    }
}
fn import(expected: &Expected, wire: &[u8]) -> Result<()> {
    if wire.len() != WIRE {
        return Err(Error::Receipt("exact V27 CFG receipt length"));
    }
    if !matches!(expected.policy, 9 | 10)
        || expected.functions == 0
        || expected.blocks == 0
        || expected.input_length == 0
        || expected.output_length == 0
    {
        return Err(Error::Receipt("nonempty nominal mixed CFG subjects"));
    }
    if wire[..UNSIGNED] != expected.unsigned() {
        return Err(Error::Receipt(
            "exact CFG boundary, policy, source, graph, execution and signer",
        ));
    }
    let key =
        VerifyingKey::from_bytes(&expected.key).map_err(|_| Error::Receipt("CFG signer key"))?;
    if key.is_weak() {
        return Err(Error::Receipt("weak CFG signer key"));
    }
    let signature = Signature::from_slice(&wire[UNSIGNED..])
        .map_err(|_| Error::Receipt("CFG signature framing"))?;
    key.verify_strict(&wire[..UNSIGNED], &signature)
        .map_err(|_| Error::Receipt("strict CFG Ed25519 signature"))
}

struct Executed<'h, 'v, 's> {
    request: Prepared<'h, 'v, 's>,
    expected: Expected,
    wire: [u8; WIRE],
    retained: usize,
    required: usize,
}
impl Executed<'_, '_, '_> {
    fn check(&self, budget: &Budget<'_>) -> Result<()> {
        self.request.check(budget)?;
        self.request.handoff.storage(self.required, budget)?;
        Ok(())
    }
    fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
        let checked = self.check(budget);
        let custody = self.request.handoff.storage(self.required, budget);
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
macro_rules! executed {
    ($name:ident, $execute:ident, $request:ident) => {
        /// Executed conditional N-to-O CFG model, retaining exact source/policy
        /// custody. It does not prove MIR lowering, concrete device operations,
        /// launch-premise discharge or protected compiler/Worker origin.
        #[must_use = "retain or explicitly settle the executed CFG owner"]
        pub struct $name<'h, 'v, 's>(Executed<'h, 'v, 's>);
        impl $name<'_, '_, '_> {
            pub fn subject(&self, budget: &Budget<'_>) -> Result<MixedOptimizerCfgSubjectV27> {
                self.0.check(budget)?;
                Ok(self.0.request.subject)
            }
            pub fn signed_receipt(&self, budget: &Budget<'_>) -> Result<&[u8]> {
                self.0.check(budget)?;
                Ok(&self.0.wire)
            }
            pub fn verifying_key(&self, budget: &Budget<'_>) -> Result<&[u8; 32]> {
                self.0.check(budget)?;
                Ok(&self.0.expected.key)
            }
            pub fn replay_signed_receipt(&self, budget: &mut Budget<'_>) -> Result<()> {
                self.0.check(budget)?;
                budget.charge_work(WIRE + UNSIGNED)?;
                import(&self.0.expected, &self.0.wire)
            }
            pub fn discard(self, budget: &mut Budget<'_>) -> Result<()> {
                self.0.discard(budget)
            }
            pub const fn proves_mir_to_native_lowering(&self) -> bool {
                false
            }
            pub const fn grants_worker_or_launch_authority(&self) -> bool {
                false
            }
        }
        pub fn $execute<'h, 'v, 's>(
            request: $request<'h, 'v, 's>,
            runtime: &Runtime,
            budget: &mut Budget<'_>,
            timeout_seconds: u32,
        ) -> Result<$name<'h, 'v, 's>> {
            execute(request.0, runtime, budget, timeout_seconds).map($name)
        }
    };
}
executed!(
    ExecutedMixedWorklistCfgRefinementV27,
    execute_mixed_worklist_cfg_refinement_v27,
    PreparedMixedWorklistCfgRefinementV27
);
executed!(
    ExecutedMixedPureCseCfgRefinementV27,
    execute_mixed_pure_cse_cfg_refinement_v27,
    PreparedMixedPureCseCfgRefinementV27
);

fn execute<'h, 'v, 's>(
    request: Prepared<'h, 'v, 's>,
    runtime: &Runtime,
    budget: &mut Budget<'_>,
    timeout_seconds: u32,
) -> Result<Executed<'h, 'v, 's>> {
    let mut retained = 0;
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        request.check(budget)?;
        if timeout_seconds == 0 || timeout_seconds > TIMEOUT_LIMIT {
            return Err(Error::Statement("bounded CFG Verus execution timeout"));
        }
        let headers = 2 * OUTPUT_LIMIT + 2 * UNSIGNED + size_of::<Executed<'_, '_, '_>>();
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
            .ok_or(Error::Statement("CFG Verus deadline overflow"))?;
        let mut attempt = runtime.begin_attempt().map_err(Error::Runtime)?;
        let toolchain =
            functional_refinement_verus_toolchain_identity_v2(runtime).map_err(Error::Execution)?;
        let observed = runtime
            .execute_generated_rust_verify(&mut attempt, &request.generated, deadline, OUTPUT_LIMIT)
            .map_err(Error::Runtime)?;
        validate_proved_output(&observed).map_err(Error::Execution)?;
        runtime.revalidate().map_err(Error::Runtime)?;
        if Instant::now() >= deadline {
            return Err(Error::Statement("CFG Verus execution deadline elapsed"));
        }
        let mut digest = Sha256::new();
        digest.update(EXECUTION_DOMAIN);
        digest.update(request.subject.policy.to_le_bytes());
        for bytes in [
            runtime.identity().as_bytes().as_slice(),
            request.generated.identity().as_bytes().as_slice(),
            request.subject.statement.as_slice(),
            observed.stdout.as_slice(),
            observed.stderr.as_slice(),
        ] {
            digest.update(
                u64::try_from(bytes.len())
                    .map_err(|_| Resource::Arithmetic)?
                    .to_le_bytes(),
            );
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
        let mut wire = [0; WIRE];
        wire[..UNSIGNED].copy_from_slice(&unsigned);
        wire[UNSIGNED..].copy_from_slice(&signing.sign(&unsigned).to_bytes());
        import(&expected, &wire)?;
        request.check(budget)?;
        attempt.complete().map_err(Error::Runtime)?;
        Ok((expected, wire))
    }));
    match caught {
        Ok(Ok((expected, wire))) => Ok(Executed {
            request,
            expected,
            wire,
            retained,
            required: budget.storage(),
        }),
        other => {
            let custody = request
                .required
                .checked_add(retained)
                .ok_or(Resource::Arithmetic)
                .and_then(|required| {
                    request
                        .handoff
                        .storage(required, budget)
                        .map_err(|_| Resource::Accounting)
                });
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
                Err(payload) => std::panic::resume_unwind(payload),
                _ => unreachable!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn expected(key: [u8; 32], policy: u16) -> Expected {
        Expected {
            policy,
            source_semantic: [1; 32],
            source_ssa: [2; 32],
            input: [3; 32],
            input_length: 123,
            output: [4; 32],
            output_length: 93,
            optimizer_execution: [5; 32],
            statement: [6; 32],
            generated: [7; 32],
            toolchain: [[8; 32], [9; 32], [10; 32], [11; 32], [12; 32]],
            proof_execution: [13; 32],
            key,
            functions: 2,
            blocks: 4,
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
    fn mixed_cfg_receipt_rejects_every_signed_and_resigned_substitution() {
        let key = SigningKey::from_bytes(&[41; 32]);
        for policy in [9, 10] {
            let expected = expected(key.verifying_key().to_bytes(), policy);
            let wire = sign(&expected, &key);
            import(&expected, &wire).unwrap();
            for byte in 0..WIRE {
                let mut wrong = wire;
                wrong[byte] ^= 1;
                assert!(
                    import(&expected, &wrong).is_err(),
                    "policy {policy}, byte {byte}"
                );
                if byte < UNSIGNED {
                    let signature = key.sign(&wrong[..UNSIGNED]).to_bytes();
                    wrong[UNSIGNED..].copy_from_slice(&signature);
                    assert!(
                        import(&expected, &wrong).is_err(),
                        "resigned policy {policy}, byte {byte}"
                    );
                }
            }
            assert!(import(&expected, &wire[..WIRE - 1]).is_err());
            let mut trailing = wire.to_vec();
            trailing.push(0);
            assert!(import(&expected, &trailing).is_err());
            let foreign = SigningKey::from_bytes(&[42; 32]);
            assert!(import(&expected, &sign(&expected, &foreign)).is_err());
        }
    }
    #[test]
    fn mixed_cfg_receipt_rejects_block_boundary_policy_substitution_and_empty_scope() {
        let key = SigningKey::from_bytes(&[43; 32]);
        let expected = expected(key.verifying_key().to_bytes(), 9);
        for field in 0..7 {
            let mut changed = expected;
            match field {
                0 => changed.policy = 10,
                1 => changed.policy = 3,
                2 => changed.functions = 0,
                3 => changed.blocks = 0,
                4 => changed.input_length = 0,
                5 => changed.output_length = 0,
                _ => changed.optimizer_execution = [99; 32],
            }
            assert!(import(&expected, &sign(&changed, &key)).is_err());
            if (1..=5).contains(&field) {
                assert!(import(&changed, &sign(&changed, &key)).is_err());
            }
        }
        let mut block = sign(&expected, &key);
        block[MAGIC.len() + 2] = 1;
        let signature = key.sign(&block[..UNSIGNED]).to_bytes();
        block[UNSIGNED..].copy_from_slice(&signature);
        assert!(import(&expected, &block).is_err());
    }
}
