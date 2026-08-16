use crate::digest::Sha256Digest;
use crate::error::{HostLinkError, HostLinkErrorCodeV1, ResultContext};
use std::fs::File;

pub const MAX_HOST_LINK_RESULT_RECORD_BYTES_V1: usize = 512;
pub const HOST_LINK_RESULT_COPY_POLICY_V1: &str = "receiver-owned-memfd-v1";
const RESULT_PREFIX_V1: &str = "fe2o3-host-lld-result-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostLinkResultRecordV1 {
    plan_digest: Sha256Digest,
    closure_digest: Sha256Digest,
    nonce_sha256: Sha256Digest,
    output_sha256: Sha256Digest,
    output_length: u64,
}

impl HostLinkResultRecordV1 {
    pub fn new(
        plan_digest: Sha256Digest,
        closure_digest: Sha256Digest,
        nonce_sha256: Sha256Digest,
        output_sha256: Sha256Digest,
        output_length: u64,
    ) -> Result<Self, HostLinkError> {
        let record = Self {
            plan_digest,
            closure_digest,
            nonce_sha256,
            output_sha256,
            output_length,
        };
        record.validate()?;
        Ok(record)
    }

    pub const fn plan_digest(&self) -> Sha256Digest {
        self.plan_digest
    }

    pub const fn closure_digest(&self) -> Sha256Digest {
        self.closure_digest
    }

    pub const fn nonce_sha256(&self) -> Sha256Digest {
        self.nonce_sha256
    }

    pub const fn output_sha256(&self) -> Sha256Digest {
        self.output_sha256
    }

    pub const fn output_length(&self) -> u64 {
        self.output_length
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>, HostLinkError> {
        self.validate()?;
        let record = format!(
            "{RESULT_PREFIX_V1}\tplan={}\tclosure={}\tnonce={}\tsha256={}\tlength={}\tcopy={HOST_LINK_RESULT_COPY_POLICY_V1}\n",
            self.plan_digest,
            self.closure_digest,
            self.nonce_sha256,
            self.output_sha256,
            self.output_length,
        )
        .into_bytes();
        if record.len() > MAX_HOST_LINK_RESULT_RECORD_BYTES_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "canonical host-link result exceeds its byte bound",
            ));
        }
        Ok(record)
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, HostLinkError> {
        if bytes.is_empty() || bytes.len() > MAX_HOST_LINK_RESULT_RECORD_BYTES_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "host-link result size is outside its exact bound",
            ));
        }
        if !bytes.ends_with(b"\n")
            || bytes[..bytes.len() - 1].contains(&b'\n')
            || bytes.contains(&b'\r')
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::NonCanonicalWire,
                "host-link result must contain one final LF",
            ));
        }
        if !bytes[..bytes.len() - 1]
            .iter()
            .all(|byte| *byte == b'\t' || (0x20..=0x7e).contains(byte))
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidText,
                "host-link result contains noncanonical ASCII",
            ));
        }
        let text = std::str::from_utf8(&bytes[..bytes.len() - 1]).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::InvalidText,
                "host-link result is not ASCII",
            )
        })?;
        let fields = text.split('\t').collect::<Vec<_>>();
        if fields.len() != 7 || fields[0] != RESULT_PREFIX_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidWire,
                "host-link result has the wrong field count or protocol selector",
            ));
        }
        let record = Self::new(
            parse_digest_field(fields[1], "plan")?,
            parse_digest_field(fields[2], "closure")?,
            parse_digest_field(fields[3], "nonce")?,
            parse_digest_field(fields[4], "sha256")?,
            parse_decimal_field(fields[5], "length")?,
        )?;
        parse_copy_policy_field(fields[6])?;
        if record.encode_canonical()? != bytes {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::NonCanonicalWire,
                "host-link result does not round-trip canonically",
            ));
        }
        Ok(record)
    }

    fn validate(&self) -> Result<(), HostLinkError> {
        if self.plan_digest == Sha256Digest::ZERO
            || self.closure_digest == Sha256Digest::ZERO
            || self.nonce_sha256 == Sha256Digest::ZERO
            || self.output_sha256 == Sha256Digest::ZERO
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidWire,
                "host-link result digests must be nonzero",
            ));
        }
        if self.output_length == 0 || self.output_length > crate::MAX_HOST_LINK_OUTPUT_BYTES_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactTooLarge,
                "host-link result length is outside its admitted bound",
            ));
        }
        Ok(())
    }
}

