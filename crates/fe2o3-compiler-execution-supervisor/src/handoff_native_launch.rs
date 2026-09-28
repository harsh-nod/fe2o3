//! Private peer transfer shared by concrete native handoff families.
use super::*;

impl Accepted {
    // Consuming launch prepays closure of the unused peer/frame/client owners.
    pub(crate) fn into_control(self) -> OwnedFd {
        self.control
    }

    pub(crate) fn clone_launch_peers(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(std::fs::File, std::fs::File, usize)> {
        budget.with_prepaid_scope(self.retained, ENTRY, Self::WORK, Self::SCRATCH, |b| {
            checks::service_peer(&self.service_peer, self.manifest().client())?;
            let peer = rustix::io::fcntl_dupfd_cloexec(&self.service_peer, 0)?;
            b.reserve_storage(Self::CONTROL_STORAGE)?;
            if checks::snapshot(&peer)? != self.service_snapshot {
                return Err(LaunchError::DescriptorChanged);
            }
            checks::service_peer(&peer, self.manifest().client())?;
            let (pidfd, delta) = self.client.try_clone_for_transfer(b)?;
            b.reserve_storage(delta.additional_storage())?;
            Ok((
                peer.into(),
                pidfd.into(),
                Self::CONTROL_STORAGE + delta.additional_storage(),
            ))
        })
    }

    pub(crate) fn revalidate_launch_peers(
        &self,
        peer: &std::fs::File,
        pidfd: &std::fs::File,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        let floor = self
            .retained
            .checked_add(Self::CONTROL_STORAGE + LiveClient::FD_STORAGE)
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, ENTRY, Self::WORK, Self::SCRATCH, |b| {
            if checks::snapshot(peer)? != self.service_snapshot {
                return Err(LaunchError::DescriptorChanged);
            }
            checks::service_peer(peer, self.manifest().client())?;
            self.client.validate_transfer(pidfd, b)?;
            Ok(())
        })
    }
}
