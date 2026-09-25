use super::*;

#[derive(Debug)]
enum Action {
    Register(Key),
    Begin(Reference, Vec<Write>),
    Abort(Reference),
    Success(Reference, Reference),
    NoEffect(Reference, Reference),
    Unknown(Reference),
}

struct OracleMember {
    slot: usize,
    allocation: ContextAllocationReferenceV1,
    epoch: u64,
    lineage: u64,
}

enum Phase {
    Reserved,
    Pending(Vec<OracleMember>),
    Unknown(Vec<OracleMember>),
}

struct OracleWriter {
    reference: Reference,
    phase: Phase,
}

struct OracleAllocation {
    descriptor: Write,
    epoch: u64,
    lineage: u64,
    owner: Option<Reference>,
}

// Expected transitions use input descriptors and maps, never the journal chain.
struct Oracle {
    allocations: BTreeMap<ContextAllocationKeyV1, OracleAllocation>,
    writers: BTreeMap<u64, OracleWriter>,
    watermark: u64,
    writer_capacity: usize,
    writer_free: Vec<usize>,
    member_free: Vec<usize>,
}

impl Oracle {
    fn new(roster: &[Write], writer_capacity: usize) -> Self {
        Self {
            allocations: roster
                .iter()
                .map(|descriptor| {
                    (
                        descriptor.allocation.key,
                        OracleAllocation {
                            descriptor: *descriptor,
                            epoch: 0,
                            lineage: 0,
                            owner: None,
                        },
                    )
                })
                .collect(),
            writers: BTreeMap::new(),
            watermark: 0,
            writer_capacity,
            writer_free: (0..writer_capacity).rev().collect(),
            member_free: (0..roster.len()).rev().collect(),
        }
    }

    fn exact_writer(&self, reference: Reference) -> Result<&OracleWriter, Error> {
        self.writers
            .get(&reference.key.local)
            .filter(|writer| writer.reference == reference)
            .ok_or(Error::InvalidReference)
    }

    fn apply(&mut self, action: &Action) -> Result<Option<Reference>, Error> {
        match action {
            Action::Register(key) => {
                if key.context_generation != 7 {
                    return Err(Error::ForeignContext);
                }
                if key.local == 0 || key.local == u64::MAX {
                    return Err(Error::InvalidWriterId);
                }
                if key.local <= self.watermark {
                    return Err(Error::WriterReplay);
                }
                if self.writers.len() == self.writer_capacity {
                    return Err(Error::WriterCapacity);
                }
                let reference = Reference {
                    slot: self.writer_free.pop().unwrap(),
                    key: *key,
                };
                self.writers.insert(
                    key.local,
                    OracleWriter {
                        reference,
                        phase: Phase::Reserved,
                    },
                );
                self.watermark = key.local;
                return Ok(Some(reference));
            }
            Action::Begin(reference, roster) => {
                if !matches!(self.exact_writer(*reference)?.phase, Phase::Reserved) {
                    return Err(Error::InvalidReference);
                }
                if roster.len() > self.allocations.len() {
                    return Err(Error::RosterCapacity);
                }
                if !roster
                    .windows(2)
                    .all(|pair| pair[0].allocation.key < pair[1].allocation.key)
                {
                    return Err(Error::NonCanonicalRoster);
                }
                for descriptor in roster {
                    let allocation = self
                        .allocations
                        .get(&descriptor.allocation.key)
                        .filter(|value| value.descriptor.allocation == descriptor.allocation)
                        .ok_or(Error::InvalidAllocationReference)?;
                    if allocation.descriptor.device != descriptor.device {
                        return Err(Error::AllocationDeviceMismatch);
                    }
                    if allocation.descriptor.byte_extent != descriptor.byte_extent {
                        return Err(Error::AllocationExtentMismatch);
                    }
                    if allocation.owner.is_some() {
                        return Err(Error::AllocationBusy);
                    }
                    if allocation.epoch == u64::MAX {
                        return Err(Error::EpochExhausted);
                    }
                }
                if roster.len() > self.member_free.len() {
                    return Err(Error::MemberCapacity);
                }
                let mut members = Vec::new();
                for descriptor in roster {
                    let allocation = self
                        .allocations
                        .get_mut(&descriptor.allocation.key)
                        .unwrap();
                    allocation.epoch += 1;
                    allocation.owner = Some(*reference);
                    members.push(OracleMember {
                        slot: self.member_free.pop().unwrap(),
                        allocation: descriptor.allocation,
                        epoch: allocation.epoch,
                        lineage: allocation.lineage,
                    });
                }
                self.writers.get_mut(&reference.key.local).unwrap().phase = Phase::Pending(members);
            }
            Action::Abort(reference) => {
                if !matches!(self.exact_writer(*reference)?.phase, Phase::Reserved) {
                    return Err(Error::InvalidReference);
                }
                self.writers.remove(&reference.key.local);
                self.writer_free.push(reference.slot);
            }
            Action::Success(reference, evidence) | Action::NoEffect(reference, evidence) => {
                if !matches!(self.exact_writer(*reference)?.phase, Phase::Pending(_)) {
                    return Err(Error::InvalidReference);
                }
                if reference != evidence {
                    return Err(Error::SettlementEvidenceMismatch);
                }
                let writer = self.writers.remove(&reference.key.local).unwrap();
                let Phase::Pending(members) = writer.phase else {
                    unreachable!()
                };
                for member in members {
                    let allocation = self.allocations.get_mut(&member.allocation.key).unwrap();
                    if matches!(action, Action::Success(..)) {
                        allocation.lineage = member.epoch;
                    }
                    allocation.owner = None;
                    self.member_free.push(member.slot);
                }
                self.writer_free.push(reference.slot);
            }
            Action::Unknown(reference) => {
                if matches!(self.exact_writer(*reference)?.phase, Phase::Reserved) {
                    return Err(Error::InvalidReference);
                }
                let writer = self.writers.get_mut(&reference.key.local).unwrap();
                if let Phase::Pending(members) = &mut writer.phase {
                    writer.phase = Phase::Unknown(core::mem::take(members));
                }
            }
        }
        Ok(None)
    }

