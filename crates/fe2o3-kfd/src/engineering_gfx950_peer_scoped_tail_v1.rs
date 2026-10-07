//! Closed serial finalnorm/head/argmax and original readbacks. No public scope.
use super::*;
use scoped_currentness::{Currentness, Window};

type Buffer = Gfx950EngineeringPeerBufferV1;
type Kernel = Gfx950EngineeringPeerKernelV1;
type Counts = Gfx950EngineeringPeerScopedCurrentnessCountsV1;

const HIDDEN_BYTES: u64 = 8192;
const HEAD_BYTES: u64 = 1_244_659_712;
const LOGIT_BYTES: u32 = 303_872;
const VOCAB: u32 = 151_936;

/// Fixed semantic roles. Construction is not admission or source authentication.
pub struct Gfx950EngineeringPeerScopedTailInputsV1<'kernel> {
    pub final_norm_kernel: &'kernel Kernel,
    pub head_kernel: &'kernel Kernel,
    pub argmax_kernel: &'kernel Kernel,
    pub hidden: Buffer,
    pub final_norm: Buffer,
    pub empty: Buffer,
    pub normalized: Buffer,
    pub head: Buffer,
    pub logits: Buffer,
    pub choice: Buffer,
}

/// Original copied data after full exit; not argmax or publication authority.
#[derive(Debug)]
pub struct Gfx950EngineeringPeerScopedTailObservationV1 {
    pub choice: [u8; 4],
    pub normalized: Vec<u8>,
    pub logits: Vec<u8>,
    /// Existing serial dispatch host durations, not GPU-only measurements.
    pub host_ns: [u64; 3],
    pub currentness: Counts,
    #[cfg(feature = "engineering-currentness-duration-diagnostics")]
    pub currentness_durations: crate::Gfx950EngineeringCurrentnessDurationsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    FinalNorm,
    Head,
    Argmax,
}
impl Kind {
    const ALL: [Self; 3] = [Self::FinalNorm, Self::Head, Self::Argmax];
    fn symbol(self) -> &'static str {
        match self {
            Self::FinalNorm => "qwen3_rmsnorm_v1",
            Self::Head => "ferric_qwen3_tp_mfma_gemm_bf16_v3",
            Self::Argmax => "ferric_qwen3_tp_batch_argmax_bf16_v2",
        }
    }
    fn explicit_bytes(self) -> u32 {
        match self {
            Self::FinalNorm => 96,
            Self::Head => 72,
            Self::Argmax => 40,
        }
    }
    fn grid(self) -> [u32; 3] {
        [if self == Self::Head { 9496 * 64 } else { 64 }, 1, 1]
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Root {
    Hidden,
    FinalNorm,
    Empty,
    Normalized,
    Head,
    Logits,
    Choice,
}
impl Root {
    const ALL: [Self; 7] = [
        Self::Hidden,
        Self::FinalNorm,
        Self::Empty,
        Self::Normalized,
        Self::Head,
        Self::Logits,
        Self::Choice,
    ];
    fn bytes(self) -> u64 {
        match self {
            Self::Hidden | Self::FinalNorm | Self::Normalized => HIDDEN_BYTES,
            Self::Empty => 2,
            Self::Head => HEAD_BYTES,
            Self::Logits => u64::from(LOGIT_BYTES) * 16,
            Self::Choice => 64,
        }
    }
}
struct Plan {
    bytes: Vec<u8>,
    slices: Vec<(Root, u32, u64, BufferAccessV1)>,
}
fn plan(kind: Kind) -> Plan {
    use BufferAccessV1::{Read, Write};
    use Root::*;
    let (explicit, slices, scalars): (usize, Vec<(Root, u64, u64, BufferAccessV1)>, &[u32]) =
        match kind {
            Kind::FinalNorm => (
                96,
                vec![
                    (Hidden, 4096, 2, Read),
                    (Empty, 0, 2, Read),
                    (FinalNorm, 4096, 2, Read),
                    (Empty, 0, 2, Write),
                    (Normalized, 4096, 2, Write),
                ],
                &[1, 4096, 897_988_541, 0],
            ),
            Kind::Head => (
                72,
                vec![
                    (Normalized, 4096, 2, Read),
                    (Head, 622_329_856, 2, Read),
                    (Logits, u64::from(VOCAB), 2, Write),
                ],
                &[1, VOCAB, 4096, 2, 6],
            ),
            Kind::Argmax => (
                40,
                vec![(Logits, u64::from(VOCAB), 2, Read), (Choice, 1, 4, Write)],
                &[1],
            ),
        };
    let mut bytes = vec![0; explicit + 256];
    for (index, (_, elements, _, _)) in slices.iter().enumerate() {
        bytes[index * 16 + 8..index * 16 + 16].copy_from_slice(&elements.to_le_bytes());
    }
    for (index, word) in scalars.iter().enumerate() {
        let offset = slices.len() * 16 + index * 4;
        bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }
    Plan {
        bytes,
        slices: slices
            .into_iter()
            .enumerate()
            .map(|(index, (root, elements, width, access))| {
                (root, index as u32 * 16, elements * width, access)
            })
            .collect(),
    }
}
impl Gfx950EngineeringPeerScopedTailInputsV1<'_> {
    fn roots(&self) -> [Buffer; 7] {
        [
            self.hidden,
            self.final_norm,
            self.empty,
            self.normalized,
            self.head,
            self.logits,
            self.choice,
        ]
    }
    fn kernel(&self, kind: Kind) -> &Kernel {
        match kind {
            Kind::FinalNorm => self.final_norm_kernel,
            Kind::Head => self.head_kernel,
            Kind::Argmax => self.argmax_kernel,
        }
    }
}

