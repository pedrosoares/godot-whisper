use godot::prelude::*;
use opus2::{Channels, Decoder};

use crate::codec::decode_opus_to_stereo;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct OpusDecoderNode {
    base: Base<Node>,
    decoder: Decoder,
    sample_rate: u32,
    frame_size: usize,
}

#[godot_api]
impl INode for OpusDecoderNode {
    fn init(base: Base<Node>) -> Self {
        let sample_rate = 48000;
        Self {
            base,
            decoder: Decoder::new(sample_rate, Channels::Stereo).unwrap(),
            sample_rate,
            frame_size: 480,
        }
    }
}

#[godot_api]
impl OpusDecoderNode {
    #[func]
    pub fn decode_audio(&mut self, encoded: Vec<u8>) -> Vec<f32> {
        decode_opus_to_stereo(
            &mut self.decoder,
            &encoded[..],
            self.sample_rate,
            self.frame_size,
        )
        .unwrap()
    }

    #[func]
    pub fn set_frame_size(&mut self, frame_size: u32) {
        self.frame_size = frame_size as usize;
    }
}
