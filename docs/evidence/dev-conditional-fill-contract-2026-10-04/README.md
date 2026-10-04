# Independent Conditional Fill Contract

Source: `9f440339a8406ea3d04fc8d158d0ef7ab7c68869`.
Parent: `bb67334d2b76fe2f5d58fa54338b3ba7a6756076`.
The closed fill's host contract is now reconstructed without trusting an
application marker. This advances the ordinary two-GPU admission path; it does
not implement the remote proof custodian or grant launch authority.

## Implementation

`derive_worker_v3_conditional_fill_host_contract_v1` takes the original checked
whole-program owner and canonical descriptor source, without a generic marker.
It reconstructs the existing domain-separated `InertAbiReceiptV3` identity and
requires exact hash/length equality with the original target-lineage ABI member.
A plain descriptor content hash is deliberately not used as that receipt identity.

The helper requires gfx942/xnack-minus, wave64, V6, one descriptor and one semantic
root. It binds the original semantic export/kernel identity, sole exclusive output,
exact u32 scalar layout, 64-bit pointer/usize ScalarPair layout, and the complete
canonical write-only disjoint-slice descriptor argument at ordinal/offset zero.
The checked whole-program owner already establishes the exact u32/Index1D write
semantics and original source-to-KIR association.

The descriptor must declare explicit ABI size 16, total kernarg size 272 and
alignment 8. Source launch must explicitly require `[64,1,1]`, with no occupancy
constraint and an absent or identical maximum. Descriptor rank, block and maximum
flat workgroup size must agree. Only the required `AmdWave` capability is allowed;
additional capabilities, LDS resources and assembly declarations reject.

The existing sealed scalar helper reconstructs the canonical Rust type/layout
identity. The existing artifacts API hashes the 16-byte explicit host ABI with
Mutable/WriteOnly/Global/UniqueBorrow/Exclusive effects and the original maximum
grid. It does not hash the 272-byte machine storage size as the host ABI.

Host association retains the derived contract. Pending admission requires equality
with the marker declaration before finalizer/currentness/service processing; a
mismatch leaves the inherited service endpoint unused. Generic request preparation
and both existing exhaustive public error enums are unchanged. The original proof,
compiler/analyzer owners and publication token remain retained independently.

The returned digest is canonical contract-equivalence data, not original ABI
provenance. Its schema intentionally excludes source/executable digests. Copying it
does not transfer compiler authentication, proof custody, currentness or authority.

## Qualification

| Check | Result |
| --- | --- |
| Host library on frozen source | 221 passed, 3 ignored |
| Host doctests | 13 positive and 36 compile-fail passed |
| Conditional-fill verifier selection | 15 passed, 3 ignored, 155 filtered |
| Generated-marker contract match | 1 passed |
| Fresh protected pending campaign | All seven cases passed; 238.36 seconds |
| Selected vertical regression | 47 passed, 5 ignored, 16 filtered |
| V2 envelope regression | 31 passed, 3 ignored |
| Host and vertical strict Clippy | Passed with `-D warnings` |
| Changed-file rustfmt and whitespace | Passed |

Seven new host tests exercise original-source binding, descriptor argument/layout
substitutions, slice/element primitive alignment, source/descriptor launch bounds,
maximum-grid commitment, capabilities, entry identity and assembly/resources.
The positive fixture uses `#[kernel(typed)]` to generate the actual fill marker,
ABI and contract. Its explicit captured namespace derives the original kernel
binding, which the default test checks alongside both names and the independently
derived contract. No global compiler-binding environment or fixture dependency was
added. The audit-only marker remains useful for non-authoritative inspection and
the new same-name/same-binding foreign-contract rejection.

The protected campaign runs Good, Marker, HostContract, Payload, StaleSuccess,
StaleFailure and ServiceFailure. Every case executes fresh native Worker
compilation/replay/finalization, authenticated machine analysis and protected Verus
refinement. Existing owner-address, canonical-subject, replacement/restoration,
endpoint-unused and AlreadyConsumed assertions remain. No archived receipt replaces
an executed proof owner. The compiler-current-record service is test-key-backed,
not a genuine deployed issuer/anchor or ordinary no-fork application campaign.

Final CPU checks use four Cargo jobs, disabled HIP discovery/incremental/debug
information, test optimization 1, and enabled debug assertions/overflow checks.
Host library threading is default; doctests/verifier use four threads, and vertical,
V2 and protected tests use one. The vertical selection excludes `strict_v3_` and
`cargo_supervisor_and_static_host_consumer`; no full workspace pass is claimed.
Development logs retain corrected receipt-domain, capability-profile, fixture and
compile errors. The frozen-source rerun is separate from the initial host run.

The native Worker SHA-256 remains
`fb020a09969938d7fa849e106a146e81668d9d0b0d7dd40ded0e2145e6836715`.
Protected runtime source/installation verification passed with manifest identity
`ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c`.
Pinned cached inputs were rechecked before isolated root provisioning and
unprivileged execution. The namespace exited, its test TMPDIR was empty, and the
owned root library overlay was removed. No host `/opt` installation remains.
No MI300X resources or GPU queues were used. No new formal theorem or HIP/HSA
performance/parity claim follows from these CPU results.

[Evidence archive](evidence.tar.gz) SHA-256:
`95927157fce3573b965f5d1d3fcb3c676279da24a479f4523d7d212826167fd4`.
It includes commands, source patch/hash/signature records, development and final
logs, all seven fresh proof/analyzer/service captures and cleanup checks.
`proof.key` files are public verifying keys, not signing keys. Binaries, downloaded
packages and the private runtime tree are excluded. Source patch SHA-256:
`59bdc0cdcf574d6e9f52c7ac7d76b4922d5eee6129e003011a10772f195083d1`.

## Remaining Multi-GPU Gates

The fixed, independently measured custodian can now reuse this non-generic contract
derivation. It must still validate the complete transferred compiler closure and
authenticate the original application occurrence and fresh session, without
reacquiring the application's publication lock. Retain the original proof through
both devices' settlement and preserve the permanent no-fork application filter.
Genuine deployed compiler/anchor qualification, the private native invocation join,
and admitted two-GPU fill/transfer/guarded readback remain required. A3 is open.