    fn writer_state(&self, reference: Reference) -> Result<ContextWriterStateV1, Error> {
        Ok(match &self.exact_writer(reference)?.phase {
            Phase::Reserved => ContextWriterStateV1::Reserved,
            Phase::Pending(members) => ContextWriterStateV1::Pending {
                member_count: members.len(),
            },
            Phase::Unknown(members) => ContextWriterStateV1::Unknown {
                member_count: members.len(),
            },
        })
    }

    fn snapshot(&self, storage: [(usize, usize); 7]) -> Snapshot {
        let capacity = self.allocations.len();
        let mut expected = Snapshot {
            context: 7,
            capacities: (capacity, self.writer_capacity),
            watermark: self.watermark,
            reserved_count: 0,
            writers: vec![None; self.writer_capacity],
            free: self.writer_free.clone(),
            allocations: vec![None; capacity],
            allocation_free: Vec::new(),
            members: vec![None; capacity],
            member_free: self.member_free.clone(),
            scratch: vec![None; capacity],
            storage,
        };
        for value in self.allocations.values() {
            let descriptor = value.descriptor;
            expected.allocations[descriptor.allocation.slot] = Some(AllocationEntryV1 {
                key: descriptor.allocation.key,
                device: descriptor.device,
                byte_extent: descriptor.byte_extent,
                attempt_epoch: value.epoch,
                content_lineage: value.lineage,
                pending_member: None,
            });
        }
        for writer in self.writers.values() {
            let members = match &writer.phase {
                Phase::Reserved => {
                    expected.reserved_count += 1;
                    expected.writers[writer.reference.slot] =
                        Some(WriterEntryV1::Reserved(writer.reference.key));
                    continue;
                }
                Phase::Pending(members) | Phase::Unknown(members) => members,
            };
            let head = members.first().map(|member| member.slot);
            let key = writer.reference.key;
            let count = members.len();
            expected.writers[writer.reference.slot] = Some(match &writer.phase {
                Phase::Pending(_) => WriterEntryV1::Pending { key, head, count },
                Phase::Unknown(_) => WriterEntryV1::Unknown { key, head, count },
                Phase::Reserved => unreachable!(),
            });
            for (index, member) in members.iter().enumerate() {
                expected.members[member.slot] = Some(MemberEntryV1 {
                    writer: writer.reference,
                    allocation: member.allocation,
                    prior_lineage: member.lineage,
                    attempt_epoch: member.epoch,
                    next: members.get(index + 1).map(|next| next.slot),
                });
                expected.allocations[member.allocation.slot]
                    .as_mut()
                    .unwrap()
                    .pending_member = Some(member.slot);
            }
        }
        expected
    }
}

struct Trace {
    journal: Journal,
    oracle: Oracle,
    roster: Vec<Write>,
    history: Vec<Reference>,
    storage: [(usize, usize); 7],
    coverage: [usize; 12],
}

