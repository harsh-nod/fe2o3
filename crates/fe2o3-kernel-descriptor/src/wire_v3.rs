//! Bounded borrowed V3 framing. No heap allocation or old owned-table decode.

use crate::decode::*;
use crate::model::{self, MAX_CAPABILITIES, PhysicalAbiComponentV1};
use crate::nominal_v3::*;
use crate::*;
use fe2o3_amd_target::{AmdTargetCapabilities, AmdTargetId};
use std::fmt::{self, Write};

type ResultV3<T, E> = Result<T, DescriptorWireErrorV3<E>>;

use crate::wire_common::Reader;
pub(crate) fn count<E>(n: usize, field: &'static str, max: usize) -> ResultV3<(), E> {
    if n > max {
        return Err(DecodeError::CountOutOfRange {
            field,
            count: n as u64,
            max,
        }
        .into());
    }
    Ok(())
}
fn invalid<E>(field: &'static str) -> DescriptorWireErrorV3<E> {
    ValidationError::InvalidValue { field }.into()
}
fn order<E, T: Ord>(previous: Option<T>, next: T, field: &'static str) -> ResultV3<(), E> {
    if let Some(previous) = previous {
        if previous == next {
            return Err(ValidationError::Duplicate { field }.into());
        }
        if previous > next {
            return Err(ValidationError::NonCanonicalOrder { field }.into());
        }
    }
    Ok(())
}

struct CompareText<'a> {
    remaining: &'a str,
}
impl Write for CompareText<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.remaining = self.remaining.strip_prefix(s).ok_or(fmt::Error)?;
        Ok(())
    }
}

