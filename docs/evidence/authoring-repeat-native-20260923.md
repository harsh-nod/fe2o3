# Bounded-repeat native observation qualification — 2026-09-23

Recorded qualification with independent retained-byte review, 2026-09-23 UTC.
This functional record is not a publication receipt or milestone signoff.
The independent reviewer launched no project tests, builds, compilers, observers
or simulations and changed no candidates.
All nine scoped functional gates are now closed and passed: four per fork
(script controls, configure, build/shape controls and native join), plus the
canonical CMake-refusal gate. Their retained outputs were independently read.
Publication policies and commit/remote readback remain separate later gates.

## Scope and custody

The native observer adds an isolated standalone target and strict source/LLVM
join driver for the existing ordered-repeat source profile. It does not
change the production worker, proof/finalizer, device marker ABI, source
representation, core ELF/MC decoder or old fixed-instruction observers.

Native preparation runs against the original capture roots
candidates/milestones-phase22 and candidates/milestones-phase22-mirror.
Historical source/LLVM receipt paths and all selected file identities are
retained unchanged; no copied-checkout rebinding or hash-only substitution
is used. The 24 repository leaves retained by those captures exclude the
new native leaves, additive native CMake target and documentation changes.

Base compiler main is e7cdd90bb5027b071dcd7ee0fcd334aae440941e;
mirror base is 621f88cdbf9204278a1727c5c67e6d65316a73c2.
Recorded functional source censuses:

| Fork | Files | Bytes | SHA-256 |
| --- | ---: | ---: | --- |
| Canonical | 6599 | 100358997 | 01f9a3776ce626edd2344298fe049d146c6ae60f286ec7ce28476e70c75ba300 |
| Mirror | 6592 | 100297872 | 039755d07151ace3ae5db55d2f37f24f50de20c5a1cda249136e79429a65fb05 |

These are qualification source censuses, not future publication commit IDs
or trusted compiler-closure attestations. Later documentation-only updates
must be distinguished by the primary agent.

All paths in the tables below are relative to the retained task root:

    /home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr

## Historical source and LLVM inputs

Each source capture previously performed four normal Rust exports, 120 CPU
simulations and eight actual frontend refusals in 136 stages, with 406 pins.
Each LLVM capture previously performed four lowerer calls and revalidated the
same 120 CPU results/eight refusals, with zero new simulations and 425 pins.
The Phase24 join may revalidate these retained bytes but must not call them
fresh CPU/source/LLVM executions.

| Receipt | Bytes | SHA-256 |
| --- | ---: | --- |
| logs/phase22w4-compiler-repeat-source-actual-r1/receipt.json | 355805 | 9edfa2fddcbf220191d6fb18feac00d3cfc6107dd8444aa8397d0a7b92265d07 |
| logs/phase22w4-compiler-repeat-llvm-actual-r1/receipt.json | 208610 | 8186ab5c539c20953e0b2b30ed32ac31fb769dec4f78d6821a454ff3fd63ccf0 |
| logs/phase22w4-mirror-repeat-source-actual-r2/receipt.json | 355569 | c5780c3a28c6d84fe0992c39c14c5d61db5b907a532b378c8ddaebfe0deb3880 |
| logs/phase22w4-mirror-repeat-llvm-actual-r1/receipt.json | 208029 | f666a968f4e5c95c94f8a199233dd2c4450a81736c4146c8b59d60e46ee4c7d9 |

All four raw receipt sizes/hashes/status fields and source-to-LLVM owner pins
were independently reread. Canonical source/LLVM selected bytes are
433140798/464882318; mirror selected bytes are 433146346/464887630.
W2/W3 captures are historical records, not replacements for these W4 inputs.

The same three complete source fixtures have SHA-256 hashes
fa7634a5a1bc841db4b2a8ed5240b0184dfce8c79259fad6e04e60c66f5d08af,
a8bd4ddbb76a6e59871f06ce4b3ee958b7b24b6a7cfd95ca0e804c094e3351f1 and
d1d3f3812f459be5583c377c2a1d1690a85b24abb483555ed53b6150453f55c3. Raw KIR and canonical
identities differ between the independently captured forks; they are never
normalized to a shared artificial identity.

