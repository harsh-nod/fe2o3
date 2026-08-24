# fe2o3-kir-sim

fe2o3-kir-sim is the standalone, Linux-only command-line boundary for bounded
deterministic CPU execution of exact verified canonical Kernel IR V6:

    fe2o3-kir-sim --kir-v6 kernel.kir --request request.json
    fe2o3-kir-sim --kir-v6 kernel.kir --request request.json --output result.json

It does not link or initialize HSA, HIP, KFD, ROCm, or a GPU. Simulation is an
observation only. It grants no source-refinement, proof, compiler, artifact,
load, launch, GPU-equivalence, race-freedom, timing, performance, or performance
prediction authority.

Inputs are regular files opened on Linux with openat2, O_NOFOLLOW, O_NONBLOCK,
and RESOLVE_NO_SYMLINKS|RESOLVE_NO_MAGICLINKS. FIFOs, devices, symlinks in any
path component, oversized files, and files changed while read are rejected.
Non-Linux platforms fail rather than using a weaker path.

An output path is published beneath a pinned no-symlink parent using a retained
anonymous 0600 O_TMPFILE inode, complete buffered write, file fsync,
linkat through a retained descriptor entry beneath a pinned, authenticated
procfs mount, and parent-directory fsync. The descriptor entry is matched to the
anonymous inode before publication. The final link inherently does not replace
an existing regular file or symlink. After a successful link, the CLI never
attempts a racy name-based rollback in a mutable parent. A later failure reports
whether durability is unknown or the published name is uncertain, and callers
must resolve that explicit state. There is no attacker-visible staging name;
filesystems or procfs setups without these primitives fail closed.

## Request V1

The strict fe2o3-simulation-request-v1 JSON shape rejects duplicate and unknown
fields:

    {
      "schema": "fe2o3-simulation-request-v1",
      "kernel": "fill",
      "grid": [4, 1, 1],
      "workgroup": [2, 1, 1],
      "shared_buffers": [
        {
          "id": 7,
          "element": "u32",
          "access": "read_write",
          "alignment": 4,
          "bytes": "0x00000000000000000000000000000000"
        }
      ],
      "arguments": [
        {"kind": "scalar", "type": "u32", "bits": "0x0000002a"},
        {
          "kind": "buffer_view",
          "backing": 7,
          "element": "u32",
          "access": "read_write",
          "alignment": 4,
          "byte_offset": 0,
          "elements": 4
        }
      ]
    }

Scalar types are bool, signed and unsigned 8/16/32/64/128-bit integers, and
64-bit index. Bits use 0x plus exactly the type width in lowercase hexadecimal;
bool uses one digit. Floats are rejected. Buffer bytes use 0x followed by
lowercase even-length hexadecimal. initialized is optional; when present it is
an exact 0x-prefixed byte bitset, least-significant bit first, with bit N
describing buffer byte N and unused high bits zero. Omission means all bytes are
initialized. Shared buffers use the same exact codec and byte budgets. A
buffer_view names one shared backing plus an aligned byte offset and element
extent; multiple views may intentionally overlap.

Files are bounded to 16 MiB, arguments and shared buffers to 4,096 each, one
decoded buffer to 4 MiB, and all distinct and shared decoded buffers together
to 16 MiB. Success is streamed as bounded deterministic
fe2o3-simulation-result-v1 JSON. Every failure is stable
fe2o3-simulation-error-v1 JSON on stderr. Parsing failures use closed application
codes selected from private structural markers, while other malformed JSON is
classified by serde's closed syntax/data categories. Input failures identify
kir_v6 or request. Dynamic failures include exact invocation hierarchy and Kernel IR
site coordinates; overlong function identities carry an explicit bounded
prefix, original byte count, and truncation flag. Unsupported preflight failures
report exact total/emitted/truncated counts and a deterministic
encoded-byte-bounded prefix with closed feature codes. Post-publication failures
include a closed publication_state.

The immutable CLI simulation profile caps one allocation at 16 MiB, all live
allocations at 64 MiB, successfully admitted and accepted preflight/execution
resident peaks at 256 MiB, logical
invocations at 1,048,576, scheduled slots at 4,194,304, and execution steps at
134,217,728. Call depth is capped at 64 and live SSA values in one frame at
4,096 so their conservative resident-memory product remains within the host
budget. The 256 MiB setting is not enforced before canonical construction or
decode: verified-owner construction and a simulator decode/re-encode later
rejected by the post-decode resident check may transiently exceed it. Those
phases remain bounded by the 16 MiB canonical input limit and frozen KIR
wire/count/depth caps.

## Result V1

Success contains status ok, authority observation_only, the exact V6 SHA-256
and canonical byte length, all execution counters including padded scheduled
slots, the deterministic schedule identity, bounded cross-invocation conflict
assessment, copied argument values, and copied shared backing buffers and views.
Scalar bits, buffer bytes, and initialization bitsets retain their exact typed
lowercase hexadecimal encodings. Result bytes are measured exactly and capped
at 64 MiB before output publication begins, then emitted directly through a
bounded writer rather than assembled as one JSON string.
