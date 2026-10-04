# Source/ISA characteristic CLI transcript V1

This deterministic fixture is synthetic, canonical, and self-claimed. It is
not authenticated producer evidence and is not a result from the protected
3x2 `gfx942`/`gfx950` acceptance matrix.

`collection.hex` decodes to the exact 1424-byte canonical self-claimed
observation archive. The SHA-256 of those raw archive bytes is
`ad395666f9a036a259ce6a8f6e47a568693dbfe1c923c3eb6bd062492627b3b4`.
The decoded collection's canonical byte length is 1424 and its domain-separated
collection identity is
`5595821cf85ebc8cb5018f68a7ac07e938af0b4ed424e9f4039201581db23a7c`.
Neither value is an authenticity claim.

Run the transcript with:

```text
xxd -r -p collection.hex > collection.bin
cargo fe2o3 inspect --format source-isa-characteristic-v1 \
  --output agent-json-v1 collection.bin < requests.jsonl
```

The exact stdout is `responses.jsonl`. Its four canonical records demonstrate
capability discovery plus target, fact, and separately paged sparse-interval
queries. The synthetic archive contains one plain global-store target with two
duplicate-equivalent source facts at distinct catalog ordinals and occurrence
identities. Each fact retains two equal sparse intervals with distinct interval
ordinals and identities. A second guarded global-store target has zero
correlations, demonstrating that structural target discovery does not fabricate
source or ISA facts. Every record retains
`service_provenance:"canonical_self_claimed_archive"`,
`archive_authenticity_proved:false`, and
`producer_evidence_authenticated:false`, together with the remaining
compiler, proof, publication, runtime, hardware, LLVM, and ISA nonclaims.
