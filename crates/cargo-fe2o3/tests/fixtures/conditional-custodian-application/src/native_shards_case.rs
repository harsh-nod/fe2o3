//! Bounded report oracle only. These values grant no native or graph authority.
use std::ffi::OsString;

pub const MAX_DEVICES: usize = 8;

#[derive(Debug, Eq, PartialEq)]
pub struct Case {
    pub producer_source: String,
    pub devices: Vec<u64>,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Case, String> {
    let mut args = args.into_iter();
    let mut next = || {
        args.next()
            .ok_or("incomplete native shard arguments")?
            .into_string()
            .map_err(|_| "native shard arguments must be UTF-8")
    };
    if next()? != "--native-v5-shards" || next()? != "--producer-source" {
        return Err("explicit native shard selector and producer source required".into());
    }
    let producer_source = next()?;
    if producer_source.is_empty()
        || producer_source.len() > 4096
        || producer_source.as_bytes().contains(&0)
        || next()? != "--devices"
    {
        return Err("bounded producer source and explicit device roster required".into());
    }
    let mut devices = Vec::with_capacity(MAX_DEVICES);
    for arg in args {
        if devices.len() == MAX_DEVICES {
            return Err("native shards exceed eight devices".into());
        }
        let digits = arg
            .to_str()
            .and_then(|text| text.strip_prefix("0x"))
            .filter(|digits| {
                digits.len() == 16
                    && digits
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            })
            .ok_or("UID requires exactly sixteen lowercase hexadecimal digits")?;
        let uid = u64::from_str_radix(digits, 16).map_err(|error| error.to_string())?;
        if uid == 0 || devices.contains(&uid) {
            return Err("distinct nonzero UIDs required".into());
        }
        devices.push(uid);
    }
    if devices.len() < 2 {
        return Err("two through eight admitted devices required".into());
    }
    Ok(Case {
        producer_source,
        devices,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Succeeded,
    RejectedBeforePublication,
    DeviceUnavailableBeforeActivation,
    ReadbackFailed,
}

impl State {
    fn token(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::RejectedBeforePublication => "rejected-before-publication",
            Self::DeviceUnavailableBeforeActivation => "device-unavailable-before-activation",
            Self::ReadbackFailed => "readback-failed-after-native-settlement",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Shard {
    pub uid: u64,
    pub render_minor: u16,
    pub node: u32,
    pub elements: usize,
    pub state: State,
    pub checked_values: usize,
}

impl Case {
    pub fn elements(&self, shard: usize) -> usize {
        assert!(shard < self.devices.len());
        32 + shard
    }

    pub fn check_roster(&self, observed: &[(u64, u16)]) -> Result<(), String> {
        if !(2..=MAX_DEVICES).contains(&self.devices.len())
            || self
                .devices
                .iter()
                .enumerate()
                .any(|(index, &uid)| uid == 0 || self.devices[..index].contains(&uid))
            || observed.len() != self.devices.len()
            || observed.iter().enumerate().any(|(index, &(uid, minor))| {
                uid != self.devices[index] || observed[..index].iter().any(|&(_, old)| old == minor)
            })
        {
            return Err("exact original admitted UID/render roster differs".into());
        }
        Ok(())
    }

    pub fn check_fill(&self, shard: usize, output: &[u32]) -> Result<(), String> {
        if shard >= self.devices.len()
            || shard >= MAX_DEVICES
            || output.len() != self.elements(shard)
            || output
                .iter()
                .enumerate()
                .any(|(index, &value)| value != index as u32)
        {
            return Err("native shard output extent or values differ".into());
        }
        Ok(())
    }

    pub fn report(
        &self,
        observed: &[(u64, u16)],
        shards: &[Shard],
        local_failure: bool,
    ) -> Result<String, String> {
        self.check_roster(observed)?;
        if shards.len() != observed.len()
            || shards.iter().enumerate().any(|(index, shard)| {
                (shard.uid, shard.render_minor) != observed[index]
                    || shard.node != index as u32 + 1
                    || shard.elements != self.elements(index)
                    || shard.checked_values
                        != if shard.state == State::Succeeded {
                            shard.elements
                        } else {
                            0
                        }
            })
            || local_failure != shards.iter().any(|shard| shard.state != State::Succeeded)
        {
            return Err("partial report lacks exact settled shard coverage".into());
        }
        let successful = shards
            .iter()
            .filter(|shard| shard.state == State::Succeeded)
            .count();
        let rows = shards.iter().map(|shard| format!(
            "{{\"uid\":\"0x{:016x}\",\"render_minor\":{},\"node\":{},\"elements\":{},\"state\":\"{}\",\"checked_values\":{}}}",
            shard.uid, shard.render_minor, shard.node, shard.elements, shard.state.token(), shard.checked_values,
        )).collect::<Vec<_>>().join(",");
        Ok(format!(
            "\"target\":\"gfx942:xnack-\",\"coverage\":\"all-admitted-context\",\"selected_roster_complete\":true,\"admitted_devices\":{},\"shards\":[{rows}],\"successful_shards\":{successful},\"failed_shards\":{},\"settled_local_failure\":{local_failure},\"graph_retired\":true,\"copied_bytes\":0,\"direct_native_data_transfer\":false",
            shards.len(),
            shards.len() - successful,
        ))
    }
}

#[cfg(test)]
#[path = "native_shards_case/tests.rs"]
mod tests;
