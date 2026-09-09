use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type NodeId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecordingFormat {
    Wav,
    Flac,
    Mp3,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMethod {
    #[default]
    Auto,
    WasapiSharedEvent,
    WasapiSharedPolling,
    WasapiExclusiveEvent,
    WasapiExclusivePolling,
    WinMmWaveIn,
}
