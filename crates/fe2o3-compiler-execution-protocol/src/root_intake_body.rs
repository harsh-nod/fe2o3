// Shared closed-version canonical engine. Each module fixes its own shape and roles.
impl Record {
    pub fn challenge(hello: &Self, nonce: [u8; 32], b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(b, RETAINED, || {
            if hello.kind() != Kind::Hello {
                return Err(Error::Framing("challenge requires hello"));
            }
            let mut bytes = hello.bytes;
            bytes[16] = Kind::Challenge as u8;
            bytes[56..88].copy_from_slice(&nonce);
            bytes[152..184].copy_from_slice(hello.identity());
            finish(bytes)
        })
    }

    pub fn input(challenge: &Self, role: Role, b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(b, RETAINED, || {
            if challenge.kind() != Kind::Challenge || !challenge.roles().any(|r| r == role) {
                return Err(Error::Framing("input requires challenge and selected role"));
            }
            let mut bytes = challenge.bytes;
            bytes[16] = Kind::Input as u8;
            bytes[17] = role as u8;
            bytes[152..184].copy_from_slice(challenge.identity());
            finish(bytes)
        })
    }

    /// Inert terminal refusal only. A decoder/caller can fabricate this record;
    /// only actual authenticated receiver custody may send it as a decision.
    /// A last-role record does not prove receipt/retention of earlier descriptors.
    pub fn enforcement_unavailable(last: &Self, b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(b, RETAINED, || {
            if last.kind() != Kind::Input || last.role() != last.roles().last() {
                return Err(Error::Framing("refusal requires final selected input"));
            }
            let mut bytes = last.bytes;
            bytes[16] = Kind::Ack as u8;
            bytes[17] = 0;
            bytes[19] = 1;
            bytes[152..184].copy_from_slice(last.identity());
            finish(bytes)
        })
    }

    pub fn decode(bytes: &[u8], b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        metered(b, resources::fixed_input_floor(bytes, N), || {
            if bytes.len() != N {
                return Err(Error::Framing("root intake length"));
            }
            validate(bytes)?;
            if bytes[DIGEST..] != digest(bytes) {
                return Err(Error::Framing("root intake digest"));
            }
            Ok((
                Self {
                    bytes: fixed(bytes),
                },
                Storage(RETAINED),
            ))
        })
    }

    /// Exact inert predecessor join. The caller separately authenticates sender,
    /// enforces no extra/missing rights and retains the complete input sequence.
    pub fn matches_predecessor(&self, previous: &Self, b: &mut Budget<'_>) -> Result<bool> {
        metered(b, 2 * RETAINED, || {
            let phase = matches!(
                (previous.kind(), self.kind()),
                (Kind::Hello, Kind::Challenge) | (Kind::Challenge, Kind::Input)
            ) || (previous.kind() == Kind::Input
                && self.kind() == Kind::Ack
                && previous.role() == previous.roles().last());
            Ok(phase
                && self.bytes[18] == previous.bytes[18]
                && self.bytes[24..56] == previous.bytes[24..56]
                && self.bytes[88..152] == previous.bytes[88..152]
                && self.bytes[184..DIGEST] == previous.bytes[184..DIGEST]
                && (previous.kind() == Kind::Hello || self.bytes[56..88] == previous.bytes[56..88])
                && self.bytes[152..184] == *previous.identity())
        })
    }

    pub fn kind(&self) -> Kind {
        match self.bytes[16] {
            1 => Kind::Hello,
            2 => Kind::Challenge,
            3 => Kind::Input,
            4 => Kind::Ack,
            _ => unreachable!("validated intake kind"),
        }
    }
    pub fn role(&self) -> Option<Role> {
        role(self.bytes[17])
    }
    pub fn roles(&self) -> impl Iterator<Item = Role> {
        selected_roles(self.bytes[18]).into_iter().flatten()
    }

    pub const fn stdio_mask(&self) -> u8 {
        self.bytes[18]
    }
    pub fn policy_identity(&self) -> &[u8; 32] {
        self.bytes[88..120].try_into().expect("fixed policy")
    }
    pub fn invocation_identity(&self) -> &[u8; 32] {
        self.bytes[120..152].try_into().expect("fixed invocation")
    }
    pub fn invocation_bytes(&self) -> u64 {
        u64::from_le_bytes(fixed(&self.bytes[184..192]))
    }
    pub fn identity(&self) -> &[u8; 32] {
        self.bytes[DIGEST..].try_into().expect("fixed digest")
    }
    pub const fn canonical_bytes(&self) -> &[u8; N] {
        &self.bytes
    }
    pub const fn retained_storage(&self) -> usize {
        RETAINED
    }
}

fn validate(bytes: &[u8]) -> Result<()> {
    let length = usize::try_from(u64::from_le_bytes(fixed(&bytes[184..192])))
        .map_err(|_| Error::Framing("root intake length overflows usize"))?;
    if &bytes[..8] != MAGIC
        || bytes[8..12] != VERSION.to_le_bytes()
        || bytes[12..16] != (N as u32).to_le_bytes()
        || bytes[20..24] != [0; 4]
        || bytes[18] & !7 != 0
        || !(1..=MAX_DESCRIPTOR_BYTES_V3).contains(&length)
    {
        return Err(Error::Framing("root intake header or bound"));
    }
    for start in [24, 88, 120] {
        if bytes[start..start + 32] == [0; 32] {
            return Err(Error::Framing("zero intake association"));
        }
    }
    let hello = bytes[16] == Kind::Hello as u8;
    if (bytes[56..88] == [0; 32]) != hello || (bytes[152..184] == [0; 32]) != hello {
        return Err(Error::Framing("intake challenge or predecessor"));
    }
    match bytes[16] {
        1 | 2 if bytes[17] == 0 && bytes[19] == 0 => Ok(()),
        3 if bytes[19] == 0 => {
            let selected =
                role(bytes[17]).is_some_and(|r| selected_roles(bytes[18]).contains(&Some(r)));
            if selected {
                Ok(())
            } else {
                Err(Error::Framing("unselected intake role"))
            }
        }
        4 if bytes[17] == 0 && bytes[19] == 1 => Ok(()),
        _ => Err(Error::Framing("intake kind, role or refusal status")),
    }
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update(&bytes[..DIGEST]);
    hash.finalize().into()
}
fn finish(mut bytes: [u8; N]) -> Result<(Record, Storage)> {
    validate(&bytes)?;
    let hash = digest(&bytes);
    bytes[DIGEST..].copy_from_slice(&hash);
    Ok((Record { bytes }, Storage(RETAINED)))
}
fn fixed<const M: usize>(bytes: &[u8]) -> [u8; M] {
    bytes.try_into().expect("fixed intake field")
}
fn metered<T>(
    b: &mut Budget<'_>,
    floor: usize,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    b.with_prepaid_scope(floor, resources::ENTRY_WORK, WORK, SCRATCH, |_| operation())
}

#[derive(Debug)]
pub enum Error {
    Framing(&'static str),
    Resource(Resource),
}
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(reason) => f.write_str(reason),
            Self::Resource(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            _ => None,
        }
    }
}
impl fmt::Debug for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(DEBUG_NAME)
            .field("kind", &self.kind())
            .field("role", &self.role())
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}
