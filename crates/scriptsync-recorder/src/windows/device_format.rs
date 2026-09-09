use crate::{
    audio::{
        format::{INTERNAL_CHANNELS, INTERNAL_SAMPLE_RATE},
        resampler::resample_interleaved_linear,
    },
    error::{BackendError, Result},
};
use wasapi::{SampleType, WaveFormat};

/// Converts capture packets to the internal format while retaining the sample
/// position between packets.  A linear interpolation sample at a packet end is
/// deliberately held until the following packet supplies its right neighbour.
pub(crate) struct StreamingInputConverter {
    format: DeviceFormat,
    input_frames: u64,
    next_output_frame: u64,
    last_input_frame: Vec<f32>,
    zero_input_frame: Vec<f32>,
    gap_before_next_packet: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DeviceFormat {
    sample_type: SampleType,
    sample_rate: u32,
    channels: u16,
    bytes_per_sample: usize,
    block_align: usize,
    valid_bits: u16,
}

impl DeviceFormat {
    pub(crate) fn from_wave_format(format: &WaveFormat) -> Result<Self> {
        let channels = format.get_nchannels();
        let block_align = format.get_blockalign() as usize;
        if channels == 0 || block_align == 0 || block_align % channels as usize != 0 {
            return Err(BackendError::Audio(format!(
                "WASAPI MixFormatのBlockAlignが不正です: {format:?}"
            )));
        }
        let bytes_per_sample = block_align / channels as usize;
        let store_bits = format.get_bitspersample();
        if store_bits as usize > bytes_per_sample * 8 {
            return Err(BackendError::Audio(format!(
                "WASAPI MixFormatのサンプル幅が不正です: {format:?}"
            )));
        }
        let sample_type = format
            .get_subformat()
            .map_err(|error| BackendError::Audio(format!("WASAPI MixFormat: {error}")))?;
        match (sample_type, bytes_per_sample) {
            (SampleType::Float, 4 | 8) | (SampleType::Int, 1..=4) => {}
            _ => {
                return Err(BackendError::Audio(format!(
                    "未対応のWASAPI MixFormatです: {format:?}"
                )))
            }
        }
        Ok(Self {
            sample_type,
            sample_rate: format.get_samplespersec(),
            channels,
            bytes_per_sample,
            block_align,
            valid_bits: format.get_validbitspersample().max(1),
        })
    }

    pub(crate) fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub(crate) fn streaming_input_converter(&self) -> StreamingInputConverter {
        StreamingInputConverter {
            format: *self,
            input_frames: 0,
            next_output_frame: 0,
            last_input_frame: Vec::new(),
            zero_input_frame: vec![0.0; self.channels as usize],
            gap_before_next_packet: false,
        }
    }

    pub(crate) fn bytes_for_frames(&self, frames: usize) -> Result<usize> {
        frames
            .checked_mul(self.block_align)
            .ok_or_else(|| BackendError::Audio("WASAPIバッファ長が上限を超えました".into()))
    }

    fn decode_packet(&self, bytes: &[u8], frames: usize, silent: bool) -> Result<Vec<f32>> {
        let required = self.bytes_for_frames(frames)?;
        if bytes.len() < required {
            return Err(BackendError::Audio(format!(
                "WASAPI Captureデータが不足しています: expected={required}, actual={}",
                bytes.len()
            )));
        }
        let mut decoded = vec![0.0f32; frames * self.channels as usize];
        if !silent {
            for (destination, source) in decoded
                .iter_mut()
                .zip(bytes[..required].chunks_exact(self.bytes_per_sample))
            {
                *destination = self.decode_sample(source)?;
            }
        }
        Ok(decoded)
    }

    pub(crate) fn encode_from_internal(
        &self,
        internal: &[f32],
        frames: usize,
        destination: &mut [u8],
    ) -> Result<()> {
        let required = self.bytes_for_frames(frames)?;
        if destination.len() < required {
            return Err(BackendError::Audio(format!(
                "WASAPI Renderバッファが不足しています: expected={required}, actual={}",
                destination.len()
            )));
        }
        destination[..required].fill(0);
        let converted;
        let samples =
            if self.sample_rate == INTERNAL_SAMPLE_RATE && self.channels == INTERNAL_CHANNELS {
                internal
            } else {
                converted = resample_interleaved_linear(
                    internal,
                    INTERNAL_SAMPLE_RATE,
                    INTERNAL_CHANNELS,
                    self.sample_rate,
                    self.channels,
                    1.0,
                )
                .samples;
                &converted
            };
        let destination_samples = frames * self.channels as usize;
        for (source, target) in samples
            .iter()
            .take(destination_samples)
            .zip(destination[..required].chunks_exact_mut(self.bytes_per_sample))
        {
            self.encode_sample(*source, target)?;
        }
        Ok(())
    }

