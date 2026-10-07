use super::*;
use fe2o3_compiler_execution_protocol::NATIVE_PROOF_CUSTODIAN_CONFIGURATION_BYTES_V1 as APP_BYTES;

const HEADER: usize = 24;
const MANAGER_BYTES: usize = 208;
const TRAILER: usize = 32;
pub(super) const MAX_BYTES: usize = HEADER + APP_BYTES + MANAGER_BYTES + POLICY_MAX + TRAILER;
const DOMAIN: &[u8] = b"FE2O3/NATIVE-PROOF-PROVISIONING-CANDIDATE/V1\0";

pub(super) struct Candidate {
    pub application: Config,
    pub manager: ManagerConfig,
    pub policy: Vec<u8>,
}

fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

impl Candidate {
    pub fn validate(&self, budget: &mut Budget<'_>) -> io::Result<()> {
        budget
            .charge_work(self.policy.len() + 4096)
            .map_err(other)?;
        require(
            (1..=POLICY_MAX).contains(&self.policy.len()),
            "native candidate policy extent",
        )?;
        let policy = (
            Sha256::digest(&self.policy).into(),
            self.policy.len() as u64,
        );
        require(
            self.application.semantic_policy() == policy
                && self.manager.semantic_policy() == policy
                && self.manager.compiler_policy_identity()
                    == self.application.compiler_policy_identity()
                && self.manager.proof_deployment_identity() == self.application.identity(),
            "native candidate changed exact policy/deployment association",
        )?;
        fe2o3_verifier::validate_native_conditional_root_policy_file_v1(&self.policy, budget)
    }

    pub fn records(&self) -> [&[u8]; 3] {
        [
            self.application.canonical_bytes(),
            &self.policy,
            self.manager.canonical_bytes(),
        ]
    }

    pub fn encode(&self, budget: &mut Budget<'_>) -> io::Result<Vec<u8>> {
        self.validate(budget)?;
        let size = HEADER + APP_BYTES + MANAGER_BYTES + self.policy.len() + TRAILER;
        budget.charge_work(size + 4096).map_err(other)?;
        budget.reserve_storage(size).map_err(other)?;
        let mut bytes = vec![0; size];
        bytes[..8].copy_from_slice(b"F3NPCD1\0");
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&(size as u32).to_le_bytes());
        bytes[16..24].copy_from_slice(&(self.policy.len() as u64).to_le_bytes());
        let app_end = HEADER + APP_BYTES;
        let manager_end = app_end + MANAGER_BYTES;
        let end = size - TRAILER;
        bytes[HEADER..app_end].copy_from_slice(self.application.canonical_bytes());
        bytes[app_end..manager_end].copy_from_slice(self.manager.canonical_bytes());
        bytes[manager_end..end].copy_from_slice(&self.policy);
        let digest = checksum(&bytes[..end]);
        bytes[end..].copy_from_slice(&digest);
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> io::Result<Self> {
        budget
            .charge_work(bytes.len().min(MAX_BYTES) + 4096)
            .map_err(other)?;
        require(
            bytes.len() > HEADER + APP_BYTES + MANAGER_BYTES + TRAILER
                && bytes.len() <= MAX_BYTES
                && budget.storage() >= bytes.len(),
            "native candidate input bounds or accounting",
        )?;
        require(
            &bytes[..8] == b"F3NPCD1\0"
                && bytes[8..10] == 1u16.to_le_bytes()
                && bytes[10..12] == [0; 2]
                && u32::from_le_bytes(bytes[12..16].try_into().map_err(other)?) as usize
                    == bytes.len(),
            "native candidate closed header",
        )?;
        let policy_len = u64::from_le_bytes(bytes[16..24].try_into().map_err(other)?);
        let app_end = HEADER + APP_BYTES;
        let manager_end = app_end + MANAGER_BYTES;
        let end = bytes.len() - TRAILER;
        require(
            policy_len == (end - manager_end) as u64 && bytes[end..] == checksum(&bytes[..end]),
            "native candidate extent or checksum",
        )?;
        let (application, charge) = Config::decode(&bytes[HEADER..app_end], budget)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let (manager, charge) = ManagerConfig::decode(&bytes[app_end..manager_end], budget)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        budget
            .reserve_storage(end - manager_end + size_of::<Self>())
            .map_err(other)?;
        let candidate = Self {
            application,
            manager,
            policy: bytes[manager_end..end].to_vec(),
        };
        candidate.validate(budget)?;
        Ok(candidate)
    }
}
