import {test} from 'node:test';import assert from 'node:assert/strict';import http from 'node:http';import {WebSocketServer} from 'ws';import {configuration,createGateway,codexText,improve} from './server.mjs';
const listen=server=>new Promise(r=>server.listen(0,'127.0.0.1',()=>r(`http://127.0.0.1:${server.address().port}`)));
test('認証・Origin・JSON形式・上流秘密の境界',async t=>{
 const config={...configuration({}),token:'synthetic-gateway-token-0123456789',origins:['https://excel-chunchun.sakura.ne.jp']};const server=createGateway(config),url=await listen(server);t.after(()=>server.close());
 assert.equal((await fetch(url+'/health')).status,401);
 const headers={Authorization:`Bearer ${config.token}`};
 assert.equal((await fetch(url+'/health',{headers:{...headers,Origin:'https://evil.invalid'}})).status,403);
 const response=await fetch(url+'/health',{headers:{...headers,Origin:config.origins[0]}});assert.equal(response.status,200);assert.equal(response.headers.get('Access-Control-Allow-Origin'),config.origins[0]);assert.deepEqual(Object.keys(await response.json()).sort(),['providers','ready','version']);
 assert.equal((await fetch(url+'/v1/text-improve',{method:'POST',headers,body:'{}'})).status,415);
 assert.equal((await fetch(url+'/unexpected',{headers})).status,404);
});
test('ResponsesとChat Completionsの本文・キー・失敗を処理',async t=>{
 let request;const upstream=http.createServer(async(req,res)=>{let data='';for await(const c of req)data+=c;request={url:req.url,key:req.headers.authorization,body:JSON.parse(data)};res.setHeader('Content-Type','application/json');res.end(JSON.stringify(req.url==='/responses'?{status:'completed',output:[{type:'message',content:[{type:'output_text',text:'校正済み'}]}]}:{choices:[{message:{content:'互換API'}}]}));});const url=await listen(upstream);t.after(()=>upstream.close());const config={...configuration({}),api:url,apiKey:'synthetic-api-key'};
 assert.equal(await improve(config,{provider:'openai',prompt:'合成原稿',model:'configured-model'}),'校正済み');assert.equal(request.key,'Bearer synthetic-api-key');assert.equal(request.body.store,false);assert.equal(request.body.model,'configured-model');
 assert.equal(await improve(config,{provider:'chat-completions',prompt:'合成原稿'}),'互換API');assert.equal(request.url,'/chat/completions');
 await assert.rejects(improve(config,{provider:'unknown',prompt:'x'}));await assert.rejects(improve(config,{prompt:''}));
});
test('Codex初期化・thread/turn・認証・完了本文',async t=>{
 const server=http.createServer(),wsServer=new WebSocketServer({server});const url=(await listen(server)).replace('http:','ws:');t.after(()=>{wsServer.close();server.close();});const methods=[];
 wsServer.on('connection',(ws,req)=>{assert.equal(req.headers.authorization,'Bearer synthetic-codex-token');ws.on('message',buffer=>{const m=JSON.parse(buffer.toString());methods.push(m.method);if(m.id===1)ws.send(JSON.stringify({id:1,result:{}}));if(m.id===2){assert.equal(m.params.approvalPolicy,'never');assert.equal(m.params.sandbox,'read-only');ws.send(JSON.stringify({id:2,result:{thread:{id:'synthetic-thread'}}}));}if(m.id===3){ws.send(JSON.stringify({method:'item/agentMessage/delta',params:{delta:'完了本文'}}));ws.send(JSON.stringify({method:'turn/completed',params:{turn:{status:'completed'}}}));}});});
 assert.equal(await codexText({...configuration({}),codex:url,codexToken:'synthetic-codex-token'},'合成本文','model'),'完了本文');assert.deepEqual(methods.slice(0,4),['initialize','initialized','thread/start','turn/start']);
});
test('Codex失敗を成功として返さない',async t=>{const server=http.createServer(),wss=new WebSocketServer({server});const url=(await listen(server)).replace('http:','ws:');t.after(()=>{wss.close();server.close();});wss.on('connection',ws=>ws.on('message',()=>ws.send(JSON.stringify({method:'turn/completed',params:{turn:{status:'failed'}}}))));await assert.rejects(codexText({...configuration({}),codex:url},'x','m'));});
