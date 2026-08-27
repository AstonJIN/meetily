//! Development-only streaming Zipformer provider.
//!
//! The provider is intentionally opt-in.  The normal Meetily startup path keeps
//! selecting Whisper or Parakeet until the Zipformer replay and live-session
//! gates in the migration plan are complete.

use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig, OnlineStream};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::provider::TranscriptionError;

pub const ZIPFORMER_PILOT_ENV: &str = "MEETILY_ZIPFORMER_V1";
pub const ZIPFORMER_MODEL_DIR_ENV: &str = "MEETILY_ZIPFORMER_MODEL_DIR";
pub const ZIPFORMER_NUM_THREADS_ENV: &str = "MEETILY_ZIPFORMER_NUM_THREADS";
pub const ZIPFORMER_HOTWORDS_FILE_ENV: &str = "MEETILY_ZIPFORMER_HOTWORDS_FILE";
pub const ZIPFORMER_MODEL_DIR_NAME: &str =
    "sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20";

const ENCODER_FILE: &str = "encoder-epoch-99-avg-1.int8.onnx";
const DECODER_FILE: &str = "decoder-epoch-99-avg-1.onnx";
const JOINER_FILE: &str = "joiner-epoch-99-avg-1.int8.onnx";
const TOKENS_FILE: &str = "tokens.txt";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipformerModelPaths {
    pub root: PathBuf,
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub joiner: PathBuf,
    pub tokens: PathBuf,
}

impl ZipformerModelPaths {
    pub fn from_dir(root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root.into();
        if !root.is_dir() {
            return Err(format!(
                "Zipformer model directory does not exist: {}",
                root.display()
            ));
        }

        let paths = Self {
            encoder: root.join(ENCODER_FILE),
            decoder: root.join(DECODER_FILE),
            joiner: root.join(JOINER_FILE),
            tokens: root.join(TOKENS_FILE),
            root,
        };

        for (label, path) in [
            ("encoder", &paths.encoder),
            ("decoder", &paths.decoder),
            ("joiner", &paths.joiner),
            ("tokens", &paths.tokens),
        ] {
            let metadata = std::fs::metadata(path).map_err(|error| {
                format!(
                    "Zipformer {} file is missing ({}): {}",
                    label,
                    path.display(),
                    error
                )
            })?;
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(format!(
                    "Zipformer {} file is empty or not a regular file: {}",
                    label,
                    path.display()
                ));
            }
        }

