//! Crate-private transfer of native supervisor inputs into prepared launch custody.
use super::*;
use crate::launch_checks::{
    EXTERNAL_ANCHOR_PEER_SOURCE_INDEX, EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX, POLICY_SOURCE_INDEX,
    ROOT_SOURCE_INDEX, SIGNING_KEY_SOURCE_INDEX, SOURCE_COUNT_V1,
};
use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV2 as PolicyCapability;
use std::os::fd::AsFd;

pub(crate) struct Inputs {
    pub launcher: File,
    pub issuer: File,
    pub root: File,
    pub policy: File,
    pub key: File,
    pub anchor_peer: File,
    pub anchor_pidfd: File,
    pub retained: usize,
}

impl ProtectedIssuerSupervisorV2 {
    pub(crate) fn clone_launch_inputs(&self, budget: &mut Budget<'_>) -> Result<Inputs> {
        budget.with_prepaid_scope(self.retained, ENTRY, Self::WORK, Self::SCRATCH, |b| {
            self.check(b)?;
            let mut retained = 0;
            let (launcher, delta) = self.program.try_clone_launcher_for_launch(b)?;
            keep(delta.additional_storage(), &mut retained, b)?;
            let (issuer, delta) = self.program.try_clone_issuer_for_launch(b)?;
            keep(delta.additional_storage(), &mut retained, b)?;
            let root = rustix::io::fcntl_dupfd_cloexec(&self.root, 0)
                .map(File::from)
                .map_err(|errno| ProtectedIssuerSupervisorErrorV2::Io {
                    operation: "clone protected issuer root",
                    errno,
                })?;
            keep(Self::ROOT_FILE_STORAGE, &mut retained, b)?;
            if root_checks::inspect(&root, self.credentials)? != self.root_snapshot {
                return Err(ProtectedIssuerSupervisorErrorV2::RootChanged);
            }
            let (policy, delta) = self.program.try_clone_policy_for_launch(b)?;
            keep(delta.additional_storage(), &mut retained, b)?;
            let (key, delta) = self.signing_key.try_clone_for_transfer(b)?;
            keep(delta.additional_storage(), &mut retained, b)?;
            let (anchor_peer, anchor_pidfd, delta) =
                self.external_anchor.try_clone_for_transfer(b)?;
            keep(delta.additional_storage(), &mut retained, b)?;
            Ok(Inputs {
                launcher,
                issuer,
                root,
                policy,
                key,
                anchor_peer: anchor_peer.into(),
                anchor_pidfd: anchor_pidfd.into(),
                retained,
            })
        })
    }

    pub(crate) fn revalidate_launch_inputs(
        &self,
        launcher: &File,
        issuer: &File,
        sources: &[File; SOURCE_COUNT_V1],
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        // Each leaf additionally checks its own input floor, including the
        // image length charge. The outer fixed descriptors are also prepaid.
        let floor = self
            .retained
            .checked_add(
                Self::ROOT_FILE_STORAGE
                    + PolicyCapability::FILE_STORAGE
                    + Key::FILE_STORAGE
                    + Anchor::PAIR_STORAGE,
            )
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, ENTRY, Self::WORK, Self::SCRATCH, |b| {
            self.check(b)?;
            self.program.revalidate_launcher_clone(launcher, b)?;
            self.program.revalidate_issuer_clone(issuer, b)?;
            if root_checks::inspect(&sources[ROOT_SOURCE_INDEX], self.credentials)?
                != self.root_snapshot
            {
                return Err(ProtectedIssuerSupervisorErrorV2::RootChanged);
            }
            self.program
                .revalidate_policy_clone(&sources[POLICY_SOURCE_INDEX], b)?;
            self.signing_key.validate_transfer(
                &sources[SIGNING_KEY_SOURCE_INDEX],
                self.policy(),
                b,
            )?;
            self.external_anchor.validate_transfer(
                sources[EXTERNAL_ANCHOR_PEER_SOURCE_INDEX].as_fd(),
                sources[EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX].as_fd(),
                b,
            )?;
            Ok(())
        })
    }
}

fn keep(bytes: usize, retained: &mut usize, budget: &mut Budget<'_>) -> Result<()> {
    let next = retained.checked_add(bytes).ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    *retained = next;
    Ok(())
}
