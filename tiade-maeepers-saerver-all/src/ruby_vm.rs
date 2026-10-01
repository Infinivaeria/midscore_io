//! In-process Ruby VM embedded with Magnus.
//!
//! Ruby's VM is single-threaded from the embedder's point of view: it must be
//! initialised once, and every Ruby call must run on the thread that
//! initialised it. This module owns that thread and exposes a job queue, so
//! async Tide handlers and the CLI thread can evaluate Ruby without blocking
//! the executor or touching Ruby from the wrong thread.
//!
//! Definitions (classes, modules, top-level methods) persist between
//! evaluations, so a prelude loaded at startup is visible to later snippets.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;
use std::sync::mpsc;
use std::time::Duration;

use magnus::{RString, Ruby, Value, value::ReprValue};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

// Native stack for the Ruby thread; Ruby code (and deep recursion in it)
// needs far more than Rust's 2 MiB default.
const RUBY_THREAD_STACK_SIZE: usize = 64 * 1024 * 1024;

type Job = Box<dyn FnOnce(&Ruby) + Send + 'static>;

#[derive(Debug)]
pub enum RubyError {
    Unavailable(String),
    Timeout(Duration),
    /// Ruby raised; holds the exception message.
    Raised(String),
}

impl std::fmt::Display for RubyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RubyError::Unavailable(message) | RubyError::Raised(message) => f.write_str(message),
            RubyError::Timeout(timeout) => {
                write!(f, "Ruby evaluation timed out after {}s", timeout.as_secs())
            }
        }
    }
}

impl std::error::Error for RubyError {}

static JOBS: OnceLock<Result<mpsc::Sender<Job>, String>> = OnceLock::new();

/// Starts the Ruby VM thread and waits until the VM is initialised.
///
/// Call this early in `main`, before installing Rust signal handlers such as
/// `ctrlc`, because Ruby installs its own handlers during initialisation and
/// this function resets them to the system defaults.
pub fn start() -> Result<(), String> {
    // `get_or_init` serialises concurrent callers, so Ruby is initialised at most once.
    JOBS.get_or_init(spawn_vm).as_ref().map(|_| ()).map_err(Clone::clone)
}

fn spawn_vm() -> Result<mpsc::Sender<Job>, String> {
    let (job_tx, job_rx) = mpsc::channel::<Job>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<String, String>>();

    std::thread::Builder::new()
        .name("ruby-vm".into())
        .stack_size(RUBY_THREAD_STACK_SIZE)
        .spawn(move || {
            // SAFETY: `init` is called once, at the top of this thread, and
            // every Ruby call happens further down this same stack. `_cleanup`
            // lives until the thread exits, after all Ruby work has finished.
            let _cleanup = unsafe { magnus::embed::init() };
            let ruby = match Ruby::get() {
                Ok(ruby) => ruby,
                Err(error) => {
                    let _ = ready_tx.send(Err(format!("Ruby unavailable after init: {error}")));
                    return;
                }
            };

            // Hand process signals back to the host so `ctrlc`, `stop-server.sh`
            // (SIGTERM) and the `killall -HUP` restart keep their pre-Ruby behaviour.
            let restore_signals = r##"
                %w[INT TERM HUP QUIT USR1 USR2 ALRM].each do |sig|
                  trap(sig, 'SYSTEM_DEFAULT') if Signal.list.key?(sig)
                end
                RUBY_DESCRIPTION
            "##;
            let ready = ruby
                .eval::<Value>(restore_signals)
                .and_then(value_to_string)
                .map_err(|error| format!("Ruby signal setup failed: {error}"));
            let failed = ready.is_err();
            let _ = ready_tx.send(ready);
            if failed {
                return;
            }

            while let Ok(job) = job_rx.recv() {
                if catch_unwind(AssertUnwindSafe(|| job(&ruby))).is_err() {
                    eprintln!("ruby-vm: a job panicked; the VM thread is still running");
                }
            }
        })
        .map_err(|error| format!("failed to spawn ruby-vm thread: {error}"))?;

    match ready_rx.recv() {
        Ok(Ok(description)) => {
            println!("Embedded Ruby VM ready: {description}");
            Ok(job_tx)
        }
        Ok(Err(error)) => Err(error),
        Err(_) => Err("ruby-vm thread exited during initialisation".into()),
    }
}

fn value_to_string(value: Value) -> Result<String, magnus::Error> {
    let string: RString = value.funcall("to_s", ())?;
    // SAFETY: the bytes are copied out before any further Ruby call can run GC.
    Ok(String::from_utf8_lossy(unsafe { string.as_slice() }).into_owned())
}

// `rb_eval_string` takes a C string with no encoding, which would make every
// literal ASCII-8BIT; the magic comment gives eval'd code the same UTF-8 source
// encoding as a normal .rb file.
fn eval_utf8(ruby: &Ruby, code: &str) -> Result<Value, magnus::Error> {
    ruby.eval(&format!("# encoding: utf-8\n{code}"))
}

fn submit(code: String) -> Result<async_std::channel::Receiver<Result<String, RubyError>>, RubyError> {
    submit_with(move |ruby| eval_utf8(ruby, &code))
}

