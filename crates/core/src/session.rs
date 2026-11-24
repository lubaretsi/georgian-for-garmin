//! Running operations: one call at a time on a [`Session`], which keeps the watch open between
//! calls (`rpc` and `mcp`) or opens it for one call ([`crate::ops::call`]). A session reopens
//! the watch when its connection stopped answering, as after an unplug and replug, and answers
//! `no_device` while there is none. [`Worker`] runs a session on its own thread, the one the
//! watch is opened on, for adapters that read requests while an operation runs.

use std::borrow::Cow;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::diagnose::{self, DiagnoseInput, Probe};
use crate::error::{Error, ErrorCode};
use crate::fonts::FontSet;
use crate::garmin_mode::{SwitchInput, SwitchOutput};
use crate::listing;
use crate::mtp::Mtp;
use crate::ops::{
    self, Connect, InspectDriveInput, InspectInput, Variant, parse, stage, stage_parts, to_value,
};
use crate::report::{self, IssueLinkInput};

/// One step of a long operation, for `notifications/progress`: `progress` grows with every
/// step; `total` is known for file counts and timeouts.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub progress: f64,
    pub total: Option<f64>,
    pub message: String,
}

/// What a caller sees of a running operation and how it stops it: a progress sink and a
/// cancellation flag, both optional, and with the `dev` feature where the session's live trace
/// goes while it is on.
#[derive(Clone, Copy)]
pub struct Control<'a> {
    progress: Option<&'a (dyn Fn(Progress) + 'a)>,
    cancel: Option<&'a AtomicBool>,
    #[cfg(feature = "dev")]
    trace: Option<&'a crate::trace::Sink>,
}

impl<'a> Control<'a> {
    /// No progress, no cancellation: one-shot calls.
    pub const NONE: Control<'static> = Control {
        progress: None,
        cancel: None,
        #[cfg(feature = "dev")]
        trace: None,
    };

    pub fn new(
        progress: Option<&'a (dyn Fn(Progress) + 'a)>,
        cancel: Option<&'a AtomicBool>,
    ) -> Self {
        Control {
            progress,
            cancel,
            #[cfg(feature = "dev")]
            trace: None,
        }
    }

    /// Where this call's PTP transactions go while the session's trace (`mtp_trace`) is on.
    #[cfg(feature = "dev")]
    pub fn with_trace(mut self, trace: Option<&'a crate::trace::Sink>) -> Self {
        self.trace = trace;
        self
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|c| c.load(Ordering::SeqCst))
    }

    /// `cancelled` before anything was written.
    pub(crate) fn check_nothing_written(&self) -> Result<(), Error> {
        if self.cancelled() {
            Err(Error::new(
                ErrorCode::Cancelled,
                "Cancelled; nothing was written.",
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn progress(&self, done: usize, total: usize, message: &str) {
        self.report(done as f64, Some(total as f64), message);
    }

    pub(crate) fn report(&self, progress: f64, total: Option<f64>, message: &str) {
        if let Some(sink) = self.progress {
            sink(Progress {
                progress,
                total,
                message: message.to_owned(),
            });
        }
    }
}

/// SHA-256s of files read in this connection, by handle, name, size and modification date.
/// Cleared whenever the watch is opened again: handles can be reused after a restart.
pub(crate) type HashCache = HashMap<(u32, String, u32, String), String>;

/// `wait_for_watch`'s input.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaitInput {
    /// Seconds to wait at most; 0 checks once. Default 60.
    #[schemars(range(min = 0, max = 3600))]
    pub timeout_seconds: Option<f64>,
}

/// The watch that could be opened.
#[derive(Debug, Serialize, JsonSchema)]
pub struct WaitOutput {
    /// The model name the watch gives over MTP, e.g. "Venu 2".
    pub model: String,
    /// Seconds this call waited.
    pub waited_seconds: f64,
}

const DEFAULT_WAIT: f64 = 60.0;
const POLL: Duration = Duration::from_millis(500);

/// Switches a watch in Garmin mode to MTP: [`crate::switch_to_mtp`], or a fake in the tests.
pub type Switch<'a> = dyn FnMut(&Control<'_>) -> Result<SwitchOutput, Error> + 'a;

/// Runs operations against one watch.
pub struct Session<'a> {
    fonts: Cow<'a, FontSet>,
    watch: Watch<'a>,
    /// Where `diagnose` lists USB devices and volumes.
    probe: Probe<'a>,
    /// How `switch_to_mtp` switches a watch in Garmin mode.
    switch: Box<Switch<'a>>,
    #[cfg(feature = "dev")]
    staged_log: Option<std::path::PathBuf>,
    /// Whether `mtp_trace` turned the live trace on.
    #[cfg(feature = "dev")]
    tracing: bool,
}

