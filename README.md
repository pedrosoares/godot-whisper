# 🎤 Godot Whisper Bridge — Rust GDExtension

**Whisper (Vulkan/Metal) + Microphone + Opus Codec — for Keyword Spotting in Godot**

This project is a high-performance **Rust GDExtension** that brings real-time speech features to **Godot 4**, including:

✅ Whisper inference with **Vulkan** (Windows/Linux) or **Metal** (macOS)

✅ Real-time **keyword spotting** and streaming transcription

✅ Native **microphone capture** with a dedicated Rust audio thread

✅ Full **Opus codec** support (encode + decode)

✅ Godot nodes exposed via `whisper_node.rs`, `opus_decoder_node.rs`

The entire extension is optimized for **low-latency**, **real-time** voice interactions for gameplay, voice commands, NPC interactions, and networked voice chat.

---

## 📂 Project Structure

Matches exactly the files you listed:

```
src/
 ├── codec.rs                 # Low-level Opus and audio codec utilities
 ├── godot_thread_print.rs    # Thread-safe print wrapper for Godot (Debug Only)
 ├── lib.rs                   # GDExtension entry point
 ├── microphone.rs            # Native microphone capture + PCM buffering
 ├── opus_decoder_node.rs     # Godot-exposed Opus decoder node
 ├── runtime.rs               # Internal async runtime (channels, threads)
 ├── whisper.rs               # Whisper (Vulkan/Metal) core wrapper
 ├── whisper_node.rs          # Godot-facing Whisper node (keywords, streaming)
```

---

## 🚀 Features

### 🎙 Whisper via Vulkan (Windows/Linux) or Metal (macOS)

Powered by **whisper-rs** with GPU backends:

* `--features vulkan` for Windows/Linux
* `--features metal` for macOS (In Progress)

Provides:

* Streaming transcription
* Very low-latency inference
* Tuned for keyword spotting
* GGML model support (`tiny`, `base`, etc.)

---

### 🔑 Keyword Spotting

The `WhisperNode` automatically detects specific keywords from streaming audio:

Disclaimer: The method and event will be renamed to a generic one.

```gdscript
# First parameter is the trigger word, The second is the emited word (For variantion purpose)
# For example:
# whisper.register_spell_trigger("fire ball", "fireball")
# whisper.register_spell_trigger("fireball", "fireball")
# Both will emit as "fireball"

whisper.register_spell_trigger("fire", "fire")
```

Godot signal:

```gdscript
signal cast(spell: String)
```

---

### 🔊 Native Microphone Capture

Handled in `microphone.rs` using:

* A dedicated audio thread
* Ring-buffer or channel-based streaming
* Automatic PCM normalization
* Non-blocking integration with Godot

```
signal speak(encoded_buffer: Array[int])
```

---

### 🎵 Opus Codec API

Provided through:

* `codec.rs` (Opus bindings + helpers)
* `opus_decoder_node.rs` (Godot-facing API)

Features:

* Encode 16-bit PCM → Opus
* Decode Opus → PCM
* Suitable for multiplayer voice chat or networked commands

Example GDScript:

```gdscript
var encoded = opus.encode(pcm) # In Progress
var decoded = opus.decode_audio(encoded)
```

---

## 📥 Installing in Godot

Copy the built extension into your Godot project:

```
/project
  /bin
    whisper_bridge.dll        # Windows
    whisper_bridge.so         # Linux
    whisper_bridge.dylib      # macOS
  /whisper_models
    ggml-base.en.bin
  project.godot
```

Enable GDExtensions:

```
project > Plugins > whisper_bridge
```

---

## 🛠 Building Manually

### Linux/Windows (Vulkan)

```sh
cargo build --release --features vulkan
```

### macOS (Metal)
In Progress
```sh
cargo build --release --features metal
```

Output files:

```
target/release/*.dll
target/release/*.so
target/release/*.dylib
```

---

## 🧪 Godot Usage Examples

### Initialize Whisper

```gdscript
var whisper := WhisperNode.new()
whisper.init_whisper("res://whisper_models/ggml-base.en.bin")
```

### Start listening for keywords

```gdscript
whisper.register_spell_trigger("fire", "fire")
whisper.connect("cast", _on_keyword_detected)
```

Handle detections:

```gdscript
func _on_keyword_detected(keyword: String) -> void:
    print("Detected:", keyword)
```

---

## 🌀 Microphone Streaming to Whisper

```gdscript
whisper.connect("speak", _speak)
```

Pull live transcription (In Progress):

```gdscript
func _process(delta):
    var text = whisper.get_transcription()
    if text != "":
        print(text)
```

---

## 📦 Cargo Features

| Feature  | Description                     |
| -------- | ------------------------------- |
| `vulkan` | GPU inference for Windows/Linux |
| `metal`  | GPU inference for macOS         |

---

## 🖥 Supported Platforms

| Platform | Microphone | Whisper | GPU    | Opus |
| -------- | ---------- | ------- | ------ | ---- |
| Windows  | ✔️         | ✔️      | Vulkan | ✔️   |
| Linux    | ✔️         | ✔️      | Vulkan | ✔️   |
| macOS    | ✔️         | ✔️      | Metal  | ✔️   |

---

## 🤝 Contributing

PRs and issues are welcome — particularly for:

* Improving real-time latency
* Expanding Opus tools
* Godot editor helpers
* Examples & demos

---

## 📄 License

MIT License — fully open for commercial and private use.

---
