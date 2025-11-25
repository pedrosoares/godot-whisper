use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
    },
    thread::JoinHandle,
};
use whisper_rs::{FullParams, WhisperContextParameters, WhisperState};

use crate::godot_thread_print::GodotThreadPrint;

#[derive(Debug, Clone)]
pub struct KeywordDetection {
    pub keyword: String,
    pub transcription: String,
    pub confidence: f32,
    pub timestamp: std::time::SystemTime,
}

/// ML-based Keyword Spotter using Whisper
pub struct WhisperKeywordSpotter {
    pub ctx: whisper_rs::WhisperContext,
    keywords: Vec<String>,
}

impl WhisperKeywordSpotter {
    pub fn new(
        model_path: &str,
        keywords: Vec<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Load Whisper model
        let mut params = WhisperContextParameters::default();
        params.use_gpu(true);
        let ctx = whisper_rs::WhisperContext::new_with_params(model_path, params)?;

        Ok(Self { ctx, keywords })
    }

    /// Transcribe audio and detect keywords
    pub fn detect(
        &mut self,
        state: &mut WhisperState,
        params: FullParams,
        samples: &[f32],
    ) -> Result<Option<KeywordDetection>, Box<dyn std::error::Error>> {
        // Transcribe
        let result = state.full(params, samples)?;
        assert!(result == 0, "stat.full error");

        let num_segments = state.full_n_segments();
        let mut transcription = String::new();

        for i in 0..num_segments {
            let segment = state.get_segment(i).unwrap();
            transcription.push_str(&segment.to_str().unwrap());
            transcription.push(' ');
        }

        let transcription = transcription.trim().to_lowercase();

        if transcription.is_empty() {
            GodotThreadPrint::print(format!("is_empty"));
            return Ok(None);
        }

        GodotThreadPrint::print(format!("📝 Transcribed: \"{}\"", transcription));

        // Check for keywords
        for keyword in &self.keywords {
            if transcription.contains(&keyword.to_lowercase()) {
                return Ok(Some(KeywordDetection {
                    keyword: keyword.clone(),
                    transcription: transcription.clone(),
                    confidence: 0.9, // Whisper doesn't provide per-word confidence easily
                    timestamp: std::time::SystemTime::now(),
                }));
            }
        }

        Ok(None)
    }

    fn is_silence(samples: &[f32], threshold: f32) -> bool {
        if samples.is_empty() {
            return true;
        }

        let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        rms < threshold
    }

    pub fn start(
        model_path: String,
        rx: Receiver<Vec<f32>>,
        running: Arc<AtomicBool>,
        keywords: Vec<String>,
        matches: Arc<Mutex<Option<String>>>,
    ) -> JoinHandle<()> {
        return std::thread::spawn(move || {
            GodotThreadPrint::print("Initializing Whisper".to_owned());
            let mut spotter = match WhisperKeywordSpotter::new(&model_path, keywords) {
                Ok(s) => s,
                Err(e) => {
                    GodotThreadPrint::print(format!(
                        "Failed to initialize Whisper: {}, model: {}",
                        e, model_path
                    ));
                    return;
                }
            };

            // Create parameters for transcription
            let mut params =
                whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });

            // Configure for real-time, English only
            params.set_language(Some("en"));
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_n_threads(2);

            // Create a mutable state
            let mut state = spotter.ctx.create_state().unwrap();

            let mut buffer: Vec<f32> = Vec::new();

            let silence_threshold = 0.015;
            let silence_hold = 2048 * 2;
            let silence_check_tail = 512; // NEW
            let mut silence_samples = 0;

            while !running.load(Ordering::Relaxed) {
                match rx.recv() {
                    Ok(bytes) => {
                        buffer.extend(bytes.clone());

                        let check = if bytes.len() > silence_check_tail {
                            &bytes[bytes.len() - silence_check_tail..]
                        } else {
                            &bytes
                        };

                        let silent = Self::is_silence(check, silence_threshold);

                        if silent {
                            silence_samples += bytes.len();
                        } else {
                            silence_samples = 0;
                        }

                        if silence_samples >= silence_hold && !buffer.is_empty() {
                            silence_samples = 0;
                        } else {
                            if buffer.len() < (16000 * 3) as usize {
                                continue;
                            }
                            // continue;
                        }

                        let silent = Self::is_silence(&buffer[..], silence_threshold);

                        if silent {
                            silence_samples = 0;
                            buffer.clear();
                            continue;
                        }

                        match spotter.detect(&mut state, params.clone(), &buffer) {
                            Ok(Some(detection)) => {
                                GodotThreadPrint::print(format!(
                                    "🔊 Keyword detected: '{}' in \"{}\"",
                                    detection.keyword, detection.transcription
                                ));
                                *matches.lock().unwrap() = Some(detection.keyword.clone());
                            }
                            _ => {}
                        }

                        buffer.clear();
                    }
                    Err(_err) => {
                        break;
                    }
                }
            }
        });
    }
}
