//! Ruby evaluation routes for Second Life, backed by the embedded Magnus VM.
//!
//! `POST /sl/ruby/eval` runs Ruby in a per-session binding, so local
//! variables persist between calls from the same object (or named session).
//! `POST /sl/ruby/reset` drops a session's binding.
//!
//! This is remote code execution with the server's privileges, so the routes
//! are disabled unless `TIADE_RUBY_EVAL_TOKEN` is set, and every request must
//! present that token. `TIADE_RUBY_EVAL_SL_OWNERS` (comma-separated avatar
//! keys) additionally restricts callers by `X-SecondLife-Owner-Key`; that
//! header is only trustworthy for requests that really come from Second Life,
//! so it is a second check, never a replacement for the token.

use std::{sync::{Arc, OnceLock}, time::Duration};

use magnus::{Object, Ruby, function};
use serde_json::Value;

use crate::ruby_vm::{self, RubyError};

const TOKEN_ENV: &str = "TIADE_RUBY_EVAL_TOKEN";
const OWNERS_ENV: &str = "TIADE_RUBY_EVAL_SL_OWNERS";
const SECONDS_ENV: &str = "TIADE_RUBY_EVAL_SECONDS";
const DEFAULT_SECONDS: u64 = 10;
const MAX_SESSION_CHARS: usize = 128;

pub type StoreCall = Arc<dyn Fn(&str, &str, Value) -> Result<Value, String> + Send + Sync>;
static STORE_CALL: OnceLock<StoreCall> = OnceLock::new();

fn host_call(session: String, operation: String, args: String) -> Result<String, magnus::Error> {
    let ruby = Ruby::get().expect("Ruby host_call runs only on the VM thread");
    let fail = |message: String| magnus::Error::new(ruby.exception_runtime_error(), message);
    let args = serde_json::from_str(&args).map_err(|error| fail(error.to_string()))?;
    let callback = STORE_CALL.get().ok_or_else(|| fail("Ruby store is unavailable".into()))?;
    let result = callback(&session, &operation, args).map_err(fail)?;
    serde_json::to_string(&result).map_err(|error| fail(error.to_string()))
}

fn install_store(ruby: &Ruby) -> Result<(), magnus::Error> {
    let module = ruby.define_module("TiadeSL")?;
    module.define_singleton_method("host_call", function!(host_call, 3))
}

// Ruby side: per-session bindings, stdout capture and a Ruby-level timeout so
// a runaway snippet cannot hold the shared VM thread. Input arrives as an
// Array of strings via `ruby_vm::call_with_strings`, never spliced into source.
const PRELUDE: &str = r###"
require 'stringio'
require 'timeout'
require 'json'

module TiadeSL
  SESSIONS = {}
  MAX_SESSIONS = 256

  module Storage
    def store_call(operation, args = {})
      JSON.parse(TiadeSL.host_call(@tiade_session, operation, JSON.generate(args)))
    end

    def var_set(name, value) = store_call('var_set', {name: name, value: value})
    def var_get(name) = store_call('var_get', {name: name})
    def var_delete(name) = store_call('var_delete', {name: name})
    def var_view = store_call('var_view')
    def file_write(name, content) = store_call('file_write', {name: name, content: content})
    def file_read(name) = store_call('file_read', {name: name})
    def file_list = store_call('file_list')
    def file_delete(name) = store_call('file_delete', {name: name})
    def matrix_get(name) = store_call('matrix_get', {name: name})
  end

  def self.binding_for(session)
    unless SESSIONS.key?(session)
      SESSIONS.shift if SESSIONS.size >= MAX_SESSIONS
      context = Object.new.extend(Storage)
      context.instance_variable_set(:@tiade_session, session)
      SESSIONS[session] = context.instance_eval { binding }
    end
    SESSIONS[session]
  end

  # Returns "OK\n<output>" or "ERR\n<output>".
  EVAL = lambda do |(session, code, seconds, include_result)|
    out = StringIO.new
    saved = $stdout
    status = 'OK'
    begin
      $stdout = out
      value = Timeout.timeout(seconds.to_f) do
        binding_for(session).eval(code, "(sl:#{session})", 1)
      end
      text = include_result == 'true' ? "#{out.string}=> #{value.inspect}" : out.string
    rescue Timeout::Error
      status = 'ERR'
      text = "#{out.string}Timeout: evaluation exceeded #{seconds}s"
    rescue Exception => e
      status = 'ERR'
      text = "#{out.string}#{e.class}: #{e.message}"
    ensure
      $stdout = saved
    end
    "#{status}\n#{text}"
  end

  RESET = lambda do |(session)|
    SESSIONS.delete(session) ? "session #{session} reset" : "session #{session} was empty"
  end