| Fork/label | Count | Whole LLVM bytes | Whole LLVM SHA-256 |
| --- | ---: | ---: | --- |
| Canonical one | 1 | 1973 | 3db9975dccf1711821121b9f8658c36a3a15784ea6f53f000ac59dc6d2cd2033 |
| Canonical two | 2 | 2003 | 308b309443865da0d2a296b842d32803649aa5ae283675628201b6fa89ce8e42 |
| Canonical fifteen/repeat | 15 | 2393 | 1b00ed17108b039e40c201804c57b91ba167bfac5f1a6a1785ae974c0dc79ad3 |
| Mirror one | 1 | 1973 | ccd5a9afa1d528d0606f010f0c2e1245080732f8792f9f2cd3fac84401de8ee4 |
| Mirror two | 2 | 2003 | 2d87b9da18da7bde70161cde7605b3e2407894ff4a97a7318cfdcacf451e0d00 |
| Mirror fifteen/repeat | 15 | 2393 | 56085b2a161bffcafee886984a90a09d1c1f39ea62156a0b2d4c8182ee2ebad3 |

The two count15 entries are distinct retained lowerer calls with identical
LLVM bytes within their own fork, not permission to skip a fresh native call.
Each native input must use the corresponding raw LLVM hash, never a preflight,
inventory or canonical KIR hash.

## Completed preparation receipts

Every row below has status command-passed, command exit0, null signal/reason,
no reported errors, unchanged source/tool/input arrays, and independently
checked complete stdout/stderr byte/hash pairs.

| Receipt | Bytes | SHA-256 | Observed result |
| --- | ---: | --- | --- |
| logs/phase24-compiler-native-script-controls-r1/receipt.json | 19736 | cecdf8a8a75759429d7e062998535f315a3b46b3aa4a9b2d3d3116ddcc865300 | 167 passed, zero failed/skipped/cancelled/todo |
| logs/phase24-compiler-native-configure-r1/receipt.json | 119169 | 4d8789aeba8922c6d21773bfb175711942ffa566b7e2ec15c4f5ffd21d62299a | Fresh configure; 78 unchanged selected input pins |
| logs/phase24-compiler-native-build-r1/receipt.json | 127193 | ce02e8a41d1dca0b6694c0353e039be841fc831cba380753205cdfe4e5a852f7 | Fresh new and old observer builds; both shape-control reports |
| logs/phase24-mirror-native-configure-r1/receipt.json | 119160 | 08dfee9b8d523c61cb75c90fb67b754a5473bd2b34cc3bc5a3081e21b69a7e65 | Independent fresh configure; 78 unchanged selected input pins |
| logs/phase24-mirror-native-build-r1/receipt.json | 127115 | 225eda0aa9a9c6efc0f8930ff902d8b5f7b806491a9993109d2770ad8accb39e | Independent new and old observer builds; both shape-control reports |
| logs/phase24-mirror-native-script-controls-r1/receipt.json | 19724 | d7cdfd1ad3fbae0661562f81e11bee18ac47c5296478292340e62b1d500c7790 | 167 passed, zero failed/skipped/cancelled/todo |

Each fork's 167 script tests comprise 23 new native-join pure groups plus 144 relevant
existing controls. Synthetic negative payloads are inert validator fixtures,
not source-authenticated or executable native observations.

Each build retained 84 unchanged selected inputs and zero stderr.
The new observer's controls reported typed shape6positive/78negative,
count selector3/4, input identity1/3 and file snapshot1/9. The unchanged old
observer also reported shape4/48, identity1/3 and snapshot1/9. Both control
reports explicitly say no target-machine/worker compilation or hardware run;
this does not erase the separate preceding build commands.

SDK verification completed before/after configure and build, reporting
2517 files/2279434376 uncompressed bytes and manifest
75e68a7d7a0a69906dce63ff8417915ded71f9cbe859d869b44e7b65d8f7fff2.
The selected LLVM claim is
rocm7.2.1-packages-sha256:eb02c62693d6697017195f0abf5ebcf7e58f60e4d2acf8356de2e944bceec540.
The selected worker claim is
fe2o3-worker-v1-sha256-f5fee9cf41ca39587c47114f112b70681084dba62dd61e3cc79484ed66c7646d.
These are checked build claims plus selected-file custody, not a trusted
transitive/runtime compiler attestation.

## Native join results

Both joins closed passed. Each performed four actual new native observer
invocations (one/two/fifteen/repeat15), eight O0/O3 cases and eight complete
HSACO payloads. Each revalidated four historical source exports, 120 historical
CPU simulations, eight frontend refusals and four historical LLVM lowerings.
Fresh source exports, fresh LLVM lowerings and fresh CPU simulations are all
zero. These counts are per fork, not one shared run described twice.

