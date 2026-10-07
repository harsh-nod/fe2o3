//! Independent native approval. No legacy record or candidate grants live custody.
use super::{current_inspection_credentials, other, require};
use crate::{
    NativeApplicationManagerConfigurationV1 as ManagerConfig,
    NativeApplicationProofCustodianDeploymentV1 as Config,
};
use fe2o3_compiler_closure_capability::{
    ProductionCompilerExecutionDeploymentV3 as Compiler,
    RootProductionCompilerExecutionDeploymentV3 as RootCompiler,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_verifier::FunctionalRefinementVerusRuntimeLeaseV1 as Runtime;
use sha2::{Digest, Sha256};
use std::{io, path::Path};

mod candidate;
mod files;
pub(super) mod policy_candidate;
mod publication;
use candidate::Candidate;
use files::{Images, read_pinned, write_candidate};

const ACCOUNT_WORK: usize = 1_000_000_000_000;
const ACCOUNT_STORAGE: usize = 16 * 1024 * 1024 * 1024;
const POLICY_MAX: usize = fe2o3_verifier::MAX_NATIVE_CONDITIONAL_ROOT_POLICY_FILE_BYTES_V1;
const RECORD_NAMES: [&str; 3] = [
    "application-native-deployment-v1",
    "native-conditional-root-policy-v1",
    "native-manager-deployment-v1",
];

fn hex(value: [u8; 32]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(64);
    for byte in value {
        write!(&mut result, "{byte:02x}").expect("String formatting");
    }
    result
}

pub(super) fn inspect(policy: &Path, pin: [u8; 32], output: &Path) -> io::Result<()> {
    let credentials = current_inspection_credentials()?;
    let process =
        fe2o3_protected_service_profile::ProofControllerProcessProfileV1::capture(credentials)
            .map_err(other)?;
    let mut account = Owned::new(Work::new(ACCOUNT_WORK), ACCOUNT_STORAGE);
    account.with_budget(|budget| {
        let (compiler, charge) = Compiler::open(budget).map_err(other)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        files::separate_credentials(credentials, compiler.profile())?;
        let policy = read_pinned(policy, pin, POLICY_MAX, budget)?;
        fe2o3_verifier::validate_native_conditional_root_policy_file_v1(&policy, budget)?;
        let images = Images::open(budget)?;
        // Only this stable non-root role may execute the bounded analyzer
        // inspection. Its child-process bounds remain a separate resource domain.
        process.revalidate_current().map_err(other)?;
        budget.charge_work(1024 * 1024).map_err(other)?;
        let limits = fe2o3_kernel_analysis::AuthenticatedPhysicalMachineEffectLimitsV1::new(
            std::time::Duration::from_secs(60),
            1024 * 1024,
            16384,
        )
        .map_err(other)?;
        let analyzer = fe2o3_kernel_analysis::inspect_physical_machine_effect_worker_candidate_v1(
            crate::deployment::WORKER_PATH,
            limits,
        )
        .map_err(other)?
        .policy();
        let (runtime, charge) = Runtime::open_closed_conditional_fill_in_original_account_v1(
            crate::deployment::RUNTIME_PATH,
            budget,
        )
        .map_err(other)?;
        budget
            .reserve_storage(charge.retained_storage())
            .map_err(other)?;
        let (controller_hash, controller_len) = images.controller.measurement();
        let semantic_policy = (Sha256::digest(&policy).into(), policy.len() as u64);
        let (application, charge) = Config::new(
            credentials,
            controller_hash,
            controller_len,
            analyzer,
            runtime.identity().as_bytes(),
            *compiler.policy().identity().as_bytes(),
            semantic_policy,
            budget,
        )?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let (manager, charge) = ManagerConfig::new(
            images.manager.measurement(),
            *compiler.policy().identity().as_bytes(),
            application.identity(),
            semantic_policy,
            budget,
        )?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let candidate = Candidate {
            application,
            manager,
            policy,
        };
        candidate.validate(budget)?;
        images.validate(&candidate, compiler.profile(), budget)?;
        runtime
            .revalidate_closed_conditional_fill_in_original_account_v1(budget)
            .map_err(other)?;
        compiler.revalidate(budget).map_err(other)?;
        process.revalidate_current().map_err(other)?;
        let encoded = candidate.encode(budget)?;
        write_candidate(output, &encoded, budget)?;
        println!(
            "native_candidate_sha256={}",
            hex(Sha256::digest(encoded).into())
        );
        Ok(())
    })
}

pub(super) fn install(path: &Path, pin: [u8; 32]) -> io::Result<()> {
    fe2o3_protected_service_spawn::require_exact_root_identity_v1().map_err(other)?;
    let mut account = Owned::new(Work::new(ACCOUNT_WORK), ACCOUNT_STORAGE);
    account.with_budget(|budget| {
        let encoded = read_pinned(path, pin, candidate::MAX_BYTES, budget)?;
        let candidate = Candidate::decode(&encoded, budget)?;
        let (compiler, charge) = RootCompiler::open(budget).map_err(other)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let images = Images::open(budget)?;
        images.validate(&candidate, compiler.profile(), budget)?;
        // Opening this lease measures the exact fixed installed closure only;
        // it invokes neither Verus nor an analyzer and cannot create a proof.
        let (runtime, charge) = Runtime::open_closed_conditional_fill_in_original_account_v1(
            crate::deployment::RUNTIME_PATH,
            budget,
        )
        .map_err(other)?;
        budget
            .reserve_storage(charge.retained_storage())
            .map_err(other)?;
        require(
            runtime.identity().as_bytes() == candidate.application.verus_identity(),
            "installed native verifier differs from independently approved candidate",
        )?;
        let sealed = images.seal(budget)?;
        let parent = super::open_config_parent()?;
        publication::publish(&parent, &candidate, 0, 0, budget, |b| {
            fe2o3_protected_service_spawn::require_exact_root_identity_v1().map_err(other)?;
            super::revalidate_config_parent(&parent)?;
            compiler.revalidate(b).map_err(other)?;
            images.validate(&candidate, compiler.profile(), b)?;
            runtime
                .revalidate_closed_conditional_fill_in_original_account_v1(b)
                .map_err(other)?;
            for image in &sealed {
                image.revalidate(b).map_err(other)?;
            }
            Ok(())
        })?;
        println!("installed_native_candidate_sha256={}", hex(pin));
        Ok(())
    })
}

#[cfg(test)]
mod tests;
