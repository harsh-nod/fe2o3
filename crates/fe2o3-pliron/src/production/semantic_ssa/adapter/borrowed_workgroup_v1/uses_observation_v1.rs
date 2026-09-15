//! Opt-in, fixed-state Uses attribution. No matcher result or authority is retained.
use std::cell::Cell;
use std::ffi::OsStr;
use std::fmt::{self, Write as _};
use std::io::Write as _;

const PARTS: usize = 18;
const OUTPUT_BYTES: usize = 4096;
const MAX_DIAGNOSTIC_EVENTS: usize = 1_048_576;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(usize)]
pub(super) enum Part {
    Setup,
    GlobalSetup,
    Statement,
    GlobalStatement,
    CarrierSource,
    CarrierSiblings,
    CarrierDisjoint,
    ClosedLane,
    GridCapture,
    DefinedCapture,
    GlobalCapture,
    GlobalTerminator,
    TerminalCache,
    TerminalArguments,
    EpochEvidence,
    ArgumentAcceptance,
    UseRecording,
    #[default]
    Unclassified,
}

const LABELS: [&str; PARTS] = [
    "setup",
    "global-setup",
    "statement",
    "global-statement",
    "carrier-source",
    "carrier-siblings",
    "carrier-disjoint",
    "closed-lane",
    "grid-capture",
    "defined-capture",
    "global-capture",
    "global-terminator",
    "terminal-cache",
    "terminal-arguments",
    "epoch-evidence",
    "argument-acceptance",
    "use-recording",
    "unclassified",
];
const ROSTERS: [&str; 5] = [
    "locals",
    "candidates",
    "epochs",
    "global-facts",
    "callables",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MatchOwner {
    pub(super) index: usize,
    pub(super) facts: usize,
    pub(super) fact_count: usize,
    pub(super) keys: [usize; 2],
}

#[derive(Clone, Copy, Default)]
struct Reader {
    queries: usize,
    matchers: usize,
    matches: usize,
    callbacks: usize,
    accepted: usize,
    dispatch: usize,
    last_accepted: Option<(u32, u32, usize)>,
}

#[derive(Clone, Copy, Default)]
struct Statement {
    site: (u32, u32),
    source: usize,
    queries: [bool; 2],
    prefix: [usize; 2],
    last: [Option<usize>; 2],
    awaiting_callback: [Option<usize>; 2],
    accepted: [Option<usize>; 2],
    bucket: Option<(usize, usize)>,
}

#[derive(Clone, Copy)]
enum Incomplete {
    Events,
    Ownership,
    Order,
    Overflow,
}

impl Incomplete {
    fn label(self) -> &'static str {
        match self {
            Self::Events => "diagnostic-budget",
            Self::Ownership => "owner-mismatch",
            Self::Order => "query-order",
            Self::Overflow => "counter-overflow",
        }
    }
}

#[derive(Default)]
pub(super) struct Observation {
    identity: Option<(u32, [u8; 32])>,
    body: usize,
    part: Part,
    site: Option<(u32, u32)>,
    units: [usize; PARTS],
    visits: [usize; PARTS],
    rosters: [usize; 5],
    owner: Option<MatchOwner>,
    current: Option<Statement>,
    readers: [Reader; 2],
    closed: [usize; 4],
    overlap: usize,
    max_audit_prefix: usize,
    max_union_prefix: usize,
    audit_precharge: usize,
    events_left: usize,
    incomplete: Option<Incomplete>,
    emitted: Cell<bool>,
}

fn add(value: &mut usize, amount: usize) -> bool {
    let Some(next) = value.checked_add(amount) else {
        return false;
    };
    *value = next;
    true
}

impl Observation {
    pub(super) fn new(identity: Option<(u32, [u8; 32])>, rosters: [usize; 5], body: usize) -> Self {
        if identity.is_none() {
            return Self::default();
        }
        let trace = std::env::var_os("FE2O3_TRACE_CAPABILITY_CUSTODY");
        let role = std::env::var_os("FE2O3_TRACE_CAPABILITY_CUSTODY_ROLE");
        Self::with_setting(
            identity,
            rosters,
            body,
            trace.as_deref(),
            role.as_deref(),
            MAX_DIAGNOSTIC_EVENTS,
        )
    }

