//! Installed native deployment joined to an original root-owned supervisor child.
use super::super::{
    ExpectedClientProcessIdentityV1 as Expected, LiveClientPidfdIdentityV2 as Client,
};
use crate::compiler_execution_issuer::{
    RootIssuerImageErrorV3, validate_original_supervisor_image_v3,
};
use fe2o3_compiler_closure_capability::{
    RootProductionCompilerExecutionDeploymentErrorV3 as DeploymentError,
    RootProductionCompilerExecutionDeploymentV3 as Deployment,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as StorageAccount,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1 as Credentials,
    ProtectedServiceNamespaceSetV2 as Namespaces, ProtectedServiceProfileErrorV2 as ProfileError,
    observations,
};
use fe2o3_protected_service_spawn::native_spawn::{
    ProtectedServiceSpawnErrorV2 as SpawnError, RootOwnedProtectedServiceChildV2 as Child,
};
use std::{fmt, marker::PhantomData, mem::size_of};

type Result<T> = std::result::Result<T, RootNativeApplicationSupervisorErrorV3>;
type Error = RootNativeApplicationSupervisorErrorV3;

#[derive(Debug)]
pub enum RootNativeApplicationSupervisorErrorV3 {
    Resource(Resource),
    Deployment(DeploymentError),
    Client(crate::LiveClientPidfdErrorV2),
    Profile(ProfileError),
    Observation(observations::Error),
    Child(SpawnError),
    Image(RootIssuerImageErrorV3),
    Io(rustix::io::Errno),
    Refused(&'static str),
}
macro_rules! convert {
    ($source:ty, $variant:ident) => {
        impl From<$source> for Error {
            fn from(value: $source) -> Self {
                Self::$variant(value)
            }
        }
    };
}
convert!(Resource, Resource);
convert!(DeploymentError, Deployment);
convert!(crate::LiveClientPidfdErrorV2, Client);
convert!(ProfileError, Profile);
convert!(observations::Error, Observation);
convert!(SpawnError, Child);
convert!(RootIssuerImageErrorV3, Image);
convert!(rustix::io::Errno, Io);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "root native application supervisor custody rejected: {self:?}"
        )
    }
}
impl std::error::Error for Error {}

/// Full unreserved owner charge; borrowed deployment/child reservations remain live.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootNativeApplicationSupervisorStorageV3(usize);
impl RootNativeApplicationSupervisorStorageV3 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Root-installed native V3 configuration and the original measured supervisor.
///
/// The borrowed child is the original native spawn owner, never a reopened PID,
/// caller pidfd or legacy service conversion. This owner does not reap, cancel,
/// release its exec lease, create a registration endpoint, or publish readiness.
/// It observes original-child liveness, the protected proc-visible profile,
/// parentage, matching namespaces and the actual static executable. Parent root,
/// the compatible kernel/procfs and exclusive consuming-wait custody are trusted.
///
/// These are point-in-time checks, not a continuous lease or proof of securebits,
/// child dumpability, limits or compiler currentness. Readiness must separately
/// join the native RootControlSessionV3 path; this type cannot manufacture that
/// join. Image/proc/profile/namespace access denial fails closed.
///
/// All operations retain original Work/account identity. Borrowed owners must
/// remain prepaid; reserve the returned full charge. Local charges are released
/// on success, retained on failure; accepted work is never refunded. Constants
/// cover local control only; nested image and native observations charge their
/// own finite schedules on the same account. This is not an RSS/latency bound.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootNativeApplicationSupervisorV3 as Native;
/// fn copy(value: Native<'_, '_>) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootNativeApplicationSupervisorV3 as Native;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<Native<'_, '_>>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::RootNativeApplicationSupervisorV3 as Native;
/// fn forge(pid: u32) -> Native<'static, 'static> { pid.into() }
/// ```
pub struct RootNativeApplicationSupervisorV3<'custody, 'work> {
    deployment: &'custody Deployment<'work>,
    child: &'custody Child,
    root: Client,
    supervisor: Client,
    namespaces: Namespaces,
    ledger: Ledger,
    storage_account: Option<StorageAccount>,
    retained: usize,
    work: PhantomData<&'work Work>,
}
impl fmt::Debug for RootNativeApplicationSupervisorV3<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootNativeApplicationSupervisorV3")
            .field("policy", &self.deployment.policy().identity())
            .field("supervisor", &self.supervisor.expected_client())
            .finish_non_exhaustive()
    }
}
impl<'custody, 'work> RootNativeApplicationSupervisorV3<'custody, 'work> {
    pub const CONTROL_WORK: usize = 64 * 1024;
    pub const CONTROL_STORAGE: usize = 128 * 1024;

