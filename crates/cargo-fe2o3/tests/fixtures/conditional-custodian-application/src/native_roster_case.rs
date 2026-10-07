//! Bounded native-V5 selected-roster and exact transport oracles, not authority.
use std::ffi::OsString;

pub const MAX_DEVICES: usize = 8;
pub const GUARD_BYTES: usize = 32;
pub const TAG_BYTES: usize = 24;
pub const FRAME_BYTES: usize = GUARD_BYTES * 2 + TAG_BYTES + (32 + MAX_DEVICES - 1) * 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    NativeXgmi,
    HostStaged,
}
impl Mode {
    pub fn token(self) -> &'static str {
        match self {
            Self::NativeXgmi => "native-xgmi",
            Self::HostStaged => "host-staged",
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
pub struct Case {
    pub producer_source: String,
    pub devices: Vec<u64>,
    pub mode: Mode,
}
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Case, String> {
    let mut args = args.into_iter();
    let mut next = || {
        args.next()
            .ok_or("incomplete native roster arguments")?
            .into_string()
            .map_err(|_| "native roster arguments must be UTF-8")
    };
    if next()? != "--native-v5-roster" || next()? != "--producer-source" {
        return Err("native roster requires its explicit selector and producer source".into());
    }
    let producer_source = next()?;
    if producer_source.is_empty()
        || producer_source.len() > 4096
        || producer_source.as_bytes().contains(&0)
    {
        return Err("invalid native producer source spelling".into());
    }
    if next()? != "--transport" {
        return Err("native roster requires explicit transport".into());
    }
    let mode = match next()?.as_str() {
        "native-xgmi" => Mode::NativeXgmi,
        "host-staged" => Mode::HostStaged,
        _ => return Err("unknown native roster transport; no fallback selection".into()),
    };
    if next()? != "--devices" {
        return Err("native roster requires explicit devices".into());
    }
    let mut devices = Vec::with_capacity(MAX_DEVICES);
    for arg in args {
        if devices.len() == MAX_DEVICES {
            return Err("native roster exceeds eight devices".into());
        }
        let text = arg.to_str().ok_or("native UID is not UTF-8")?;
        let digits = text
            .strip_prefix("0x")
            .filter(|digits| {
                digits.len() == 16
                    && digits
                        .bytes()
                        .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
            })
            .ok_or("native UID requires 16 lowercase hexadecimal digits")?;
        let uid = u64::from_str_radix(digits, 16).map_err(|e| e.to_string())?;
        if uid == 0 || devices.contains(&uid) {
            return Err("native roster requires distinct nonzero UIDs".into());
        }
        devices.push(uid);
    }
    if devices.len() < 2 {
        return Err("native roster requires two through eight selected devices".into());
    }
    Ok(Case {
        producer_source,
        devices,
        mode,
    })
}

impl Case {
    pub fn elements(&self, shard: usize) -> usize {
        assert!(shard < self.devices.len());
        32 + shard
    }
    pub fn routes(&self) -> impl Iterator<Item = (usize, usize)> {
        let count = self.devices.len();
        (0..count).flat_map(move |source| {
            (0..count)
                .filter(move |destination| *destination != source)
                .map(move |destination| (source, destination))
        })
    }
    pub fn check_roster(&self, observed: &[(u64, u16)]) -> Result<(), String> {
        if observed.len() != self.devices.len()
            || observed.iter().enumerate().any(|(i, (uid, minor))| {
                *uid != self.devices[i]
                    || observed[..i]
                        .iter()
                        .any(|(_, old_minor)| old_minor == minor)
            })
        {
            return Err("native admitted UID/render roster differs or aliases".into());
        }
        Ok(())
    }
    pub fn check_fill(&self, shard: usize, output: &[u32]) -> Result<(), String> {
        if output.len() != self.elements(shard)
            || output.iter().enumerate().any(|(i, x)| *x != i as u32)
        {
            return Err("native shard output value or exact extent differs".into());
        }
        Ok(())
    }
    pub fn payload(&self, shard: usize, output: &[u32]) -> Result<Vec<u8>, String> {
        self.check_fill(shard, output)?;
        let mut bytes = Vec::with_capacity(TAG_BYTES + output.len() * 4);
        bytes.extend_from_slice(b"F2NROST1");
        bytes.extend_from_slice(&self.devices[shard].to_le_bytes());
        bytes.extend_from_slice(&(shard as u64).to_le_bytes());
        for value in output {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        Ok(bytes)
    }
    pub fn frame(&self, source: usize, destination: usize) -> Vec<u8> {
        assert!(
            source < self.devices.len()
                && destination < self.devices.len()
                && source != destination
        );
        vec![0xa0 ^ ((source as u8) << 3) ^ destination as u8; FRAME_BYTES]
    }
    pub fn check_route(
        &self,
        source: usize,
        destination: usize,
        payload: &[u8],
        source_bytes: &[u8],
        destination_bytes: &[u8],
    ) -> Result<(), String> {
        if payload.len() != TAG_BYTES + self.elements(source) * 4
            || &payload[..8] != b"F2NROST1"
            || payload[8..16] != self.devices[source].to_le_bytes()
            || payload[16..TAG_BYTES] != (source as u64).to_le_bytes()
            || payload[TAG_BYTES..]
                .chunks_exact(4)
                .enumerate()
                .any(|(index, word)| word != (index as u32).to_le_bytes())
            || source_bytes != payload
        {
            return Err("native peer source was mutated or truncated".into());
        }
        let mut expected = self.frame(source, destination);
        expected[GUARD_BYTES..GUARD_BYTES + payload.len()].copy_from_slice(payload);
        if expected != destination_bytes {
            return Err("native peer destination payload or canaries differ".into());
        }
        Ok(())
    }
}

pub struct Ledger {
    fills: [bool; MAX_DEVICES],
    routes: [[bool; MAX_DEVICES]; MAX_DEVICES],
    complete: usize,
    observations: Vec<(usize, usize, u64)>,
}
impl Ledger {
    pub fn new() -> Self {
        Self {
            fills: [false; MAX_DEVICES],
            routes: [[false; MAX_DEVICES]; MAX_DEVICES],
            complete: 0,
            observations: Vec::with_capacity(MAX_DEVICES * (MAX_DEVICES - 1)),
        }
    }
    pub fn fill(&mut self, case: &Case, shard: usize) -> Result<(), String> {
        if shard >= case.devices.len() || self.fills[shard] {
            return Err("duplicate or foreign native shard".into());
        }
        self.fills[shard] = true;
        Ok(())
    }
    pub fn route(
        &mut self,
        case: &Case,
        pair: (usize, usize),
        native_counter: u64,
    ) -> Result<(), String> {
        let (source, destination) = pair;
        if source >= case.devices.len()
            || destination >= case.devices.len()
            || source == destination
            || !self.fills[..case.devices.len()].iter().all(|x| *x)
            || self.routes[source][destination]
        {
            return Err("native route duplicated, foreign or precedes generated settlement".into());
        }
        let expected = match case.mode {
            Mode::NativeXgmi => (self.complete + 1) as u64,
            Mode::HostStaged => 0,
        };
        if native_counter != expected {
            return Err("native/staged route mechanism counter differs".into());
        }
        self.routes[source][destination] = true;
        self.observations
            .push((source, destination, native_counter));
        self.complete += 1;
        Ok(())
    }
    pub fn finish(&self, case: &Case, observed: &[(u64, u16)]) -> Result<String, String> {
        case.check_roster(observed)?;
        if !self.fills[..case.devices.len()].iter().all(|x| *x)
            || case.routes().any(|(s, d)| !self.routes[s][d])
        {
            return Err("native roster campaign is incomplete".into());
        }
        let devices = observed
            .iter()
            .map(|(uid, minor)| format!("{{\"uid\":\"0x{uid:016x}\",\"render_minor\":{minor}}}"))
            .collect::<Vec<_>>()
            .join(",");
        let outputs = (0..case.devices.len())
            .map(|i| case.elements(i).to_string())
            .collect::<Vec<_>>()
            .join(",");
        let pairs = self.observations.iter().map(|&(source, destination, counter)| {
            format!(
                "{{\"source\":\"0x{:016x}\",\"destination\":\"0x{:016x}\",\"payload_bytes\":{},\"native_peer_completions\":{counter}}}",
                case.devices[source], case.devices[destination], TAG_BYTES + case.elements(source) * 4,
            )
        }).collect::<Vec<_>>().join(",");
        Ok(format!(
            "\"devices\":[{devices}],\"target\":\"gfx942:xnack-\",\"outputs\":[{outputs}],\"transport\":\"{}\",\"settled_native_launches\":{},\"settled_directed_copies\":{},\"native_peer_completions\":{},\"selected_roster_complete\":true,\"copy_compute_order\":\"compute-then-serial-copy\",\"directed_pairs\":[{pairs}]",
            case.mode.token(),
            case.devices.len(),
            self.complete,
            if case.mode == Mode::NativeXgmi {
                self.complete
            } else {
                0
            }
        ))
    }
}

#[cfg(test)]
#[path = "native_roster_case/tests.rs"]
mod tests;
