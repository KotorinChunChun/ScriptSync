use crate::{audio::format::AudioBlock, error::Result};

/// 録音writerが利用する最小の音声出力境界。
pub trait AudioSink: Send + Sync {
    fn write(&self, block: &AudioBlock) -> Result<()>;
    fn pause(&self) {}
}
