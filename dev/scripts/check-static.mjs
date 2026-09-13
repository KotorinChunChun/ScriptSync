import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
export function checkStatic(root,{releasePage=false}={}){
 const errors=[];let checks=0;const expect=(ok,message)=>{checks++;if(!ok)errors.push(message);};
 const read=name=>fs.readFileSync(path.join(root,name),'utf8');
 const json=name=>JSON.parse(read(name));
 const run=(name,fn)=>{try{fn();}catch(error){errors.push(`${name}: ${error.message}`);}};
 let app,version;
 run('版情報',()=>{
  const pkg=json('package.json'),lock=json('package-lock.json'),release=json('dev/msix-version.json'),tauri=json('src-tauri/tauri.conf.json');
  app=pkg.version;version=`${app}.${release.revision}`;
  expect(/^\d+\.\d+\.\d+$/.test(app)&&app.split('.').every(n=>Number(n)<=65535),'アプリ版は0～65535の3桁');
  expect(Number.isInteger(release.revision)&&release.revision>=0&&release.revision<=65535,'MSIX再配布番号が不正');
  for(const [name,value] of [['package-lock',lock.version],['package-lock root',lock.packages?.['']?.version],['MSIX正本',release.applicationVersion],['Tauri',tauri.version]])expect(value===app,`${name}のアプリ版が不一致`);
  const cargo=read('src-tauri/Cargo.toml').match(/\[package\]([\s\S]*?)(?=\n\[|$)/)?.[1];
  expect(cargo?.match(/^version\s*=\s*"([^"]+)"/m)?.[1]===app,'Cargo.toml版が不一致');
  const own=read('src-tauri/Cargo.lock').split('[[package]]').find(block=>/^name = "scriptsync"$/m.test(block));
  expect(own?.match(/^version = "([^"]+)"/m)?.[1]===app,'Cargo.lock版が不一致');
  expect(tauri.app?.windows?.[0]?.title?.endsWith(` ${version}`),'nativeタイトルのMSIX版が不一致');
  expect(read('index.html').match(/<title>(.*?)<\/title>/s)?.[1]?.endsWith(` ${version}`),'HTMLタイトルのMSIX版が不一致');
  expect(read('src/appInfo.ts').includes('../dev/msix-version.json?raw'),'アプリ情報がMSIX正本を参照していない');
 });
 const skills=['scriptsync-project-validation','scriptsync-project-release'];
 for(const skill of skills)run(skill,()=>{
  const name=`.agents/skills/${skill}/SKILL.md`,text=read(name);
  const front=text.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  expect(!!front&&new RegExp(`^name: ${skill}$`,'m').test(front[1].replaceAll('\r','')),`${skill}: nameが不正`);
  expect(!!front&&/^description: .+/m.test(front[1]),`${skill}: descriptionがない`);
  for(const match of text.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)){
   const target=match[1];if(/^(https?:|#)/.test(target))continue;
   expect(fs.existsSync(path.resolve(root,path.dirname(name),target.split('#')[0])),`${skill}: 参照先なし ${target}`);
  }
 });
 if(releasePage)run('配布ページ',()=>{
  const html=read('dev/release-page/index.html');
  expect(read('README.md').includes('現在の配布版は'+version+'（'),'READMEの現行配布版が不一致');
  const versions=[...html.matchAll(/\b\d+\.\d+\.\d+\.\d+\b/g)].map(m=>m[0]);
  expect(versions.length>0&&versions.every(v=>v===version),'配布ページのMSIX版が現在版と不一致');
  for(const target of [`./ScriptSync_${version}_x64.msix`,'./ScriptSync.appinstaller','./ScriptSync.cer','./icon.png','../assets/styles.css'])expect(html.includes(`href="${target}"`)||html.includes(`src="${target}"`),`配布リンクがない: ${target}`);
  expect(html.includes('https://excel-chunchun.sakura.ne.jp/download/ScriptSync/'),'配布ページcanonicalが不一致');
 });
 return {ok:errors.length===0,checks,appVersion:app,msixVersion:version,scope:releasePage?'release-page':'local',errors};
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 const args=process.argv.slice(2);const allowed=new Set(['--release-page','--json']);
 if(args.some(arg=>!allowed.has(arg))){console.error('使い方: node dev/scripts/check-static.mjs [--release-page] [--json]');process.exitCode=2;}
 else{const report=checkStatic(path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'),{releasePage:args.includes('--release-page')});
 console.log(args.includes('--json')?JSON.stringify(report):`${report.ok?'成功':'失敗'}: 静的チェック ${report.checks}項目 / MSIX ${report.msixVersion}`);
 if(!args.includes('--json'))report.errors.forEach(error=>console.error(`- ${error}`));process.exitCode=report.ok?0:1;}
}