fn parse_digest_field(field: &str, name: &str) -> Result<Sha256Digest, HostLinkError> {
    let value = field
        .strip_prefix(name)
        .and_then(|value| value.strip_prefix('='));
    let Some(value) = value else {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NonCanonicalWire,
            format!("host-link result is missing canonical {name} field"),
        ));
    };
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NonCanonicalWire,
            format!("host-link result {name} is not lowercase SHA-256 hex"),
        ));
    }
    let mut digest = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        digest[index] = (hex_nibble(chunk[0]) << 4) | hex_nibble(chunk[1]);
    }
    Ok(Sha256Digest::from_bytes(digest))
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("hex was validated before conversion"),
    }
}

fn parse_decimal_field(field: &str, name: &str) -> Result<u64, HostLinkError> {
    let value = field
        .strip_prefix(name)
        .and_then(|value| value.strip_prefix('='));
    let Some(value) = value else {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NonCanonicalWire,
            format!("host-link result is missing canonical {name} field"),
        ));
    };
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NonCanonicalWire,
            format!("host-link result {name} is not canonical decimal"),
        ));
    }
    value.parse().map_err(|_| {
        HostLinkError::new(
            HostLinkErrorCodeV1::FieldTooLarge,
            format!("host-link result {name} does not fit u64"),
        )
    })
}

