/** 通常の製品処理をCLIだけで検証する。WebView・OS画面操作・利用者データは使わない。 */
import fs from 'node:fs';
import path from 'node:path';
import http from 'node:http';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {spawn,spawnSync} from 'node:child_process';
import {createInterface} from 'node:readline';
import {once} from 'node:events';

const root=process.cwd(),args=process.argv.slice(2);
let exe=path.join(root,'src-tauri/target/debug/scriptsync-cli.exe'),realUrl,realModel='whisper/base',realWav;
for(let i=0;i<args.length;i++){
 if(args[i]==='--exe')exe=path.resolve(args[++i]);
 else if(args[i]==='--asr-url')realUrl=args[++i];
 else if(args[i]==='--asr-model')realModel=args[++i];
 else if(args[i]==='--asr-wav')realWav=path.resolve(args[++i]);
 else throw Error('使い方: test-cli.mjs [--exe EXE] [--asr-url URL --asr-model ID --asr-wav 合成WAV]');
}
assert.equal(Boolean(realUrl),Boolean(realWav),'実接続はURLと合成WAVを一緒に指定してください');
const version=JSON.parse(fs.readFileSync('dev/msix-version.json','utf8'));
const run=path.join(root,`dev/history/v${version.applicationVersion}.${version.revision}/logs/cli-${new Date().toISOString().replaceAll(':','-')}`);
fs.mkdirSync(run,{recursive:true});
const report={computerUse:false,webView:false,executableSha256:crypto.createHash('sha256').update(fs.readFileSync(exe)).digest('hex'),checks:[],outcome:'running'};
const check=async(name,operation)=>{await operation();report.checks.push(name);console.log('成功: '+name);};
const hash=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
function wav(){
 const count=24000,bytes=Buffer.alloc(44+count*2);
 bytes.write('RIFF');bytes.writeUInt32LE(bytes.length-8,4);bytes.write('WAVEfmt ',8);bytes.writeUInt32LE(16,16);
 bytes.writeUInt16LE(1,20);bytes.writeUInt16LE(1,22);bytes.writeUInt32LE(48000,24);bytes.writeUInt32LE(96000,28);
 bytes.writeUInt16LE(2,32);bytes.writeUInt16LE(16,34);bytes.write('data',36);bytes.writeUInt32LE(count*2,40);
 for(let n=0;n<count;n++)bytes.writeInt16LE(Math.round(Math.sin(n/48000*440*Math.PI*2)*5000),44+n*2);
 return bytes;
}
const settings=path.join(run,'settings'),projectFile=path.join(run,'fixture.scriptsync'),input=path.join(run,'fixture.wav'),bytes=wav();
fs.writeFileSync(input,bytes);const baseline='前半の合成台本です。\n後半の合成台本です。';
const markdown=path.join(run,'fixture.md');fs.writeFileSync(markdown,baseline);
const fixtureToken='fixture-cli-token',recognizedText='合成音声の認識候補です。';
let posts=0,healthError=false,mockFailure;
const server=http.createServer(async(req,res)=>{
 try{
  let value;
  if(req.url==='/api/health'){
   if(healthError){res.writeHead(503,{'Content-Type':'application/json'}).end(JSON.stringify({error:{message:fixtureToken}}));return;}
   value={ok:true,ready:true,acceptingJobs:true,serverVersion:'0.1.21',protocolVersion:3,authMode:'lan',token:fixtureToken,capabilities:{openaiAudio:true,modelDiscovery:true}};
  }else if(req.url==='/v1/models')value={object:'list',data:[{id:'whisper/base',x_meeting:{engineId:'whisper',modelId:'base',task:'transcription',availability:'installed',runtimeReady:true,supported:true,supportedLanguages:['ja']}}]};
  else if(req.url==='/v1/audio/transcriptions'){
   posts++;assert.equal(req.headers['x-local-token'],fixtureToken);
   const chunks=[];for await(const chunk of req)chunks.push(chunk);const body=Buffer.concat(chunks);
   assert.ok(body.includes(bytes));assert.match(body.toString('latin1'),/whisper\/base/);
   value={text:recognizedText,token:fixtureToken};
  }else{res.writeHead(404).end();return;}
  res.writeHead(200,{'Content-Type':'application/json'}).end(JSON.stringify(value));
 }catch(error){mockFailure=error;res.writeHead(500).end('{}');}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const url=`http://127.0.0.1:${server.address().port}`;
let processHandle,reader,pending,serial=0,exited;
function start(){
 processHandle=spawn(exe,['--settings-dir',settings],{windowsHide:true,stdio:['pipe','pipe','pipe']});
 exited=once(processHandle,'exit');
 const log=fs.createWriteStream(path.join(run,`process-${serial}.log`));processHandle.stderr.pipe(log);
 reader=createInterface({input:processHandle.stdout});
 reader.on('line',line=>{if(!pending)return;const current=pending;pending=null;clearTimeout(current.timer);try{const value=JSON.parse(line);assert.equal(value.id,current.id);current.resolve(value);}catch(error){current.reject(error);}});
 processHandle.on('exit',()=>{if(pending){clearTimeout(pending.timer);pending.reject(Error('CLIが応答前に終了しました'));pending=null;}});
}
function response(action,payload={},timeout=15000){return new Promise((resolve,reject)=>{
 assert.ok(pending==null,'CLIへの呼出しは直列で実行してください');
 const id=++serial,timer=setTimeout(()=>{pending=null;reject(Error('CLI応答の期限切れ: '+action));},timeout);
 pending={id,resolve,reject,timer};processHandle.stdin.write(JSON.stringify({id,action,payload})+'\n');
});}
async function rpc(action,payload={},timeout){const r=await response(action,payload,timeout);if(!r.ok)throw Error(action+': '+r.error);return r.result;}
async function close(){await rpc('shutdown',{},650000);processHandle.stdin.end();const [code]=await exited;assert.equal(code,0);reader.close();}
let confirmed,segment,originalTakes;
try{
 start();
 await check('隔離設定の初期化とOS画面を開かない失敗応答',async()=>{
  assert.equal((await rpc('state')).project,null);
  assert.equal((await response('new')).ok,false);
  assert.equal((await response('open_settings_link')).ok,false);
  assert.equal((await response('unknown_cli_probe')).ok,false);
 });
 await check('設定保存・プロトコル3の接続確認・保存前の送信抑止',async()=>{
  const info=await rpc('asr_server_check',{url});assert.equal(info.protocol,3);assert.equal(info.models[0].id,'whisper/base');
  const value=(await rpc('machine_settings')).settings;value.asr_provider='transcription_server';value.asr_server_url=url;value.asr_server_model='whisper/base';
  await rpc('settings_save',{settings:value});assert.equal(posts,0);
  assert.ok(!fs.readFileSync(path.join(settings,'config/machine-settings.json'),'utf8').includes(fixtureToken));
 });
 await check('Markdown取込・台本確定・合成WAVの2テイク登録',async()=>{
  assert.equal((await rpc('markdown_import',{paths:[markdown]})).files.length,1);
  await rpc('new',{path:projectFile});const initial=(await rpc('state')).project;
  await rpc('script_commit',{project_id:initial.id,base_revision:initial.script_revision,base_generation:initial.generation,sources:[{id:crypto.randomUUID(),name:'合成台本.md',kind:'markdown',text:baseline,approved_text:baseline,lines:[{line:0,enabled:true},{line:1,enabled:true}]}],deleted_source_ids:[]});
  confirmed=(await rpc('state')).project;assert.equal(confirmed.segments.length,2);segment=confirmed.segments[0].id;
  await rpc('import_audio',{segment,path:input});await rpc('import_audio',{segment,path:input});
  originalTakes=(await rpc('state')).project.segments[0].takes;assert.equal(originalTakes.length,2);assert.equal(posts,0);
 });
 await check('前後テイク結合・波形・元音声とハッシュの保持',async()=>{
  const [before,after]=originalTakes;
  await rpc('join_takes',{project_id:confirmed.id,segment,before:before.id,after:after.id,before_hash:before.hash,after_hash:after.hash});
  const p=(await rpc('state')).project;assert.equal(p.segments[0].takes.length,3);assert.deepEqual(p.segments[0].takes.slice(0,2),originalTakes);
  const wave=await rpc('waveform',{file:before.file,hash:before.hash});assert.ok(wave.duration>0);
  assert.equal(hash(fs.readFileSync(input)),hash(bytes));
 });
 await check('トリム試聴用WAVとテイク取出し・既存出力の保護',async()=>{
  const take=originalTakes[0],clip=await rpc('preview_take_edit',{segment,take:take.id,start:.05,end:.4,cut_start:.15,cut_end:.25,mode:'full'});
  assert.equal(typeof clip.hash,'string');
  const destination=path.join(run,'download.wav');await rpc('download_take',{segment,take:take.id,path:destination});
  assert.ok(fs.statSync(destination).size>44);assert.equal((await response('download_take',{segment,take:take.id,path:destination})).ok,false);
 });
 await check('採用とWAV・FLAC出力を通常の処理で実行',async()=>{
  await rpc('select',{segment,take:originalTakes[0].id});
  const second=confirmed.segments[1].id;await rpc('import_audio',{segment:second,path:input});
  const secondTake=(await rpc('state')).project.segments[1].takes[0];await rpc('select',{segment:second,take:secondTake.id});
  const options={unit:'all',naming:'jp',normalize:true,subtitle_format:'vtt',boundaries:[],global_gaps:{normal:0,part:0,chapter:0,section:0,head:0,tail:0},blocks:{}};
  await rpc('export_blocks',{...options,format:'wav'});
  await rpc('export_blocks',{...options,format:'flac'});
  const base=(await rpc('state')).root;
  const files=directory=>fs.readdirSync(directory,{withFileTypes:true}).flatMap(e=>e.isDirectory()?files(path.join(directory,e.name)):[path.join(directory,e.name)]);
  const flac=files(path.join(base,'exports')).filter(p=>p.endsWith('.flac'));assert.ok(flac.length>0);assert.equal(fs.readFileSync(flac[0]).subarray(0,4).toString(),'fLaC');
 });
 await check('明示文字起こし・修正案への反映と承認前の台本保持',async()=>{
  await rpc('recording_asr',{segment,take:originalTakes[0].id});await rpc('wait_idle',{},650000);
  assert.equal(posts,1);if(mockFailure)throw mockFailure;
  const p=(await rpc('state')).project,result=p.segments[0].transcriptions.at(-1);assert.equal(result.status,'succeeded');assert.equal(result.raw,recognizedText);
  assert.equal(p.segments[0].recording_draft,null);assert.equal(p.script_revision,confirmed.script_revision);
  await rpc('recording_draft',{segment,text:result.raw});await rpc('save_project');
  await rpc('close_project');await rpc('open',{path:projectFile});
  const reopened=(await rpc('state')).project;assert.equal(reopened.segments[0].recording_draft,recognizedText);assert.equal(reopened.segments[0].text,confirmed.segments[0].text);assert.equal(reopened.script_sources[0].confirmed_text,baseline);
  assert.ok(!JSON.stringify(reopened).includes(fixtureToken));
 });
 await check('接続失敗の応答からサーバー秘密値を除外',async()=>{
  healthError=true;const result=await response('asr_server_check',{url});assert.equal(result.ok,false);assert.ok(!JSON.stringify(result).includes(fixtureToken));healthError=false;
 });
 if(realUrl)await check('指定した実TranscriptionServerへ合成WAVを送信',async()=>{
  const info=await rpc('asr_server_check',{url:realUrl});assert.equal(info.protocol,3);assert.ok(info.models.some(m=>m.id===realModel));report.realServerVersion=info.version;
  const value=(await rpc('machine_settings')).settings;value.asr_server_url=realUrl;value.asr_server_model=realModel;await rpc('settings_save',{settings:value});
  const copied=path.join(run,'synthetic-ja.wav');fs.copyFileSync(realWav,copied);await rpc('import_audio',{segment,path:copied});
  const take=(await rpc('state')).project.segments[0].takes.at(-1);await rpc('recording_asr',{segment,take:take.id});await rpc('wait_idle',{},650000);
  const p=(await rpc('state')).project,result=p.segments[0].transcriptions.at(-1);assert.equal(result.status,'succeeded');assert.ok(result.raw.length>0);assert.equal(p.segments[0].text,confirmed.segments[0].text);assert.equal(p.script_revision,confirmed.script_revision);
 });
 await check('正常終了・別CLIプロセスでの設定と台本の再読込',async()=>{
  await rpc('save_project');await rpc('close_project');await close();start();
  assert.equal((await rpc('machine_settings')).settings.asr_provider,'transcription_server');await rpc('open',{path:projectFile});
  const p=(await rpc('state')).project;assert.equal(p.id,confirmed.id);assert.equal(p.segments[0].takes[0].hash,originalTakes[0].hash);await rpc('close_project');await close();
 });
 await check('大きすぎる入力を非ゼロ終了で拒否',async()=>{
  const result=spawnSync(exe,['--settings-dir',path.join(run,'limit-settings')],{windowsHide:true,input:' '.repeat(1024*1024+1),encoding:'utf8'});assert.equal(result.status,2);
 });
 report.outcome='passed';
}catch(error){report.outcome='failed';report.error=String(error);console.error(String(error));process.exitCode=1;}
finally{
 if(processHandle?.exitCode===null){try{await close();}catch{processHandle.kill();report.outcome='failed';process.exitCode=1;}}
 server.close();fs.writeFileSync(path.join(run,'result.json'),JSON.stringify(report,null,2)+'\n');console.log('試験結果: '+path.join(run,'result.json'));
}
