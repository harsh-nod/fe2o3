// Native inherited admission is separate from the terminal exchange implementation.
macro_rules! inherited_admission_adapter {
    ($Client:ident, $Error:ident) => {
        impl<'budget, 'work> $Client<'budget, 'work> {
            /// Consumes the service peer inherited at fixed child descriptor 195.
            ///
            /// Prepay `Self::PEER_STORAGE` on the original budget. Before inspecting
            /// or duplicating the slot, charge eight fixed-slot work units plus the
            /// existing 64-KiB peer-admission work bound and reserve 8192 scratch bytes.
            /// Resource denial still consumes the canonical slot, without inspection.
            ///
            /// The V1 fixed-slot mechanics retain one private CLOEXEC duplicate and
            /// consume the canonical descriptor before timeout/transport validation.
            /// No legacy policy/packet decode, fallback or new account is involved.
            /// Success owns and refunds the same peer charge as `Self::admit`.
            /// Errors close all consumed descriptors and leave the caller's input
            /// reservation intact; work, peaks and denial history are never reset.
            ///
            /// This admits transport only, not protected compiler or issuer authority.
            /// Use `Self::admit` when an `OwnedFd` already represents the input.
            ///
            /// # Safety
            /// Transfer exclusive ownership of FD 195, inherited without a Rust
            /// owner or explicitly relinquished for this transfer. It must have no
            /// existing Rust owner or outstanding borrow. No thread, signal handler,
            /// or foreign code may close, replace, or acquire it during this call.
            /// If absent, keep the slot unallocated until return. Consume this
            /// transfer only once, even after an error, resource refusal, or unwind.
            /// A later occupant of FD 195 is not another inherited input. Checks
            /// cannot prove ownership.
            #[doc = concat!("\n```compile_fail,E0133\nuse fe2o3_compiler_execution_client::{", stringify!($Client), " as Client, ", stringify!($Error), " as Error};\nuse fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;\nfn admit<'b, 'w>(b: &'b mut Budget<'w>) -> Result<Client<'b, 'w>, Error> {\n    b.reserve_storage(Client::PEER_STORAGE)?;\n    Client::admit_inherited_child(std::time::Duration::from_secs(1), b)\n}\n```")]
            pub unsafe fn admit_inherited_child(
                timeout: Duration,
                budget: &'budget mut Budget<'work>,
            ) -> std::result::Result<Self, $Error> {
                // SAFETY: the caller transfers exclusive custody through all exits,
                // including budget refusal before the descriptor can be inspected.
                let pending = unsafe { crate::inherited_admission::PendingInheritedPeer::new() };
                let (peer, deadline) = budget.with_prepaid_scope::<_, $Error>(
                    Self::PEER_STORAGE,
                    8,
                    64 * 1024 + 8,
                    8192,
                    |_| {
                        let peer = pending.retain()?;
                        if timeout.is_zero() || timeout > Duration::from_secs(300) {
                            return Err(TransportError::InvalidTimeout.into());
                        }
                        set_close_on_exec(&peer)?;
                        validate_seqpacket_peer(&peer)?;
                        let deadline = Instant::now()
                            .checked_add(timeout)
                            .ok_or(TransportError::DeadlineOverflow)?;
                        Ok((peer, deadline))
                    },
                )?;
                budget.reserve_storage(Self::RETAINED - Self::PEER_STORAGE)?;
                Ok(Self {
                    peer: Some(peer),
                    deadline,
                    budget,
                    retained: Self::RETAINED,
                })
            }
        }
    };
}
pub(crate) use inherited_admission_adapter;
