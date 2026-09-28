//! Private bounded transport for a separately supervised generated-proof executor.
//!
//! Neither a socket, session identifier nor a valid reply authenticates execution.
//! In particular, `InertExecutorReplyV1` cannot become a retained-runtime output.
//! Controlled endpoint bootstrap and lifecycle admission are separate requirements.
//! Absolute CLOCK_MONOTONIC deadlines require a bootstrap-admitted common time
//! namespace; this transport does not establish namespace or process origin.

use super::{
    RetainedFunctionalRefinementRuntimeErrorV1, RetainedFunctionalRefinementRuntimeOutputV1,
    RetainedGeneratedVerusRuntimeBackendV1,
};
use crate::{
    CanonicalGeneratedVerusProofInputV3 as Source, MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3,
};
use rustix::time::{ClockId, clock_gettime};
use sha2::{Digest, Sha256};
use std::{
    io,
    time::{Duration, Instant},
};

#[path = "functional_refinement_executor_socket_v1.rs"]
mod socket;
use socket::{CredentialSocketV1 as Socket, PACKET_BYTES};

const HEADER_BYTES: usize = 192;
const MAGIC: &[u8; 16] = b"FE2O3-PROOF-RPC1";
const DOMAIN: &[u8] = b"FE2O3/PROOF-EXECUTOR/FRAME/V1\0";
const REQUEST: u32 = 1;
const REPLY: u32 = 2;
const FINISH: u32 = 3;
const FINISHED: u32 = 4;
const MAX_OUTPUT: usize = 64 * 1024;
const MAX_REQUESTS: u64 = 1_024;
const MAX_TOTAL_SOURCE: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_OUTPUT: u64 = 8 * 1024 * 1024;
const MAX_SESSION: Duration = Duration::from_secs(3_600);
const MAX_REQUEST: Duration = Duration::from_secs(600);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SessionBindingV1 {
    pub(super) session: [u8; 32],
    pub(super) runtime: [u8; 32],
}

