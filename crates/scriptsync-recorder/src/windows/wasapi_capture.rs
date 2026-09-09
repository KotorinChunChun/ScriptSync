use crate::{
    audio::{buffer::AudioRingBuffer, clock::CaptureClock, graph::CaptureMethod},
    error::{BackendError, Result},
    windows::device_format::DeviceFormat,
    windows::worker_status::{DeviceWorkerSnapshot, DeviceWorkerStatus},
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::audio::format::{INTERNAL_CHANNELS, INTERNAL_SAMPLE_RATE};

/// WASAPI が通知した欠落と、キャプチャ経路の設定値です。
///
/// `dropped_samples` とは別に保持する。前者はアプリ内リングへ入らな
/// かったサンプル数であり、こちらはデバイスが通知した欠落を表す。
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WasapiCaptureDiagnostics {
    pub data_discontinuity_count: u64,
    pub missing_device_frames: u64,
    pub compensated_internal_frames: u64,
    pub conversion_error_count: u64,
    pub last_conversion_error: Option<String>,
    pub unrecoverable_discontinuity_count: u64,
    pub estimated_missing_frame_count: u64,
    pub silent_packet_count: u64,
    pub timestamp_error_count: u64,
    pub last_discontinuity_at_ms: Option<u64>,
    pub mix_sample_rate: Option<u32>,
    pub mix_channels: Option<u16>,
    pub mix_sample_format: Option<String>,
    pub buffer_frames: Option<u32>,
    pub default_period_hns: Option<i64>,
    pub minimum_period_hns: Option<i64>,
}

#[derive(Clone, Copy)]
struct PreviousPacket {
    index: u64,
    frames: usize,
    timestamp_100ns: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PacketObservation {
    discontinuity: bool,
    missing_device_frames: u64,
    estimated_missing_frames: bool,
}

fn observe_packet(
    previous: Option<PreviousPacket>,
    index: u64,
    timestamp_100ns: u64,
    data_discontinuity: bool,
    timestamp_error: bool,
    sample_rate: u32,
) -> PacketObservation {
    let position_gap = previous.and_then(|previous| {
        index.checked_sub(previous.index.saturating_add(previous.frames as u64))
    });
    let position_gap = position_gap.filter(|frames| *frames > 0);
    let timestamp_gap = if data_discontinuity && !timestamp_error && position_gap.is_none() {
        previous.and_then(|previous| {
            if previous.timestamp_100ns == 0 || timestamp_100ns == 0 {
                return None;
            }
            let expected =
                previous
                    .timestamp_100ns
                    .saturating_add(crate::audio::clock::frames_to_100ns(
                        previous.frames,
                        sample_rate,
                    ));
            timestamp_100ns.checked_sub(expected).map(|delta| {
                ((u128::from(delta) * u128::from(sample_rate))
                    / u128::from(crate::audio::clock::UNITS_100NS_PER_SEC)) as u64
            })
        })
    } else {
        None
    };
    PacketObservation {
        discontinuity: data_discontinuity || position_gap.is_some(),
        missing_device_frames: position_gap.or(timestamp_gap).unwrap_or(0),
        estimated_missing_frames: position_gap.is_none() && timestamp_gap.is_some(),
    }
}

fn record_packet_observation(
    report: &mut WasapiCaptureDiagnostics,
    observation: PacketObservation,
    silent: bool,
    timestamp_error: bool,
    elapsed_ms: u64,
) {
    if observation.discontinuity {
        report.data_discontinuity_count = report.data_discontinuity_count.saturating_add(1);
        report.missing_device_frames = report
            .missing_device_frames
            .saturating_add(observation.missing_device_frames);
        if observation.estimated_missing_frames {
            report.estimated_missing_frame_count = report
                .estimated_missing_frame_count
                .saturating_add(observation.missing_device_frames);
        }
        report.last_discontinuity_at_ms = Some(elapsed_ms);
    }
    if silent {
        report.silent_packet_count = report.silent_packet_count.saturating_add(1);
    }
    if timestamp_error {
        report.timestamp_error_count = report.timestamp_error_count.saturating_add(1);
    }
}

#[derive(Default)]
struct PacketContinuityRepair {
    pending_tail: Vec<f32>,
    fade_in_frames_remaining: usize,
}

impl PacketContinuityRepair {
    const FADE_FRAMES: usize = (INTERNAL_SAMPLE_RATE as usize * 5) / 1_000;

    fn push(
        &mut self,
        packet: &[f32],
        discontinuity: bool,
        missing_frames: usize,
        mut emit: impl FnMut(&[f32]),
    ) {
        if discontinuity {
            // Keep the final 5 ms out of the ring until the following packet
            // is known. This permits a real fade-out before the inserted
            // silence instead of connecting unrelated waveform endpoints.
            let mut tail = std::mem::take(&mut self.pending_tail);
            fade_out(&mut tail);
            emit(&tail);
            if missing_frames > 0 {
                let silence = vec![0.0; missing_frames * INTERNAL_CHANNELS as usize];
                emit(&silence);
            }
            self.fade_in_frames_remaining = Self::FADE_FRAMES;
        }

        let mut packet = packet.to_vec();
        self.fade_in(&mut packet);
        let tail_samples = Self::FADE_FRAMES * INTERNAL_CHANNELS as usize;
        if discontinuity {
            self.pending_tail = packet;
        } else {
            self.pending_tail.extend_from_slice(&packet);
        }
        if self.pending_tail.len() > tail_samples {
            let split = self.pending_tail.len() - tail_samples;
            emit(&self.pending_tail[..split]);
            self.pending_tail.drain(..split);
        }
    }

    fn fade_in(&mut self, samples: &mut [f32]) {
        let frames = samples.len() / INTERNAL_CHANNELS as usize;
        let fade_frames = frames.min(self.fade_in_frames_remaining);
        let already_faded = Self::FADE_FRAMES.saturating_sub(self.fade_in_frames_remaining);
        for (frame, values) in samples
            .chunks_exact_mut(INTERNAL_CHANNELS as usize)
            .take(fade_frames)
            .enumerate()
        {
            let gain = (already_faded + frame + 1) as f32 / Self::FADE_FRAMES as f32;
            for value in values {
                *value *= gain;
            }
        }
        self.fade_in_frames_remaining = self.fade_in_frames_remaining.saturating_sub(fade_frames);
    }

    fn finish(self, mut emit: impl FnMut(&[f32])) {
        if !self.pending_tail.is_empty() {
            emit(&self.pending_tail);
        }
    }
}

fn fade_out(samples: &mut [f32]) {
    let frames = samples.len() / INTERNAL_CHANNELS as usize;
    for (frame, values) in samples
        .chunks_exact_mut(INTERNAL_CHANNELS as usize)
        .enumerate()
    {
        let gain = 1.0 - (frame + 1) as f32 / frames.max(1) as f32;
        for value in values {
            *value *= gain;
        }
    }
}

pub struct CaptureHandle {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    status: DeviceWorkerStatus,
    dropped_samples: Arc<AtomicU64>,
    clock: Arc<CaptureClock>,
    diagnostics: Arc<parking_lot::Mutex<WasapiCaptureDiagnostics>>,
}

pub struct MeterCaptureHandle {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    status: DeviceWorkerStatus,
    meter: Arc<crate::audio::meter::AtomicMeter>,
}

impl MeterCaptureHandle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }

    pub fn status(&self) -> DeviceWorkerSnapshot {
        self.status.snapshot()
    }

    pub fn meter(&self) -> crate::audio::meter::MeterFrame {
        self.meter.load()
    }
}