fn parse_copy_policy_field(field: &str) -> Result<(), HostLinkError> {
    if field.strip_prefix("copy=") != Some(HOST_LINK_RESULT_COPY_POLICY_V1) {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NonCanonicalWire,
            "host-link result must bind the exact receiver-owned copy policy",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SocketIdentityV1 {
    pub device: u64,
    pub inode: u64,
}

pub(crate) struct HostLinkResultChannelV1 {
    receiver: File,
    receiver_identity: SocketIdentityV1,
    child_identity: SocketIdentityV1,
}

pub(crate) enum ResultChannelReadV1 {
    Pending,
    Closed,
    Packet(HostLinkResultRecordV1, File),
}

impl HostLinkResultChannelV1 {
    pub(crate) fn new() -> Result<(Self, File), HostLinkError> {
        platform::new_channel()
    }

    pub(crate) const fn child_identity(&self) -> SocketIdentityV1 {
        self.child_identity
    }

    pub(crate) fn revalidate_receiver(&self) -> Result<(), HostLinkError> {
        platform::validate_endpoint(&self.receiver, self.receiver_identity)
    }

    pub(crate) fn revalidate_child(&self, child: &File) -> Result<(), HostLinkError> {
        platform::validate_endpoint(child, self.child_identity)
    }

    pub(crate) fn try_receive(
        &self,
        expected_worker_pid: u32,
    ) -> Result<ResultChannelReadV1, HostLinkError> {
        self.revalidate_receiver()?;
        let received = match platform::try_receive_raw(&self.receiver)? {
            platform::RawResultReadV1::Pending => ResultChannelReadV1::Pending,
            platform::RawResultReadV1::Closed => ResultChannelReadV1::Closed,
            platform::RawResultReadV1::Packet(record, mut descriptors, credentials_pid) => {
                if credentials_pid != Some(expected_worker_pid) {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::WorkerIdentity,
                        "host-link result sender is not the exact authenticated static LLD child",
                    ));
                }
                if descriptors.len() != 1 {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::InvalidWire,
                        format!(
                            "host-link result carried {} output descriptors instead of exactly one",
                            descriptors.len()
                        ),
                    ));
                }
                let output = descriptors.pop().expect("exact descriptor count checked");
                ResultChannelReadV1::Packet(
                    HostLinkResultRecordV1::decode_canonical(&record)?,
                    output,
                )
            }
        };
        self.revalidate_receiver()?;
        Ok(received)
    }

    pub(crate) fn poll_write_closed(&self) -> Result<bool, HostLinkError> {
        self.revalidate_receiver()?;
        let closed = match platform::try_receive_raw(&self.receiver)? {
            platform::RawResultReadV1::Pending => false,
            platform::RawResultReadV1::Closed => true,
            platform::RawResultReadV1::Packet(_, _, _) => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::DuplicateRecord,
                    "host-link worker sent more than one result packet",
                ));
            }
        };
        self.revalidate_receiver()?;
        Ok(closed)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use rustix::net::{
        AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SocketFlags, SocketType,
    };
    use std::io::IoSliceMut;
    use std::mem::MaybeUninit;
    use std::os::unix::fs::MetadataExt;

    pub(super) enum RawResultReadV1 {
        Pending,
        Closed,
        Packet(Vec<u8>, Vec<File>, Option<u32>),
    }

    pub(super) fn new_channel() -> Result<(HostLinkResultChannelV1, File), HostLinkError> {
        let (receiver, child) = rustix::net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .context(HostLinkErrorCodeV1::Io, || {
            "create AF_UNIX SOCK_SEQPACKET host-link result channel".to_owned()
        })?;
        let receiver = File::from(receiver);
        let child = File::from(child);
        rustix::net::sockopt::set_socket_passcred(&receiver, true)
            .context(HostLinkErrorCodeV1::Io, || {
                "enable authenticated credentials on host-link result receiver".to_owned()
            })?;
        let receiver_identity = endpoint_identity(&receiver)?;
        let child_identity = endpoint_identity(&child)?;
        Ok((
            HostLinkResultChannelV1 {
                receiver,
                receiver_identity,
                child_identity,
            },
            child,
        ))
    }

    fn endpoint_identity(file: &File) -> Result<SocketIdentityV1, HostLinkError> {
        if rustix::net::sockopt::socket_type(file)
            .context(HostLinkErrorCodeV1::DescriptorChanged, || {
                "inspect result endpoint socket type".to_owned()
            })?
            != SocketType::SEQPACKET
            || rustix::net::sockopt::socket_domain(file)
                .context(HostLinkErrorCodeV1::DescriptorChanged, || {
                    "inspect result endpoint socket domain".to_owned()
                })?
                != AddressFamily::UNIX
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DescriptorChanged,
                "result endpoint is not AF_UNIX SOCK_SEQPACKET",
            ));
        }
        let metadata = file.metadata().context(HostLinkErrorCodeV1::Io, || {
            "fstat host-link result endpoint".to_owned()
        })?;
        Ok(SocketIdentityV1 {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    pub(super) fn validate_endpoint(
        file: &File,
        expected: SocketIdentityV1,
    ) -> Result<(), HostLinkError> {
        if endpoint_identity(file)? != expected {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DescriptorChanged,
                "host-link result endpoint identity changed",
            ));
        }
        Ok(())
    }

    pub(super) fn try_receive_raw(receiver: &File) -> Result<RawResultReadV1, HostLinkError> {
        let mut bytes = [0_u8; MAX_HOST_LINK_RESULT_RECORD_BYTES_V1 + 1];
        let mut ancillary_space =
            [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2), ScmCredentials(1))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut ancillary_space);
        let received = {
            let mut iov = [IoSliceMut::new(&mut bytes)];
            match rustix::net::recvmsg(
                receiver,
                &mut iov,
                &mut ancillary,
                RecvFlags::CMSG_CLOEXEC | RecvFlags::TRUNC | RecvFlags::DONTWAIT,
            ) {
                Ok(received) => received,
                Err(error) if error == rustix::io::Errno::AGAIN => {
                    return Ok(RawResultReadV1::Pending);
                }
                Err(error) => {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::Io,
                        format!("receive host-link result packet: {error}"),
                    ));
                }
            }
        };
        if received
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
            || received.bytes > MAX_HOST_LINK_RESULT_RECORD_BYTES_V1
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "host-link result packet or descriptor control was truncated",
            ));
        }
        let received_bytes = received.bytes;
        let mut descriptors = Vec::new();
        let mut credentials_pid = None;
        let mut carried_ancillary = false;
        for message in ancillary.drain() {
            carried_ancillary = true;
            match message {
                RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
                RecvAncillaryMessage::ScmCredentials(credentials) => {
                    let pid = u32::try_from(credentials.pid.as_raw_pid()).map_err(|_| {
                        HostLinkError::new(
                            HostLinkErrorCodeV1::WorkerIdentity,
                            "host-link result carried an invalid sender PID",
                        )
                    })?;
                    if credentials_pid.replace(pid).is_some() {
                        return Err(HostLinkError::new(
                            HostLinkErrorCodeV1::DuplicateRecord,
                            "host-link result carried duplicate sender credentials",
                        ));
                    }
                }
                _ => {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::InvalidWire,
                        "host-link result carried unsupported ancillary data",
                    ));
                }
            }
        }
        if received_bytes == 0 {
            if !carried_ancillary {
                return Ok(RawResultReadV1::Closed);
            }
            // SOCK_SEQPACKET permits an empty datagram. SO_PASSCRED makes such
            // a datagram distinguishable from orderly shutdown: the datagram
            // carries SCM_CREDENTIALS, while transport EOF carries no control
            // messages. Preserve it as a packet so duplicate/shape checks run.
            return Ok(RawResultReadV1::Packet(
                Vec::new(),
                descriptors.into_iter().map(File::from).collect(),
                credentials_pid,
            ));
        }
        Ok(RawResultReadV1::Packet(
            bytes[..received_bytes].to_vec(),
            descriptors.into_iter().map(File::from).collect(),
            credentials_pid,
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use rustix::net::{
            SendAncillaryBuffer, SendAncillaryMessage, SendFlags, Shutdown, sendmsg, shutdown,
        };
        use std::io::{IoSlice, Write};
        use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
        use std::process::Command;
        use tempfile::tempfile;

        fn send_packet(child: &File, bytes: &[u8], descriptors: &[BorrowedFd<'_>]) {
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
            let mut ancillary = SendAncillaryBuffer::new(&mut space);
            if !descriptors.is_empty() {
                assert!(ancillary.push(SendAncillaryMessage::ScmRights(descriptors)));
            }
            assert_eq!(
                sendmsg(
                    child,
                    &[IoSlice::new(bytes)],
                    &mut ancillary,
                    SendFlags::NOSIGNAL,
                )
                .unwrap(),
                bytes.len()
            );
        }

        fn current_pid() -> u32 {
            u32::try_from(rustix::process::getpid().as_raw_pid()).unwrap()
        }

        fn receive_error_code(channel: &HostLinkResultChannelV1) -> HostLinkErrorCodeV1 {
            match channel.try_receive(current_pid()) {
                Err(error) => error.code(),
                Ok(_) => panic!("hostile result packet was unexpectedly accepted"),
            }
        }

        fn result_record() -> HostLinkResultRecordV1 {
            HostLinkResultRecordV1::new(
                Sha256Digest::from_bytes([1; 32]),
                Sha256Digest::from_bytes([2; 32]),
                Sha256Digest::from_bytes([3; 32]),
                Sha256Digest::from_bytes([4; 32]),
                64,
            )
            .unwrap()
        }

        #[test]
        fn ordinary_file_is_not_a_result_endpoint() {
            let error = endpoint_identity(&tempfile().unwrap()).unwrap_err();
            assert_eq!(error.code(), HostLinkErrorCodeV1::DescriptorChanged);
        }

        #[test]
        fn datagram_socket_is_not_a_result_endpoint() {
            let socket = rustix::net::socket_with(
                AddressFamily::UNIX,
                SocketType::DGRAM,
                SocketFlags::CLOEXEC,
                None,
            )
            .unwrap();
            let error = endpoint_identity(&File::from(socket)).unwrap_err();
            assert_eq!(error.code(), HostLinkErrorCodeV1::DescriptorChanged);
        }

        #[test]
        fn substituted_seqpacket_endpoint_has_the_wrong_identity() {
            let (channel, _) = HostLinkResultChannelV1::new().unwrap();
            let (_, substituted) = HostLinkResultChannelV1::new().unwrap();
            let error = channel.revalidate_child(&substituted).unwrap_err();
            assert_eq!(error.code(), HostLinkErrorCodeV1::DescriptorChanged);
        }

        #[test]
        fn pending_does_not_consume_the_result_channel() {
            let (channel, child) = HostLinkResultChannelV1::new().unwrap();
            assert!(matches!(
                channel.try_receive(current_pid()).unwrap(),
                ResultChannelReadV1::Pending
            ));
            let mut output = tempfile().unwrap();
            output.write_all(b"output").unwrap();
            send_packet(&child, b"not-canonical\n", &[output.as_fd()]);
            assert_eq!(
                receive_error_code(&channel),
                HostLinkErrorCodeV1::InvalidWire
            );
        }

        #[test]
        fn zero_and_multiple_rights_are_rejected() {
            let (channel, child) = HostLinkResultChannelV1::new().unwrap();
            send_packet(&child, b"record\n", &[]);
            assert_eq!(
                receive_error_code(&channel),
                HostLinkErrorCodeV1::InvalidWire
            );

            let (channel, child) = HostLinkResultChannelV1::new().unwrap();
            let first = tempfile().unwrap();
            let second = tempfile().unwrap();
            send_packet(&child, b"record\n", &[first.as_fd(), second.as_fd()]);
            assert_eq!(
                receive_error_code(&channel),
                HostLinkErrorCodeV1::InvalidWire
            );
        }

        #[test]
        fn credentialed_empty_seqpacket_is_not_transport_eof() {
            let (channel, child) = HostLinkResultChannelV1::new().unwrap();
            send_packet(&child, b"", &[]);
            assert_eq!(
                receive_error_code(&channel),
                HostLinkErrorCodeV1::InvalidWire
            );
        }

        #[test]
        fn credentialed_empty_packet_after_result_is_a_duplicate() {
            let (channel, child) = HostLinkResultChannelV1::new().unwrap();
            let output = tempfile().unwrap();
            let canonical = result_record().encode_canonical().unwrap();
            send_packet(&child, &canonical, &[output.as_fd()]);
            assert!(matches!(
                channel.try_receive(current_pid()).unwrap(),
                ResultChannelReadV1::Packet(_, _)
            ));

            send_packet(&child, b"", &[]);
            send_packet(&child, &canonical, &[output.as_fd()]);
            shutdown(&child, Shutdown::Write).unwrap();
            assert_eq!(
                channel.poll_write_closed().unwrap_err().code(),
                HostLinkErrorCodeV1::DuplicateRecord
            );
        }

        #[test]
        fn credentialed_empty_packet_with_rights_is_never_eof() {
            let (channel, child) = HostLinkResultChannelV1::new().unwrap();
            let output = tempfile().unwrap();
            send_packet(&child, b"", &[output.as_fd()]);
            assert_eq!(
                receive_error_code(&channel),
                HostLinkErrorCodeV1::FieldTooLarge
            );

            let (channel, child) = HostLinkResultChannelV1::new().unwrap();
            let canonical = result_record().encode_canonical().unwrap();
            send_packet(&child, &canonical, &[output.as_fd()]);
            assert!(matches!(
                channel.try_receive(current_pid()).unwrap(),
                ResultChannelReadV1::Packet(_, _)
            ));
            send_packet(&child, b"", &[output.as_fd()]);
            shutdown(&child, Shutdown::Write).unwrap();
            assert_eq!(
                channel.poll_write_closed().unwrap_err().code(),
                HostLinkErrorCodeV1::DuplicateRecord
            );
        }

        #[test]
        fn only_ancillary_free_zero_length_receive_is_transport_eof() {
            let (channel, child) = HostLinkResultChannelV1::new().unwrap();
            shutdown(&child, Shutdown::Write).unwrap();
            assert!(matches!(
                channel.try_receive(current_pid()).unwrap(),
                ResultChannelReadV1::Closed
            ));
        }

        #[test]
        fn sender_credentials_reject_a_different_process_with_the_real_endpoint() {
            let (channel, child) = HostLinkResultChannelV1::new().unwrap();
            rustix::io::fcntl_setfd(&child, rustix::io::FdFlags::empty()).unwrap();
            let script = r#"
import array
import os
import socket
import sys
endpoint = socket.socket(fileno=int(sys.argv[1]))
output = os.memfd_create("fe2o3-fake-result")
os.write(output, b"fake")
endpoint.sendmsg([b"record\n"], [(socket.SOL_SOCKET, socket.SCM_RIGHTS, array.array("i", [output]))])
"#;
            let status = Command::new("python3")
                .arg("-c")
                .arg(script)
                .arg(child.as_raw_fd().to_string())
                .status()
                .unwrap();
            assert!(status.success());
            assert_eq!(
                receive_error_code(&channel),
                HostLinkErrorCodeV1::WorkerIdentity
            );
        }

        #[test]
        fn truncated_oversized_and_noncanonical_packets_are_rejected() {
            for (bytes, expected) in [
                (b"truncated".to_vec(), HostLinkErrorCodeV1::NonCanonicalWire),
                (
                    vec![b'x'; MAX_HOST_LINK_RESULT_RECORD_BYTES_V1 + 1],
                    HostLinkErrorCodeV1::FieldTooLarge,
                ),
                (
                    b"NONCANONICAL\r\n".to_vec(),
                    HostLinkErrorCodeV1::NonCanonicalWire,
                ),
            ] {
                let (channel, child) = HostLinkResultChannelV1::new().unwrap();
                let output = tempfile().unwrap();
                send_packet(&child, &bytes, &[output.as_fd()]);
                assert_eq!(receive_error_code(&channel), expected);
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::*;

    pub(super) enum RawResultReadV1 {
        Pending,
        Closed,
        Packet(Vec<u8>, Vec<File>, Option<u32>),
    }

    fn unsupported<T>() -> Result<T, HostLinkError> {
        Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnsupportedPlatform,
            "host-link result channels require Linux AF_UNIX descriptor passing",
        ))
    }

    pub(super) fn new_channel() -> Result<(HostLinkResultChannelV1, File), HostLinkError> {
        unsupported()
    }

    pub(super) fn validate_endpoint(
        _file: &File,
        _expected: SocketIdentityV1,
    ) -> Result<(), HostLinkError> {
        unsupported()
    }

    pub(super) fn try_receive_raw(_receiver: &File) -> Result<RawResultReadV1, HostLinkError> {
        unsupported()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> Sha256Digest {
        Sha256Digest::from_bytes([byte; 32])
    }

    #[test]
    fn result_record_round_trips_and_rejects_noncanonical_bytes() {
        let record =
            HostLinkResultRecordV1::new(digest(1), digest(2), digest(3), digest(4), 64).unwrap();
        let canonical = record.encode_canonical().unwrap();
        assert_eq!(
            HostLinkResultRecordV1::decode_canonical(&canonical).unwrap(),
            record
        );
        for hostile in [
            canonical[..canonical.len() - 1].to_vec(),
            canonical.iter().copied().chain(*b"\n").collect(),
            canonical.iter().map(u8::to_ascii_uppercase).collect(),
            canonical
                .iter()
                .copied()
                .chain(std::iter::repeat_n(
                    b'x',
                    MAX_HOST_LINK_RESULT_RECORD_BYTES_V1,
                ))
                .collect(),
        ] {
            assert!(HostLinkResultRecordV1::decode_canonical(&hostile).is_err());
        }
    }

    #[test]
    fn output_size_policy_has_an_exact_512_mib_boundary() {
        assert!(
            HostLinkResultRecordV1::new(
                digest(1),
                digest(2),
                digest(3),
                digest(4),
                crate::MAX_HOST_LINK_OUTPUT_BYTES_V1,
            )
            .is_ok()
        );
        assert_eq!(
            HostLinkResultRecordV1::new(
                digest(1),
                digest(2),
                digest(3),
                digest(4),
                crate::MAX_HOST_LINK_OUTPUT_BYTES_V1 + 1,
            )
            .unwrap_err()
            .code(),
            HostLinkErrorCodeV1::ArtifactTooLarge
        );
    }

    #[test]
    fn copy_policy_is_exact_and_sender_mode_is_not_on_the_wire() {
        let canonical = HostLinkResultRecordV1::new(digest(1), digest(2), digest(3), digest(4), 64)
            .unwrap()
            .encode_canonical()
            .unwrap();
        let text = std::str::from_utf8(&canonical).unwrap();
        assert!(text.ends_with("\tcopy=receiver-owned-memfd-v1\n"));
        assert!(!text.contains("mode="));

        for hostile in [
            text.replace(
                "copy=receiver-owned-memfd-v1",
                "copy=receiver-owned-memfd-v2",
            )
            .into_bytes(),
            text.replace("copy=receiver-owned-memfd-v1", "mode=0555")
                .into_bytes(),
        ] {
            assert_eq!(
                HostLinkResultRecordV1::decode_canonical(&hostile)
                    .unwrap_err()
                    .code(),
                HostLinkErrorCodeV1::NonCanonicalWire
            );
        }
    }

    #[test]
    fn parser_never_panics_on_bounded_hostile_bytes() {
        for length in 0..=MAX_HOST_LINK_RESULT_RECORD_BYTES_V1 + 1 {
            let hostile = (0..length)
                .map(|index| (index as u8).wrapping_mul(41).wrapping_add(7))
                .collect::<Vec<_>>();
            assert!(
                std::panic::catch_unwind(|| {
                    let _ = HostLinkResultRecordV1::decode_canonical(&hostile);
                })
                .is_ok()
            );
        }
    }
}
