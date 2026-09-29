use super::*;

// Existing native issuer ABI; the listener is used only for local root validation.
pub(super) const DESTINATIONS: [i32; 9] = [3, 4, 5, 6, 7, 8, 9, 10, 11];
pub(super) const BINDINGS_STORAGE: usize = size_of::<[Binding<'static>; 9]>();
const _: () = {
    assert!(READY_BYTES == 120);
    assert!(READY_BYTES == launch_io::MAX_PIPE_READY_BYTES);
    assert!(READY_BYTES > launch_io::MAX_READY_BYTES);
};

pub(super) struct Channels {
    pub(super) ready_reader: OwnedFd,
    pub(super) ready_writer: OwnedFd,
    pub(super) profile_reader: OwnedFd,
    pub(super) profile_writer: OwnedFd,
    pub(super) gate_reader: OwnedFd,
    pub(super) gate_writer: OwnedFd,
    pub(super) exec_reader: OwnedFd,
    pub(super) exec_writer: OwnedFd,
}
pub(super) struct Readers {
    pub ready: OwnedFd,
    pub profile: OwnedFd,
    pub gate: OwnedFd,
    pub exec: OwnedFd,
}
impl Channels {
    pub const STORAGE: usize = 8 * FILE_STORAGE;
    pub fn new() -> Result<Self> {
        let pipe = || {
            pipe::pipe_with(pipe::PipeFlags::CLOEXEC | pipe::PipeFlags::NONBLOCK)
                .map_err(|e| launch::io("create issuer readiness pipe", e))
        };
        let (ready_reader, ready_writer) = pipe()?;
        let (profile_reader, profile_writer) = pipe()?;
        let (gate_reader, gate_writer) = pipe::pipe_with(pipe::PipeFlags::CLOEXEC)
            .map_err(|e| launch::io("create issuer release gate", e))?;
        let (exec_reader, exec_writer) = net::socketpair(
            net::AddressFamily::UNIX,
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|e| launch::io("create issuer exec status", e))?;
        Ok(Self {
            ready_reader,
            ready_writer,
            profile_reader,
            profile_writer,
            gate_reader,
            gate_writer,
            exec_reader,
            exec_writer,
        })
    }
    pub fn close_child_ends(self) -> Readers {
        let Self {
            ready_reader,
            ready_writer,
            profile_reader,
            profile_writer,
            gate_reader,
            gate_writer,
            exec_reader,
            exec_writer,
        } = self;
        drop(ready_writer);
        drop(profile_writer);
        drop(gate_reader);
        drop(exec_writer);
        Readers {
            ready: ready_reader,
            profile: profile_reader,
            gate: gate_writer,
            exec: exec_reader,
        }
    }
}

pub(super) fn source_storage(issuer_bytes: u64) -> Result<usize> {
    sum(&[
        Image::file_storage_for_length(issuer_bytes)?,
        Inputs::PAIR_STORAGE,
        PolicyCap::FILE_STORAGE,
        ServiceKey::FILE_STORAGE,
        ManifestCap::FILE_STORAGE,
        AnchorTransfer::STORAGE,
        6 * FILE_STORAGE,
        BINDINGS_STORAGE,
    ])
}

#[allow(unsafe_code)]
pub(super) fn stage<T: Send + 'static>(
    p: &Payload<T>,
    anchor: AnchorTransfer,
    peer: BorrowedFd<'_>,
    pidfd: BorrowedFd<'_>,
    channels: &Channels,
    b: &mut Budget<'_>,
) -> Result<(Stage, usize)> {
    let floor = sum(&[
        p.retained,
        AnchorTransfer::STORAGE,
        Channels::STORAGE,
        2 * FILE_STORAGE,
    ])?;
    b.with_prepaid_scope(floor, 8, LOCAL_WORK, FRAME, |b| {
        let prepared = &p.prepared;
        prepared.revalidate(b)?;
        let (issuer, c) = prepared.programs[2].try_clone_for_exec(b)?;
        b.reserve_storage(c.additional_storage())?;
        let ((listener, root), c) = prepared.service_inputs.try_clone_ordered_for_spawn(b)?;
        b.reserve_storage(c.additional_storage())?;
        let (policy, c) = prepared.trust.policy().try_clone_for_transfer(b)?;
        b.reserve_storage(c.additional_storage())?;
        let (key, c) = p.key.try_clone_for_transfer(
            prepared.trust.key_template(),
            prepared.trust.deployment().deployment(),
            prepared.trust.policy().policy(),
            b,
        )?;
        b.reserve_storage(c.additional_storage())?;
        let (manifest, c) = p.manifest.try_clone_for_transfer(b)?;
        b.reserve_storage(c.additional_storage())?;
        let (anchor_peer, anchor_pidfd) = anchor.into_ordered_descriptors(b)?;
        // Conservatively keep the full consumed anchor transfer reservation until
        // the enclosing scope retires all source aliases after final validation.
        b.reserve_storage(BINDINGS_STORAGE)?;
        let sources = [
            root.as_fd(),
            peer,
            pidfd,
            policy.as_fd(),
            key.as_fd(),
            manifest.as_fd(),
            channels.ready_writer.as_fd(),
            anchor_peer.as_fd(),
            anchor_pidfd.as_fd(),
        ];
        let bindings = bindings(sources)?;
        let source = source_storage(prepared.programs[2].measurement().byte_len())?;
        // SAFETY: the closed table covers all borrowed source bytes and descriptors,
        // including full issuer image, local listener/root pair and control channels.
        // Their original typed owners remain charged; final Stage aliases are checked
        // immediately below against those same owners before clone/gate release.
        let (stage, c) = unsafe {
            Stage::stage(
                &issuer,
                &bindings,
                channels.profile_writer.as_fd(),
                channels.gate_reader.as_fd(),
                channels.exec_writer.as_fd(),
                source,
                b,
            )
        }?;
        b.reserve_storage(c.additional_storage())?;
        validate(p, &stage, listener.as_fd(), peer, pidfd, channels, b)?;
        Ok((stage, c.additional_storage()))
    })
}

