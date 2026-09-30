# Queued Producer Query Qualification

This packet qualifies the shared outer/child queued-read query bodies, not A2
as a whole, native execution, or HIP/HSA parity.

## Source And Integration

Qualified signed candidate: `200729aa9da3fc6366d8c9d0093c371b482e5d63`.
Its 13-file change was cherry-picked with a new signature as `4d31322f0`.
The complete runtime-model subtree was byte-identical immediately after that
integration, and the four source/controller calibration groups passed locally.
Later retained-pair commits add separate proof files; they do not alter the
queued-query implementation or its complete model-source guard.

The only intended query behavior change returns `InvalidReference` for a missing
read-count slot instead of panicking. Validation order and the full abstract
read-only owner frame are preserved. Locally reciprocal cycles retain their
legacy query behavior; this is not a whole-list validity theorem.

## Results

- Full model library: 1095 passed, zero failed, 19 unchanged manual benchmark
  ignores. The seven focused query groups are included in this total.
- All 29 model doctests passed. Two owner-projection compile-fail examples also
  passed a focused diagnostic check; they are not additional distinct examples.
- Strict all-target/all-feature model Clippy, no-default model compilation,
  scoped formatting, and all changed-file whitespace checks passed.
- Signed campaigns: all 25 child and 11 outer mutations were rejected for the
  intended logical failures. Compile errors and timeouts were not accepted.
- Six complete unfiltered proof runs each passed 161 overlapping obligations
  with zero errors. Repeated runs are not 966 distinct obligations.
- All 56 fresh campaign process groups were observed closed in their live
  recorder's unchanged PID namespace. No historical or host-wide absence claim.

CPU qualification preceded signing, with all 6416 selected input files bound
byte-for-byte to the signed commit. It was not rerun merely because the commit
was signed. Each inherited checker covers 6286 inputs; the wrapper additionally
binds 130 benchmark inputs. The executable proof include closure is 36 files.

The combined runtime after integration has not received a new full-suite or
native qualification. The previously recorded telemetry failures remain open.

## Contents

`raw.tar.gz` contains 8943 regular-file members and 146724138 uncompressed bytes:
all 6416 signed source inputs, seven attempt histories, versioned recorders,
external source/log/public-identity dependencies, raw Git membership evidence,
and the exact inclusion/checksum controls. Opening and closing source hashes,
file modes, archive membership, headers, and every archived content hash passed
the packaging checks recorded in `packaging.json`.

Archive SHA256:
`68fec5a52f5e6164223eab701d90af1050bb152039cfc94762a978b81daa4bd1`.
Compressed size: 31414090 bytes.

The archive preserves both the rejected visibility proof attempt and the
rejected selector attempt. The latter accepted zero logical negatives; its
historical `logical_negatives_executed=1` field denotes a launch attempt, not an
accepted result. Neither failure was rewritten or promoted.

Generated CPU ELF bytes and build targets are deliberately omitted. The original
ELF metadata and hashes remain in the logs; tracked binary source fixtures remain
included. This is a source-and-log record, not a self-contained executable replay
capsule. The omitted ELF cannot be rehashed or executed using only this archive.
Pinned Verus/Z3/Rust tools and their dynamic-loader environment are not bundled
or claimed to be hermetically attested.

`packet-files.json` and `packet.sha256` inside the archive define the full payload
roster. Original absolute paths in historical recorders remain unchanged; the
inventory maps them to archived paths. Do not run an old unsigned recorder on
the signed HEAD: its source/HEAD guards intentionally refer to the original run.
The internal preparation README records the pre-packaging state; `packaging.json`
is the subsequent archive-creation receipt.

Check the published files with `sha256sum --check SHA256SUMS` from this directory.
That checks artifact integrity, not a rerun of the proof or CPU qualification.

## Open Boundaries

Constructor reachability, mutation/admission/cancellation/credit correctness,
complete Context reconciliation, physical overlap, bounded residency, native
device ordering, allocation/OOM/unwind, compiler/ISA correctness, and performance
remain outside this packet. No GPU test or benchmark ran here. All A0-A7 exits
remain open.
