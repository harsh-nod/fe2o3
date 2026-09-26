// Shared schedule over concrete native supervisor/session/limit types.
const ENTRY: usize = 8;
const MAX_PATH_BYTES: usize = 108;

impl Service {
    /// Logical charge for the consumed inherited socket, including delta metadata.
    pub const SOCKET_STORAGE: usize = size_of::<(OwnedFd, Storage)>();
    /// Conservative metadata/path/report growth; not allocator use or RSS.
    pub const OWNER_GROWTH: usize =
        size_of::<(Self, Report, DispatchReport, Storage)>() + MAX_PATH_BYTES;
    /// Fixed allowance for socket observations, activation, metadata and retirement.
    /// Native supervisor checks and accept turns charge their work separately.
    pub const WORK: usize = ENTRY + 128 * 1024;
    /// Logical temporary custody and path buffers above all retained inputs.
    pub const SCRATCH: usize = 8 * size_of::<Self>() + 4096;

    /// Consumes the genuine supervisor and the sole fixed, bound service socket.
    /// No supplied pathname or legacy authority conversion is accepted.
    pub fn bind(
        supervisor: Supervisor,
        listener: OwnedFd,
        limits: SessionLimits,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        let policy = FilesystemPolicy::production(supervisor.credentials());
        Self::bind_at(
            supervisor,
            listener,
            limits,
            Path::new(SOCKET_PATH),
            policy,
            budget,
        )
    }

    fn bind_at(
        supervisor: Supervisor,
        listener: OwnedFd,
        limits: SessionLimits,
        path: &Path,
        filesystem: FilesystemPolicy,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        budget.charge_work(ENTRY)?;
        let floor = supervisor
            .retained_storage()
            .checked_add(Self::SOCKET_STORAGE)
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, 0, Self::WORK - ENTRY, Self::SCRATCH, |b| {
            if path.as_os_str().as_encoded_bytes().len() >= MAX_PATH_BYTES {
                return Err(Error::InvalidListener(
                    "listener pathname exceeds Unix socket capacity",
                ));
            }
            supervisor.revalidate(b)?;
            b.reserve_storage(Self::OWNER_GROWTH)?;
            let socket = Socket::admit(listener, path, filesystem)?.activate()?;
            let service = Self {
                supervisor,
                socket,
                limits,
                retained: floor
                    .checked_add(Self::OWNER_GROWTH)
                    .ok_or(Resource::Arithmetic)?,
            };
            service.revalidate(b)?;
            Ok((service, Storage(Self::OWNER_GROWTH)))
        })
    }

    /// Full inherited and added logical charge, retired by the caller after Drop.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Rechecks the exact native supervisor plus descriptor, socket path and parent.
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        budget.with_prepaid_scope(self.retained, ENTRY, Self::WORK, Self::SCRATCH, |b| {
            self.supervisor.revalidate(b)?;
            require_socket_state(self.socket.revalidate()?, SocketState::Listening)?;
            Ok(())
        })
    }

    /// Accepts one connection and dispatches only the matching native `run_session`.
    ///
    /// Every poll, EINTR, EAGAIN and accept race consumes a prepaid finite turn.
    /// Session owners retire before post-session continuity checks on the same
    /// account. The result is only inert observations, never escaped custody.
    /// Failed sessions retain their exact stage unless service continuity also
    /// fails, matching the existing dispatch precedence. A caller must continue
    /// pumping the independently funded cleanup service after refusal.
    ///
    /// This is one dispatch, not an installed worker loop or deployment/recovery
    /// implementation. The complete service reservation must be prepaid on budget.
    pub fn serve_one(
        &self,
        accept_limits: Wait,
        cleanup: &mut Cleanup,
        budget: &mut Budget<'_>,
    ) -> Result<Report> {
        let work = accept_limits
            .attempts()
            .checked_mul(accept::ATTEMPT_WORK)
            .and_then(|n| n.checked_add(ENTRY))
            .ok_or(Resource::Arithmetic)?;
        let control =
            budget.with_prepaid_scope(self.retained, ENTRY, work, Self::SCRATCH, |b| {
                self.revalidate(b)?;
                b.reserve_storage(Accepted::CONTROL_STORAGE)?;
                let control = accept::accept(
                    &self.socket.descriptor,
                    accept_limits.attempts(),
                    accept_limits.timeout(),
                )?;
                self.revalidate(b)?;
                Ok::<_, Error>(control)
            })?;
        // Reserve after scope restoration while the returned descriptor is still
        // locally owned; reservation refusal closes it without adopting a charge.
        budget.reserve_storage(Accepted::CONTROL_STORAGE)?;
        let outcome = self
            .supervisor
            .run_session(control, cleanup, self.limits, budget)
            .map(|exited| Report {
                pid: exited.pid(),
                termination: exited.termination(),
                readiness: exited.readiness().identity(),
            })
            .map_err(Error::Session);
        let continuity = self.revalidate(budget);
        match (outcome, continuity) {
            (result, Ok(())) => result,
            (_, Err(error)) => Err(error),
        }
    }
}

const _: () = assert!(Wait::MAX_ATTEMPTS == accept::MAX_ATTEMPTS);
const _: () = assert!(Wait::MAX_TIMEOUT.as_secs() == accept::MAX_TIMEOUT.as_secs());
