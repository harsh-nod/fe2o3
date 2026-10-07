//! Move-only actual issuer custody retained across pre-ACK observation consumption.
use super::*;
use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Deployment;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionServiceLaunchManifestV3 as Manifest,
};
use fe2o3_runtime_protocol::NativeApplicationRegistrationBindingV1 as Binding;

// This private trait erases only the cleanup payload type. Its only implementor
// is the original native retained child, never caller-provided validation code.
trait OriginalIssuer {
    fn retained_storage(&self) -> usize;
    fn validate(
        &self,
        client: &Client,
        root: &Client,
        namespaces: &Namespaces,
        policy: &Policy,
        b: &mut Budget<'_>,
    ) -> io::Result<()>;
}
impl<T: Send + 'static> OriginalIssuer for Child<T> {
    fn retained_storage(&self) -> usize {
        Child::retained_storage(self)
    }
    fn validate(
        &self,
        client: &Client,
        root: &Client,
        namespaces: &Namespaces,
        policy: &Policy,
        b: &mut Budget<'_>,
    ) -> io::Result<()> {
        validate_child_parts(root, self, client, namespaces, policy, b)
    }
}

/// Original root-connected currentness issuer, not a decoded gate identity.
/// The actual child, root installation and original cumulative account remain
/// live. This owner may move into the fixed proof-controller startup owner while
/// the registration's original app/Cargo owners move alongside it. It does not
/// authenticate their proof, acknowledge a pipe, or activate that controller.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ApplicationCurrentnessCustodyV3;
/// fn fake(id:[u8;32]) { let _=ApplicationCurrentnessCustodyV3::from_identity(id); }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::ApplicationCurrentnessCustodyV3;
/// fn duplicate(value:ApplicationCurrentnessCustodyV3<'_, '_>) {let _=value.clone();}
/// ```
pub struct ApplicationCurrentnessCustodyV3<'root, 'work> {
    connection: RootApplicationCurrentnessConnectionV3<'work>,
    issuer: &'root dyn OriginalIssuer,
    deployment: &'root Deployment<'work>,
    root: Client,
    manifest: Manifest,
    application: (Expected, u64),
    cargo: (Expected, u64),
    retained: usize,
}
impl<'work> RootApplicationCurrentnessConnectionV3<'work> {
    /// Consumes the actual connected owner. Source registration and original
    /// child are revalidated; the returned charge is growth above this owner.
    /// No public constructor accepts raw descriptors, validation callbacks or
    /// identity records. Root/child/config borrows outlive proof startup.
    pub fn into_application_custody<'root, 'registry, 'custody, T: Send + 'static>(
        self,
        registration: &Registration<'registry, 'custody, 'work>,
        issuer: &'root Child<T>,
        b: &mut Budget<'work>,
    ) -> io::Result<(ApplicationCurrentnessCustodyV3<'root, 'work>, usize)>
    where
        'custody: 'root,
    {
        self.validate(registration, issuer, b)?;
        let floor = b.storage();
        b.charge_work(Self::LOCAL_WORK).map_err(error)?;
        b.reserve_storage(Self::LOCAL_STORAGE).map_err(error)?;
        let (fd, charge) = registration
            .root_client()
            .try_clone_for_transfer(b)
            .map_err(error)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(error)?;
        let (root, charge) =
            Client::admit(fd, registration.root_client().expected_client(), b).map_err(error)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(error)?;
        let (manifest, charge) = Manifest::decode(
            registration
                .binding()
                .compiler_handoff()
                .launch_manifest()
                .canonical_bytes(),
            b,
        )
        .map_err(error)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(error)?;
        let input = self.retained;
        let retained = size_of::<ApplicationCurrentnessCustodyV3<'root, 'work>>()
            .checked_add(input)
            .and_then(|n| n.checked_add(root.retained_storage()))
            .and_then(|n| n.checked_add(manifest.retained_storage()))
            .ok_or_else(|| refused("currentness custody accounting"))?;
        let value = ApplicationCurrentnessCustodyV3 {
            connection: self,
            issuer,
            deployment: registration.deployment(),
            root,
            manifest,
            application: (
                registration.application().expected_client(),
                registration.application().start_time_ticks(),
            ),
            cargo: (
                registration.parent().expected_client(),
                registration.parent().start_time_ticks(),
            ),
            retained,
        };
        value.check(b)?;
        b.release_storage(
            b.storage()
                .checked_sub(floor)
                .ok_or_else(|| refused("currentness custody accounting"))?,
        )
        .map_err(error)?;
        Ok((
            value,
            retained
                .checked_sub(input)
                .ok_or_else(|| refused("currentness custody accounting"))?,
        ))
    }
}
impl<'work> ApplicationCurrentnessCustodyV3<'_, 'work> {
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Inert association only. Revalidate this actual owner at each use boundary.
    pub fn gate_identity(&self) -> [u8; 32] {
        self.connection.request.identity()
    }
    pub fn registration_identity(&self) -> [u8; 32] {
        self.connection.request.registration_identity()
    }
    pub fn revalidate(&self, b: &mut Budget<'work>) -> io::Result<()> {
        self.revalidate_in_phase(true, b)
    }
    fn revalidate_in_phase(&self, startup: bool, b: &mut Budget<'work>) -> io::Result<()> {
        b.charge_work(RootApplicationCurrentnessConnectionV3::LOCAL_WORK)
            .map_err(error)?;
        let deployment_storage = self.deployment.retained_storage().map_err(error)?;
        let inputs = self
            .retained
            .checked_add(self.issuer.retained_storage())
            .and_then(|n| n.checked_add(deployment_storage))
            .ok_or_else(|| refused("currentness custody input accounting"))?;
        if b.storage() < inputs {
            return Err(refused("currentness custody inputs not prepaid"));
        }
        b.reserve_storage(RootApplicationCurrentnessConnectionV3::LOCAL_STORAGE)
            .map_err(error)?;
        self.check_continuity(b)?;
        require_deadline_in_phase(startup, self.connection.deadline, std::time::Instant::now())?;
        b.release_storage(RootApplicationCurrentnessConnectionV3::LOCAL_STORAGE)
            .map_err(error)
    }
    /// Match the original owners moved into proof startup, including process
    /// occurrence and exact native binding, without creating substitute custody.
    pub fn revalidate_application(
        &self,
        application: &Client,
        cargo: &Client,
        binding: &Binding,
        b: &mut Budget<'work>,
    ) -> io::Result<()> {
        self.revalidate(b)?;
        self.check_application(application, cargo, binding, b)
    }
    /// Rechecks the same original live owners and account without treating an
    /// elapsed handshake deadline as loss of currentness custody. This grants
    /// no startup, acknowledgment or activation permission. The private active
    /// controller owner must separately enforce its original finite phase policy.
    /// Startup callers must continue using `revalidate_application` instead.
    pub fn revalidate_continuous_application(
        &self,
        application: &Client,
        cargo: &Client,
        binding: &Binding,
        b: &mut Budget<'work>,
    ) -> io::Result<()> {
        self.revalidate_in_phase(false, b)?;
        self.check_application(application, cargo, binding, b)
    }
    fn check_application(
        &self,
        application: &Client,
        cargo: &Client,
        binding: &Binding,
        b: &mut Budget<'work>,
    ) -> io::Result<()> {
        b.charge_work(128).map_err(error)?;
        if self.application
            != (
                application.expected_client(),
                application.start_time_ticks(),
            )
            || self.cargo != (cargo.expected_client(), cargo.start_time_ticks())
            || self.registration_identity() != *binding.identity().as_bytes()
            || self.connection.request.association_identity()
                != *binding.association().identity().as_bytes()
            || self.connection.request.carriage_identity()
                != binding.association().carriage_identity()
            || self.manifest.canonical_bytes()
                != binding
                    .compiler_handoff()
                    .launch_manifest()
                    .canonical_bytes()
        {
            return Err(refused("currentness custody original application changed"));
        }
        application.validate_parent(cargo, b).map_err(error)
    }
    fn check(&self, b: &mut Budget<'work>) -> io::Result<()> {
        self.check_continuity(b)?;
        check_deadline(self.connection.deadline)
    }
    fn check_continuity(&self, b: &mut Budget<'work>) -> io::Result<()> {
        let c = &self.connection;
        if c.ledger != b.work_ledger_identity_v1()
            || c.account != b.storage_account_identity_v1()
            || c.thread != rustix::thread::gettid().as_raw_pid() as u32
        {
            return Err(refused("currentness custody account or thread changed"));
        }
        self.deployment.revalidate(b).map_err(error)?;
        self.root.validate_liveness(b).map_err(error)?;
        if self.root.expected_client().pid() != std::process::id()
            || self.root.expected_client().uid() != 0
            || self.root.expected_client().gid() != 0
        {
            return Err(refused("currentness custody root changed"));
        }
        c.channel.validate_root_endpoint(b).map_err(error)?;
        self.issuer.validate(
            &c.issuer,
            &self.root,
            &c.namespaces,
            self.deployment.policy(),
            b,
        )?;
        if !c
            .request
            .matches_launch(self.deployment.policy(), &self.manifest, b)
            .map_err(error)?
        {
            return Err(refused("currentness custody launch changed"));
        }
        Ok(())
    }
}

fn require_deadline_in_phase(
    startup: bool,
    deadline: std::time::Instant,
    now: std::time::Instant,
) -> io::Result<()> {
    if startup && now >= deadline {
        return Err(refused("currentness root handshake deadline"));
    }
    Ok(())
}

#[cfg(test)]
mod phase_tests {
    use super::*;
    #[test]
    fn expired_handshake_refuses_startup_but_not_continuous_owner_checks() {
        let deadline = std::time::Instant::now();
        assert!(
            require_deadline_in_phase(
                true,
                deadline,
                deadline - std::time::Duration::from_nanos(1)
            )
            .is_ok()
        );
        assert!(require_deadline_in_phase(true, deadline, deadline).is_err());
        assert!(
            require_deadline_in_phase(
                true,
                deadline,
                deadline + std::time::Duration::from_secs(300)
            )
            .is_err()
        );
        assert!(
            require_deadline_in_phase(
                false,
                deadline,
                deadline + std::time::Duration::from_secs(300)
            )
            .is_ok()
        );
    }
}
