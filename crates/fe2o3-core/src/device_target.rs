use crate::{Error, Result, check};
use fe2o3_amd_target::AmdTargetId;
use fe2o3_hip_sys::{
    Fe2o3HipDeviceProperties, HIP_DEVICE_ARCH_HAS_GLOBAL_INT32_ATOMICS,
    HIP_DEVICE_ARCH_HAS_GLOBAL_INT64_ATOMICS, HIP_DEVICE_ARCH_HAS_SHARED_INT32_ATOMICS,
    HIP_DEVICE_ARCH_HAS_SHARED_INT64_ATOMICS, HIP_DEVICE_ARCH_HAS_WARP_BALLOT,
    HIP_DEVICE_ARCH_HAS_WARP_SHUFFLE, HIP_DEVICE_ARCH_HAS_WARP_VOTE,
};

const KNOWN_ARCHITECTURE_FEATURES: u64 = HIP_DEVICE_ARCH_HAS_GLOBAL_INT32_ATOMICS
    | HIP_DEVICE_ARCH_HAS_SHARED_INT32_ATOMICS
    | HIP_DEVICE_ARCH_HAS_GLOBAL_INT64_ATOMICS
    | HIP_DEVICE_ARCH_HAS_SHARED_INT64_ATOMICS
    | HIP_DEVICE_ARCH_HAS_WARP_VOTE
    | HIP_DEVICE_ARCH_HAS_WARP_BALLOT
    | HIP_DEVICE_ARCH_HAS_WARP_SHUFFLE;

/// Device identity, launch limits, and capability facts observed from HIP.
///
/// Fields are private so parsed text or caller-provided limits cannot forge a
/// runtime observation. Production values are created only while constructing
/// a [`crate::GpuContext`]. This value says nothing about a code object's
/// embedded target or ABI and therefore does not authorize loading or launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedDeviceTarget {
    device_id: i32,
    target_id: AmdTargetId,
    warp_size: u32,
    max_threads_per_block: u32,
    max_block_dimensions: [u32; 3],
    max_grid_dimensions: [u32; 3],
    shared_memory_per_block: u64,
    shared_memory_per_block_optin: Option<u64>,
    architecture_features: u64,
}

impl ObservedDeviceTarget {
    pub const fn device_id(&self) -> i32 {
        self.device_id
    }

    pub const fn target_id(&self) -> AmdTargetId {
        self.target_id
    }

    pub const fn warp_size(&self) -> u32 {
        self.warp_size
    }

    pub const fn max_threads_per_block(&self) -> u32 {
        self.max_threads_per_block
    }

    pub const fn max_block_dimensions(&self) -> [u32; 3] {
        self.max_block_dimensions
    }

    pub const fn max_grid_dimensions(&self) -> [u32; 3] {
        self.max_grid_dimensions
    }

    pub const fn shared_memory_per_block(&self) -> u64 {
        self.shared_memory_per_block
    }

    /// Returns the opt-in shared-memory limit when this HIP version reports it.
    pub const fn shared_memory_per_block_optin(&self) -> Option<u64> {
        self.shared_memory_per_block_optin
    }

    pub const fn has_global_int32_atomics(&self) -> bool {
        self.has_feature(HIP_DEVICE_ARCH_HAS_GLOBAL_INT32_ATOMICS)
    }

    pub const fn has_shared_int32_atomics(&self) -> bool {
        self.has_feature(HIP_DEVICE_ARCH_HAS_SHARED_INT32_ATOMICS)
    }

    pub const fn has_global_int64_atomics(&self) -> bool {
        self.has_feature(HIP_DEVICE_ARCH_HAS_GLOBAL_INT64_ATOMICS)
    }

    pub const fn has_shared_int64_atomics(&self) -> bool {
        self.has_feature(HIP_DEVICE_ARCH_HAS_SHARED_INT64_ATOMICS)
    }

    pub const fn has_warp_vote(&self) -> bool {
        self.has_feature(HIP_DEVICE_ARCH_HAS_WARP_VOTE)
    }

    pub const fn has_warp_ballot(&self) -> bool {
        self.has_feature(HIP_DEVICE_ARCH_HAS_WARP_BALLOT)
    }

    pub const fn has_warp_shuffle(&self) -> bool {
        self.has_feature(HIP_DEVICE_ARCH_HAS_WARP_SHUFFLE)
    }

    /// The current coarse atomics capability requires all global/shared 32/64-bit facts.
    pub const fn has_complete_integer_atomics(&self) -> bool {
        self.has_global_int32_atomics()
            && self.has_shared_int32_atomics()
            && self.has_global_int64_atomics()
            && self.has_shared_int64_atomics()
    }

    pub(crate) fn query_hip(device_id: i32) -> Result<Self> {
        Self::query(device_id, &HipDevicePropertyQuery)
    }

    fn query(device_id: i32, query: &impl DevicePropertyQuery) -> Result<Self> {
        Self::from_properties(device_id, query.properties(device_id)?)
    }

