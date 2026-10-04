//! Bounded observations of the actual completed V89/V90 managed path, never authority.

use crate::source_isa_observation::SourceIsaObservationFrameV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1,
};
use fe2o3_runtime_protocol::WorkerV3LoadEnvelopeWireV2;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const KIND: &str = "production-census-v91";
pub(crate) const PREFIX: &str = "[cargo-fe2o3] production-census-v91";
pub(crate) const MAX_BYTES: usize = 128 * 1024;
pub(crate) const MAX_AGGREGATE: usize = 512 * 1024;
const MAX_INPUT: usize = 64 * 1024 * 1024;
const MAX_KERNELS: usize = 256;
const MAX_NAME: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Kernel {
    pub(crate) logical_name: String,
    pub(crate) entry_name: String,
    pub(crate) root: u32,
    pub(crate) output_function: u32,
    pub(crate) contract: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Census {
    pub(crate) schema: String,
    pub(crate) config: [u8; 32],
    pub(crate) unit: [u8; 32],
    pub(crate) session: [u8; 16],
    pub(crate) invocation: [u8; 32],
    pub(crate) generation: u64,
    pub(crate) finalization: [u8; 32],
    pub(crate) target: String,
    pub(crate) descriptor_version: u16,
    pub(crate) typed_receipt_version: u16,
    pub(crate) policy: u16,
    pub(crate) compiler: [[u8; 32]; 6],
    pub(crate) compiler_subject: [u8; 32],
    pub(crate) compiler_receipt: [u8; 32],
    pub(crate) compiler_policy: [u8; 32],
    pub(crate) rustc_invocation: [u8; 32],
    pub(crate) semantic_source: [u8; 32],
    pub(crate) source_ssa: [u8; 32],
    pub(crate) graphs: [([u8; 32], u64); 4],
    pub(crate) typed_execution: [u8; 32],
    pub(crate) proof_runtime: [u8; 32],
    pub(crate) proof_tools: [[u8; 32]; 5],
    pub(crate) generated_source: [u8; 32],
    pub(crate) artifact: [u8; 32],
    pub(crate) artifact_bytes: u64,
    pub(crate) descriptor: [u8; 32],
    pub(crate) kernels: Vec<Kernel>,
    pub(crate) authority: bool,
}

fn error(value: impl std::fmt::Display) -> String {
    value.to_string()
}

impl Census {
    fn validate(&self) -> Result<(), String> {
        if self.schema != KIND
            || self.authority
            || self.policy != 11
            || self.descriptor_version != 89
            || self.typed_receipt_version != 90
            || !matches!(self.target.as_str(), "gfx942" | "gfx950")
            || self.artifact_bytes == 0
            || self.artifact_bytes > MAX_INPUT as u64
            || self.generation == 0
            || self.session == [0; 16]
            || self.kernels.is_empty()
            || self.kernels.len() > MAX_KERNELS
        {
            return Err("invalid production census header".to_owned());
        }
        let hashes = [
            self.config,
            self.unit,
            self.invocation,
            self.finalization,
            self.compiler_subject,
            self.compiler_receipt,
            self.compiler_policy,
            self.rustc_invocation,
            self.semantic_source,
            self.source_ssa,
            self.typed_execution,
            self.proof_runtime,
            self.generated_source,
            self.artifact,
            self.descriptor,
        ];
        if hashes.contains(&[0; 32])
            || self.compiler.contains(&[0; 32])
            || self.proof_tools.contains(&[0; 32])
            || self
                .graphs
                .iter()
                .any(|(hash, len)| *hash == [0; 32] || *len == 0 || *len > MAX_INPUT as u64)
        {
            return Err("empty production census identity".to_owned());
        }
        for (i, kernel) in self.kernels.iter().enumerate() {
            if kernel.contract == [0; 32]
                || kernel.root as usize >= self.kernels.len()
                || [kernel.logical_name.as_str(), kernel.entry_name.as_str()]
                    .iter()
                    .any(|name| {
                        name.is_empty()
                            || name.len() > MAX_NAME
                            || name.bytes().any(|b| !b.is_ascii_graphic())
                    })
                || self.kernels[..i].iter().any(|prior| {
                    prior.entry_name == kernel.entry_name
                        || prior.logical_name == kernel.logical_name
                        || prior.root == kernel.root
                        || prior.output_function == kernel.output_function
                })
            {
                return Err("invalid production kernel census".to_owned());
            }
        }
        Ok(())
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        // The shape is bounded before serialization, including every variable-length name.
        let bytes = serde_json::to_vec(self).map_err(error)?;
        if bytes.len() > MAX_BYTES {
            return Err("production census byte bound".to_owned());
        }
        Ok(bytes)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > MAX_BYTES {
            return Err("production census byte bound".to_owned());
        }
        let value: Self = serde_json::from_slice(bytes).map_err(error)?;
        if value.encode()? != bytes {
            return Err("noncanonical production census".to_owned());
        }
        Ok(value)
    }

    pub(crate) fn check_frame(&self, frame: &SourceIsaObservationFrameV1) -> Result<(), String> {
        let context = frame.context();
        let attempt = context.attempt();
        if self.config != context.config()
            || self.unit != context.unit()
            || self.session != *attempt.session().as_bytes()
            || self.invocation != *attempt.invocation().as_bytes()
            || self.generation != attempt.generation()
            || self.finalization != context.finalization()
        {
            return Err("production census differs from brokered occurrence".to_owned());
        }
        Ok(())
    }
}

/// Called only with a live/recovered load-envelope owner after profile validation.
/// Parsing and signatures here are observation checks, not a substitute for that owner.
pub(crate) fn snapshot(
    config: [u8; 32],
    unit: [u8; 32],
    wire: &WorkerV3LoadEnvelopeWireV2,
    artifact: &[u8],
) -> Result<Census, String> {
    let replay = wire.replay();
    if artifact.is_empty() || artifact.len() > MAX_INPUT || replay.outer_handoff().len() > MAX_INPUT
    {
        return Err("production census input bound".to_owned());
    }
    let handoff =
        fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3::decode(replay.outer_handoff())
            .map_err(error)?;
    let subject = wire
        .reconstructed_compiler_execution_subject_v1()
        .map_err(error)?;
    let closure = subject.compiler_closure();
    let receipts = handoff.capsule().receipts();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 256 * 1024 * 1024);
    let scratch = fe2o3_hsaco_finalize::NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V89
        + fe2o3_compiler_lineage::MIXED_MIDDLE_END_WORKING_STORAGE_V90;
    budget.reserve_storage(scratch).map_err(error)?;
    let lineage = fe2o3_compiler_lineage::read_mixed_middle_end_v90(
        receipts.middle_end().canonical_preimage(),
        fe2o3_compiler_lineage::MIXED_MIDDLE_END_WORKING_STORAGE_V90,
        |n| budget.charge_work(n),
    )
    .map_err(error)?;
    let input = lineage.input();
    if input.semantic_mir != receipts.semantic_mir().canonical_preimage()
        || input.forwarded != receipts.kernel_ir().canonical_preimage()
    {
        return Err("production census lineage receipt mismatch".to_owned());
    }
    let typed = fe2o3_verifier::check_inert_predicated_typed_source_receipt_v90(input, &mut budget)
        .map_err(error)?;
    let inspection = fe2o3_hsaco_finalize::inspect_finalized_nominal_hsaco_v89(
        artifact,
        fe2o3_hsaco_finalize::NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V89,
        &mut |n| budget.charge_work(n),
    )
    .map_err(error)?;
    let table = inspection.descriptor_table();
    if table.kernel_count() == 0 || table.kernel_count() > MAX_KERNELS {
        return Err("production census kernel bound".to_owned());
    }
    let graphs = typed.graph_identities();
    budget
        .charge_work(
            artifact
                .len()
                .checked_add(table.canonical_bytes().len())
                .ok_or_else(|| "production census hash extent overflow".to_owned())?,
        )
        .map_err(error)?;
    let mut kernels = Vec::new();
    kernels
        .try_reserve_exact(table.kernel_count())
        .map_err(error)?;
    for i in 0..table.kernel_count() {
        let kernel = table
            .kernel(i, &mut |n| budget.charge_work(n))
            .map_err(error)?;
        let contract = table
            .contract(i, &mut |n| budget.charge_work(n))
            .map_err(error)?;
        let subjects = contract.subjects();
        if subjects.source_semantic_identity != typed.source_semantic_identity()
            || subjects.original_graph_identity != graphs[0].0
            || subjects.output_graph_identity != graphs[3].0
            || kernel.logical_name().len() > MAX_NAME
            || kernel.entry_name().len() > MAX_NAME
        {
            return Err("production census descriptor graph mismatch".to_owned());
        }
        kernels.push(Kernel {
            logical_name: kernel.logical_name().to_owned(),
            entry_name: kernel.entry_name().to_owned(),
            root: subjects.original_root,
            output_function: subjects.output_function,
            contract: *contract.identity(),
        });
    }
    let attempt = crate::source_isa_observation::inert_source_isa_attempt_v1(subject.attempt())
        .map_err(error)?;
    let value = Census {
        schema: KIND.to_owned(),
        config,
        unit,
        session: *attempt.session().as_bytes(),
        invocation: *attempt.invocation().as_bytes(),
        generation: attempt.generation(),
        finalization: *replay
            .publication_intent_record()
            .plan()
            .finalization()
            .as_bytes(),
        target: table.device_target().to_string(),
        descriptor_version: 89,
        typed_receipt_version: 90,
        policy: 11,
        compiler: [
            closure.cargo_executable_sha256(),
            closure.cargo_binding_trampoline_sha256(),
            closure.cargo_fe2o3_binding_wrapper_sha256(),
            closure.rustc_executable_sha256(),
            closure.rustc_runtime_tree_sha256(),
            closure.codegen_backend_sha256(),
        ],
        compiler_subject: *subject.identity().sha256(),
        compiler_receipt: *wire.compiler_execution_receipt().identity().as_bytes(),
        compiler_policy: *wire
            .compiler_execution_receipt()
            .policy()
            .identity()
            .as_bytes(),
        rustc_invocation: *subject.rustc_invocation_sha256(),
        semantic_source: typed.source_semantic_identity(),
        source_ssa: typed.source_ssa_identity(),
        graphs,
        typed_execution: typed.claimed_execution_identity(),
        proof_runtime: typed.claimed_runtime_identity(),
        proof_tools: typed.claimed_toolchain_identities(),
        generated_source: typed.generated_identity(),
        artifact: Sha256::digest(artifact).into(),
        artifact_bytes: artifact.len() as u64,
        descriptor: Sha256::digest(table.canonical_bytes()).into(),
        kernels,
        authority: false,
    };
    value.validate()?;
    Ok(value)
}

