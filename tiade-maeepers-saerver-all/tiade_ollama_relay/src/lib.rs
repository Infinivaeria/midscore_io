use partitioned_array_rust::{LineDb, LineDbConfig};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const RELAY_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug)]
pub struct RelayConfig {
    pub ollama_http_address: String,
    pub ollama_model_name: String,
    pub team_log_dir: PathBuf,
    pub team_history_char_limit: usize,
    pub team_history_entry_limit: usize,
    pub team_history_storage_limit: usize,
    pub max_chat_message_chars: usize,
    pub max_team_prompt_chars: usize,
    pub max_game_state_chars: usize,
    pub team_prompt_token: Option<String>,
}

impl Default for RelayConfig {
    fn default() -> Self {
        let ollama_http_address =
            std::env::var("OLLAMA_HTTP_ADDRESS").unwrap_or_else(|_| "http://localhost:11434".to_string());
        let ollama_model_name =
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama2-uncensored:latest".to_string());

        Self {
            ollama_http_address,
            ollama_model_name,
            team_log_dir: PathBuf::from("/root/midscore_io/logs/ollama_teams"),
            team_history_char_limit: 2_000,
            team_history_entry_limit: 8,
            team_history_storage_limit: 500,
            max_chat_message_chars: 8_000,
            max_team_prompt_chars: 8_000,
            max_game_state_chars: 32_000,
            team_prompt_token: std::env::var("OLLAMA_TEAM_PROMPT_TOKEN")
                .ok()
                .filter(|token| !token.trim().is_empty()),
        }
    }
}

#[derive(Deserialize)]
struct ChatInput {
    message: String,
}

#[derive(Deserialize)]
struct GameInput {
    message: String,
    game_prompt: Option<String>,
}

#[derive(Deserialize)]
struct GameTurnInput {
    action: String,
    game_prompt: Option<String>,
    state: Option<Value>,
}

#[derive(Deserialize)]
struct TeamPromptInput {
    prompt: String,
}

#[derive(Serialize)]
struct ChatOutput {
    response: String,
    team: String,
    model: String,
    fallback_used: bool,
    history_chars: usize,
}

#[derive(Serialize)]
struct GameOutput {
    response: String,
    team: String,
    player: String,
    model: String,
    fallback_used: bool,
    history_chars: usize,
    game_prompt_applied: bool,
}

#[derive(Serialize)]
struct GameTurnOutput {
    response: String,
    directive: Value,
    state: Value,
    team: String,
    player: String,
    model: String,
    fallback_used: bool,
}

#[derive(Serialize)]
struct GameStateOutput {
    state: Value,
    team: String,
    player: String,
}

#[derive(Serialize)]
struct HistoryOutput {
    history: String,
    team: String,
}

#[derive(Serialize)]
struct HealthOutput {
    status: &'static str,
    version: &'static str,
    ollama_http_address: String,
    configured_model: String,
    available_models: Vec<String>,
}

#[derive(Serialize)]
struct RouteInfo {
    method: &'static str,
    path: &'static str,
    description: &'static str,
    authorization: Option<&'static str>,
}

#[derive(Serialize)]
struct RouteCatalogOutput {
    service: &'static str,
    version: &'static str,
    routes: Vec<RouteInfo>,
}