fn validate_roots(incarnation: u64, roots: &[Buffer; 7]) -> Result<()> {
    for (index, role) in Root::ALL.into_iter().enumerate() {
        let token = roots[index];
        if token.group != incarnation
            || token.owner != 0
            || token.bytes != role.bytes()
            || roots[..index].iter().any(|other| other.id == token.id)
        {
            return Err("scoped tail root group, rank, extent or alias".into());
        }
    }
    Ok(())
}
fn validate_kernels(incarnation: u64, facts: [(u64, usize, u64); 3]) -> Result<()> {
    for (index, (group, rank, id)) in facts.into_iter().enumerate() {
        if group != incarnation || rank != 0 || facts[..index].iter().any(|v| v.2 == id) {
            return Err("scoped tail kernel group, rank or alias".into());
        }
    }
    Ok(())
}
fn check_deadline(now: Instant, until: Instant) -> Result<()> {
    if now >= until {
        Err("scoped tail inherited deadline expired".into())
    } else {
        Ok(())
    }
}
fn validate_program(kind: Kind, symbol: &str, bytes: u32) -> Result<()> {
    if symbol != kind.symbol() || bytes != kind.explicit_bytes() + 256 {
        return Err("scoped tail exact kernel role and argument extent".into());
    }
    Ok(())
}
fn validate_timeout(timeout_ms: u32) -> Result<()> {
    if !(1..=10_000).contains(&timeout_ms) {
        return Err("scoped tail unchanged active-forward timeout".into());
    }
    Ok(())
}
fn validate_counts(counts: &Counts) -> Result<()> {
    // Twelve two-rank group checkpoints and fifteen rank-local checkpoints.
    // Periodic polls add only rank-local checkpoints, never full discoveries.
    let before = counts
        .local_checkpoints
        .checked_add(16)
        .ok_or("tail count overflow")?;
    let probes = counts
        .local_checkpoints
        .checked_mul(2)
        .and_then(|n| n.checked_add(3))
        .ok_or("tail probe overflow")?;
    if counts.full_discoveries != 2
        || counts.local_checkpoints < 27
        || counts.before_calls != before
        || counts.after_calls != before
        || counts.generation_probes != probes
    {
        return Err("scoped tail original boundary census".into());
    }
    Ok(())
}
fn observation(
    choice: Vec<u8>,
    normalized: Vec<u8>,
    logits: Vec<u8>,
    host_ns: [u64; 3],
    currentness: Counts,
) -> Result<Gfx950EngineeringPeerScopedTailObservationV1> {
    if normalized.len() != HIDDEN_BYTES as usize || logits.len() != LOGIT_BYTES as usize {
        return Err("scoped tail original readback extent".into());
    }
    validate_counts(&currentness)?;
    Ok(Gfx950EngineeringPeerScopedTailObservationV1 {
        choice: choice.try_into().map_err(|_| "scoped tail choice extent")?,
        normalized,
        logits,
        host_ns,
        currentness,
        #[cfg(feature = "engineering-currentness-duration-diagnostics")]
        currentness_durations: Default::default(),
    })
}