| Receipt | Bytes | SHA-256 | Status |
| --- | ---: | --- | --- |
| logs/phase24-compiler-repeat-native-join-r1/receipt.json | 136028 | c6ce1b649dff1019d3bb8dab268ea1e45c68df18a04606c4a50526287093d443 | command-passed |
| logs/phase24-compiler-repeat-native-actual-r1/receipt.json | 302531 | 7b13ad313fc51715c45f387ea1258e85365a2a526b66ba88002350fea04c4661 | passed |
| logs/phase24-mirror-repeat-native-join-r1/receipt.json | 135912 | 0b4492a006afeaaeb8540afc88d871b3644759fa8eb7f4a1b79fb8d05150f947 | command-passed |
| logs/phase24-mirror-repeat-native-actual-r1/receipt.json | 301842 | e3f9052feef61b71d9343de1b558fc6fa6e883f938f57000db55d503e29fa3d0 | passed |

The actual joined schema is task-ordered-repeat-source-native-join-v1,
authority observation_only. Outer source/tool/input arrays remain identical;
their streams and inner receipt pins match complete retained bytes.
Each of the four actual child calls exited0 with no signal/reason/stderr.

Canonical retained 444 pins/572895319 bytes and recorded3139151755 reread
bytes; mirror retained444/572900034 and recorded3139173806 reread bytes.
The independent reviewer streamed all444 files in each final ledger, checking
complete hashes and retained stat identities: zero mismatches. All eight
refused KIR outputs in each original source capture remain absent; neither
native output directory has a failure marker.

The two separately built observer executables are each107660264 bytes,
SHA-256 17650c67e1f68c6753e310664f0aa28d2935ec0676cc5173ff7b431f30aeb6d0.
They live in target-milestones-phase24-compiler-native-r1 and
target-milestones-phase24-mirror-native-r1, respectively; exact distinct
filesystem identities are retained in their own joined receipts.

All eight raw stdout reports per both-fork pair match their corresponding
recorded native reports and stage pins. All eight input LLVM files match
their own fork's raw hash and exact command arguments, without substituting
KIR identity. All16 whole native payloads were independently reread, including
every reported instruction word and the entire64-byte descriptor at its ELF
file offset. Each reported sequence has2/3/16/16 instructions, exactly one MOV
followed by N ADDs and unique_sequence_matches=1.

| Optimization, both forks/all counts | MOV file offset | Descriptor file offset | Declared extent | Encoded capacity | Architected boundary |
| --- | ---: | ---: | ---: | ---: | ---: |
| O0 | 2692 | 2368 | 37 | 56 | 40 |
| O3 | 2348 | 2112 | 37 | 40 | 40 |

Each following ADD is four bytes after the preceding instruction. Count15's
last ADD is at2752 for O0 and2408 for O3. MOV bytes are2203427e and all ADD
bytes21474268, matching the separately reviewed GFX9 encoding expectations.
Descriptor hashes are47b932221d3ec007edd1010a7e78d24334bf289682143ae1a73ca802aaf0e75a
at O0 and433daf570ce101ae8dcf440ce5665ebfaef6d3002d0d9e676c0edb61d0a2f767
at O3. Their full64-byte records and rsrc1/rsrc3 words were joined directly to
the retained payloads, not a raw-byte search or fabricated descriptor.

All16 native cases retain kernel ordered_repeat_u32, targetgfx942:xnack-,
wave64, required/max workgroup64, code-object6, kernarg_size288/alignment8
and group/private size0. These are actual bounded metadata observations;
the unexpected-looking ABI padding is not normalized to a guessed32-byte
signature layout. Encoded capacity is not allocator/lifetime or occupancy proof.

### Complete payload pins

The corresponding canonical/mirror payloads are byte-identical for this
finite set despite their distinct retained LLVM hashes. The table applies
to both actual roots, logs/phase24-{compiler,mirror}-repeat-native-actual-r1.
Each label has its own LABEL-payloads/O0.hsaco and O3.hsaco files.

| Label(s) | Optimization | Whole bytes | SHA-256 |
| --- | --- | ---: | --- |
| one | O0 | 6224 | e9c97e7715d7ae0f9bd5a795fa099025199b7ab77619267100fa640cb4eee6c9 |
| one | O3 | 5456 | b887cc57f057c0460f4dc62e7c4c364e3898a91909c00202e9b4650d966a634a |
| two | O0 | 6224 | 44593f3cdea0680607793c58cdaf5e5be93fa7bf49bfccff3666038f4c3e5d70 |
| two | O3 | 5456 | 623d9fd9af95759ce7271202139ce715bfaa0e81e6769ced747642948de638a0 |
| fifteen and repeat | O0 | 6288 | 9b2dcccb95cdd5bbccb46276219ab94c186a7191de4ad7a285eb5c05e0384381 |
| fifteen and repeat | O3 | 5520 | 4ccde21d79ad6c08118219d334ed570684775012702a231d9efb62c46e959df5 |