impl Drop for MeterCaptureHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
impl CaptureHandle {
    pub fn stop_and_dropped(mut self) -> u64 {
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
        self.dropped_samples.load(Ordering::Acquire)
    }
    pub fn stop_and_diagnostics(mut self) -> (u64, WasapiCaptureDiagnostics, DeviceWorkerSnapshot) {
        let worker_status = self.status.snapshot();
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
        (
            self.dropped_samples.load(Ordering::Acquire),
            self.diagnostics.lock().clone(),
            worker_status,
        )
    }
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
    pub fn status(&self) -> DeviceWorkerSnapshot {
        self.status.snapshot()
    }
    pub fn wait_until_running(&self, timeout: Duration) -> Result<DeviceWorkerSnapshot> {
        self.status.wait_until_running(timeout)
    }
    pub fn dropped_samples(&self) -> u64 {
        self.dropped_samples.load(Ordering::Acquire)
    }
    pub fn clock(&self) -> Arc<CaptureClock> {
        self.clock.clone()
    }
    pub fn diagnostics(&self) -> WasapiCaptureDiagnostics {
        self.diagnostics.lock().clone()
    }
}
impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

pub fn capture_method_name(method: CaptureMethod) -> &'static str {
    match method {
        CaptureMethod::Auto => "auto",
        CaptureMethod::WasapiSharedEvent => "wasapi_shared_event",
        CaptureMethod::WasapiSharedPolling => "wasapi_shared_polling",
        CaptureMethod::WasapiExclusiveEvent => "wasapi_exclusive_event",
        CaptureMethod::WasapiExclusivePolling => "wasapi_exclusive_polling",
        CaptureMethod::WinMmWaveIn => "win_mm_wave_in",
    }
}

