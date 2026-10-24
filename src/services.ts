export type ServiceSettings={service_url?:string;ai_provider?:string;tts_model?:string;tts_voice?:string};
let token='';
/** 接続トークンはプロジェクト・永続設定へ書かない。ブラウザーを閉じると破棄する。 */
export function setServiceToken(value:string){token=value;}
export function serviceToken(){return token;}
export function serviceUrl(value='http://localhost:4318'){const u=new URL(value);if(!['http:','https:'].includes(u.protocol)||u.username||u.password||u.search||u.hash)throw Error('接続先はhttp(s)のURLを指定してください');return u.href.replace(/\/$/,'');}
export async function serviceRequest(settings:ServiceSettings,path:string,body?:unknown){
 const response=await fetch(serviceUrl(settings.service_url)+path,{method:body===undefined?'GET':'POST',headers:{Authorization:`Bearer ${token}`,...(body===undefined?{}:{'Content-Type':'application/json'})},body:body===undefined?undefined:JSON.stringify(body),signal:AbortSignal.timeout(300_000)});
 if(!response.ok)throw Error(`サービス接続に失敗しました（HTTP ${response.status}）。URL・トークン・ブラウザーのローカル接続許可を確認してください。`);return response;
}