/// Validated transport data only, never an observed execution or proof receipt.
pub(super) struct InertExecutorReplyV1 {
    pub(super) exit_code: Option<i32>,
    pub(super) signal: Option<i32>,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

#[derive(Debug)]
pub(super) enum ChannelErrorV1 {
    Framing,
    Association,
    Limit,
    Deadline,
    Terminal,
    Io(io::Error),
    Runtime(RetainedFunctionalRefinementRuntimeErrorV1),
}
type Result<T> = std::result::Result<T, ChannelErrorV1>;

/// Bounds describe transport, not original-account credit or endpoint provenance.
pub(super) struct ExecutorChannelV1 {
    stream: Socket,
    binding: SessionBindingV1,
    deadline: u64,
    sequence: u64,
    source_bytes: u64,
    output_bytes: u64,
    terminal: bool,
}

struct Frame {
    header: [u8; HEADER_BYTES],
    body: Vec<u8>,
}

enum ExpectedFrame<'a> {
    RequestOrFinish,
    ReplyTo(&'a [u8; HEADER_BYTES]),
    Finished(&'a [u8; HEADER_BYTES]),
}

impl ExecutorChannelV1 {
    /// Constructs transport only. No production authority path accepts this owner.
    pub(super) fn new(stream: Socket, binding: SessionBindingV1, deadline: u64) -> Result<Self> {
        let remaining = remaining(deadline)?;
        if remaining > MAX_SESSION || binding.session == [0; 32] || binding.runtime == [0; 32] {
            return Err(ChannelErrorV1::Limit);
        }
        Ok(Self {
            stream,
            binding,
            deadline,
            sequence: 0,
            source_bytes: 0,
            output_bytes: 0,
            terminal: false,
        })
    }

    pub(super) fn exchange(
        &mut self,
        source: &Source,
        deadline: Instant,
        output_limit: usize,
    ) -> Result<InertExecutorReplyV1> {
        self.begin()?;
        let deadline = absolute_deadline(deadline)?.min(self.deadline);
        if remaining(deadline)? > MAX_REQUEST || !(1..=MAX_OUTPUT).contains(&output_limit) {
            return Err(ChannelErrorV1::Limit);
        }
        self.charge_source(source.byte_len())?;
        let mut header = self.header(REQUEST, deadline);
        header[96..128].copy_from_slice(&source.identity().as_bytes());
        put32(&mut header, 136, source.byte_len() as u32);
        put32(&mut header, 140, output_limit as u32);
        seal(&mut header, source.source());
        write_all(&mut self.stream, &header, deadline)?;
        write_all(&mut self.stream, source.source(), deadline)?;
        let response = self.receive(deadline, ExpectedFrame::ReplyTo(&header))?;
        self.charge_output(response.body.len() as u64)?;
        let stdout_len = field32(&response.header, 144) as usize;
        let (exit_code, signal) = termination(&response.header)?;
        let mut stdout = response.body;
        let stderr = stdout.split_off(stdout_len);
        remaining(deadline)?;
        self.terminal = false;
        Ok(InertExecutorReplyV1 {
            exit_code,
            signal,
            stdout,
            stderr,
        })
    }

    /// A close acknowledgement is transport evidence only, not guard completion.
    pub(super) fn finish(&mut self) -> Result<()> {
        self.begin()?;
        let mut header = self.header(FINISH, self.deadline);
        seal(&mut header, &[]);
        write_all(&mut self.stream, &header, self.deadline)?;
        self.receive(self.deadline, ExpectedFrame::Finished(&header))?;
        // Do not accept an acknowledgement followed by another frame or a live
        // helper that never closes its stream. The outer owner still must reap it.
        let mut extra = [0];
        if read(&mut self.stream, &mut extra, self.deadline)? != 0 {
            return Err(ChannelErrorV1::Framing);
        }
        remaining(self.deadline)?;
        Ok(())
    }

    /// Runs only the existing direct retained backend, with its process affinity
    /// and pre/post execution validation intact. No signing occurs in the helper.
    pub(super) fn serve(mut self, runtime: &RetainedGeneratedVerusRuntimeBackendV1) -> Result<()> {
        if runtime.identity() != self.binding.runtime {
            return Err(ChannelErrorV1::Association);
        }
        runtime.revalidate().map_err(ChannelErrorV1::Runtime)?;
        loop {
            self.begin()?;
            let request = self.receive(self.deadline, ExpectedFrame::RequestOrFinish)?;
            let deadline = field64(&request.header, 128);
            if request.kind() == FINISH {
                if deadline != self.deadline {
                    return Err(ChannelErrorV1::Association);
                }
                runtime.revalidate().map_err(ChannelErrorV1::Runtime)?;
                let mut header = self.header(FINISHED, deadline);
                seal(&mut header, &[]);
                write_all(&mut self.stream, &header, deadline)?;
                return Ok(());
            }
            self.charge_source(request.body.len() as u64)?;
            let source = Source::new(request.body).map_err(|_| ChannelErrorV1::Framing)?;
            if source.identity().as_bytes() != request.header[96..128] {
                return Err(ChannelErrorV1::Association);
            }
            let limit = field32(&request.header, 140) as usize;
            let output = runtime
                .execute_generated_rust_verify(&source, local_deadline(deadline)?, limit)
                .map_err(ChannelErrorV1::Runtime)?;
            self.send_output(request.header, output)?;
            self.terminal = false;
        }
    }

    fn begin(&mut self) -> Result<()> {
        if self.terminal {
            return Err(ChannelErrorV1::Terminal);
        }
        self.terminal = true;
        remaining(self.deadline)?;
        self.sequence = self.sequence.checked_add(1).ok_or(ChannelErrorV1::Limit)?;
        if self.sequence > MAX_REQUESTS + 1 {
            return Err(ChannelErrorV1::Limit);
        }
        Ok(())
    }

    fn charge_source(&mut self, bytes: u64) -> Result<()> {
        if self.sequence > MAX_REQUESTS {
            return Err(ChannelErrorV1::Limit);
        }
        self.source_bytes = self
            .source_bytes
            .checked_add(bytes)
            .ok_or(ChannelErrorV1::Limit)?;
        if self.source_bytes > MAX_TOTAL_SOURCE {
            return Err(ChannelErrorV1::Limit);
        }
        Ok(())
    }

    fn charge_output(&mut self, bytes: u64) -> Result<()> {
        self.output_bytes = self
            .output_bytes
            .checked_add(bytes)
            .ok_or(ChannelErrorV1::Limit)?;
        if self.output_bytes > MAX_TOTAL_OUTPUT {
            return Err(ChannelErrorV1::Limit);
        }
        Ok(())
    }

    fn header(&self, kind: u32, deadline: u64) -> [u8; HEADER_BYTES] {
        let mut header = [0; HEADER_BYTES];
        header[..16].copy_from_slice(MAGIC);
        put32(&mut header, 16, kind);
        header[24..32].copy_from_slice(&self.sequence.to_le_bytes());
        header[32..64].copy_from_slice(&self.binding.session);
        header[64..96].copy_from_slice(&self.binding.runtime);
        header[128..136].copy_from_slice(&deadline.to_le_bytes());
        header
    }

    fn receive(&mut self, deadline: u64, expected: ExpectedFrame<'_>) -> Result<Frame> {
        let mut header = [0; HEADER_BYTES];
        read_exact(&mut self.stream, &mut header, deadline)?;
        if header[..16] != MAGIC[..] || header[20..24] != [0; 4] {
            return Err(ChannelErrorV1::Framing);
        }
        if field64(&header, 24) != self.sequence
            || header[32..64] != self.binding.session
            || header[64..96] != self.binding.runtime
            || field64(&header, 128) > deadline
        {
            return Err(ChannelErrorV1::Association);
        }
        let frame_deadline = field64(&header, 128);
        let duration = remaining(frame_deadline)?;
        // Reject a wrong operation, echo or time bound before allocating or
        // waiting for its body. Session bounds alone are not request bounds.
        match expected {
            ExpectedFrame::RequestOrFinish => match field32(&header, 16) {
                REQUEST if duration <= MAX_REQUEST => {}
                REQUEST => return Err(ChannelErrorV1::Limit),
                FINISH if frame_deadline == self.deadline => {}
                FINISH => return Err(ChannelErrorV1::Association),
                _ => return Err(ChannelErrorV1::Framing),
            },
            ExpectedFrame::ReplyTo(sent)
                if field32(&header, 16) == REPLY && header[24..144] == sent[24..144] => {}
            ExpectedFrame::Finished(sent)
                if field32(&header, 16) == FINISHED && header[24..160] == sent[24..160] => {}
            _ => return Err(ChannelErrorV1::Association),
        }
        let body_len = body_length(&header)?;
        match field32(&header, 16) {
            REQUEST
                if self.sequence > MAX_REQUESTS
                    || self.source_bytes + body_len as u64 > MAX_TOTAL_SOURCE =>
            {
                return Err(ChannelErrorV1::Limit);
            }
            REPLY if self.output_bytes + body_len as u64 > MAX_TOTAL_OUTPUT => {
                return Err(ChannelErrorV1::Limit);
            }
            _ => {}
        }
        let mut body = Vec::new();
        body.try_reserve_exact(body_len)
            .map_err(|_| ChannelErrorV1::Limit)?;
        body.resize(body_len, 0);
        read_exact(&mut self.stream, &mut body, frame_deadline)?;
        if digest(&header, &body) != header[160..192] {
            return Err(ChannelErrorV1::Framing);
        }
        Ok(Frame { header, body })
    }

    fn send_output(
        &mut self,
        mut header: [u8; HEADER_BYTES],
        output: RetainedFunctionalRefinementRuntimeOutputV1,
    ) -> Result<()> {
        let limit = field32(&header, 140) as usize;
        if output.stdout.len() > limit || output.stderr.len() > limit {
            return Err(ChannelErrorV1::Limit);
        }
        self.charge_output((output.stdout.len() + output.stderr.len()) as u64)?;
        put32(&mut header, 16, REPLY);
        put32(&mut header, 144, output.stdout.len() as u32);
        put32(&mut header, 148, output.stderr.len() as u32);
        let (code, signal) = match (output.exit_code, output.signal) {
            (Some(code @ 0..=255), None) => (code as u32 + 1, 0),
            (None, Some(signal @ 1..=64)) => (0, signal as u32),
            _ => return Err(ChannelErrorV1::Framing),
        };
        put32(&mut header, 152, code);
        put32(&mut header, 156, signal);
        termination(&header)?;
        let deadline = field64(&header, 128);
        let mut body = output.stdout;
        body.try_reserve_exact(output.stderr.len())
            .map_err(|_| ChannelErrorV1::Limit)?;
        body.extend(output.stderr);
        seal(&mut header, &body);
        write_all(&mut self.stream, &header, deadline)?;
        write_all(&mut self.stream, &body, deadline)
    }
}

impl Frame {
    fn kind(&self) -> u32 {
        field32(&self.header, 16)
    }
}

fn body_length(header: &[u8; HEADER_BYTES]) -> Result<usize> {
    let source = field32(header, 136) as usize;
    let limit = field32(header, 140) as usize;
    let stdout = field32(header, 144) as usize;
    let stderr = field32(header, 148) as usize;
    match field32(header, 16) {
        REQUEST | REPLY
            if (1..=MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3).contains(&source)
                && (1..=MAX_OUTPUT).contains(&limit)
                && header[96..128] != [0; 32] =>
        {
            if field32(header, 16) == REQUEST {
                if header[144..160] != [0; 16] {
                    return Err(ChannelErrorV1::Framing);
                }
                Ok(source)
            } else {
                termination(header)?;
                if stdout > limit || stderr > limit {
                    return Err(ChannelErrorV1::Limit);
                }
                Ok(stdout + stderr)
            }
        }
        FINISH | FINISHED if header[96..128] == [0; 32] && header[136..160] == [0; 24] => Ok(0),
        _ => Err(ChannelErrorV1::Framing),
    }
}

fn termination(header: &[u8; HEADER_BYTES]) -> Result<(Option<i32>, Option<i32>)> {
    match (field32(header, 152), field32(header, 156)) {
        (code @ 1..=256, 0) => Ok((Some(code as i32 - 1), None)),
        (0, signal @ 1..=64) => Ok((None, Some(signal as i32))),
        _ => Err(ChannelErrorV1::Framing),
    }
}

fn field32(bytes: &[u8; HEADER_BYTES], start: usize) -> u32 {
    u32::from_le_bytes(
        bytes[start..start + 4]
            .try_into()
            .expect("fixed frame field"),
    )
}

fn field64(bytes: &[u8; HEADER_BYTES], start: usize) -> u64 {
    u64::from_le_bytes(
        bytes[start..start + 8]
            .try_into()
            .expect("fixed frame field"),
    )
}

fn put32(bytes: &mut [u8; HEADER_BYTES], start: usize, value: u32) {
    bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
}

fn digest(header: &[u8; HEADER_BYTES], body: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update(&header[..160]);
    digest.update(body);
    digest.finalize().into()
}

fn seal(header: &mut [u8; HEADER_BYTES], body: &[u8]) {
    let digest = digest(header, body);
    header[160..192].copy_from_slice(&digest);
}

fn monotonic_ns() -> Result<u64> {
    let now = clock_gettime(ClockId::Monotonic);
    u64::try_from(now.tv_sec)
        .ok()
        .and_then(|seconds| seconds.checked_mul(1_000_000_000))
        .and_then(|seconds| seconds.checked_add(u64::try_from(now.tv_nsec).ok()?))
        .ok_or(ChannelErrorV1::Deadline)
}

fn remaining(deadline: u64) -> Result<Duration> {
    let nanos = deadline
        .checked_sub(monotonic_ns()?)
        .filter(|value| *value != 0)
        .ok_or(ChannelErrorV1::Deadline)?;
    Ok(Duration::from_nanos(nanos))
}

/// Sample in conservative order: serialization may shorten, never restart, a deadline.
pub(super) fn absolute_deadline(deadline: Instant) -> Result<u64> {
    let monotonic = monotonic_ns()?;
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or(ChannelErrorV1::Deadline)?;
    monotonic
        .checked_add(u64::try_from(remaining.as_nanos()).map_err(|_| ChannelErrorV1::Deadline)?)
        .ok_or(ChannelErrorV1::Deadline)
}

fn local_deadline(deadline: u64) -> Result<Instant> {
    let now = Instant::now();
    now.checked_add(remaining(deadline)?)
        .ok_or(ChannelErrorV1::Deadline)
}

fn read(stream: &mut Socket, bytes: &mut [u8], deadline: u64) -> Result<usize> {
    let count = stream
        .receive_packet(bytes, local_deadline(deadline)?)
        .map_err(ChannelErrorV1::Io)?;
    remaining(deadline)?;
    match count {
        None => Ok(0),
        Some(0) => Err(ChannelErrorV1::Framing),
        Some(count) => Ok(count),
    }
}

fn read_exact(stream: &mut Socket, bytes: &mut [u8], deadline: u64) -> Result<()> {
    for chunk in bytes.chunks_mut(PACKET_BYTES) {
        if read(stream, chunk, deadline)? != chunk.len() {
            return Err(ChannelErrorV1::Framing);
        }
    }
    remaining(deadline)?;
    Ok(())
}

fn write_all(stream: &mut Socket, bytes: &[u8], deadline: u64) -> Result<()> {
    for chunk in bytes.chunks(PACKET_BYTES) {
        stream
            .send_packet(chunk, local_deadline(deadline)?)
            .map_err(ChannelErrorV1::Io)?;
    }
    remaining(deadline)?;
    Ok(())
}

#[cfg(test)]
#[path = "functional_refinement_executor_channel_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "functional_refinement_executor_channel_v1_protected_tests.rs"]
mod protected_tests;