impl Trace {
    fn new(allocations: usize, writers: usize) -> Self {
        let mut journal = Journal::new(7, allocations, writers).unwrap();
        let device = ContextJournalDeviceKeyV1 {
            context_generation: 7,
            local: 11,
        };
        let mut roster = Vec::new();
        for index in (0..allocations).rev() {
            let byte_extent = 64 + index as u64;
            let allocation = journal
                .enroll_allocation(
                    ContextAllocationKeyV1 {
                        context_generation: 7,
                        local: 100 + 10 * index as u64,
                    },
                    device,
                    byte_extent,
                )
                .unwrap();
            roster.push(Write {
                allocation,
                device,
                byte_extent,
            });
        }
        roster.reverse();
        let storage = storage(&journal);
        let oracle = Oracle::new(&roster, writers);
        assert_eq!(snapshot(&journal), oracle.snapshot(storage));
        Self {
            journal,
            oracle,
            roster,
            history: Vec::new(),
            storage,
            coverage: [0; 12],
        }
    }

    fn step(&mut self, action: Action) -> Option<Reference> {
        let before = snapshot(&self.journal);
        let repeated_unknown = match action {
            Action::Unknown(reference) => matches!(
                self.oracle.writer_state(reference),
                Ok(ContextWriterStateV1::Unknown { .. })
            ),
            _ => false,
        };
        let expected = self.oracle.apply(&action);
        let actual = match &action {
            Action::Register(key) => self.journal.register_writer(*key).map(Some),
            Action::Begin(reference, roster) => {
                self.journal.begin_write(*reference, roster).map(|()| None)
            }
            Action::Abort(reference) => self.journal.abort_reserved(*reference).map(|()| None),
            Action::Success(reference, evidence) => {
                settle(&mut self.journal, *reference, *evidence, true).map(|()| None)
            }
            Action::NoEffect(reference, evidence) => {
                settle(&mut self.journal, *reference, *evidence, false).map(|()| None)
            }
            Action::Unknown(reference) => self.journal.mark_unknown(*reference).map(|()| None),
        };
        assert_eq!(actual, expected, "independent transition: {action:?}");
        if expected.is_err() {
            self.coverage[8] += 1;
            if expected == Err(Error::WriterCapacity) {
                self.coverage[9] += 1;
            }
            if expected == Err(Error::AllocationBusy) {
                self.coverage[10] += 1;
            }
            assert_eq!(
                snapshot(&self.journal),
                before,
                "rejected transition: {action:?}"
            );
        } else {
            let bin = match &action {
                Action::Register(key) if key.kind == Kind::Synchronous => 0,
                Action::Register(_) => 1,
                Action::Begin(_, roster) => {
                    if roster.is_empty() {
                        self.coverage[11] += 1;
                    }
                    2
                }
                Action::Success(..) => 3,
                Action::NoEffect(..) => 4,
                Action::Unknown(_) if repeated_unknown => 6,
                Action::Unknown(_) => 5,
                Action::Abort(_) => 7,
            };
            self.coverage[bin] += 1;
        }
        if let Ok(Some(reference)) = expected {
            self.history.push(reference);
        }
        assert_eq!(
            snapshot(&self.journal),
            self.oracle.snapshot(self.storage),
            "independent retained state: {action:?}"
        );
        for reference in &self.history {
            assert_eq!(
                self.journal.lookup_writer(*reference),
                self.oracle.writer_state(*reference)
            );
        }
        for allocation in self.oracle.allocations.values() {
            assert_eq!(
                self.journal
                    .lookup_allocation(allocation.descriptor.allocation),
                Ok(ContextAllocationStateV1 {
                    device: allocation.descriptor.device,
                    byte_extent: allocation.descriptor.byte_extent,
                    attempt_epoch: allocation.epoch,
                    content_lineage: allocation.lineage,
                    pending_writer: allocation.owner,
                })
            );
        }
        audit(&self.journal);
        expected.ok().flatten()
    }

    fn register(&mut self, local: u64, kind: Kind) -> Reference {
        self.step(Action::Register(Key { kind, ..key(local) }))
            .unwrap()
    }
}

