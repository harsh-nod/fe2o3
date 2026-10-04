// One hardened fixed-FD entrypoint; schema and owned capabilities are nominal bindings.
macro_rules! native_entrypoint {
    ($entrypoint:ident, $Error:ident) => {
        /// Bounded refusal from native inherited admission or its consuming service.
        #[derive(Debug)]
        pub enum $Error {
            Resource(Resource),
            Descriptor(crate::CompilerExecutionIssuerEntrypointErrorV1),
            Process(
                fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionErrorV1,
            ),
            ExpectedClient(fe2o3_broker_authority_service::ProtectedServiceAdmissionErrorV1),
            Inputs(InputError),
            Policy(PolicyError),
            Key(fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2),
            Client(fe2o3_broker_authority_service::LiveClientPidfdErrorV2),
            ServiceAdmission(fe2o3_broker_authority_service::ProtectedServiceAdmissionErrorV2),
            Anchor(fe2o3_broker_authority_service::ProtectedExternalAnchorServiceErrorV2),
            Admission(AdmissionError),
            Service(ServiceError),
        }
        macro_rules! from_error {
            ($ty:ty, $variant:ident) => {
                impl From<$ty> for Error {
                    fn from(error: $ty) -> Self {
                        Self::$variant(error)
                    }
                }
            };
        }
        from_error!(Resource, Resource);
        from_error!(crate::CompilerExecutionIssuerEntrypointErrorV1, Descriptor);
        from_error!(InputError, Inputs);
        from_error!(PolicyError, Policy);
        from_error!(
            fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2,
            Key
        );
        from_error!(
            fe2o3_broker_authority_service::LiveClientPidfdErrorV2,
            Client
        );
        from_error!(
            fe2o3_broker_authority_service::ProtectedServiceAdmissionErrorV2,
            ServiceAdmission
        );
        from_error!(
            fe2o3_broker_authority_service::ProtectedExternalAnchorServiceErrorV2,
            Anchor
        );
        from_error!(AdmissionError, Admission);
        from_error!(ServiceError, Service);

        impl fmt::Display for Error {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("native issuer entrypoint refused: ")?;
                match self {
                    Self::Resource(e) => e.fmt(f),
                    Self::Descriptor(e) => e.fmt(f),
                    Self::Process(e) => e.fmt(f),
                    Self::ExpectedClient(e) => e.fmt(f),
                    Self::Inputs(e) => e.fmt(f),
                    Self::Policy(e) => e.fmt(f),
                    Self::Key(e) => e.fmt(f),
                    Self::Client(e) => e.fmt(f),
                    Self::ServiceAdmission(e) => e.fmt(f),
                    Self::Anchor(e) => e.fmt(f),
                    Self::Admission(e) => e.fmt(f),
                    Self::Service(e) => e.fmt(f),
                }
            }
        }
        impl std::error::Error for Error {}

        // Prepay the common process-hardening and fixed FD take/close mechanics. Each
        // native capability, image, transport and durable operation charges separately.
        const IO_WORK: usize = 8 + 64 * 1024 + INHERITED_CHECK_WORK;
        const FRAME: usize = 16 * 1024
            + Admission::PROCESS_STORAGE
            + Service::FD_PAIR_STORAGE
            + Client::FD_STORAGE
            + Anchor::PAIR_STORAGE
            + Key::FILE_STORAGE
            + Inputs::INPUT_STORAGE
            + Admission::READINESS_WRITER_STORAGE
            + ROOT_CONTROL_STORAGE;

        /// Runs one native protected issuer from its nominal inherited descriptor table.
        /// V2 requires FD3..11; V3 additionally requires the root endpoint at FD12.
        ///
        /// Reads no argv/environment and never dispatches or retries through V1. The
        /// process-hardening and descriptor mechanics are shared, not
        /// admitted policy/key/service owners. The separately installed native binary
        /// must be pinned by the native supervisor's program/policy capabilities.
        ///
        /// One caller-owned budget covers admission, recovery, readiness and the entire
        /// session. Scopes preserve inherited storage, cumulative work and denial
        /// history. Call only in an isolated launched child: process hardening is
        /// irreversible, and inherited descriptors are consumed. Only acknowledged
        /// cancellation succeeds. Permission denial during compiler inspection is
        /// terminal; readiness does not certify that permission or a compiler result.
        pub fn $entrypoint(b: &mut Budget<'_>) -> Result<()> {
            let floor = b.storage();
            b.with_prepaid_scope(floor, 8, IO_WORK, FRAME, run)
        }

        fn run(b: &mut Budget<'_>) -> Result<()> {
            // harden uses only setrlimit/getrlimit and prctl, never allocating FDs.
            let process = Process::harden().map_err(Error::Process)?;
            require_inherited_table()?;
            b.reserve_storage(Admission::PROCESS_STORAGE + Inputs::INPUT_STORAGE)?;
            let (inputs, charge) = Inputs::from_inherited(b)?;
            b.reserve_storage(charge.additional_storage())?;
            close_inherited(POLICY)?;
            close_inherited(MANIFEST)?;
            b.release_storage(Inputs::INPUT_STORAGE)?;

            // Independently decoded native ownership for Admission; the sealed input
            // capabilities remain live for exact pre/post-session revalidation.
            let (policy, charge) = Policy::decode(inputs.policy().canonical_bytes(), b)?;
            b.reserve_storage(charge.additional_storage())?;
            b.reserve_storage(
                Service::FD_PAIR_STORAGE
                    + Client::FD_STORAGE
                    + Anchor::PAIR_STORAGE
                    + Key::FILE_STORAGE
                    + Admission::READINESS_WRITER_STORAGE
                    + ROOT_CONTROL_STORAGE,
            )?;
            let root = take_inherited(ROOT)?;
            let peer = take_inherited(PEER)?;
            let client = take_inherited(CLIENT)?;
            let key = File::from(take_inherited(KEY)?);
            // Close original fd9 before any readiness can be emitted; supervisor waits
            // for both the exact frame and EOF, never a surviving inherited writer.
            let writer = take_inherited(READY)?;
            let anchor = take_inherited(ANCHOR)?;
            let anchor_pid = take_inherited(ANCHOR_PID)?;
            let root_control = take_root_control()?;

            let expected = inputs.manifest().client();
            let expected = Expected::new(expected.pid(), expected.uid(), expected.gid())
                .map_err(Error::ExpectedClient)?;
            let (client, charge) = Client::admit(client, expected, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (service, charge) = Service::admit(root, peer, client, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (anchor, charge) = Anchor::admit(
                anchor,
                anchor_pid,
                inputs.manifest().external_anchor_service(),
                b,
            )?;
            b.reserve_storage(charge.additional_storage())?;
            let (key, charge) = Key::from_file(key, &policy, b)?;
            b.reserve_storage(charge.additional_storage())?;
            inputs.revalidate(b)?;
            let (admission, charge) = Admission::admit(process, service, policy, key, anchor, b)?;
            b.reserve_storage(charge.additional_storage())?;
            inputs.revalidate(b)?;
            serve(admission, &inputs, writer, root_control, b)?;
            inputs.revalidate(b)?;
            Ok(())
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

            fn entrypoint(b: &mut Budget<'_>) -> Result<()> {
                $entrypoint(b)
            }

            include!("native_inherited_tests.rs");

            #[test]
            fn native_entrypoint_denies_work_before_hardening_or_inherited_io() {
                let mut work = Work::new(0);
                let mut b = Budget::new(&mut work, 1_000_000);
                b.reserve_storage(19).unwrap();
                let e = $entrypoint(&mut b).unwrap_err();
                assert!(matches!(e, Error::Resource(_)));
                assert_eq!(b.storage(), 19);
                assert_eq!(b.work(), 0);
            }

            #[test]
            fn native_entrypoint_denies_scratch_before_hardening_or_inherited_io() {
                let mut work = Work::new(IO_WORK);
                let mut b = Budget::new(&mut work, FRAME - 1);
                let e = $entrypoint(&mut b).unwrap_err();
                assert!(matches!(e, Error::Resource(_)));
                assert_eq!(b.storage(), 0);
                assert_eq!(b.work(), IO_WORK);
            }
        }
    };
}
pub(super) use native_entrypoint;
