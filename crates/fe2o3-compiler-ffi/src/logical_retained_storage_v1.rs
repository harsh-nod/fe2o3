//! Read-only actual-capacity observations for the closed FFI owners.
//!
//! Reports count a root header and all heap payload reachable through that
//! owner. Vec/String spare capacity is included. Arc allocation bookkeeping,
//! allocator rounding, transient pipeline owners, stack scratch and RSS are not.
//! Shared backing is charged in full once within each current owner, regardless
//! of selected range or strong count. The closed roots contain at most one Arc
//! backing slot; separately measured aliases MUST NOT simply be summed.
//!
//! For a field embedded in an already charged enclosing header, add only
//! heap_bytes and visited_items to the enclosing checked ledger. For an
//! independently stored root, include header_bytes once as well. Charge shared
//! backing once at the actual aggregate owner, not once per alias. None of these
//! observations changes a constructor, wire bound, identity, or authority gate.

use std::{error::Error, fmt, mem::size_of};

use crate::{CompilerFfiContractV1, CompilerFfiEnvelopeV1, CompilerFfiSourceOwnerV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerFfiLogicalStorageLimitsV1 {
    /// Observation target only; None imposes no byte target.
    pub max_bytes: Option<usize>,
    /// Bounds owner and collection visits before subsequent traversal.
    pub max_items: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerFfiLogicalStorageErrorV1 {
    Arithmetic,
    ByteLimit,
    ItemLimit,
}
impl fmt::Display for CompilerFfiLogicalStorageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FFI logical retained-storage observation refused: {self:?}"
        )
    }
}
impl Error for CompilerFfiLogicalStorageErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerFfiLogicalStorageV1 {
    header_bytes: usize,
    heap_bytes: usize,
    total_bytes: usize,
    visited_items: usize,
}
impl CompilerFfiLogicalStorageV1 {
    pub const fn header_bytes(self) -> usize {
        self.header_bytes
    }
    pub const fn heap_bytes(self) -> usize {
        self.heap_bytes
    }
    pub const fn total_bytes(self) -> usize {
        self.total_bytes
    }
    pub const fn visited_items(self) -> usize {
        self.visited_items
    }
}

pub(crate) type StorageResult = Result<(), CompilerFfiLogicalStorageErrorV1>;

/// Owner-local arithmetic, deliberately not an upward KIR dependency.
pub(crate) struct Counter {
    limits: CompilerFfiLogicalStorageLimitsV1,
    value: CompilerFfiLogicalStorageV1,
}
impl Counter {
    fn new<T>(
        limits: CompilerFfiLogicalStorageLimitsV1,
    ) -> Result<Self, CompilerFfiLogicalStorageErrorV1> {
        let mut counter = Self {
            limits,
            value: CompilerFfiLogicalStorageV1 {
                header_bytes: size_of::<T>(),
                heap_bytes: 0,
                total_bytes: size_of::<T>(),
                visited_items: 0,
            },
        };
        counter.charge(0, 0)?;
        Ok(counter)
    }
    fn charge(&mut self, bytes: usize, items: usize) -> StorageResult {
        let heap = self
            .value
            .heap_bytes
            .checked_add(bytes)
            .ok_or(CompilerFfiLogicalStorageErrorV1::Arithmetic)?;
        let total = self
            .value
            .header_bytes
            .checked_add(heap)
            .ok_or(CompilerFfiLogicalStorageErrorV1::Arithmetic)?;
        let visits = self
            .value
            .visited_items
            .checked_add(items)
            .ok_or(CompilerFfiLogicalStorageErrorV1::Arithmetic)?;
        if self.limits.max_bytes.is_some_and(|limit| total > limit) {
            return Err(CompilerFfiLogicalStorageErrorV1::ByteLimit);
        }
        if visits > self.limits.max_items {
            return Err(CompilerFfiLogicalStorageErrorV1::ItemLimit);
        }
        self.value.heap_bytes = heap;
        self.value.total_bytes = total;
        self.value.visited_items = visits;
        Ok(())
    }
    pub(crate) fn extent(&mut self, count: usize, width: usize) -> StorageResult {
        let bytes = count
            .checked_mul(width)
            .ok_or(CompilerFfiLogicalStorageErrorV1::Arithmetic)?;
        self.charge(bytes, 1)
    }
    pub(crate) fn owner(&mut self) -> StorageResult {
        self.charge(0, 1)
    }
    pub(crate) fn array<T>(&mut self, count: usize) -> StorageResult {
        self.extent(count, size_of::<T>())
    }
    pub(crate) fn vector<T>(&mut self, value: &Vec<T>) -> StorageResult {
        self.array::<T>(value.capacity())
    }
    pub(crate) fn string(&mut self, value: &String) -> StorageResult {
        self.array::<u8>(value.capacity())
    }
}
pub(crate) fn observe<T>(
    value: &T,
    limits: CompilerFfiLogicalStorageLimitsV1,
    walk: fn(&T, &mut Counter) -> StorageResult,
) -> Result<CompilerFfiLogicalStorageV1, CompilerFfiLogicalStorageErrorV1> {
    let mut counter = Counter::new::<T>(limits)?;
    walk(value, &mut counter)?;
    Ok(counter.value)
}

