#!/usr/bin/env node
// Fresh normal Rust export -> topology gate -> bounded in-process CPU observation.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { measureNativeBuildInput, requireDiskReserve } from './assembly-region-worker-prototype.mjs';
import { runRuntimeOriginCommand } from './debug-runtime-origin-source-v1-process.mjs';
import { SOURCE,CRATE,RUSTC_COMMIT,LIMITS,demand,same,absolute,artifact,sha,json,uint,
  exportArguments,manifest,validateCensus,validateObserver,validateAttribution } from './debug-runtime-origin-source-v1-data.mjs';

const HERE=path.dirname(fileURLToPath(import.meta.url));
const inside=(a,b)=>{const relative=path.relative(a,b);return relative===''||
  (relative!=='..'&&!relative.startsWith('../')&&!path.isAbsolute(relative));};
export function parseArguments(args) {
  const names=['output','bin-dir','observer','rustc','cargo','rustc-driver','cargo-home',
    'cache-root','secondary-cache-root'];
  demand(args.length===names.length*2,'nine explicit absolute options');
  const options={};
  for(let i=0;i<args.length;i+=2) {
    const key=args[i].slice(2);
    demand(args[i].startsWith('--')&&names.includes(key)&&!Object.hasOwn(options,key),'unknown/duplicate option');
    options[key]=absolute(args[i+1]);
  }
  const c={repo:path.resolve(HERE,'..'),output:options.output,bin:options['bin-dir'],
    observer:options.observer,rustc:options.rustc,cargo:options.cargo,driver:options['rustc-driver'],
    cargo_home:options['cargo-home'],cache:[options['cache-root'],options['secondary-cache-root']]};
  demand(/^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/.test(path.basename(c.output)),'output label');
  const roots=[c.repo,c.output,...c.cache];
  for(let i=0;i<roots.length;i++) for(let j=i+1;j<roots.length;j++)
    demand(!inside(roots[i],roots[j])&&!inside(roots[j],roots[i]),'overlapping repo/output/cache roots');
  demand(c.output!==os.homedir()&&path.dirname(c.output)!==c.output,'broad output path');
  demand(path.basename(c.rustc)==='rustc'&&path.basename(c.cargo)==='cargo'
    &&path.dirname(c.driver)===path.join(path.dirname(path.dirname(c.rustc)),'lib')
    &&/^librustc_driver-[0-9a-f]+\.so$/.test(path.basename(c.driver)),'direct pinned toolchain paths');
  c.target=path.join(c.cache[0],'runtime-origin-'+path.basename(c.output));
  return c;
}
function read(file,cap) {
  const fd=fs.openSync(file,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW);
  try {
    const before=fs.fstatSync(fd);
    demand(before.isFile()&&before.size>=1&&before.size<=cap,'bounded regular file');
    const bytes=Buffer.alloc(before.size);let offset=0;
    while(offset<bytes.length) { const n=fs.readSync(fd,bytes,offset,bytes.length-offset,offset);
      demand(n>0,'short read');offset+=n; }
    demand(fs.readSync(fd,Buffer.alloc(1),0,1,offset)===0,'grew during read');
    const after=fs.fstatSync(fd);
    for(const key of ['dev','ino','size','mtimeMs','ctimeMs']) demand(before[key]===after[key],'changed during read');
    return bytes;
  } finally {fs.closeSync(fd);}
}
function ram() {
  const bytes=fs.readFileSync('/proc/meminfo');demand(bytes.length<=65536,'meminfo cap');
  const matches=[...bytes.toString().matchAll(/^MemAvailable:\s+(\d+) kB$/gm)];
  demand(matches.length===1,'available RAM observation');
  const count=BigInt(matches[0][1])*1024n;
  demand(count>=BigInt(LIMITS.ram_bytes),'RAM reserve below64GiB');return count.toString();
}
function write(file,bytes) {fs.writeFileSync(file,bytes,{flag:'wx',mode:0o600});}
function serialize(value,cap=LIMITS.capture) {
  const bytes=Buffer.from(JSON.stringify(value,null,2)+'\n');demand(bytes.length<=cap,'serialized capture cap');return bytes;
}
export function roster(c) {
  return [
    ...['smoke','data','process','test'].map(name=>['runner_'+name,path.join(c.repo,
      'scripts/debug-runtime-origin-source-v1-'+name+'.mjs')]),
    ['shared_measure',path.join(c.repo,'scripts/assembly-region-worker-prototype.mjs')],
    ['shared_json',path.join(c.repo,'scripts/ordered-program-source-native.mjs')],
    ['shared_ordered',path.join(c.repo,'scripts/ordered-program-worker-prototype.mjs')],
    ...['Cargo.toml','Cargo.lock','rust-toolchain.toml'].map(name=>[name,path.join(c.repo,name)]),
    ...['fe2o3-export-sim','fe2o3-author','fe2o3-rustc-extract','librustc_codegen_fe2o3.so']
      .map(name=>[name,path.join(c.bin,name)]),
    ['observer',c.observer],['rustc',c.rustc],['rustc_driver',c.driver],['cargo',c.cargo],
    ['node',process.execPath],['git','/usr/bin/git'],['du','/usr/bin/du'],
    ...['run','topology','collector','tests'].map(name=>['observer_'+name,path.join(c.repo,
      'crates/fe2o3-kir-sim/examples/runtime_origin_source_v1/'+name+'.rs')]),
    ['observer_root',path.join(c.repo,'crates/fe2o3-kir-sim/examples/observe_runtime_origin_source_v1.rs')],
  ];
}
export async function runCapture(c) {
  demand(process.platform==='linux'&&process.arch==='x64'&&Number(process.versions.node.split('.')[0])>=22,
    'Linux x64 Node22+');
  for(const directory of [c.repo,c.bin,c.cargo_home,...c.cache,path.dirname(c.output)])
    demand(fs.realpathSync(directory)===directory&&fs.statSync(directory).isDirectory(),'resolved directory');
  demand(fs.statSync(c.repo).dev===fs.statSync(path.dirname(c.output)).dev,'persistent output filesystem');
  requireDiskReserve(path.dirname(c.output));ram();
  demand(!fs.existsSync(c.target),'fresh source target required');
  fs.mkdirSync(c.output,{mode:0o700});
  const receipt={schema:'task-runtime-origin-source-capture-v1',status:'running',configuration:c,
    run_id:crypto.randomBytes(32).toString('hex'),limits:LIMITS,measurements_before:[],
    measurements_after:[],stages:[],resource_guards:[],artifacts:{},
    source_authenticated:false,compiler_resume_authority:false,hardware_observed:false,
    source_binding:'same-run census attribution, not source-to-SSA ownership',
    runtime_closure:'selected input measurements only'};
  const save=(name,value,cap)=>write(path.join(c.output,name),serialize(value,cap));
  let current='initialize';
  try {
    for(const name of ['logs','tmp','source']) fs.mkdirSync(path.join(c.output,name),{mode:0o700});
    fs.mkdirSync(path.join(c.output,'source/src'),{mode:0o700});
    fs.mkdirSync(c.target,{mode:0o700});
    const source=Buffer.from(SOURCE),cargoManifest=Buffer.from(manifest(c.repo));
    demand(source.length<=LIMITS.source&&cargoManifest.length<=LIMITS.artifact,'source bounds');
    write(path.join(c.output,'source/src/lib.rs'),source);
    write(path.join(c.output,'source/Cargo.toml'),cargoManifest);
    receipt.measurements_before=roster(c).map(([role,file])=>({role,...measureNativeBuildInput(file)}));
    const env={PATH:path.dirname(c.rustc)+':'+path.dirname(c.cargo)+':/usr/bin:/bin',
      HOME:os.homedir(),LANG:'C',LC_ALL:'C',TMPDIR:path.join(c.output,'tmp'),
      RUSTC:c.rustc,CARGO:c.cargo,CARGO_HOME:c.cargo_home,CARGO_TARGET_DIR:c.target,
      RUSTUP_TOOLCHAIN:'nightly-2026-04-03',CARGO_BUILD_JOBS:'2',CARGO_INCREMENTAL:'0',
      CARGO_PROFILE_DEV_DEBUG:'0',CARGO_TERM_COLOR:'never',CARGO_NET_OFFLINE:'true',
      LD_LIBRARY_PATH:path.dirname(c.driver)+':'+c.bin};
    let retained=0;
    const guard=()=>{requireDiskReserve(c.output);ram();};
    const run=async(name,executable,args,input,delta={})=>{
      current=name;
      const disk_before=requireDiskReserve(c.output).toString(),ram_before=ram();
      const result=await runRuntimeOriginCommand({executable,args,cwd:c.repo,env:{...env,...delta},input,
        timeoutMs:name==='export'?LIMITS.export_ms:name==='observe'?LIMITS.observer_ms:30000,
        outputCap:LIMITS.stream,guard});
      for(const stream of ['stdout','stderr']) write(path.join(c.output,'logs',name+'.'+stream),result[stream]);
      const row={name,executable,args,cwd:c.repo,env_delta:delta,
        input:input?{bytes:input.length,sha256:sha(input)}:null,disk_before,ram_before,ram_after:ram(),
        code:result.code,signal:result.signal,reason:result.reason,elapsed_ms:result.elapsed_ms,
        stdout:artifact(result.stdout),stderr:artifact(result.stderr)};
      receipt.stages.push(row);retained+=result.stdout.length+result.stderr.length;
      demand(retained<=LIMITS.retained,'cumulative retained command output');
      demand(result.code===0&&result.signal===null&&result.reason===null,
        name+': external command failure is not semantic evidence');
      return result.stdout;
    };
    const cacheGuard=async label=>{
      const bytes=await run('cache-'+label,'/usr/bin/du',['-sb','--',...c.cache,c.output]);
      const lines=bytes.toString().trimEnd().split('\n');demand(lines.length===3,'du root count');
      let total=0n;
      [...c.cache,c.output].forEach((root,i)=>{
        const fields=lines[i].split('\t');demand(fields.length===2&&fields[1]===root&&/^\d{1,20}$/.test(fields[0]),
          'du exact roots');total+=BigInt(fields[0]);});
      demand(total<=BigInt(LIMITS.cache_bytes),'combined cache/output exceeds20GiB');
      receipt.resource_guards.push({label,total_bytes:total.toString()});
    };
    await run('git-head','/usr/bin/git',['rev-parse','HEAD']);
    await run('git-status','/usr/bin/git',['status','--porcelain=v1','--untracked-files=normal']);
    const version=(await run('rustc-version',c.rustc,['-vV'])).toString();
    demand(version.split('\n').filter(v=>v==='commit-hash: '+RUSTC_COMMIT).length===1
      &&version.split('\n').filter(v=>v==='release: 1.96.0-nightly').length===1,'pinned rustc version');
    await cacheGuard('before-lock');
    await run('lock',c.cargo,['generate-lockfile','--offline','--manifest-path',path.join(c.output,'source/Cargo.toml')]);
    const lock=read(path.join(c.output,'source/Cargo.lock'),LIMITS.artifact);
    receipt.artifacts.source=artifact(source);receipt.artifacts.manifest=artifact(cargoManifest);
    receipt.artifacts.lock=artifact(lock);
    await cacheGuard('before-export');
    await run('export',path.join(c.bin,'fe2o3-export-sim'),exportArguments(c),undefined,{
      FE2O3_DIAGNOSTIC_SOURCE_CENSUS_PATH_V1:path.join(c.output,'census.json'),
      FE2O3_DIAGNOSTIC_SOURCE_CENSUS_RUN_ID_V1:receipt.run_id});
    demand(!receipt.stages.at(-1).stderr.utf8.includes('fe2o3 diagnostic source census unavailable:'),
      'census warning prevents source qualification');
    await cacheGuard('after-export');
    const bundle=read(path.join(c.output,'loop-helper-v6.fe2sim'),LIMITS.bundle);
    const census=read(path.join(c.output,'census.json'),LIMITS.census);
    receipt.artifacts.bundle={bytes:bundle.length,sha256:sha(bundle)};
    receipt.artifacts.census=artifact(census);
    const fileIdentity=validateCensus(json(census,LIMITS.census),receipt.run_id,
      path.join(c.output,'source/src/lib.rs'),source);
    // Observer itself refuses missing actual loop/call topology BEFORE simulation.
    const reportBytes=await run('observe',c.observer,[path.join(c.output,'loop-helper-v6.fe2sim'),sha(bundle)]);
    const report=json(reportBytes,LIMITS.report);validateObserver(report,sha(bundle));
    receipt.artifacts.observer=artifact(reportBytes);
    const inspect=await run('inspect',path.join(c.bin,'fe2o3-author'),['inspect'],bundle);
    receipt.artifacts.inspect=artifact(inspect);const summary=json(inspect,LIMITS.artifact);
    uint(summary.operation_count,LIMITS.operations,'operation census cap');
    const operations=[];receipt.artifacts.pages=[];
    while(operations.length<summary.operation_count) {
      const start=operations.length;
      const bytes=await run('operations-'+start,path.join(c.bin,'fe2o3-author'),
        ['operations','--bundle-identity',summary.bundle_identity,'--start',String(start),'--limit','64'],bundle);
      const page=json(bytes,LIMITS.artifact);
      demand(page.start===start&&page.total_operations===summary.operation_count
        &&page.bundle_identity===summary.bundle_identity&&page.canonical_kir_digest===summary.canonical_kir_digest
        &&Array.isArray(page.operations)&&page.operations.length>0&&page.operations.length<=64,'exact page identity/range');
      operations.push(...page.operations);
      demand(operations.length<=summary.operation_count&&page.next_start===
        (operations.length<summary.operation_count?operations.length:null),'complete page progression');
      receipt.artifacts.pages.push(artifact(bytes));
    }
    validateAttribution(report,summary,operations,fileIdentity,source);
    await cacheGuard('after-observer');
    receipt.measurements_after=roster(c).map(([role,file])=>({role,...measureNativeBuildInput(file)}));
    same(receipt.measurements_after,receipt.measurements_before,'measured tools/source changed');
    for(const [name,file,cap] of [['source','src/lib.rs',LIMITS.source],['manifest','Cargo.toml',LIMITS.artifact],
      ['lock','Cargo.lock',LIMITS.artifact]]) same(artifact(read(path.join(c.output,'source',file),cap)),
        receipt.artifacts[name],'retained source input changed');
    same({bytes:bundle.length,sha256:sha(read(path.join(c.output,'loop-helper-v6.fe2sim'),LIMITS.bundle))},
      receipt.artifacts.bundle,'bundle changed');
    same(artifact(read(path.join(c.output,'census.json'),LIMITS.census)),receipt.artifacts.census,'census changed');
    receipt.status='passed';receipt.summary={contextual_runs:6,opt_out_runs:6,helper_activations:report.helper_activations,
      actual_retained_loop_helper:true,source_file_identity:fileIdentity};
    save('receipt.json',receipt);
    return receipt;
  } catch(error) {
    receipt.status='failed';receipt.failed_stage=current;receipt.error=String(error).slice(0,4096);
    save('failure.json',{schema:receipt.schema,status:'failed',failed_stage:current,error:receipt.error,
      configuration:c,run_id:receipt.run_id,hardware_observed:false,source_authenticated:false,
      stages:receipt.stages.map(({stdout,stderr,...row})=>({...row,
        stdout:{bytes:stdout.bytes,sha256:stdout.sha256},
        stderr:{bytes:stderr.bytes,sha256:stderr.sha256}}))});throw error;
  }
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  runCapture(parseArguments(process.argv.slice(2))).then(capture=>{
    process.stdout.write('Runtime source origin acceptance passed: '+capture.configuration.output+'\n');
  }).catch(error=>{process.stderr.write(String(error)+'\n');process.exitCode=1;});
}