pub(super) fn bindings(sources: [BorrowedFd<'_>; 9]) -> Result<[Binding<'_>; 9]> {
    let mut bindings = [Binding::new(sources[0], DESTINATIONS[0])
        .map_err(|_| Error::Invalid("invalid issuer descriptor binding"))?;
        9];
    for ((binding, source), destination) in bindings.iter_mut().zip(sources).zip(DESTINATIONS) {
        *binding = Binding::new(source, destination)
            .map_err(|_| Error::Invalid("invalid issuer descriptor binding"))?;
    }
    Ok(bindings)
}

#[allow(clippy::too_many_arguments)]
fn validate<T: Send + 'static>(
    p: &Payload<T>,
    stage: &Stage,
    listener: BorrowedFd<'_>,
    peer: BorrowedFd<'_>,
    pidfd: BorrowedFd<'_>,
    channels: &Channels,
    b: &mut Budget<'_>,
) -> Result<()> {
    let prepared = &p.prepared;
    prepared.revalidate(b)?;
    let file = |fd| {
        stage
            .binding(fd)
            .ok_or(Error::Invalid("missing issuer staged input"))
    };
    prepared.programs[2].revalidate_exec_clone(stage.executable(), b)?;
    prepared
        .service_inputs
        .validate_transfer(listener, file(3)?, b)?;
    prepared.trust.policy().validate_transfer(file(6)?, b)?;
    p.key.validate_transfer(
        file(7)?,
        prepared.trust.key_template(),
        prepared.trust.deployment().deployment(),
        prepared.trust.policy().policy(),
        b,
    )?;
    p.manifest.validate_transfer(file(8)?, b)?;
    prepared.anchor.validate_supervisor_transfer(
        file(10)?.as_fd(),
        file(11)?.as_fd(),
        prepared.trust.deployment(),
        prepared.trust.policy(),
        b,
    )?;
    validate_duplicate(peer, file(4)?.as_fd())?;
    validate_duplicate(pidfd, file(5)?.as_fd())?;
    validate_peer(file(4)?.as_fd(), p.manifest.manifest().client())?;
    require_idle(file(5)?.as_fd())?;
    validate_duplicate(channels.ready_writer.as_fd(), file(9)?.as_fd())?;
    if fs::FileType::from_raw_mode(
        fs::fstat(file(9)?)
            .map_err(|e| launch::io("inspect issuer ready pipe", e))?
            .st_mode,
    ) != fs::FileType::Fifo
        || fs::fcntl_getfl(file(9)?).map_err(|e| launch::io("inspect issuer ready pipe mode", e))?
            & fs::OFlags::ACCMODE
            != fs::OFlags::WRONLY
    {
        return Err(Error::Invalid("issuer readiness requires a pipe writer"));
    }
    Ok(())
}

// Private to this closed Stage schedule: provenance is the original callback FD,
// never descriptor import. Check the final frozen duplicate, not just credentials.
pub(super) fn validate_duplicate(original: BorrowedFd<'_>, staged: BorrowedFd<'_>) -> Result<()> {
    let original_stat = fs::fstat(original).map_err(|e| launch::io("inspect issuer source", e))?;
    let staged_stat =
        fs::fstat(staged).map_err(|e| launch::io("inspect staged issuer source", e))?;
    if (
        original_stat.st_dev,
        original_stat.st_ino,
        original_stat.st_mode,
        original_stat.st_rdev,
    ) != (
        staged_stat.st_dev,
        staged_stat.st_ino,
        staged_stat.st_mode,
        staged_stat.st_rdev,
    ) || io::fcntl_getfd(staged)
        .map_err(|e| launch::io("inspect staged issuer descriptor flags", e))?
        != io::FdFlags::CLOEXEC
        || fs::fcntl_getfl(original).map_err(|e| launch::io("inspect issuer source access", e))?
            != fs::fcntl_getfl(staged).map_err(|e| launch::io("inspect staged issuer access", e))?
    {
        return Err(Error::Invalid(
            "issuer staged source is not the original duplicate",
        ));
    }
    Ok(())
}
pub(super) fn validate_peer(peer: BorrowedFd<'_>, client: Client) -> Result<()> {
    let actual = net::sockopt::socket_peercred(peer)
        .map_err(|e| launch::io("inspect issuer client peer", e))?;
    if net::sockopt::socket_type(peer).map_err(|e| launch::io("inspect issuer client socket", e))?
        != net::SocketType::SEQPACKET
        || fs::fcntl_getfl(peer).map_err(|e| launch::io("inspect issuer client access", e))?
            & fs::OFlags::ACCMODE
            != fs::OFlags::RDWR
        || u32::try_from(actual.pid.as_raw_nonzero().get()).ok() != Some(client.pid())
        || actual.uid.as_raw() != client.uid()
        || actual.gid.as_raw() != client.gid()
    {
        return Err(Error::Invalid("issuer client peer identity changed"));
    }
    require_idle(peer)
}
fn require_idle(fd: BorrowedFd<'_>) -> Result<()> {
    let mut fds = [event::PollFd::new(&fd, event::PollFlags::IN)];
    event::poll(
        &mut fds,
        Some(&event::Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )
    .map_err(|e| launch::io("inspect issuer source liveness", e))?;
    if !fds[0].revents().is_empty() {
        return Err(Error::Invalid("issuer source closed or already readable"));
    }
    Ok(())
}
