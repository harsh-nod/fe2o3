//! Test-only Linux process observations, never compiler-owner heap accounting.
//! The ordinary latency selector never reads procfs or emits these records.
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
};

const STATUS_CAP: usize = 16 * 1024;
const READ_CALL_CAP: usize = 64;
const RECORD_CAP: usize = 4096;
const PREFIX: &[u8] = b"FE2O3_RECIPE_PROCESS_MEMORY_V1 ";

#[derive(Debug)]
struct StatusBytes {
    bytes: [u8; STATUS_CAP + 1],
    len: usize,
}
fn read_bounded(reader: &mut impl Read) -> Result<StatusBytes, String> {
    let mut out = StatusBytes {
        bytes: [0; STATUS_CAP + 1],
        len: 0,
    };
    for _ in 0..READ_CALL_CAP {
        let end = (out.len + 4096).min(STATUS_CAP + 1);
        let count = match reader.read(&mut out.bytes[out.len..end]) {
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("process status read: {error}")),
        };
        if count == 0 {
            if out.len == 0 {
                return Err("empty process status".into());
            }
            return Ok(out);
        }
        out.len = out
            .len
            .checked_add(count)
            .ok_or("process status read overflow")?;
        if out.len > STATUS_CAP {
            return Err("process status byte cap".into());
        }
    }
    Err("process status read-call cap".into())
}
fn read_current() -> Result<StatusBytes, String> {
    if std::env::consts::OS != "linux" {
        return Err("process RSS observation requires Linux procfs".into());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open("/proc/self/status")
        .map_err(|e| format!("process status open: {e}"))?;
    read_bounded(&mut file)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Snapshot {
    rss_bytes: u64,
    high_water_rss_bytes: u64,
}
fn decimal(value: &str) -> Result<u64, String> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err("process status unsigned decimal".into());
    }
    value
        .parse::<u64>()
        .map_err(|_| "process status decimal overflow".into())
}
fn parse_status(bytes: &[u8], expected_pid: u32) -> Result<Snapshot, String> {
    if bytes.is_empty()
        || bytes.len() > STATUS_CAP
        || !bytes.ends_with(b"\n")
        || bytes.contains(&0)
        || bytes.contains(&b'\r')
        || expected_pid == 0
    {
        return Err("process status framing/identity".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "process status UTF-8")?;
    let mut pid = None;
    let mut tgid = None;
    let mut rss = None;
    let mut hwm = None;
    for line in text.lines() {
        let (slot, value, memory) = if let Some(value) = line.strip_prefix("Pid:") {
            (&mut pid, value, false)
        } else if let Some(value) = line.strip_prefix("Tgid:") {
            (&mut tgid, value, false)
        } else if let Some(value) = line.strip_prefix("VmRSS:") {
            (&mut rss, value, true)
        } else if let Some(value) = line.strip_prefix("VmHWM:") {
            (&mut hwm, value, true)
        } else {
            continue;
        };
        if slot.is_some() {
            return Err("duplicate process status field".into());
        }
        let mut fields = value.split_ascii_whitespace();
        let value = decimal(fields.next().ok_or("missing process status value")?)?;
        let value = if memory {
            if fields.next() != Some("kB") {
                return Err("process status kB unit required".into());
            }
            value
                .checked_mul(1024)
                .ok_or("process status byte conversion overflow")?
        } else {
            value
        };
        if fields.next().is_some() {
            return Err("extra process status field".into());
        }
        *slot = Some(value);
    }
    if pid != Some(u64::from(expected_pid)) || tgid != Some(u64::from(expected_pid)) {
        return Err("process status PID/TGID differs".into());
    }
    // Linux documents both values as approximate; separately sampled fields
    // need not form an atomic snapshot. Do not invent hwm >= rss admission.
    Ok(Snapshot {
        rss_bytes: rss.ok_or("missing process VmRSS")?,
        high_water_rss_bytes: hwm.ok_or("missing process VmHWM")?,
    })
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stage {
    BeforeRun,
    BeforeTransaction,
    AfterReturn,
    AfterResultDrop,
    Terminal,
}
impl Stage {
    fn name(self) -> &'static str {
        match self {
            Self::BeforeRun => "before_run",
            Self::BeforeTransaction => "before_transaction",
            Self::AfterReturn => "after_return_result_retained",
            Self::AfterResultDrop => "after_result_drop",
            Self::Terminal => "terminal_before_child_return",
        }
    }
}
struct Identity {
    config: String,
    source: String,
    workload: &'static str,
    pid: u32,
}
pub(super) struct Profile {
    identity: Option<Identity>,
    calls: usize,
    emitted: usize,
    poisoned: bool,
}
impl Profile {
    pub(super) fn new(
        config_sha256: Option<&str>,
        source_sha256: &str,
        workload: &'static str,
        calls: usize,
    ) -> Result<Self, String> {
        if !matches!(calls, 1 | 35) {
            return Err("closed process observation call count".into());
        }
        let identity = match config_sha256 {
            None => None,
            Some(config) => {
                super::hash(config)?;
                super::hash(source_sha256)?;
                if !matches!(workload, "checked_rebind" | "exact_revision_refusal") {
                    return Err("closed process observation workload".into());
                }
                Some(Identity {
                    config: config.to_owned(),
                    source: source_sha256.to_owned(),
                    workload,
                    pid: std::process::id(),
                })
            }
        };
        Ok(Self {
            identity,
            calls,
            emitted: 0,
            poisoned: false,
        })
    }
    pub(super) fn enabled(&self) -> bool {
        self.identity.is_some()
    }
    fn expected(&self) -> Result<(Stage, Option<usize>), String> {
        if self.emitted == 0 {
            return Ok((Stage::BeforeRun, None));
        }
        if self.emitted == 3 * self.calls + 1 {
            return Ok((Stage::Terminal, None));
        }
        if self.emitted > 3 * self.calls + 1 {
            return Err("process observation sample cap".into());
        }
        let item = self.emitted - 1;
        Ok((
            match item % 3 {
                0 => Stage::BeforeTransaction,
                1 => Stage::AfterReturn,
                _ => Stage::AfterResultDrop,
            },
            Some(item / 3 + 1),
        ))
    }
    pub(super) fn observe(&mut self, stage: Stage, ordinal: Option<usize>) -> Result<(), String> {
        if !self.enabled() {
            return Ok(());
        }
        self.observe_with(stage, ordinal, read_current, &mut std::io::stderr().lock())
    }
    fn observe_with(
        &mut self,
        stage: Stage,
        ordinal: Option<usize>,
        read: impl FnOnce() -> Result<StatusBytes, String>,
        out: &mut impl Write,
    ) -> Result<(), String> {
        if !self.enabled() {
            return Ok(());
        }
        if self.poisoned {
            return Err("process observation already refused".into());
        }
        // Any refusal permanently prevents a terminal-complete record.
        self.poisoned = true;
        if self.expected()? != (stage, ordinal) {
            return Err("process observation stage/ordinal sequence".into());
        }
        let identity = self
            .identity
            .as_ref()
            .ok_or("process identity unavailable")?;
        let bytes = read()?;
        let snapshot = parse_status(&bytes.bytes[..bytes.len], identity.pid)?;
        let record = serde_json::json!({
            "schema":"fe2o3-recipe-process-memory-v1",
            "scope":"whole_linux_process_including_frontend_and_observer",
            "source":"/proc/self/status", "platform":"linux",
            "pid":identity.pid,"config_sha256":identity.config,
            "current_source_sha256":identity.source,"workload":identity.workload,
            "sequence":self.emitted + 1,"expected_samples":3 * self.calls + 2,
            "call_ordinal":ordinal,"stage":stage.name(),
            "rss_bytes":snapshot.rss_bytes,"os_high_water_rss_bytes":snapshot.high_water_rss_bytes,
            "reported_unit":"kB","byte_multiplier":1024,"os_values_approximate":true,
            "fields_atomic_snapshot":false,"process_exit_observed":false,
            "high_water_scope":"process_lifetime_through_this_checkpoint_not_reset",
            "terminal_checkpoint_complete":stage == Stage::Terminal,
            "latency_profile_comparable":false,"retained_owner_bytes":null,
            "temporary_owner_overlap_bytes":null,"heap_peak_bytes":null,
            "cancellation_observed":false,"budget_accepted":false,"grants_authority":false
        });
        let bytes = super::json_bytes(&record, RECORD_CAP)?;
        super::record_to(out, PREFIX, &bytes)?;
        self.emitted += 1;
        self.poisoned = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn status(pid: u32) -> Vec<u8> {
        format!("Name:\ttest\nPid:\t{pid}\nTgid:\t{pid}\nVmRSS:\t12 kB\nVmHWM:\t19 kB\n")
            .into_bytes()
    }
    fn enabled_profile(calls: usize) -> Profile {
        Profile::new(
            Some(&"11".repeat(32)),
            &"22".repeat(32),
            "checked_rebind",
            calls,
        )
        .unwrap()
    }
    fn emit(
        profile: &mut Profile,
        stage: Stage,
        ordinal: Option<usize>,
        out: &mut Vec<u8>,
    ) -> Result<(), String> {
        let bytes = status(std::process::id());
        profile.observe_with(
            stage,
            ordinal,
            || read_bounded(&mut Cursor::new(bytes)),
            out,
        )
    }
    #[test]
    fn linux_units_and_exact_pid_tgid_are_checked() {
        assert_eq!(
            parse_status(&status(41), 41).unwrap(),
            Snapshot {
                rss_bytes: 12288,
                high_water_rss_bytes: 19456
            }
        );
        assert!(parse_status(&status(41), 42).is_err());
        assert!(parse_status(&status(41), 0).is_err());
        let changed = String::from_utf8(status(41))
            .unwrap()
            .replace("Tgid:\t41", "Tgid:\t42");
        assert!(parse_status(changed.as_bytes(), 41).is_err());
    }
    #[test]
    fn missing_and_duplicate_required_fields_refuse() {
        let base = String::from_utf8(status(41)).unwrap();
        for prefix in ["Pid:", "Tgid:", "VmRSS:", "VmHWM:"] {
            let line = base.lines().find(|line| line.starts_with(prefix)).unwrap();
            let missing = base.replace(&format!("{line}\n"), "");
            assert!(parse_status(missing.as_bytes(), 41).is_err());
            assert!(parse_status(format!("{base}{line}\n").as_bytes(), 41).is_err());
        }
    }
    #[test]
    fn malformed_unit_decimal_and_overflow_refuse() {
        let base = String::from_utf8(status(41)).unwrap();
        for bad in [
            "12 KB",
            "12 B",
            "12",
            "-1 kB",
            "+1 kB",
            "1.5 kB",
            "18446744073709551616 kB",
            "18014398509481984 kB",
            "1 kB extra",
            "kB",
        ] {
            assert!(
                parse_status(base.replace("12 kB", bad).as_bytes(), 41).is_err(),
                "{bad}"
            );
        }
        let accepted = base.replace("12 kB", "18014398509481983 kB");
        assert_eq!(
            parse_status(accepted.as_bytes(), 41).unwrap().rss_bytes,
            u64::MAX - 1023
        );
    }
    #[test]
    fn framing_utf8_empty_and_size_refuse() {
        let base = status(41);
        for bytes in [
            Vec::new(),
            base[..base.len() - 1].to_vec(),
            vec![b'x'; STATUS_CAP + 1],
            b"Pid:\0\n".to_vec(),
            b"Pid:\r\n".to_vec(),
            vec![255, b'\n'],
        ] {
            assert!(parse_status(&bytes, 41).is_err());
        }
    }
    #[test]
    fn approximate_fields_are_not_forced_into_an_atomic_order() {
        let bytes = String::from_utf8(status(41))
            .unwrap()
            .replace("19 kB", "1 kB");
        let snapshot = parse_status(bytes.as_bytes(), 41).unwrap();
        assert!(snapshot.high_water_rss_bytes < snapshot.rss_bytes);
    }
    #[test]
    fn exact_byte_limit_has_eof_probe_and_one_more_refuses() {
        assert_eq!(
            read_bounded(&mut Cursor::new(vec![b'x'; STATUS_CAP]))
                .unwrap()
                .len,
            STATUS_CAP
        );
        assert!(read_bounded(&mut Cursor::new(vec![b'x'; STATUS_CAP + 1])).is_err());
        assert!(read_bounded(&mut Cursor::new(Vec::<u8>::new())).is_err());
    }
    #[test]
    fn interrupted_and_short_reads_cannot_escape_read_call_cap() {
        struct Interrupted;
        impl Read for Interrupted {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::Interrupted.into())
            }
        }
        struct One;
        impl Read for One {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                out[0] = b'x';
                Ok(1)
            }
        }
        assert!(
            read_bounded(&mut Interrupted)
                .unwrap_err()
                .contains("read-call cap")
        );
        assert!(
            read_bounded(&mut One)
                .unwrap_err()
                .contains("read-call cap")
        );
    }
    #[test]
    fn full_one_and_thirty_five_call_sequences_bind_identity_and_count() {
        for calls in [1, 35] {
            let mut profile = enabled_profile(calls);
            let mut out = Vec::new();
            emit(&mut profile, Stage::BeforeRun, None, &mut out).unwrap();
            for ordinal in 1..=calls {
                for stage in [
                    Stage::BeforeTransaction,
                    Stage::AfterReturn,
                    Stage::AfterResultDrop,
                ] {
                    emit(&mut profile, stage, Some(ordinal), &mut out).unwrap();
                }
            }
            emit(&mut profile, Stage::Terminal, None, &mut out).unwrap();
            let text = String::from_utf8(out).unwrap();
            let rows: Vec<serde_json::Value> = text
                .lines()
                .filter_map(|line| {
                    line.strip_prefix(std::str::from_utf8(PREFIX).unwrap())
                        .map(|text| serde_json::from_str(text).unwrap())
                })
                .collect();
            assert_eq!(rows.len(), 3 * calls + 2);
            for (index, row) in rows.iter().enumerate() {
                assert_eq!(row["sequence"], index + 1);
                assert_eq!(row["expected_samples"], 3 * calls + 2);
                assert_eq!(row["pid"], std::process::id());
                assert_eq!(row["config_sha256"], "11".repeat(32));
                assert_eq!(row["current_source_sha256"], "22".repeat(32));
                assert_eq!(row["workload"], "checked_rebind");
                assert!(row["retained_owner_bytes"].is_null());
                assert_eq!(row["terminal_checkpoint_complete"], index + 1 == rows.len());
                assert_eq!(row["process_exit_observed"], false);
                assert_eq!(row["latency_profile_comparable"], false);
            }
            assert!(emit(&mut profile, Stage::Terminal, None, &mut Vec::new()).is_err());
        }
    }
    #[test]
    fn skipped_duplicate_wrong_ordinal_and_early_terminal_poison_profile() {
        for (stage, ordinal) in [(Stage::Terminal, None), (Stage::BeforeTransaction, Some(1))] {
            let mut profile = enabled_profile(1);
            assert!(emit(&mut profile, stage, ordinal, &mut Vec::new()).is_err());
            assert!(emit(&mut profile, Stage::BeforeRun, None, &mut Vec::new()).is_err());
        }
        for (stage, ordinal) in [
            (Stage::BeforeRun, None),
            (Stage::BeforeTransaction, Some(2)),
            (Stage::AfterReturn, Some(1)),
            (Stage::Terminal, None),
        ] {
            let mut profile = enabled_profile(1);
            emit(&mut profile, Stage::BeforeRun, None, &mut Vec::new()).unwrap();
            assert!(emit(&mut profile, stage, ordinal, &mut Vec::new()).is_err());
        }
    }
    #[test]
    fn read_and_output_failure_cannot_be_completed_or_retried() {
        let mut profile = enabled_profile(1);
        assert!(
            profile
                .observe_with(
                    Stage::BeforeRun,
                    None,
                    || Err("injected read".into()),
                    &mut Vec::new()
                )
                .is_err()
        );
        assert!(emit(&mut profile, Stage::BeforeRun, None, &mut Vec::new()).is_err());
        struct Fail;
        impl Write for Fail {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::Other.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut profile = enabled_profile(1);
        assert!(
            profile
                .observe_with(
                    Stage::BeforeRun,
                    None,
                    || read_bounded(&mut Cursor::new(status(std::process::id()))),
                    &mut Fail
                )
                .is_err()
        );
        assert!(emit(&mut profile, Stage::BeforeRun, None, &mut Vec::new()).is_err());
    }
    #[test]
    fn disabled_latency_profile_never_reads_writes_or_sequences() {
        struct Fail;
        impl Write for Fail {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                panic!("unexpected write")
            }
            fn flush(&mut self) -> std::io::Result<()> {
                panic!("unexpected flush")
            }
        }
        let mut profile = Profile::new(None, "not used", "not used", 35).unwrap();
        assert!(!profile.enabled());
        for _ in 0..108 {
            profile
                .observe_with(
                    Stage::Terminal,
                    Some(999),
                    || panic!("unexpected read"),
                    &mut Fail,
                )
                .unwrap();
        }
        assert_eq!(profile.emitted, 0);
    }
    #[test]
    fn closed_constructor_rejects_unbounded_calls_and_bad_binding() {
        for calls in [0, 2, 34, 36, usize::MAX] {
            assert!(Profile::new(None, "", "", calls).is_err());
        }
        assert!(Profile::new(Some("bad"), &"22".repeat(32), "checked_rebind", 1).is_err());
        assert!(Profile::new(Some(&"11".repeat(32)), "bad", "checked_rebind", 1).is_err());
        assert!(Profile::new(Some(&"11".repeat(32)), &"22".repeat(32), "cancel", 1).is_err());
    }
}
