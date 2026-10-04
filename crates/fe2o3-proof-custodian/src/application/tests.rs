use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV1 as Policy,
    CompilerExecutionServiceLaunchManifestV1 as Manifest,
    CompilerExecutionSupervisorHandoffV1 as Handoff,
};
use fe2o3_runtime_protocol::{
    WorkerV3ApplicationHandoffChallengeV1 as Challenge,
    WorkerV3ApplicationHandoffExpectationV1 as Expectation, WorkerV3ApplicationIdentityV1 as Image,
    WorkerV3ApplicationInputOccurrenceV1 as Input, WorkerV3ApplicationOccurrenceV1 as Occurrence,
    WorkerV3ApplicationRegistrationDescriptorsV1 as Descriptors,
    WorkerV3LoadEnvelopeIdentityV1 as Envelope,
};
use std::os::fd::FromRawFd;

pub(crate) fn pidfd(pid: u32) -> OwnedFd {
    // SAFETY: qualification-only original live-child/self acquisition; no pointer arguments.
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    assert!(fd >= 0, "pidfd_open: {}", io::Error::last_os_error());
    // SAFETY: successful syscall yielded an exclusively owned descriptor.
    unsafe { OwnedFd::from_raw_fd(fd as i32) }
}

// These inert image/input records are codec fixtures, not authenticated application observation.
pub(crate) fn binding(app: u32, cargo: u32, uid: u32, gid: u32, envelope: &[u8]) -> Binding {
    let mut image = vec![0; 4097];
    image[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    for (at, value) in [(16, 2_u16), (18, 62), (52, 64), (54, 56), (56, 4)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    image[20..24].copy_from_slice(&1_u32.to_le_bytes());
    image[24..32].copy_from_slice(&0x401000_u64.to_le_bytes());
    image[32..40].copy_from_slice(&64_u64.to_le_bytes());
    for (i, (kind, flags, offset, address, size, alignment)) in [
        (6_u32, 4_u32, 64_u64, 0x400040_u64, 224_u64, 8_u64),
        (1, 4, 0, 0x400000, 288, 4096),
        (1, 5, 4096, 0x401000, 1, 4096),
        (0x6474e551, 6, 0, 0, 0, 16),
    ]
    .into_iter()
    .enumerate()
    {
        let at = 64 + i * 56;
        image[at..at + 4].copy_from_slice(&kind.to_le_bytes());
        image[at + 4..at + 8].copy_from_slice(&flags.to_le_bytes());
        for (delta, value) in [
            (8, offset),
            (16, address),
            (32, size),
            (40, size),
            (48, alignment),
        ] {
            image[at + delta..at + delta + 8].copy_from_slice(&value.to_le_bytes());
        }
    }
    image[4096] = 0xc3;
    let key = |hex: &str| {
        std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
    };
    let policy = Policy::new(
        1,
        Measurement::new([1; 32], 123).unwrap(),
        Measurement::new([2; 32], 456).unwrap(),
        key("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"),
        key("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"),
    )
    .unwrap();
    let occurrence = Occurrence::new(
        Image::from_sealed_static_elf_v1(&image).unwrap(),
        [10; 32],
        &(1..=4)
            .map(|slot| Input::new(slot, [slot as u8; 32]).unwrap())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let expectation = Expectation::new(Envelope::from_exact_bytes(envelope).unwrap(), &occurrence);
    Binding::new(
        Handoff::new(
            Client::new(cargo, uid, gid).unwrap(),
            Manifest::new(
                Client::new(app, uid, gid).unwrap(),
                Anchor::new(6000, 7000).unwrap(),
                &policy,
            ),
        )
        .unwrap(),
        occurrence,
        Descriptors::new(10, 11, 12, 13).unwrap(),
        expectation,
        Challenge::from_bytes([11; 32]).unwrap(),
    )
    .unwrap()
}

struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        self.0.wait().unwrap();
    }
}

#[test]
fn capsule_requires_exact_original_processes_peer_and_transcript() {
    let mut child = Child(
        std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap(),
    );
    let app = pidfd(child.0.id());
    let cargo = pidfd(std::process::id());
    let (peer, _app_peer) = wire::control_pair().unwrap();
    let peer = wire::ControlEndpoint::admit(peer).unwrap();
    let binding = binding(
        child.0.id(),
        std::process::id(),
        rustix::process::getuid().as_raw(),
        rustix::process::getgid().as_raw(),
        b"envelope",
    );
    let transcript = Transcript::new([12; 32], [13; 32], *binding.identity().as_bytes()).unwrap();
    let capsule = Capsule::capture(binding.clone(), transcript, &app, &cargo, &peer).unwrap();
    let bytes = capsule.encode();
    assert_eq!(Capsule::decode(&bytes).unwrap().encode(), bytes);
    let wrong = Transcript::new([12; 32], [13; 32], [14; 32]).unwrap();
    assert!(Capsule::capture(binding.clone(), wrong, &app, &cargo, &peer).is_err());
    assert!(Capsule::capture(binding, transcript, &cargo, &app, &peer).is_err());
    let (substitute, _other) = wire::control_pair().unwrap();
    assert!(
        capsule
            .check_peer(&wire::ControlEndpoint::admit(substitute).unwrap())
            .is_err()
    );
    for at in [0, 12] {
        let mut changed = bytes;
        changed[at] ^= 1;
        assert!(Capsule::decode(&changed).is_err());
    }
    for range in [
        8..12,
        16..48,
        BINDING_END..BINDING_END + 32,
        BINDING_END + 32..BINDING_END + 40,
        BINDING_END + 40..CAPSULE_BYTES,
    ] {
        let mut changed = bytes;
        changed[range].fill(0);
        assert!(Capsule::decode(&changed).is_err());
    }
    for nonce in [transcript.app_nonce(), transcript.root_nonce()] {
        let mut changed = bytes;
        changed[16..48].copy_from_slice(&nonce);
        assert!(Capsule::decode(&changed).is_err());
    }
    for len in [0, CAPSULE_BYTES - 1, CAPSULE_BYTES + 1] {
        let mut changed = bytes.to_vec();
        changed.resize(len, 0);
        assert!(Capsule::decode(&changed).is_err());
    }
    let admit = |capsule| {
        StagedApplication::admit(
            capsule,
            rustix::io::fcntl_dupfd_cloexec(&app, 0).unwrap(),
            rustix::io::fcntl_dupfd_cloexec(&cargo, 0).unwrap(),
            rustix::io::fcntl_dupfd_cloexec(&peer, 0).unwrap(),
            61000,
        )
    };
    let mut changed = Capsule::decode(&bytes).unwrap();
    changed.app_start += 1;
    assert!(admit(changed).is_err());
    let mut changed = Capsule::decode(&bytes).unwrap();
    changed.cargo_start += 1;
    assert!(admit(changed).is_err());
    let staged = admit(capsule).unwrap();
    staged.revalidate().unwrap();
    child.0.kill().unwrap();
    wire::wait(
        app.as_fd(),
        rustix::event::PollFlags::IN,
        std::time::Instant::now() + std::time::Duration::from_secs(5),
    )
    .unwrap();
    assert!(staged.revalidate().is_err());
}
