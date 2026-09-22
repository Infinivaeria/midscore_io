use js_sys::JSON;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, RequestMode, Response};

fn endpoint(server_url: &str, path: &str) -> String {
    format!("{}/{}", server_url.trim_end_matches('/'), path.trim_start_matches('/'))
}

async fn request_json(
    server_url: &str,
    path: &str,
    method: &str,
    body: Option<String>,
) -> Result<JsValue, JsValue> {
    let init = RequestInit::new();
    init.set_method(method);
    init.set_mode(RequestMode::Cors);

    let headers = Headers::new()?;
    headers.set("Accept", "application/json")?;
    if let Some(body) = body {
        headers.set("Content-Type", "application/json")?;
        init.set_body(&JsValue::from_str(&body));
    }
    init.set_headers(&headers);

    let request = Request::new_with_str_and_init(&endpoint(server_url, path), &init)?;
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("browser window unavailable"))?;
    let response = JsFuture::from(window.fetch_with_request(&request))
        .await?
        .dyn_into::<Response>()?;
    let status = response.status();
    let text = JsFuture::from(response.text()?)
        .await?
        .as_string()
        .unwrap_or_default();

    if !response.ok() {
        return Err(JsValue::from_str(&format!("relay returned HTTP {status}: {text}")));
    }
    JSON::parse(&text)
}

#[wasm_bindgen]
pub async fn routes(server_url: String) -> Result<JsValue, JsValue> {
    request_json(&server_url, "/ollama", "GET", None).await
}

#[wasm_bindgen]
pub async fn health(server_url: String) -> Result<JsValue, JsValue> {
    request_json(&server_url, "/ollama/health", "GET", None).await
}

#[wasm_bindgen]
pub async fn chat(server_url: String, team: String, message: String) -> Result<JsValue, JsValue> {
    let payload = js_sys::Object::new();
    js_sys::Reflect::set(&payload, &JsValue::from_str("message"), &JsValue::from_str(&message))?;
    let body = JSON::stringify(&payload)?.as_string().unwrap_or_default();
    request_json(&server_url, &format!("/chat/{team}"), "POST", Some(body)).await
}

#[wasm_bindgen]
pub async fn game_turn(
    server_url: String,
    team: String,
    player: String,
    message: String,
    game_prompt: String,
) -> Result<JsValue, JsValue> {
    let payload = js_sys::Object::new();
    js_sys::Reflect::set(&payload, &JsValue::from_str("action"), &JsValue::from_str(&message))?;
    if !game_prompt.trim().is_empty() {
        js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("game_prompt"),
            &JsValue::from_str(&game_prompt),
        )?;
    }
    let body = JSON::stringify(&payload)?.as_string().unwrap_or_default();
    request_json(
        &server_url,
        &format!("/game/{team}/{player}/turn"),
        "POST",
        Some(body),
    )
    .await
}

#[wasm_bindgen]
pub async fn history(server_url: String, team: String) -> Result<JsValue, JsValue> {
    request_json(&server_url, &format!("/history/{team}"), "GET", None).await
}