fn submit_with<F>(work: F) -> Result<async_std::channel::Receiver<Result<String, RubyError>>, RubyError>
where
    F: FnOnce(&Ruby) -> Result<Value, magnus::Error> + Send + 'static,
{
    let jobs = match JOBS.get() {
        Some(Ok(jobs)) => jobs,
        Some(Err(error)) => return Err(RubyError::Unavailable(error.clone())),
        None => return Err(RubyError::Unavailable("embedded Ruby VM is not running".into())),
    };
    let (result_tx, result_rx) = async_std::channel::bounded(1);
    let job: Job = Box::new(move |ruby: &Ruby| {
        let result = work(ruby)
            .and_then(value_to_string)
            .map_err(|error| RubyError::Raised(error.to_string()));
        let _ = result_tx.try_send(result);
    });
    jobs.send(job)
        .map_err(|_| RubyError::Unavailable("embedded Ruby VM has stopped".into()))?;
    Ok(result_rx)
}

async fn wait(
    result_rx: async_std::channel::Receiver<Result<String, RubyError>>,
    timeout: Duration,
) -> Result<String, RubyError> {
    match async_std::future::timeout(timeout, result_rx.recv()).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(RubyError::Unavailable("embedded Ruby VM dropped the job".into())),
        Err(_) => Err(RubyError::Timeout(timeout)),
    }
}

/// Evaluates `callable` (Ruby source returning something that responds to
/// `call`) and calls it with one Array of UTF-8 strings. Untrusted input goes
/// in `args`, never into the source, so it cannot change the code being run.
pub async fn call_with_strings(
    callable: impl Into<String>,
    args: Vec<String>,
    timeout: Duration,
) -> Result<String, RubyError> {
    let callable = callable.into();
    let result_rx = submit_with(move |ruby| {
        let target = eval_utf8(ruby, &callable)?;
        let array = ruby.ary_new_capa(args.len());
        for arg in &args {
            array.push(ruby.str_new(arg))?;
        }
        target.funcall("call", (array,))
    })?;
    wait(result_rx, timeout).await
}

/// Evaluates Ruby source on the VM thread and returns the result's `to_s`.
pub async fn eval(code: impl Into<String>) -> Result<String, RubyError> {
    eval_with_timeout(code, DEFAULT_TIMEOUT).await
}

pub async fn eval_with_timeout(
    code: impl Into<String>,
    timeout: Duration,
) -> Result<String, RubyError> {
    let result_rx = submit(code.into())?;
    wait(result_rx, timeout).await
}

/// Blocking variant for non-async callers such as startup code and the CLI thread.
pub fn eval_blocking(code: impl Into<String>) -> Result<String, RubyError> {
    async_std::task::block_on(eval(code))
}

/// Evaluates `code` and wraps the outcome in a `text/plain` Tide response.
pub async fn text_response(code: &str) -> tide::Result {
    let (status, body) = match eval(code).await {
        Ok(output) => (tide::StatusCode::Ok, output),
        Err(error @ RubyError::Timeout(_)) => (tide::StatusCode::GatewayTimeout, error.to_string()),
        Err(error) => (tide::StatusCode::InternalServerError, format!("Error: {error}")),
    };
    let mut res = tide::Response::new(status);
    res.set_body(body);
    res.insert_header("Content-Type", "text/plain; charset=utf-8");
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ruby can only be initialised once per process, so all checks share one test.
    #[test]
    fn embedded_vm_round_trip() {
        start().expect("Ruby VM should start");
        start().expect("start is idempotent");

        assert_eq!(eval_blocking("1 + 1").unwrap(), "2");

        eval_blocking("class TiadeProbe; def self.hi = 'hi'; end").unwrap();
        assert_eq!(eval_blocking("TiadeProbe.hi").unwrap(), "hi");

        match eval_blocking("raise ArgumentError, 'boom'") {
            Err(RubyError::Raised(message)) => assert!(message.contains("boom"), "{message}"),
            other => panic!("expected Raised, got {other:?}"),
        }
        assert_eq!(eval_blocking("2 * 21").unwrap(), "42", "VM survives exceptions");

        eval_blocking("def tiade_moon_glyph = '\u{1F315} full'").unwrap();
        assert_eq!(eval_blocking("tiade_moon_glyph").unwrap(), "\u{1F315} full");
        assert_eq!(eval_blocking("'é'.encoding").unwrap(), "UTF-8");

        assert_eq!(
            eval_blocking("trap('HUP', 'SYSTEM_DEFAULT').to_s").unwrap(),
            "SYSTEM_DEFAULT",
            "Ruby must not own SIGHUP"
        );

        let fast = async_std::task::block_on(eval_with_timeout("sleep 2; :late", Duration::from_millis(100)));
        assert!(matches!(fast, Err(RubyError::Timeout(_))), "{fast:?}");

        let res = async_std::task::block_on(text_response("'ok'")).unwrap();
        assert_eq!(res.status(), tide::StatusCode::Ok);
    }
}