    fn decode_sample(&self, bytes: &[u8]) -> Result<f32> {
        match (self.sample_type, self.bytes_per_sample) {
            (SampleType::Float, 4) => Ok(f32::from_le_bytes(bytes.try_into().unwrap())),
            (SampleType::Float, 8) => Ok(f64::from_le_bytes(bytes.try_into().unwrap()) as f32),
            (SampleType::Int, 1) => Ok((bytes[0] as f32 - 128.0) / 128.0),
            (SampleType::Int, 2) => {
                Ok(i16::from_le_bytes(bytes.try_into().unwrap()) as f32 / 32_768.0)
            }
            (SampleType::Int, 3) => {
                let raw = (bytes[0] as i32) | ((bytes[1] as i32) << 8) | ((bytes[2] as i32) << 16);
                let signed = (raw << 8) >> 8;
                Ok(signed as f32 / 8_388_608.0)
            }
            (SampleType::Int, 4) => {
                Ok(i32::from_le_bytes(bytes.try_into().unwrap()) as f32 / 2_147_483_648.0)
            }
            _ => Err(BackendError::Audio("未対応のWASAPIサンプル形式です".into())),
        }
    }

    fn encode_sample(&self, sample: f32, bytes: &mut [u8]) -> Result<()> {
        let sample = sample.clamp(-1.0, 1.0);
        match (self.sample_type, self.bytes_per_sample) {
            (SampleType::Float, 4) => bytes.copy_from_slice(&sample.to_le_bytes()),
            (SampleType::Float, 8) => bytes.copy_from_slice(&(sample as f64).to_le_bytes()),
            (SampleType::Int, 1) => {
                bytes[0] = (sample * 128.0 + 128.0).round().clamp(0.0, 255.0) as u8;
            }
            (SampleType::Int, width @ 2..=4) => {
                let store_bits = (width * 8) as u16;
                let scale = (1_i64 << (store_bits - 1)) as f64;
                let maximum = scale - 1.0;
                let mut value = (sample as f64 * scale).round().clamp(-scale, maximum) as i64;
                let unused_bits = store_bits.saturating_sub(self.valid_bits.min(store_bits));
                if unused_bits > 0 {
                    value = (value >> unused_bits) << unused_bits;
                }
                let encoded = value.to_le_bytes();
                bytes.copy_from_slice(&encoded[..width]);
            }
            _ => return Err(BackendError::Audio("未対応のWASAPIサンプル形式です".into())),
        }
        Ok(())
    }
}

impl StreamingInputConverter {
    /// Advances the input timeline over a WASAPI discontinuity.  The caller
    /// must insert this many internal silent frames into the output.  Counting
    /// from the cumulative timeline avoids packet-local rounding drift.
    pub(crate) fn skip_input_frames(&mut self, frames: usize) -> usize {
        if frames == 0 {
            // WASAPI can report a discontinuity without a measurable position
            // gap. Do not interpolate that packet from the old endpoint.
            self.gap_before_next_packet = true;
            return 0;
        }
        let timeline_end = self.input_frames.saturating_add(frames as u64);
        let expected_output = self.output_frames_for_input(timeline_end);
        let silence_frames = expected_output.saturating_sub(self.next_output_frame);
        self.input_frames = timeline_end;
        self.next_output_frame = expected_output;
        self.gap_before_next_packet = true;
        silence_frames as usize
    }

    /// Decodes one WASAPI packet and converts it to 48 kHz stereo.
    pub(crate) fn decode_to_internal(
        &mut self,
        bytes: &[u8],
        frames: usize,
        silent: bool,
    ) -> Result<Vec<f32>> {
        let decoded = self.format.decode_packet(bytes, frames, silent)?;
        Ok(self.convert_decoded(&decoded))
    }

