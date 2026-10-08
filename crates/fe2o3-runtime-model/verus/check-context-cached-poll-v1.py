"""Source correspondence only. Never invokes Cargo, rustc, or Verus."""
import hashlib
import json
import os
import stat
from pathlib import Path
import re
import types

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
CONTEXT = Path('crates/fe2o3-runtime/src/context.rs')
UNPUBLISHED = CONTEXT.parent / 'context/unpublished.rs'
BODY = CONTEXT.parent / 'context/cached_poll_body.rs'
PROOF = Path('crates/fe2o3-runtime-model/verus/context_cached_poll_v1.rs')
PROOF_SHA = '6f5eb3581bfa24de99759aad3b59e1d1ba3337675c404d1acf767c41ecaea69f'
LEXER = ROOT / 'crates/fe2o3-runtime-model/verus/check-negative-quality.py'
LEXER_SHA = '74c83c0776206e6c486ebb0bf9fbf6bcfdf207a1f86c22bed6054b5f7438c5fd'
BASE_PINS = {
    CONTEXT: 'dcc242b6e0b780eaec86b924d2d3ee7599e24585f80d7de7529857f6ec4c75e1',
    UNPUBLISHED: '7f205fc8f12d555559d3dc3e36e537247ea738cd7f058269301a8d7984b2dd75',
}
PAIRS = {
    'submission_record': 'cached_submission_record_body_v1',
    'live_submission_record': 'cached_live_submission_body_v1',
    'unheld_stream_v1': 'cached_unheld_stream_body_v1',
    'is_terminal': 'cached_status_terminal_body_v1',
    'legacy_poll': 'cached_status_legacy_body_v1',
    'observe_status': 'cached_observe_status_body_v1',
}


def need(value, message):
    if not value:
        raise ValueError(message)



LIB = CONTEXT.parent / 'lib.rs'
IDENTITY_TESTS = CONTEXT.parent / 'context/tests/submission_identity_tests.rs'
CACHED_TESTS = IDENTITY_TESTS.with_suffix('') / 'cached_poll.rs'
FILES = (CONTEXT, UNPUBLISHED, BODY, PROOF, LIB, IDENTITY_TESTS, CACHED_TESTS)
MANIFEST = HERE / 'pins/CONTEXT_CACHED_POLL_INPUTS_V1.json'
MANIFEST_PIN = HERE / 'pins/CONTEXT_CACHED_POLL_INPUTS_V1_SHA256'
REFERENCE = HERE / 'pins/CONTEXT_CACHED_POLL_PREDECESSOR_V1.json'
REFERENCE_SHA = '4d2f5c6b2e264348b5487215489981331f67045342b83cd179c24ce11c9604af'
SUPPORT = HERE / 'pins/CONTEXT_CACHED_POLL_SUPPORT_V1.json'
SUPPORT_PIN = HERE / 'pins/CONTEXT_CACHED_POLL_SUPPORT_V1_SHA256'
SUPPORT_FILES = (
    HERE / 'check-context-cached-poll-v1.py',
    HERE / 'test-context-cached-poll-v1.py',
    HERE / 'context-cached-poll-mutations-v1.py',
    HERE / 'context-cached-poll-diagnostics-v1.py',
    HERE / 'test-context-cached-poll-diagnostics-v1.py',
    HERE / 'qualify-context-cached-poll-v1.py',
    HERE / 'pins/CONTEXT_CACHED_POLL_DIAGNOSTICS_V1.json',
    HERE / 'pins/CONTEXT_CACHED_POLL_TOOLCHAIN.toml',
    ROOT / 'scripts/qualify-runtime-production-proofs.sh',
    ROOT / 'scripts/tests/runtime-production-proof-pipeline.py',
    HERE / 'CONTEXT_CACHED_POLL_BOUNDARY.md',
    REFERENCE, MANIFEST, MANIFEST_PIN,
)


