export type StoredProject={project:any;audio:Record<string,Blob>};
/** 1トランザクションに本文と音声を入れ、失敗時に従来の正常データを保持する。 */
export class BrowserStore{
 private db?:IDBDatabase;
 async open(){if(this.db)return this.db;return this.db=await new Promise<IDBDatabase>((resolve,reject)=>{const r=indexedDB.open('scriptsync-full-web',1);r.onupgradeneeded=()=>{r.result.createObjectStore('projects',{keyPath:'id'});r.result.createObjectStore('settings');};r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(Error('端末の保存領域を開けません'));});}
 async load(id?:string):Promise<StoredProject|undefined>{const db=await this.open();return new Promise((resolve,reject)=>{const t=db.transaction('projects'),s=t.objectStore('projects'),r=id?s.get(id):s.getAll();r.onsuccess=()=>{const value=id?r.result:r.result.sort((a:any,b:any)=>b.saved-a.saved)[0];resolve(value?.data)};r.onerror=()=>reject(r.error);});}
 async save(data:StoredProject){const db=await this.open();await new Promise<void>((resolve,reject)=>{const t=db.transaction('projects','readwrite');t.objectStore('projects').put({id:data.project.id,saved:Date.now(),data});t.oncomplete=()=>resolve();t.onabort=t.onerror=()=>reject(Error('端末保存に失敗しました。空き容量を確認し、ZIPで退避してください'));});}
 async settings(value?:unknown):Promise<any>{const db=await this.open();return new Promise((resolve,reject)=>{const t=db.transaction('settings',value===undefined?'readonly':'readwrite'),s=t.objectStore('settings');if(value===undefined){const r=s.get('machine');r.onsuccess=()=>resolve(r.result??{});}else{s.put(value,'machine');t.oncomplete=()=>resolve(value);}t.onabort=t.onerror=()=>reject(Error('設定を保存できません'));});}
}