/// The watch connection of a [`Session`].
struct Watch<'a> {
    connect: Box<Connect<'a>>,
    dev: Option<Box<dyn Mtp>>,
    keep_open: bool,
    poll: Duration,
    hashes: HashCache,
}

impl<'a> Session<'a> {
    /// A session that keeps the watch open between calls.
    pub fn new(fonts: Cow<'a, FontSet>, connect: Box<Connect<'a>>) -> Self {
        Session {
            fonts,
            watch: Watch {
                connect,
                dev: None,
                keep_open: true,
                poll: POLL,
                hashes: HashCache::new(),
            },
            probe: Probe::system(),
            switch: Box::new(crate::switch_to_mtp),
            #[cfg(feature = "dev")]
            staged_log: crate::dev::default_log(),
            #[cfg(feature = "dev")]
            tracing: false,
        }
    }

    /// A session that opens the watch for each call and closes it afterwards, so other
    /// programs (and the app's next poll) can use it in between.
    pub fn one_shot(fonts: Cow<'a, FontSet>, connect: Box<Connect<'a>>) -> Self {
        let mut session = Session::new(fonts, connect);
        session.watch.keep_open = false;
        session
    }

    /// How often `wait_for_watch` tries to open the watch.
    pub fn with_poll_interval(mut self, poll: Duration) -> Self {
        self.watch.poll = poll;
        self
    }

    /// Where `diagnose` lists USB devices and volumes, instead of this computer.
    pub fn with_probe(mut self, probe: Probe<'a>) -> Self {
        self.probe = probe;
        self
    }

    /// How `switch_to_mtp` switches a watch in Garmin mode, instead of this computer's USB.
    pub fn with_switch(mut self, switch: Box<Switch<'a>>) -> Self {
        self.switch = switch;
        self
    }

    /// Where `stage_file` logs the ids it hands out; `remove_staged` reads it.
    #[cfg(feature = "dev")]
    pub fn with_staged_log(mut self, path: std::path::PathBuf) -> Self {
        self.staged_log = Some(path);
        self
    }

    /// Runs operation `name` with JSON `input`. Input is validated before the watch is touched;
    /// output is validated before it is returned. While the live trace is on, every PTP
    /// transaction of the call goes to `control`'s trace sink.
    pub fn call(
        &mut self,
        name: &str,
        input: Value,
        control: &Control<'_>,
    ) -> Result<Value, Error> {
        let op = ops::known(name)?;
        ops::validate_input(op, &input)?;
        #[cfg(feature = "dev")]
        let _trace = control
            .trace
            .filter(|_| self.tracing)
            .map(|sink| crate::trace::install(sink.clone()));
        let output = self.dispatch(name, input, control);
        if !self.watch.keep_open {
            self.watch.dev = None;
        }
        let output = output?;
        ops::validate_output(op, &output)?;
        Ok(output)
    }