    pub fn admit(
        deployment: &'custody Deployment<'work>,
        child: &'custody Child,
        budget: &mut Budget<'work>,
    ) -> Result<(Self, RootNativeApplicationSupervisorStorageV3)> {
        let floor = budget.storage();
        let inputs = deployment
            .retained_storage()?
            .checked_add(child.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(Self::CONTROL_WORK)?;
        if floor < inputs {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(Self::CONTROL_STORAGE)?;
        deployment.revalidate(budget)?;
        if !child.is_live(budget)? {
            return Err(Error::Refused("original supervisor child exited"));
        }
        let root_fd = rustix::process::pidfd_open(
            rustix::process::getpid(),
            rustix::process::PidfdFlags::empty(),
        )?;
        budget.reserve_storage(Client::FD_STORAGE)?;
        let (root, storage) = Client::admit(
            root_fd,
            Expected {
                pid: std::process::id(),
                uid: 0,
                gid: 0,
            },
            budget,
        )?;
        budget.reserve_storage(storage.additional_storage())?;
        let (child_fd, storage) = child.try_clone_pidfd(budget)?;
        budget.reserve_storage(storage.additional_storage())?;
        let (supervisor, storage) = Client::admit(
            child_fd,
            Expected {
                pid: child.pid().as_raw_pid() as u32,
                uid: deployment.supervisor().service_uid(),
                gid: deployment.supervisor().service_gid(),
            },
            budget,
        )?;
        budget.reserve_storage(storage.additional_storage())?;
        let (namespaces, storage) = Namespaces::capture_self(budget)?;
        budget.reserve_storage(storage.additional_storage())?;
        let retained = size_of::<(Self, RootNativeApplicationSupervisorStorageV3)>()
            .checked_add(root.retained_storage())
            .and_then(|n| n.checked_add(supervisor.retained_storage()))
            .and_then(|n| n.checked_add(namespaces.retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        if retained > Self::CONTROL_STORAGE {
            return Err(Resource::Accounting.into());
        }
        let value = Self {
            deployment,
            child,
            root,
            supervisor,
            namespaces,
            ledger: budget.work_ledger_identity_v1(),
            storage_account: budget.storage_account_identity_v1(),
            retained,
            work: PhantomData,
        };
        value.revalidate_inner(budget)?;
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
        Ok((value, RootNativeApplicationSupervisorStorageV3(retained)))
    }

    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub(crate) const fn deployment(&self) -> &'custody Deployment<'work> {
        self.deployment
    }
    pub(crate) const fn expected_supervisor(&self) -> Expected {
        self.supervisor.expected_client()
    }
    pub(crate) const fn expected_root(&self) -> Expected {
        self.root.expected_client()
    }
    pub(crate) const fn root_client(&self) -> &Client {
        &self.root
    }

    pub fn revalidate(&self, budget: &mut Budget<'work>) -> Result<()> {
        budget.charge_work(Self::CONTROL_WORK)?;
        let floor = self
            .retained
            .checked_add(self.deployment.retained_storage()?)
            .and_then(|n| n.checked_add(self.child.retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        if self.ledger != budget.work_ledger_identity_v1()
            || self.storage_account != budget.storage_account_identity_v1()
            || budget.storage() < floor
        {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(Self::CONTROL_STORAGE)?;
        self.revalidate_inner(budget)?;
        budget.release_storage(Self::CONTROL_STORAGE)?;
        Ok(())
    }

    fn revalidate_inner(&self, budget: &mut Budget<'work>) -> Result<()> {
        self.deployment.revalidate(budget)?;
        if !self.child.is_live(budget)? {
            return Err(Error::Refused("original supervisor child exited"));
        }
        require_coordinates(
            self.root.expected_client(),
            self.supervisor.expected_client(),
            self.child.pid().as_raw_pid() as u32,
            self.deployment.supervisor().service_uid(),
            self.deployment.supervisor().service_gid(),
            std::process::id(),
        )?;
        self.root.validate_liveness(budget)?;
        self.supervisor.validate_parent(&self.root, budget)?;
        self.namespaces.revalidate_self(budget)?;
        self.namespaces
            .revalidate_process(self.child.pid(), budget)?;
        let credentials = Credentials::new(
            self.deployment.supervisor().service_uid(),
            self.deployment.supervisor().service_gid(),
        )
        .map_err(|_| Error::Refused("invalid installed service credentials"))?;
        budget.with_prepaid_scope(
            self.namespaces.retained_storage(),
            8,
            8 + observations::PROCESS_VALIDATE_WORK,
            observations::PROCESS_VALIDATE_SCRATCH,
            |_| -> Result<()> {
                Ok(observations::validate_process(
                    credentials,
                    self.child.pid(),
                )?)
            },
        )?;
        validate_original_supervisor_image_v3(self.child, self.deployment.supervisor(), budget)?;
        self.supervisor.validate_parent(&self.root, budget)?;
        self.namespaces.revalidate_self(budget)?;
        self.namespaces
            .revalidate_process(self.child.pid(), budget)?;
        if !self.child.is_live(budget)? {
            return Err(Error::Refused(
                "original supervisor child exited during observation",
            ));
        }
        self.deployment.revalidate(budget)?;
        Ok(())
    }
}

fn require_coordinates(
    root: Expected,
    supervisor: Expected,
    child: u32,
    uid: u32,
    gid: u32,
    current: u32,
) -> Result<()> {
    if root
        != (Expected {
            pid: current,
            uid: 0,
            gid: 0,
        })
        || supervisor
            != (Expected {
                pid: child,
                uid,
                gid,
            })
        || current == 0
        || child == 0
        || child == current
        || uid == 0
    {
        return Err(Error::Refused(
            "native supervisor root/child coordinates changed",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn child_coordinates_bind_every_root_and_supervisor_axis() {
        let root = Expected {
            pid: 17,
            uid: 0,
            gid: 0,
        };
        let supervisor = Expected {
            pid: 23,
            uid: 31,
            gid: 37,
        };
        require_coordinates(root, supervisor, 23, 31, 37, 17).unwrap();
        for changed in [
            Expected { pid: 18, ..root },
            Expected { uid: 1, ..root },
            Expected { gid: 1, ..root },
        ] {
            assert!(require_coordinates(changed, supervisor, 23, 31, 37, 17).is_err());
        }
        for changed in [
            Expected {
                pid: 24,
                ..supervisor
            },
            Expected {
                uid: 32,
                ..supervisor
            },
            Expected {
                gid: 38,
                ..supervisor
            },
        ] {
            assert!(require_coordinates(root, changed, 23, 31, 37, 17).is_err());
        }
        for (child, uid, gid, current) in [
            (24, 31, 37, 17),
            (23, 32, 37, 17),
            (23, 31, 38, 17),
            (23, 31, 37, 18),
            (0, 31, 37, 17),
            (17, 31, 37, 17),
            (23, 0, 37, 17),
        ] {
            assert!(require_coordinates(root, supervisor, child, uid, gid, current).is_err());
        }
    }
}