    fn with_setting(
        identity: Option<(u32, [u8; 32])>,
        rosters: [usize; 5],
        body: usize,
        trace: Option<&OsStr>,
        role: Option<&OsStr>,
        events: usize,
    ) -> Self {
        Self {
            identity: identity
                .filter(|_| trace == Some(OsStr::new("1")) && role == Some(OsStr::new("uses"))),
            body,
            rosters,
            part: Part::Setup,
            events_left: events.min(MAX_DIAGNOSTIC_EVENTS),
            ..Self::default()
        }
    }

    #[cfg(test)]
    pub(super) fn for_test(body: usize, facts: usize, events: usize) -> Self {
        let this = Self::with_setting(
            Some((27, [0x81; 32])),
            [0, 0, 0, facts, 0],
            body,
            Some(OsStr::new("1")),
            Some(OsStr::new("uses")),
            events,
        );
        this.emitted.set(true);
        this
    }

    #[cfg(test)]
    pub(super) fn test_counts(&self) -> ([usize; 2], [usize; 2], usize, bool) {
        let prefix = self.current.map_or([0; 2], |current| current.prefix);
        (
            self.readers.map(|r| r.matchers),
            self.readers.map(|r| r.callbacks),
            self.overlap + prefix[0].min(prefix[1]),
            self.incomplete.is_none(),
        )
    }

    fn event(&mut self) -> bool {
        if self.identity.is_none() || self.incomplete.is_some() {
            return false;
        }
        let Some(left) = self.events_left.checked_sub(1) else {
            self.incomplete = Some(Incomplete::Events);
            return false;
        };
        self.events_left = left;
        true
    }

    fn close_statement(&mut self) {
        let Some(current) = self.current.take() else {
            return;
        };
        if current.awaiting_callback != [None; 2] {
            self.incomplete = Some(Incomplete::Order);
            self.current = Some(current);
            return;
        }
        let class = usize::from(current.queries[0]) + 2 * usize::from(current.queries[1]);
        if !add(&mut self.closed[class], 1)
            || !add(&mut self.overlap, current.prefix[0].min(current.prefix[1]))
        {
            self.incomplete = Some(Incomplete::Overflow);
        }
        self.max_audit_prefix = self.max_audit_prefix.max(current.prefix[0]);
        self.max_union_prefix = self
            .max_union_prefix
            .max(current.prefix[0].max(current.prefix[1]));
    }

    pub(super) fn statement(&mut self, body: usize, source: usize, block: u32, statement: u32) {
        if !self.event() {
            return;
        }
        if body != self.body {
            self.incomplete = Some(Incomplete::Ownership);
            return;
        }
        self.close_statement();
        if self.incomplete.is_some() {
            return;
        }
        self.current = Some(Statement {
            source,
            site: (block, statement),
            ..Statement::default()
        });
        self.part = Part::Statement;
        self.site = Some((block, statement));
        if !add(&mut self.visits[Part::Statement as usize], 1) {
            self.incomplete = Some(Incomplete::Overflow);
        }
    }

    pub(super) fn setup(&mut self, part: Part) {
        if !self.event() {
            return;
        }
        self.close_statement();
        self.part = part;
        self.site = None;
        if !add(&mut self.visits[part as usize], 1) {
            self.incomplete = Some(Incomplete::Overflow);
        }
    }

    pub(super) fn enter(&mut self, part: Part, block: u32, statement: u32) {
        if !self.event() {
            return;
        }
        let site = (block, statement);
        if self.current.is_some_and(|current| current.site != site) {
            self.close_statement();
        }
        self.part = part;
        self.site = Some(site);
        if !add(&mut self.visits[part as usize], 1) {
            self.incomplete = Some(Incomplete::Overflow);
        }
    }

    pub(super) fn charged(&mut self, work: usize) {
        if self.event() && !add(&mut self.units[self.part as usize], work) {
            self.incomplete = Some(Incomplete::Overflow);
        }
    }

    fn reader(&mut self) -> Option<usize> {
        match self.part {
            Part::GlobalStatement => Some(0),
            Part::GlobalCapture => Some(1),
            _ => {
                self.incomplete = Some(Incomplete::Order);
                None
            }
        }
    }