    fn dispatch(
        &mut self,
        name: &str,
        input: Value,
        control: &Control<'_>,
    ) -> Result<Value, Error> {
        let fonts: &FontSet = &self.fonts;
        let watch = &mut self.watch;
        match name {
            "inspect" => {
                let InspectInput {} = parse(input)?;
                to_value(watch.run(|dev, _| ops::inspect(dev, fonts))?)
            }
            "install" | "restore_stock" => {
                let parts = stage_parts(input, fonts)?;
                let variant = if name == "install" {
                    Variant::Georgian
                } else {
                    Variant::Stock
                };
                to_value(watch.run(|dev, _| stage(dev, fonts, variant, parts.as_ref(), control))?)
            }
            "inspect_drive" => {
                let InspectDriveInput { path } = parse(input)?;
                to_value(ops::inspect_drive(path.as_deref(), fonts)?)
            }
            "issue_link" => {
                let input: IssueLinkInput = parse(input)?;
                to_value(report::issue_link(&input)?)
            }
            "extdata_list" => {
                let input: listing::ExtdataListInput = parse(input)?;
                to_value(watch.run(|dev, hashes| listing::extdata_list(dev, &input, hashes))?)
            }
            "wait_for_watch" => {
                let WaitInput { timeout_seconds } = parse(input)?;
                to_value(watch.wait(timeout_seconds.unwrap_or(DEFAULT_WAIT), control)?)
            }
            "diagnose" => {
                let DiagnoseInput { events } = parse(input)?;
                let events = events.unwrap_or_default();
                diagnose::check_events(&events)?;
                // Listed before the watch is opened, so a holder is another program's unless
                // this session already holds the watch. The watch goes through the session's
                // connection, as for inspect: a second open would find it busy.
                let usb = (self.probe.usb)();
                let drives = (self.probe.drives)();
                let connection = watch.run(|dev, _| ops::inspect(dev, fonts));
                to_value(diagnose::diagnose(fonts, events, usb, drives, connection))
            }
            "switch_to_mtp" => {
                let SwitchInput {} = parse(input)?;
                to_value((self.switch)(control)?)
            }
            #[cfg(feature = "dev")]
            "read_file" => {
                let input: crate::dev::ReadFileInput = parse(input)?;
                to_value(watch.run(|dev, _| crate::dev::read_file(dev, &input))?)
            }
            #[cfg(feature = "dev")]
            "stage_file" => {
                let input: crate::dev::StageFileInput = parse(input)?;
                let log = staged_log(&self.staged_log)?;
                to_value(
                    watch.run(|dev, _| crate::dev::stage_file(dev, fonts, log, &input, control))?,
                )
            }
            #[cfg(feature = "dev")]
            "remove_staged" => {
                let input: crate::dev::RemoveStagedInput = parse(input)?;
                let log = staged_log(&self.staged_log)?;
                if let Some(done) = crate::dev::already_removed(log, &input)? {
                    return to_value(done);
                }
                to_value(watch.run(|dev, _| crate::dev::remove_staged(dev, log, &input))?)
            }
            #[cfg(feature = "dev")]
            "mtp_device_info" => {
                let input: crate::raw::DeviceInfoInput = parse(input)?;
                to_value(watch.run(|dev, _| crate::raw::device_info(dev, &input))?)
            }
            #[cfg(feature = "dev")]
            "mtp_storage_ids" => {
                let crate::raw::NoInput {} = parse(input)?;
                to_value(watch.run(|dev, _| crate::raw::storage_ids(dev))?)
            }
            #[cfg(feature = "dev")]
            "mtp_storage_info" => {
                let input: crate::raw::StorageInput = parse(input)?;
                to_value(watch.run(|dev, _| crate::raw::storage_info(dev, &input))?)
            }
            #[cfg(feature = "dev")]
            "mtp_handles" => {
                let input: crate::raw::HandlesInput = parse(input)?;
                to_value(watch.run(|dev, _| crate::raw::handles(dev, &input))?)
            }
            #[cfg(feature = "dev")]
            "mtp_object_info" => {
                let input: crate::raw::HandleInput = parse(input)?;
                to_value(watch.run(|dev, _| crate::raw::object_info(dev, &input))?)
            }
            #[cfg(feature = "dev")]
            "mtp_read" => {
                let input: crate::raw::ReadInput = parse(input)?;
                to_value(watch.run(|dev, _| crate::raw::read(dev, &input))?)
            }
            #[cfg(feature = "dev")]
            "ptp_transaction" => {
                let input: crate::raw::PtpTransactionInput = parse(input)?;
                crate::raw::check_read(&input)?;
                to_value(watch.run(|dev, _| crate::raw::ptp_transaction(dev, &input))?)
            }
            #[cfg(feature = "dev")]
            "mtp_create" => {
                let input: crate::dev::MtpCreateInput = parse(input)?;
                crate::dev::check_allowed(name, input.allow_write)?;
                let log = staged_log(&self.staged_log)?;
                to_value(
                    watch.run(|dev, _| crate::dev::mtp_create(dev, fonts, log, &input, control))?,
                )
            }
            #[cfg(feature = "dev")]
            "mtp_delete" => {
                let input: crate::dev::MtpDeleteInput = parse(input)?;
                crate::dev::check_allowed(name, input.allow_write)?;
                let log = staged_log(&self.staged_log)?;
                to_value(
                    watch.run(|dev, _| crate::dev::mtp_delete(dev, fonts, log, &input, control))?,
                )
            }
            #[cfg(feature = "dev")]
            "mtp_trace" => {
                let crate::raw::TraceInput { enabled } = parse(input)?;
                self.tracing = enabled;
                to_value(crate::raw::TraceOutput { enabled })
            }
            _ => Err(Error::new(
                ErrorCode::Internal,
                format!("operation {name} has no implementation"),
            )),
        }
    }
}

