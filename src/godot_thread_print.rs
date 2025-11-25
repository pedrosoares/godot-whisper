use godot::prelude::*;
#[cfg(debug_assertions)]
use std::fs::{self, OpenOptions};
#[cfg(debug_assertions)]
use std::io::Write;
#[cfg(debug_assertions)]
use std::path::Path;

#[derive(GodotClass)]
#[class(base=Object)]
pub struct GodotThreadPrint {
    base: Base<Object>,
}

#[godot_api]
impl IObject for GodotThreadPrint {
    fn init(base: Base<Object>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl GodotThreadPrint {
    #[cfg(debug_assertions)]
    pub fn print(message: String) {
        let path = Path::new(r"C:\godot_whisper\debug.txt");

        // Ensure the directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        // Open file for append (create if it does not exist)
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();

        // Write the content (with newline if desired)
        file.write_all(format!("{}\n", message).as_bytes()).unwrap();
    }

    #[cfg(not(debug_assertions))]
    pub fn print(_message: String) {}
}