trait ClosedBackend {
    type Prepared;
    type Pending;
    fn enter(&mut self) -> Result<()>;
    fn prepare(&mut self, kind: Kind) -> Result<Self::Prepared>;
    fn publish(&mut self, prepared: Self::Prepared) -> Result<Self::Pending>;
    fn poll(&mut self, pending: &mut Self::Pending) -> Result<Option<u64>>;
    fn pause(&mut self, kind: Kind);
    fn retired(&mut self, kind: Kind) -> Result<()>;
    fn read(&mut self, root: Root) -> Result<Vec<u8>>;
    fn exit(&mut self) -> Result<Counts>;
    fn build(
        &mut self,
        choice: Vec<u8>,
        normalized: Vec<u8>,
        logits: Vec<u8>,
        host_ns: [u64; 3],
        counts: Counts,
    ) -> Result<Gfx950EngineeringPeerScopedTailObservationV1>;
    fn final_deadline(&mut self) -> Result<()>;
    fn quarantine(&mut self);
}
struct TailOperation<'a, B: ClosedBackend> {
    backend: &'a mut B,
    committed: bool,
}
impl<B: ClosedBackend> Drop for TailOperation<'_, B> {
    fn drop(&mut self) {
        if !self.committed {
            self.backend.quarantine();
        }
    }
}
fn closed_tail<B: ClosedBackend>(
    backend: &mut B,
) -> Result<Gfx950EngineeringPeerScopedTailObservationV1> {
    let mut guard = TailOperation {
        backend,
        committed: false,
    };
    guard.backend.enter()?;
    let mut host_ns = [0; 3];
    for (index, kind) in Kind::ALL.into_iter().enumerate() {
        let prepared = guard.backend.prepare(kind)?;
        let mut pending = guard.backend.publish(prepared)?;
        host_ns[index] = loop {
            if let Some(elapsed) = guard.backend.poll(&mut pending)? {
                break elapsed;
            }
            guard.backend.pause(kind);
        };
        guard.backend.retired(kind)?;
    }
    let choice = guard.backend.read(Root::Choice)?;
    let normalized = guard.backend.read(Root::Normalized)?;
    let logits = guard.backend.read(Root::Logits)?;
    let counts = guard.backend.exit()?;
    let output = guard
        .backend
        .build(choice, normalized, logits, host_ns, counts)?;
    guard.backend.final_deadline()?;
    guard.committed = true;
    Ok(output)
}

