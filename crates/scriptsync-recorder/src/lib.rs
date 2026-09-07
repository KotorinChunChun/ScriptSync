pub mod audio {
    pub mod buffer;
    pub mod clock;
    pub mod engine;
    pub mod format;
    pub mod graph;
    pub mod meter;
    pub mod resampler;
}

pub mod error;

pub mod recording {
    pub mod codecs;
    pub mod sample_session;
    pub mod writer;
}

pub mod windows {
    pub mod default_device;
    pub mod device_format;
    pub mod devices;
    pub mod microphone_recording;
    pub mod wasapi_capture;
    pub mod wasapi_render;
    pub mod winmm_capture;
    pub mod worker_status;
}