#[cfg(windows)]
fn capture_candidates(
    requested: CaptureMethod,
    auto_priority: &[CaptureMethod],
) -> Vec<CaptureMethod> {
    use CaptureMethod::*;
    match requested {
        Auto => {
            let mut candidates: Vec<_> = auto_priority
                .iter()
                .copied()
                .filter(|method| *method != Auto)
                .collect();
            for fallback in [WasapiSharedEvent, WasapiSharedPolling, WinMmWaveIn] {
                if !candidates.contains(&fallback) {
                    candidates.push(fallback);
                }
            }
            candidates
        }
        WasapiSharedEvent => vec![WasapiSharedEvent],
        WasapiSharedPolling => vec![WasapiSharedPolling],
        WasapiExclusiveEvent => vec![WasapiExclusiveEvent],
        WasapiExclusivePolling => vec![WasapiExclusivePolling],
        WinMmWaveIn => vec![WinMmWaveIn],
    }
}

#[cfg(windows)]
fn run_wasapi_capture(
    endpoint_id: &str,
    method: CaptureMethod,
    stop: &Arc<AtomicBool>,
    status: &DeviceWorkerStatus,
    clock: &CaptureClock,
    diagnostics: &Arc<parking_lot::Mutex<WasapiCaptureDiagnostics>>,
    mut on_samples: impl FnMut(&[f32]),
) -> std::result::Result<(), String> {
    use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

    let en = DeviceEnumerator::new().map_err(|e| format!("DeviceEnumerator::new: {e}"))?;
    let device = en
        .get_device(endpoint_id)
        .map_err(|e| format!("get_device: {e}"))?;
    let mut client = device
        .get_iaudioclient()
        .map_err(|e| format!("get_iaudioclient: {e}"))?;
    let mix_fmt = client
        .get_mixformat()
        .map_err(|e| format!("get_mixformat: {e}"))?;
    let exclusive = matches!(
        method,
        CaptureMethod::WasapiExclusiveEvent | CaptureMethod::WasapiExclusivePolling
    );
    let fmt = if matches!(
        method,
        CaptureMethod::WasapiSharedEvent | CaptureMethod::WasapiSharedPolling
    ) {
        // GetMixFormat is the format the Windows audio engine guarantees for
        // shared mode. DeviceFormat converts/resamples it into the internal
        // 48 kHz stereo representation after capture.
        mix_fmt.clone()
    } else if exclusive {
        // The shared-mode mix format is not necessarily accepted by the device
        // in exclusive mode. Query the driver with common native formats and
        // use the first format it explicitly accepts.
        let mut candidates = vec![mix_fmt.clone()];
        let mut sample_rates = vec![mix_fmt.get_samplespersec()];
        for sample_rate in [48_000, 44_100, 96_000] {
            if !sample_rates.contains(&sample_rate) {
                sample_rates.push(sample_rate);
            }
        }
        let mut channels = vec![mix_fmt.get_nchannels()];
        for channel_count in [2, 1] {
            if !channels.contains(&channel_count) {
                channels.push(channel_count);
            }
        }
        for sample_rate in sample_rates {
            for channel_count in &channels {
                let sample_rate = sample_rate as usize;
                let channel_count = *channel_count as usize;
                candidates.extend([
                    WaveFormat::new(32, 32, &SampleType::Float, sample_rate, channel_count, None),
                    WaveFormat::new(32, 24, &SampleType::Int, sample_rate, channel_count, None),
                    WaveFormat::new(24, 24, &SampleType::Int, sample_rate, channel_count, None),
                    WaveFormat::new(16, 16, &SampleType::Int, sample_rate, channel_count, None),
                ]);
            }
        }
        let mut errors = Vec::new();
        let mut supported = None;
        for candidate in candidates {
            match client.is_supported_exclusive_with_quirks(&candidate) {
                Ok(format) => {
                    supported = Some(format);
                    break;
                }
                Err(error) => errors.push(format!("{candidate:?}: {error}")),
            }
        }
        supported.ok_or_else(|| {
            format!(
                "exclusive format negotiation failed; mix={mix_fmt:?}; tried={}",
                errors.join(" | ")
            )
        })?
    } else {
        return Err("invalid WASAPI capture method".into());
    };
    let device_format =
        DeviceFormat::from_wave_format(&fmt).map_err(|e| format!("device format: {e}"))?;
    let (default_period, minimum_period) = client
        .get_device_period()
        .map_err(|e| format!("get_device_period: {e}"))?;
    let exclusive_period = if exclusive {
        Some(
            client
                .calculate_aligned_period_near(default_period, Some(128), &fmt)
                .map_err(|e| format!("calculate_aligned_period_near: {e}"))?,
        )
    } else {
        None
    };
    let mode = match method {
        CaptureMethod::WasapiSharedEvent => StreamMode::EventsShared {
            autoconvert: false,
            buffer_duration_hns: default_period,
        },
        CaptureMethod::WasapiSharedPolling => StreamMode::PollingShared {
            autoconvert: false,
            buffer_duration_hns: default_period,
        },
        CaptureMethod::WasapiExclusiveEvent => StreamMode::EventsExclusive {
            period_hns: exclusive_period.expect("exclusive period"),
        },
        CaptureMethod::WasapiExclusivePolling => StreamMode::PollingExclusive {
            buffer_duration_hns: 4 * exclusive_period.expect("exclusive period"),
            period_hns: exclusive_period.expect("exclusive period"),
        },
        CaptureMethod::Auto | CaptureMethod::WinMmWaveIn => {
            return Err("invalid WASAPI capture method".into())
        }
    };
    client
        .initialize_client(&fmt, &Direction::Capture, &mode)
        .map_err(|e| {
            format!(
                "initialize_client: {e}; endpoint={endpoint_id}; requested={fmt:?}; mix={mix_fmt:?}; default_period_hns={default_period}; minimum_period_hns={minimum_period}; mode={mode:?}"
            )
        })?;
    let event = if matches!(
        method,
        CaptureMethod::WasapiSharedEvent | CaptureMethod::WasapiExclusiveEvent
    ) {
        Some(
            client
                .set_get_eventhandle()
                .map_err(|e| format!("set_get_eventhandle: {e}"))?,
        )
    } else {
        None
    };
    let capture = client
        .get_audiocaptureclient()
        .map_err(|e| format!("get_audiocaptureclient: {e}"))?;
    let max_frames = client.get_buffer_size().unwrap_or(4096).max(1) as usize;
    {
        let mut report = diagnostics.lock();
        report.mix_sample_rate = Some(mix_fmt.get_samplespersec());
        report.mix_channels = Some(mix_fmt.get_nchannels());
        report.mix_sample_format = mix_fmt.get_subformat().ok().map(|kind| format!("{kind:?}"));
        report.buffer_frames = Some(max_frames as u32);
        report.default_period_hns = Some(default_period);
        report.minimum_period_hns = Some(minimum_period);
    }
    let mut bytes = vec![
        0u8;
        device_format
            .bytes_for_frames(max_frames)
            .map_err(|e| e.to_string())?
    ];
    client
        .start_stream()
        .map_err(|e| format!("start_stream: {e}"))?;
    status.running_with_method(capture_method_name(method));

    let poll_delay = Duration::from_micros((default_period.max(10_000) as u64) / 10);
    let capture_started = std::time::Instant::now();
    let mut previous_packet: Option<PreviousPacket> = None;
    let mut converter = device_format.streaming_input_converter();
    let mut repair = PacketContinuityRepair::default();
    while !stop.load(Ordering::Acquire) {
        if let Some(event) = &event {
            if event.wait_for_event(100).is_err() {
                continue;
            }
        } else {
            thread::sleep(poll_delay.min(Duration::from_millis(20)));
        }
        loop {
            let frames = match capture.get_next_packet_size().map_err(|e| e.to_string())? {
                Some(n) if n > 0 => n as usize,
                _ => break,
            };
            if frames > max_frames {
                return Err("capture packet exceeded WASAPI buffer size".into());
            }
            let packet_bytes = device_format
                .bytes_for_frames(frames)
                .map_err(|e| e.to_string())?;
            let (read, info) = capture
                .read_from_device(&mut bytes[..packet_bytes])
                .map_err(|e| e.to_string())?;
            let (ticks, freq) = crate::audio::clock::qpc_counter();
            let (time, accuracy) = crate::audio::clock::resolve_capture_timestamp(
                info.timestamp,
                info.flags.timestamp_error,
                ticks,
                freq,
            );
            clock.observe(time, accuracy);
            let observation = observe_packet(
                previous_packet,
                info.index,
                info.timestamp,
                info.flags.data_discontinuity,
                info.flags.timestamp_error,
                device_format.sample_rate(),
            );
            let missing_device_frames = observation.missing_device_frames;
            let discontinuity = observation.discontinuity;
            if discontinuity || info.flags.silent || info.flags.timestamp_error {
                record_packet_observation(
                    &mut diagnostics.lock(),
                    observation,
                    info.flags.silent,
                    info.flags.timestamp_error,
                    capture_started
                        .elapsed()
                        .as_millis()
                        .min(u128::from(u64::MAX)) as u64,
                );
            }
            let max_recoverable_device_frames = u64::from(device_format.sample_rate()) * 60;
            if discontinuity && missing_device_frames > max_recoverable_device_frames {
                let message = format!(
                    "WASAPI不連続の欠落が上限を超えました: {missing_device_frames} frames (最大 {max_recoverable_device_frames} frames)"
                );
                let mut report = diagnostics.lock();
                report.unrecoverable_discontinuity_count =
                    report.unrecoverable_discontinuity_count.saturating_add(1);
                report.last_conversion_error = Some(message.clone());
                return Err(message);
            }
            let missing_internal_frames = if discontinuity {
                converter.skip_input_frames(missing_device_frames as usize)
            } else {
                0
            };
            let decoded =
                match converter.decode_to_internal(&bytes, read as usize, info.flags.silent) {
                    Ok(decoded) => decoded,
                    Err(error) => {
                        let message = error.to_string();
                        let mut report = diagnostics.lock();
                        report.conversion_error_count =
                            report.conversion_error_count.saturating_add(1);
                        report.last_conversion_error = Some(message.clone());
                        return Err(message);
                    }
                };
            if discontinuity {
                let mut report = diagnostics.lock();
                report.compensated_internal_frames = report
                    .compensated_internal_frames
                    .saturating_add(missing_internal_frames as u64);
            }
            repair.push(
                &decoded,
                discontinuity,
                missing_internal_frames,
                |samples| on_samples(samples),
            );
            previous_packet = Some(PreviousPacket {
                index: info.index,
                frames: read as usize,
                timestamp_100ns: if info.flags.timestamp_error {
                    0
                } else {
                    info.timestamp
                },
            });
        }
    }
    let tail = converter.finish_to_internal();
    if !tail.is_empty() {
        repair.push(&tail, false, 0, |samples| on_samples(samples));
    }
    repair.finish(|samples| on_samples(samples));
    let _ = client.stop_stream();
    Ok(())
}