end
:tiade_sl_ruby_ready
"###;

pub fn mount<State: Clone + Send + Sync + 'static>(
    app: &mut tide::Server<State>,
    store_call: StoreCall,
) -> Result<(), String> {
    STORE_CALL.set(store_call).map_err(|_| "Ruby store already registered".to_string())?;
    if let Err(error) = ruby_vm::install(install_store) {
        eprintln!("Second Life Ruby routes: Ruby store installation failed: {error}");
    }
    if let Err(error) = ruby_vm::eval_blocking(PRELUDE) {
        eprintln!("Second Life Ruby routes: prelude failed to load: {error}");
    }
    if configured_token().is_none() {
        println!("Second Life Ruby routes are disabled; set {TOKEN_ENV} to enable /sl/ruby/*");
    }

    app.at("/sl/ruby/eval").post(|mut req: tide::Request<State>| async move {
        let body = req.body_string().await.unwrap_or_default();
        let input = match SlRubyRequest::parse(&req, &body) {
            Ok(input) => input,
            Err(response) => return Ok(response),
        };
        if input.code.trim().is_empty() {
            return Ok(text(tide::StatusCode::BadRequest, "code is required"));
        }
        let seconds = eval_seconds();
        let result = ruby_vm::call_with_strings(
            "TiadeSL::EVAL",
            vec![input.session, input.code, seconds.to_string(), input.include_result.to_string()],
            // The Ruby-side timeout fires first; this only covers a stuck VM.
            Duration::from_secs(seconds + 5),
        )
        .await;
        Ok(match result {
            Ok(output) => match output.split_once('\n') {
                Some(("OK", rest)) => text(tide::StatusCode::Ok, rest),
                Some((_, rest)) => text(tide::StatusCode::UnprocessableEntity, rest),
                None => text(tide::StatusCode::Ok, &output),
            },
            Err(error) => ruby_error_response(error),
        })
    });

    app.at("/sl/ruby/reset").post(|mut req: tide::Request<State>| async move {
        let body = req.body_string().await.unwrap_or_default();
        let input = match SlRubyRequest::parse(&req, &body) {
            Ok(input) => input,
            Err(response) => return Ok(response),
        };
        let result = ruby_vm::call_with_strings(
            "TiadeSL::RESET",
            vec![input.session],
            Duration::from_secs(10),
        )
        .await;
        Ok(match result {
            Ok(output) => text(tide::StatusCode::Ok, &output),
            Err(error) => ruby_error_response(error),
        })
    });
    Ok(())
}

struct SlRubyRequest {
    session: String,
    code: String,
    include_result: bool,
}

impl SlRubyRequest {
    /// Authorizes the request and reads `{"code","session","token","include_result"}` JSON or a
    /// plain-text body (code), with the token in `X-Ruby-Token` or
    /// `Authorization: Bearer`.
    fn parse<State>(req: &tide::Request<State>, body: &str) -> Result<Self, tide::Response> {
        let json = match serde_json::from_str::<Value>(body) {
            Ok(Value::Object(object)) => Some(object),
            _ => None,
        };
        let field = |name: &str| {
            json.as_ref()
                .and_then(|object| object.get(name))
                .and_then(Value::as_str)
                .map(str::to_string)
        };

        let Some(expected) = configured_token() else {
            return Err(text(
                tide::StatusCode::ServiceUnavailable,
                &format!("Second Life Ruby evaluation is disabled; set {TOKEN_ENV}"),
            ));
        };
        let header = |name: &str| req.header(name).map(|values| values.last().as_str().to_string());
        let given = header("X-Ruby-Token")
            .or_else(|| {
                header("Authorization")
                    .and_then(|value| value.strip_prefix("Bearer ").map(str::to_string))
            })
            .or_else(|| field("token"))
            .unwrap_or_default();
        if !tokens_match(&expected, given.trim()) {
            return Err(text(tide::StatusCode::Unauthorized, "invalid Ruby evaluation token"));
        }

        let owner = header("X-SecondLife-Owner-Key").unwrap_or_default();
        if !owner_allowed(std::env::var(OWNERS_ENV).ok().as_deref(), &owner) {
            return Err(text(tide::StatusCode::Forbidden, "this Second Life owner is not allowed"));
        }

        let session = field("session")
            .filter(|session| !session.trim().is_empty())
            .or_else(|| header("X-SecondLife-Object-Key"))
            .unwrap_or_else(|| "default".to_string());
        let session: String = session.trim().chars().take(MAX_SESSION_CHARS).collect();
        let include_result = match json.as_ref().and_then(|object| object.get("include_result")) {
            None => true,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err(text(tide::StatusCode::BadRequest, "include_result must be a boolean")),
        };
        let code = match json {
            Some(_) => field("code").unwrap_or_default(),
            None => body.to_string(),
        };
        Ok(Self { session, code, include_result })
    }
}

