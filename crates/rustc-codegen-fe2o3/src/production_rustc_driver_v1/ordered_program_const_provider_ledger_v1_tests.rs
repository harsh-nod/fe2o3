//! Fixed-capacity observer state. No query, formatting, file I/O or allocation
//! is requested by this ledger. Inclusive nested clocks must never be summed.
use super::{DefId, Serialize};
use std::thread::ThreadId;
use std::time::Instant;

pub(super) const ROW_CAP: usize = 256;
pub(super) const DEPTH_CAP: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Key {
    pub(super) definition: DefId,
    pub(super) item: bool,
    pub(super) empty_args: bool,
    pub(super) no_promoted: bool,
    pub(super) fully_monomorphized: bool,
}
impl Key {
    pub(super) fn exact_supported(self) -> bool {
        self.item && self.empty_args && self.no_promoted && self.fully_monomorphized
    }
}

#[derive(Clone, Copy)]
pub(super) struct Row {
    pub(super) key: Key,
    pub(super) parent: Option<usize>,
    pub(super) depth: usize,
    pub(super) start_ns: Option<u128>,
    pub(super) end_ns: Option<u128>,
    pub(super) elapsed_ns: Option<u128>,
    pub(super) returned_ok: Option<bool>,
    pub(super) unwound: bool,
}
#[derive(Serialize)]
pub(super) struct WireRow {
    ordinal: usize,
    definition_crate: u32,
    definition_index: u32,
    item: bool,
    empty_args: bool,
    no_promoted: bool,
    fully_monomorphized: bool,
    parent: Option<usize>,
    depth: usize,
    // Decimal strings keep all u128 nanoseconds exact in JSON consumers.
    start_ns: Option<String>,
    end_ns: Option<String>,
    elapsed_ns: Option<String>,
    returned_ok: Option<bool>,
    unwound: bool,
}
impl Row {
    pub(super) fn wire(self, ordinal: usize) -> WireRow {
        WireRow {
            ordinal,
            definition_crate: self.key.definition.krate.as_u32(),
            definition_index: self.key.definition.index.as_u32(),
            item: self.key.item,
            empty_args: self.key.empty_args,
            no_promoted: self.key.no_promoted,
            fully_monomorphized: self.key.fully_monomorphized,
            parent: self.parent,
            depth: self.depth,
            start_ns: self.start_ns.map(|value| value.to_string()),
            end_ns: self.end_ns.map(|value| value.to_string()),
            elapsed_ns: self.elapsed_ns.map(|value| value.to_string()),
            returned_ok: self.returned_ok,
            unwound: self.unwound,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Snapshot {
    pub(super) rows: [Option<Row>; ROW_CAP],
    pub(super) count: usize,
    pub(super) installed: bool,
    pub(super) failed: bool,
    pub(super) active_depth: usize,
    pub(super) fenced: bool,
}
impl Snapshot {
    pub(super) fn ready(self) -> bool {
        self.installed && !self.failed && self.fenced && self.active_depth == 0
    }
    pub(super) fn unique_completed(self, definition: DefId) -> Result<(usize, Row), &'static str> {
        if !self.ready() {
            return Err("provider ledger is incomplete or refused");
        }
        let mut selected = None;
        for (ordinal, row) in self.rows[..self.count].iter().enumerate() {
            let row = row.ok_or("reserved provider row absent")?;
            if row.key.definition == definition {
                if selected.is_some() {
                    return Err("selected const has multiple provider invocations");
                }
                if !row.key.exact_supported()
                    || row.returned_ok != Some(true)
                    || row.unwound
                    || row.start_ns.is_none()
                    || row.end_ns.is_none()
                    || row.elapsed_ns.is_none()
                {
                    return Err("selected provider key or completion differs");
                }
                selected = Some((ordinal, row));
            }
        }
        selected.ok_or("selected const has no actual provider invocation")
    }
}

pub(super) struct Ledger {
    rows: [Option<Row>; ROW_CAP],
    count: usize,
    stack: [usize; DEPTH_CAP],
    depth: usize,
    thread: Option<ThreadId>,
    session: Option<usize>,
    origin: Option<Instant>,
    installed: bool,
    failed: bool,
    fenced: bool,
}
impl Ledger {
    pub(super) const fn new() -> Self {
        Self {
            rows: [None; ROW_CAP],
            count: 0,
            stack: [0; DEPTH_CAP],
            depth: 0,
            thread: None,
            session: None,
            origin: None,
            installed: false,
            failed: false,
            fenced: false,
        }
    }
    pub(super) fn fail(&mut self) {
        self.failed = true;
    }
    pub(super) fn arm(&mut self, session: usize, origin: Instant) -> bool {
        if self.installed || self.fenced || self.session.is_some() {
            self.fail();
            return false;
        }
        self.installed = true;
        self.session = Some(session);
        self.origin = Some(origin);
        true
    }
    pub(super) fn begin(&mut self, key: Key, session: usize, thread: ThreadId) -> Option<usize> {
        if self.fenced {
            return None;
        }
        if !self.installed
            || self.session != Some(session)
            || self.thread.is_some_and(|prior| prior != thread)
            || self.count == ROW_CAP
            || self.depth == DEPTH_CAP
        {
            self.fail();
            return None;
        }
        self.thread = Some(thread);
        let ordinal = self.count;
        self.rows[ordinal] = Some(Row {
            key,
            parent: self.depth.checked_sub(1).map(|index| self.stack[index]),
            depth: self.depth,
            start_ns: None,
            end_ns: None,
            elapsed_ns: None,
            returned_ok: None,
            unwound: false,
        });
        self.count += 1;
        self.stack[self.depth] = ordinal;
        self.depth += 1;
        Some(ordinal)
    }
    pub(super) fn complete(
        &mut self,
        ordinal: usize,
        start: Instant,
        end: Instant,
        returned_ok: Option<bool>,
    ) {
        let timing = self
            .origin
            .and_then(|origin| {
                start
                    .checked_duration_since(origin)
                    .zip(end.checked_duration_since(origin))
            })
            .zip(end.checked_duration_since(start));
        if ordinal >= self.count || self.depth == 0 || self.stack[self.depth - 1] != ordinal {
            self.fail();
            return;
        }
        self.depth -= 1;
        let Some(row) = self.rows[ordinal].as_mut() else {
            self.fail();
            return;
        };
        if row.start_ns.is_some() || row.unwound {
            self.fail();
            return;
        }
        if let Some(((start, end), elapsed)) = timing {
            row.start_ns = Some(start.as_nanos());
            row.end_ns = Some(end.as_nanos());
            row.elapsed_ns = Some(elapsed.as_nanos());
        } else {
            self.failed = true;
        }
        row.returned_ok = returned_ok;
        row.unwound = returned_ok.is_none();
        if row.unwound {
            self.failed = true;
        }
    }
    pub(super) fn fence(&mut self) {
        self.fenced = true;
        if self.depth != 0 {
            self.fail();
        }
    }
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            rows: self.rows,
            count: self.count,
            installed: self.installed,
            failed: self.failed,
            active_depth: self.depth,
            fenced: self.fenced,
        }
    }
}