/// Copy leaves cannot own an independently dropped heap allocation.
pub(crate) fn inline<T: Copy>(_: &T) {}

impl CompilerFfiEnvelopeV1 {
    /// Complete standalone logical report; use heap_bytes when embedded.
    pub fn logical_retained_storage_v1(
        &self,
        limits: CompilerFfiLogicalStorageLimitsV1,
    ) -> Result<CompilerFfiLogicalStorageV1, CompilerFfiLogicalStorageErrorV1> {
        observe(self, limits, Self::charge_logical_heap_v1)
    }

    pub(crate) fn charge_logical_heap_v1(&self, counter: &mut Counter) -> StorageResult {
        counter.owner()?;
        let Self {
            target,
            code_object_version,
            contracts,
            canonical_bytes,
            identity,
            inspection,
        } = self;
        inline(target);
        inline(code_object_version);
        inline(identity);
        inline(inspection);
        counter.vector(contracts)?;
        counter.vector(canonical_bytes)?;
        for contract in contracts {
            counter.owner()?;
            let CompilerFfiContractV1 {
                contract_identity,
                direction,
                link_role,
                target,
                code_object_version,
                source_owner,
                symbol,
                physical_abi,
                effects,
                effect_abi_identity,
                semantic_identity,
            } = contract;
            inline(contract_identity);
            inline(direction);
            inline(link_role);
            inline(target);
            inline(code_object_version);
            inline(effect_abi_identity);
            inline(semantic_identity);
            counter.owner()?;
            let CompilerFfiSourceOwnerV1 {
                crate_label,
                item_path,
                def_path_hash,
                concrete_instance_symbol,
                identity,
            } = source_owner;
            inline(def_path_hash);
            inline(identity);
            counter.string(crate_label)?;
            counter.string(item_path)?;
            counter.string(concrete_instance_symbol)?;
            counter.string(symbol)?;
            counter.string(physical_abi)?;
            counter.string(effects)?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::*;
    use reserved_fe2o3_symbols::{
        DEVICE_FFI_DIRECTION_EXPORT_V1, DeviceFfiContractFieldsV1, derive_device_ffi_contract_id_v1,
    };

    pub(crate) fn limits() -> CompilerFfiLogicalStorageLimitsV1 {
        CompilerFfiLogicalStorageLimitsV1 {
            max_bytes: None,
            max_items: 100_000,
        }
    }
    pub(crate) fn envelope() -> CompilerFfiEnvelopeV1 {
        let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
        let abi = "C(u32[size=4,align=4])->u32[size=4,align=4]";
        let semantic = [0x22_u8; 32];
        let id = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: DEVICE_FFI_DIRECTION_EXPORT_V1,
            symbol: "helper",
            calling_convention: "C",
            code_object_version: 5,
            target: "gfx942:xnack-",
            physical_abi: abi,
            effects: "none",
            semantic_identity: &"22".repeat(32),
        });
        let contract = CompilerFfiContractV1::new(
            id,
            DeviceFfiDirectionV1::Export,
            CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
            target,
            CodeObjectVersion::V5,
            CompilerFfiSourceOwnerV1::new("crate", "crate::helper", [3; 16], "_Rhelper").unwrap(),
            "helper",
            abi,
            "none",
            semantic,
        )
        .unwrap();
        let mut builder =
            CompilerFfiEnvelopeBuilderV1::new(target, CodeObjectVersion::V5, 1).unwrap();
        builder.push(contract).unwrap();
        builder.finish().unwrap()
    }