fn configured_token() -> Option<String> {
    std::env::var(TOKEN_ENV)
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

fn eval_seconds() -> u64 {
    std::env::var(SECONDS_ENV)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .filter(|seconds| (1..=120).contains(seconds))
        .unwrap_or(DEFAULT_SECONDS)
}

fn tokens_match(expected: &str, given: &str) -> bool {
    expected.len() == given.len()
        && expected
            .bytes()
            .zip(given.bytes())
            .fold(0u8, |diff, (left, right)| diff | (left ^ right))
            == 0
}

fn owner_allowed(allowlist: Option<&str>, owner: &str) -> bool {
    let allowed: Vec<&str> = allowlist
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .collect();
    allowed.is_empty() || allowed.iter().any(|key| key.eq_ignore_ascii_case(owner.trim()))
}

fn ruby_error_response(error: RubyError) -> tide::Response {
    let status = match error {
        RubyError::Timeout(_) => tide::StatusCode::GatewayTimeout,
        RubyError::Raised(_) => tide::StatusCode::UnprocessableEntity,
        RubyError::Unavailable(_) => tide::StatusCode::ServiceUnavailable,
    };
    text(status, &error.to_string())
}

fn text(status: tide::StatusCode, body: &str) -> tide::Response {
    let mut response = tide::Response::new(status);
    response.set_body(body.to_string());
    response.insert_header("Content-Type", "text/plain; charset=utf-8");
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_and_owner_checks() {
        assert!(tokens_match("secret", "secret"));
        assert!(!tokens_match("secret", "secreT"));
        assert!(!tokens_match("secret", "secret-and-more"));
        assert!(!tokens_match("secret", ""));

        assert!(owner_allowed(None, "anyone"));
        assert!(owner_allowed(Some(" , "), "anyone"));
        assert!(owner_allowed(Some("AAA, bbb"), "aaa"));
        assert!(!owner_allowed(Some("aaa,bbb"), "ccc"));
        assert!(!owner_allowed(Some("aaa"), ""));
    }

    async fn post(app: &tide::Server<()>, path: &str, headers: &[(&str, &str)], body: &str) -> (u16, String) {
        let mut request = tide::http::Request::new(
            tide::http::Method::Post,
            tide::http::Url::parse(&format!("http://localhost{path}")).unwrap(),
        );
        for (name, value) in headers {
            request.insert_header(*name, *value);
        }
        request.set_body(body.to_string());
        let mut response: tide::Response = app.respond(request).await.unwrap();
        let body = response.take_body().into_string().await.unwrap();
        (response.status() as u16, body)
    }

    // Env vars are process-wide, so every route check lives in this one test.
    #[test]
    fn sl_ruby_routes_end_to_end() {
        ruby_vm::start().expect("Ruby VM should start");
        let mut app = tide::new();
        let calls = Arc::new(std::sync::Mutex::new(Vec::<(String, String, Value)>::new()));
        let recorded = Arc::clone(&calls);
        let snapshot_path = std::env::temp_dir().join(format!("ruby-route-{}.json", uuid::Uuid::new_v4()));
        let persisted_path = snapshot_path.clone();
        let mut entries = partitioned_array_rust::PartitionedArray::new(1, 2, 1, true);
        entries.allocate(false);
        let entries = Arc::new(std::sync::Mutex::new(entries));
        let stored_entries = Arc::clone(&entries);
        let store: StoreCall = Arc::new(move |session, operation, args| {
            recorded.lock().unwrap().push((session.to_string(), operation.to_string(), args.clone()));
            if operation.starts_with("var_") {
                let mut entries = stored_entries.lock().unwrap();
                let entry_id = entries.non_empty_ids().into_iter().find(|entry_id| {
                    let row = entries.get(*entry_id).unwrap();
                    row.get("session").and_then(Value::as_str) == Some(session)
                        && row.get("name") == args.get("name")
                });
                return match operation {
                    "var_set" => crate::persistent_snapshot::update(&mut *entries, |next| {
                        let write = |row: &mut serde_json::Map<String, Value>| {
                            row.insert("session".into(), serde_json::json!(session));
                            row.insert("name".into(), args["name"].clone());
                            row.insert("value".into(), args["value"].clone());
                        };
                        if let Some(entry_id) = entry_id {
                            assert!(next.set_with(entry_id, write));
                        } else {
                            next.add(write).ok_or_else(|| "store full".to_string())?;
                        }
                        Ok(args["value"].clone())
                    }, |next| crate::persistent_snapshot::save(&persisted_path, next)),
                    "var_get" => entry_id.map(|entry_id| entries.get(entry_id).unwrap()["value"].clone())
                        .ok_or_else(|| "entry was not found".into()),
                    "var_delete" => crate::persistent_snapshot::update(&mut *entries, |next| {
                        if let Some(entry_id) = entry_id { next.delete(entry_id); }
                        Ok(Value::Bool(entry_id.is_some()))
                    }, |next| crate::persistent_snapshot::save(&persisted_path, next)),
                    "var_view" => Ok(Value::Object(entries.non_empty_ids().into_iter().filter_map(|entry_id| {
                        let row = entries.get(entry_id)?;
                        (row.get("session")?.as_str()? == session)
                            .then(|| (row["name"].as_str().unwrap().to_string(), row["value"].clone()))
                    }).collect())),
                    _ => Err(format!("unexpected operation: {operation}")),
                };
            }
            match operation {
                "file_delete" => Ok(Value::Bool(true)),
                "file_write" | "file_read" => Ok(serde_json::json!("hello")),
                "file_list" => Ok(serde_json::json!(["memo.txt"])),
                "matrix_get" => Ok(serde_json::json!({"rows": 1, "cols": 2, "values": [3, 4]})),
                _ => Err(format!("unexpected operation: {operation}")),
            }
        });
        // SAFETY: no other test in this crate reads or writes these variables.
        unsafe {
            std::env::remove_var(TOKEN_ENV);
            std::env::remove_var(OWNERS_ENV);
            std::env::set_var(SECONDS_ENV, "1");
        }
        mount(&mut app, store).expect("Ruby store should mount");

        async_std::task::block_on(async {
            let (status, _) = post(&app, "/sl/ruby/eval", &[], "1 + 1").await;
            assert_eq!(status, 503, "disabled without a token");

            unsafe { std::env::set_var(TOKEN_ENV, "s3cret") };
            let (status, _) = post(&app, "/sl/ruby/eval", &[("X-Ruby-Token", "nope")], "1").await;
            assert_eq!(status, 401);

            let auth = [("X-Ruby-Token", "s3cret"), ("X-SecondLife-Object-Key", "obj-1")];
            let (status, body) = post(&app, "/sl/ruby/eval", &auth, "x = 20; puts 'hé'; x + 1").await;
            assert_eq!((status, body.as_str()), (200, "hé\n=> 21"));

            let (_, body) = post(&app, "/sl/ruby/eval", &auth, "x * 2").await;
            assert_eq!(body, "=> 40", "locals persist per object session");

            let (status, body) = post(&app, "/sl/ruby/eval", &auth,
                r#"{"code":"puts 'hello'; 42","include_result":false}"#).await;
            assert_eq!((status, body.as_str()), (200, "hello\n"));
            let (status, body) = post(&app, "/sl/ruby/eval", &auth,
                r#"{"code":"42","include_result":false}"#).await;
            assert_eq!((status, body.as_str()), (200, ""));
            let (status, body) = post(&app, "/sl/ruby/eval", &auth,
                r#"{"code":"raise 'bad'","include_result":false}"#).await;
            assert_eq!((status, body.as_str()), (422, "RuntimeError: bad"));
            let (status, body) = post(&app, "/sl/ruby/eval", &auth,
                r#"{"code":"42","include_result":"false"}"#).await;
            assert_eq!((status, body.as_str()), (400, "include_result must be a boolean"));

            let long_code = format!("{}puts 'ok'", "# filler\n".repeat(2_100));
            assert!(long_code.len() > 16_000);
            let (status, body) = post(&app, "/sl/ruby/eval", &auth, &long_code).await;
            assert_eq!((status, body.as_str()), (200, "ok\n=> nil"));

            let (status, body) = post(&app, "/sl/ruby/eval", &auth,
                r#"{"code":"puts 'x' * 20000","include_result":false}"#).await;
            assert_eq!(status, 200);
            assert_eq!(body, format!("{}\n", "x".repeat(20_000)));

            let (_, body) = post(&app, "/sl/ruby/eval", &[("Authorization", "Bearer s3cret")],
                r#"{"code":"defined?(x).inspect","session":"other"}"#).await;
            assert_eq!(body, "=> \"nil\"", "sessions are isolated");

            let (status, body) = post(&app, "/sl/ruby/eval", &auth,
                r#"puts var_set("score", 42); puts var_get("score"); puts var_view["score"]; puts var_delete("score"); puts file_write("memo.txt", "hello"); puts file_read("memo.txt"); puts file_list.inspect; puts file_delete("memo.txt"); puts matrix_get("A")["values"].inspect"#).await;
            assert_eq!(status, 200, "{body}");
            assert!(body.contains("42\n42\n42\ntrue\nhello\nhello\n[\"memo.txt\"]\ntrue\n[3, 4]\n"), "{body}");
            let recorded = calls.lock().unwrap();
            assert_eq!(recorded.len(), 9);
            assert!(recorded.iter().all(|(session, _, _)| session == "obj-1"));
            assert_eq!(recorded[0].2, serde_json::json!({"name":"score","value":42}));
            drop(recorded);

            *entries.lock().unwrap() = crate::persistent_snapshot::load(&snapshot_path).unwrap().unwrap();
            let (_, body) = post(&app, "/sl/ruby/eval", &auth, "var_view").await;
            assert_eq!(body, "=> {}", "deletion survives restoring the partitioned snapshot");
            let (status, _) = post(&app, "/sl/ruby/eval", &auth,
                r#"var_set("score", 42); var_set("progress", {"items" => ["key", true, nil]})"#).await;
            assert_eq!(status, 200);
            let (status, _) = post(&app, "/sl/ruby/eval", &[("X-Ruby-Token", "s3cret")],
                r#"{"session":"other","code":"var_set('score', 7)"}"#).await;
            assert_eq!(status, 200);
            *entries.lock().unwrap() = crate::persistent_snapshot::load(&snapshot_path).unwrap().unwrap();
            let (_, body) = post(&app, "/sl/ruby/eval", &auth, r#"var_get("progress")["items"].to_json"#).await;
            assert_eq!(body, r#"=> "[\"key\",true,null]""#);

            let (status, body) = post(&app, "/sl/ruby/eval", &auth, r#"var_get("missing")"#).await;
            assert_eq!((status, body.as_str()), (422, "RuntimeError: entry was not found"));

            let (status, body) = post(&app, "/sl/ruby/eval", &auth, "raise ArgumentError, 'boom'").await;
            assert_eq!((status, body.as_str()), (422, "ArgumentError: boom"));

            let (status, body) = post(&app, "/sl/ruby/eval", &auth, "exit 3").await;
            assert_eq!(status, 422, "exit must not stop the server: {body}");

            let (status, body) = post(&app, "/sl/ruby/eval", &auth, "loop {}").await;
            assert_eq!(status, 422, "{body}");
            assert!(body.contains("Timeout"), "{body}");
            let (status, _) = post(&app, "/sl/ruby/eval", &auth, "1").await;
            assert_eq!(status, 200, "VM is free again after a timeout");

            unsafe { std::env::set_var(OWNERS_ENV, "owner-a") };
            let (status, _) = post(&app, "/sl/ruby/eval", &auth, "1").await;
            assert_eq!(status, 403);
            let with_owner = [("X-Ruby-Token", "s3cret"), ("X-SecondLife-Object-Key", "obj-1"),
                ("X-SecondLife-Owner-Key", "OWNER-A")];
            let (status, _) = post(&app, "/sl/ruby/eval", &with_owner, "1").await;
            assert_eq!(status, 200);

            let (_, body) = post(&app, "/sl/ruby/reset", &with_owner, "").await;
            assert_eq!(body, "session obj-1 reset");
            let (_, body) = post(&app, "/sl/ruby/eval", &with_owner, "defined?(x).inspect").await;
            assert_eq!(body, "=> \"nil\"");
            let (_, body) = post(&app, "/sl/ruby/eval", &with_owner, r#"var_get("score")"#).await;
            assert_eq!(body, "=> 42", "reset must not erase the shared store");
        });

        unsafe {
            std::env::remove_var(TOKEN_ENV);
            std::env::remove_var(OWNERS_ENV);
            std::env::remove_var(SECONDS_ENV);
        }
        std::fs::remove_file(snapshot_path).unwrap();
    }
}