    fn from_properties(device_id: i32, properties: Fe2o3HipDeviceProperties) -> Result<Self> {
        if properties.architecture_features & !KNOWN_ARCHITECTURE_FEATURES != 0 {
            return Err(Error::InvalidDeviceProperties(
                "architecture feature bits contain an unknown value",
            ));
        }

        let target_id = parse_target_id(&properties.gcn_arch_name)?;
        let warp_size = positive_u32(properties.warp_size, "warp size must be positive")?;
        if !matches!(warp_size, 32 | 64) {
            return Err(Error::InvalidDeviceProperties("warp size must be 32 or 64"));
        }

        let max_threads_per_block = positive_u32(
            properties.max_threads_per_block,
            "maximum threads per block must be positive",
        )?;
        let max_block_dimensions = positive_dimensions(
            properties.max_block_dim,
            "maximum block dimensions must be positive",
        )?;
        if warp_size > max_threads_per_block
            || max_block_dimensions
                .iter()
                .any(|&dimension| dimension > max_threads_per_block)
        {
            return Err(Error::InvalidDeviceProperties(
                "warp or block dimension exceeds maximum threads per block",
            ));
        }
        let max_grid_dimensions = positive_dimensions(
            properties.max_grid_dim,
            "maximum grid dimensions must be positive",
        )?;
        if properties.shared_mem_per_block == 0 {
            return Err(Error::InvalidDeviceProperties(
                "shared memory per block must be positive",
            ));
        }
        let shared_memory_per_block_optin = match properties.shared_mem_per_block_optin {
            0 => None,
            value if value >= properties.shared_mem_per_block => Some(value),
            _ => {
                return Err(Error::InvalidDeviceProperties(
                    "opt-in shared memory is smaller than the default limit",
                ));
            }
        };

        Ok(Self {
            device_id,
            target_id,
            warp_size,
            max_threads_per_block,
            max_block_dimensions,
            max_grid_dimensions,
            shared_memory_per_block: properties.shared_mem_per_block,
            shared_memory_per_block_optin,
            architecture_features: properties.architecture_features,
        })
    }

    const fn has_feature(&self, feature: u64) -> bool {
        self.architecture_features & feature != 0
    }
}

trait DevicePropertyQuery {
    fn properties(&self, device_id: i32) -> Result<Fe2o3HipDeviceProperties>;
}

struct HipDevicePropertyQuery;

impl DevicePropertyQuery for HipDevicePropertyQuery {
    fn properties(&self, device_id: i32) -> Result<Fe2o3HipDeviceProperties> {
        let mut properties = Fe2o3HipDeviceProperties::default();
        // SAFETY: `properties` is a live writable value and the context setup
        // validated and selected `device_id` before this query.
        check(unsafe { fe2o3_hip_sys::fe2o3HipGetDeviceProperties(device_id, &mut properties) })?;
        Ok(properties)
    }
}

fn parse_target_id(name: &[core::ffi::c_char; 256]) -> Result<AmdTargetId> {
    let nul = name
        .iter()
        .position(|&byte| byte == 0)
        .ok_or(Error::InvalidDeviceProperties(
            "GCN architecture name is not NUL-terminated",
        ))?;
    let bytes: Vec<u8> = name[..nul].iter().map(|&byte| byte as u8).collect();
    let text = core::str::from_utf8(&bytes)
        .map_err(|_| Error::InvalidDeviceProperties("GCN architecture name is not valid UTF-8"))?;
    AmdTargetId::parse(text).map_err(Error::InvalidDeviceTarget)
}

fn positive_u32(value: i32, reason: &'static str) -> Result<u32> {
    u32::try_from(value)
        .ok()
        .filter(|&value| value != 0)
        .ok_or(Error::InvalidDeviceProperties(reason))
}