    pub(crate) fn descriptor_table() -> fe2o3_kernel_descriptor::DeviceDescriptorTableV1 {
        use fe2o3_kernel_descriptor::*;
        let evidence = BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([1; 32]),
            EvidenceDigest::from_sha256_bytes([2; 32]),
        );
        let kernel = KernelDescriptorV1::new(
            KernelId::from_bytes([3; 32]),
            ValidName::new("logical").unwrap(),
            ValidName::new("entry").unwrap(),
            ValidName::new("entry.kd").unwrap(),
            evidence,
            evidence,
            Vec::with_capacity(7),
            KernelAbiLayoutV1::new(0, 0, 1).unwrap(),
            LaunchConstraintsV1::new(
                1,
                BlockSizeV1::Any,
                DimensionsV1::new(1, 1, 1).unwrap(),
                64,
                0,
                0,
            )
            .unwrap(),
            Vec::with_capacity(11),
        )
        .unwrap();
        let mut kernels = Vec::with_capacity(3);
        kernels.push(kernel);
        DeviceDescriptorTableV1::new(
            CanonicalCodeObjectDigest::from_bytes([0; 32]),
            CodeObjectVersion::V5,
            CompilerIdentityV1::new(
                Text::new("compiler").unwrap(),
                Text::new("release").unwrap(),
                [4; 20],
            ),
            ProducerIdentityV1::new(
                Text::new("producer").unwrap(),
                Text::new("version").unwrap(),
            ),
            DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            Vec::with_capacity(5),
            Vec::with_capacity(13),
            kernels,
        )
        .unwrap()
    }

    #[test]
    fn empty_envelope_counts_canonical_and_spare_empty_contract_capacity() {
        let mut value = CompilerFfiEnvelopeV1::for_module_without_device_ffi(
            DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            CodeObjectVersion::V5,
        )
        .unwrap();
        value.contracts.reserve(7);
        value.canonical_bytes.reserve(19);
        let identity = value.identity();
        let bytes = value.canonical_bytes().to_vec();
        let r = value.logical_retained_storage_v1(limits()).unwrap();
        assert_eq!(r.header_bytes(), size_of::<CompilerFfiEnvelopeV1>());
        assert_eq!(
            r.heap_bytes(),
            value.contracts.capacity() * size_of::<CompilerFfiContractV1>()
                + value.canonical_bytes.capacity()
        );
        assert_eq!(r.total_bytes(), r.header_bytes() + r.heap_bytes());
        assert_eq!(value.identity(), identity);
        assert_eq!(value.canonical_bytes(), bytes);
        assert!(!value.grants_link_authority());
    }

    #[test]
    fn envelope_walk_covers_all_six_owned_strings_and_both_vectors() {
        let mut value = envelope();
        value.contracts.reserve(11);
        value.canonical_bytes.reserve(23);
        let c = &mut value.contracts[0];
        c.source_owner.crate_label.reserve(17);
        c.source_owner.item_path.reserve(19);
        c.source_owner.concrete_instance_symbol.reserve(29);
        c.symbol.reserve(31);
        c.physical_abi.reserve(37);
        c.effects.reserve(41);
        let c = &value.contracts[0];
        let expected = value.contracts.capacity() * size_of::<CompilerFfiContractV1>()
            + value.canonical_bytes.capacity()
            + c.source_owner.crate_label.capacity()
            + c.source_owner.item_path.capacity()
            + c.source_owner.concrete_instance_symbol.capacity()
            + c.symbol.capacity()
            + c.physical_abi.capacity()
            + c.effects.capacity();
        let before = value.clone();
        let r = value.logical_retained_storage_v1(limits()).unwrap();
        assert_eq!(r.heap_bytes(), expected);
        assert_eq!(value, before);
        assert_eq!(
            value.logical_retained_storage_v1(CompilerFfiLogicalStorageLimitsV1 {
                max_bytes: Some(r.total_bytes()),
                max_items: r.visited_items(),
            }),
            Ok(r)
        );
        assert_eq!(
            value.logical_retained_storage_v1(CompilerFfiLogicalStorageLimitsV1 {
                max_bytes: Some(r.total_bytes() - 1),
                max_items: r.visited_items(),
            }),
            Err(CompilerFfiLogicalStorageErrorV1::ByteLimit)
        );
        for max_items in 0..r.visited_items() {
            assert_eq!(
                value.logical_retained_storage_v1(CompilerFfiLogicalStorageLimitsV1 {
                    max_bytes: None,
                    max_items,
                }),
                Err(CompilerFfiLogicalStorageErrorV1::ItemLimit)
            );
        }
    }

    #[test]
    fn arithmetic_and_limit_refusals_do_not_commit_partial_charges() {
        let mut c = Counter::new::<()>(limits()).unwrap();
        assert_eq!(
            c.extent(usize::MAX, 2),
            Err(CompilerFfiLogicalStorageErrorV1::Arithmetic)
        );
        assert_eq!((c.value.heap_bytes, c.value.visited_items), (0, 0));
        c.limits.max_items = usize::MAX;
        c.charge(usize::MAX, usize::MAX).unwrap();
        let before = c.value;
        assert_eq!(
            c.charge(1, 0),
            Err(CompilerFfiLogicalStorageErrorV1::Arithmetic)
        );
        assert_eq!(
            c.charge(0, 1),
            Err(CompilerFfiLogicalStorageErrorV1::Arithmetic)
        );
        assert_eq!(c.value, before);
        let mut header = Counter::new::<u8>(limits()).unwrap();
        assert_eq!(
            header.charge(usize::MAX, 0),
            Err(CompilerFfiLogicalStorageErrorV1::Arithmetic)
        );
        assert_eq!((header.value.heap_bytes, header.value.total_bytes), (0, 1));
        let mut c = Counter::new::<u8>(CompilerFfiLogicalStorageLimitsV1 {
            max_bytes: Some(2),
            max_items: 1,
        })
        .unwrap();
        c.charge(1, 1).unwrap();
        let before = c.value;
        assert_eq!(
            c.charge(1, 0),
            Err(CompilerFfiLogicalStorageErrorV1::ByteLimit)
        );
        assert_eq!(
            c.charge(0, 1),
            Err(CompilerFfiLogicalStorageErrorV1::ItemLimit)
        );
        assert_eq!(c.value, before);
        assert!(matches!(
            Counter::new::<u8>(CompilerFfiLogicalStorageLimitsV1 {
                max_bytes: Some(0),
                max_items: 1,
            }),
            Err(CompilerFfiLogicalStorageErrorV1::ByteLimit)
        ));
    }
}