#[derive(Serialize)]
struct TeamPromptOutput {
    team: String,
    prompt: String,
    prompt_chars: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TeamLogEntry {
    user: String,
    reply: String,
}

pub fn mount_routes<State: Clone + Send + Sync + 'static>(
    app: &mut tide::Server<State>,
    config: RelayConfig,
) -> tide::Result<()> {
    fs::create_dir_all(&config.team_log_dir)?;
    let cfg = Arc::new(config);
    let team_store = Arc::new(Mutex::new(open_team_store(&cfg)?));

    {
        let cfg = cfg.clone();
        app.at("/ollama")
            .options(|_| async { Ok(cors_preflight_response()) })
            .get(|_| async { Ok(json_response(tide::StatusCode::Ok, route_catalog())) })
            .post(move |mut req: tide::Request<State>| {
                let cfg = cfg.clone();
                async move {
                    let body = req.body_string().await?;
                    let payload: Value = serde_json::from_str(&body).map_err(|error| {
                        tide::Error::from_str(
                            tide::StatusCode::BadRequest,
                            format!("invalid JSON payload: {error}"),
                        )
                    })?;
                    let payload = standard_chat_request(payload, &cfg.ollama_model_name)
                        .map_err(|error| tide::Error::from_str(tide::StatusCode::BadRequest, error))?;
                    let response = post_json(&cfg, "/api/chat", payload).await?;
                    Ok(json_response(tide::StatusCode::Ok, response))
                }
            });
    }

    {
        let cfg = cfg.clone();
        let team_store = team_store.clone();
        app.at("/chat/:team")
            .options(|_| async { Ok(cors_preflight_response()) })
            .post(move |mut req: tide::Request<State>| {
                let cfg = cfg.clone();
                let team_store = team_store.clone();
                async move {
                    let team = req.param("team")?.to_string();
                    validate_team(&team)?;
                    let body = req.body_string().await?;
                    let payload: ChatInput = serde_json::from_str(&body).map_err(|e| {
                        tide::Error::from_str(
                            tide::StatusCode::BadRequest,
                            format!("invalid JSON payload: {e}"),
                        )
                    })?;
                    let message = payload.message.trim();
                    if message.is_empty() {
                        return Ok(json_error(tide::StatusCode::BadRequest, "message is required"));
                    }
                    if message.chars().count() > cfg.max_chat_message_chars {
                        return Ok(json_error(
                            tide::StatusCode::PayloadTooLarge,
                            "message exceeds the configured size limit",
                        ));
                    }

                    let (model_name, available_models) = resolve_model_name(&cfg).await?;
                    let model_name = match model_name {
                        Some(v) => v,
                        None => {
                            let error = json!({
                                "error": format!(
                                    "no Ollama models are installed on {}; run 'ollama pull {}' or set OLLAMA_MODEL to an installed model",
                                    cfg.ollama_http_address,
                                    cfg.ollama_model_name,
                                ),
                                "configured_model": cfg.ollama_model_name,
                                "available_models": available_models,
                            });
                            return Ok(json_response(tide::StatusCode::ServiceUnavailable, error));
                        }
                    };

                    let messages = build_messages(&team_store, &cfg, &team, message)?;
                    let chat_payload = json!({
                        "model": model_name,
                        "stream": false,
                        "messages": messages,
                    });

                    let result = post_json(&cfg, "/api/chat", chat_payload).await?;
                    let mut reply = extract_reply(&result);
                    let mut fallback_used = false;

                    if no_text_reply(&reply) {
                        let fallback = generate_fallback(&cfg, &model_name, message).await?;
                        let fallback_reply = extract_reply(&fallback);
                        if !no_text_reply(&fallback_reply) {
                            reply = fallback_reply;
                            fallback_used = true;
                        }
                    }

                    append_team_log(&team_store, &cfg, &team, message, &reply)?;
                    let history_chars = team_history(&team_store, &cfg, &team)?.len();
                    let out = ChatOutput {
                        response: reply,
                        team,
                        model: model_name,
                        fallback_used,
                        history_chars,
                    };

                    Ok(json_response(tide::StatusCode::Ok, out))
                }
            });
    }

    {
        let cfg = cfg.clone();
        let team_store = team_store.clone();
        app.at("/game/:team/:player/turn")
            .options(|_| async { Ok(cors_preflight_response()) })
            .post(move |mut req: tide::Request<State>| {
                let cfg = cfg.clone();
                let team_store = team_store.clone();
                async move {
                    let team = req.param("team")?.to_string();
                    let player = req.param("player")?.to_string();
                    validate_team(&team)?;
                    validate_player(&player)?;
                    let body = req.body_string().await?;
                    let input: GameTurnInput = serde_json::from_str(&body).map_err(|error| {
                        tide::Error::from_str(tide::StatusCode::BadRequest, format!("invalid JSON payload: {error}"))
                    })?;
                    let action = input.action.trim();
                    if action.is_empty() {
                        return Ok(json_error(tide::StatusCode::BadRequest, "action is required"));
                    }
                    if action.chars().count() > cfg.max_chat_message_chars {
                        return Ok(json_error(tide::StatusCode::PayloadTooLarge, "action exceeds the configured size limit"));
                    }
                    let game_prompt = input.game_prompt.unwrap_or_default().trim().to_string();
                    if game_prompt.chars().count() > cfg.max_team_prompt_chars {
                        return Ok(json_error(tide::StatusCode::PayloadTooLarge, "game_prompt exceeds the configured size limit"));
                    }
                    let current_state = input.state.unwrap_or(game_state(&team_store, &team, &player)?);
                    if !current_state.is_object() {
                        return Ok(json_error(tide::StatusCode::BadRequest, "state must be a JSON object"));
                    }
                    if let Err(error) = save_game_state(&team_store, &cfg, &team, &player, &current_state) {
                        return Ok(json_error(error.status(), &error.to_string()));
                    }

                    let (model_name, available_models) = resolve_model_name(&cfg).await?;
                    let Some(model_name) = model_name else {
                        return Ok(json_response(tide::StatusCode::ServiceUnavailable, json!({
                            "error": "no Ollama models are installed",
                            "configured_model": cfg.ollama_model_name,
                            "available_models": available_models,
                        })));
                    };
                    let history_key = game_history_key(&team, &player);
                    let messages = build_game_turn_messages(
                        &team_store, &cfg, &team, &history_key, &player, action, &game_prompt, &current_state,
                    )?;
                    let result = post_json(&cfg, "/api/chat", json!({
                        "model": model_name,
                        "stream": false,
                        "format": "json",
                        "messages": messages,
                    })).await?;
                    let mut response = extract_reply(&result);
                    let mut fallback_used = false;
                    if no_text_reply(&response) {
                        let fallback = generate_fallback(&cfg, &model_name, action).await?;
                        let fallback_reply = extract_reply(&fallback);
                        if !no_text_reply(&fallback_reply) {
                            response = fallback_reply;
                            fallback_used = true;
                        }
                    }
                    let mut directive = game_directive(&response, &current_state);
                    let next_state = directive
                        .get("state")
                        .filter(|state| state.is_object())
                        .cloned()
                        .unwrap_or(current_state);
                    directive["state"] = next_state.clone();
                    save_game_state(&team_store, &cfg, &team, &player, &next_state)?;
                    append_team_log(&team_store, &cfg, &history_key, action, &response)?;

                    Ok(json_response(tide::StatusCode::Ok, GameTurnOutput {
                        response,
                        directive,
                        state: next_state,
                        team,
                        player,
                        model: model_name,
                        fallback_used,
                    }))
                }
            });
    }

    {
        let team_store = team_store.clone();
        app.at("/game/:team/:player/state")
            .options(|_| async { Ok(cors_preflight_response()) })
            .get(move |req: tide::Request<State>| {
                let team_store = team_store.clone();
                async move {
                    let team = req.param("team")?.to_string();
                    let player = req.param("player")?.to_string();
                    validate_team(&team)?;
                    validate_player(&player)?;
                    Ok(json_response(tide::StatusCode::Ok, GameStateOutput {
                        state: game_state(&team_store, &team, &player)?,
                        team,
                        player,
                    }))
                }
            });
    }

    {
        let team_store = team_store.clone();
        app.at("/game/:team/:player/reset")
            .options(|_| async { Ok(cors_preflight_response()) })
            .post(move |req: tide::Request<State>| {
                let team_store = team_store.clone();
                async move {
                    let team = req.param("team")?.to_string();
                    let player = req.param("player")?.to_string();
                    validate_team(&team)?;
                    validate_player(&player)?;
                    reset_game_session(&team_store, &team, &player)?;
                    Ok(json_response(tide::StatusCode::Ok, GameStateOutput {
                        state: json!({}),
                        team,
                        player,
                    }))
                }
            });
    }

    {
        let cfg = cfg.clone();
        let team_store = team_store.clone();
        app.at("/game/:team/:player")
            .options(|_| async { Ok(cors_preflight_response()) })
            .post(move |mut req: tide::Request<State>| {
                let cfg = cfg.clone();
                let team_store = team_store.clone();
                async move {
                    let team = req.param("team")?.to_string();
                    let player = req.param("player")?.to_string();
                    validate_team(&team)?;
                    validate_player(&player)?;

                    let body = req.body_string().await?;
                    let input: GameInput = serde_json::from_str(&body).map_err(|error| {
                        tide::Error::from_str(
                            tide::StatusCode::BadRequest,
                            format!("invalid JSON payload: {error}"),
                        )
                    })?;
                    let message = input.message.trim();
                    if message.is_empty() {
                        return Ok(json_error(tide::StatusCode::BadRequest, "message is required"));
                    }
                    if message.chars().count() > cfg.max_chat_message_chars {
                        return Ok(json_error(
                            tide::StatusCode::PayloadTooLarge,
                            "message exceeds the configured size limit",
                        ));
                    }
                    let game_prompt = input.game_prompt.unwrap_or_default().trim().to_string();
                    if game_prompt.chars().count() > cfg.max_team_prompt_chars {
                        return Ok(json_error(
                            tide::StatusCode::PayloadTooLarge,
                            "game_prompt exceeds the configured size limit",
                        ));
                    }

                    let (model_name, available_models) = resolve_model_name(&cfg).await?;
                    let model_name = match model_name {
                        Some(model) => model,
                        None => {
                            return Ok(json_response(tide::StatusCode::ServiceUnavailable, json!({
                                "error": "no Ollama models are installed",
                                "configured_model": cfg.ollama_model_name,
                                "available_models": available_models,
                            })));
                        }
                    };

                    let history_key = game_history_key(&team, &player);
                    let messages = build_game_messages(
                        &team_store,
                        &cfg,
                        &team,
                        &history_key,
                        &player,
                        message,
                        &game_prompt,
                    )?;
                    let result = post_json(&cfg, "/api/chat", json!({
                        "model": model_name,
                        "stream": false,
                        "messages": messages,
                    })).await?;
                    let mut reply = extract_reply(&result);
                    let mut fallback_used = false;
                    if no_text_reply(&reply) {
                        let fallback = generate_fallback(&cfg, &model_name, message).await?;
                        let fallback_reply = extract_reply(&fallback);
                        if !no_text_reply(&fallback_reply) {
                            reply = fallback_reply;
                            fallback_used = true;
                        }
                    }

                    append_team_log(&team_store, &cfg, &history_key, message, &reply)?;
                    let history_chars = team_history(&team_store, &cfg, &history_key)?.len();
                    Ok(json_response(tide::StatusCode::Ok, GameOutput {
                        response: reply,
                        team,
                        player,
                        model: model_name,
                        fallback_used,
                        history_chars,
                        game_prompt_applied: !game_prompt.is_empty(),
                    }))
                }
            });
    }

    {
        let cfg = cfg.clone();
        let team_store = team_store.clone();
        let get_cfg = cfg.clone();
        let get_team_store = team_store.clone();
        app.at("/teams/:team/prompt")
            .options(|_| async { Ok(cors_preflight_response()) })
            .get(move |req: tide::Request<State>| {
                let cfg = get_cfg.clone();
                let team_store = get_team_store.clone();
                async move {
                    let team = req.param("team")?.to_string();
                    validate_team(&team)?;
                    if let Some(response) = team_prompt_authorization_error(&req, &cfg) {
                        return Ok(response);
                    }

                    let prompt = team_prompt(&team_store, &team)?.unwrap_or_default();
                    let prompt_chars = prompt.chars().count();
                    Ok(json_response(
                        tide::StatusCode::Ok,
                        TeamPromptOutput { team, prompt, prompt_chars },
                    ))
                }
            })
            .post(move |mut req: tide::Request<State>| {
                let cfg = cfg.clone();
                let team_store = team_store.clone();
                async move {
                    let team = req.param("team")?.to_string();
                    validate_team(&team)?;
                    if let Some(response) = team_prompt_authorization_error(&req, &cfg) {
                        return Ok(response);
                    }

                    let body = req.body_string().await?;
                    let input: TeamPromptInput = serde_json::from_str(&body).map_err(|error| {
                        tide::Error::from_str(
                            tide::StatusCode::BadRequest,
                            format!("invalid JSON payload: {error}"),
                        )
                    })?;
                    if input.prompt.chars().count() > cfg.max_team_prompt_chars {
                        return Ok(json_error(
                            tide::StatusCode::PayloadTooLarge,
                            "prompt exceeds the configured size limit",
                        ));
                    }

                    let prompt = input.prompt.trim().to_string();
                    let prompt_chars = save_team_prompt(&team_store, &team, &prompt)?;
                    Ok(json_response(
                        tide::StatusCode::Ok,
                        TeamPromptOutput { team, prompt, prompt_chars },
                    ))
                }
            });
    }

    {
        let cfg = cfg.clone();
        let team_store = team_store.clone();
        app.at("/history/:team")
            .options(|_| async { Ok(cors_preflight_response()) })
            .get(move |req: tide::Request<State>| {
                let cfg = cfg.clone();
                let team_store = team_store.clone();
                async move {
                    let team = req.param("team")?.to_string();
                    validate_team(&team)?;
                    let out = HistoryOutput {
                        history: team_history(&team_store, &cfg, &team)?,
                        team,
                    };
                    Ok(json_response(tide::StatusCode::Ok, out))
                }
            });
    }

    {
        let cfg = cfg.clone();
        app.at("/ollama/health")
            .options(|_| async { Ok(cors_preflight_response()) })
            .get(move |_req: tide::Request<State>| {
                let cfg = cfg.clone();
                async move {
                    match available_models(&cfg).await {
                        Ok(available_models) => Ok(json_response(
                            tide::StatusCode::Ok,
                            HealthOutput {
                                status: "ok",
                                version: RELAY_VERSION,
                                ollama_http_address: cfg.ollama_http_address.clone(),
                                configured_model: cfg.ollama_model_name.clone(),
                                available_models,
                            },
                        )),
                        Err(error) => Ok(json_response(
                            tide::StatusCode::ServiceUnavailable,
                            json!({
                                "status": "unavailable",
                                "version": RELAY_VERSION,
                                "error": error.to_string(),
                            }),
                        )),
                    }
                }
            });
    }

    Ok(())
}