#[cfg(windows)]
pub fn start_shared_meter(endpoint_id: impl Into<String>) -> Result<MeterCaptureHandle> {
    let endpoint_id = endpoint_id.into();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let status = DeviceWorkerStatus::capture("wasapi_shared_event");
    let worker_status = status.clone();
    let meter = Arc::new(crate::audio::meter::AtomicMeter::default());
    let worker_meter = meter.clone();
    let join = thread::Builder::new()
        .name("wasapi-shared-meter".into())
        .spawn(move || {
            let _ = wasapi::initialize_mta();
            while !worker_stop.load(Ordering::Acquire) {
                let mut failures = Vec::new();
                for method in [
                    CaptureMethod::WasapiSharedEvent,
                    CaptureMethod::WasapiSharedPolling,
                ] {
                    if worker_stop.load(Ordering::Acquire) {
                        break;
                    }
                    let meter_clock = CaptureClock::default();
                    let result = run_wasapi_capture(
                        &endpoint_id,
                        method,
                        &worker_stop,
                        &worker_status,
                        &meter_clock,
                        &Arc::new(parking_lot::Mutex::new(WasapiCaptureDiagnostics::default())),
                        |samples| worker_meter.store(crate::audio::meter::measure(samples)),
                    );
                    match result {
                        Ok(()) if worker_stop.load(Ordering::Acquire) => break,
                        Ok(()) => failures.push(format!(
                            "{} stopped unexpectedly",
                            capture_method_name(method)
                        )),
                        Err(error) => {
                            failures.push(format!("{}: {error}", capture_method_name(method)))
                        }
                    }
                }
                if worker_stop.load(Ordering::Acquire) {
                    break;
                }
                worker_status.reconnecting(failures.join(" | "));
                thread::sleep(Duration::from_millis(500));
            }
            worker_status.stopped();
            wasapi::deinitialize();
        })
        .map_err(BackendError::Io)?;
    Ok(MeterCaptureHandle {
        stop,
        join: Some(join),
        status,
        meter,
    })
}

