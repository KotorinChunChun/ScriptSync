//! Windowsの選択済み再生デバイスへ、既存録音基盤を通して試聴する。
use crate::{audio, domain::Result, storage::{hash_file, safe}};
use audio_mixer_backend::{audio::buffer::AudioRingBuffer, windows::{devices::{resolve_endpoint_id, list_devices, DeviceDirection, DEFAULT_CONSOLE_ENDPOINT}, wasapi_render::{start_render, RenderHandle}}};
use serde::Serialize;
use std::{path::Path, sync::{Arc, atomic::{AtomicBool, AtomicUsize, AtomicU32, Ordering}}, thread::{self, JoinHandle}, time::Duration};

const SAMPLES_PER_SECOND: f64 = 96000.0;
#[derive(Clone, Debug, Serialize)]
pub struct PlaybackSnapshot {
    pub playing: bool, pub position: f64, pub duration: f64, pub file: String,
    pub error: Option<String>,
}
struct Loaded { samples: Arc<Vec<f32>>, gain: Arc<AtomicU32>, file: String, endpoint: String, position: usize, end: usize }
struct Active {
    stop: Arc<AtomicBool>, ended: Arc<AtomicBool>, supplied: Arc<AtomicUsize>,
    ring: AudioRingBuffer, worker: Option<JoinHandle<()>>, render: Option<RenderHandle>, start: usize,
}
impl Active {
    fn position(&self) -> usize {
        let padding=self.render.as_ref().map_or(0,|render|render.padding_frames().load(Ordering::Relaxed) as usize*2);
        self.start + self.supplied.load(Ordering::Acquire).saturating_sub(self.ring.len_samples()).saturating_sub(padding)
    }
}
impl Drop for Active {
    fn drop(&mut self) {
        self.stop.store(true,Ordering::Release);
        if let Some(worker)=self.worker.take() { let _=worker.join(); }
        if let Some(render)=self.render.take() { render.stop(); }
    }
}
#[derive(Default)]
pub struct NativePlayback { loaded: Option<Loaded>, active: Option<Active>, error: Option<String> }

