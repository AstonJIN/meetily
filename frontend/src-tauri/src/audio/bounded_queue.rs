//! Queue adapters used by the streaming-pipeline pilot.
//!
//! The audio callback must never await. `AudioQueueSender::try_send` keeps
//! that boundary explicit while allowing the existing unbounded path to stay
//! available behind the rollback switch.

use std::env;

use tokio::sync::mpsc;

pub const STREAMING_PIPELINE_ENV: &str = "MEETILY_STREAMING_PIPELINE_V1";
pub const AUDIO_INPUT_QUEUE_CAPACITY_ENV: &str = "MEETILY_AUDIO_INPUT_QUEUE_CAPACITY";
pub const TRANSCRIPTION_QUEUE_CAPACITY_ENV: &str = "MEETILY_TRANSCRIPTION_QUEUE_CAPACITY";
pub const RECORDING_QUEUE_CAPACITY_ENV: &str = "MEETILY_RECORDING_QUEUE_CAPACITY";
pub const AUDIO_MIX_WINDOW_MS_ENV: &str = "MEETILY_AUDIO_MIX_WINDOW_MS";
pub const DEFAULT_AUDIO_INPUT_QUEUE_CAPACITY: usize = 64;
pub const DEFAULT_TRANSCRIPTION_QUEUE_CAPACITY: usize = 8;
pub const DEFAULT_RECORDING_QUEUE_CAPACITY: usize = 256;
pub const DEFAULT_STREAMING_MIX_WINDOW_MS: u32 = 40;
pub const LEGACY_MIX_WINDOW_MS: u32 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamingPipelineConfig {
    pub enabled: bool,
    pub audio_input_capacity: usize,
    pub transcription_capacity: usize,
    pub recording_capacity: usize,
    pub mix_window_ms: u32,
}

impl Default for StreamingPipelineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            audio_input_capacity: DEFAULT_AUDIO_INPUT_QUEUE_CAPACITY,
            transcription_capacity: DEFAULT_TRANSCRIPTION_QUEUE_CAPACITY,
            recording_capacity: DEFAULT_RECORDING_QUEUE_CAPACITY,
            mix_window_ms: DEFAULT_STREAMING_MIX_WINDOW_MS,
        }
    }
}

impl StreamingPipelineConfig {
    pub fn from_env() -> Self {
        let enabled = env::var(STREAMING_PIPELINE_ENV)
            .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "on"))
            .unwrap_or(false);
        let audio_input_capacity = env::var(AUDIO_INPUT_QUEUE_CAPACITY_ENV)
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|capacity| *capacity > 0)
            .unwrap_or(DEFAULT_AUDIO_INPUT_QUEUE_CAPACITY);
        let transcription_capacity = env::var(TRANSCRIPTION_QUEUE_CAPACITY_ENV)
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|capacity| *capacity > 0)
            .unwrap_or(DEFAULT_TRANSCRIPTION_QUEUE_CAPACITY);
        let recording_capacity = env::var(RECORDING_QUEUE_CAPACITY_ENV)
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|capacity| *capacity > 0)
            .unwrap_or(DEFAULT_RECORDING_QUEUE_CAPACITY);
        let mix_window_ms = parse_mix_window_ms(env::var(AUDIO_MIX_WINDOW_MS_ENV).ok().as_deref());

        Self {
            enabled,
            audio_input_capacity,
            transcription_capacity,
            recording_capacity,
            mix_window_ms,
        }
    }

    pub fn effective_mix_window_ms(self) -> u32 {
        if self.enabled {
            self.mix_window_ms
        } else {
            LEGACY_MIX_WINDOW_MS
        }
    }
}

fn parse_mix_window_ms(value: Option<&str>) -> u32 {
    value
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| matches!(value, 20 | 40 | 50))
        .unwrap_or(DEFAULT_STREAMING_MIX_WINDOW_MS)
}

#[derive(Debug, PartialEq, Eq)]
pub enum AudioQueueSendError<T> {
    Full(T),
    Closed(T),
}

#[derive(Debug)]
pub enum AudioQueueSender<T> {
    Unbounded(mpsc::UnboundedSender<T>),
    Bounded(mpsc::Sender<T>),
}

impl<T> Clone for AudioQueueSender<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Unbounded(sender) => Self::Unbounded(sender.clone()),
            Self::Bounded(sender) => Self::Bounded(sender.clone()),
        }
    }
}

impl<T> AudioQueueSender<T> {
    pub fn try_send(&self, item: T) -> Result<(), AudioQueueSendError<T>> {
        match self {
            Self::Unbounded(sender) => sender
                .send(item)
                .map_err(|error| AudioQueueSendError::Closed(error.0)),
            Self::Bounded(sender) => sender.try_send(item).map_err(|error| match error {
                mpsc::error::TrySendError::Full(item) => AudioQueueSendError::Full(item),
                mpsc::error::TrySendError::Closed(item) => AudioQueueSendError::Closed(item),
            }),
        }
    }

