//! Opt-in wire trace on stderr: set `GEORGIAN_FOR_GARMIN_TRACE=1`. With the `dev` feature, a
//! session's `mtp_trace` also sends every PTP transaction it makes to a [`Sink`].

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

#[cfg(feature = "dev")]
pub use live::{Installed, Sink, Transaction, install, live, transaction};

/// The live trace: per thread, since a session runs on one thread and the backend sends its
/// transactions there.
#[cfg(feature = "dev")]
mod live {
    use std::cell::RefCell;
    use std::sync::Arc;

    use serde::Serialize;

    /// One PTP transaction, as the live trace sends it.
    #[derive(Debug, Clone, PartialEq, Serialize)]
    pub struct Transaction {
        pub opcode: u16,
        /// Its PTP or MTP name; null for one this program doesn't name.
        pub operation: Option<&'static str>,
        pub params: Vec<u32>,
        /// Null when no response came; `error` says why.
        pub response_code: Option<u16>,
        /// The response code's name; null for one this program doesn't name.
        pub response: Option<&'static str>,
        pub response_params: Vec<u32>,
        pub data_out_bytes: u64,
        pub data_in_bytes: u64,
        pub duration_ms: f64,
        /// A failed transfer or unparsable reply.
        pub error: Option<String>,
    }

    /// Where a session's transactions go while its trace is on.
    pub type Sink = Arc<dyn Fn(&Transaction) + Send + Sync>;

    thread_local! {
        static SINK: RefCell<Option<Sink>> = const { RefCell::new(None) };
    }

    /// Sends this thread's transactions to `sink` until the guard drops.
    pub fn install(sink: Sink) -> Installed {
        let previous = SINK.with(|s| s.borrow_mut().replace(sink));
        Installed(previous)
    }

    /// Puts back the sink [`install`] replaced.
    pub struct Installed(Option<Sink>);

    impl Drop for Installed {
        fn drop(&mut self) {
            let previous = self.0.take();
            SINK.with(|s| *s.borrow_mut() = previous);
        }
    }

    /// Whether this thread's transactions are traced: a backend measures them only then.
    pub fn live() -> bool {
        SINK.with(|s| s.borrow().is_some())
    }

    /// Sends `t` to this thread's sink, if there is one.
    pub fn transaction(t: Transaction) {
        let sink = SINK.with(|s| s.borrow().clone());
        if let Some(sink) = sink {
            sink(&t);
        }
    }
}
