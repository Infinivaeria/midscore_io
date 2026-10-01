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

use std::time::Duration;

use serde_json::Value;

use crate::ruby_vm::{self, RubyError};

const TOKEN_ENV: &str = "TIADE_RUBY_EVAL_TOKEN";
const OWNERS_ENV: &str = "TIADE_RUBY_EVAL_SL_OWNERS";
const SECONDS_ENV: &str = "TIADE_RUBY_EVAL_SECONDS";
const DEFAULT_SECONDS: u64 = 10;
const MAX_CODE_CHARS: usize = 16_000;
const MAX_SESSION_CHARS: usize = 128;

// Ruby side: per-session bindings, stdout capture and a Ruby-level timeout so
// a runaway snippet cannot hold the shared VM thread. Input arrives as an
// Array of strings via `ruby_vm::call_with_strings`, never spliced into source.
const PRELUDE: &str = r###"
require 'stringio'
require 'timeout'

module TiadeSL
  SESSIONS = {}
  MAX_SESSIONS = 256
  MAX_OUTPUT = 4000

  def self.binding_for(session)
    unless SESSIONS.key?(session)
      SESSIONS.shift if SESSIONS.size >= MAX_SESSIONS
      SESSIONS[session] = Object.new.instance_eval { binding }
    end
    SESSIONS[session]
  end

  # Returns "OK\n<output>" or "ERR\n<output>".
  EVAL = lambda do |(session, code, seconds)|
    out = StringIO.new
    saved = $stdout
    status = 'OK'
    begin
      $stdout = out
      value = Timeout.timeout(seconds.to_f) do
        binding_for(session).eval(code, "(sl:#{session})", 1)
      end
      text = "#{out.string}=> #{value.inspect}"
    rescue Timeout::Error
      status = 'ERR'
      text = "#{out.string}Timeout: evaluation exceeded #{seconds}s"
    rescue Exception => e
      status = 'ERR'
      text = "#{out.string}#{e.class}: #{e.message}"
    ensure
      $stdout = saved
    end
    text = text[0, MAX_OUTPUT] + "…" if text.length > MAX_OUTPUT
    "#{status}\n#{text}"
  end

  RESET = lambda do |(session)|
    SESSIONS.delete(session) ? "session #{session} reset" : "session #{session} was empty"
  end
end
:tiade_sl_ruby_ready
"###;

pub fn mount<State: Clone + Send + Sync + 'static>(app: &mut tide::Server<State>) {
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
        if input.code.chars().count() > MAX_CODE_CHARS {
            return Ok(text(tide::StatusCode::PayloadTooLarge, "code is too long"));
        }

        let seconds = eval_seconds();
        let result = ruby_vm::call_with_strings(
            "TiadeSL::EVAL",
            vec![input.session, input.code, seconds.to_string()],
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
}

struct SlRubyRequest {
    session: String,
    code: String,
}

impl SlRubyRequest {
    /// Authorizes the request and reads `{"code","session","token"}` JSON or a
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
        let code = match json {
            Some(_) => field("code").unwrap_or_default(),
            None => body.to_string(),
        };
        Ok(Self { session, code })
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
        // SAFETY: no other test in this crate reads or writes these variables.
        unsafe {
            std::env::remove_var(TOKEN_ENV);
            std::env::remove_var(OWNERS_ENV);
            std::env::set_var(SECONDS_ENV, "1");
        }
        mount(&mut app);

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

            let (_, body) = post(&app, "/sl/ruby/eval", &[("Authorization", "Bearer s3cret")],
                r#"{"code":"defined?(x).inspect","session":"other"}"#).await;
            assert_eq!(body, "=> \"nil\"", "sessions are isolated");

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
        });

        unsafe {
            std::env::remove_var(TOKEN_ENV);
            std::env::remove_var(OWNERS_ENV);
            std::env::remove_var(SECONDS_ENV);
        }
    }
}
