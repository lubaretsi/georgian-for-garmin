//! Opt-in wire trace on stderr: set `GEORGIAN_FOR_GARMIN_TRACE=1`.

use std::sync::LazyLock;
use std::time::Instant;

pub const TRACE_ENV: &str = "GEORGIAN_FOR_GARMIN_TRACE";

static START: LazyLock<Instant> = LazyLock::new(Instant::now);
static ENABLED: LazyLock<bool> = LazyLock::new(|| {
    LazyLock::force(&START);
    std::env::var_os(TRACE_ENV).is_some_and(|v| !v.is_empty() && v != "0")
});

pub fn enabled() -> bool {
    *ENABLED
}

#[doc(hidden)]
pub fn emit(args: std::fmt::Arguments<'_>) {
    eprintln!("[trace {:9.3}s] {args}", START.elapsed().as_secs_f64());
}

/// `trace!("…", args)`: one stderr line when tracing is enabled.
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        if $crate::trace::enabled() {
            $crate::trace::emit(format_args!($($arg)*));
        }
    };
}