struct NativeTail<'owner, 'inputs, 'kernel> {
    group: &'owner mut Gfx950EngineeringPeerGroupV1,
    inputs: &'inputs Gfx950EngineeringPeerScopedTailInputsV1<'kernel>,
    timeout_ms: u32,
    until: Instant,
    window: Option<Window>,
}
impl ClosedBackend for NativeTail<'_, '_, '_> {
    type Prepared = PreparedDispatch;
    type Pending = PendingDispatch;
    fn enter(&mut self) -> Result<()> {
        check_deadline(Instant::now(), self.until)?;
        validate_timeout(self.timeout_ms)?;
        self.group.require_active()?;
        let roots = self.inputs.roots();
        validate_roots(self.group.incarnation, &roots)?;
        validate_kernels(
            self.group.incarnation,
            Kind::ALL.map(|kind| {
                let kernel = self.inputs.kernel(kind);
                (kernel.group, kernel.rank, kernel.id)
            }),
        )?;
        for kind in Kind::ALL {
            let metadata = &self.inputs.kernel(kind).metadata;
            validate_program(kind, &metadata.symbol, metadata.kernarg_bytes)?;
        }
        for token in roots {
            require_public_vram(self.group.validate_token(token)?)?;
        }
        self.window = Some(Window::enter(self.group, self.until)?);
        Ok(())
    }
    fn prepare(&mut self, kind: Kind) -> Result<PreparedDispatch> {
        let plan = plan(kind);
        let roots = self.inputs.roots();
        let pointers = plan
            .slices
            .into_iter()
            .map(|(root, offset, extent, access)| {
                roots[root as usize].pointer(offset, 0, extent, access)
            })
            .collect::<Vec<_>>();
        let mut route = Currentness::Scoped(self.window.as_mut().ok_or("missing tail window")?);
        route.idle_group(self.group)?;
        self.group.prepare_peer_dispatch_currentness(
            self.inputs.kernel(kind),
            plan.bytes,
            [64, 1, 1],
            kind.grid(),
            &pointers,
            self.timeout_ms,
            &mut route,
        )
    }
    fn publish(&mut self, prepared: PreparedDispatch) -> Result<PendingDispatch> {
        let mut route = Currentness::Scoped(self.window.as_mut().ok_or("missing tail window")?);
        let mut rank = route.rank(self.group, 0)?;
        // SAFETY: exact fixed plans and authenticated caller roles; Group and
        // the private signal/kernarg remain exclusively retained through polling.
        unsafe {
            self.group.contexts[0].publish_prepared_dispatch_with_currentness(
                prepared,
                self.timeout_ms,
                false,
                &mut rank,
            )
        }
    }
    fn poll(&mut self, pending: &mut PendingDispatch) -> Result<Option<u64>> {
        check_deadline(Instant::now(), self.until)?;
        let mut route = Currentness::Scoped(self.window.as_mut().ok_or("missing tail window")?);
        let mut rank = route.rank(self.group, 0)?;
        self.group.contexts[0].poll_pending_dispatch_with_currentness(pending, &mut rank)
    }
    fn pause(&mut self, _kind: Kind) {
        std::thread::sleep(Duration::from_micros(50));
    }
    fn retired(&mut self, _kind: Kind) -> Result<()> {
        Currentness::Scoped(self.window.as_mut().ok_or("missing tail window")?)
            .idle_group(self.group)
    }
    fn read(&mut self, root: Root) -> Result<Vec<u8>> {
        let bytes = match root {
            Root::Choice => 4,
            Root::Normalized => HIDDEN_BYTES as u32,
            Root::Logits => LOGIT_BYTES,
            _ => return Err("scoped tail closed read roster".into()),
        };
        let mut route = Currentness::Scoped(self.window.as_mut().ok_or("missing tail window")?);
        self.group
            .read_currentness(self.inputs.roots()[root as usize], 0, bytes, &mut route)
    }
    fn exit(&mut self) -> Result<Counts> {
        Ok(self
            .window
            .as_mut()
            .ok_or("missing tail window")?
            .finish(self.group)?
            .into())
    }
    fn build(
        &mut self,
        choice: Vec<u8>,
        normalized: Vec<u8>,
        logits: Vec<u8>,
        host_ns: [u64; 3],
        counts: Counts,
    ) -> Result<Gfx950EngineeringPeerScopedTailObservationV1> {
        let result = observation(choice, normalized, logits, host_ns, counts)?;
        #[cfg(feature = "engineering-currentness-duration-diagnostics")]
        let result = {
            let mut result = result;
            result.currentness_durations = self
                .window
                .as_ref()
                .ok_or("tail duration window absent")?
                .durations()?;
            result
        };
        Ok(result)
    }
    fn final_deadline(&mut self) -> Result<()> {
        check_deadline(Instant::now(), self.until)
    }
    fn quarantine(&mut self) {
        self.group.poisoned = true;
        for context in &mut self.group.contexts {
            context.ordered_batch_poisoned = true;
            crate::device::ScopedCurrentnessV1::poison_devices(&mut [context
                .backend
                .engineering_peer_device()]);
        }
    }
}
impl Gfx950EngineeringPeerGroupV1 {
    /// Fixed serial tail and three readbacks under one closed currentness scope.
    /// Old dispatch/read methods and every Group default remain unchanged.
    ///
    /// # Safety
    /// The caller authenticates all three exact machine-code roles, sealed
    /// bindings and transposed head, and retains exclusive same-forward custody
    /// after all36 layer completions. Hidden is that forward's final residual.
    /// No external work, owner rearm, mutation, or release may interleave. The
    /// supplied absolute deadline is the existing owning worker deadline, never
    /// a renewed budget. Per-dispatch timeouts and source/range obligations
    /// remain unchanged. All failures are terminal in a disposable process.
    /// Returned bytes require the original worker finite/argmax and publication
    /// checks; success is not numerical or Full-workload authority.
    pub unsafe fn dispatch_tail_scoped_currentness_unchecked_v1(
        &mut self,
        inputs: &Gfx950EngineeringPeerScopedTailInputsV1<'_>,
        timeout_ms: u32,
        deadline: Instant,
    ) -> Result<Gfx950EngineeringPeerScopedTailObservationV1> {
        closed_tail(&mut NativeTail {
            group: self,
            inputs,
            timeout_ms,
            until: deadline,
            window: None,
        })
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_scoped_tail_v1_tests.rs"]
mod tests;
