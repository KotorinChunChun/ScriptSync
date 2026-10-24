/// <reference lib="webworker" />
import {pipeline,env} from '@huggingface/transformers';
env.allowLocalModels=false;env.useBrowserCache=false;env.useCustomCache=true;env.backends.onnx.wasm!.numThreads=1;
const cacheReady=caches.open('scriptsync-full-whisper').then(cache=>{env.customCache=cache;});
let loadedModel='',recognizer:any;
self.onmessage=async(event:MessageEvent)=>{const {id,audio,model='tiny'}=event.data;try{await cacheReady;if(!['tiny','base','small'].includes(model))throw Error('Web版の認識モデルはtiny/base/smallを選択してください');if(loadedModel!==model){recognizer=await pipeline('automatic-speech-recognition',`Xenova/whisper-${model}`,{device:'wasm',dtype:'q8',progress_callback:(progress:unknown)=>self.postMessage({id,progress})});loadedModel=model;}if(audio){const result=await recognizer(audio,{language:'japanese',task:'transcribe',return_timestamps:true,chunk_length_s:30,stride_length_s:5});self.postMessage({id,result});}else self.postMessage({id,ready:true});}catch(error){self.postMessage({id,error:String(error)});}};
