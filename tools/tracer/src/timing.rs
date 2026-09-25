use std::sync::OnceLock;
use std::time::Instant;

static ENABLED: OnceLock<bool> = OnceLock::new();

pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| std::env::var_os("TRACE_TIMING").is_some_and(|value| value == "1"))
}

pub fn phase<T>(name: &str, call: impl FnOnce() -> T) -> T {
    if !enabled() {
        return call();
    }
    let started = Instant::now();
    let value = call();
    eprintln!("timing {name} {}", started.elapsed().as_micros());
    value
}

pub fn start() -> Option<Instant> {
    enabled().then(Instant::now)
}

pub fn total(started: Option<Instant>) {
    if let Some(started) = started {
        eprintln!("timing total {}", started.elapsed().as_micros());
    }
}
