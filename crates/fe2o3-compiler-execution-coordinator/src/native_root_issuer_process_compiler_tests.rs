use super::*;
use crate::native_launch as native;
use fe2o3_compiler_execution_supervisor::IssuerServiceCredentialProfileV1 as Credentials;
use std::{
    ffi::CString,
    fs::File,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

pub(super) struct Backing {
    _image: File,
    _cwd: File,
    _unused: File,
    _publication: Option<PublicationInputs>,
    drops: Arc<AtomicUsize>,
}

// Immutable launch inputs only. No artifact lease/token is inherited by clone.
pub(super) struct PublicationInputs {
    pub argv: Vec<CString>,
    pub environment: Vec<CString>,
    pub invocation: File,
    pub backend: File,
    pub artifact: File,
}
impl PublicationInputs {
    pub(super) fn storage(&self) -> usize {
        size_of::<Self>()
            + 3 * FILE_STORAGE
            + usize::try_from(self.invocation.metadata().unwrap().len()).unwrap()
            + usize::try_from(self.backend.metadata().unwrap().len()).unwrap()
            + (self.argv.capacity() + self.environment.capacity()) * size_of::<CString>()
            + self
                .argv
                .iter()
                .chain(&self.environment)
                .map(|s| s.as_bytes_with_nul().len())
                .sum::<usize>()
    }
}
impl Drop for Backing {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

// Uses the audited child-channel stage, original clone pidfd and trace seizure.
// No Prepared, occurrence or provider implementation is fabricated. Publication
// tests separately supply explicitly inert handoff content, not proof authority.
#[allow(unsafe_code)]
pub(super) fn confirmed<'work>(
    f: &fixtures::Fixture,
    uid: u32,
    pool: &mut Cleanup,
    b: &mut Budget<'work>,
) -> (CompilerTrace<'work, Backing>, OwnedFd, Arc<AtomicUsize>) {
    confirmed_inputs(f, uid, None, pool, b)
}

pub(super) fn confirmed_publication<'work>(
    f: &fixtures::Fixture,
    inputs: PublicationInputs,
    pool: &mut Cleanup,
    b: &mut Budget<'work>,
) -> (CompilerTrace<'work, Backing>, OwnedFd, Arc<AtomicUsize>) {
    confirmed_inputs(f, fixtures::CLIENT, Some(inputs), pool, b)
}

