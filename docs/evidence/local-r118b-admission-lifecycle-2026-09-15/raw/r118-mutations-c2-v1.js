// Prospective behavioral negatives. These definitions do not establish execution.
const path = 'crates/fe2o3-runtime/src/generated_source.rs';
const authorityPath = 'crates/fe2o3-runtime/src/authorized_execution.rs';
const oracle_path = 'crates/fe2o3-runtime/src/authorized_execution/tests/generated_identity.rs';
const prefix = 'authorized_execution::tests::generated_identity::';
const tests = {
  D: prefix + 'generated_descriptor_matches_bind_every_coordinate_bidirectionally',
  T: prefix + 'generated_identity_survives_transfer_and_rejects_identical_replacement',
  A: prefix + 'generated_transferred_source_rejects_later_artifact_substitution',
  U: prefix + 'generated_transferred_source_rechecks_authority_and_currentness',
};
const mutations = [];
function add(name, alias, line, expected, edits, target = path) {
  mutations.push({name: 'c2-' + name, kind: 'production', test: tests[alias],
    oracle_path, oracle_line: line, expected: [expected], patches: [{path: target, edits}]});
}
add('replace-source-identity', 'T', 317, 'transferred replacement matched original', [[
  'std::sync::Arc::ptr_eq(&self.source_identity, &other.source_identity)',
  'std::sync::Arc::ptr_eq(&self.source_identity, &self.source_identity)',
]]);
add('asymmetric-count', 'D', 286, 'reverse descriptor match: Count', [[
  'self.count == other.count', 'self.count >= other.count',
]]);
for (const [name, field, label] of [
  ['omit-readback', 'readback_bytes', 'Readback'],
  ['omit-fixups', 'fixup_count', 'Fixups'],
  ['omit-dispatch', 'dispatch_contract_sha256', 'Dispatch'],
]) add(name, 'D', 282, 'forward descriptor match: ' + label, [[
  '&& self.' + field + ' == other.' + field, '',
]]);
for (const [name, predicate, label] of [
  ['omit-buffer-ordinal', 'a.bytes == b.bytes && a.access == b.access', 'Ordinal(0)'],
  ['omit-buffer-bytes', 'a.ordinal == b.ordinal && a.access == b.access', 'Bytes(0)'],
  ['omit-buffer-access', 'a.ordinal == b.ordinal && a.bytes == b.bytes', 'Access(0, ReadOnly)'],
]) add(name, 'D', 282, 'forward descriptor match: ' + label, [[
  'self.buffers == other.buffers',
  `self.buffers.iter().zip(other.buffers.iter()).all(|(a, b)| match (a, b) {
                (Some(a), Some(b)) => ${predicate},
                (None, None) => true,
                _ => false,
            })`,
]]);
add('ignore-missing-buffer', 'D', 282, 'forward descriptor match: Missing(0)', [[
  'self.buffers == other.buffers',
  'self.buffers.iter().zip(other.buffers.iter()).enumerate().all(|(i, (a, b))| (i < self.count && (a.is_none() || b.is_none())) || a == b)',
]]);
add('ignore-trailing-buffer', 'D', 282, 'forward descriptor match: Trailing(3)', [[
  'self.buffers == other.buffers',
  'self.buffers.iter().zip(other.buffers.iter()).take(self.count).all(|(a, b)| a == b)',
]]);
add('omit-source-roster-match', 'D', 290, 'source descriptor match: Count', [[
  '.is_ok_and(|actual| actual.matches(expected))', '.is_ok()',
]]);
const digest = '<[u8; 32]>::from(Sha256::digest(self.hsaco)) != projection.identity().object_sha256()';
add('omit-artifact-digest', 'A', 354, 'post-transfer artifact substitution: length=false', [[
  digest, 'false && ' + digest,
]]);
const artifact = 'u64::try_from(self.hsaco.len()).ok() != Some(projection.finalized_hsaco_length())\n            || ' + digest;
add('paired-artifact-length', 'A', 354, 'post-transfer artifact substitution: length=true', [[
  artifact, 'u64::try_from(self.hsaco.len()).ok() == Some(projection.finalized_hsaco_length())\n            && ' + digest,
]]);
for (const [index, name, field, argument] of [
  [0, 'omit-authority-digest', 'finalized_hsaco_sha256', 'finalized_hsaco_sha256'],
  [1, 'omit-authority-length', 'finalized_hsaco_length', 'finalized_hsaco_length'],
  [2, 'omit-authority-kernel', 'kernel_name', 'kernel_name'],
  [3, 'omit-authority-dispatch', 'dispatch_contract_sha256', 'dispatch_contract_sha256'],
  [4, 'omit-authority-device', 'device_unique_id', 'device_unique_id'],
]) {
  const predicate = 'authority.' + field + '() != ' + argument;
  add(name, 'U', 391, 'post-transfer authority substitution: field=' + index,
    [[predicate, 'false && ' + predicate]], authorityPath);
}
add('omit-currentness-revalidation', 'U', 396, 'post-transfer stale authority validation', [[
  '        self.revalidate()?;\n', '',
]]);
module.exports = {tests, mutations};