#[cfg(windows)]
fn spawn_endpoint_capture(
    endpoint_id: String,
    loopback: bool,
    requested_method: CaptureMethod,
    ring: AudioRingBuffer,
    auto_priority: Vec<CaptureMethod>,
) -> Result<CaptureHandle> {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let status = DeviceWorkerStatus::capture(capture_method_name(requested_method));
    let worker_status = status.clone();
    let dropped_samples = Arc::new(AtomicU64::new(0));
    let worker_dropped_samples = dropped_samples.clone();
    let clock = Arc::new(CaptureClock::default());
    let worker_clock = clock.clone();
    let diagnostics = Arc::new(parking_lot::Mutex::new(WasapiCaptureDiagnostics::default()));
    let worker_diagnostics = diagnostics.clone();
    let join = thread::Builder::new()
        .name(if loopback {
            "wasapi-loopback-capture".into()
        } else {
            "microphone-capture".into()
        })
        .spawn(move || {
            let _ = wasapi::initialize_mta();
            while !worker_stop.load(Ordering::Acquire) {
                let mut failures = Vec::new();
                for method in capture_candidates(requested_method, &auto_priority) {
                    if worker_stop.load(Ordering::Acquire) {
                        break;
                    }
                    let result = if method == CaptureMethod::WinMmWaveIn {
                        crate::windows::winmm_capture::run_winmm_capture(
                            &endpoint_id,
                            &ring,
                            &worker_stop,
                            &worker_dropped_samples,
                            worker_clock.as_ref(),
                            || worker_status.running_with_method(capture_method_name(method)),
                        )
                        .map_err(|error| error.to_string())
                    } else {
                        run_wasapi_capture(
                            &endpoint_id,
                            method,
                            &worker_stop,
                            &worker_status,
                            worker_clock.as_ref(),
                            &worker_diagnostics,
                            |samples| {
                                let written = ring.push_slice(samples);
                                if written < samples.len() {
                                    worker_dropped_samples.fetch_add(
                                        (samples.len() - written) as u64,
                                        Ordering::Relaxed,
                                    );
                                }
                            },
                        )
                    };
                    match result {
                        Ok(()) if worker_stop.load(Ordering::Acquire) => break,
                        Ok(()) => failures.push(format!(
                            "{} stopped unexpectedly",
                            capture_method_name(method)
                        )),
                        Err(error) => {
                            failures.push(format!("{}: {error}", capture_method_name(method)))
                        }
                    }
                }
                if worker_stop.load(Ordering::Acquire) {
                    break;
                }
                worker_status.reconnecting(failures.join(" | "));
                thread::sleep(Duration::from_millis(500));
            }
            worker_status.stopped();
            wasapi::deinitialize();
        })
        .map_err(BackendError::Io)?;
    Ok(CaptureHandle {
        stop,
        join: Some(join),
        status,
        dropped_samples,
        clock,
        diagnostics,
    })
}