    /// Converts an already decoded interleaved device packet.  This is kept
    /// separate so the capture loop can keep its WASAPI buffer ownership.
    pub(crate) fn convert_decoded(&mut self, input: &[f32]) -> Vec<f32> {
        let channels = self.format.channels as usize;
        let frames = input.len() / channels;
        if frames == 0 {
            return Vec::new();
        }

        if self.format.sample_rate == INTERNAL_SAMPLE_RATE {
            let mut output = Vec::with_capacity(frames * INTERNAL_CHANNELS as usize);
            for frame in input.chunks_exact(channels) {
                self.push_converted_frame(&mut output, frame, frame, 0.0);
            }
            self.input_frames += frames as u64;
            self.next_output_frame += frames as u64;
            self.last_input_frame = input[(frames - 1) * channels..frames * channels].to_vec();
            self.gap_before_next_packet = false;
            return output;
        }

        let packet_start = self.input_frames;
        let packet_end = packet_start + frames as u64;
        let mut output = Vec::new();
        while self.source_index(self.next_output_frame) + 1 < packet_end {
            let (index, fraction) = self.source_position(self.next_output_frame);
            let first = self.frame_at(input, packet_start, index);
            let second = self.frame_at(input, packet_start, index + 1);
            self.push_converted_frame(&mut output, first, second, fraction);
            self.next_output_frame += 1;
        }
        self.input_frames = packet_end;
        self.last_input_frame = input[(frames - 1) * channels..frames * channels].to_vec();
        self.gap_before_next_packet = false;
        output
    }

    /// Emits the single deferred interpolation region when capture ends.  The
    /// final device frame is extended only here, never at an arbitrary packet
    /// boundary.
    pub(crate) fn finish_to_internal(&mut self) -> Vec<f32> {
        if self.input_frames == 0 || self.format.sample_rate == INTERNAL_SAMPLE_RATE {
            return Vec::new();
        }
        let expected = self.output_frames_for_input(self.input_frames);
        let mut output = Vec::new();
        while self.next_output_frame < expected {
            let (index, fraction) = self.source_position(self.next_output_frame);
            let first = &self.last_input_frame;
            let second = if index + 1 >= self.input_frames {
                &self.last_input_frame
            } else {
                // The only un-emitted source position before the end is the
                // final input frame, so both sides are the saved final frame.
                &self.last_input_frame
            };
            self.push_converted_frame(&mut output, first, second, fraction);
            self.next_output_frame += 1;
        }
        output
    }

    fn source_position(&self, output_frame: u64) -> (u64, f32) {
        let numerator = output_frame as u128 * self.format.sample_rate as u128;
        let denominator = INTERNAL_SAMPLE_RATE as u128;
        let index = (numerator / denominator) as u64;
        let fraction = (numerator % denominator) as f32 / denominator as f32;
        (index, fraction)
    }

    fn source_index(&self, output_frame: u64) -> u64 {
        self.source_position(output_frame).0
    }

    fn output_frames_for_input(&self, input_frames: u64) -> u64 {
        ((input_frames as u128 * INTERNAL_SAMPLE_RATE as u128
            + (self.format.sample_rate as u128 / 2))
            / self.format.sample_rate as u128) as u64
    }

    fn frame_at<'a>(&'a self, input: &'a [f32], packet_start: u64, index: u64) -> &'a [f32] {
        let channels = self.format.channels as usize;
        if index < packet_start {
            if self.gap_before_next_packet {
                return &self.zero_input_frame;
            }
            return &self.last_input_frame;
        }
        let offset = (index - packet_start) as usize * channels;
        &input[offset..offset + channels]
    }

    fn push_converted_frame(
        &self,
        output: &mut Vec<f32>,
        first: &[f32],
        second: &[f32],
        fraction: f32,
    ) {
        for output_channel in 0..INTERNAL_CHANNELS as usize {
            let input_channel = if self.format.channels == 1 {
                0
            } else {
                output_channel.min(self.format.channels as usize - 1)
            };
            let a = first[input_channel];
            let b = second[input_channel];
            output.push(a + (b - a) * fraction);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float_format(sample_rate: u32, channels: u16) -> DeviceFormat {
        let wave = WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            sample_rate as usize,
            channels as usize,
            None,
        );
        DeviceFormat::from_wave_format(&wave).unwrap()
    }

    fn sine_packet(start: usize, frames: usize, sample_rate: u32, channels: u16) -> Vec<f32> {
        let mut packet = Vec::with_capacity(frames * channels as usize);
        for frame in start..start + frames {
            let phase = frame as f32 * std::f32::consts::TAU * 440.0 / sample_rate as f32;
            for channel in 0..channels {
                packet.push((phase + channel as f32 * 0.25).sin() * 0.7);
            }
        }
        packet
    }

    fn random_packet_sizes(total: usize) -> Vec<usize> {
        let mut state = 0x9e37_79b9u32;
        let mut remaining = total;
        let mut sizes = Vec::new();
        while remaining > 0 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let frames = (state as usize % 977 + 1).min(remaining);
            sizes.push(frames);
            remaining -= frames;
        }
        sizes
    }