fn json_response<T: Serialize>(status: tide::StatusCode, data: T) -> tide::Response {
    let mut res = tide::Response::new(status);
    res.set_content_type(tide::http::mime::JSON);
    add_cors_headers(&mut res);
    let body = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string());
    res.set_body(body);
    res
}

fn add_cors_headers(response: &mut tide::Response) {
    response.insert_header("Access-Control-Allow-Origin", "*");
    response.insert_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
    response.insert_header("Access-Control-Allow-Headers", "Authorization, Content-Type");
}

fn cors_preflight_response() -> tide::Response {
    let mut response = tide::Response::new(tide::StatusCode::NoContent);
    add_cors_headers(&mut response);
    response
}

fn route_catalog() -> RouteCatalogOutput {
    RouteCatalogOutput {
        service: "ollama-team-relay",
        version: RELAY_VERSION,
        routes: vec![
            RouteInfo {
                method: "GET",
                path: "/ollama",
                description: "Lists all generic Ollama relay routes.",
                authorization: None,
            },
            RouteInfo {
                method: "POST",
                path: "/ollama",
                description: "Sends a standard non-streaming Ollama /api/chat request.",
                authorization: None,
            },
            RouteInfo {
                method: "GET",
                path: "/ollama/health",
                description: "Reports Ollama upstream availability and installed models.",
                authorization: None,
            },
            RouteInfo {
                method: "POST",
                path: "/chat/:team",
                description: "Sends a JSON message to a team's Ollama conversation.",
                authorization: None,
            },
            RouteInfo {
                method: "POST",
                path: "/game/:team/:player",
                description: "Sends game input with isolated player history and an optional request-scoped game prompt.",
                authorization: None,
            },
            RouteInfo {
                method: "POST",
                path: "/game/:team/:player/turn",
                description: "Runs a model-directed game turn and persists its complete next game state.",
                authorization: None,
            },
            RouteInfo {
                method: "GET",
                path: "/game/:team/:player/state",
                description: "Returns the persisted game state for one player.",
                authorization: None,
            },
            RouteInfo {
                method: "POST",
                path: "/game/:team/:player/reset",
                description: "Clears one player's game state and game conversation history.",
                authorization: None,
            },
            RouteInfo {
                method: "GET",
                path: "/history/:team",
                description: "Returns recent persisted conversation history for a team.",
                authorization: None,
            },
            RouteInfo {
                method: "GET",
                path: "/teams/:team/prompt",
                description: "Reads a team's saved system prompt.",
                authorization: Some("Bearer OLLAMA_TEAM_PROMPT_TOKEN"),
            },
            RouteInfo {
                method: "POST",
                path: "/teams/:team/prompt",
                description: "Creates, updates, or clears a team's saved system prompt.",
                authorization: Some("Bearer OLLAMA_TEAM_PROMPT_TOKEN"),
            },
        ],
    }
}