#[cfg(windows)]
pub fn start_microphone_capture_with_method(
    endpoint_id: impl Into<String>,
    method: CaptureMethod,
    ring: AudioRingBuffer,
) -> Result<CaptureHandle> {
    start_microphone_capture_with_auto_priority(endpoint_id, method, ring, Vec::new())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn auto_capture_candidates_always_keep_safe_fallbacks() {
        assert_eq!(
            capture_candidates(CaptureMethod::Auto, &[CaptureMethod::WasapiExclusiveEvent]),
            vec![
                CaptureMethod::WasapiExclusiveEvent,
                CaptureMethod::WasapiSharedEvent,
                CaptureMethod::WasapiSharedPolling,
                CaptureMethod::WinMmWaveIn,
            ]
        );
        assert_eq!(
            capture_candidates(CaptureMethod::Auto, &[CaptureMethod::WasapiSharedEvent]),
            vec![
                CaptureMethod::WasapiSharedEvent,
                CaptureMethod::WasapiSharedPolling,
                CaptureMethod::WinMmWaveIn,
            ]
        );
    }

    #[test]
    fn explicit_capture_method_does_not_fall_back() {
        assert_eq!(
            capture_candidates(
                CaptureMethod::WasapiExclusivePolling,
                &[CaptureMethod::WasapiSharedEvent],
            ),
            vec![CaptureMethod::WasapiExclusivePolling]
        );
    }
}