Fifteen/repeat are separate native invocations and files, not aliases or
deduplicated execution. Their complete byte equality was checked at both
optimization levels in each fork. No universal or future byte stability is
claimed; matching binaries or payloads do not merge independent custody.

Every native case retained the exact mutation-control roster:28 decoded-field
refusals, one each stale identity/gap/duplicate/extraADD/wrongcount/raw-byte/
redecoded-opposite-opcode refusal, one synthetic-view positive, and descriptor
capacity1positive/3refusals. The opposite-opcode control redecoded a separately
mutated payload; mutated_payload_executed_on_hardware remainsfalse. None of
these negative controls changes or executes the retained original payload.

## Canonical CMake refusal controls

| Receipt | Bytes | SHA-256 | Status |
| --- | ---: | --- | --- |
| logs/phase24-compiler-native-cmake-refusals-r2/receipt.json | 128462 | 92452b8822a4864866a406912b97fa8ea2326be2bb332465d3a250f2d19644cb | command-passed |
| logs/phase24-native-cmake-refusals-actual-r2/receipt.json | 84532 | 1ded251d9c99dc8451a8dc9701caad0f1906b5e5ba1d518408bebff9b8c567ca | passed |

All12 real configure commands produced the exact intended first diagnostic:
missing/noncanonical worker, missing/noncanonical SDK, worker-size/hash
mismatch, LLVM version/claim mismatch, worker claim mismatch, nonempty
device-library provider, in-source build, and monolithic SDK graph.
Each negative exited1 with null signal/error and untruncated streams.
The reviewer reread all24 raw stdout/stderr files; every recorded size/hash
matches and the combined6993 bytes equal the receipt census.
These are successful refusal controls, not failed qualification gates.

The outer86 inputs/source/tools are unchanged; the inner records69 selected
pins, unchanged SDK2517files/2279434376bytes,6081499858 input bytes read,
and166 outputfiles/2289508 bytes. Configure compiler probes occurred, but no
worker/observer target build, native observer execution or hardware execution
occurred within this refusal gate.

R1 was prepared but NEVER executed. Its copied helper had historical
pre-publication README/SOURCE_PROVENANCE pins. Before any refusal execution,
the primary agent checked the current two documents against committed base
e7cdd90 and made a separate R2 helper changing only those two pin rows:

- README.md:17261 bytes,
  f8cf5e0a42cbc457aacd8f197d6ab103e29b5e59a4077b88daa1f7cbab7ee966.
- SOURCE_PROVENANCE.md:17900 bytes,
  f52c4eed004917410aaf5d792ac0449b26992bd184fc596fe577d248b0ddada3.

The reviewer independently compared the full R1/R2 helper diff: no predicate,
limit, code pin or case change. R2 helper is27346 bytes,
SHA-25650d41906c52cc38ec89b969d824b51f5e6ec3c314f1aa634fd9baf404a16d37c.
The original R1 draft/request remains unrun, neither passed nor failed.

## Failures, bounds and limits

No Phase24 functional gate failure was observed among these nine closed gates.
The unrun CMake R1 preparation is not a failed attempt. Prior Phase22 failures
and baseline strict Clippy failure remain retained historical evidence,
unaffected by this additive native work. No strict-lint pass or rerun is
claimed here; later publication-policy results are outside this report.

The native join has four bounded native commands, 180s per command and 19min
cooperative total; 64KiB streams, 1MiB complete payload/receipt caps, 600 selected
pins/3GiB unique bytes/12GiB rereads and output32MiB/32files/5directories.
The C++ observer has its own90s alarm. External qualification lock, source and
SDK/build pins, process-group timeout and full retained-root resource guards
remain separate requirements; sampled observations do not prove descendant
quiescence or complete transitive runtime closure.

The profile is one MOV plus N ADD instructions at count1/2/15, exact target
gfx942:xnack-, wave64/workgroup64/code-object6, kernel ordered_repeat_u32,
register plan[32,33,34,35,36] and declared extent37. The source requests
compile-time instruction repetition, not runtime-loop scheduling/unrolling.

The passed final native joins establish only this finite set of
retained-byte, decoded-instruction and encoded-resource observations:
no GPU execution, native numerical correctness, allocator/lifetime proof,
runtime physical values, source authentication, protected proof/finalizer
admission, generic source lifting, performance or universal byte stability.
The production/proof/hardware/native-whole-correctness flags remain false,
runtime_closure_attestation unavailable, and native_qualified=false.
Broad accepted milestones remain 3/18 (M1, V1, U1); M2 and U2 are not closed.
Commit and main readback are tracked separately from this functional record.