def ordinary(path):
    need(path.is_file() and not path.is_symlink() and path.resolve() == path,
         'ordinary canonical input: ' + str(path))
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        before = os.fstat(fd)
        need(stat.S_ISREG(before.st_mode) and before.st_size <= 1024**2,
             'bounded regular input')
        with os.fdopen(fd, 'rb', closefd=False) as handle:
            raw = handle.read(1024**2 + 1)
        after = os.fstat(fd)
        identity = lambda value: (value.st_dev, value.st_ino, value.st_mode, value.st_size,
                                 value.st_uid, value.st_gid, value.st_mtime_ns, value.st_ctime_ns)
        need(len(raw) == before.st_size and identity(before) == identity(after),
             'stable complete input')
        return raw, stat.S_IMODE(before.st_mode)
    finally:
        os.close(fd)


def unique_json(raw):
    def pairs(items):
        result = {}
        for name, value in items:
            need(name not in result, 'duplicate JSON key')
            result[name] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs)


def record(raw, mode):
    return dict(bytes=len(raw), mode=mode, sha256=hashlib.sha256(raw).hexdigest())


def reference():
    raw, mode = ordinary(REFERENCE)
    need(mode == 0o644 and hashlib.sha256(raw).hexdigest() == REFERENCE_SHA,
         'exact authenticated predecessor projection')
    selected = unique_json(raw)
    need(selected['original_sha256'] == {str(p): h for p, h in BASE_PINS.items()},
         'unchanged predecessor provenance')
    return selected


def validate_input_roster(rows):
    need(set(rows) == {str(p) for p in FILES}, 'closed seven-file source roster')
    return rows


def snapshot():
    raw, mode = ordinary(MANIFEST)
    sidecar, sidecar_mode = ordinary(MANIFEST_PIN)
    need(mode == sidecar_mode == 0o644 and sidecar == (hashlib.sha256(raw).hexdigest() + '\n').encode(),
         'input manifest byte pin')
    rows = validate_input_roster(unique_json(raw))
    values = {}
    for path in FILES:
        contents, file_mode = ordinary(ROOT / path)
        need(record(contents, file_mode) == rows[str(path)], 'exact source bytes/mode: ' + str(path))
        values[path] = contents.decode()
    return values


def support_check():
    raw, mode = ordinary(SUPPORT)
    sidecar, sidecar_mode = ordinary(SUPPORT_PIN)
    need(mode == sidecar_mode == 0o644 and sidecar == (hashlib.sha256(raw).hexdigest() + '\n').encode(),
         'support manifest byte pin')
    actual = {}
    for path in SUPPORT_FILES:
        contents, file_mode = ordinary(path)
        actual[str(path.relative_to(ROOT))] = record(contents, file_mode)
    need(unique_json(raw) == actual, 'closed support roster and all transitive manifest pins')
    return actual


def root_item_prefix(prefix, message):
    stack = []
    closing = {')': '(', ']': '[', '}': '{'}
    for token in prefix:
        if token in '([{':
            stack.append(token)
        elif token in closing:
            need(stack and stack.pop() == closing[token], 'balanced item prefix')
    need(not stack and (not prefix.strip() or prefix.rstrip()[-1] in ';}'), message)


def root_module(source, name):
    code = CODE(source)
    hits = list(re.finditer(r'\bmod\s+' + re.escape(name) + r'\s*;', code))
    need(len(hits) == 1, 'one active module ' + name)
    prefix = code[:hits[0].start()]
    prefix = re.sub(r'\bpub(?:\s*\([^()]*\))?\s*$', '', prefix)
    root_item_prefix(prefix, 'root module has no hidden attributes/path ' + name)


def module_edges(values):
    root_module(values[LIB], 'context')
    root_module(values[CONTEXT], 'unpublished')
    code = CODE(values[CONTEXT])
    hits = list(re.finditer(r'#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+tests\s*\{', code))
    need(len(hits) == 1, 'exact cfg(test) inline test owner')
    root_item_prefix(code[:hits[0].start()], 'root test owner has no additional attribute')
    # Keep lexical boundaries for the nested module declaration.
    content, _ = block(code, hits[0].end())
    root_module(content, 'submission_identity_tests')
    root_module(values[IDENTITY_TESTS], 'cached_poll')


