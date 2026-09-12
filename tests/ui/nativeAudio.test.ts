import {afterEach,beforeEach,expect,it,vi} from "vitest";
let bridge:typeof import("../../src/nativeAudio");
let rpc:ReturnType<typeof vi.fn>,notice:ReturnType<typeof vi.fn>,browserPlay:ReturnType<typeof vi.spyOn>;
const snapshot=(more:Record<string,unknown>={})=>({playing:true,position:0,duration:2,file:"takes/a/audio.wav",error:null,...more});
async function settle(){for(let index=0;index<20;index++)await Promise.resolve();}
function audio(url="http://localhost/preview/a"){
 const value=new Audio(url);Object.defineProperty(value,"paused",{value:true,writable:true,configurable:true});
 Object.defineProperty(value,"ended",{value:false,writable:true,configurable:true});
 Object.defineProperty(value,"currentTime",{value:0,writable:true,configurable:true});
 return value;
}
beforeEach(async()=>{
 vi.resetModules();vi.useFakeTimers();
 browserPlay=vi.spyOn(HTMLMediaElement.prototype,"play").mockImplementation(function(this:HTMLMediaElement){Object.defineProperty(this,"paused",{value:false,writable:true});return Promise.resolve();});
 vi.spyOn(HTMLMediaElement.prototype,"pause").mockImplementation(function(this:HTMLMediaElement){Object.defineProperty(this,"paused",{value:true,writable:true});});
 rpc=vi.fn(async()=>snapshot());notice=vi.fn();bridge=await import("../../src/nativeAudio");
 bridge.installNativeAudio(rpc,notice);bridge.registerNativeAudio("http://localhost/preview/a","takes/a/audio.wav","hash-a");
});
afterEach(()=>{vi.clearAllTimers();vi.useRealTimers();vi.restoreAllMocks();vi.unstubAllGlobals();});
it("登録済み音声を常時無音化してnativeへ送り、未登録の汎用メディアは従来再生する",async()=>{
 const registered=audio();await registered.play();
 expect(registered.muted).toBe(true);expect(rpc).toHaveBeenCalledWith("playback_play",{file:"takes/a/audio.wav",hash:"hash-a",start:0});
 expect(browserPlay).toHaveBeenCalledTimes(1);
 rpc.mockClear();await audio("http://localhost/sound.wav").play();expect(rpc).not.toHaveBeenCalled();
});
it("効果音は独立したnative経路で再生して主音声を停止しない",async()=>{
 vi.stubGlobal('fetch',vi.fn(async()=>({ok:true,arrayBuffer:async()=>new ArrayBuffer(1)})));
 vi.stubGlobal('OfflineAudioContext',class {decodeAudioData=async()=>({numberOfChannels:1,length:2,getChannelData:()=>new Float32Array([.5,-.5])});});
 rpc.mockImplementation(async()=>snapshot({playing:false}));
 const pending=bridge.playNativeCue('/sounds/start.ogg',.2);await settle();await vi.advanceTimersByTimeAsync(100);await pending;
 expect(rpc).toHaveBeenCalledWith('playback_cue',{samples:[.5,.5,-.5,-.5],volume:.2});
 expect(rpc).not.toHaveBeenCalledWith('playback_stop');expect(browserPlay).not.toHaveBeenCalled();
});
it("効果音の指定デバイス失敗は通知しブラウザー既定出力へ逃がさない",async()=>{
 vi.stubGlobal('fetch',vi.fn(async()=>({ok:true,arrayBuffer:async()=>new ArrayBuffer(1)})));
 vi.stubGlobal('OfflineAudioContext',class {decodeAudioData=async()=>({numberOfChannels:1,length:1,getChannelData:()=>new Float32Array([0])});});
 rpc.mockRejectedValue(Error('指定デバイスなし'));
 await bridge.playNativeCue('/sounds/start.ogg',.2);
 expect(notice).toHaveBeenCalledWith('Error: 指定デバイスなし',true);expect(browserPlay).not.toHaveBeenCalled();
});
it("src切替直後にcurrentSrcが旧音声でも新しい音声を再生する",async()=>{
 bridge.registerNativeAudio("http://localhost/preview/b","takes/b/audio.wav","hash-b");
 const registered=audio("http://localhost/preview/b");
 Object.defineProperty(registered,"currentSrc",{value:"http://localhost/preview/a",configurable:true});
 await registered.play();
 expect(rpc).toHaveBeenCalledWith("playback_play",{file:"takes/b/audio.wav",hash:"hash-b",start:0});
});
it("停止とシークをnativeへ伝え、native時計の同期を再シークしない",async()=>{
 const registered=audio();await registered.play();rpc.mockClear();
 registered.currentTime=.5;registered.dispatchEvent(new Event("seeking"));await settle();
 expect(rpc).toHaveBeenCalledWith("playback_seek",{position:.5});
 registered.pause();registered.dispatchEvent(new Event("pause"));await settle();
 expect(rpc).toHaveBeenCalledWith("playback_pause");
 rpc.mockImplementation(async()=>snapshot({position:1}));
 await vi.advanceTimersByTimeAsync(100);await settle();expect(registered.currentTime).toBe(1);
 rpc.mockClear();registered.dispatchEvent(new Event("seeking"));await settle();expect(rpc).not.toHaveBeenCalledWith("playback_seek",expect.anything());
});
it("native失敗時はHTML音声を再生せず停止し、呼出元へエラーを返す",async()=>{
 rpc.mockImplementation(async(action:string)=>{if(action==="playback_play")throw Error("デバイスなし");return snapshot();});
 const registered=audio();await expect(registered.play()).rejects.toThrow("デバイスなし");
 expect(browserPlay).not.toHaveBeenCalled();expect(registered.paused).toBe(true);expect(rpc).toHaveBeenCalledWith("playback_stop");
});
it("再生デバイス切断の状態を通知してHTML時計も停止する",async()=>{
 const registered=audio();await registered.play();rpc.mockImplementation(async()=>snapshot({playing:false,error:"切断"}));
 await vi.advanceTimersByTimeAsync(100);await settle();expect(registered.paused).toBe(true);expect(notice).toHaveBeenCalledWith("切断",true);
});