    // Called only after find's original initial charge succeeds.
    pub(super) fn query(&mut self, source: usize, owner: MatchOwner) {
        if !self.event() {
            return;
        }
        let Some(reader) = self.reader() else { return };
        let Some(current) = self.current.as_mut() else {
            self.incomplete = Some(Incomplete::Ownership);
            return;
        };
        if current.source != source
            || self.site != Some(current.site)
            || self.owner.is_some_and(|old| old != owner)
            || owner.fact_count != self.rosters[3]
        {
            self.incomplete = Some(Incomplete::Ownership);
            return;
        }
        if current.queries[reader] || (reader == 0 && current.queries[1]) {
            self.incomplete = Some(Incomplete::Order);
            return;
        }
        self.owner = Some(owner);
        current.queries[reader] = true;
        if !add(&mut self.readers[reader].queries, 1) || !add(&mut self.readers[reader].dispatch, 1)
        {
            self.incomplete = Some(Incomplete::Overflow);
        }
    }

    pub(super) fn dispatch(&mut self, work: usize) {
        if !self.event() {
            return;
        }
        let Some(reader) = self.reader() else { return };
        if !self.current.is_some_and(|current| current.queries[reader]) {
            self.incomplete = Some(Incomplete::Order);
        } else if !add(&mut self.readers[reader].dispatch, work) {
            self.incomplete = Some(Incomplete::Overflow);
        }
    }

    pub(super) fn bucket(&mut self, pointer: usize, len: usize) {
        if !self.event() {
            return;
        }
        let Some(reader) = self.reader() else { return };
        let Some(current) = self.current.as_mut() else {
            self.incomplete = Some(Incomplete::Ownership);
            return;
        };
        if !current.queries[reader] || current.bucket.is_some_and(|old| old != (pointer, len)) {
            self.incomplete = Some(Incomplete::Ownership);
            return;
        }
        current.bucket = Some((pointer, len));
    }

    // The pure matcher has returned; rejected matches still extend the prefix.
    pub(super) fn matcher(&mut self, ordinal: usize, matched: bool) {
        if !self.event() {
            return;
        }
        let Some(reader) = self.reader() else { return };
        let Some(current) = self.current.as_mut() else {
            self.incomplete = Some(Incomplete::Ownership);
            return;
        };
        if !current.queries[reader]
            || current.bucket.is_none()
            || self.owner.is_none_or(|owner| ordinal >= owner.fact_count)
            || current.last[reader].is_some_and(|last| ordinal <= last)
            || current.awaiting_callback[reader].is_some()
            || current.accepted[reader].is_some()
            || current.prefix[reader] >= current.bucket.unwrap().1
        {
            self.incomplete = Some(Incomplete::Order);
            return;
        }
        current.last[reader] = Some(ordinal);
        current.awaiting_callback[reader] = matched.then_some(ordinal);
        if !add(&mut current.prefix[reader], 1)
            || !add(&mut self.readers[reader].matchers, 1)
            || !add(&mut self.readers[reader].matches, usize::from(matched))
        {
            self.incomplete = Some(Incomplete::Overflow);
        }
    }

    // A callback returning None is an executed callback, not an accepted use.
    pub(super) fn callback(&mut self, ordinal: usize, accepted: bool) {
        if !self.event() {
            return;
        }
        let Some(reader) = self.reader() else { return };
        let Some(current) = self.current.as_mut() else {
            self.incomplete = Some(Incomplete::Ownership);
            return;
        };
        if current.awaiting_callback[reader] != Some(ordinal) {
            self.incomplete = Some(Incomplete::Order);
            return;
        }
        current.awaiting_callback[reader] = None;
        if accepted {
            current.accepted[reader] = Some(ordinal);
            self.readers[reader].last_accepted = Some((current.site.0, current.site.1, ordinal));
        }
        if !add(&mut self.readers[reader].callbacks, 1)
            || !add(&mut self.readers[reader].accepted, usize::from(accepted))
        {
            self.incomplete = Some(Incomplete::Overflow);
        }
    }

    pub(super) fn audit_precharge(&mut self, work: usize) {
        if self.event() && !add(&mut self.audit_precharge, work) {
            self.incomplete = Some(Incomplete::Overflow);
        }
    }

    pub(super) fn complete(&mut self, total: usize, remaining: usize) {
        if self.event() {
            self.close_statement();
        }
        self.emit_summary("uses-complete", total, Some((remaining, 0)));
    }

    pub(super) fn emit(&self, total: usize, remaining: usize, requested: usize) {
        self.emit_summary(
            "compiler-budget-prefix",
            total,
            Some((remaining, requested)),
        );
    }

