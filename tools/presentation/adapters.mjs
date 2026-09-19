import { createHash, randomUUID } from 'node:crypto';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { ensure, text } from './model.mjs';

export function powerpoint(action, path, slideId) {
  ensure(['open','goto'].includes(action), 'PowerPoint操作が不正です');text(path,4096);
  ensure(/\.pptx$/i.test(path), 'PPTXを指定してください');
  if(action==='goto')ensure(Number.isSafeInteger(slideId)&&slideId>0,'スライドIDが不正です');
  return new Promise((resolve,reject)=>{
    const child=spawn('pwsh',['-NoLogo','-NoProfile','-NonInteractive','-File',fileURLToPath(new URL('./powerpoint.ps1',import.meta.url))],{windowsHide:true,stdio:['pipe','pipe','pipe']});
    let out='',err='';const timer=setTimeout(()=>{child.kill();reject(new Error('PowerPointが30秒以内に応答しませんでした'));},30000);
    child.stdout.setEncoding('utf8');child.stderr.setEncoding('utf8');
    child.stdout.on('data',v=>{out+=v;});child.stderr.on('data',v=>{err+=v;});
    child.on('error',e=>{clearTimeout(timer);reject(e);});child.on('close',code=>{clearTimeout(timer);if(code!==0)return reject(new Error(err.trim()||'PowerPointを操作できません'));try{resolve(JSON.parse(out));}catch{reject(new Error('PowerPointの応答が不正です'));}});
    child.stdin.on('error',()=>{});child.stdin.end(JSON.stringify({action,path,slideId}));
  });
}

const sha = value => createHash('sha256').update(value).digest('base64');
export class ObsClient {
  socket=null;pending=new Map();connected=false;
  disconnect(){this.connected=false;if(this.socket)this.socket.close();this.socket=null;for(const p of this.pending.values()){clearTimeout(p.timer);p.reject(new Error('OBSとの接続が切れました'));}this.pending.clear();}
  async connect(address,password='') {
    text(address,500);text(password,1000);const url=new URL(address);
    ensure(url.protocol==='ws:' && ['127.0.0.1','localhost','[::1]'].includes(url.hostname) && !url.username && !url.password,'試作では同じPCのOBS（ws://127.0.0.1:4455）に接続してください');
    this.disconnect();
    await new Promise((resolve,reject)=>{
      const ws=new WebSocket(address);this.socket=ws;
      let settled=false;const fail=e=>{if(!settled){settled=true;clearTimeout(timer);reject(e);}if(this.socket===ws)this.disconnect();};
      const timer=setTimeout(()=>fail(new Error('OBS接続がタイムアウトしました')),8000);
      ws.addEventListener('error',()=>fail(new Error('OBSへ接続できません。WebSocket設定を確認してください')));
      ws.addEventListener('close',()=>{if(this.socket===ws)fail(new Error('OBS接続が終了しました。パスワードを確認してください'));});
      ws.addEventListener('message',event=>{
        if(this.socket!==ws)return;
        try {
          const msg=JSON.parse(event.data);
          if(msg.op===0){const d={rpcVersion:1,eventSubscriptions:0};if(msg.d.authentication)d.authentication=sha(sha(password+msg.d.authentication.salt)+msg.d.authentication.challenge);ws.send(JSON.stringify({op:1,d}));password='';}
          else if(msg.op===2){this.connected=true;settled=true;clearTimeout(timer);resolve();}
          else if(msg.op===7){const p=this.pending.get(msg.d.requestId);if(p){this.pending.delete(msg.d.requestId);clearTimeout(p.timer);if(msg.d.requestStatus.result)p.resolve(msg.d.responseData??{});else p.reject(new Error(msg.d.requestStatus.comment||'OBS操作に失敗しました'));}}
        }catch{fail(new Error('OBSの応答形式が不正です'));}
      });
    });
    return this.request('GetSceneList');
  }
  request(requestType,requestData={}) {
    ensure(this.connected && this.socket?.readyState===1,'OBSへ接続してください');
    return new Promise((resolve,reject)=>{const id=randomUUID(),timer=setTimeout(()=>{this.pending.delete(id);reject(new Error('OBS操作がタイムアウトしました'));},5000);this.pending.set(id,{resolve,reject,timer});this.socket.send(JSON.stringify({op:6,d:{requestId:id,requestType,requestData}}));});
  }
  scene(name){text(name,200);return this.request('SetCurrentProgramScene',{sceneName:name});}
}
