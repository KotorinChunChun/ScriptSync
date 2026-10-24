/** 公開前の追跡内容・Git履歴を検査する。秘密の値は出力しない。 */
import fs from 'node:fs';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const tokenRules = [
 ['private-key', /-----BEGIN (?:RSA |EC |DSA |OPENSSH |ENCRYPTED )?PRIVATE KEY-----/g],
 ['github-token', /\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,})/g],
 ['openai-key', /\bsk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{24,}/g],
 ['aws-access-id', /\b(?:AKIA|ASIA)[A-Z0-9]{16}\b/g],
 ['google-key', /\bAIza[A-Za-z0-9_-]{30,}/g],
 ['slack-token', /\bxox[baprs]-[A-Za-z0-9-]{20,}/g],
 ['jwt', /\beyJ[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}/g],
 ['url-password', /https?:\/\/[^\s/:"']{2,}:[^\s/@"']{3,}@/g],
];
const privateName = /(?:^|\/)(?:\.env(?:\..*)?|id_rsa|id_ed25519|signing\.json|machine-settings\.json)$|\.(?:pfx|p12|pem|key|scriptsync)(?:$|\.data\/)|\.(?:wav|flac|mp3|mp4|webm|sqlite3?|db)$/i;
const credential = /["']?(?:api[_-]?key|access[_-]?token|auth[_-]?token|client[_-]?secret|password|passwd|ftp_pass|secret[_-]?key)["']?\s*[:=]\s*["']([^"'\r\n]{4,})["']/gi;
const placeholder = /^(?:example|placeholder|your[_ -]|replace[_ -]|test(?:[-_ ]|$)|dummy|fixture|\$|\{|<|\.\.\.)/i;

export function scanContent(name, buffer) {
 const findings=[];
 if(privateName.test(name) && !name.endsWith('/.env.example') && name!=='.env.example')findings.push({kind:'private-file',line:1});
 const text=buffer[0]===0xff && buffer[1]===0xfe ? buffer.subarray(2).toString('utf16le') : buffer.toString('utf8');
 for(const [kind,rule] of tokenRules){
  rule.lastIndex=0;
  for(const match of text.matchAll(rule))findings.push({kind,line:text.slice(0,match.index).split('\n').length});
 }
 // 一般的なパスワードのfixtureは、既知トークンと別に扱う。
 if(!name.startsWith('tests/') && !/\.test\.[cm]?[jt]s$/.test(name)){
  for(const match of text.matchAll(credential))if(!placeholder.test(match[1]))findings.push({kind:'credential-literal',line:text.slice(0,match.index).split('\n').length});
 }
 return findings;
}

function git(root,args,input){
 const result=spawnSync('git',['-c',`safe.directory=${root.replaceAll('\\','/')}`,'-c','core.quotepath=false',...args],{cwd:root,input,maxBuffer:256*1024*1024});
 if(result.error || result.status!==0)throw new Error('Git読取に失敗: '+args[0]);
 return result.stdout;
}

export function auditRepository(root,{history=false}={}){
 const names=git(root,['ls-files','-z']).toString('utf8').split('\0').filter(Boolean);
 const findings=[];
 for(const name of names){
  const file=path.join(root,name);
  if(!fs.statSync(file).isFile())throw new Error('通常ファイルではない追跡対象があります');
  findings.push(...scanContent(name,fs.readFileSync(file)).map(item=>({...item,path:name,scope:'working-tree'})));
 }
 let blobs=0;
 if(history){
  const historicalNames=new Set(git(root,['log','--all','--name-only','--format=','--diff-filter=A','--no-renames','-z']).toString('utf8').split('\0').map(name=>name.replace(/^\n+/, '')).filter(Boolean));
  for(const name of historicalNames)findings.push(...scanContent(name,Buffer.alloc(0)).map(item=>({...item,path:name,scope:'history-name'})));
  const objects=new Map(git(root,['rev-list','--objects','--all']).toString('utf8').trim().split('\n').map(line=>{const i=line.indexOf(' ');return i<0?[line,'']:[line.slice(0,i),line.slice(i+1)];}));
  const checked=git(root,['cat-file','--batch-check'],[...objects.keys()].join('\n')+'\n').toString('utf8').trim().split('\n');
  const ids=checked.filter(line=>line.split(' ')[1]==='blob').map(line=>line.split(' ')[0]);
  const data=git(root,['cat-file','--batch'],ids.join('\n')+'\n');
  let cursor=0;
  for(const id of ids){
   const end=data.indexOf(10,cursor);if(end<0)throw new Error('Git blobの応答が不正です');
   const fields=data.subarray(cursor,end).toString('utf8').split(' '),size=Number(fields[2]);
   if(fields[0]!==id || fields[1]!=='blob' || !Number.isSafeInteger(size) || size<0 || end+1+size>=data.length)throw new Error('Git blobのサイズが不正です');
   const name=objects.get(id);
   findings.push(...scanContent(name,data.subarray(end+1,end+1+size)).map(item=>({...item,path:name,scope:'history',blob:id})));
   cursor=end+1+size+1;blobs++;
  }
 }
 return {ok:findings.length===0,files:names.length,historyBlobs:blobs,findings};
}

if(process.argv[1] && path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 try{
  const args=process.argv.slice(2);if(args.some(arg=>arg!=='--history' && arg!=='--json'))throw new Error('使い方: check-public-source.mjs [--history] [--json]');
  const report=auditRepository(path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..'),{history:args.includes('--history')});
  if(args.includes('--json'))console.log(JSON.stringify(report));
  else{console.log(`${report.ok?'成功':'失敗'}: 公開前検査 ${report.files}ファイル / 履歴${report.historyBlobs} blobs / 指摘${report.findings.length}件`);for(const f of report.findings)console.log(`${f.scope}: ${f.path}:${f.line} ${f.kind}`);}
  process.exitCode=report.ok?0:1;
 }catch{console.error('公開前検査を完了できません。Gitと追跡ファイルを確認してください。');process.exitCode=2;}
}
