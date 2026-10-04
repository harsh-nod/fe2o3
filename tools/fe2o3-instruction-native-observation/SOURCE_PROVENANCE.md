# Source allowlist and qualification provenance

## New repository leaves only

The proposed addition is confined to
`tools/fe2o3-instruction-native-observation/`. No existing repository file,
protected worker implementation/header/CMake, root build, ABI, policy, worker
registration or publication route is changed. The directory is configured
explicitly and independently; it is not a new compiler backend.

The source owner retained the external worker from harsh-nod/fe2o3 base
`c60cd746e63b34b9072a493744d73b87ed1defc3`. The observed worker subtree is
unchanged against that checkout. [WorkerSourcePins.cmake](WorkerSourcePins.cmake)
lists exact byte lengths and SHA256 values of all twelve files used in that
worker's CMake build-claim preimage, plus the directly included
`tests/OrderedProgramWorkerSupport.inc`. The latter is used from the explicit
external tree; it is not copied here. Those configure-time pins do not authenticate
an executable or SDK and do not replace external before/after custody.

## Byte-identical runtime postimages

The five InstructionSource files come from the private Phase19 instruction-native
draft. DefaultSourceMachine.inc comes from the unchanged retained Phase13 matcher.
These six postimages have **no runtime-code changes**:

| New local file | Bytes | SHA256 |
| --- | ---: | --- |
| InstructionSourceCandidate.cpp | 7200 | `e98673b8b9df44f69c09cb5cfffe368f4f4e02c374672182fd1027f58a12906e` |
| InstructionSourceInput.inc | 10832 | `ecfea3b9a29efddb82def2c363693fe167368e8376cebe743d567b9aaa86a2ea` |
| InstructionSourcePayloads.inc | 6231 | `8e10ec07ecb2db39f3a68f8d088012d029bd87fc556e2db7b9718c042b2924a4` |
| InstructionSourceControls.inc | 8553 | `dcec5d1b4d772bafc3953715e3c3cf9a048cac3958b050491a0013f12b414c17` |
| InstructionSourceMachine.inc | 8109 | `8410be85e84a41213c0f032cb7b559584266c2eb2dfc864ccb95919ea1b2d6aa` |
| DefaultSourceMachine.inc | 8280 | `1878d1f33b42dce5b9b3ebf2384c9ced6e8b20ba8ebc4328e8cddf8e330f7bd0` |

The retained comments and report kinds still say private/test-only. That describes
their non-authoritative transport, not an implicit production admission after
placing source in a repository. DefaultSourceMachine.inc checks literals,
instruction shape and descriptor bytes; it is not a fork of the worker's decoder.

## Reviewable build-only delta

Relative to private Phase19 CMake SHA256
`f91360a290775adb5f918d1d1876c44134e6c85c8aadcee97bc622fad009df82`:

1. Delete the hard-coded task root and equality against one absolute worker path.
2. Require an explicit canonical external worker directory and exact reviewed
   source allowlist before evaluating its unchanged CMake.
3. Resolve DefaultSourceMachine.inc locally, byte-identical, instead of through
   an absolute private Phase13 include directory.
4. Require explicit SDK/version/build-ID/provider paths and a caller-selected
   expected worker claim; compare the worker's generated claim exactly.
5. Scope the project to native Linux, Release, the actual LLVM 22.0.0git static
   component package graph, empty provider directories and out-of-source builds.
6. Replace the forced BUILD_TESTING cache setting with a standalone normal
   variable OFF and add the worker EXCLUDE_FROM_ALL. No test/install/protected
   route is registered; build the same named observer target explicitly.
7. Keep observer compile flags, executable name, worker dependency, runtime
   guard/controls, output schema, exact selected profiles and limits unchanged;
   explicitly map/link AsmParser for the observer's direct parseAssemblyString use
   without relying on the now-disabled worker test-target branch.

The README replaces the old task-specific build instructions. The initial portable
postimage was unconfigured and unrun; the separate Phase20 qualification below
now records a fresh standalone build and execution of this directory. A relocated
SDK/source/build configuration still needs fresh qualification, even when all six
runtime-file hashes match.

## R2 correction: retain the actually qualified SDK graph

Independent review found the first unrun portable draft incorrectly required a
monolithic LLVM target. That original draft is preserved separately. The actual
qualified SDK's LLVMConfig.cmake/LLVMExports.cmake declare static component
targets and no target named LLVM. The passing Phase19 observer link command uses
LLVM and LLD component archives, including LLVMAsmParser; it does not link a
monolithic libLLVM. R2 changes only CMake and these two documentation files:
13 external worker pins and all six runtime postimages remain byte-identical.

The reviewed historical inputs have these SHA256 values:

| Retained input | SHA256 |
| --- | --- |
| Phase19 native instruction-source-candidate.dir/link.txt | `26f29f6b3b4d6cc2f65d23441c85a3a37bd0d15fc2d655d837fad265907f00ea` |
| SDK llvm/LLVMConfig.cmake | `d2e10e24840e435584ea186b4590f23f016b8ded92f4c6602bf22128a5166ef0` |
| SDK llvm/LLVMExports.cmake | `28701231503662a488c89dafc1fd0b74c55a42e16c99c85c85e6769d8de45657` |
| SDK lld/LLDTargets.cmake | `4ece54d26d399eca41f55e758227305f37a0395bca1abe6c3ee76311e398c37c` |

These identify the reviewed historical static configuration, not by themselves
a newly measured portable build or complete SDK/runtime attestation. The following
Phase20 records separately establish the bounded standalone run.

## Phase20 standalone qualification

The canonical candidate's new standalone directory was freshly configured and
built on mi350 using CMake 3.28.3, GNU C++ 13.3.0, Unix Makefiles/Release and two
build jobs. The external worker remained unchanged at the allowlisted source
base above. The configuration used the reviewed LLVM 22.0.0git static component
SDK and empty explicit gfx942/gfx950 provider directories. Neither a monolithic
SDK nor a different host/target was qualified.

The independently configured worker claim and the observer's freshly generated
claim matched exactly:

~~~text
fe2o3-worker-v1-sha256-f5fee9cf41ca39587c47114f112b70681084dba62dd61e3cc79484ed66c7646d
~~~

The observer executable was 107651648 bytes, SHA256
`9d96c264864667429c6275beebe4e8658cd49e1ecdc0ad6243628a6f2ee2f1c7`.
The actual wrapper CMake SHA256 was
`f0bb9eb873841d4c2aa845ff8b8089039ce98d6ffd40fd45094abd2e9b311f9a`;
WorkerSourcePins.cmake was
`fd0dc04e9f7b28503574aec54d2db1ae9362e26ff05633486684917bdab5b906`.
All six runtime-file hashes still match the table above. Configuration claims
remain asserted strings, separate from the executable's measured full-file hash.

### Completed receipt chain