    pub fn is_bounded(&self) -> bool {
        matches!(self, Self::Bounded(_))
    }
}

#[derive(Debug)]
pub enum AudioQueueReceiver<T> {
    Unbounded(mpsc::UnboundedReceiver<T>),
    Bounded(mpsc::Receiver<T>),
}

impl<T> AudioQueueReceiver<T> {
    pub async fn recv(&mut self) -> Option<T> {
        match self {
            Self::Unbounded(receiver) => receiver.recv().await,
            Self::Bounded(receiver) => receiver.recv().await,
        }
    }
}

pub fn unbounded<T>() -> (AudioQueueSender<T>, AudioQueueReceiver<T>) {
    let (sender, receiver) = mpsc::unbounded_channel();
    (
        AudioQueueSender::Unbounded(sender),
        AudioQueueReceiver::Unbounded(receiver),
    )
}

pub fn bounded<T>(capacity: usize) -> (AudioQueueSender<T>, AudioQueueReceiver<T>) {
    assert!(capacity > 0, "audio queue capacity must be greater than zero");
    let (sender, receiver) = mpsc::channel(capacity);
    (
        AudioQueueSender::Bounded(sender),
        AudioQueueReceiver::Bounded(receiver),
    )
}

pub fn create_audio_input_queue(
    config: StreamingPipelineConfig,
) -> (AudioQueueSender<crate::audio::AudioChunk>, AudioQueueReceiver<crate::audio::AudioChunk>) {
    if config.enabled {
        bounded(config.audio_input_capacity)
    } else {
        unbounded()
    }
}

pub fn create_transcription_queue(
    config: StreamingPipelineConfig,
) -> (AudioQueueSender<crate::audio::AudioChunk>, AudioQueueReceiver<crate::audio::AudioChunk>) {
    if config.enabled {
        bounded(config.transcription_capacity)
    } else {
        unbounded()
    }
}

pub fn create_recording_queue(
    config: StreamingPipelineConfig,
) -> (AudioQueueSender<crate::audio::AudioChunk>, AudioQueueReceiver<crate::audio::AudioChunk>) {
    if config.enabled {
        bounded(config.recording_capacity)
    } else {
        unbounded()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_queue_rejects_full_without_waiting() {
        let (sender, mut receiver) = bounded::<u8>(1);
        assert!(sender.try_send(1).is_ok());
        assert_eq!(sender.try_send(2), Err(AudioQueueSendError::Full(2)));

        let runtime = tokio::runtime::Runtime::new().expect("runtime should be available");
        assert_eq!(runtime.block_on(receiver.recv()), Some(1));
    }

    #[test]
    fn closed_queue_returns_the_item_to_the_caller() {
        let (sender, receiver) = bounded::<u8>(1);
        drop(receiver);
        assert_eq!(sender.try_send(7), Err(AudioQueueSendError::Closed(7)));
    }

    #[test]
    fn disabled_config_keeps_the_legacy_unbounded_adapter() {
        let (sender, _receiver) = create_audio_input_queue(StreamingPipelineConfig::default());
        assert!(!sender.is_bounded());
    }

    #[test]
    fn enabled_config_selects_the_configured_capacity() {
        let config = StreamingPipelineConfig {
            enabled: true,
            audio_input_capacity: 3,
            transcription_capacity: 2,
            recording_capacity: 4,
            mix_window_ms: 40,
        };
        let (sender, _receiver) = create_audio_input_queue(config);
        assert!(sender.is_bounded());
    }

    #[test]
    fn enabled_config_selects_bounded_recording_queue() {
        let config = StreamingPipelineConfig {
            enabled: true,
            audio_input_capacity: 3,
            transcription_capacity: 2,
            recording_capacity: 4,
            mix_window_ms: 40,
        };
        let (sender, _receiver) = create_recording_queue(config);
        assert!(sender.is_bounded());
    }

    #[test]
    fn only_supported_streaming_mix_windows_are_accepted() {
        assert_eq!(parse_mix_window_ms(Some("20")), 20);
        assert_eq!(parse_mix_window_ms(Some("40")), 40);
        assert_eq!(parse_mix_window_ms(Some("50")), 50);
        assert_eq!(parse_mix_window_ms(Some("600")), DEFAULT_STREAMING_MIX_WINDOW_MS);
        assert_eq!(parse_mix_window_ms(Some("invalid")), DEFAULT_STREAMING_MIX_WINDOW_MS);
    }

    #[test]
    fn legacy_mix_window_is_used_when_streaming_is_disabled() {
        let config = StreamingPipelineConfig::default();
        assert_eq!(config.effective_mix_window_ms(), LEGACY_MIX_WINDOW_MS);
    }
}