fn positive_dimensions(values: [i32; 3], reason: &'static str) -> Result<[u32; 3]> {
    Ok([
        positive_u32(values[0], reason)?,
        positive_u32(values[1], reason)?,
        positive_u32(values[2], reason)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HipError;

    struct FakeQuery(Result<Fe2o3HipDeviceProperties>);

    impl DevicePropertyQuery for FakeQuery {
        fn properties(&self, _device_id: i32) -> Result<Fe2o3HipDeviceProperties> {
            match &self.0 {
                Ok(properties) => Ok(*properties),
                Err(Error::Hip(error)) => Err(Error::Hip(*error)),
                Err(_) => unreachable!("fake query errors are limited to HIP errors"),
            }
        }
    }

    #[test]
    fn validated_observation_preserves_identity_limits_and_capabilities() {
        let target = ObservedDeviceTarget::query(3, &FakeQuery(Ok(valid_properties()))).unwrap();

        assert_eq!(target.device_id(), 3);
        assert_eq!(target.target_id().to_string(), "gfx942:sramecc+:xnack-");
        assert_eq!(target.warp_size(), 64);
        assert_eq!(target.max_threads_per_block(), 1024);
        assert_eq!(target.max_block_dimensions(), [1024, 1024, 1024]);
        assert_eq!(
            target.max_grid_dimensions(),
            [i32::MAX as u32, 65536, 65536]
        );
        assert_eq!(target.shared_memory_per_block(), 65_536);
        assert_eq!(target.shared_memory_per_block_optin(), Some(131_072));
        assert!(target.has_complete_integer_atomics());
        assert!(target.has_warp_vote());
        assert!(target.has_warp_ballot());
        assert!(target.has_warp_shuffle());
    }

    #[test]
    fn missing_optin_limit_is_explicitly_unknown() {
        let mut properties = valid_properties();
        properties.shared_mem_per_block_optin = 0;
        let target = ObservedDeviceTarget::from_properties(0, properties).unwrap();
        assert_eq!(target.shared_memory_per_block_optin(), None);
    }

    #[test]
    fn malformed_target_names_fail_closed() {
        let mut missing_nul = valid_properties();
        missing_nul.gcn_arch_name.fill(b'x' as core::ffi::c_char);
        assert!(matches!(
            ObservedDeviceTarget::from_properties(0, missing_nul),
            Err(Error::InvalidDeviceProperties(_))
        ));

        let mut invalid_utf8 = valid_properties();
        invalid_utf8.gcn_arch_name = [0; 256];
        invalid_utf8.gcn_arch_name[0] = -1;
        assert!(matches!(
            ObservedDeviceTarget::from_properties(0, invalid_utf8),
            Err(Error::InvalidDeviceProperties(_))
        ));

        let mut invalid_target = valid_properties();
        set_architecture_name(&mut invalid_target, "gfx600:xnack+");
        assert!(matches!(
            ObservedDeviceTarget::from_properties(0, invalid_target),
            Err(Error::InvalidDeviceTarget(_))
        ));
    }

    #[test]
    fn invalid_limits_and_unknown_features_fail_closed() {
        for mutate in [
            |properties: &mut Fe2o3HipDeviceProperties| properties.warp_size = 48,
            |properties: &mut Fe2o3HipDeviceProperties| properties.max_threads_per_block = 0,
            |properties: &mut Fe2o3HipDeviceProperties| properties.max_block_dim[1] = -1,
            |properties: &mut Fe2o3HipDeviceProperties| properties.max_grid_dim[2] = 0,
        ] {
            let mut properties = valid_properties();
            mutate(&mut properties);
            assert!(matches!(
                ObservedDeviceTarget::from_properties(0, properties),
                Err(Error::InvalidDeviceProperties(_))
            ));
        }

        let mut no_shared_memory = valid_properties();
        no_shared_memory.shared_mem_per_block = 0;
        assert!(ObservedDeviceTarget::from_properties(0, no_shared_memory).is_err());

        let mut bad_optin = valid_properties();
        bad_optin.shared_mem_per_block_optin = bad_optin.shared_mem_per_block - 1;
        assert!(ObservedDeviceTarget::from_properties(0, bad_optin).is_err());

        let mut unknown_feature = valid_properties();
        unknown_feature.architecture_features |= 1 << 63;
        assert!(ObservedDeviceTarget::from_properties(0, unknown_feature).is_err());

        let mut inconsistent_block_limit = valid_properties();
        inconsistent_block_limit.max_block_dim[0] = 2048;
        assert!(ObservedDeviceTarget::from_properties(0, inconsistent_block_limit).is_err());
    }

    #[test]
    fn query_errors_do_not_create_observations() {
        let error = ObservedDeviceTarget::query(0, &FakeQuery(Err(Error::Hip(HipError::new(801)))))
            .unwrap_err();
        assert!(matches!(error, Error::Hip(error) if error.code() == 801));
    }

    #[test]
    #[ignore = "requires a working HIP device"]
    fn context_observes_a_real_hip_device() {
        let context = crate::GpuContext::new(0).unwrap();
        let target = context.observed_target();

        assert_eq!(target.device_id(), 0);
        assert!(target.target_id().processor().starts_with("gfx"));
        assert!(matches!(target.warp_size(), 32 | 64));
        assert!(target.max_threads_per_block() > 0);
        assert!(
            target
                .max_block_dimensions()
                .into_iter()
                .all(|value| value > 0)
        );
        assert!(
            target
                .max_grid_dimensions()
                .into_iter()
                .all(|value| value > 0)
        );
        assert!(target.shared_memory_per_block() > 0);
    }

    fn valid_properties() -> Fe2o3HipDeviceProperties {
        let mut properties = Fe2o3HipDeviceProperties {
            warp_size: 64,
            max_threads_per_block: 1024,
            max_block_dim: [1024, 1024, 1024],
            max_grid_dim: [i32::MAX, 65_536, 65_536],
            shared_mem_per_block: 65_536,
            shared_mem_per_block_optin: 131_072,
            architecture_features: KNOWN_ARCHITECTURE_FEATURES,
            ..Fe2o3HipDeviceProperties::default()
        };
        set_architecture_name(&mut properties, "gfx942:sramecc+:xnack-");
        properties
    }

    fn set_architecture_name(properties: &mut Fe2o3HipDeviceProperties, name: &str) {
        properties.gcn_arch_name = [0; 256];
        for (output, input) in properties.gcn_arch_name.iter_mut().zip(name.bytes()) {
            *output = input as core::ffi::c_char;
        }
    }
}