        Ok(paths)
    }

    pub fn default_home_dir(home: &Path) -> PathBuf {
        home.join(ZIPFORMER_MODEL_DIR_NAME)
    }

    pub fn from_environment() -> Result<Self, String> {
        let root = if let Some(path) = std::env::var_os(ZIPFORMER_MODEL_DIR_ENV) {
            PathBuf::from(path)
        } else {
            let home = dirs::home_dir().ok_or_else(|| {
                format!(
                    "{} is not set and the home directory could not be resolved",
                    ZIPFORMER_MODEL_DIR_ENV
                )
            })?;
            Self::default_home_dir(&home)
        };

        Self::from_dir(root)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZipformerRuntimeConfig {
    pub model: ZipformerModelPaths,
    pub num_threads: i32,
    pub hotwords_file: Option<PathBuf>,
}

impl ZipformerRuntimeConfig {
    pub fn from_environment() -> Result<Self, String> {
        let num_threads = parse_num_threads(
            std::env::var(ZIPFORMER_NUM_THREADS_ENV).ok().as_deref(),
        )?;
        Ok(Self {
            model: ZipformerModelPaths::from_environment()?,
            num_threads,
            hotwords_file: std::env::var_os(ZIPFORMER_HOTWORDS_FILE_ENV).map(PathBuf::from),
        })
    }
}

pub fn pilot_enabled() -> bool {
    env_flag(std::env::var(ZIPFORMER_PILOT_ENV).ok().as_deref())
}

fn env_flag(value: Option<&str>) -> bool {
    matches!(
        value.map(|item| item.trim().to_ascii_lowercase()).as_deref(),
        Some("1") | Some("true") | Some("yes") | Some("on")
    )
}

fn parse_num_threads(value: Option<&str>) -> Result<i32, String> {
    let num_threads = value
        .unwrap_or("1")
        .trim()
        .parse::<i32>()
        .map_err(|error| {
            format!(
                "{} must be a positive integer, got '{}': {}",
                ZIPFORMER_NUM_THREADS_ENV,
                value.unwrap_or("1"),
                error
            )
        })?;

    if num_threads <= 0 {
        return Err(format!(
            "{} must be a positive integer, got {}",
            ZIPFORMER_NUM_THREADS_ENV, num_threads
        ));
    }

    Ok(num_threads)
}

#[derive(Debug, Clone, PartialEq)]
pub struct StreamingAsrResult {
    pub text: String,
    pub is_final: bool,
    pub start_time: Option<f64>,
}

/// Provider-neutral streaming ASR lifecycle used by the phase-2 pilot.
pub trait StreamingAsrEngine: Send + Sync {
    fn accept_audio(
        &self,
        sample_rate: u32,
        samples: &[f32],
    ) -> Result<Vec<StreamingAsrResult>, TranscriptionError>;
    fn partial(&self) -> Option<StreamingAsrResult>;
    fn final_result(&self) -> Option<StreamingAsrResult>;
    /// Replace the optional hotwords file. Replacing it resets the current
    /// stream so the new decoding configuration cannot mix with old state.
    fn set_hotwords_file(&self, hotwords_file: Option<&Path>) -> Result<(), TranscriptionError>;
    fn reset(&self) -> Result<(), TranscriptionError>;
    fn drain(&self) -> Result<Vec<StreamingAsrResult>, TranscriptionError>;
    fn unload(&self);
}

struct ZipformerSession {
    recognizer: OnlineRecognizer,
    stream: OnlineStream,
    last_partial: Option<StreamingAsrResult>,
    last_final: Option<StreamingAsrResult>,
    last_partial_text: String,
    has_input: bool,
}

pub struct ZipformerProvider {
    model: ZipformerModelPaths,
    num_threads: i32,
    hotwords_file: Mutex<Option<PathBuf>>,
    session: Mutex<Option<ZipformerSession>>,
}

impl ZipformerProvider {
    pub fn from_environment() -> Result<Self, String> {
        let runtime = ZipformerRuntimeConfig::from_environment()?;
        Self::from_runtime_config(runtime)
    }

    pub fn from_runtime_config(config: ZipformerRuntimeConfig) -> Result<Self, String> {
        let session = Self::build_session(
            &config.model,
            config.num_threads,
            config.hotwords_file.as_deref(),
        )?;

        Ok(Self {
            model: config.model,
            num_threads: config.num_threads,
            hotwords_file: Mutex::new(config.hotwords_file),
            session: Mutex::new(Some(session)),
        })
    }

    fn build_session(
        model: &ZipformerModelPaths,
        num_threads: i32,
        hotwords_file: Option<&Path>,
    ) -> Result<ZipformerSession, String> {
        let mut recognizer_config = OnlineRecognizerConfig::default();
        recognizer_config.model_config.transducer.encoder =
            Some(model.encoder.to_string_lossy().into_owned());
        recognizer_config.model_config.transducer.decoder =
            Some(model.decoder.to_string_lossy().into_owned());
        recognizer_config.model_config.transducer.joiner =
            Some(model.joiner.to_string_lossy().into_owned());
        recognizer_config.model_config.tokens =
            Some(model.tokens.to_string_lossy().into_owned());
        recognizer_config.model_config.num_threads = num_threads;
        recognizer_config.model_config.provider = Some("cpu".to_string());
        if let Some(path) = hotwords_file {
            let metadata = std::fs::metadata(path).map_err(|error| {
                format!(
                    "Zipformer hotwords file is not readable ({}): {}",
                    path.display(),
                    error
                )
            })?;
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(format!(
                    "Zipformer hotwords file is empty or not a regular file: {}",
                    path.display()
                ));
            }
            recognizer_config.hotwords_file = Some(path.to_string_lossy().into_owned());
            recognizer_config.hotwords_score = 1.0;
        }
        recognizer_config.enable_endpoint = true;
        recognizer_config.decoding_method = Some("greedy_search".to_string());

        let recognizer = OnlineRecognizer::create(&recognizer_config)
            .ok_or_else(|| "sherpa-onnx could not create the Zipformer recognizer".to_string())?;
        let stream = recognizer.create_stream();

        Ok(ZipformerSession {
            recognizer,
            stream,
            last_partial: None,
            last_final: None,
            last_partial_text: String::new(),
            has_input: false,
        })
    }

    pub fn model_root(&self) -> &Path {
        &self.model.root
    }

    fn session_error() -> TranscriptionError {
        TranscriptionError::ModelNotLoaded
    }

    fn lock_session(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, Option<ZipformerSession>>, TranscriptionError> {
        self.session
            .lock()
            .map_err(|_| TranscriptionError::EngineFailed("Zipformer session lock poisoned".to_string()))
    }

    fn take_result(
        session: &mut ZipformerSession,
        force_final: bool,
    ) -> Option<StreamingAsrResult> {
        let result = session.recognizer.get_result(&session.stream)?;
        let text = result.text.trim().to_string();
        if text.is_empty() {
            return None;
        }

        let is_final = force_final || result.is_final;
        let update = StreamingAsrResult {
            text: text.clone(),
            is_final,
            start_time: result.start_time.map(f64::from),
        };

        if is_final {
            session.last_final = Some(update.clone());
            session.last_partial = None;
            session.last_partial_text.clear();
            Some(update)
        } else if text != session.last_partial_text {
            session.last_partial_text = text;
            session.last_partial = Some(update.clone());
            Some(update)
        } else {
            None
        }
    }

    fn decode_ready(session: &mut ZipformerSession) -> Vec<StreamingAsrResult> {
        let mut results = Vec::new();
        while session.recognizer.is_ready(&session.stream) {
            session.recognizer.decode(&session.stream);
            if let Some(result) = Self::take_result(session, false) {
                results.push(result);
            }
        }
        results
    }
}

impl StreamingAsrEngine for ZipformerProvider {
    fn accept_audio(
        &self,
        sample_rate: u32,
        samples: &[f32],
    ) -> Result<Vec<StreamingAsrResult>, TranscriptionError> {
        if samples.is_empty() {
            return Err(TranscriptionError::AudioTooShort {
                samples: 0,
                minimum: 1,
            });
        }
        if sample_rate != 16_000 {
            return Err(TranscriptionError::EngineFailed(format!(
                "Zipformer requires 16000 Hz mono audio, got {} Hz",
                sample_rate
            )));
        }

        let mut guard = self.lock_session()?;
        let session = guard.as_mut().ok_or_else(Self::session_error)?;
        session.stream.accept_waveform(sample_rate as i32, samples);
        session.has_input = true;

        let results = Self::decode_ready(session);
        if session.recognizer.is_endpoint(&session.stream) {
            if let Some(result) = Self::take_result(session, true) {
                if !results.iter().any(|item| item.is_final && item.text == result.text) {
                    // The endpoint result is final even when the preceding
                    // decode returned the same text as a partial hypothesis.
                    let mut results = results;
                    results.push(result);
                    session.recognizer.reset(&session.stream);
                    session.has_input = false;
                    return Ok(results);
                }
            }
            session.recognizer.reset(&session.stream);
            session.has_input = false;
        }

        Ok(results)
    }

    fn partial(&self) -> Option<StreamingAsrResult> {
        self.session.lock().ok()?.as_ref()?.last_partial.clone()
    }

    fn final_result(&self) -> Option<StreamingAsrResult> {
        self.session.lock().ok()?.as_ref()?.last_final.clone()
    }

    fn set_hotwords_file(&self, hotwords_file: Option<&Path>) -> Result<(), TranscriptionError> {
        let new_session = Self::build_session(&self.model, self.num_threads, hotwords_file)
            .map_err(TranscriptionError::EngineFailed)?;
        let mut session = self.lock_session()?;
        *session = Some(new_session);
        let mut current_hotwords = self.hotwords_file.lock().map_err(|_| {
            TranscriptionError::EngineFailed("Zipformer hotwords lock poisoned".to_string())
        })?;
        *current_hotwords = hotwords_file.map(Path::to_path_buf);
        Ok(())
    }

    fn reset(&self) -> Result<(), TranscriptionError> {
        let mut guard = self.lock_session()?;
        let session = guard.as_mut().ok_or_else(Self::session_error)?;
        session.recognizer.reset(&session.stream);
        session.last_partial = None;
        session.last_final = None;
        session.last_partial_text.clear();
        session.has_input = false;
        Ok(())
    }

    fn drain(&self) -> Result<Vec<StreamingAsrResult>, TranscriptionError> {
        let mut guard = self.lock_session()?;
        let session = guard.as_mut().ok_or_else(Self::session_error)?;
        if !session.has_input {
            return Ok(Vec::new());
        }

        session.stream.input_finished();
        let mut results = Self::decode_ready(session);
        if let Some(result) = Self::take_result(session, true) {
            if !results.iter().any(|item| item.is_final && item.text == result.text) {
                results.push(result);
            }
        }
        session.recognizer.reset(&session.stream);
        session.has_input = false;
        Ok(results)
    }

    fn unload(&self) {
        if let Ok(mut guard) = self.session.lock() {
            *guard = None;
        }
    }
}

impl ZipformerProvider {
    pub fn is_loaded(&self) -> bool {
        self.session.lock().map(|session| session.is_some()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn validates_expected_bilingual_model_layout() {
        let temp = tempfile::tempdir().expect("temp dir");
        for file in [ENCODER_FILE, DECODER_FILE, JOINER_FILE, TOKENS_FILE] {
            fs::write(temp.path().join(file), b"model").expect("model fixture");
        }

        let paths = ZipformerModelPaths::from_dir(temp.path()).expect("valid layout");
        assert_eq!(paths.encoder.file_name().unwrap(), ENCODER_FILE);
        assert_eq!(paths.decoder.file_name().unwrap(), DECODER_FILE);
        assert_eq!(paths.joiner.file_name().unwrap(), JOINER_FILE);
        assert_eq!(paths.tokens.file_name().unwrap(), TOKENS_FILE);
    }

    #[test]
    fn rejects_incomplete_model_layout() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(temp.path().join(ENCODER_FILE), b"model").expect("model fixture");

        let error = ZipformerModelPaths::from_dir(temp.path()).expect_err("missing files");
        assert!(error.contains("decoder"));
    }

    #[test]
    fn default_home_path_matches_downloaded_archive_name() {
        let home = Path::new("/tmp/test-home");
        assert_eq!(
            ZipformerModelPaths::default_home_dir(home),
            home.join(ZIPFORMER_MODEL_DIR_NAME)
        );
    }

    #[test]
    fn runtime_config_rejects_non_positive_thread_count() {
        assert!(parse_num_threads(Some("0")).is_err());
        assert!(parse_num_threads(Some("-1")).is_err());
        assert!(parse_num_threads(Some("not-a-number")).is_err());
        assert_eq!(parse_num_threads(None).unwrap(), 1);
    }

    #[test]
    fn accepts_common_truthy_pilot_flags_only() {
        assert!(env_flag(Some("1")));
        assert!(env_flag(Some(" TRUE ")));
        assert!(env_flag(Some("on")));
        assert!(!env_flag(Some("0")));
        assert!(!env_flag(Some("disabled")));
        assert!(!env_flag(None));
    }

    #[test]
    fn hotwords_are_optional_in_the_runtime_contract() {
        let model = ZipformerModelPaths {
            root: PathBuf::from("/tmp/model"),
            encoder: PathBuf::from("encoder"),
            decoder: PathBuf::from("decoder"),
            joiner: PathBuf::from("joiner"),
            tokens: PathBuf::from("tokens"),
        };
        let config = ZipformerRuntimeConfig {
            model,
            num_threads: 1,
            hotwords_file: None,
        };
        assert!(config.hotwords_file.is_none());
    }

    #[test]
    #[ignore = "requires the downloaded Zipformer archive and native sherpa-onnx runtime"]
    fn replays_downloaded_bilingual_fixture() {
        let provider = ZipformerProvider::from_environment().expect("Zipformer model");
        let wav_path = provider.model_root().join("test_wavs/0.wav");
        let wav = sherpa_onnx::Wave::read(wav_path.to_str().expect("UTF-8 wav path"))
            .expect("test wav");

        let mut emitted = Vec::new();
        for frame in wav.samples().chunks(320) {
            emitted.extend(
                provider
                    .accept_audio(wav.sample_rate() as u32, frame)
                    .expect("streaming decode"),
            );
        }
        emitted.extend(provider.drain().expect("stream drain"));

        assert!(
            emitted.iter().any(|result| !result.text.is_empty()),
            "fixture replay produced no transcription updates"
        );
        assert!(
            emitted.iter().any(|result| result.is_final),
            "fixture replay did not produce a final result"
        );
        assert!(provider.final_result().is_some());
        provider.unload();
        assert!(!provider.is_loaded());
    }
}