#[cfg(feature = "dev")]
fn staged_log(log: &Option<std::path::PathBuf>) -> Result<&std::path::Path, Error> {
    log.as_deref().ok_or_else(|| {
        Error::new(
            ErrorCode::Internal,
            "No folder for the staged-file log: set GEORGIAN_FOR_GARMIN_STATE_DIR.",
        )
    })
}

/// Whether a held connection still reaches the watch. `GetStorageIDs` goes to the watch every
/// time; `device_info` doesn't: both backends keep the answer from when the session opened.
fn answers(dev: &mut dyn Mtp) -> bool {
    dev.storage_ids().is_ok()
}

impl Watch<'_> {
    /// Runs `f` on the watch: the held connection if it still answers, else a new one. A
    /// transfer failure or a vanished watch drops the connection, so the next call opens it
    /// again.
    fn run<T>(
        &mut self,
        f: impl FnOnce(&mut dyn Mtp, &mut HashCache) -> Result<T, Error>,
    ) -> Result<T, Error> {
        if let Some(dev) = &mut self.dev
            && !answers(dev.as_mut())
        {
            self.dev = None;
        }
        let dev = match &mut self.dev {
            Some(dev) => dev,
            None => {
                self.hashes.clear();
                self.dev.insert((self.connect)()?)
            }
        };
        let result = f(dev.as_mut(), &mut self.hashes);
        if let Err(e) = &result
            && matches!(e.code, ErrorCode::DeviceError | ErrorCode::NoDevice)
        {
            self.dev = None;
        }
        result
    }

    /// Tries to open the watch every poll interval until it opens, `timeout` seconds pass, or
    /// the call is cancelled. Only `no_device` and `device_error` are waited out.
    fn wait(&mut self, timeout: f64, control: &Control<'_>) -> Result<WaitOutput, Error> {
        let start = Instant::now();
        let limit = Duration::from_secs_f64(timeout);
        loop {
            if let Some(dev) = &mut self.dev {
                if answers(dev.as_mut()) {
                    return Ok(WaitOutput {
                        model: dev.device_info()?.model,
                        waited_seconds: start.elapsed().as_secs_f64(),
                    });
                }
                self.dev = None;
            }
            let last = match (self.connect)() {
                Ok(mut dev) => {
                    let info = dev.device_info()?;
                    self.hashes.clear();
                    if self.keep_open {
                        self.dev = Some(dev);
                    }
                    return Ok(WaitOutput {
                        model: info.model,
                        waited_seconds: start.elapsed().as_secs_f64(),
                    });
                }
                Err(e) if matches!(e.code, ErrorCode::NoDevice | ErrorCode::DeviceError) => e,
                Err(e) => return Err(e),
            };
            let waited = start.elapsed();
            if waited >= limit {
                return Err(Error::new(
                    last.code,
                    format!(
                        "No watch could be opened within {timeout} s: {}",
                        last.message
                    ),
                ));
            }
            control.report(
                waited.as_secs_f64(),
                Some(timeout),
                &format!("Waiting for the watch ({})", last.code.as_str()),
            );
            let until = (Instant::now() + self.poll).min(start + limit);
            loop {
                if control.cancelled() {
                    return Err(Error::new(
                        ErrorCode::Cancelled,
                        "Cancelled while waiting for the watch.",
                    ));
                }
                let now = Instant::now();
                if now >= until {
                    break;
                }
                std::thread::sleep((until - now).min(Duration::from_millis(20)));
            }
        }
    }
}