fn json_error(status: tide::StatusCode, message: &str) -> tide::Response {
    json_response(status, json!({ "error": message }))
}

fn standard_chat_request(mut payload: Value, default_model: &str) -> Result<Value, String> {
    let Some(request) = payload.as_object_mut() else {
        return Err("request must be a JSON object".to_string());
    };
    if !request.get("messages").is_some_and(Value::is_array) {
        return Err("messages must be a JSON array".to_string());
    }
    if request.get("stream").and_then(Value::as_bool) == Some(true) {
        return Err("streaming is not supported on POST /ollama; set stream to false".to_string());
    }
    request.insert("stream".to_string(), Value::Bool(false));
    match request.get("model") {
        Some(Value::String(model)) if !model.trim().is_empty() => {}
        Some(_) => return Err("model must be a non-empty string".to_string()),
        None => {
            request.insert("model".to_string(), Value::String(default_model.to_string()));
        }
    }
    Ok(payload)
}

fn safe_team_name(team: &str) -> String {
    team
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn validate_team(team: &str) -> tide::Result<()> {
    validate_identifier(team, "team")
}

fn validate_player(player: &str) -> tide::Result<()> {
    validate_identifier(player, "player")
}

fn validate_identifier(value: &str, label: &str) -> tide::Result<()> {
    if value.is_empty() || value == "." || value == ".." || safe_team_name(value) != value {
        return Err(tide::Error::from_str(
            tide::StatusCode::BadRequest,
            format!("{label} must contain only letters, numbers, dots, hyphens, or underscores"),
        ));
    }
    Ok(())
}

fn game_history_key(team: &str, player: &str) -> String {
    format!("game_{}_player_{}", team, player)
}

fn game_state_key(team: &str, player: &str) -> String {
    format!("game_{}_player_{}_state", safe_team_name(team), safe_team_name(player))
}

fn team_prompt_authorization_error<State>(
    req: &tide::Request<State>,
    cfg: &RelayConfig,
) -> Option<tide::Response> {
    let Some(expected_token) = cfg.team_prompt_token.as_deref() else {
        return Some(json_error(
            tide::StatusCode::ServiceUnavailable,
            "team prompt management is disabled",
        ));
    };
    let authorization = req
        .header("Authorization")
        .and_then(|values| values.get(0))
        .map(|value| value.as_str());
    let expected_bearer = format!("Bearer {expected_token}");
    if authorization == Some(expected_bearer.as_str()) {
        None
    } else {
        Some(json_error(tide::StatusCode::Unauthorized, "invalid bearer token"))
    }
}

fn team_store_name(team: &str) -> String {
    format!("team_{}", safe_team_name(team))
}

fn team_prompt_store_name(team: &str) -> String {
    format!("prompt_{}", safe_team_name(team))
}

fn open_team_store(cfg: &RelayConfig) -> tide::Result<LineDb> {
    let store_root = cfg.team_log_dir.join("line_db");
    let database_list = store_root.join("db").join("db_list.txt");
    LineDb::new(
        &store_root,
        "db",
        &database_list.to_string_lossy(),
        LineDbConfig::default(),
    )
    .map_err(|error| tide::Error::from_str(tide::StatusCode::InternalServerError, error.to_string()))
}

fn team_store_lock(store: &Mutex<LineDb>) -> tide::Result<std::sync::MutexGuard<'_, LineDb>> {
    store.lock().map_err(|_| {
        tide::Error::from_str(
            tide::StatusCode::InternalServerError,
            "team history store is unavailable",
        )
    })
}

