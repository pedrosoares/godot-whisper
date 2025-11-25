pub mod codec;
pub mod godot_thread_print;
pub mod microphone;
pub mod opus_decoder_node;
pub mod runtime;
pub mod whisper;
pub mod whisper_node;

use godot::prelude::*;

use crate::runtime::Runtime;

struct WhisperExtension;

#[gdextension]
unsafe impl ExtensionLibrary for WhisperExtension {
    fn on_level_init(level: InitLevel) {
        match level {
            InitLevel::Scene => {
                godot_print!("Initializing Engine");
            }
            _ => (),
        }
    }

    fn on_level_deinit(level: InitLevel) {
        match level {
            InitLevel::Scene => {
                Runtime::free();
            }
            _ => (),
        }
    }
}
