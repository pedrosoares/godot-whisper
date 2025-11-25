use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, Stream, StreamConfig, SupportedStreamConfig};
use godot::global::godot_print;
use opus2::{Application, Channels, Encoder};
use rubato::{
    Resampler, SincFixedOut, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::error::Error;
use std::sync::mpsc::Sender;

use crate::codec::encode_stereo_to_opus;
use crate::godot_thread_print::GodotThreadPrint;

const OPUS_FRAME_SIZE: usize = 480; // 20ms @ 48kHz

pub struct Microphone {
    host: Host,
    device: Device,
    output_device: Option<Device>,
    config: Option<SupportedStreamConfig>,
    stream: Option<Stream>,
    output_config: Option<SupportedStreamConfig>,
    output_stream: Option<Stream>,
    debug: bool,
}

impl Microphone {
    pub fn new(debug: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        godot_print!("Using input device: {}", device.name()?);

        let config = device.default_input_config().ok();
        godot_print!("Default input config: {:?}", config);

        let (output_device, output_config) = if debug {
            let device = host.default_output_device().unwrap();
            godot_print!("Using output device: {}", device.name()?);

            let config = device.default_output_config().unwrap();
            godot_print!("Default output config: {:?}", config);

            (Some(device), Some(config))
        } else {
            (None, None)
        };

        Ok(Self {
            host,
            device,
            output_device,
            config,
            stream: None,
            output_config,
            output_stream: None,
            debug,
        })
    }

    pub fn get_current_input(&self) -> Device {
        self.device.clone()
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.config.clone().unwrap().sample_rate().0
    }

    pub fn list_inputs(&self) -> Vec<Device> {
        match self.host.input_devices() {
            Ok(devices) => devices.collect(),
            _ => Vec::new(),
        }
    }

    pub fn set_input(&mut self, device: Device) {
        self.device = device;
        self.config = self.device.default_input_config().ok();
    }

    pub fn rubato_resample(
        stereo_samples: Vec<f32>,
        sample_rate: f32,
        to_sample_rate: f32,
    ) -> Result<Vec<f32>, Box<dyn Error>> {
        // Se as taxas são iguais, retorna direto
        if (sample_rate - to_sample_rate).abs() < 0.01 {
            return Ok(stereo_samples);
        }

        // Separar canais interleaved -> [left_channel, right_channel]
        let frames = stereo_samples.len() / 2;
        let mut left: Vec<f64> = Vec::with_capacity(frames);
        let mut right: Vec<f64> = Vec::with_capacity(frames);

        for chunk in stereo_samples.chunks_exact(2) {
            left.push(chunk[0] as f64);
            right.push(chunk[1] as f64);
        }

        // Calcular número de frames de saída
        let ratio = to_sample_rate as f64 / sample_rate as f64;
        let output_frames = (frames as f64 * ratio).round() as usize;

        // Configurar parâmetros de interpolação sinc
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        // Criar resampler com tamanho de saída fixo
        let mut resampler = SincFixedOut::<f64>::new(ratio, 2.0, params, output_frames, 2)?;

        // CRÍTICO: Verificar quantos frames de entrada são necessários
        let input_frames_needed = resampler.input_frames_next();

        // Adicionar padding se necessário
        left.resize(input_frames_needed, 0.0);
        right.resize(input_frames_needed, 0.0);

        // Preparar dados de entrada
        let waves_in = vec![left, right];

        // Processar
        let waves_out = resampler.process(&waves_in, None)?;

        // Intercalar canais de volta: [L, R, L, R, ...]
        let mut result = Vec::with_capacity(output_frames * 2);
        for i in 0..output_frames {
            result.push(waves_out[0][i] as f32);
            result.push(waves_out[1][i] as f32);
        }

        Ok(result)
    }

    /// Linearly resample interleaved stereo audio
    /// Simple linear resampling for stereo audio
    fn resample_linear_stereo(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return samples.to_vec();
        }

        const CHANNELS: usize = 2;
        let ratio = from_rate as f64 / to_rate as f64;
        let input_frames = samples.len() / CHANNELS;
        let output_frames = (input_frames as f64 / ratio).round() as usize;

        let mut output = Vec::with_capacity(output_frames * CHANNELS);

        for i in 0..output_frames {
            let pos = i as f64 * ratio;
            let idx = pos.floor() as usize;
            let frac = (pos - idx as f64) as f32;

            for ch in 0..CHANNELS {
                let s0 = samples.get(idx * CHANNELS + ch).copied().unwrap_or(0.0);
                let s1 = samples
                    .get((idx + 1) * CHANNELS + ch)
                    .copied()
                    .unwrap_or(s0);

                output.push(s0 * (1.0 - frac) + s1 * frac);
            }
        }

        output
    }

    /// Simple linear resampling
    fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return samples.to_vec();
        }

        let ratio = from_rate as f32 / to_rate as f32;
        let output_len = (samples.len() as f32 / ratio) as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            let pos = i as f32 * ratio;
            let idx = pos as usize;

            if idx + 1 < samples.len() {
                let frac = pos - idx as f32;
                let sample = samples[idx] * (1.0 - frac) + samples[idx + 1] * frac;
                output.push(sample);
            } else if idx < samples.len() {
                output.push(samples[idx]);
            }
        }

        output
    }

    fn build_stream(
        &mut self,
        tx: Sender<Vec<f32>>,
        relay_audio: Sender<Vec<u8>>,
    ) -> Result<cpal::Stream, Box<dyn std::error::Error>> {
        GodotThreadPrint::print(format!("Building Stream"));
        let config: StreamConfig = self.config.clone().unwrap().into();
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate.0;
        godot_print!("sample_rate: {}", config.sample_rate.0);
        let target_sample_rate = 16000; // Whisper expects 16kHz

        let (dtx, drx) = std::sync::mpsc::channel::<Vec<f32>>();

        if self.debug {
            match &self.output_device {
                Some(output_device) => {
                    let oc = self.output_config.clone().unwrap();
                    let config: StreamConfig = oc.into();
                    let output_stream = output_device
                        .build_output_stream(
                            &config,
                            move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                                let mut mic_data: Vec<f32> = Vec::new();
                                loop {
                                    match drx.recv() {
                                        Ok(buffer) => {
                                            mic_data.extend(buffer);

                                            if mic_data.len() >= output.len() {
                                                break;
                                            }
                                            // data.copy_from_slice(&mic_data[..data.len()]);
                                        }
                                        Err(err) => GodotThreadPrint::print(format!(
                                            "Reading Mic Data {:?}",
                                            err
                                        )),
                                    }
                                }
                                for (i, v) in mic_data.iter().enumerate() {
                                    if i < output.len() {
                                        output[i] = v.clone();
                                    }
                                }
                            },
                            |err| GodotThreadPrint::print(format!("Stream error: {}", err)),
                            None,
                        )
                        .unwrap();
                    output_stream.play().unwrap();
                    self.output_stream = Some(output_stream);
                }
                None => GodotThreadPrint::print(format!("No output device")),
            }
        }

        let debug = self.debug.clone();
        let mut local_buffer: Vec<f32> = Vec::new();
        let mut encoder = Encoder::new(48000, Channels::Stereo, Application::Voip).unwrap();
        let stream = self.device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if debug {
                    match dtx.send(data.to_vec()) {
                        Err(err) => GodotThreadPrint::print(format!("Stream error: {}", err)),
                        _ => {}
                    }
                }

                // let sampled =
                //     match Self::rubato_resample(data.to_vec(), sample_rate as f32, 48000.0) {
                //         Ok(a) => a,
                //         Err(err) => {
                //             let error = format!("{:?}", err);
                //             GodotThreadPrint::print(error);
                //             panic!("error on opus");
                //         }
                //     };

                fn normalize_f32_inplace(samples: &mut [f32]) {
                    let mut max = 0.0f32;
                    for &s in samples.iter() {
                        if s.abs() > max {
                            max = s.abs();
                        }
                    }
                    if max > 1.0 {
                        for v in samples.iter_mut() {
                            *v /= max;
                        }
                    }
                }

                // let mut cpal_buffer = data.to_vec().clone();
                // normalize_f32_inplace(&mut cpal_buffer);

                let sampled = Self::resample_linear_stereo(&data[..], sample_rate as u32, 48000);

                local_buffer.extend(sampled);

                let samples_per_frame = OPUS_FRAME_SIZE * channels;

                // Processar todos os frames completos disponíveis
                while local_buffer.len() >= samples_per_frame {
                    let frame: Vec<f32> = local_buffer.drain(..samples_per_frame).collect();

                    let duration_seconds =
                        (frame.len() as f32 / (48000 as f32 * channels as f32)) * 1000.0;

                    let frame_size = 48000 * duration_seconds as i32 / 1000;

                    GodotThreadPrint::print(format!(
                        "frame_size: {}, duration: {}, sampled: {}",
                        frame_size,
                        duration_seconds,
                        frame.len()
                    ));

                    let opus_encoded = match encode_stereo_to_opus(
                        &mut encoder,
                        &frame[..],
                        48000,
                        OPUS_FRAME_SIZE,
                    ) {
                        Ok(a) => a,
                        Err(err) => {
                            let error = format!("{:?}", err);
                            GodotThreadPrint::print(error);
                            panic!("error on opus");
                        }
                    };

                    relay_audio.send(opus_encoded).unwrap();
                }

                // if frame_size > 0 {
                //     let opus_encoded = match encode_stereo_to_opus(
                //         &local_buffer[..],
                //         48000,
                //         frame_size as usize,
                //     ) {
                //         Ok(a) => a,
                //         Err(err) => {
                //             let error = format!("{:?}", err);
                //             GodotThreadPrint::print(error);
                //             panic!("error on opus");
                //         }
                //     };

                //     relay_audio.send(opus_encoded).unwrap();
                // }

                let inv_channels = 1.0 / channels as f32;

                let mono_samples: Vec<f32> = data
                    .chunks(channels)
                    .map(|frame| frame.iter().copied().sum::<f32>() * inv_channels)
                    .collect();

                // Resample if needed
                let resampled = if sample_rate != target_sample_rate {
                    Self::resample_linear(&mono_samples, sample_rate, target_sample_rate)
                } else {
                    mono_samples
                };

                // GodotThreadPrint::print(format!("1: Sending: {}", resampled.len()));

                match tx.send(resampled) {
                    Err(err) => GodotThreadPrint::print(format!("1: Stream error: {}", err)),
                    _ => {}
                }
            },
            |err| GodotThreadPrint::print(format!("2: Stream error: {}", err)),
            None,
        )?;

        Ok(stream)
    }

    pub fn start(
        &mut self,
        tx: Sender<Vec<f32>>,
        relay_audio: Sender<Vec<u8>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Start audio capture
        let stream = match self.config.clone().unwrap().sample_format() {
            cpal::SampleFormat::F32 => self.build_stream(tx, relay_audio)?,
            _ => return Err("Unsupported sample format".into()),
        };

        stream.play()?;

        godot_print!("\n🎤 Listening for keywords with Whisper ML... (Press Ctrl+C to stop)\n");
        //godot_print!("Keywords: {:?}\n", keywords);
        godot_print!("Speak into your microphone!\n");

        self.stream = Some(stream);

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            let _ = stream.pause();
        }
    }
}