fn trim_text(text: &str, char_limit: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() <= char_limit {
        return trimmed.to_string();
    }
    chars[chars.len() - char_limit..].iter().collect()
}

fn load_team_entries(db: &mut LineDb, team: &str) -> tide::Result<Vec<TeamLogEntry>> {
    let store_name = team_store_name(team);
    if let Some(entries) = db
        .load_json_value::<Vec<TeamLogEntry>>(&store_name)
        .map_err(|error| tide::Error::from_str(tide::StatusCode::InternalServerError, error.to_string()))?
    {
        return Ok(entries);
    }
    Ok(Vec::new())
}

fn team_entries(store: &Mutex<LineDb>, team: &str) -> tide::Result<Vec<TeamLogEntry>> {
    let mut db = team_store_lock(store)?;
    load_team_entries(&mut db, team)
}

fn team_prompt(store: &Mutex<LineDb>, team: &str) -> tide::Result<Option<String>> {
    let mut db = team_store_lock(store)?;
    let prompt = db
        .load_json_value::<String>(&team_prompt_store_name(team))
        .map_err(|error| tide::Error::from_str(tide::StatusCode::InternalServerError, error.to_string()))?;
    Ok(prompt.filter(|value| !value.is_empty()))
}

fn save_team_prompt(store: &Mutex<LineDb>, team: &str, prompt: &str) -> tide::Result<usize> {
    let mut db = team_store_lock(store)?;
    db.save_json_value(&team_prompt_store_name(team), &prompt)
        .map_err(|error| tide::Error::from_str(tide::StatusCode::InternalServerError, error.to_string()))?;
    Ok(prompt.chars().count())
}

fn game_state(store: &Mutex<LineDb>, team: &str, player: &str) -> tide::Result<Value> {
    let mut db = team_store_lock(store)?;
    Ok(db
        .load_json_value::<Value>(&game_state_key(team, player))
        .map_err(|error| tide::Error::from_str(tide::StatusCode::InternalServerError, error.to_string()))?
        .unwrap_or_else(|| json!({})))
}

fn save_game_state(
    store: &Mutex<LineDb>,
    cfg: &RelayConfig,
    team: &str,
    player: &str,
    state: &Value,
) -> tide::Result<()> {
    let state_chars = serde_json::to_string(state)
        .map_err(|error| tide::Error::from_str(tide::StatusCode::BadRequest, error.to_string()))?
        .chars()
        .count();
    if state_chars > cfg.max_game_state_chars {
        return Err(tide::Error::from_str(
            tide::StatusCode::PayloadTooLarge,
            "state exceeds the configured size limit",
        ));
    }
    let mut db = team_store_lock(store)?;
    db.save_json_value(&game_state_key(team, player), state)
        .map_err(|error| tide::Error::from_str(tide::StatusCode::InternalServerError, error.to_string()))
}

fn reset_game_session(store: &Mutex<LineDb>, team: &str, player: &str) -> tide::Result<()> {
    let history_key = game_history_key(team, player);
    let mut db = team_store_lock(store)?;
    db.save_json_value(&team_store_name(&history_key), &Vec::<TeamLogEntry>::new())
        .map_err(|error| tide::Error::from_str(tide::StatusCode::InternalServerError, error.to_string()))?;
    db.save_json_value(&game_state_key(team, player), &json!({}))
        .map_err(|error| tide::Error::from_str(tide::StatusCode::InternalServerError, error.to_string()))
}

