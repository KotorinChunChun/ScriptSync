/** 効果音の定義を変える場合は dev/SOUND_EFFECTS.md も更新する。 */
export type Cue = 'start'|'saved'|'next'|'invalid';
export const soundDefinitions: Record<Cue,{file:string;volume:number}> = {
 start:{file:'/sounds/start.ogg',volume:.2},
 saved:{file:'/sounds/saved.ogg',volume:.2},
 next:{file:'/sounds/next.ogg',volume:.2},
 invalid:{file:'/sounds/invalid.ogg',volume:.12},
};
let queue:Promise<void>=Promise.resolve();
let lastInvalid=0;
export function playCue(cue:Cue):Promise<void>{
 if(cue==='invalid'&&Date.now()-lastInvalid<300)return Promise.resolve();
 if(cue==='invalid')lastInvalid=Date.now();
 const task=()=>new Promise<void>(resolve=>{
  const def=soundDefinitions[cue],audio=new Audio(def.file);audio.volume=def.volume;
  let finished=false;
  const done=()=>{if(finished)return;finished=true;clearTimeout(timer);audio.pause();audio.removeAttribute('src');resolve();};
  const timer=setTimeout(done,1800);
  audio.onended=done;audio.onerror=done;
  try{void audio.play().catch(done);}catch{done();}
 });
 queue=queue.then(task,task);return queue;
}
