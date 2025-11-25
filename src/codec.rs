use opus2::{Application, Bandwidth, Channels, Decoder, Encoder, Signal};
use std::error::Error;

/// Encode a stereo f32 buffer to Opus with packet framing.
/// frame_size = frames per channel
pub fn encode_stereo_to_opus(
    encoder: &mut Encoder,
    stereo: &[f32],
    sample_rate: u32,
    frame_size: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    validate_input(stereo, sample_rate, frame_size)?;

    encoder.set_bitrate(opus2::Bitrate::Bits(128000))?;
    encoder.set_bandwidth(Bandwidth::Fullband)?;
    encoder.set_signal(Signal::Music)?;

    let mut output = Vec::new();
    let samples_per_frame = frame_size * 2;
    let total_frames = stereo.len() / samples_per_frame;

    for frame_idx in 0..total_frames {
        let offset = frame_idx * samples_per_frame;
        let frame = &stereo[offset..offset + samples_per_frame];

        let mut encoded_buf = vec![0u8; 4000];
        let encoded_len = encoder.encode_float(frame, &mut encoded_buf)?;

        output.extend_from_slice(&(encoded_len as u16).to_le_bytes());
        output.extend_from_slice(&encoded_buf[..encoded_len]);
    }

    Ok(output)
}

/// Decode Opus data back to stereo f32 buffer
pub fn decode_opus_to_stereo(
    decoder: &mut Decoder,
    opus_data: &[u8],
    sample_rate: u32,
    frame_size: usize,
) -> Result<Vec<f32>, Box<dyn Error>> {
    let mut output = Vec::new();
    let mut offset = 0;

    while offset + 2 <= opus_data.len() {
        let packet_len = u16::from_le_bytes([opus_data[offset], opus_data[offset + 1]]) as usize;
        offset += 2;

        if offset + packet_len > opus_data.len() {
            break;
        }

        let packet = &opus_data[offset..offset + packet_len];
        offset += packet_len;

        let mut pcm = vec![0f32; frame_size * 2];

        match decoder.decode_float(packet, &mut pcm, false) {
            Ok(decoded_frames) => output.extend_from_slice(&pcm[..decoded_frames * 2]),
            Err(_) => output.extend(vec![0.0f32; frame_size * 2]),
        }
    }

    Ok(output)
}

/// Encode into individual Opus packets
pub fn encode_stereo_to_opus_packets(
    stereo: &[f32],
    sample_rate: u32,
    frame_size: usize,
) -> Result<Vec<Vec<u8>>, Box<dyn Error>> {
    validate_input(stereo, sample_rate, frame_size)?;

    let mut encoder = Encoder::new(sample_rate, Channels::Stereo, Application::Audio)?;
    encoder.set_bitrate(opus2::Bitrate::Bits(128000))?;
    encoder.set_bandwidth(Bandwidth::Fullband)?;
    encoder.set_signal(Signal::Music)?;

    let mut packets = Vec::new();
    let samples_per_frame = frame_size * 2;

    for chunk in stereo.chunks(samples_per_frame) {
        if chunk.len() != samples_per_frame {
            continue;
        }

        let mut buf = vec![0u8; 4000];
        let len = encoder.encode_float(chunk, &mut buf)?;
        buf.truncate(len);

        packets.push(buf);
    }

    Ok(packets)
}

/// Decode individual Opus packets to stereo f32
pub fn decode_opus_packets_to_stereo(
    packets: &[Vec<u8>],
    sample_rate: u32,
    frame_size: usize,
) -> Result<Vec<f32>, Box<dyn Error>> {
    let mut decoder = Decoder::new(sample_rate, Channels::Stereo)?;
    let mut output = Vec::new();

    for packet in packets {
        let mut pcm = vec![0f32; frame_size * 2];

        match decoder.decode_float(packet, &mut pcm, false) {
            Ok(decoded_frames) => output.extend_from_slice(&pcm[..decoded_frames * 2]),
            Err(_) => output.extend(vec![0.0f32; frame_size * 2]),
        }
    }

    Ok(output)
}

/// Validate input buffer, sample rate, frame size
fn validate_input(
    stereo: &[f32],
    sample_rate: u32,
    frame_size: usize,
) -> Result<(), Box<dyn Error>> {
    if !matches!(sample_rate, 8000 | 12000 | 16000 | 24000 | 48000) {
        return Err("Invalid sample rate".into());
    }

    if !get_valid_frame_sizes(sample_rate).contains(&frame_size) {
        return Err("Invalid frame size".into());
    }

    if stereo.len() % 2 != 0 {
        return Err("Stereo input must be interleaved".into());
    }

    Ok(())
}

/// Allowed frame sizes for each sample rate
fn get_valid_frame_sizes(sample_rate: u32) -> Vec<usize> {
    match sample_rate {
        48000 => vec![120, 240, 480, 960, 1920, 2880],
        24000 => vec![60, 120, 240, 480, 960, 1440],
        16000 => vec![40, 80, 160, 320, 640, 960],
        12000 => vec![30, 60, 120, 240, 480, 720],
        8000 => vec![20, 40, 80, 160, 320, 480],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_encode_decode() {
        let sample_rate = 48000;
        let frame_size = 960;
        let duration_frames = 50;

        let mut stereo_samples = Vec::new();
        for i in 0..(frame_size * duration_frames) {
            let t = i as f32 / sample_rate as f32;
            let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
            stereo_samples.push(s);
            stereo_samples.push(s);
        }

        let packets =
            encode_stereo_to_opus_packets(&stereo_samples, sample_rate, frame_size).unwrap();
        let decoded = decode_opus_packets_to_stereo(&packets, sample_rate, frame_size).unwrap();

        assert!(decoded.len() > 0);

        let delay = 312 * 2; // approximate Opus lookahead in stereo samples
        let compare_len = decoded.len().min(stereo_samples.len() - delay);

        let mut signal = 0.0;
        let mut noise = 0.0;

        for i in 0..compare_len {
            let orig = stereo_samples[i + delay] as f64;
            let dec = decoded[i] as f64;
            let err = orig - dec;

            signal += orig * orig;
            noise += err * err;
        }

        let snr = 10.0 * (signal / noise.max(1e-12)).log10();
        println!("SNR = {:.2} dB", snr);

        assert!(snr > 25.0, "SNR too low!"); // agora seguro para f32 contínuo
    }
}