fn team_history(store: &Mutex<LineDb>, cfg: &RelayConfig, team: &str) -> tide::Result<String> {
    let entries = team_entries(store, team)?;
    let entries = entries
        .into_iter()
        .map(|entry| {
            if entry.user.is_empty() {
                format!("AI: {}", entry.reply)
            } else {
                format!("USER: {}\nAI: {}", entry.user, entry.reply)
            }
        })
        .collect::<Vec<_>>();

    let recent = entries
        .into_iter()
        .rev()
        .take(cfg.team_history_entry_limit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n\n");

    Ok(trim_text(&recent, cfg.team_history_char_limit))
}

fn append_team_log(
    store: &Mutex<LineDb>,
    cfg: &RelayConfig,
    team: &str,
    user_message: &str,
    reply: &str,
) -> tide::Result<()> {
    let mut db = team_store_lock(store)?;
    let mut entries = load_team_entries(&mut db, team)?;
    entries.push(TeamLogEntry {
        user: user_message.split_whitespace().collect::<Vec<_>>().join(" "),
        reply: reply.split_whitespace().collect::<Vec<_>>().join(" "),
    });
    let storage_limit = cfg.team_history_storage_limit.max(1);
    let excess_entries = entries.len().saturating_sub(storage_limit);
    if excess_entries > 0 {
        entries.drain(..excess_entries);
    }

    db.save_json_value(&team_store_name(team), &entries)
        .map_err(|error| tide::Error::from_str(tide::StatusCode::InternalServerError, error.to_string()))?;
    Ok(())
}

fn build_messages(
    store: &Mutex<LineDb>,
    cfg: &RelayConfig,
    team: &str,
    message: &str,
) -> tide::Result<Vec<Value>> {
    let history = team_history(store, cfg, team)?;

    let mut messages = vec![json!({
        "role": "system",
        "content": format!(
            "You are assisting team {}. Reply to the user's message directly. Keep replies concise, plain text, and useful.",
            team
        )
    })];

    if let Some(prompt) = team_prompt(store, team)? {
        messages.push(json!({
            "role": "system",
            "content": prompt,
        }));
    }

    if !history.is_empty() {
        messages.push(json!({
            "role": "system",
            "content": format!("Team conversation history:\n{}", history)
        }));
    }

    messages.push(json!({
        "role": "user",
        "content": message
    }));

    Ok(messages)
}

fn build_game_messages(
    store: &Mutex<LineDb>,
    cfg: &RelayConfig,
    team: &str,
    history_key: &str,
    player: &str,
    message: &str,
    game_prompt: &str,
) -> tide::Result<Vec<Value>> {
    let history = team_history(store, cfg, history_key)?;
    let mut messages = vec![json!({
        "role": "system",
        "content": format!(
            "You are the game assistant for team {team} and player {player}. Respond to the player's current game input. Keep responses useful, concise, and suitable for direct use by the game."
        )
    })];

    if let Some(prompt) = team_prompt(store, team)? {
        messages.push(json!({ "role": "system", "content": prompt }));
    }
    if !game_prompt.is_empty() {
        messages.push(json!({ "role": "system", "content": game_prompt }));
    }
    if !history.is_empty() {
        messages.push(json!({
            "role": "system",
            "content": format!("Player conversation history:\n{history}")
        }));
    }
    messages.push(json!({ "role": "user", "content": message }));
    Ok(messages)
}

fn build_game_turn_messages(
    store: &Mutex<LineDb>,
    cfg: &RelayConfig,
    team: &str,
    history_key: &str,
    player: &str,
    action: &str,
    game_prompt: &str,
    state: &Value,
) -> tide::Result<Vec<Value>> {
    let mut messages = build_game_messages(
        store, cfg, team, history_key, player, action, game_prompt,
    )?;
    messages.insert(1, json!({
        "role": "system",
        "content": format!(
            "You direct the next game turn. Return only valid JSON with narrative (string), state (the complete next state object), choices (array of objects with id and label), and game_over (boolean). Current state: {}",
            serde_json::to_string(state).unwrap_or_else(|_| "{}".to_string())
        )
    }));
    Ok(messages)
}

fn game_directive(reply: &str, current_state: &Value) -> Value {
    let text = reply.trim();
    let candidate = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .unwrap_or(text)
        .trim_end_matches("```")
        .trim();
    serde_json::from_str::<Value>(candidate)
        .ok()
        .filter(Value::is_object)
        .unwrap_or_else(|| {
            json!({
                "narrative": text,
                "state": current_state,
                "choices": [],
                "game_over": false,
            })
        })
}

async fn get_json(cfg: &RelayConfig, path: &str) -> tide::Result<Value> {
    let url = format!("{}{}", cfg.ollama_http_address, path);
    let mut res = async_std::future::timeout(
        Duration::from_secs(20),
        surf::get(url),
    )
    .await
    .map_err(|_| {
        tide::Error::from_str(
            tide::StatusCode::BadGateway,
            "timeout calling Ollama /api/tags",
        )
    })?
    .map_err(|e| tide::Error::from_str(tide::StatusCode::BadGateway, e.to_string()))?;

    if !res.status().is_success() {
        return Ok(json!({}));
    }

    let body = res
        .body_string()
        .await
        .map_err(|e| tide::Error::from_str(tide::StatusCode::BadGateway, e.to_string()))?;
    let parsed = serde_json::from_str::<Value>(&body).unwrap_or_else(|_| json!({ "raw_body": body }));
    Ok(parsed)
}

async fn post_json(cfg: &RelayConfig, path: &str, payload: Value) -> tide::Result<Value> {
    let url = format!("{}{}", cfg.ollama_http_address, path);
    let request = surf::post(url)
        .body_json(&payload)
        .map_err(|e| tide::Error::from_str(tide::StatusCode::BadGateway, e.to_string()))?;

    let mut res = async_std::future::timeout(Duration::from_secs(75), request)
        .await
        .map_err(|_| {
            tide::Error::from_str(
                tide::StatusCode::BadGateway,
                format!("timeout calling Ollama {}", path),
            )
        })?
        .map_err(|e| tide::Error::from_str(tide::StatusCode::BadGateway, e.to_string()))?;

    let status = res.status();
    let body = res
        .body_string()
        .await
        .map_err(|e| tide::Error::from_str(tide::StatusCode::BadGateway, e.to_string()))?;

    let parsed = serde_json::from_str::<Value>(&body).unwrap_or_else(|_| json!({ "raw_body": body }));

    if status.is_success() {
        Ok(parsed)
    } else {
        let msg = parsed
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("unknown ollama error")
            .to_string();
        Err(tide::Error::from_str(
            tide::StatusCode::BadGateway,
            format!("Ollama {} failed with HTTP {}: {}", path, status as u16, msg),
        ))
    }
}

async fn available_models(cfg: &RelayConfig) -> tide::Result<Vec<String>> {
    let payload = get_json(cfg, "/api/tags").await?;
    let models = payload
        .get("models")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let names = models
        .iter()
        .filter_map(|m| m.get("name").and_then(Value::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    Ok(names)
}

async fn resolve_model_name(cfg: &RelayConfig) -> tide::Result<(Option<String>, Vec<String>)> {
    let models = available_models(cfg).await?;
    if models.is_empty() {
        return Ok((None, models));
    }

    if models.contains(&cfg.ollama_model_name) {
        return Ok((Some(cfg.ollama_model_name.clone()), models));
    }

    let bare = cfg.ollama_model_name.trim_end_matches(":latest");
    if let Some(found) = models
        .iter()
        .find(|m| *m == bare || m.trim_end_matches(":latest") == bare)
    {
        return Ok((Some(found.clone()), models));
    }

    Ok((models.first().cloned(), models))
}

fn extract_reply(result: &Value) -> String {
    let candidates = [
        result
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(Value::as_str),
        result.get("response").and_then(Value::as_str),
        result.get("content").and_then(Value::as_str),
        result.get("message").and_then(Value::as_str),
    ];

    for maybe in candidates.into_iter().flatten() {
        let text = maybe.trim();
        if !text.is_empty() {
            return text.to_string();
        }
    }

    if let Some(done) = result.get("done_reason").and_then(Value::as_str) {
        let done = done.trim();
        if !done.is_empty() {
            return format!("Model returned no text output (done_reason={}).", done);
        }
    }

    "Model returned no text output. Please retry your message.".to_string()
}

fn no_text_reply(reply: &str) -> bool {
    reply.starts_with("Model returned no text output")
}

async fn generate_fallback(
    cfg: &RelayConfig,
    model_name: &str,
    user_message: &str,
) -> tide::Result<Value> {
    let prompt = format!(
        "Reply concisely and helpfully to this user message:\n{}",
        user_message
    );

    post_json(
        cfg,
        "/api/generate",
        json!({
            "model": model_name,
            "stream": false,
            "prompt": prompt,
        }),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn team_history_persists_in_partitioned_array_store() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let store_path = std::env::temp_dir().join(format!("tiade-ollama-relay-{unique}"));
        let mut config = RelayConfig::default();
        config.team_log_dir = store_path.clone();
        config.team_history_storage_limit = 2;

        let store = Mutex::new(open_team_store(&config).expect("open initial LineDb"));
        append_team_log(&store, &config, "alpha", "first message", "first reply")
            .expect("persist first team entry");
        append_team_log(&store, &config, "alpha", "hello\nthere", "general kenobi")
            .expect("persist second team entry");
        append_team_log(&store, &config, "alpha", "third message", "third reply")
            .expect("persist third team entry");

        let reopened = Mutex::new(open_team_store(&config).expect("reopen LineDb"));
        let history = team_history(&reopened, &config, "alpha").expect("read team history");
        assert_eq!(
            history,
            "USER: hello there\nAI: general kenobi\n\nUSER: third message\nAI: third reply"
        );

        std::fs::remove_dir_all(store_path).expect("remove temporary LineDb");
    }

    #[test]
    fn team_prompt_persists_and_is_injected_as_a_system_message() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let store_path = std::env::temp_dir().join(format!("tiade-ollama-prompt-{unique}"));
        let mut config = RelayConfig::default();
        config.team_log_dir = store_path.clone();

        let store = Mutex::new(open_team_store(&config).expect("open initial LineDb"));
        save_team_prompt(&store, "alpha", "Answer as a concise technical editor.")
            .expect("persist team prompt");

        let reopened = Mutex::new(open_team_store(&config).expect("reopen LineDb"));
        assert_eq!(
            team_prompt(&reopened, "alpha").expect("load team prompt"),
            Some("Answer as a concise technical editor.".to_string())
        );
        let messages = build_messages(&reopened, &config, "alpha", "Review this.")
            .expect("build Ollama messages");
        assert_eq!(
            messages[1]["content"].as_str(),
            Some("Answer as a concise technical editor.")
        );

        save_team_prompt(&reopened, "alpha", "").expect("clear team prompt");
        assert_eq!(team_prompt(&reopened, "alpha").expect("load cleared prompt"), None);
        std::fs::remove_dir_all(store_path).expect("remove temporary LineDb");
    }

    #[test]
    fn game_messages_layer_team_and_game_prompts_per_player() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let store_path = std::env::temp_dir().join(format!("tiade-ollama-game-{unique}"));
        let mut config = RelayConfig::default();
        config.team_log_dir = store_path.clone();
        let store = Mutex::new(open_team_store(&config).expect("open LineDb"));

        save_team_prompt(&store, "arcade", "Keep the campaign tone hopeful.")
            .expect("save team prompt");
        append_team_log(
            &store,
            &config,
            &game_history_key("arcade", "player-one"),
            "I inspect the gate.",
            "The gate is locked.",
        )
        .expect("save player-one history");
        let messages = build_game_messages(
            &store,
            &config,
            "arcade",
            &game_history_key("arcade", "player-one"),
            "player-one",
            "I try the key.",
            "The player has a brass key and one torch.",
        )
        .expect("build game messages");

        assert_eq!(messages[1]["content"].as_str(), Some("Keep the campaign tone hopeful."));
        assert_eq!(messages[2]["content"].as_str(), Some("The player has a brass key and one torch."));
        assert!(messages[3]["content"].as_str().unwrap_or_default().contains("I inspect the gate."));
        assert_eq!(messages[4]["content"].as_str(), Some("I try the key."));
        assert!(team_history(&store, &config, &game_history_key("arcade", "player-two"))
            .expect("read player-two history")
            .is_empty());

        std::fs::remove_dir_all(store_path).expect("remove temporary LineDb");
    }

    #[test]
    fn game_state_persists_per_player_and_reset_clears_it() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let store_path = std::env::temp_dir().join(format!("tiade-ollama-game-state-{unique}"));
        let mut config = RelayConfig::default();
        config.team_log_dir = store_path.clone();
        let store = Mutex::new(open_team_store(&config).expect("open LineDb"));
        let saved_state = json!({"scene": "north-gate", "inventory": ["brass-key"]});

        save_game_state(&store, &config, "arcade", "player-one", &saved_state)
            .expect("save player state");
        let reopened = Mutex::new(open_team_store(&config).expect("reopen LineDb"));
        assert_eq!(
            game_state(&reopened, "arcade", "player-one").expect("load player state"),
            saved_state
        );
        assert_eq!(
            game_state(&reopened, "arcade", "player-two").expect("load isolated player state"),
            json!({})
        );

        reset_game_session(&reopened, "arcade", "player-one").expect("reset player session");
        assert_eq!(
            game_state(&reopened, "arcade", "player-one").expect("load reset state"),
            json!({})
        );
        std::fs::remove_dir_all(store_path).expect("remove temporary LineDb");
    }

    #[test]
    fn team_names_must_be_safe_and_non_empty() {
        assert!(validate_team("alpha-team_2.0").is_ok());
        assert!(validate_team("").is_err());
        assert!(validate_team("../other").is_err());
        assert!(validate_team("two words").is_err());
    }

    #[test]
    fn cors_preflight_allows_browser_api_requests() {
        let response = cors_preflight_response();

        assert_eq!(response.status(), tide::StatusCode::NoContent);
        assert_eq!(
            response
                .header("Access-Control-Allow-Origin")
                .and_then(|values| values.get(0))
                .map(|value| value.as_str()),
            Some("*")
        );
        assert_eq!(
            response
                .header("Access-Control-Allow-Headers")
                .and_then(|values| values.get(0))
                .map(|value| value.as_str()),
            Some("Authorization, Content-Type")
        );
    }

    #[test]
    fn general_route_catalog_lists_the_generic_ollama_api() {
        let catalog = route_catalog();
        let routes = catalog
            .routes
            .iter()
            .map(|route| (route.method, route.path))
            .collect::<Vec<_>>();

        assert_eq!(catalog.version, RELAY_VERSION);
        assert!(routes.contains(&("GET", "/ollama")));
        assert!(routes.contains(&("POST", "/ollama")));
        assert!(routes.contains(&("GET", "/ollama/health")));
        assert!(routes.contains(&("POST", "/chat/:team")));
        assert!(routes.contains(&("POST", "/game/:team/:player")));
        assert!(routes.contains(&("POST", "/game/:team/:player/turn")));
        assert!(routes.contains(&("GET", "/game/:team/:player/state")));
        assert!(routes.contains(&("POST", "/game/:team/:player/reset")));
        assert!(routes.contains(&("GET", "/history/:team")));
        assert!(routes.contains(&("GET", "/teams/:team/prompt")));
        assert!(routes.contains(&("POST", "/teams/:team/prompt")));
    }

    #[test]
    fn standard_ollama_route_defaults_model_and_rejects_streaming() {
        let request = standard_chat_request(json!({
            "messages": [{ "role": "user", "content": "Hello" }],
        }), "game-model:latest")
        .expect("normalize standard Ollama request");

        assert_eq!(request["model"], "game-model:latest");
        assert_eq!(request["stream"], false);
        assert!(standard_chat_request(json!({
            "messages": [],
            "stream": true,
        }), "game-model:latest")
        .is_err());
    }

    #[test]
    fn general_route_serves_the_catalog_to_http_clients() {
        async_std::task::block_on(async {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is after Unix epoch")
                .as_nanos();
            let store_path = std::env::temp_dir().join(format!("tiade-ollama-route-{unique}"));
            let mut config = RelayConfig::default();
            config.team_log_dir = store_path.clone();

            let mut app: tide::Server<()> = tide::new();
            mount_routes(&mut app, config).expect("mount relay routes");
            let request = tide::http::Request::new(
                tide::http::Method::Get,
                tide::http::Url::parse("http://localhost/ollama").expect("parse URL"),
            );
            let mut response: tide::Response = app.respond(request).await.expect("serve route");

            assert_eq!(response.status(), tide::StatusCode::Ok);
            assert_eq!(
                response
                    .header("Access-Control-Allow-Origin")
                    .and_then(|values| values.get(0))
                    .map(|value| value.as_str()),
                Some("*")
            );
            let body = response.take_body().into_string().await.expect("read catalog body");
            let catalog: Value = serde_json::from_str(&body).expect("decode catalog JSON");
            assert_eq!(catalog["service"], "ollama-team-relay");
            assert!(catalog["routes"].as_array().expect("route array").len() >= 7);

            std::fs::remove_dir_all(store_path).expect("remove temporary LineDb");
        });
    }

}