#[cfg(test)]
mod continuity_tests {
    use super::*;

    fn previous() -> PreviousPacket {
        PreviousPacket {
            index: 1_000,
            frames: 480,
            timestamp_100ns: 10_000_000,
        }
    }

    #[test]
    fn packet_observation_distinguishes_flags_and_position_gap() {
        let mut report = WasapiCaptureDiagnostics::default();
        let clean = observe_packet(Some(previous()), 1_480, 10_100_000, false, false, 48_000);
        record_packet_observation(&mut report, clean, false, false, 1);

        let position_gap =
            observe_packet(Some(previous()), 1_600, 10_100_000, false, false, 48_000);
        assert!(position_gap.discontinuity);
        assert_eq!(position_gap.missing_device_frames, 120);
        assert!(!position_gap.estimated_missing_frames);
        record_packet_observation(&mut report, position_gap, true, false, 20);

        let flagged_zero_gap =
            observe_packet(Some(previous()), 1_480, 10_100_000, true, false, 48_000);
        assert!(flagged_zero_gap.discontinuity);
        assert_eq!(flagged_zero_gap.missing_device_frames, 0);
        record_packet_observation(&mut report, flagged_zero_gap, false, true, 30);

        assert_eq!(report.data_discontinuity_count, 2);
        assert_eq!(report.missing_device_frames, 120);
        assert_eq!(report.silent_packet_count, 1);
        assert_eq!(report.timestamp_error_count, 1);
        assert_eq!(report.last_discontinuity_at_ms, Some(30));
    }

    #[test]
    fn zero_position_gap_uses_timestamp_only_when_valid() {
        let expected = previous().timestamp_100ns
            + crate::audio::clock::frames_to_100ns(previous().frames, 48_000);
        let observation = observe_packet(
            Some(previous()),
            1_480,
            expected + 2_500_000,
            true,
            false,
            48_000,
        );
        assert!(observation.discontinuity);
        assert_eq!(observation.missing_device_frames, 12_000);
        assert!(observation.estimated_missing_frames);

        let invalid_previous = PreviousPacket {
            timestamp_100ns: 0,
            ..previous()
        };
        let timestamp_error = observe_packet(
            Some(invalid_previous),
            1_480,
            expected + 2_500_000,
            true,
            true,
            48_000,
        );
        assert!(timestamp_error.discontinuity);
        assert_eq!(timestamp_error.missing_device_frames, 0);
        assert!(!timestamp_error.estimated_missing_frames);
    }