/// One call for a [`Worker`].
pub struct Job {
    pub name: String,
    pub input: Value,
    /// Where progress goes, when the caller asked for it.
    pub progress: Option<Box<dyn Fn(Progress) + Send>>,
    /// Set to cancel the call, before it starts or while it runs.
    pub cancel: Arc<AtomicBool>,
    /// Where the call's PTP transactions go while the session's trace is on (`mtp_trace`).
    #[cfg(feature = "dev")]
    pub trace: Option<crate::trace::Sink>,
    /// Receives the result, on the worker's thread.
    pub done: Box<dyn FnOnce(Result<Value, Error>) + Send>,
}

/// A session on its own thread, running jobs one at a time in the order they come. The thread
/// also opens the watch, so a backend bound to one thread (Windows' COM) keeps it.
pub struct Worker {
    jobs: Option<mpsc::Sender<Job>>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    /// Starts the thread; `open` runs on it and makes the session.
    pub fn spawn<F>(open: F) -> Worker
    where
        F: FnOnce() -> Session<'static> + Send + 'static,
    {
        let (jobs, queue) = mpsc::channel::<Job>();
        let thread = std::thread::spawn(move || {
            let mut session = open();
            for job in queue {
                let result = if job.cancel.load(Ordering::SeqCst) {
                    Err(Error::new(
                        ErrorCode::Cancelled,
                        "Cancelled before it started; nothing was written.",
                    ))
                } else {
                    let progress = job.progress.as_deref().map(|p| p as &dyn Fn(Progress));
                    let control = Control::new(progress, Some(&job.cancel));
                    #[cfg(feature = "dev")]
                    let control = control.with_trace(job.trace.as_ref());
                    let name = job.name.clone();
                    catch_unwind(AssertUnwindSafe(|| {
                        session.call(&name, job.input, &control)
                    }))
                    .unwrap_or_else(|_| {
                        session.watch.dev = None;
                        Err(Error::new(
                            ErrorCode::Internal,
                            format!("{name} panicked; the watch connection was closed"),
                        ))
                    })
                };
                (job.done)(result);
            }
        });
        Worker {
            jobs: Some(jobs),
            thread: Some(thread),
        }
    }

    /// Queues `job`. If the worker is gone, `job.done` gets an `internal` error at once.
    pub fn submit(&self, job: Job) {
        let sent = match &self.jobs {
            Some(jobs) => jobs.send(job).map_err(|e| e.0),
            None => Err(job),
        };
        if let Err(job) = sent {
            (job.done)(Err(Error::new(
                ErrorCode::Internal,
                "the session thread has stopped",
            )));
        }
    }

    /// Runs the queued jobs to the end, then closes the watch.
    pub fn finish(mut self) {
        self.stop();
    }

    fn stop(&mut self) {
        self.jobs = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
    }
}