#[cfg(test)]
pub(crate) fn test_census() -> Census {
    Census {
        schema: KIND.to_owned(),
        config: [0x30; 32],
        unit: [0x40; 32],
        session: [0x31; 16],
        invocation: [0x32; 32],
        generation: 3,
        finalization: [0x33; 32],
        target: "gfx942".to_owned(),
        descriptor_version: 89,
        typed_receipt_version: 90,
        policy: 11,
        compiler: [[4; 32]; 6],
        compiler_subject: [5; 32],
        compiler_receipt: [6; 32],
        compiler_policy: [7; 32],
        rustc_invocation: [8; 32],
        semantic_source: [9; 32],
        source_ssa: [10; 32],
        graphs: [([11; 32], 41); 4],
        typed_execution: [12; 32],
        proof_runtime: [13; 32],
        proof_tools: [[14; 32]; 5],
        generated_source: [15; 32],
        artifact: [16; 32],
        artifact_bytes: 500,
        descriptor: [17; 32],
        kernels: vec![Kernel {
            logical_name: "kernel".to_owned(),
            entry_name: "kernel.entry".to_owned(),
            root: 0,
            output_function: 1,
            contract: [18; 32],
        }],
        authority: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_census_roundtrip_is_bounded_canonical_and_non_authoritative() {
        let row = test_census();
        let bytes = row.encode().unwrap();
        assert_eq!(Census::decode(&bytes).unwrap(), row);
        assert!(!row.authority);
        let mut spaced = bytes.clone();
        spaced.push(b' ');
        assert!(Census::decode(&spaced).is_err());
        assert!(Census::decode(&vec![b' '; MAX_BYTES + 1]).is_err());
        for extra in [b",\"unknown\":0".as_slice(), b",\"policy\":11".as_slice()] {
            let mut changed = bytes[..bytes.len() - 1].to_vec();
            changed.extend_from_slice(extra);
            changed.push(b'}');
            assert!(Census::decode(&changed).is_err());
        }
    }

    #[test]
    fn production_census_rejects_legacy_identity_roster_and_authority_substitution() {
        for fault in 0..14 {
            let mut row = test_census();
            match fault {
                0 => row.schema = "source-isa-summary-v1".to_owned(),
                1 => row.descriptor_version = 53,
                2 => row.typed_receipt_version = 50,
                3 => row.policy = 10,
                4 => row.authority = true,
                5 => row.compiler[3] = [0; 32],
                6 => row.proof_tools[2] = [0; 32],
                7 => row.graphs[2].1 = 0,
                8 => row.kernels.clear(),
                9 => row.kernels.push(row.kernels[0].clone()),
                10 => row.kernels[0].root = 1,
                11 => row.kernels[0].entry_name.push('\n'),
                12 => row.kernels[0].logical_name = "a".repeat(MAX_NAME + 1),
                13 => row.artifact_bytes = MAX_INPUT as u64 + 1,
                _ => unreachable!(),
            }
            assert!(row.encode().is_err(), "fault {fault}");
        }
    }

    #[test]
    fn production_census_binds_every_broker_occurrence_axis() {
        use crate::source_isa_observation::*;
        let row = test_census();
        let attempt = SourceIsaObservationAttemptV1::new(
            row.generation,
            SourceIsaObservationSessionV1::from_bytes(row.session),
            SourceIsaObservationInvocationV1::from_bytes(row.invocation),
        )
        .unwrap();
        let frame = SourceIsaObservationFrameV1::new(
            SourceIsaObservationContextV1::new(row.config, row.unit, attempt, row.finalization)
                .unwrap(),
            SourceIsaObservationOutcomeV1::Unavailable(
                SourceIsaObservationUnavailableReasonV1::SourceProjectionForKirV18,
            ),
        );
        row.check_frame(&frame).unwrap();
        for fault in 0..6 {
            let mut changed = row.clone();
            match fault {
                0 => changed.config[0] ^= 1,
                1 => changed.unit[0] ^= 1,
                2 => changed.session[0] ^= 1,
                3 => changed.invocation[0] ^= 1,
                4 => changed.generation += 1,
                5 => changed.finalization[0] ^= 1,
                _ => unreachable!(),
            }
            assert!(changed.check_frame(&frame).is_err());
        }
    }
}