Paths below are retained task evidence locators relative to `ROOT/logs`, not
checked-in files, automatically fetched artifacts or authentication. ROOT for
this run was
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr`.
Each listed passed supervisor has direct/close exit 0, no signal/error and
unchanged before/after selected source/tool/input observations.

| Stage and retained file | Result | Bytes | SHA256 |
| --- | --- | ---: | --- |
| phase20-native-worker-claim-r1/receipt.json | Independent worker configuration passed | 111886 | `f0f6f5aefe6b0524b6cc5a1bba0b35597f17abbefc872a36a4e7936b7a172cab` |
| phase20-native-configure-r1/receipt.json | New standalone observer configuration passed | 119125 | `74fc66c23d105fcd37b207481d02d050ce9ff123cec4579957c52150c9fdd5f2` |
| phase20-native-build-r1/receipt.json | New build plus synthetic shape/identity/stat controls passed | 125385 | `a7bdc94b9f39658cb7bf1e455e5ebd63605467686fa2f691d45f146c0d8b32c6` |
| phase20-static-emitter-prepare-r1/receipt.json | Normal diagnostic emitter rebuilt with static Rust dependency linkage; fresh source seed prepared | 44045 | `b3869bf208ed088d55312cb3d175466b105e1b6930f320523387cf1fee100cc6` |
| phase20-instruction-source-r2/receipt.json | Fresh source runner passed | 36831 | `4c8614c5a4dd927b4fbd1a0579b07c438b4587862a75f765598f89278d107f40` |
| phase20-source-instruction-edit-r2/receipt.json | Actual source acceptance: 3 exports, 90 CPU simulations, 100 stages | 248121 | `067d454fbf270f15838299accc636caaa909d6e76cbe91e7fd4b6eebc2cb7825` |
| phase20-instruction-native-default-r2/receipt.json | New observer default O0/O3 passed | 127636 | `0eaa7f52cdb44dc1d0d7d6126eee70606515eeca5f607b98dc0a756eff42544a` |
| phase20-instruction-native-edited-r2/receipt.json | New observer edited O0/O3 passed | 127628 | `11fa27c9ea0958aa0a1aadf6bb193e641f482e4cf9398b9a920538feb4d79ba5` |
| phase20-instruction-native-join-r1/receipt.json | Strict join supervisor passed | 55394 | `d6840280f8717274a838de5e7e5a3cb71a0d5e8abe7ac10621f42e641a3c85fe` |
| phase20-instruction-native-join-r1/join.json | Actual source/native join: 4 complete HSACOs, 294 input pins/137299031 bytes | 140457 | `66edd64c990d2305affe058c9007c97cd0649879ce3c68ad3bbff710b9717bbc` |
| phase20-native-cmake-refusals-r2/receipt.json | Twelve-case CMake control supervisor passed | 122139 | `acc7148a6bd1c641e6197d169dfef174c9e349bb3cfd2ea7aa6c54fe98a7cd41` |
| phase20-native-cmake-refusals-actual-r1/receipt.json | All 12 exact expected refusals passed | 82053 | `ec245c1ceb04d103db02893c0a3d2cf606dd6690979e321b87a07d525845b152` |

The worker/configure gates used source census 6559 files/99718280 bytes,
SHA256 `129d7565a010bc62fc7dd7294e0750e1d5be1675894ae947e450916ed22b71a0`.
Build/source/native/join/control gates used the later explicit census
6561 files/99789345 bytes,
SHA256 `e1d48a1a29dc3c44e1746deae2db4032ede02ae330017284ab3378e0ab1fdaa6`.
Each gate retained its own unchanged before/after census; these two versions must
not be represented as one identical cross-stage source snapshot. The later
documentation-only qualification refresh is also not part of those earlier
measured source censuses.

### Fresh source and complete native bytes

The source acceptance records exactly one final XOR-to-OR edit; original and
surrounding source bytes were retained unchanged. It independently checked
complete output buffers, guards and initialization for five input triples,
lengths 0/1/65 and two repeats across three normal exports. The source receipt has
287 input pins/137009725 bytes. Its semantic identity join passed; its native,
hardware, proof and production flags remain false, because native evidence is
a separate joined observation.

The new source exports have different compiler-origin identities from the
historical Phase19 export. The raw LLVM selected by the new native commands is:

| Source variant | LLVM bytes | Full LLVM SHA256 |
| --- | ---: | --- |
| Default | 1994 | `9694e30f5ae816e67ee3be122dfedb85c977da414c58d5add07c098a6443b44c` |
| Edited and repeat | 1993 | `cd686930d7e1a06405714965082ca14260112227700204684cdb61987d6a2ada` |

These are not the historical Phase19 LLVM hashes. Default canonical KIR is
`3d68f39bc6d4ae65166c00b3daef343e7149f158c05a29dafed05a4e547ff844`;
edited/repeat is
`26dfd1e94e187824ac94b61e21604a6ddad9fcab9164ecd57052a2d5ad829612`.
Their unchanged root/contract inventory identity is
`ff2858bbe12aa10f9d41307b3976f064a7c4d9bd2ce0d33824fc56cd3df94827`.
Edited/repeat source, semantic MIR, preflight, canonical KIR and LLVM identities
agree; default versus edited body-sensitive identities differ. No native repeat
execution is claimed.

New native stdout pins are default 9059 bytes/SHA256
`f885333185226a6a0976421a4dc61658cb6873629facd1e406cefc9af5f09ea2`
and edited 9054 bytes/SHA256
`7b2a51b677b717ad656c4ea13164090cac9d4fad90f3d9e68afec708593d2568`,
under their respective Phase20 r2 supervisor directories. Each retained two
create-new payloads; the strict join reread every complete HSACO:

| New payload, under phase20-instruction-native-* | Bytes | SHA256 |
| --- | ---: | --- |
| default-r2/payloads/O0.hsaco | 6152 | `0786de8ada4300d144018ac871fe384065b0f225b8e25dc423bc6c8a3454ba41` |
| default-r2/payloads/O3.hsaco | 5384 | `9484ee4d7f5f75730367a49ed960e4608ce07fb76c3415bb91e302f1ddea49c7` |
| edited-r2/payloads/O0.hsaco | 6152 | `f39f619d9db7dc56f72b31dae527b926f9cf65004c2dd8e92092e77331f11a04` |
| edited-r2/payloads/O3.hsaco | 5384 | `e37254dc428d1bdb680fccd3c3f52769caa6b85d24e070aba0d4935c780709cd` |

All four full payload hashes match their historical Phase19 counterparts, but
they were freshly emitted and checked against the new selected LLVM. This
particular byte equality is not a universal whole-kernel stability promise.
Actual instruction ELF file offsets are 2692/2696/2700 at O0 and 2340/2344/2348
at O3. The 64-byte descriptors are at 2368/O0 and 2048/O3. Declared register
high-water 6 is covered by encoded capacities 24/O0 and 8/O3; architected boundary 8
is not a count of live VGPR values, allocator lifetime proof or occupancy result.

### Controls and retained failures

The build's actual shape-controls report passed four typed positives and 48
negatives, one input-identity positive/three negatives and one file-stat
positive/nine negatives. Controls-only did not invoke the worker/target machine.
Each native O0/O3 case also passed seven decoded-field refusals and one each
stale identity, synthetic gapped sequence, raw-byte mismatch and redecoded
opposite opcode refusal. No mutated payload was executed.

The CMake ladder passed twelve exact first-diagnostic refusals: missing and
noncanonical worker/SDK paths, worker size and hash changes, wrong LLVM version,
wrong LLVM claim, wrong worker claim, nonempty provider, in-source build, and a
synthetic monolithic-target conflict. Each direct configure exited 1 normally
with no timeout/signal/truncation; these were expected refusals, not twelve
successful configurations. The graph conflict is a synthetic target injected
beside the real SDK, not qualification of an alternate SDK. The ladder invoked
normal host compiler probes but no worker/observer target build or native run.
Before writing its final receipt it retained 166 files/2289508 bytes and 6993
stream bytes; the final directory has 167 files/2371561 bytes. Its 66 original input pins
and full 2517-file/2279434376-byte SDK census were unchanged. Full input reads
were 6080319432 bytes under the retained 6 GiB ceiling. The SDK manifest SHA256 is
`75e68a7d7a0a69906dce63ff8417915ded71f9cbe859d869b44e7b65d8f7fff2`.

Two failed attempts remain separate evidence, never counted as passes:

1. `phase20-instruction-source-r1/receipt.json`, 22209 bytes,
   SHA256 `5e4f512a3c768fcf9da52e8bfd64f47cd2dc8592f9d0495e4e31b06697123353`.
   The normal LLVM emitter process exited 127 because
   `libstd-fa01d964e82d0da8.so` was unavailable. Its 259-byte
   `phase20-source-instruction-edit-r1/default-llvm.stderr` has SHA256
   `2814a69683b97e72c3d9018adfccd0547a1573a2c5f18dd785da6a617538f429`.
   There was no passed source-acceptance receipt. The emitter was rebuilt from
   the unchanged normal example with static Rust dependency linkage; a new seed
   and a separate r2 source output were used. No opcode, identity or result check
   was weakened.
2. `phase20-instruction-native-default-r1/receipt.json`, 22628 bytes,
   SHA256 `67cf05f2ce6148d10fe076241b796cc4ef63868cd7c8ac6691d4f93ef3a5c2bd`.
   The outer preflight refused an outdated copied source census before any
   native subprocess ran (`command:null`). This is not a native-code failure.
   A new r2 request pinned the actual current census and new r2 payload directory.

The final observed scope is local developer byte/shape/correspondence consistency
for these two exact profiles and this explicit configuration. It is not hardware
execution, whole-kernel native correctness, physical allocation/lifetime proof,
source authentication, compiler/runtime closure attestation, protected/ranked
proof/admission, production publication or milestone closure. All corresponding
report flags remain false/unavailable. The sampled outer resource guards also
do not prove descendant quiescence or privileged-writer resistance.

## Historical actual run, not a portable pass

The private Phase19b source runner observed three fresh exports, 100 child stages
and 90 independent whole-buffer CPU simulations. Its actual receipt was
248601 bytes, SHA256
`cee8b86f0d4c7f22cb0e868ebd0ca7dadf6f1f2740ad4a1ba7befc2239b8f6e9`.
The earlier inventory-inequality failure was retained, then corrected to exact
same-root/contract inventory equality; it was not relabeled as success.

The private native observer produced these complete report streams:

| Retained label | Bytes | SHA256 |
| --- | ---: | --- |
| phase19b-instruction-native-default-r1/command.stdout | 9061 | `7cdfd7f2aa951f74ae016a31204b53bcfdb557ad12cd1353fc3db71434f05bd8` |
| phase19b-instruction-native-edited-r1/command.stdout | 9056 | `28b2156b3ef2b2d0a464ea36616f00ea7bd21abbcb70775301cb44cee0838557` |
| phase19b-instruction-native-join-r1/join.json | 140747 | `5230415719fa0c7c81473d5fea338d5f3a85c7a3a9a91fd55c3900e20165d162` |

That strict join independently read four complete O0/O3 HSACO payloads and their
actual instruction/descriptor file offsets. Its external supervisor receipt was
64140 bytes, SHA256
`9e3c77907c37abce49febe91db7c6b20f67967b27d8f600ad7ed41c6c8e766f5`;
18 pure join-control groups passed separately. These are locators for retained
historical evidence, not files bundled in this source directory.

Historical configured claims were:

~~~text
LLVM: rocm7.2.1-packages-sha256:eb02c62693d6697017195f0abf5ebcf7e58f60e4d2acf8356de2e944bceec540
worker: fe2o3-worker-v1-sha256-f5fee9cf41ca39587c47114f112b70681084dba62dd61e3cc79484ed66c7646d
~~~

They are asserted configuration claims, not measured final binaries or loaded
runtime closure. The worker claim incorporates configuration paths; reusing it
after relocation without independently comparing the actual configuration is
incorrect. Those historical records alone do not establish a new portable build
or native pass. Neither historical nor current observations establish hardware
results, lifetime proof, source authentication, production admission or milestone
closure.