    #[test]
    fn float32_stereo_round_trip_keeps_internal_samples() {
        let wave = WaveFormat::new(32, 32, &SampleType::Float, 48_000, 2, None);
        let format = DeviceFormat::from_wave_format(&wave).unwrap();
        let source = [-0.75, 0.5, 0.25, -0.125];
        let mut bytes = vec![0; format.bytes_for_frames(2).unwrap()];
        format.encode_from_internal(&source, 2, &mut bytes).unwrap();
        let decoded = format
            .streaming_input_converter()
            .decode_to_internal(&bytes, 2, false)
            .unwrap();
        assert_eq!(decoded, source);
    }

    #[test]
    fn integer_mono_device_is_converted_to_internal_stereo() {
        let wave = WaveFormat::new(16, 16, &SampleType::Int, 24_000, 1, None);
        let format = DeviceFormat::from_wave_format(&wave).unwrap();
        let bytes = [0x00, 0x40, 0x00, 0xC0];
        let mut converter = format.streaming_input_converter();
        let mut decoded = converter.decode_to_internal(&bytes, 2, false).unwrap();
        decoded.extend(converter.finish_to_internal());
        assert_eq!(decoded.len(), 8);
        assert!((decoded[0] - 0.5).abs() < 0.0001);
        assert_eq!(decoded[0], decoded[1]);
        assert!((decoded[6] + 0.5).abs() < 0.0001);
        assert_eq!(decoded[6], decoded[7]);
    }

    #[test]
    fn streaming_converter_keeps_sine_wave_continuous_across_random_packets() {
        for (rate, channels) in [(44_100, 1), (44_100, 2), (48_000, 1), (48_000, 2)] {
            let format = float_format(rate, channels);
            let total_frames = 22_157;
            let source = sine_packet(0, total_frames, rate, channels);

            let mut whole_converter = format.streaming_input_converter();
            let mut expected = whole_converter.convert_decoded(&source);
            expected.extend(whole_converter.finish_to_internal());

            let mut packet_converter = format.streaming_input_converter();
            let mut actual = Vec::new();
            let mut start = 0;
            for frames in random_packet_sizes(total_frames) {
                actual.extend(packet_converter.convert_decoded(
                    &source[start * channels as usize..(start + frames) * channels as usize],
                ));
                start += frames;
            }
            actual.extend(packet_converter.finish_to_internal());

            assert_eq!(
                actual.len(),
                expected.len(),
                "{rate} Hz, {channels} channels"
            );
            for (actual, expected) in actual.iter().zip(&expected) {
                assert!(
                    (actual - expected).abs() < 0.000_001,
                    "{rate} Hz, {channels} channels"
                );
            }

            let largest_step = actual
                .chunks_exact(INTERNAL_CHANNELS as usize)
                .map(|frame| frame[0])
                .collect::<Vec<_>>()
                .windows(2)
                .map(|frames| (frames[1] - frames[0]).abs())
                .fold(0.0f32, f32::max);
            assert!(largest_step < 0.05, "packet boundary jump: {largest_step}");
        }
    }

    #[test]
    fn streaming_converter_uses_cumulative_frame_count_for_long_capture() {
        let format = float_format(44_100, 2);
        let total_frames = 2_646_123; // 60 seconds plus a non-integral tail.
        let mut converter = format.streaming_input_converter();
        let mut produced = 0usize;
        let mut start = 0;
        for frames in random_packet_sizes(total_frames) {
            let packet = sine_packet(start, frames, 44_100, 2);
            produced += converter.convert_decoded(&packet).len() / INTERNAL_CHANNELS as usize;
            start += frames;
        }
        produced += converter.finish_to_internal().len() / INTERNAL_CHANNELS as usize;

        let expected =
            ((total_frames as u128 * INTERNAL_SAMPLE_RATE as u128 + 22_050) / 44_100) as usize;
        assert_eq!(produced, expected);
    }

    #[test]
    fn skipped_device_frames_advance_the_resampler_timeline() {
        let format = float_format(44_100, 1);
        let mut converter = format.streaming_input_converter();
        let first = sine_packet(0, 1_001, 44_100, 1);
        let second = sine_packet(1_138, 1_003, 44_100, 1);
        let mut produced = converter.convert_decoded(&first).len() / INTERNAL_CHANNELS as usize;
        let silence = converter.skip_input_frames(137);
        produced += silence;
        produced += converter.convert_decoded(&second).len() / INTERNAL_CHANNELS as usize;
        produced += converter.finish_to_internal().len() / INTERNAL_CHANNELS as usize;

        let expected = ((2_141u128 * INTERNAL_SAMPLE_RATE as u128 + 22_050) / 44_100) as usize;
        assert_eq!(produced, expected);
        assert!(silence > 0);
    }
}