#[allow(unsafe_code)]
fn confirmed_inputs<'work>(
    f: &fixtures::Fixture,
    uid: u32,
    publication: Option<PublicationInputs>,
    pool: &mut Cleanup,
    b: &mut Budget<'work>,
) -> (CompilerTrace<'work, Backing>, OwnedFd, Arc<AtomicUsize>) {
    let floor = b.storage();
    let result = b
        .with_prepaid_scope(floor, 0, 0, 0, |b| -> Result<_> {
            let image = File::open(&f.compiler).unwrap();
            let cwd = File::open(f.dir.path()).unwrap();
            let unused = File::open("/dev/null").unwrap();
            let channels = native::Channels::new().unwrap();
            let argv = [CString::new("native-root-issuer-compiler").unwrap()];
            let ordinary = [Binding::new(unused.as_fd(), 198).unwrap()];
            let publication_bindings;
            let (argv, environment, bindings) = match &publication {
                Some(p) => {
                    publication_bindings = [
                        Binding::new(
                            p.invocation.as_fd(),
                            fe2o3_compiler_closure_capability::RUSTC_INVOCATION_CHILD_FD_V1,
                        )
                        .unwrap(),
                        Binding::new(
                            p.backend.as_fd(),
                            fe2o3_artifact_transaction::BROKERED_CODEGEN_BACKEND_CHILD_FD_V1,
                        )
                        .unwrap(),
                        Binding::new(
                            p.artifact.as_fd(),
                            fe2o3_artifact_transaction::BROKERED_ARTIFACT_DIRECTORY_CHILD_FD_V1,
                        )
                        .unwrap(),
                    ];
                    (
                        p.argv.as_slice(),
                        p.environment.as_slice(),
                        publication_bindings.as_slice(),
                    )
                }
                None => (argv.as_slice(), [].as_slice(), ordinary.as_slice()),
            };
            // Full image plus bounded metadata/FD/string/Arc/control-channel envelope.
            let source = usize::try_from(image.metadata().unwrap().len()).unwrap()
                + publication.as_ref().map_or(0, PublicationInputs::storage)
                + 32768;
            b.reserve_storage(source)?;
            // SAFETY: every source and the stage stay charged and live through clone;
            // this fixture's first instruction stays held for the entire test.
            let (stage, c) = unsafe {
                Stage::stage_compiler_with_child_channel(
                    &image,
                    argv,
                    environment,
                    cwd.as_fd(),
                    [None; 3],
                    bindings,
                    channels.profile_writer.as_fd(),
                    channels.gate_reader.as_fd(),
                    channels.exec_writer.as_fd(),
                    channels.child.as_fd(),
                    source,
                    b,
                )
            }?;
            b.reserve_storage(c.additional_storage())?;
            let credentials = Credentials::new(uid, uid).unwrap();
            let drops = Arc::new(AtomicUsize::new(0));
            // SAFETY: close-only Files, this bounded fixture's argv/environment
            // allocations and one atomic increment are funded and nonpanicking;
            // the root creator and cleanup controller survive.
            let (child, c) = unsafe {
                stage.spawn_retaining(
                    credentials,
                    Backing {
                        _image: image,
                        _cwd: cwd,
                        _unused: unused,
                        _publication: publication,
                        drops: Arc::clone(&drops),
                    },
                    source,
                    pool,
                    b,
                )
            }?;
            b.reserve_storage(c.additional_storage())?;
            let deadline = Instant::now() + TIMEOUT;
            launch_io::await_profile_ready(
                channels.profile_reader.as_fd(),
                channels.exec_reader.as_fd(),
                &mut Observer {
                    child: &child,
                    budget: b,
                },
                deadline,
            )?;
            fe2o3_protected_service_profile::observations::validate_process(
                credentials,
                child.pid(),
            )
            .unwrap();
            let (exit, c) = child.try_clone_pidfd(b)?;
            b.reserve_storage(c.additional_storage())?;
            assert_eq!(c.additional_storage(), FILE_STORAGE);
            let native::Channels {
                root,
                child: sender,
                exec_reader,
                exec_writer,
                profile_reader,
                profile_writer,
                gate_reader,
                gate_writer,
            } = channels;
            let (mut trace, full) = CompilerTrace::receive(child, root, credentials, deadline, b)?;
            // The outer scope keeps all overlapping source and trace charges until
            // stage/control retirement; only full returned owners escape that scope.
            b.reserve_storage(full)?;
            drop(stage);
            drop((
                sender,
                exec_writer,
                profile_reader,
                profile_writer,
                gate_reader,
            ));
            assert_eq!(
                io::write(
                    &gate_writer,
                    &[fe2o3_protected_service_spawn::PROTECTED_SERVICE_GATE_RELEASE_V1]
                )
                .unwrap(),
                1
            );
            drop(gate_writer);
            let mut held = false;
            for _ in 0..10000 {
                let event = trace.poll(b)?;
                if !event.is_pending() {
                    assert!(
                        event.is_exec(),
                        "compiler must stop at exact first exec: {event:?}"
                    );
                    held = true;
                    break;
                }
                assert!(Instant::now() < deadline, "compiler exec deadline");
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(held, "missing actual compiler exec stop");
            assert_eq!(
                net::recv(&exec_reader, &mut [0; 1], net::RecvFlags::DONTWAIT).unwrap(),
                (0, 0)
            );
            drop(exec_reader);
            // SAFETY: original held first exec and status EOF observed; every parent
            // stage/status alias closed. Native bootstrap cannot fork, and no fixture
            // instruction has run. No artifact-lock aliases were staged.
            unsafe {
                trace.confirm_exec(b)?;
            }
            Ok((trace, exit, drops))
        })
        .expect("actual compiler channel and confirmed first exec");
    b.reserve_storage(result.0.retained_storage() + FILE_STORAGE)
        .unwrap();
    result
}
