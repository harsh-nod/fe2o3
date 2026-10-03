//! Inert storage/type descriptors. Owner/table/graph custody lives in the bridge.
use pliron::{
    common_traits::Verify,
    context::Context,
    derive::{pliron_attr, pliron_type},
    result::Result,
    verify_err,
};

#[pliron_attr(
    name = "gpu.storage_table_key_v18",
    format = "`<` $a `,` $b `,` $c `,` $d `,` $length `>`",
    verifier = "succ"
)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StorageTableKeyAttrV18 {
    a: u64,
    b: u64,
    c: u64,
    d: u64,
    length: u64,
}
impl StorageTableKeyAttrV18 {
    pub fn new(digest: [u8; 32], length: u64) -> Self {
        Self {
            a: u64::from_le_bytes(digest[0..8].try_into().unwrap()),
            b: u64::from_le_bytes(digest[8..16].try_into().unwrap()),
            c: u64::from_le_bytes(digest[16..24].try_into().unwrap()),
            d: u64::from_le_bytes(digest[24..32].try_into().unwrap()),
            length,
        }
    }
    pub fn digest(self) -> [u8; 32] {
        let mut bytes = [0; 32];
        for (part, word) in bytes
            .chunks_exact_mut(8)
            .zip([self.a, self.b, self.c, self.d])
        {
            part.copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }
    pub const fn encoded_length(self) -> u64 {
        self.length
    }
}

#[pliron_attr(name = "gpu.storage_ordinal_v18", format = "$0", verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StorageOrdinalAttrV18(pub u32);

/// Storage-only descriptor, never a first-class aggregate SSA value.
/// A matching table-content key is not source or graph authority.
#[pliron_type(
    name = "gpu.storage_object_v18",
    format = "`<` $table `,` $row `>`",
    generate_get = true
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StorageObjectTypeV18 {
    table: StorageTableKeyAttrV18,
    row: StorageOrdinalAttrV18,
}
impl StorageObjectTypeV18 {
    pub const fn table(&self) -> StorageTableKeyAttrV18 {
        self.table
    }
    pub const fn row(&self) -> u32 {
        self.row.0
    }
}
impl Verify for StorageObjectTypeV18 {
    fn verify(&self, _context: &Context) -> Result<()> {
        if self.table.length < 4 {
            return verify_err!(
                pliron::location::Location::Unknown,
                "storage table key must include its canonical row count"
            );
        }
        // Only the exact owning bridge can look up and validate the row.
        Ok(())
    }
}

/// Closed inert role/geometry for V18 transport of the existing V15 lifecycle.
/// No integer representation, scalar snapshot or implicit role conversion.
#[pliron_type(
    name = "gpu.execution_role_v18",
    format = "`<` $role `,` $lanes `,` $elements `>`",
    generate_get = true
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ExecutionRoleTypeV18 {
    role: StorageOrdinalAttrV18,
    lanes: StorageOrdinalAttrV18,
    elements: StorageOrdinalAttrV18,
}
impl ExecutionRoleTypeV18 {
    pub const fn role(&self) -> u32 {
        self.role.0
    }
    pub const fn lanes(&self) -> u32 {
        self.lanes.0
    }
    pub const fn elements(&self) -> u32 {
        self.elements.0
    }
}
impl Verify for ExecutionRoleTypeV18 {
    fn verify(&self, _context: &Context) -> Result<()> {
        let valid = match self.role.0 {
            1 | 2 => self.lanes.0 == 0 && self.elements.0 == 0,
            3 | 4 => (1..=256).contains(&self.lanes.0) && (1..=125).contains(&self.elements.0),
            _ => false,
        };
        if !valid {
            return verify_err!(
                pliron::location::Location::Unknown,
                "invalid closed execution role or geometry"
            );
        }
        Ok(())
    }
}
