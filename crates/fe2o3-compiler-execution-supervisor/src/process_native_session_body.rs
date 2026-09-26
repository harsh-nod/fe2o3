// Each native module supplies its genuine handoff, preparation and process owners.
impl Supervisor {
    /// Runs authenticated handoff through exact terminal reaping on the original ledger.
    ///
    /// Prepay the supervisor and `AcceptedCompilerExecutionHandoffV2::CONTROL_STORAGE`
    /// (or the corresponding V3 constant) before passing the consumed control socket.
    /// Invalid input accounting closes the socket but leaves reservations unchanged.
    /// Once adopted, every refusal drops consumed owners before releasing their
    /// reservations. Accepted work and first-denial history are never refunded.
    /// The terminal result continues borrowing the same ledger until its final Drop.
    ///
    /// The independently funded cleanup service remains controller-owned and must
    /// still be pumped after refusal. This method neither provisions a listener nor
    /// installs privileges, activates compiler publication, or grants GPU authority.
    /// All existing profile, policy, pidfd and exact-readiness checks remain in force.
    pub fn run_session<'a, 'work>(
        &'a self,
        control: OwnedFd,
        cleanup: &mut Cleanup,
        limits: SessionLimits,
        budget: &'a mut Budget<'work>,
    ) -> std::result::Result<Exited<'a, 'work>, SessionError> {
        let guard = session_control(control, self.retained_storage(), budget)
            .map_err(SessionError::Handoff)?;
        let guard = guard
            .advance(|control, b| {
                self.accept_handoff(control, limits.handoff(), b)
                    .map(|(owner, growth)| (owner, growth.additional_storage()))
            })
            .map_err(SessionError::Handoff)?;
        let guard = guard
            .advance(|accepted, b| {
                self.prepare_launch(accepted, b)
                    .map(|(owner, growth)| (owner, growth.additional_storage()))
            })
            .map_err(SessionError::Preparation)?;
        let floor = guard
            .funding
            .retained
            .checked_add(self.retained_storage())
            .and_then(|n| n.checked_add(OWNER_GROWTH))
            .ok_or_else(|| SessionError::Launch(Resource::Arithmetic.into()))?;
        let launched = self
            .launch_funded(
                Funded {
                    owner: Some(guard.owner),
                    funding: guard.funding,
                },
                cleanup,
                limits.launch(),
                floor,
            )
            .map_err(SessionError::Launch)?;
        let ready = launched
            .await_readiness(limits.readiness())
            .map_err(SessionError::Readiness)?;
        let serving = ready
            .publish_readiness(limits.publication())
            .map_err(SessionError::Publication)?;
        serving
            .wait_for_exit(limits.exit())
            .map_err(SessionError::Exit)
    }
}

fn session_control<'a, 'work>(
    control: OwnedFd,
    supervisor_storage: usize,
    budget: &'a mut Budget<'work>,
) -> std::result::Result<Funded<OwnedFd, RequestFunding<'a, 'work>>, HandoffError> {
    let floor = supervisor_storage
        .checked_add(Accepted::CONTROL_STORAGE)
        .ok_or(Resource::Arithmetic)?;
    if budget.storage() < floor {
        budget.charge_work(ENTRY)?;
        return Err(Resource::Accounting.into());
    }
    let guard = Funded {
        owner: control,
        funding: RequestFunding {
            budget,
            retained: Accepted::CONTROL_STORAGE,
        },
    };
    guard.funding.budget.charge_work(ENTRY)?;
    Ok(guard)
}
