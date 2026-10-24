/** 自分で起動する隔離WebView2へCDP接続し、OS画面操作なしで検証する。 */
import fs from 'node:fs';
import path from 'node:path';
import net from 'node:net';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {spawn,spawnSync} from 'node:child_process';
import {once} from 'node:events';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const args=process.argv.slice(2),versionInfo=JSON.parse(fs.readFileSync(path.join(root,'dev/msix-version.json'),'utf8'));
const version=`${versionInfo.applicationVersion}.${versionInfo.revision}`;
let exe=path.join(root,'target/kotorintoys/scriptsync.exe'),withDevice=false;
for(let i=0;i<args.length;i++){
 if(args[i]==='--exe' && args[i+1])exe=path.resolve(args[++i]);
 else if(args[i]==='--with-audio-device')withDevice=true;
 else throw new Error('使い方: test-native.mjs [--exe EXE] [--with-audio-device]');
}
if(process.platform!=='win32')throw new Error('ネイティブ試験はWindows専用です');
if(!fs.existsSync(exe))throw new Error('画面内包EXEを先にビルドしてください: npm run verify:native');
const run=path.join(root,`dev/history/v${version}/logs/native-${new Date().toISOString().replaceAll(/[:.]/g,'-')}-${crypto.randomUUID().slice(0,8)}`);
fs.mkdirSync(run,{recursive:true});
const sid=spawnSync('whoami',['/user','/fo','csv','/nh'],{encoding:'utf8'}).stdout?.match(/S-1-5-\d+(?:-\d+)+/)?.[0];
if(!sid)throw new Error('現在利用者のSIDを確認できません');
const pipe=`\\\\.\\pipe\\KotorinToys.${sid}.script-sync`;
const report={version,executableSha256:crypto.createHash('sha256').update(fs.readFileSync(exe)).digest('hex'),computerUse:false,checks:[],pageErrors:[],consoleErrors:[],artifacts:[],outcome:'running'};
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function until(operation,message,timeout=20000){
 const end=Date.now()+timeout;let last;
 do{try{const value=await operation();if(value)return value;}catch(error){last=error;}await delay(100);}while(Date.now()<end);
 throw new Error(message+(last?' / '+last.message:''));
}
function request(command,pid){return new Promise((resolve,reject)=>{
 const socket=net.createConnection(pipe),id=crypto.randomUUID();let data='';
 const timer=setTimeout(()=>{socket.destroy();reject(new Error('管理通信の期限切れ'));},3000);
 socket.once('error',error=>{clearTimeout(timer);reject(error);});
 socket.once('connect',()=>socket.write(JSON.stringify({command,requestId:id,...pid===undefined?{}:{expectedPid:pid}})+'\n'));
 socket.on('data',chunk=>{
  data+=chunk.toString('utf8');if(data.length>16384){socket.destroy();clearTimeout(timer);reject(new Error('管理応答が大きすぎます'));return;}
  if(data.includes('\n')){clearTimeout(timer);socket.end();try{const result=JSON.parse(data.split('\n')[0]);assert.equal(result.requestId,id);resolve(result);}catch(error){reject(error);}}
 });
});}
async function check(name,operation){const started=Date.now();try{const detail=await operation();report.checks.push({name,passed:true,milliseconds:Date.now()-started,...detail?{detail}: {}});console.log('PASS: '+name);}catch(error){report.checks.push({name,passed:false,error:String(error)});throw error;}}
function wave(seconds){
 const count=Math.round(seconds*48000)*2,data=Buffer.alloc(44+count*4);
 data.write('RIFF');data.writeUInt32LE(data.length-8,4);data.write('WAVEfmt ',8);data.writeUInt32LE(16,16);data.writeUInt16LE(3,20);data.writeUInt16LE(2,22);data.writeUInt32LE(48000,24);data.writeUInt32LE(48000*8,28);data.writeUInt16LE(8,32);data.writeUInt16LE(32,34);data.write('data',36);data.writeUInt32LE(count*4,40);return data;
}
let child,browser,page,stopped=false;
const output=fs.openSync(path.join(run,'process.log'),'a');
try{
 let existing;try{existing=await request('status');}catch(error){if(error.code!=='ENOENT')throw error;}
 if(existing)throw new Error('既存ScriptSyncが動作中です。停止・変更せず試験を中止します');
 const server=net.createServer();server.listen(0,'127.0.0.1');await once(server,'listening');const port=server.address().port;await new Promise(resolve=>server.close(resolve));
 child=spawn(exe,['--startup','--settings-dir',path.join(run,'settings')],{cwd:root,windowsHide:true,stdio:['ignore',output,output],env:{...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1 --autoplay-policy=no-user-gesture-required --disable-background-timer-throttling --disable-renderer-backgrounding`}});
 await until(async()=>{if(child.exitCode!==null)throw new Error('試験EXEが終了しました');const response=await fetch(`http://127.0.0.1:${port}/json/version`,{signal:AbortSignal.timeout(1000)});return response.ok;},'試験用WebView2の準備に失敗',30000);
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
 page=await until(async()=>browser.contexts().flatMap(context=>context.pages()).find(p=>/^https?:\/\/tauri\.localhost/.test(p.url())),'アプリ画面を確認できません');
 page.setDefaultTimeout(10000);
 page.on('pageerror',error=>report.pageErrors.push(String(error)));
 page.on('console',message=>{if(message.type()==='error')report.consoleErrors.push(message.text());});
 const rpc=(action,payload={})=>page.evaluate(({action,payload})=>window.__TAURI_INTERNALS__.invoke('command',{action,payload}),{action,payload});
 await check('CSP下で画面・実IPC・配布版・管理readyが成立',async()=>{
  await page.locator('#app button').first().waitFor();const status=await until(async()=>{const r=await request('status');return r.ready?r:false;},'画面準備通知が届きません');
  assert.equal(status.pid,child.pid);assert.equal(status.version,version);assert.equal(status.protocolVersion,2);assert.equal((await rpc('state')).project,null);
 });
 await check('inline scriptをCSPが拒否',async()=>{
  const blocked=await page.evaluate(()=>{window.__scriptsyncInlineProbe=false;const s=document.createElement('script');s.textContent='window.__scriptsyncInlineProbe=true';document.head.append(s);s.remove();return !window.__scriptsyncInlineProbe;});assert.equal(blocked,true);
 });
 const project=path.join(run,'fixture.scriptsync'),source=path.join(run,'script.txt'),input=path.join(run,'silence.wav');
 fs.writeFileSync(source,'自動試験の一行目です。\n自動試験の二行目です。\n');fs.writeFileSync(input,wave(2));
 await check('ファイル選択画面なしで新規作成・台本取込、既存入口の上書き拒否',async()=>{
  await rpc('new',{path:project});const before=fs.readFileSync(project);await assert.rejects(rpc('new',{path:project}));assert.deepEqual(fs.readFileSync(project),before);
  await rpc('import_script',{path:source});assert.equal((await rpc('state')).project.segments.length,2);
 });
 let state=await rpc('state'),first=state.project.segments[0],second=state.project.segments[1];
 await check('WAV取込・テイク結合で元音声を保持',async()=>{
  await rpc('import_audio',{segment:first.id,path:input});await rpc('import_audio',{segment:first.id,path:input});
  state=await rpc('state');first=state.project.segments[0];const [a,b]=first.takes;
  const joined=await rpc('join_takes',{project_id:state.project.id,segment:first.id,before:a.id,after:b.id,before_hash:a.hash,after_hash:b.hash});
  state=await rpc('state');first=state.project.segments[0];assert.equal(first.takes.length,3);assert.equal(first.takes[0].hash,a.hash);assert.equal(first.takes[1].hash,b.hash);assert.ok(first.takes[2].duration>3.99);return {duration:first.takes[2].duration};
 });
 await check('専用音声protocolのRange取得・HTML音声のデコードと時計',async()=>{
  const take=first.takes[0],url=await rpc('audio',{file:take.file,hash:take.hash});
  const media=await page.evaluate(async url=>{
   const response=await fetch(url,{headers:{Range:'bytes=0-43'}}),bytes=new Uint8Array(await response.arrayBuffer());
   const audio=new Audio();audio.muted=true;audio.src=url;
   await Promise.race([new Promise((resolve,reject)=>{audio.onloadedmetadata=resolve;audio.onerror=()=>reject(new Error('HTML音声読込失敗'));}),new Promise((_,reject)=>setTimeout(()=>reject(new Error('音声metadataの期限切れ')),7000))]);
   await audio.play();await new Promise(resolve=>setTimeout(resolve,350));const position=audio.currentTime;audio.pause();audio.removeAttribute('src');audio.load();
   return {status:response.status,bytes:bytes.length,riff:String.fromCharCode(...bytes.slice(0,4)),position};
  },url);
  assert.equal(media.status,206);assert.equal(media.bytes,44);assert.equal(media.riff,'RIFF');assert.ok(media.position>0);return media;
 });
 await page.locator('[data-tab="recording"]').first().click();
 await check('編集直後のPageDownが移動先の台本を汚さない',async()=>{
  await page.locator('[data-action="edit_script"]').click();const editor=page.getByLabel('台本の修正案');await editor.fill('一行目だけの自動修正です。');await editor.press('PageDown');
  await until(async()=>{const s=await rpc('state');return s.project.segments[0].recording_draft==='一行目だけの自動修正です。' && !s.project.segments[1].recording_draft;},'修正原稿の保存先が不正です');
  await page.keyboard.press('PageUp');
 });
 if(withDevice){
  await check('無音fixtureを実WASAPIで再生・一時停止・先頭停止',async()=>{
   const t=first.takes[0];await rpc('playback_play',{file:t.file,hash:t.hash,start:0});
   await until(async()=>{const r=await rpc('playback_state');if(r.error)throw new Error(r.error);return r.position>.1;},'WASAPIの再生時計が進みません',5000);
   const paused=await rpc('playback_pause');assert.equal(paused.playing,false);assert.ok(paused.position>.1);const stopped=await rpc('playback_stop');assert.equal(stopped.position,0);
  });
  await check('テイク編集のトグル・途中削除・再生バーと時刻・停止を実画面で検証',async()=>{
   await page.locator(`[data-trim-edit="${first.takes[0].id}"]`).click();
   const modal=page.locator('.take-trim-modal');await modal.waitFor();
   assert.equal(await modal.locator('[data-trim-start]').isDisabled(),true);
   await modal.locator('[data-trim-edit-mode="on"]').click();assert.equal(await modal.locator('[data-trim-start]').isEnabled(),true);
   const cut=modal.locator('[data-trim-cut-toggle]');await cut.click();assert.match(await cut.innerText(),/解除/);await cut.click();assert.match(await cut.innerText(),/追加/);
   await modal.locator('[data-trim-play]').click();
   const marker=modal.locator('[data-trim-playhead]');
   await until(async()=>parseFloat(await marker.evaluate(element=>element.style.left))>1,'編集画面の再生バーが動きません',5000);
   const position=await modal.locator('[data-trim-playhead-time]').innerText();assert.match(position,/\d/);
   const image='native-edit.png';await page.screenshot({path:path.join(run,image)});report.artifacts.push(image);
   await modal.locator('[data-trim-play]').click();assert.match(await modal.locator('[data-trim-play]').innerText(),/再生/);
   await modal.locator('[data-trim-stop]').click();assert.equal(parseFloat(await marker.evaluate(element=>element.style.left)),0);
   await modal.locator('[data-trim-cancel]').click();await modal.waitFor({state:'detached'});return {position};
  });
 }else report.checks.push({name:'実WASAPIの無音試験',skipped:true,reason:'--with-audio-device指定時のみ。可聴品質は別途確認'});
 await check('設定画面中の管理停止拒否と対象PIDの照合',async()=>{
  await page.locator('[data-action="settings"]').click();await page.locator('.settings-overlay').waitFor();
  assert.equal((await request('stop',child.pid+1)).result,'refused');assert.equal((await request('stop',child.pid)).result,'refused');
  await page.locator('.settings-overlay [data-cancel]').click();await page.locator('.settings-overlay').waitFor({state:'detached'});
 });
 const shot=path.join(run,'native.png');await page.screenshot({path:shot});report.artifacts.push('native.png');
 assert.deepEqual(report.pageErrors,[]);
 await rpc('close_project');
 await check('試験用プロセスを管理契約から正常停止',async()=>{
  await until(async()=>{const r=await request('stop',child.pid);return r.result==='ok' || r.result==='accepted';},'正常停止が受理されません',7000);
  await until(async()=>child.exitCode!==null,'停止後もプロセスが残っています',7000);stopped=true;
 });
 report.outcome='passed';
}catch(error){report.outcome='failed';report.error=String(error);console.error('FAIL: '+String(error));if(page)try{await page.screenshot({path:path.join(run,'failure.png')});report.artifacts.push('failure.png');}catch{}}
finally{
 if(child && !stopped && child.exitCode===null){
  try{await request('stop',child.pid);await until(async()=>child.exitCode!==null,'終了待機',3500);}catch{}
  if(child.exitCode===null){child.kill();report.cleanup='forced-owned-fixture';report.outcome='failed';}
 }
 if(browser)try{await browser.close();}catch{}fs.closeSync(output);
 fs.writeFileSync(path.join(run,'result.json'),JSON.stringify(report,null,2)+'\n');
 const escape=s=>String(s).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
 fs.writeFileSync(path.join(run,'report.html'),`<!doctype html><meta charset="utf-8"><title>ScriptSync自動試験</title><h1>ScriptSync ${escape(version)}：${escape(report.outcome)}</h1><p>Computer Use不使用。専用設定・合成データ。実マイクと可聴品質は対象外。</p><ul>${report.checks.map(c=>`<li>${c.skipped?'省略':c.passed?'成功':'失敗'}：${escape(c.name)} ${escape(c.error??'')}</li>`).join('')}</ul><pre>${escape(report.error??'')}</pre>${report.artifacts.map(name=>`<p><img style="max-width:100%" src="${escape(name)}" alt="自動取得した試験画面"></p>`).join('')}`);
 console.log('試験結果: '+path.join(run,'report.html'));process.exitCode=report.outcome==='passed'?0:1;
}