    #[test]
    fn discontinuity_inserts_silence_and_fades_packet_edges() {
        let mut repair = PacketContinuityRepair::default();
        let mut output = Vec::new();
        repair.push(&vec![1.0; 480 * 2], false, 0, |samples| {
            output.extend_from_slice(samples)
        });
        repair.push(&vec![1.0; 480 * 2], true, 12, |samples| {
            output.extend_from_slice(samples)
        });
        repair.finish(|samples| output.extend_from_slice(samples));

        // 480 frames before and after, plus the recovered 12-frame gap.
        assert_eq!(output.len(), (480 + 12 + 480) * 2);
        let gap_start = (480 - PacketContinuityRepair::FADE_FRAMES) * 2;
        let silence_start = 480 * 2;
        let silence_end = silence_start + 12 * 2;
        assert!(output[gap_start] < 1.0);
        assert!(output[silence_start..silence_end]
            .iter()
            .all(|sample| *sample == 0.0));
        assert!(output[silence_end] > 0.0 && output[silence_end] < 1.0);
        assert!(output.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn discontinuity_fade_continues_across_short_packets() {
        let mut repair = PacketContinuityRepair::default();
        let mut output = Vec::new();
        repair.push(&vec![1.0; 480 * 2], false, 0, |samples| {
            output.extend_from_slice(samples)
        });
        repair.push(&vec![1.0; 100 * 2], true, 0, |samples| {
            output.extend_from_slice(samples)
        });
        repair.push(&vec![1.0; 100 * 2], false, 0, |samples| {
            output.extend_from_slice(samples)
        });
        repair.push(&vec![1.0; 100 * 2], false, 0, |samples| {
            output.extend_from_slice(samples)
        });
        repair.finish(|samples| output.extend_from_slice(samples));

        let first_after_gap = 480 * 2;
        assert!((output[first_after_gap] - 1.0 / 240.0).abs() < 0.000_001);
        // 100 frames of the first packet have already been faded when the
        // second short packet begins, so the gain must continue at 101/240.
        assert!((output[first_after_gap + 100 * 2] - 101.0 / 240.0).abs() < 0.000_001);
    }
}

#[cfg(windows)]
pub fn start_microphone_capture_with_auto_priority(
    endpoint_id: impl Into<String>,
    method: CaptureMethod,
    ring: AudioRingBuffer,
    auto_priority: Vec<CaptureMethod>,
) -> Result<CaptureHandle> {
    spawn_endpoint_capture(endpoint_id.into(), false, method, ring, auto_priority)
}

#[cfg(windows)]
pub fn start_microphone_capture(
    endpoint_id: impl Into<String>,
    ring: AudioRingBuffer,
) -> Result<CaptureHandle> {
    start_microphone_capture_with_method(endpoint_id, CaptureMethod::Auto, ring)
}

#[cfg(windows)]
pub fn start_endpoint_loopback(
    endpoint_id: impl Into<String>,
    ring: AudioRingBuffer,
) -> Result<CaptureHandle> {
    spawn_endpoint_capture(
        endpoint_id.into(),
        true,
        CaptureMethod::WasapiSharedEvent,
        ring,
        Vec::new(),
    )
}

#[cfg(not(windows))]
pub fn start_shared_meter(_endpoint_id: impl Into<String>) -> Result<MeterCaptureHandle> {
    Err(BackendError::PlatformUnsupported(
        "WASAPI shared meter requires Windows".into(),
    ))
}

#[cfg(not(windows))]
pub fn start_microphone_capture_with_auto_priority(
    endpoint_id: impl Into<String>,
    method: CaptureMethod,
    ring: AudioRingBuffer,
    _auto_priority: Vec<CaptureMethod>,
) -> Result<CaptureHandle> {
    start_microphone_capture_with_method(endpoint_id, method, ring)
}

#[cfg(not(windows))]
pub fn start_microphone_capture(
    endpoint_id: impl Into<String>,
    ring: AudioRingBuffer,
) -> Result<CaptureHandle> {
    start_microphone_capture_with_method(endpoint_id, CaptureMethod::Auto, ring)
}

#[cfg(not(windows))]
pub fn start_endpoint_loopback(
    _endpoint_id: impl Into<String>,
    _ring: AudioRingBuffer,
) -> Result<CaptureHandle> {
    Err(BackendError::PlatformUnsupported(
        "WASAPI loopback requires Windows".into(),
    ))
}