    pub(super) fn aborted(&self, total: usize) {
        self.emit_summary("aborted-prefix", total, None);
    }

    fn emit_summary(&self, outcome: &str, total: usize, boundary: Option<(usize, usize)>) {
        if let Some(text) = self.take_summary(outcome, total, boundary) {
            let _ = std::io::stderr().lock().write_all(&text.bytes[..text.len]);
        }
    }

    fn take_summary(
        &self,
        outcome: &str,
        total: usize,
        boundary: Option<(usize, usize)>,
    ) -> Option<Text> {
        self.identity?;
        if self.emitted.replace(true) {
            return None;
        }
        let mut text = Text::default();
        if self.render(&mut text, outcome, total, boundary).is_err() {
            text = Text::default();
            let (root, view) = self.identity?;
            write!(text, "fe2o3-capability-uses root={root} view=").ok()?;
            for byte in view {
                write!(text, "{byte:02x}").ok()?;
            }
            writeln!(
                text,
                " collector=incomplete reason=output-capacity overlap=unknown"
            )
            .ok()?;
        }
        Some(text)
    }

    fn render(
        &self,
        text: &mut Text,
        outcome: &str,
        total: usize,
        boundary: Option<(usize, usize)>,
    ) -> fmt::Result {
        let (root, view) = self.identity.ok_or(fmt::Error)?;
        let prefix = self.current.map_or([0; 2], |current| current.prefix);
        let overlap = self.overlap.checked_add(prefix[0].min(prefix[1]));
        let sum = self
            .units
            .iter()
            .try_fold(0usize, |sum, work| sum.checked_add(*work));
        let reason = self
            .incomplete
            .or_else(|| (overlap.is_none() || sum.is_none()).then_some(Incomplete::Overflow));
        let complete = reason.is_none() && sum == Some(total);
        write!(text, "fe2o3-capability-uses root={root} view=")?;
        for byte in view {
            write!(text, "{byte:02x}")?;
        }
        write!(
            text,
            " schema=overlap88 outcome={outcome} part={} site={:?} total={total} boundary={boundary:?}",
            LABELS[self.part as usize], self.site
        )?;
        write!(
            text,
            " collector={} reason={} events-left={} ",
            if complete { "complete" } else { "incomplete" },
            reason.map_or(
                if complete {
                    "none"
                } else {
                    "region-total-mismatch"
                },
                Incomplete::label
            ),
            self.events_left
        )?;
        if complete {
            write!(text, "overlap={}", overlap.unwrap())?;
        } else {
            write!(
                text,
                "overlap=unknown observed-overlap-prefix={:?}",
                overlap
            )?;
        }
        write!(
            text,
            " current-prefix={prefix:?} current-first-accepted={:?} closed-none-audit-uses-both={:?} max-audit-prefix={} max-union-prefix={} audit-callback-precharge={} keys={:?}",
            self.current.map(|current| current.accepted),
            self.closed,
            self.max_audit_prefix.max(prefix[0]),
            self.max_union_prefix.max(prefix[0].max(prefix[1])),
            self.audit_precharge,
            self.owner.map(|owner| owner.keys)
        )?;
        for (label, reader) in ["audit", "uses"].into_iter().zip(self.readers) {
            write!(
                text,
                " {label}-queries={} {label}-matchers={} {label}-matches={} {label}-callbacks={} {label}-accepted={} {label}-dispatch={} {label}-last-accepted={:?}",
                reader.queries,
                reader.matchers,
                reader.matches,
                reader.callbacks,
                reader.accepted,
                reader.dispatch,
                reader.last_accepted
            )?;
        }
        for (label, count) in ROSTERS.iter().zip(self.rosters) {
            write!(text, " roster-{label}={count}")?;
        }
        for (index, label) in LABELS.iter().enumerate() {
            write!(
                text,
                " {label}={}/{}",
                self.units[index], self.visits[index]
            )?;
        }
        text.write_char('\n')
    }
}

struct Text {
    bytes: [u8; OUTPUT_BYTES],
    len: usize,
}
impl Default for Text {
    fn default() -> Self {
        Self {
            bytes: [0; OUTPUT_BYTES],
            len: 0,
        }
    }
}
impl fmt::Write for Text {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.len.checked_add(value.len()).ok_or(fmt::Error)?;
        self.bytes
            .get_mut(self.len..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[cfg(test)]
#[path = "uses_observation_v1/tests.rs"]
mod tests;
