#!/usr/bin/env node
// Fresh normal exports, public sealed capture, independent bounded report validation.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { measureNativeBuildInput,requireDiskReserve } from './assembly-region-worker-prototype.mjs';
import { parseArguments as parseOldArguments } from './debug-runtime-origin-source-v1-smoke.mjs';
import { runRuntimeOriginCommand } from './debug-runtime-origin-source-v1-process.mjs';
import { admissionInput,admissionArguments,validateAdmission } from './debug-runtime-observations-source-v1-admission.mjs';
import { validateNativeWorkgroupInspection } from './debug-runtime-observations-source-v1-native-v5.mjs';
import { SOURCE,RUSTC_COMMIT,LIMITS,PROFILE,WG_SOURCE,WG_MANIFEST,demand,same,sha,json,artifact,manifest,
  requestDocument,exportArguments,validateCensus,validateLoop,validateWorkgroup,
  validateWorkgroupCensus,validateAttribution,uint }
  from './debug-runtime-observations-source-v1-data.mjs';
export function parseArguments(args) {
  const c=parseOldArguments(args);
  c.target=path.join(c.cache[0],'runtime-observations-loop-'+path.basename(c.output));
  c.workgroup_target=path.join(c.cache[1],'runtime-observations-workgroup-'+path.basename(c.output));return c;
}
function read(file,cap) {
  const fd=fs.openSync(file,fs.constants.O_RDONLY|fs.constants.O_NOFOLLOW);
  try {
    const before=fs.fstatSync(fd);demand(before.isFile()&&before.size>=1&&before.size<=cap,'bounded regular input');
    const bytes=Buffer.alloc(before.size);let offset=0;
    while(offset<bytes.length){const n=fs.readSync(fd,bytes,offset,bytes.length-offset,offset);demand(n>0,'short read');offset+=n;}
    demand(fs.readSync(fd,Buffer.alloc(1),0,1,offset)===0,'input grew');
    const after=fs.fstatSync(fd);
    for(const key of ['dev','ino','size','mtimeMs','ctimeMs'])demand(before[key]===after[key],'input changed');
    return bytes;
  } finally {fs.closeSync(fd);}
}
function ram() {
  const bytes=fs.readFileSync('/proc/meminfo');demand(bytes.length<=65536,'meminfo bound');
  const matches=[...bytes.toString().matchAll(/^MemAvailable:\s+(\d+) kB$/gm)];
  demand(matches.length===1,'RAM observation');const count=BigInt(matches[0][1])*1024n;
  demand(count>=BigInt(LIMITS.ram_bytes),'RAM reserve below64GiB');return count.toString();
}
function write(file,bytes){fs.writeFileSync(file,bytes,{flag:'wx',mode:0o600});}
function serialized(value,cap=LIMITS.capture) {
  const bytes=Buffer.from(JSON.stringify(value,null,2)+'\n');demand(bytes.length<=cap,'serialized receipt bound');return bytes;
}
const pin=(file,bytes)=>({path:file,bytes:bytes.length,sha256:sha(bytes)});
export function roster(c) {
  return [
    ...['smoke','data','test','admission','native-v5'].map(n=>['runner_'+n,path.join(c.repo,'scripts/debug-runtime-observations-source-v1-'+n+'.mjs')]),
    ...['smoke','data','process'].map(n=>['shared_source_'+n,path.join(c.repo,'scripts/debug-runtime-origin-source-v1-'+n+'.mjs')]),
    ...['assembly-region-worker-prototype','ordered-program-source-native','ordered-program-worker-prototype']
      .map(n=>['shared_'+n,path.join(c.repo,'scripts/'+n+'.mjs')]),
    ...['Cargo.toml','Cargo.lock','rust-toolchain.toml'].map(n=>[n,path.join(c.repo,n)]),
    ...['fe2o3-export-sim','fe2o3-author','fe2o3-debug','fe2o3-rustc-extract','librustc_codegen_fe2o3.so'].map(n=>[n,path.join(c.bin,n)]),
    ['observer',c.observer],['rustc',c.rustc],['rustc_driver',c.driver],['cargo',c.cargo],
    ['node',process.execPath],['git','/usr/bin/git'],['du','/usr/bin/du'],
    ...['run','common','loop_cases','workgroup_cases','workgroup_inspection','tests'].map(n=>['observer_'+n,path.join(c.repo,
      'crates/fe2o3-debug-cli/examples/runtime_observations_source_v1/'+n+'.rs')]),
    ['observer_root',path.join(c.repo,'crates/fe2o3-debug-cli/examples/observe_runtime_observations_source_v1.rs')],
    ['shared_loop_topology',path.join(c.repo,'crates/fe2o3-kir-sim/examples/runtime_origin_source_v1/topology.rs')],
    ['workgroup_source',path.join(c.repo,WG_SOURCE)],['workgroup_manifest',path.join(c.repo,WG_MANIFEST)],
  ];
}
export async function runCapture(c) {
  demand(process.platform==='linux'&&process.arch==='x64'&&Number(process.versions.node.split('.')[0])>=22,'Linux x64 Node22+');
  for(const directory of [c.repo,c.bin,c.cargo_home,...c.cache,path.dirname(c.output)])
    demand(fs.realpathSync(directory)===directory&&fs.statSync(directory).isDirectory(),'resolved directories');
  demand(fs.statSync(c.repo).dev===fs.statSync(path.dirname(c.output)).dev,'persistent same-filesystem output');
  requireDiskReserve(path.dirname(c.output));ram();
  demand(!fs.existsSync(c.target)&&!fs.existsSync(c.workgroup_target),'both export targets fresh');
  fs.mkdirSync(c.output,{mode:0o700});
  const receipt={schema:'task-runtime-observations-source-capture-v1',status:'running',configuration:c,
    run_ids:{loop:crypto.randomBytes(32).toString('hex'),workgroup:crypto.randomBytes(32).toString('hex')},
    limits:LIMITS,profile:PROFILE,measurements_before:[],measurements_after:[],stages:[],resource_guards:[],
    artifacts:{requests:{},loop:{},workgroup:{}},source_authenticated:false,hardware_observed:false,
    compiler_resume_authority:false,source_binding:'same-run diagnostic census; no source-variable-to-SSA ownership',
    runtime_closure:'selected measured inputs, not dependency closure or transport/browser qualification'};
  let current='initialize';
  try {
    for(const directory of ['logs','tmp','source','requests'])fs.mkdirSync(path.join(c.output,directory),{mode:0o700});
    fs.mkdirSync(path.join(c.output,'source/src'),{mode:0o700});
    fs.mkdirSync(c.target,{mode:0o700});fs.mkdirSync(c.workgroup_target,{mode:0o700});
    const source=Buffer.from(SOURCE),cargoManifest=Buffer.from(manifest(c.repo));
    const sourceFile=path.join(c.output,'source/src/lib.rs');
    const wgSourceFile=path.join(c.repo,WG_SOURCE),wgSource=read(wgSourceFile,LIMITS.source);
    demand(source.length<=LIMITS.source&&cargoManifest.length<=LIMITS.artifact,'source input bounds');
    write(sourceFile,source);write(path.join(c.output,'source/Cargo.toml'),cargoManifest);
    receipt.artifacts.loop.source=pin(sourceFile,source);receipt.artifacts.workgroup.source=pin(wgSourceFile,wgSource);
    receipt.artifacts.loop.manifest=pin(path.join(c.output,'source/Cargo.toml'),cargoManifest);
    for(const [name,mode,rounds] of [['loop-rounds-0','loop',0],['loop-rounds-1','loop',1],
      ['loop-rounds-3','loop',3],['workgroup-reduce','workgroup',0]]) {
      const file=path.join(c.output,'requests',name+'.json'),bytes=Buffer.from(JSON.stringify(requestDocument(mode,rounds))+'\n');
      write(file,bytes);receipt.artifacts.requests[name]=pin(file,bytes);
    }
    receipt.measurements_before=roster(c).map(([role,file])=>({role,...measureNativeBuildInput(file)}));
    const env={PATH:path.dirname(c.rustc)+':'+path.dirname(c.cargo)+':/usr/bin:/bin',HOME:os.homedir(),
      LANG:'C',LC_ALL:'C',TMPDIR:path.join(c.output,'tmp'),RUSTC:c.rustc,CARGO:c.cargo,CARGO_HOME:c.cargo_home,
      CARGO_TARGET_DIR:c.target,RUSTUP_TOOLCHAIN:'nightly-2026-04-03',CARGO_BUILD_JOBS:'2',CARGO_INCREMENTAL:'0',
      CARGO_PROFILE_DEV_DEBUG:'0',CARGO_TERM_COLOR:'never',CARGO_NET_OFFLINE:'true',
      LD_LIBRARY_PATH:path.dirname(c.driver)+':'+c.bin};
    let retained=0;
    const guard=()=>{requireDiskReserve(c.output);ram();};
    const run=async(name,executable,args,input,delta={})=>{
      current=name;const disk_before=requireDiskReserve(c.output).toString(),ram_before=ram();
      const result=await runRuntimeOriginCommand({executable,args,cwd:c.repo,env:{...env,...delta},input,
        timeoutMs:name.startsWith('export-')?LIMITS.export_ms:(name.startsWith('observe-')||name.startsWith('admit-'))?LIMITS.observer_ms:30000,
        outputCap:LIMITS.stream,guard});
      for(const stream of ['stdout','stderr'])write(path.join(c.output,'logs',name+'.'+stream),result[stream]);
      receipt.stages.push({name,executable,args,cwd:c.repo,env_delta:delta,
        input:input?{bytes:input.length,sha256:sha(input)}:null,disk_before,ram_before,ram_after:ram(),
        code:result.code,signal:result.signal,reason:result.reason,elapsed_ms:result.elapsed_ms,
        stdout:artifact(result.stdout),stderr:artifact(result.stderr)});
      retained+=result.stdout.length+result.stderr.length;demand(retained<=LIMITS.retained,'cumulative command output8MiB');
      demand(result.code===0&&result.signal===null&&result.reason===null,name+': command failure is not semantic evidence');
      return result.stdout;
    };
    const cacheGuard=async label=>{
      const bytes=await run('cache-'+label,'/usr/bin/du',['-sb','--',...c.cache,c.output]);
      const lines=bytes.toString().trimEnd().split('\n');demand(lines.length===3,'exact cache root count');
      let total=0n;[...c.cache,c.output].forEach((root,i)=>{
        const fields=lines[i].split('\t');demand(fields.length===2&&fields[1]===root&&/^\d{1,20}$/.test(fields[0]),'exact cache roots');
        total+=BigInt(fields[0]);});
      demand(total<=BigInt(LIMITS.cache_bytes),'combined cache/output exceeds20GiB');
      receipt.resource_guards.push({label,total_bytes:total.toString()});
    };
    await run('git-head','/usr/bin/git',['rev-parse','HEAD']);
    await run('git-status','/usr/bin/git',['status','--porcelain=v1','--untracked-files=normal']);
    const version=(await run('rustc-version',c.rustc,['-vV'])).toString();
    demand(version.split('\n').filter(v=>v==='commit-hash: '+RUSTC_COMMIT).length===1
      &&version.split('\n').filter(v=>v==='release: 1.96.0-nightly').length===1,'exact pinned rustc');
    await cacheGuard('before-lock');
    await run('loop-lock',c.cargo,['generate-lockfile','--offline','--manifest-path',path.join(c.output,'source/Cargo.toml')]);
    const lockFile=path.join(c.output,'source/Cargo.lock');
    receipt.artifacts.loop.lock=pin(lockFile,read(lockFile,LIMITS.artifact));
    const reports={};
    for(const mode of ['loop','workgroup']) {
      await cacheGuard('before-'+mode);
      const censusFile=path.join(c.output,mode+'-census.json');
      await run('export-'+mode,path.join(c.bin,'fe2o3-export-sim'),exportArguments(c,mode),undefined,{
        CARGO_TARGET_DIR:mode==='loop'?c.target:c.workgroup_target,
        FE2O3_DIAGNOSTIC_SOURCE_CENSUS_PATH_V1:censusFile,
        FE2O3_DIAGNOSTIC_SOURCE_CENSUS_RUN_ID_V1:receipt.run_ids[mode]});
      demand(!receipt.stages.at(-1).stderr.utf8.includes('fe2o3 diagnostic source census unavailable:'),'census warning fails source gate');
      const bundleFile=path.join(c.output,mode==='loop'?'loop-helper-v6.fe2sim':'workgroup-reduce-v5.fe2sim');
      const bundle=read(bundleFile,LIMITS.bundle),censusBytes=read(censusFile,LIMITS.census);
      const bundleSha=sha(bundle),census=json(censusBytes,LIMITS.census);
      const fileIdentity=mode==='loop'?validateCensus(census,receipt.run_ids.loop,sourceFile,source)
        :validateWorkgroupCensus(census,receipt.run_ids.workgroup,wgSourceFile,wgSource);
      receipt.artifacts[mode].bundle=pin(bundleFile,bundle);
      receipt.artifacts[mode].census={path:censusFile,...artifact(censusBytes)};
      receipt.artifacts[mode].source_file_identity=fileIdentity;
      receipt.artifacts[mode].cli_admissions=[];
      for(const name of mode==='loop'?['loop-rounds-0','loop-rounds-1','loop-rounds-3']:['workgroup-reduce']) {
        const request=receipt.artifacts.requests[name];
        same(pin(request.path,read(request.path,16384)),request,'exact request before actual CLI admission');
        const bytes=await run('admit-'+name,path.join(c.bin,'fe2o3-debug'),
          admissionArguments(mode,bundleFile,request.path),admissionInput());
        const validation=validateAdmission(bytes);
        receipt.artifacts[mode].cli_admissions.push({request,protocol:artifact(bytes),validation});
      }
      const reportBytes=await run('observe-'+mode,c.observer,[mode,bundleFile,bundleSha]);
      const report=json(reportBytes,LIMITS.report);
      const result=mode==='loop'?validateLoop(report,bundleSha):validateWorkgroup(report,bundleSha);reports[mode]=result;
      receipt.artifacts[mode].observer={path:path.join(c.output,'logs','observe-'+mode+'.stdout'),...artifact(reportBytes)};
      if(mode==='workgroup') {
        const bytes=await run('inspect-workgroup',c.observer,['inspect-workgroup',bundleFile,bundleSha]);
        const inspection=json(bytes,LIMITS.artifact);
        receipt.artifacts.workgroup.inspect=artifact(bytes);
        receipt.artifacts.workgroup.pages=[];
        receipt.artifacts.workgroup.native_v5_census_join=
          validateNativeWorkgroupInspection(result,inspection,fileIdentity,wgSource);
      } else {
        const inspect=await run('inspect-'+mode,path.join(c.bin,'fe2o3-author'),['inspect'],bundle);
        receipt.artifacts[mode].inspect=artifact(inspect);const summary=json(inspect,LIMITS.artifact);
        uint(summary.operation_count,LIMITS.operations,'operation census bound');
        const operations=[];receipt.artifacts[mode].pages=[];
        while(operations.length<summary.operation_count) {
          const start=operations.length,bytes=await run('operations-'+mode+'-'+start,path.join(c.bin,'fe2o3-author'),
            ['operations','--bundle-identity',summary.bundle_identity,'--start',String(start),'--limit','64'],bundle);
          const page=json(bytes,LIMITS.artifact);
          demand(page.start===start&&page.total_operations===summary.operation_count
            &&page.bundle_identity===summary.bundle_identity&&page.canonical_kir_digest===summary.canonical_kir_digest
            &&Array.isArray(page.operations)&&page.operations.length>0&&page.operations.length<=64,'operation page exact identity/range');
          operations.push(...page.operations);
          demand(operations.length<=summary.operation_count&&page.next_start===
            (operations.length<summary.operation_count?operations.length:null),'complete bounded operation pages');
          receipt.artifacts[mode].pages.push(artifact(bytes));
        }
        validateAttribution(result,summary,operations,fileIdentity,source);
      }
      await cacheGuard('after-'+mode);
    }
    receipt.measurements_after=roster(c).map(([role,file])=>({role,...measureNativeBuildInput(file)}));
    same(receipt.measurements_after,receipt.measurements_before,'measured inputs changed');
    for(const entry of [...Object.values(receipt.artifacts.requests),
      receipt.artifacts.loop.source,receipt.artifacts.loop.manifest,receipt.artifacts.loop.lock,
      receipt.artifacts.workgroup.source,receipt.artifacts.loop.bundle,receipt.artifacts.workgroup.bundle])
      same(pin(entry.path,read(entry.path,LIMITS.bundle)),entry,'retained input changed');
    for(const mode of ['loop','workgroup']) {
      const entry=receipt.artifacts[mode].census;
      same({path:entry.path,...artifact(read(entry.path,LIMITS.census))},entry,'retained source census changed');
    }
    receipt.status='passed';receipt.summary={reuse_on_runs:8,reuse_off_runs:8,helper_activations:reports.loop.helper_activations,
      source_loop_and_call:true,source_workgroup_storage_reuse:true,strict_cli_requests_admitted:4,private_alloca_source_qualified:false,
      transport_qualified:false,browser_qualified:false};
    write(path.join(c.output,'receipt.json'),serialized(receipt));return receipt;
  } catch(error) {
    receipt.status='failed';receipt.failed_stage=current;receipt.error=String(error).slice(0,4096);
    write(path.join(c.output,'failure.json'),serialized({...receipt,
      stages:receipt.stages.map(({stdout,stderr,...row})=>({...row,
        stdout:{bytes:stdout.bytes,sha256:stdout.sha256},stderr:{bytes:stderr.bytes,sha256:stderr.sha256}}))}));
    throw error;
  }
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url))
  runCapture(parseArguments(process.argv.slice(2))).then(receipt=>{
    process.stdout.write('Public runtime source acceptance passed: '+receipt.configuration.output+'\n');
  }).catch(error=>{process.stderr.write(String(error)+'\n');process.exitCode=1;});
