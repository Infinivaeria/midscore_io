use magnus::{Error, Ruby, function, method, prelude::*};
use serde_json::{Value, json};

const RUBY_ADAPTER: &str = include_str!("../ruby/partitioned_game_client.rb");

#[magnus::wrap(class = "OllamaGameClientNative::Transport")]
struct Transport {
    server_url: String,
    team: String,
    player: String,
}

impl Transport {
    fn new(server_url: String, team: String, player: String) -> Self {
        Self {
            server_url: server_url.trim_end_matches('/').to_string(),
            team,
            player,
        }
    }

    fn routes(&self) -> Result<String, Error> {
        self.request("GET", "/ollama", None)
    }

    fn health(&self) -> Result<String, Error> {
        self.request("GET", "/ollama/health", None)
    }

    fn state(&self) -> Result<String, Error> {
        self.request("GET", &self.game_path("state"), None)
    }

    fn reset(&self) -> Result<String, Error> {
        self.request("POST", &self.game_path("reset"), Some(json!({})))
    }

    fn turn(&self, action: String, game_prompt: String, state_json: String) -> Result<String, Error> {
        let mut payload = json!({ "action": action });
        if !game_prompt.trim().is_empty() {
            payload["game_prompt"] = Value::String(game_prompt);
        }
        if !state_json.trim().is_empty() {
            let state: Value = serde_json::from_str(&state_json)
                .map_err(|error| ruby_error(format!("state must be valid JSON: {error}")))?;
            if !state.is_object() {
                return Err(ruby_error("state must be a JSON object"));
            }
            payload["state"] = state;
        }
        self.request("POST", &self.game_path("turn"), Some(payload))
    }

    fn game_path(&self, action: &str) -> String {
        format!(
            "/game/{}/{}/{}",
            urlencoding::encode(&self.team),
            urlencoding::encode(&self.player),
            action
        )
    }

    fn request(&self, method: &str, path: &str, body: Option<Value>) -> Result<String, Error> {
        let url = format!("{}{}", self.server_url, path);
        let response = match (method, body) {
            ("GET", None) => ureq::get(&url)
                .header("Accept", "application/json")
                .call(),
            ("POST", Some(body)) => ureq::post(&url)
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")
                .send_json(body),
            ("POST", None) => return Err(ruby_error("POST requests require a JSON body")),
            _ => return Err(ruby_error("unsupported HTTP method")),
        }
        .map_err(|error| ruby_error(format!("relay request failed: {error}")))?;
        let status = response.status();
        let text = response
            .into_body()
            .read_to_string()
            .map_err(|error| ruby_error(format!("could not read relay response: {error}")))?;
        if !(200..300).contains(&status.as_u16()) {
            return Err(ruby_error(format!("relay returned HTTP {}: {text}", status.as_u16())));
        }
        Ok(text)
    }
}

fn ruby_error(message: impl ToString) -> Error {
    let ruby = Ruby::get().expect("native Ruby methods require an active Ruby VM");
    Error::new(ruby.exception_runtime_error(), message.to_string())
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("OllamaGameClientNative")?;
    let class = module.define_class("Transport", ruby.class_object())?;
    class.define_singleton_method("new", function!(Transport::new, 3))?;
    class.define_method("routes_json", method!(Transport::routes, 0))?;
    class.define_method("health_json", method!(Transport::health, 0))?;
    class.define_method("state_json", method!(Transport::state, 0))?;
    class.define_method("reset_json", method!(Transport::reset, 0))?;
    class.define_method("turn_json", method!(Transport::turn, 3))?;
    ruby.eval::<magnus::Value>(RUBY_ADAPTER)?;
    Ok(())
}