def root_include(text, include):
    need(text.count(include) == 1, 'exact shared include')
    marker = '__cached_poll_include__'
    need(marker not in text, 'reserved include marker')
    active = CODE(text.replace(include, marker + ';'))
    hits = list(re.finditer(r'\b' + marker + r'\s*;', active))
    need(len(hits) == 1, 'active shared include')
    root_item_prefix(active[:hits[0].start()], 'root unqualified include')


def lexer():
    raw, _ = ordinary(LEXER)
    need(hashlib.sha256(raw).hexdigest() == LEXER_SHA, 'original lexical helper')
    module = types.ModuleType('cached_poll_lexer')
    module.__file__ = str(LEXER)
    exec(compile(raw, str(LEXER), 'exec'), module.__dict__)
    return module.code_only


CODE = lexer()


def compact(source):
    return re.sub(r',([)}])', r'\1', re.sub(r'\s+', '', CODE(source)))


def block(source, start):
    depth, end = 1, start
    while depth and end < len(source):
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    need(depth == 0, 'balanced declaration')
    return source[start:end - 1], end


def method(source, name, proof=False):
    code = CODE(source)
    if name in ('is_terminal', 'legacy_poll'):
        start = code.index('impl RuntimeCompletionStatusV1')
        source = source[start:]
        code = code[start:]
    hits = list(re.finditer(r'\bfn\s+' + name + r'\b', code))
    need(len(hits) == 1, 'unique method ' + name)
    if proof:
        opening = re.search(r'(?m)^[ \t]*\{', code[hits[0].end():])
        need(opening is not None, 'separate proof body ' + name)
        start = hits[0].end() + opening.end() - 1
    else:
        start = code.index('{', hits[0].end())
    body, end = block(code, start + 1)
    return body


def fields(source, name):
    code = compact(source)
    hits = list(re.finditer(r'struct' + name + r'(?:<[^{};]+>)?\{', code))
    need(len(hits) == 1, 'unique fields ' + name)
    body, _ = block(code, hits[0].end())
    parts, start, depth = [], 0, 0
    for index, char in enumerate(body):
        depth += (char in '(<[{') - (char in ')>]}')
        if char == ',' and depth == 0:
            parts.append(body[start:index]); start = index + 1
    parts.append(body[start:])
    result = {}
    for part in parts:
        name, kind = re.sub(r'^pub(?:\([^)]*\))?', '', part).split(':', 1)
        need(name not in result, 'distinct fields')
        result[name] = kind
    return result


def macro_body(source, name):
    code = CODE(source)
    marker = 'macro_rules! ' + name
    need(code.count(marker) == 1, 'unique body ' + name)
    start = code.index('$syntax!({', code.index(marker)) + len('$syntax!({')
    body, _ = block(code, start)
    body = body.replace('$context', 'self').replace('$submission', 'submission')
    if name in ('cached_status_terminal_body_v1', 'cached_status_legacy_body_v1'):
        body = body.replace('$status', 'self')
    elif name == 'cached_observe_status_body_v1':
        body = body.replace('submission', 'self').replace('$status', 'status')
    return body.replace('$stream', 'stream')


