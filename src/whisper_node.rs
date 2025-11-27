use cpal::traits::DeviceTrait;
use godot::classes::Node;
use godot::prelude::*;
use opus2::{Channels, Decoder};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::codec::decode_opus_to_stereo;
use crate::godot_thread_print::GodotThreadPrint;
use crate::microphone::Microphone;
use crate::runtime::Runtime;
use crate::whisper::WhisperKeywordSpotter;

#[derive(GodotClass)]
#[class(base=Node)]
struct Whisper {
    running: Arc<AtomicBool>,
    keywords: Vec<String>,
    spellbook: HashMap<String, String>,
    base: Base<Node>,
    microphone: Microphone,
    whisper_thread: Option<JoinHandle<()>>,
    matches: Arc<Mutex<Option<String>>>,
    reander: Receiver<Vec<u8>>,
    sender: Option<Sender<Vec<u8>>>,
    decoder: Decoder,
}

#[godot_api]
impl INode for Whisper {
    fn init(base: Base<Node>) -> Self {
        godot_print!("Hello, world!"); // Prints to the Godot console
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        Self {
            running: Runtime::running(),
            keywords: Vec::new(),
            spellbook: HashMap::new(),
            base,
            whisper_thread: None,
            microphone: Microphone::new(false).unwrap(),
            matches: Arc::new(Mutex::new(None)),
            reander: rx,
            sender: Some(tx),
            decoder: Decoder::new(48000, Channels::Stereo).unwrap(),
        }
    }

    fn process(&mut self, _delta: f64) {
        let magic = if let Ok(mut matches) = self.matches.try_lock() {
            let mut ret = None;
            if let Some(magic) = matches.take() {
                if magic != "" {
                    ret = Some(magic);
                }
            }
            ret
        } else {
            None
        };

        match self.reander.recv_timeout(Duration::from_millis(1)) {
            Ok(audio) => {
                self.signals().speak().emit(audio);
            }
            Err(_) => {}
        }

        if let Some(magic) = magic {
            let spell = self.spellbook[&magic].clone();
            self.signals().cast().emit(spell);
        }

        if let Some(thread) = self.whisper_thread.take() {
            if thread.is_finished() {
                match &thread.join() {
                    Ok(_) => godot_print!("Thread finished normally."),
                    Err(err) => {
                        if let Some(msg) = err.downcast_ref::<&str>() {
                            godot_print!("Thread panicked with message: {}", msg);
                        } else if let Some(msg) = err.downcast_ref::<String>() {
                            godot_print!("Thread panicked with message: {}", msg);
                        } else {
                            godot_print!("Thread panicked with unknown message type.");
                        }
                    }
                }
            } else {
                self.whisper_thread = Some(thread);
            }
        }
    }
}

#[godot_api]
impl Whisper {
    #[func]
    fn init_whisper(&mut self, model_path: String) {
        let (tx, rx) = std::sync::mpsc::channel::<Vec<f32>>();

        self.whisper_thread = Some(WhisperKeywordSpotter::start(
            model_path,
            rx,
            self.running.clone(),
            self.keywords.clone(),
            self.matches.clone(),
        ));

        // TODO Handle NONE sender

        match self.microphone.start(tx, self.sender.take().unwrap()) {
            Ok(_) => GodotThreadPrint::print("started".to_owned()),
            Err(err) => godot_error!("{:?}", err),
        }
    }

    #[func]
    fn get_sample_rate(&self) -> u32 {
        self.microphone.get_sample_rate()
    }

    #[func]
    fn decode_audio(&mut self, encoded: Vec<u8>, _sample_rate: i32) -> Vec<f32> {
        // let frame_size = sample_rate * 10 / 1000;
        decode_opus_to_stereo(&mut self.decoder, &encoded[..], 48000 as u32, 480 as usize).unwrap()
    }

    #[func]
    fn get_current_input_device(&self) -> GString {
        let device = self.microphone.get_current_input();
        if let Ok(name) = device.name() {
            return GString::from_str(name.as_str()).unwrap();
        }
        return GString::from_str("").unwrap();
    }

    #[func]
    fn list_input_devices(&self) -> Array<GString> {
        let mut inputs: Array<GString> = Array::new();

        for device in self.microphone.list_inputs() {
            if let Ok(name) = device.name() {
                let device_name: GString = GString::from_str(name.as_str()).unwrap();
                inputs.push(&device_name);
            }
        }

        inputs
    }

    #[func]
    fn select_input_device(&mut self, device_name: String) {
        let inputs = self.microphone.list_inputs();
        let device = inputs
            .iter()
            .find(|d| d.name().unwrap_or("".to_owned()) == device_name)
            .unwrap();
        self.microphone.set_input(device.clone());
        // TODO Implement the device change
    }

    #[func]
    fn register_spell_trigger(&mut self, trigger_frase: String, spell: String) {
        self.keywords.push(trigger_frase.clone());
        self.spellbook.insert(trigger_frase, spell);
    }

    #[signal]
    fn cast(magic: String);

    #[signal]
    fn speak(audio: Vec<u8>);
}