fn target<E>(
    text: &str,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<DeviceTargetV1, E> {
    pay(c, 3 * text.len() + 1)?;
    let value = AmdTargetId::parse(text).map_err(|_| invalid("device target"))?;
    let mut compare = CompareText { remaining: text };
    if write!(&mut compare, "{value}").is_err() || !compare.remaining.is_empty() {
        return Err(ValidationError::NonCanonicalOrder {
            field: "device target features",
        }
        .into());
    }
    Ok(DeviceTargetV1::new(value))
}

fn source_row<E>(
    bytes: &[u8],
    start: usize,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<SourceTypeRecordV3, E> {
    let mut r = Reader::at(bytes, start);
    let identity = RustTypeIdentity::from_bytes(r.fixed(c)?);
    let tag = r.u8(c)?;
    let scalar = r.u8(c)?;
    r.zero("source type", c)?;
    let descriptor = match tag {
        1 => SourceTypeDescriptorV3::Scalar(parse_scalar(scalar)?),
        2 => SourceTypeDescriptorV3::SharedSlice(parse_scalar(scalar)?),
        3 => SourceTypeDescriptorV3::DisjointSlice(parse_scalar(scalar)?),
        4 => SourceTypeDescriptorV3::GlobalMutPointer(parse_scalar(scalar)?),
        5 | 6 => {
            if scalar != 0 {
                return Err(DecodeError::NonzeroReserved {
                    field: "nominal scalar",
                }
                .into());
            }
            if tag == 5 {
                SourceTypeDescriptorV3::Usize
            } else {
                SourceTypeDescriptorV3::Isize
            }
        }
        _ => {
            return Err(DecodeError::UnknownTag {
                kind: "source type V3",
                tag: u16::from(tag),
            }
            .into());
        }
    };
    let actual = SourceTypeRecordV3::new(descriptor, c)?;
    if actual.identity() != identity {
        return Err(ValidationError::IdentityMismatch { field: "Rust type" }.into());
    }
    Ok(actual)
}

fn layout_row<E>(
    bytes: &[u8],
    start: usize,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<DeviceLayoutRecordV1, E> {
    let mut r = Reader::at(bytes, start);
    let identity = DeviceLayoutIdentity::from_bytes(r.fixed(c)?);
    let descriptor = DeviceLayoutDescriptorV1 {
        kind: parse_descriptor_kind(r.u8(c)?)?,
        element: parse_scalar(r.u8(c)?)?,
        size: r.u16(c)?,
        alignment: r.u16(c)?,
        pointer_width: r.u8(c)?,
        length_width: r.u8(c)?,
    };
    r.zero("device layout", c)?;
    r.zero("device layout", c)?;
    let actual = device_layout_record_v3(descriptor, c)?;
    if actual.identity() != identity {
        return Err(ValidationError::IdentityMismatch {
            field: "device layout",
        }
        .into());
    }
    Ok(actual)
}

fn search<E>(
    bytes: &[u8],
    start: usize,
    n: usize,
    stride: usize,
    key: &[u8; 32],
    field: &'static str,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<usize, E> {
    let (mut lo, mut hi) = (0, n);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let mut r = Reader::at(bytes, start + mid * stride);
        let id = r.fixed::<32, E>(c)?;
        match id.cmp(key) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => return Ok(mid),
        }
    }
    Err(ValidationError::DanglingReference { field }.into())
}
pub(crate) fn lookup_source<E>(
    v: &DeviceDescriptorTableV3<'_>,
    id: RustTypeIdentity,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<SourceTypeRecordV3, E> {
    let i = search(
        v.bytes,
        v.types_start,
        v.type_count,
        36,
        id.as_bytes(),
        "Rust type",
        c,
    )?;
    source_row(v.bytes, v.types_start + i * 36, c)
}
pub(crate) fn lookup_layout<E>(
    v: &DeviceDescriptorTableV3<'_>,
    id: DeviceLayoutIdentity,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<DeviceLayoutRecordV1, E> {
    let i = search(
        v.bytes,
        v.layouts_start,
        v.layout_count,
        44,
        id.as_bytes(),
        "device layout",
        c,
    )?;
    layout_row(v.bytes, v.layouts_start + i * 44, c)
}

fn evidence<E>(
    r: &mut Reader<'_>,
    tag: u8,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<BuildEvidenceV1, E> {
    if r.fixed::<4, E>(c)? != [tag, 1, 1, 0] {
        return Err(invalid("build evidence header"));
    }
    Ok(BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes(r.fixed(c)?),
        EvidenceDigest::from_sha256_bytes(r.fixed(c)?),
    ))
}

pub(crate) fn kernel<'v, 'w, E>(
    v: &'v DeviceDescriptorTableV3<'w>,
    index: usize,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<KernelDescriptorRefV3<'v, 'w>, E> {
    pay(c, 1)?;
    if index >= v.kernel_count {
        return Err(DescriptorWireErrorV3::Index {
            field: "kernel",
            index,
            count: v.kernel_count,
        });
    }
    let mut r = Reader::at(v.bytes, v.kernel_offsets[index]);
    let id = KernelId::from_bytes(r.fixed(c)?);
    let logical_name = r.text("kernel logical name", true, c)?;
    let entry_name = r.text("kernel entry name", true, c)?;
    let descriptor_symbol = r.text("kernel descriptor symbol", true, c)?;
    let source = evidence(&mut r, 1, c)?;
    let executable = evidence(&mut r, 2, c)?;
    let capability_count = r.count("kernel capabilities", MAX_CAPABILITIES, c)?;
    let capabilities_start = r.position;
    r.take(capability_count * 2, c)?;
    let rank = r.u8(c)?;
    let block_tag = r.u8(c)?;
    r.zero("launch constraints", c)?;
    let dims = [r.u32(c)?, r.u32(c)?, r.u32(c)?];
    let block = match block_tag {
        0 if dims == [0; 3] => BlockSizeV1::Any,
        0 => {
            return Err(DecodeError::NonzeroReserved {
                field: "unconstrained block dimensions",
            }
            .into());
        }
        1 => BlockSizeV1::Exact(DimensionsV1::new(dims[0], dims[1], dims[2])?),
        2 => BlockSizeV1::AtMost(DimensionsV1::new(dims[0], dims[1], dims[2])?),
        _ => {
            return Err(DecodeError::UnknownTag {
                kind: "block size",
                tag: u16::from(block_tag),
            }
            .into());
        }
    };
    let grid = DimensionsV1::new(r.u32(c)?, r.u32(c)?, r.u32(c)?)?;
    pay(c, 32)?;
    let launch = LaunchConstraintsV1::new(rank, block, grid, r.u32(c)?, r.u32(c)?, r.u32(c)?)?;
    let argument_count = r.count("kernel arguments", MAX_ARGUMENTS_PER_KERNEL, c)?;
    let component_count = r.count(
        "physical ABI components",
        MAX_PHYSICAL_COMPONENTS_PER_KERNEL,
        c,
    )?;
    let abi = KernelAbiLayoutV1::new(r.u32(c)?, r.u32(c)?, r.u32(c)?)?;
    let end = if index + 1 < v.kernel_count && v.kernel_offsets[index + 1] != 0 {
        v.kernel_offsets[index + 1]
    } else {
        v.requirements_start
    };
    Ok(KernelDescriptorRefV3 {
        table: v,
        id,
        logical_name,
        entry_name,
        descriptor_symbol,
        source,
        executable,
        capabilities_start,
        capability_count,
        launch,
        abi,
        arguments_start: r.position,
        argument_count,
        component_count,
        end,
    })
}
pub(crate) fn find_kernel<'v, 'w, E>(
    v: &'v DeviceDescriptorTableV3<'w>,
    id: KernelId,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<KernelDescriptorRefV3<'v, 'w>, E> {
    let (mut lo, mut hi) = (0, v.kernel_count);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let mut r = Reader::at(v.bytes, v.kernel_offsets[mid]);
        match r.fixed::<32, E>(c)?.cmp(id.as_bytes()) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => return kernel(v, mid, c),
        }
    }
    Err(ValidationError::DanglingReference { field: "kernel" }.into())
}
pub(crate) fn next_capability<E>(
    cursor: &mut CapabilityCursorV3<'_, '_>,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<Option<CapabilityV1>, E> {
    pay(c, 1)?;
    if cursor.remaining == 0 {
        return Ok(None);
    }
    let mut r = Reader::at(cursor.table.bytes, cursor.position);
    let value = parse_capability(r.u16(c)?)?;
    cursor.position = r.position;
    cursor.remaining -= 1;
    Ok(Some(value))
}
pub(crate) fn next_argument<'v, 'w, E>(
    cursor: &mut ArgumentCursorV3<'v, 'w>,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<Option<LogicalArgumentRefV3<'v, 'w>>, E> {
    pay(c, 1)?;
    if cursor.remaining == 0 {
        return Ok(None);
    }
    let mut r = Reader::at(cursor.table.bytes, cursor.position);
    let source_index = r.u16(c)?;
    r.zero("argument", c)?;
    let name = r.text("argument name", true, c)?;
    let source_type = RustTypeIdentity::from_bytes(r.fixed(c)?);
    let device_layout = DeviceLayoutIdentity::from_bytes(r.fixed(c)?);
    let ownership = parse_ownership(r.u8(c)?)?;
    let access = parse_access(r.u8(c)?)?;
    let alias = parse_alias(r.u8(c)?)?;
    if r.u8(c)? != 0 {
        return Err(DecodeError::NonzeroReserved { field: "argument" }.into());
    }
    let component_count = r.count("argument components", MAX_PHYSICAL_COMPONENTS_PER_KERNEL, c)?;
    r.zero("argument components", c)?;
    let components_start = r.position;
    r.take(component_count * 16, c)?;
    if r.position > cursor.end {
        return Err(DecodeError::Truncated.into());
    }
    let result = LogicalArgumentRefV3 {
        table: cursor.table,
        source_index,
        name,
        source_type,
        device_layout,
        ownership,
        access,
        alias,
        components_start,
        component_count,
    };
    cursor.position = r.position;
    cursor.remaining -= 1;
    Ok(Some(result))
}
pub(crate) fn component<E>(
    a: &LogicalArgumentRefV3<'_, '_>,
    index: usize,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<PhysicalComponentV3, E> {
    pay(c, 1)?;
    if index >= a.component_count {
        return Err(DescriptorWireErrorV3::Index {
            field: "component",
            index,
            count: a.component_count,
        });
    }
    let mut r = Reader::at(a.table.bytes, a.components_start + 16 * index);
    let tag = r.u8(c)?;
    let scalar = r.u8(c)?;
    let kind = match tag {
        1 => PhysicalAbiComponentKind::ScalarByValue(parse_scalar(scalar)?),
        2 if scalar == 0 => PhysicalAbiComponentKind::GlobalPointer,
        2 => {
            return Err(DecodeError::NonzeroReserved {
                field: "global pointer scalar tag",
            }
            .into());
        }
        3 if parse_scalar(scalar)? == ScalarTypeV1::U64 => PhysicalAbiComponentKind::SliceLengthU64,
        3 => {
            return Err(
                ValidationError::InvalidPhysicalAbi("slice length must be tagged u64").into(),
            );
        }
        _ => {
            return Err(DecodeError::UnknownTag {
                kind: "physical ABI component",
                tag: u16::from(tag),
            }
            .into());
        }
    };
    let access = parse_access(r.u8(c)?)?;
    let alias = parse_alias(r.u8(c)?)?;
    let offset = r.u32(c)?;
    let size = r.u16(c)?;
    let alignment = r.u16(c)?;
    r.zero("physical ABI component flags", c)?;
    r.zero("physical ABI component", c)?;
    Ok(PhysicalComponentV3 {
        kind,
        offset,
        size,
        alignment,
        access,
        alias,
    })
}
pub(crate) fn requirement<E>(
    v: &DeviceDescriptorTableV3<'_>,
    index: usize,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<KernelTargetRequirementsV2, E> {
    pay(c, 1)?;
    if index >= v.kernel_count {
        return Err(DescriptorWireErrorV3::Index {
            field: "requirement",
            index,
            count: v.kernel_count,
        });
    }
    let mut r = Reader::at(v.bytes, v.requirements_start + index * 48);
    Ok(crate::wire_v2::parse_requirement_bytes(r.take(48, c)?)?)
}

const EMPTY_COMPONENT: PhysicalComponentV3 = PhysicalComponentV3 {
    kind: PhysicalAbiComponentKind::GlobalPointer,
    offset: 0,
    size: 8,
    alignment: 8,
    access: AccessMode::ReadOnly,
    alias: AliasSemantics::SharedReadOnly,
};

pub(crate) fn validate_argument<E>(
    a: &LogicalArgumentInputV3<'_>,
    source: SourceTypeDescriptorV3,
    layout: &DeviceLayoutDescriptorV1,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<(), E> {
    pay(c, 32 + a.components.len() * 16)?;
    if *layout != source.expected_layout() {
        return Err(
            ValidationError::InvalidArgument("source type and device layout disagree").into(),
        );
    }
    if a.components.is_empty() || a.components.len() > 2 {
        return Err(
            ValidationError::InvalidPhysicalAbi("noncanonical argument component count").into(),
        );
    }
    let mut values = [EMPTY_COMPONENT.legacy(), EMPTY_COMPONENT.legacy()];
    for (out, input) in values.iter_mut().zip(a.components) {
        *out = input.legacy();
    }
    let parts = model::BorrowedArgumentParts {
        ownership: a.ownership,
        access: a.access,
        alias: a.alias,
        components: &values[..a.components.len()],
    };
    match source {
        SourceTypeDescriptorV3::Scalar(s) => model::validate_scalar_argument(&parts, s)?,
        SourceTypeDescriptorV3::Usize => {
            model::validate_scalar_argument(&parts, ScalarTypeV1::U64)?
        }
        SourceTypeDescriptorV3::Isize => {
            model::validate_scalar_argument(&parts, ScalarTypeV1::I64)?
        }
        SourceTypeDescriptorV3::SharedSlice(_) => model::validate_shared_slice_argument(&parts)?,
        SourceTypeDescriptorV3::DisjointSlice(_) => {
            model::validate_disjoint_slice_argument(&parts)?
        }
        SourceTypeDescriptorV3::GlobalMutPointer(_) => {
            model::validate_global_mut_pointer_argument(&parts)?
        }
    }
    Ok(())
}

#[derive(Default)]
pub(crate) struct Components {
    end: u64,
    alignment: u64,
    count: usize,
}
impl Components {
    pub(crate) fn add<E>(
        &mut self,
        p: PhysicalComponentV3,
        abi: KernelAbiLayoutV1,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV3<(), E> {
        pay(c, 16)?;
        if p.size == 0
            || p.alignment == 0
            || !p.alignment.is_power_of_two()
            || !p.offset.is_multiple_of(u32::from(p.alignment))
        {
            return Err(ValidationError::InvalidPhysicalAbi(
                "component size, alignment, or offset is invalid",
            )
            .into());
        }
        let end = u64::from(p.offset) + u64::from(p.size);
        if u64::from(p.offset) < self.end {
            return Err(ValidationError::InvalidPhysicalAbi(
                "kernel components overlap or are out of source order",
            )
            .into());
        }
        if end > u64::from(abi.explicit_argument_size())
            || u32::from(p.alignment) > abi.kernarg_segment_alignment()
        {
            return Err(ValidationError::InvalidPhysicalAbi(
                "component exceeds the explicit argument region or alignment",
            )
            .into());
        }
        self.end = end;
        self.alignment = self.alignment.max(u64::from(p.alignment));
        self.count += 1;
        count(
            self.count,
            "physical ABI components",
            MAX_PHYSICAL_COMPONENTS_PER_KERNEL,
        )
    }
    pub(crate) fn finish<E>(
        &self,
        abi: KernelAbiLayoutV1,
        declared: usize,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV3<(), E> {
        pay(c, 8)?;
        let size = if self.end == 0 {
            0
        } else {
            (self.end + self.alignment - 1) & !(self.alignment - 1)
        };
        if size != u64::from(abi.explicit_argument_size()) || self.count != declared {
            return Err(ValidationError::InvalidPhysicalAbi(
                "noncanonical explicit size or component count",
            )
            .into());
        }
        Ok(())
    }
}

struct Scratch<'a> {
    types: [bool; MAX_TYPE_RECORDS],
    layouts: [bool; MAX_LAYOUT_RECORDS],
    names: [[&'a str; 3]; MAX_KERNELS],
    arguments: [&'a str; MAX_ARGUMENTS_PER_KERNEL],
    capabilities: [CapabilityV1; MAX_CAPABILITIES],
    components: [PhysicalComponentV3; 2],
}
impl Scratch<'_> {
    fn new() -> Self {
        Self {
            types: [false; MAX_TYPE_RECORDS],
            layouts: [false; MAX_LAYOUT_RECORDS],
            names: [[""; 3]; MAX_KERNELS],
            arguments: [""; MAX_ARGUMENTS_PER_KERNEL],
            capabilities: [CapabilityV1::Subgroup; MAX_CAPABILITIES],
            components: [EMPTY_COMPONENT; 2],
        }
    }
}

pub(crate) fn distinct<E>(
    names: &[&str],
    next: &str,
    field: &'static str,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<(), E> {
    for old in names {
        pay(c, old.len().max(next.len()) + 1)?;
        if *old == next {
            return Err(ValidationError::Duplicate { field }.into());
        }
    }
    Ok(())
}

fn validate_view<E>(
    v: &DeviceDescriptorTableV3<'_>,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<(), E> {
    pay(
        c,
        MAX_TYPE_RECORDS
            + MAX_LAYOUT_RECORDS
            + MAX_KERNELS * 3
            + MAX_ARGUMENTS_PER_KERNEL
            + MAX_CAPABILITIES
            + 2,
    )?;
    let mut scratch = Scratch::new();
    pay(c, 32)?;
    let target = AmdTargetCapabilities::derive(v.target.as_amd_target_id()).map_err(|_| {
        ValidationError::TargetMismatch {
            field: "target capability profile",
        }
    })?;
    let mut previous = None;
    for i in 0..v.kernel_count {
        let k = kernel(v, i, c)?;
        pay(c, 32)?;
        order(previous, k.id, "kernel ID")?;
        previous = Some(k.id);
        let names = [k.logical_name, k.entry_name, k.descriptor_symbol];
        for old in &scratch.names[..i] {
            for (a, b) in old.iter().zip(names) {
                pay(c, a.len().max(b.len()) + 1)?;
                if *a == b {
                    return Err(ValidationError::Duplicate {
                        field: "kernel name or symbol",
                    }
                    .into());
                }
            }
        }
        scratch.names[i] = names;
        let mut caps = k.capabilities();
        let mut prior = None;
        let mut cap_count = 0;
        while let Some(cap) = caps.next(c)? {
            pay(c, 1)?;
            order(prior, cap, "capabilities")?;
            prior = Some(cap);
            scratch.capabilities[cap_count] = cap;
            cap_count += 1;
        }
        let mut args = k.arguments();
        let mut index = 0;
        let mut components = Components::default();
        while let Some(a) = args.next(c)? {
            pay(c, 1)?;
            if usize::from(a.source_index) != index {
                return Err(ValidationError::InvalidArgument(
                    "source argument indices must be contiguous",
                )
                .into());
            }
            distinct(&scratch.arguments[..index], a.name, "argument name", c)?;
            scratch.arguments[index] = a.name;
            let ti = search(
                v.bytes,
                v.types_start,
                v.type_count,
                36,
                a.source_type.as_bytes(),
                "Rust type",
                c,
            )?;
            let li = search(
                v.bytes,
                v.layouts_start,
                v.layout_count,
                44,
                a.device_layout.as_bytes(),
                "device layout",
                c,
            )?;
            scratch.types[ti] = true;
            scratch.layouts[li] = true;
            let source = source_row(v.bytes, v.types_start + ti * 36, c)?;
            let layout = layout_row(v.bytes, v.layouts_start + li * 44, c)?;
            if a.component_count > 2 {
                return Err(ValidationError::InvalidPhysicalAbi(
                    "noncanonical argument component count",
                )
                .into());
            }
            for (j, p) in scratch.components[..a.component_count]
                .iter_mut()
                .enumerate()
            {
                *p = a.component(j, c)?;
                components.add(*p, k.abi, c)?;
            }
            let input = LogicalArgumentInputV3 {
                source_index: a.source_index,
                name: a.name,
                source_type: a.source_type,
                device_layout: a.device_layout,
                ownership: a.ownership,
                access: a.access,
                alias: a.alias,
                components: &scratch.components[..a.component_count],
            };
            validate_argument(&input, source.descriptor(), layout.descriptor(), c)?;
            index += 1;
        }
        if args.position != k.end {
            return Err(DecodeError::NonCanonical.into());
        }
        components.finish(k.abi, k.component_count, c)?;
        let req = requirement(v, i, c)?;
        pay(c, 32 + 32 + 4 * cap_count)?;
        if req.kernel_id() != k.id {
            return Err(ValidationError::DanglingReference {
                field: "kernel target requirement",
            }
            .into());
        }
        crate::requirements_v2::validate_borrowed_kernel_requirement_v3(
            &k.launch,
            &scratch.capabilities[..cap_count],
            req,
            target,
        )?;
    }
    pay(c, v.type_count + v.layout_count)?;
    if scratch.types[..v.type_count].contains(&false) {
        return Err(ValidationError::UnreachableRecord { field: "Rust type" }.into());
    }
    if scratch.layouts[..v.layout_count].contains(&false) {
        return Err(ValidationError::UnreachableRecord {
            field: "device layout",
        }
        .into());
    }
    Ok(())
}

/// Caller prepays VIEW + READER scratch and the borrowed input bytes before entry.
/// No usable view is returned until every record, kernel and requirement is checked.
pub fn decode_device_descriptor_table_v3<'a, E>(
    bytes: &'a [u8],
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<DeviceDescriptorTableV3<'a>, E> {
    let (view, end) = decode_nominal_prefix(bytes, DEVICE_DESCRIPTOR_VERSION_V3, c)?;
    if end != bytes.len() {
        return Err(DecodeError::TrailingBytes.into());
    }
    Ok(view)
}

/// Private common row decoder. V4 must validate its mandatory trailer before
/// returning a distinct public view; this never exposes V4 through V3 decode.
pub(crate) fn decode_nominal_prefix<'a, E>(
    bytes: &'a [u8],
    expected_version: u16,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<(DeviceDescriptorTableV3<'a>, usize), E> {
    pay(c, 1)?;
    if bytes.len() > MAX_DESCRIPTOR_TABLE_BYTES {
        return Err(DecodeError::TooLarge {
            max: MAX_DESCRIPTOR_TABLE_BYTES,
        }
        .into());
    }
    let mut r = Reader::at(bytes, 0);
    if r.fixed::<8, E>(c)? != DEVICE_DESCRIPTOR_MAGIC {
        return Err(DecodeError::InvalidMagic.into());
    }
    let version = r.u16(c)?;
    if version != expected_version {
        return Err(DecodeError::UnknownVersion(version).into());
    }
    let flags = r.u16(c)?;
    if flags != 0 {
        return Err(DecodeError::UnsupportedFlags(flags).into());
    }
    if r.u32(c)? as usize != bytes.len() {
        return Err(DecodeError::NonCanonical.into());
    }
    let digest = CanonicalCodeObjectDigest::from_bytes(r.fixed(c)?);
    let cov = parse_code_object_version(r.u8(c)?)?;
    if r.fixed::<3, E>(c)? != [8, 1, 0] {
        return Err(invalid("target pointer width/endian"));
    }
    let compiler_name = r.text("compiler name", false, c)?;
    let compiler_release = r.text("compiler release", false, c)?;
    let compiler_commit = r.fixed(c)?;
    let producer_name = r.text("producer name", false, c)?;
    let producer_version = r.text("producer version", false, c)?;
    let target = target(r.text("device target", false, c)?, c)?;
    let type_count = r.count("type records", MAX_TYPE_RECORDS, c)?;
    let layout_count = r.count("layout records", MAX_LAYOUT_RECORDS, c)?;
    let kernel_count = r.count("kernels", MAX_KERNELS, c)?;
    let requirements = r.count("kernel target requirements", MAX_KERNELS, c)?;
    if kernel_count == 0 {
        return Err(ValidationError::Empty { field: "kernels" }.into());
    }
    if requirements != kernel_count {
        return Err(invalid("kernel target requirement closure"));
    }
    let types_start = r.position;
    let mut previous = None;
    for _ in 0..type_count {
        let row = source_row(bytes, r.position, c)?;
        pay(c, 32)?;
        order(previous, row.identity(), "Rust type")?;
        previous = Some(row.identity());
        r.take(36, c)?;
    }
    let layouts_start = r.position;
    let mut previous = None;
    for _ in 0..layout_count {
        let row = layout_row(bytes, r.position, c)?;
        pay(c, 32)?;
        order(previous, row.identity(), "device layout")?;
        previous = Some(row.identity());
        r.take(44, c)?;
    }
    pay(c, MAX_KERNELS)?;
    let mut v = DeviceDescriptorTableV3 {
        bytes,
        digest,
        cov,
        compiler_name,
        compiler_release,
        compiler_commit,
        producer_name,
        producer_version,
        target,
        types_start,
        layouts_start,
        requirements_start: bytes.len(),
        type_count,
        layout_count,
        kernel_count,
        kernel_offsets: [0; MAX_KERNELS],
    };
    for i in 0..kernel_count {
        v.kernel_offsets[i] = r.position;
        let k = kernel(&v, i, c)?;
        let mut args = k.arguments();
        while args.next(c)?.is_some() {}
        r.position = args.position;
    }
    v.requirements_start = r.position;
    r.take(requirements * 48, c)?;
    // Preserve V3's original rejection order and charging before view replay.
    if expected_version == DEVICE_DESCRIPTOR_VERSION_V3 && r.position != bytes.len() {
        return Err(DecodeError::TrailingBytes.into());
    }
    validate_view(&v, c)?;
    Ok((v, r.position))
}

// These are explicit simultaneous typed extents, not an allocator/RSS promise.
// Returned views/cursors supplied by callers remain separately live and paid.
pub(crate) const QUERY_SCRATCH_BYTES: usize = size_of::<Reader<'static>>() * 3
    + size_of::<KernelDescriptorRefV3<'static, 'static>>()
    + size_of::<ArgumentCursorV3<'static, 'static>>()
    + size_of::<CapabilityCursorV3<'static, 'static>>()
    + size_of::<LogicalArgumentRefV3<'static, 'static>>()
    + size_of::<PhysicalComponentV3>() * 3
    + size_of::<SourceTypeRecordV3>() * 2
    + size_of::<DeviceLayoutRecordV1>() * 2
    + size_of::<KernelTargetRequirementsV2>()
    + size_of::<sha2::Sha256>()
    + size_of::<[u8; 32]>() * 2
    + size_of::<[u8; 48]>()
    + size_of::<[u8; 12]>()
    + size_of::<[u8; 4]>()
    + size_of::<[u8; 16]>()
    + size_of::<[PhysicalAbiComponentV1; 2]>()
    + size_of::<model::BorrowedArgumentParts<'static>>()
    + size_of::<LogicalArgumentInputV3<'static>>()
    + size_of::<Components>();
pub(crate) const READER_SCRATCH_BYTES: usize = size_of::<Scratch<'static>>()
    + QUERY_SCRATCH_BYTES
    + size_of::<Reader<'static>>()
    + size_of::<CompareText<'static>>()
    + size_of::<AmdTargetCapabilities>()
    + size_of::<AmdTargetId>();
pub(crate) const ENCODER_SCRATCH_BYTES: usize = READER_SCRATCH_BYTES
    + size_of::<DeviceDescriptorTableInputV3<'static>>()
    + size_of::<TargetText>()
    + crate::nominal_v3::OUTPUT_HEADER_STORAGE;

pub(crate) struct TargetText {
    pub(crate) bytes: [u8; MAX_TEXT_BYTES],
    pub(crate) length: usize,
}
impl Write for TargetText {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.length.checked_add(s.len()).ok_or(fmt::Error)?;
        self.bytes
            .get_mut(self.length..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(s.as_bytes());
        self.length = end;
        Ok(())
    }
}

fn text_len<E>(
    s: &str,
    name: bool,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<usize, E> {
    pay(c, 3 * s.len() + 1)?;
    if name {
        model::validate_name(s, "V3 name")?;
    } else {
        model::validate_text(s, "V3 text")?;
    }
    Ok(s.len() + 2)
}
fn input_find<E, T>(
    values: &[T],
    key: impl Fn(&T) -> [u8; 32],
    id: [u8; 32],
    field: &'static str,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<usize, E> {
    let (mut lo, mut hi) = (0, values.len());
    while lo < hi {
        pay(c, 33)?;
        let mid = lo + (hi - lo) / 2;
        match key(&values[mid]).cmp(&id) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => return Ok(mid),
        }
    }
    Err(ValidationError::DanglingReference { field }.into())
}

/// Complete borrowed-input validation, then exact output size. No output writes.
pub(crate) fn preflight<E>(
    v: &DeviceDescriptorTableInputV3<'_>,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV3<(usize, TargetText), E> {
    pay(c, 1)?;
    count(v.kernels.len(), "kernels", MAX_KERNELS)?;
    count(v.type_records.len(), "type records", MAX_TYPE_RECORDS)?;
    count(v.layout_records.len(), "layout records", MAX_LAYOUT_RECORDS)?;
    if v.kernels.is_empty() {
        return Err(ValidationError::Empty { field: "kernels" }.into());
    }
    if v.requirements.len() != v.kernels.len() {
        return Err(invalid("kernel target requirement closure"));
    }
    pay(
        c,
        MAX_TYPE_RECORDS
            + MAX_LAYOUT_RECORDS
            + MAX_KERNELS * 3
            + MAX_ARGUMENTS_PER_KERNEL
            + MAX_CAPABILITIES
            + 2,
    )?;
    let mut scratch = Scratch::new();
    pay(c, MAX_TEXT_BYTES * 2 + 32)?;
    let mut target_text = TargetText {
        bytes: [0; MAX_TEXT_BYTES],
        length: 0,
    };
    write!(&mut target_text, "{}", v.device_target).map_err(|_| invalid("device target"))?;
    let target =
        AmdTargetCapabilities::derive(v.device_target.as_amd_target_id()).map_err(|_| {
            ValidationError::TargetMismatch {
                field: "target capability profile",
            }
        })?;
    let mut n = 48 + 4 + 20 + 8 + 2 + target_text.length;
    for s in [
        v.compiler.name().as_str(),
        v.compiler.release().as_str(),
        v.producer.name().as_str(),
        v.producer.version().as_str(),
    ] {
        n += text_len(s, false, c)?;
    }
    let mut prior = None;
    for row in v.type_records {
        pay(c, 32)?;
        order(prior, row.identity(), "Rust type")?;
        prior = Some(row.identity());
        let actual = SourceTypeRecordV3::new(row.descriptor(), c)?;
        if actual != *row {
            return Err(ValidationError::IdentityMismatch { field: "Rust type" }.into());
        }
        n += 36;
    }
    let mut prior = None;
    for row in v.layout_records {
        pay(c, 32)?;
        order(prior, row.identity(), "device layout")?;
        prior = Some(row.identity());
        let actual = device_layout_record_v3(row.descriptor().clone(), c)?;
        if actual != *row {
            return Err(ValidationError::IdentityMismatch {
                field: "device layout",
            }
            .into());
        }
        n += 44;
    }
    let mut prior = None;
    for (i, k) in v.kernels.iter().enumerate() {
        pay(c, 32)?;
        order(prior, k.kernel_id, "kernel ID")?;
        prior = Some(k.kernel_id);
        count(
            k.arguments.len(),
            "kernel arguments",
            MAX_ARGUMENTS_PER_KERNEL,
        )?;
        count(
            k.capabilities.len(),
            "kernel capabilities",
            MAX_CAPABILITIES,
        )?;
        let names = [k.logical_name, k.entry_name, k.descriptor_symbol];
        for s in names {
            n += text_len(s, true, c)?;
        }
        for old in &scratch.names[..i] {
            for (a, b) in old.iter().zip(names) {
                pay(c, a.len().max(b.len()) + 1)?;
                if *a == b {
                    return Err(ValidationError::Duplicate {
                        field: "kernel name or symbol",
                    }
                    .into());
                }
            }
        }
        scratch.names[i] = names;
        let mut prior_cap = None;
        for cap in k.capabilities {
            pay(c, 1)?;
            order(prior_cap, *cap, "capabilities")?;
            prior_cap = Some(*cap);
        }
        // Opaque launch/layout values were admitted by their unchanged constructors.
        n += 32 + 136 + 2 + 2 * k.capabilities.len() + 40 + 16;
        let mut components = Components::default();
        let mut total_components = 0;
        for (j, a) in k.arguments.iter().enumerate() {
            pay(c, 1)?;
            if usize::from(a.source_index) != j {
                return Err(ValidationError::InvalidArgument(
                    "source argument indices must be contiguous",
                )
                .into());
            }
            n += text_len(a.name, true, c)?;
            distinct(&scratch.arguments[..j], a.name, "argument name", c)?;
            scratch.arguments[j] = a.name;
            let ti = input_find(
                v.type_records,
                |r| *r.identity().as_bytes(),
                *a.source_type.as_bytes(),
                "Rust type",
                c,
            )?;
            let li = input_find(
                v.layout_records,
                |r| *r.identity().as_bytes(),
                *a.device_layout.as_bytes(),
                "device layout",
                c,
            )?;
            scratch.types[ti] = true;
            scratch.layouts[li] = true;
            count(
                a.components.len(),
                "argument components",
                MAX_PHYSICAL_COMPONENTS_PER_KERNEL,
            )?;
            validate_argument(
                a,
                v.type_records[ti].descriptor(),
                v.layout_records[li].descriptor(),
                c,
            )?;
            for p in a.components {
                components.add(*p, k.abi_layout, c)?;
            }
            total_components += a.components.len();
            n += 4 + 64 + 4 + 4 + 16 * a.components.len();
        }
        components.finish(k.abi_layout, total_components, c)?;
        let req = v.requirements[i];
        pay(c, 64 + 4 * k.capabilities.len())?;
        if req.kernel_id() != k.kernel_id {
            return Err(ValidationError::DanglingReference {
                field: "kernel target requirement",
            }
            .into());
        }
        crate::requirements_v2::validate_borrowed_kernel_requirement_v3(
            k.launch,
            k.capabilities,
            req,
            target,
        )?;
        n += 48;
    }
    pay(c, v.type_records.len() + v.layout_records.len())?;
    if scratch.types[..v.type_records.len()].contains(&false) {
        return Err(ValidationError::UnreachableRecord { field: "Rust type" }.into());
    }
    if scratch.layouts[..v.layout_records.len()].contains(&false) {
        return Err(ValidationError::UnreachableRecord {
            field: "device layout",
        }
        .into());
    }
    if n > MAX_DESCRIPTOR_TABLE_BYTES {
        return Err(ValidationError::EncodedTableTooLarge {
            max: MAX_DESCRIPTOR_TABLE_BYTES,
        }
        .into());
    }
    Ok((n, target_text))
}