def audit(inputs):
    reference_data = reference()
    module_edges(inputs)
    context, unpublished, body, proof = (inputs[p] for p in (CONTEXT, UNPUBLISHED, BODY, PROOF))
    need(hashlib.sha256(proof.encode()).hexdigest() == PROOF_SHA,
         'exact reviewed semantic proof and contracts')
    for text, include in (
        (context, 'include!("context/cached_poll_body.rs");'),
        (proof, 'include!("../../fe2o3-runtime/src/context/cached_poll_body.rs");'),
    ):
        root_include(text, include)
    need('macro_rules!cached_' not in compact(context + unpublished), 'no runtime body shadow')
    code = compact(body)
    declarations = []
    for hit in re.finditer(r'macro_rules!([A-Za-z_0-9]+)\{', code):
        contents, end = block(code, hit.end())
        declarations.append(code[hit.start():end])
        name = hit.group(1)
        if name == 'cached_poll_rust_expr':
            need(contents == '($body:expr)=>{$body};', 'identity Rust syntax adapter')
            continue
        args = {
            'cached_status_terminal_body_v1': '$syntax:ident,$status:ident',
            'cached_status_legacy_body_v1': '$syntax:ident,$status:ident',
            'cached_observe_status_body_v1': '$syntax:ident,$submission:ident,$status:ident',
            'cached_unheld_stream_body_v1': '$syntax:ident,$context:ident,$stream:ident',
        }.get(name, '$syntax:ident,$context:ident,$submission:ident')
        opening = '(' + args + ')=>{$syntax!({'
        need(contents.startswith(opening), 'one unqualified macro arm ' + name)
        payload, end = block(contents, len(opening))
        need(contents[end:] == ')};', 'no additional macro arm ' + name)
    need(''.join(declarations) == code and len(declarations) == 8,
         'closed eight-macro source with no attributes or extra declarations')
    need(set(re.findall(r'macro_rules!([A-Za-z_0-9]+)\{', code)) ==
         set(PAIRS.values()) | {'cached_poll_prefix_body_v1', 'cached_poll_rust_expr'}, 'closed macro roster')
    for name, macro in PAIRS.items():
        owner = UNPUBLISHED if name == 'unheld_stream_v1' else CONTEXT
        need(compact(macro_body(body, macro)) == reference_data['methods'][str(owner)][name],
             'exact original body ' + name)
        arguments = {
            'unheld_stream_v1': 'self,stream', 'is_terminal': 'self',
            'legacy_poll': 'self', 'observe_status': 'self,status',
        }.get(name, 'self,submission')
        need(compact(method(inputs[owner], name)) == macro + '!(cached_poll_rust_expr,' + arguments + ')',
             'actual runtime wrapper ' + name)
        need(compact(method(proof, name, proof=True)) == macro + '!(verus_exec_expr,' + arguments + ')',
             'actual proof wrapper ' + name)
    prefix = compact(macro_body(body, 'cached_poll_prefix_body_v1'))
    need(prefix == 'letrecord=self.live_submission_record(submission)?;self.require_stream_unheld_v1(record.stream)?;'
         'ifrecord.status.is_terminal(){returnOk(Some(submission.observe_status(record.status)));}Ok(None)',
         'closed read-only prefix')
    new_poll = compact(method(context, 'poll_with_graph_access_v1'))
    need(new_poll == 'self.require_graph_access(access)?;'
         'ifletSome(status)=self.poll_cached_status_v1(submission)?{returnOk(status);}'
         'letstatus=self.observe_completion_step_v1(submission.id,|backend,id|backend.poll_v1(id))?;'
         'Ok(submission.observe_status(status))', 'outer gate and fresh tail unchanged')
    old_poll = reference_data['methods'][str(CONTEXT)]['poll_with_graph_access_v1']
    need(old_poll == 'self.require_graph_access(access)?;letrecord=self.live_submission_record(submission)?;'
         'self.require_stream_unheld_v1(record.stream)?;ifrecord.status.is_terminal(){returnOk(submission.observe_status(record.status));}'
         'letstatus=self.observe_completion_step_v1(submission.id,|backend,id|backend.poll_v1(id))?;'
         'Ok(submission.observe_status(status))', 'original split point')
    for text, syntax in ((context, 'cached_poll_rust_expr'), (proof, 'verus_exec_expr')):
        need(compact(method(text, 'poll_cached_status_v1', proof=syntax == 'verus_exec_expr')) ==
             'cached_poll_prefix_body_v1!(' + syntax + ',self,submission)', 'prefix is reached')
    need(reference_data['methods'][str(UNPUBLISHED)]['require_stream_unheld_v1'] ==
         'self.unheld_stream_v1(stream).map(|_|())', 'exact original unheld wrapper')
    need(compact(method(unpublished, 'require_stream_unheld_v1')) ==
         compact(method(proof, 'require_stream_unheld_v1', proof=True)) ==
         'self.unheld_stream_v1(stream).map(|_held_stream|())',
         'exact shared unused binder; no alternate wrapper')
    for name in ('RuntimeContextV1', 'SubmissionRecordV1', 'RuntimeSubmissionV1', 'StreamRecordV1'):
        base_fields = reference_data['fields'][name]
        need(fields(context, name) == base_fields, 'unchanged actual field types ' + name)
        projected = fields(proof, name)
        need(set(projected) == set(base_fields), 'complete field projection ' + name)
        for field, kind in projected.items():
            need(kind == base_fields[field] or kind in ('O', 'A', 'T', 'Option<W>', 'Option<R>', 'Option<P>')
                 or (field == 'submissions' and kind == 'HashMap<RuntimeSubmissionIdV1,SubmissionRecordV1<W,R,P>>'),
                 'explicit projection type ' + name + '.' + field)
    code = compact(proof)
    for name in ('RuntimeCompletionFailureV1', 'RuntimeCompletionStatusV1', 'RuntimePollV1'):
        definitions = []
        for text in (context, proof):
            tokens = compact(text)
            marker = 'enum' + name + '{'
            need(tokens.count(marker) == 1, 'one status enum ' + name)
            definitions.append(block(tokens, tokens.index(marker) + len(marker))[0])
        need(definitions[0] == definitions[1] == reference_data['enums'][name], 'complete status enum ' + name)
    for name, value in (('RUNTIME_CANCELLED_CODE_V1', '-2'), ('RUNTIME_QUIESCENT_WITHOUT_RESULT_CODE_V1', '-3')):
        need(reference_data['constants'][name] == value, 'unchanged predecessor status constant')
        declaration = 'const' + name + ':i64=' + value + ';'
        need(all(compact(text).count(declaration) == 1 for text in (context, proof)),
             'exact legacy status constant ' + name)
    need(code.count('usestd::collections::HashMap;') == 1, 'real std map')
    need(not re.search(r'(?:struct|enum|type|trait|mod|macro_rules!)(?:HashMap|std)\b', code), 'no map shadow')
    need(not re.search(r'\b(?:assume\w*|admit\w*|axiom|external\w*|verifier|unsafe|extern|cfg|cfg_attr|trait)\b', CODE(proof)),
         'no new trust declaration or inactive obligations')
    need(not re.search(r'macro_rules!cached_', code), 'no proof body shadow')
    requires = re.findall(r'requires(.*?)ensures', code)
    need(requires == [
        'vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),', 'key_contracts_v1(),',
        'vstd::std_specs::hash::obeys_key_model::<RuntimeStreamIdV1>(),',
        'vstd::std_specs::hash::obeys_key_model::<RuntimeStreamIdV1>(),',
        'key_contracts_v1(),', 'key_contracts_v1(),'], 'only explicit std key prerequisites')
    need(compact(method(proof, 'key_contracts_v1')) ==
         '&&&vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>()'
         '&&&vstd::std_specs::hash::obeys_key_model::<RuntimeStreamIdV1>()', 'no hidden key premise')
    for name in ('RuntimeSubmissionIdV1', 'RuntimeStreamIdV1', 'RuntimeDeviceIdV1', 'RuntimeAllocationIdV1'):
        need(fields(proof, name) == {'context_generation': 'u64', 'local': 'u64'}, 'complete identity ' + name)
    need(compact(method(proof, 'cached_prefix_frames_owner_v1', proof=True)) == 'context.poll_cached_status_v1(submission)',
         'whole-owner wrapper calls actual shared helper')
    need('*final(context)==*old(context)' in code and 'token_frame_v1(*old(submission),*final(submission))' in code,
         'explicit owner and token frame obligations')
    need('before.peer_transfer==after.peer_transfer' in code and 'before.marker==after.marker' in code,
         'opaque token custody framed')
    return {'actual_shared_bodies': 7, 'context_fields': 31, 'record_fields': 15, 'token_fields': 7,
            'cargo_ran': False, 'verus_ran': False, 'native_ran': False}


def inputs():
    return snapshot()


if __name__ == '__main__':
    import json
    support_check()
    print(json.dumps(audit(inputs()), sort_keys=True))
