#!/usr/bin/env bash
set -euo pipefail
stage=$(cd -- "$(dirname -- "$0")" && pwd)
cd "$stage"
node <<'NODE'
const fs=require('fs'),cp=require('child_process'),crypto=require('crypto');
const root='/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917';
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const raw=fs.readFileSync('source-qualified-before.log');
if(sha(raw)!=='d22891250b363dda63dc154cf5a3e3a66b5bd4cd110cdc7f148c02793cd7834c')throw Error('frozen manifest changed');
const doc=JSON.parse(raw);
for(const[name,digest]of Object.entries(doc.files))if(sha(fs.readFileSync(root+'/'+name))!==digest)throw Error('source '+name);
for(const name of ['benchmarks/runtime_gfx942/copy-host-observe.py','crates/fe2o3-runtime/src/kfd_backend/retained_release_tests/copy_accounting.rs'])if(!doc.files[name])throw Error('missing source '+name);
if(sha(fs.readFileSync('copy-host-observe.py'))!==doc.files['benchmarks/runtime_gfx942/copy-host-observe.py'])throw Error('observer copy');
const binary='target/x86_64-unknown-linux-musl/debug/deps/fe2o3_runtime-afaad0775e07e3e7';
const expected='8e729393c536a7fdcb7ca42d81a667b55dc7b7163b37c5f27bdb6484698abd89';
if(sha(fs.readFileSync(root+'/'+binary))!==expected||sha(fs.readFileSync('runtime-test'))!==expected)throw Error('binary mismatch');
console.log(JSON.stringify({base:doc.base,sourceFiles:Object.keys(doc.files).length,sourceManifestSha256:sha(raw),observerSha256:sha(fs.readFileSync('copy-host-observe.py')),binarySourcePath:root+'/'+binary,binarySha256:expected,sourceMatchesFrozenMap:true},null,2));
NODE
file runtime-test
readelf -h runtime-test
readelf -l runtime-test
if readelf -l runtime-test | grep -q INTERP; then exit 1; fi
sha256sum runtime-test copy-host-observe.py source-qualified-before.log