/// 空設定は開始時のWindows既定を解決する。明示指定を別機器へ切り替えない。
pub fn resolve_selected_endpoint(endpoint: &str, direction: DeviceDirection) -> Result<String> {
    resolve_selected_with(endpoint, direction, |selector, direction| {
        resolve_endpoint_id(selector,direction).map_err(|e|e.to_string())
    }, |id, direction| {
        let devices=list_devices(direction).map_err(|e|e.to_string())?;
        if devices.iter().any(|device|device.endpoint_id==id) {Ok(())}
        else {Err("指定された音声デバイスが見つかりません。設定を確認してください".into())}
    })
}
fn resolve_selected_with(endpoint: &str, direction: DeviceDirection,
    resolve: impl FnOnce(&str, DeviceDirection)->Result<String>,
    validate: impl FnOnce(&str, DeviceDirection)->Result<()>) -> Result<String> {
    let id=resolve(if endpoint.is_empty(){DEFAULT_CONSOLE_ENDPOINT}else{endpoint},direction)?;
    validate(&id,direction)?;
    Ok(id)
}
fn sample_position(seconds: f64, duration: f64) -> Result<usize> {
    if !seconds.is_finite() || seconds < 0.0 || seconds > duration { return Err("試聴位置が音声の範囲外です".into()); }
    Ok((seconds * 48000.0).floor() as usize * 2)
}
impl NativePlayback {
    pub fn play(&mut self, root: &Path, file: &str, hash: &str, endpoint: &str, start: f64, end: Option<f64>) -> Result<PlaybackSnapshot> {
        let path=safe(root,file)?;
        if hash_file(&path)? != hash { return Err("音声が差し替えられています".into()); }
        let samples=audio::samples(&path)?;
        let duration=samples.len() as f64/SAMPLES_PER_SECOND;
        let begin=sample_position(start,duration)?;
        let finish=sample_position(end.unwrap_or(duration),duration)?.min(samples.len());
        if begin>=finish { return Err("試聴区間は開始より終了を後にしてください".into()); }
        let resolved=resolve_selected_endpoint(endpoint,DeviceDirection::Render)?;
        self.stop();
        self.loaded=Some(Loaded {samples:Arc::new(samples),gain:Arc::new(AtomicU32::new(1.0f32.to_bits())),file:file.into(),endpoint:resolved,position:begin,end:finish});
        self.resume()
    }
    pub fn play_samples(&mut self, samples: Vec<f32>, endpoint: &str, volume: f64) -> Result<PlaybackSnapshot> {
        if samples.is_empty() || samples.len()>96000*5 || samples.len()%2!=0 || samples.iter().any(|v|!v.is_finite() || v.abs()>1.0) {
            return Err("効果音データが不正です".into());
        }
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {return Err("効果音の音量が不正です".into());}
        let endpoint=resolve_selected_endpoint(endpoint,DeviceDirection::Render)?;
        let end=samples.len();self.close();
        self.loaded=Some(Loaded {samples:Arc::new(samples),gain:Arc::new(AtomicU32::new((volume as f32).to_bits())),file:"cue".into(),endpoint,position:0,end});
        self.resume()
    }
    pub fn resume(&mut self) -> Result<PlaybackSnapshot> {
        if self.active.is_some() { return Ok(self.snapshot()); }
        let loaded=self.loaded.as_ref().ok_or("試聴する音声がありません")?;
        if loaded.position>=loaded.end { return Ok(self.snapshot()); }
        let ring=AudioRingBuffer::new(9600); // 100ms分。長い音声をリングへ一括複製しない。
        let render=start_render(loaded.endpoint.clone(),ring.clone()).map_err(|e|e.to_string())?;
        render.wait_until_running(Duration::from_secs(3)).map_err(|e|e.to_string())?;
        let stop=Arc::new(AtomicBool::new(false));let worker_stop=stop.clone();
        let ended=Arc::new(AtomicBool::new(false));let worker_ended=ended.clone();
        let supplied=Arc::new(AtomicUsize::new(0));let worker_supplied=supplied.clone();
        let worker_ring=ring.clone();let samples=loaded.samples.clone();let gain=loaded.gain.clone();let begin=loaded.position;let end=loaded.end;
        let worker=thread::Builder::new().name("scriptsync-preview-feed".into()).spawn(move|| {
            let mut cursor=begin;
            while !worker_stop.load(Ordering::Acquire) && cursor<end {
                let free=worker_ring.capacity_samples().saturating_sub(worker_ring.len_samples())/2*2;
                let count=free.min(end-cursor);
                if count>0 { let volume=f32::from_bits(gain.load(Ordering::Relaxed));let scaled:Vec<_>=samples[cursor..cursor+count].iter().map(|sample|sample*volume).collect();let written=worker_ring.push_slice(&scaled);cursor+=written;worker_supplied.store(cursor-begin,Ordering::Release); }
                thread::sleep(Duration::from_millis(5));
            }
            while !worker_stop.load(Ordering::Acquire) && worker_ring.len_samples()>0 { thread::sleep(Duration::from_millis(5)); }
            // デバイスへ渡った最後のバッファを停止で切り捨てない。
            for _ in 0..50 { if worker_stop.load(Ordering::Acquire){return;}thread::sleep(Duration::from_millis(5)); }
            worker_ended.store(true,Ordering::Release);
        }).map_err(|e|e.to_string())?;
        self.active=Some(Active {stop,ended,supplied,ring,worker:Some(worker),render:Some(render),start:begin});
        self.error=None;
        Ok(self.snapshot())
    }
    pub fn pause(&mut self) -> PlaybackSnapshot {
        if let Some(active)=self.active.take() {
            if let Some(loaded)=self.loaded.as_mut() { loaded.position=active.position().min(loaded.end); }
            drop(active);
        }
        self.snapshot()
    }
    pub fn stop(&mut self) -> PlaybackSnapshot {
        self.active=None;
        if let Some(loaded)=self.loaded.as_mut(){loaded.position=0;}
        self.error=None;
        self.snapshot()
    }
    pub fn seek(&mut self, seconds: f64) -> Result<PlaybackSnapshot> {
        let loaded=self.loaded.as_ref().ok_or("試聴する音声がありません")?;
        let target=sample_position(seconds,loaded.end as f64/SAMPLES_PER_SECOND)?;
        let playing=self.active.is_some();self.pause();
        if let Some(loaded)=self.loaded.as_mut(){loaded.position=target;}
        if playing {self.resume()} else {Ok(self.snapshot())}
    }
    pub fn set_volume(&mut self, volume: f64) -> Result<PlaybackSnapshot> {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {return Err("再生音量は0から1で指定してください".into());}
        if let Some(loaded)=&self.loaded {loaded.gain.store((volume as f32).to_bits(),Ordering::Relaxed);}
        Ok(self.snapshot())
    }
    pub fn snapshot(&mut self) -> PlaybackSnapshot {
        let finished=self.active.as_ref().is_some_and(|active|active.ended.load(Ordering::Acquire));
        let render_error=self.active.as_ref().and_then(|active|active.render.as_ref()).and_then(|render|render.status().last_error);
        if finished || render_error.is_some() {
            if let (Some(loaded),Some(active))=(self.loaded.as_mut(),self.active.as_ref()) {loaded.position=if finished {loaded.end}else{active.position().min(loaded.end)};}
            self.active=None;self.error=render_error;
        }
        match &self.loaded {
            Some(loaded)=>PlaybackSnapshot {playing:self.active.is_some(),position:self.active.as_ref().map_or(loaded.position,Active::position).min(loaded.end) as f64/SAMPLES_PER_SECOND,
                duration:loaded.samples.len() as f64/SAMPLES_PER_SECOND,file:loaded.file.clone(),error:self.error.clone()},
            None=>PlaybackSnapshot {playing:false,position:0.0,duration:0.0,file:String::new(),error:self.error.clone()},
        }
    }
    pub fn close(&mut self) {self.active=None;self.loaded=None;self.error=None;}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_endpoint_preserves_direction_and_never_falls_back() {
        for direction in [DeviceDirection::Capture,DeviceDirection::Render] {
            let resolved=resolve_selected_with("",direction,|selector,actual|{
                assert_eq!(selector,DEFAULT_CONSOLE_ENDPOINT);assert_eq!(actual,direction);Ok("current-device".into())
            },|id,actual|{assert_eq!(id,"current-device");assert_eq!(actual,direction);Ok(())}).unwrap();
            assert_eq!(resolved,"current-device");
            let explicit=resolve_selected_with("chosen-device",direction,|selector,actual|{
                assert_eq!(selector,"chosen-device");assert_eq!(actual,direction);Ok(selector.into())
            },|id,_|{assert_eq!(id,"chosen-device");Err("missing".into())});
            assert_eq!(explicit.unwrap_err(),"missing");
            let missing_default=resolve_selected_with("",direction,|_,_|Err("no default".into()),|_,_|panic!("unexpected validation"));
            assert_eq!(missing_default.unwrap_err(),"no default");
        }
    }
    #[test]
    fn malformed_cue_does_not_replace_loaded_main_audio() {
        let mut player=NativePlayback::default();
        assert!(player.play_samples(vec![f32::NAN,0.0],"",0.2).is_err());
        assert!(player.play_samples(vec![0.0,0.0],"",2.0).is_err());
        assert!(player.loaded.is_none());
    }
    #[test]
    fn seek_bounds_and_stereo_alignment() {
        assert_eq!(sample_position(0.25,1.0).unwrap(),24000);
        assert!(sample_position(f64::NAN,1.0).is_err());assert!(sample_position(-0.1,1.0).is_err());assert!(sample_position(1.1,1.0).is_err());
    }
    #[test]
    fn paused_seek_and_close_need_no_device() {
        let mut player=NativePlayback::default();
        player.loaded=Some(Loaded {samples:Arc::new(vec![0.0;96000]),gain:Arc::new(AtomicU32::new(1.0f32.to_bits())),file:"test.wav".into(),endpoint:String::new(),position:0,end:96000});
        assert_eq!(player.seek(0.5).unwrap().position,0.5);assert!(!player.snapshot().playing);
        assert_eq!(player.stop().position,0.0);player.close();assert_eq!(player.snapshot().duration,0.0);
    }
}
