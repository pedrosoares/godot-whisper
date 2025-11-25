use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};

static RUNTIME: OnceLock<Mutex<Runtime>> = OnceLock::new();

fn get_runtime() -> &'static Mutex<Runtime> {
    RUNTIME.get_or_init(|| {
        Mutex::new(Runtime {
            running: Arc::new(AtomicBool::new(false)),
        })
    })
}

pub struct Runtime {
    running: Arc<AtomicBool>,
}

impl Runtime {
    pub fn singleton() -> &'static Mutex<Runtime> {
        return get_runtime();
    }

    pub fn running() -> Arc<AtomicBool> {
        if let Ok(runtime) = get_runtime().lock() {
            return runtime.running.clone();
        }
        panic!("Something went wrong");
    }

    pub fn free() {
        if let Ok(runtime) = get_runtime().lock() {
            runtime.running.store(false, Ordering::Relaxed);
        }
    }
}