#[test]
fn independent_traces_preserve_disjoint_writers_and_lineage_below_watermark() {
    for count in [0, 1, 3] {
        let mut trace = Trace::new(16, 8);
        let a = trace.register(41, Kind::Synchronous);
        let b = trace.register(44, Kind::Submission);
        let c = trace.register(47, Kind::Synchronous);
        let d = trace.register(50, Kind::Submission);
        let x = trace.roster[..count].to_vec();
        trace.step(Action::Begin(b, x.clone()));
        trace.step(Action::Begin(c, trace.roster[count..2 * count].to_vec()));
        trace.step(Action::Begin(
            d,
            trace.roster[2 * count..3 * count].to_vec(),
        ));
        trace.step(Action::Unknown(d));
        trace.step(Action::Success(b, b));
        trace.step(Action::Begin(a, x.clone()));
        let e = trace.register(53, Kind::Submission);
        assert_eq!(e.slot, b.slot);
        trace.step(Action::Begin(
            e,
            trace.roster[3 * count..4 * count].to_vec(),
        ));
        trace.step(Action::Success(a, b));
        trace.step(Action::Success(b, b));
        trace.step(Action::NoEffect(a, a));
        let f = trace.register(56, Kind::Synchronous);
        trace.step(Action::Begin(f, x.clone()));
        trace.step(Action::Success(f, f));
        for epoch in 4..=9 {
            let writer = trace.register(56 + epoch, Kind::Submission);
            trace.step(Action::Begin(writer, x.clone()));
            if epoch == 9 {
                trace.step(Action::Success(writer, writer));
            } else {
                trace.step(Action::NoEffect(writer, writer));
            }
        }
        trace.step(Action::Unknown(d));
        trace.step(Action::NoEffect(d, d));
        trace.step(Action::Begin(d, Vec::new()));
        trace.step(Action::Abort(d));
        trace.step(Action::Success(c, c));
        trace.step(Action::NoEffect(e, e));
        for descriptor in &x {
            let allocation = trace
                .journal
                .lookup_allocation(descriptor.allocation)
                .unwrap();
            assert_eq!(
                (allocation.attempt_epoch, allocation.content_lineage),
                (9, 9)
            );
        }
    }
}

#[test]
fn independent_action_sequences_cover_replay_pressure_and_all_writer_phases() {
    for seed in [1_u64, 7, 41, 0x9e37_79b9] {
        let mut trace = Trace::new(8, 4);
        let older = trace.register(11, Kind::Synchronous);
        let unknown = trace.register(14, Kind::Submission);
        trace.step(Action::Begin(unknown, vec![trace.roster[7]]));
        trace.step(Action::Unknown(unknown));
        let mut random = seed;
        for index in 0..16 {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let kind = if random & (1 << 32) == 0 {
                Kind::Synchronous
            } else {
                Kind::Submission
            };
            let reference = trace.register(20 + 2 * index, kind);
            let mut roster: Vec<_> = trace.roster[..7]
                .iter()
                .enumerate()
                .filter(|(slot, _)| random & (1 << slot) != 0)
                .map(|(_, value)| *value)
                .collect();
            if roster.is_empty() {
                roster.push(trace.roster[0]);
            }
            trace.step(Action::Begin(reference, roster.clone()));
            trace.step(Action::Begin(reference, Vec::new()));
            trace.step(Action::Success(reference, unknown));
            let probe = trace.register(21 + 2 * index, kind);
            trace.step(Action::Begin(probe, roster.clone()));
            trace.step(Action::Register(key(9_000 + index)));
            let mut malformed = roster.clone();
            malformed.push(roster[0]);
            trace.step(Action::Begin(probe, malformed));
            trace.step(Action::Abort(probe));
            if index % 2 == 0 {
                trace.step(Action::Success(reference, reference));
            } else {
                trace.step(Action::NoEffect(reference, reference));
            }
            trace.step(Action::Success(reference, reference));
            trace.step(Action::Unknown(unknown));
            trace.step(Action::Unknown(Reference {
                slot: usize::MAX,
                ..unknown
            }));
            trace.step(Action::Register(key(11)));
            trace.step(Action::Register(Key {
                context_generation: 8,
                ..key(0)
            }));
        }
        trace.step(Action::Begin(older, Vec::new()));
        trace.step(Action::NoEffect(older, older));
        for (index, label) in [
            "synchronous issuance",
            "submission issuance",
            "Begin",
            "Success",
            "NoEffect",
            "first Unknown",
            "repeated Unknown",
            "Reserved abort",
            "rejection",
            "writer pressure",
            "busy allocation",
            "empty Begin",
        ]
        .iter()
        .enumerate()
        {
            assert!(trace.coverage[index] > 0, "missing {label} for seed {seed}");
        }
    }
}
