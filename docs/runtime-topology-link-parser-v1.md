# Fixed-Schema Topology Link Parser

The private link parser uses thirteen numeric slots and a presence bitmask in
place of a `BTreeMap<String, u64>`. Successful parsing creates no property-key
strings or tree nodes. The existing bounded file reader still allocates its
text buffer; this is not an allocation-free discovery path.

Only IO/P2P link-property parsing changes. Node and platform parsing, fresh
directory/file inspection, no-follow/nonblocking open, opened-file identity,
bounded read, post-read identity, UTF-8 validation, and all full-currentness
boundaries remain unchanged. No cached topology, generation-only shortcut,
retained file descriptor, new unsafe code, or execution authority is added.

## Compatibility

The parser preserves the existing thirteen-key schema and arbitrary key order.
Its first-error order is: final newline, line-count bound, line/decimal syntax,
known key, `u64` overflow, field range, duplicate key, then the first missing key
in schema order. Endpoint validation still precedes latency/bandwidth range
validation; zero maximum retains its existing special meaning.

Differential tests compare every success field and error payload against the
unchanged generic parser with an independent copy of the old schema. They cover
all 8,191 nonempty key subsets, 154 permutation cases, every field's limits,
duplicates, malformed numbers, unknown keys, overflow, and terminator/line-limit
precedence. Integration tests cover both link sets and complete discovery
snapshots with one, two, and three GPUs. Existing file-race and fresh-read tests
remain part of qualification.

The [CPU packet](evidence/dev-topology-link-parser-cpu-2026-09-20/PROTOCOL.md)
records GNU/musl and feature-off tests, source maps, command receipts, linting,
and the unsafe-source inventory. This is regression evidence, not a formal
proof of parser refinement or Linux/driver behavior.

## Performance Boundary

The prior [MI300X attribution](evidence/dev-xgmi-aggregate-attribution-mi300x-2026-09-19/README.md)
put 98.8085% of hot-copy backend time in full opening/closing currentness.
That measurement does not identify parsing's share. Each successful aggregate
call still performs two whole-host discoveries, including every GPU and both
link sets, render/PCI/partition correlation, and boot/kernel/module checks.

No native speedup or HIP/HSA parity is claimed for this change. The next
measurement should split endpoint pre/postchecks, topology-tree discovery,
render correlation, and provenance checks. Endpoint-only discovery,
generation-only reuse, or an epoch spanning independently conclusive calls would
change the currentness contract and require separate design and proof work.
