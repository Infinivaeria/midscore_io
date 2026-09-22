use crate::AppState;
use chrono::{Duration, Timelike, Utc};
use image::{DynamicImage, ImageFormat, imageops::FilterType};
use partitioned_array_rust::{LineDb, LineDbConfig};
use pulldown_cmark::{Options as MarkdownOptions, Parser, html};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::fmt::Write as _;
use std::sync::{Mutex, OnceLock};
use tide::{Request, Response, StatusCode};
use uuid::Uuid;

const BLOG_STORE_PATH: &str = "/root/midscore_io/logs/tiade_roda_compat/blog_store.json";
const GALLERY_STORE_PATH: &str = "/root/midscore_io/logs/tiade_roda_compat/gallery_store.json";
const ADMIN_PASSWORD: &str = "gUilmon95458a";
const BLOG_SUPER_PASSWORD: &str = "gUilmon#95458a";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlogPost {
    id: usize,
    title: String,
    body: String,
    body_markdown: Option<String>,
    tags: Vec<String>,
    date: String,
    author: String,
    comments: String,
    status: String,
    locked: bool,
    rendered_type: String,
    timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BlogUser {
    password: String,
    private_view: bool,
    page_views: u64,
    pinned: Option<BlogPost>,
    posts: Vec<BlogPost>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BlogStore {
    users: HashMap<String, BlogUser>,
    sessions: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GalleryAttachment {
    id: usize,
    value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GalleryPost {
    id: usize,
    title: String,
    body: String,
    tags: Vec<String>,
    attachments: Vec<GalleryAttachment>,
    created_at: String,
    views: Option<u64>,
    file: Option<String>,
    extension: Option<String>,
    size: Option<usize>,
    sum_identifier: Option<u64>,
    thumbnail_file: Option<String>,
    resized_file: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct GalleryStore {
    users: HashMap<String, Vec<GalleryPost>>,
    uwu_collections: HashMap<usize, Vec<(String, usize)>>,
    owo_value: i64,
}

static BLOG_STORE: OnceLock<Mutex<BlogStore>> = OnceLock::new();
static GALLERY_STORE: OnceLock<Mutex<GalleryStore>> = OnceLock::new();
static LINEDB: OnceLock<Mutex<LineDb>> = OnceLock::new();

fn load_json_or_default<T: for<'a> Deserialize<'a> + Default>(path: &str) -> T {
    let table = table_name_for_path(path);
    if let Ok(mut linedb_guard) = linedb().lock() {
        if let Ok(Some(value)) = linedb_guard.load_json_value::<T>(&table) {
            return value;
        }
    }

    T::default()
}

fn save_json<T: Serialize>(path: &str, value: &T) {
    let table = table_name_for_path(path);
    if let Ok(mut linedb_guard) = linedb().lock() {
        let _ = linedb_guard.save_json_value(&table, value);
    }
}

fn table_name_for_path(path: &str) -> String {
    let name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("default_store");
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn linedb() -> &'static Mutex<LineDb> {
    LINEDB.get_or_init(|| {
        let primary = LineDb::new(
            "/root/midscore_io/logs/tiade_roda_compat/line_db",
            "db",
            "/root/midscore_io/logs/tiade_roda_compat/line_db/db/db_list.txt",
            LineDbConfig::default(),
        );
        let db = primary.unwrap_or_else(|_| {
            LineDb::new(
                "/tmp/tiade_line_db",
                "db",
                "/tmp/tiade_line_db/db/db_list.txt",
                LineDbConfig::default(),
            )
            .expect("fallback linedb path should be creatable")
        });
        Mutex::new(db)
    })
}

fn blog_store() -> &'static Mutex<BlogStore> {
    BLOG_STORE.get_or_init(|| Mutex::new(load_json_or_default(BLOG_STORE_PATH)))
}

fn gallery_store() -> &'static Mutex<GalleryStore> {
    GALLERY_STORE.get_or_init(|| Mutex::new(load_json_or_default(GALLERY_STORE_PATH)))
}

fn html_response(body: String) -> tide::Result {
    let mut resp = Response::new(StatusCode::Ok);
    resp.set_body(body);
    resp.insert_header("Content-Type", "text/html; charset=utf-8");
    Ok(resp)
}

fn html_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn page_shell(title: &str, body: &str, extra_head: &str, extra_js: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title><link rel=\"stylesheet\" href=\"/assets/style.css\"><link rel=\"stylesheet\" href=\"/assets/style2.css\">{}<style>body{{max-width:980px;margin:24px auto;padding:0 16px}}.bar{{display:flex;gap:12px;flex-wrap:wrap;margin:12px 0 24px}}.card{{border:1px solid #ccc;border-radius:12px;padding:12px;margin:10px 0}}.muted{{opacity:.75}}textarea{{width:100%;min-height:160px}}input[type=text],input[type=password],input[type=url]{{width:100%;max-width:760px}}</style></head><body><header><h1>{}</h1><nav class=\"bar\"><a href=\"/blog/login\">blog login</a><a href=\"/blog/signup\">blog signup</a><a href=\"/gallery\">gallery</a><a href=\"/admin\">admin</a><a href=\"/moon\">moon</a><a href=\"/sun\">sun</a></nav></header><main>{}</main>{}<script>function copyValue(id){{const el=document.getElementById(id);if(!el)return;navigator.clipboard.writeText(el.value||el.textContent||'');}}</script></body></html>",
        html_escape(title),
        extra_head,
        html_escape(title),
        body,
        extra_js
    )
}

fn set_cookie(resp: &mut Response, name: &str, value: &str) {
    let cookie = format!("{}={}; Path=/; HttpOnly; SameSite=Lax", name, value);
    resp.append_header("Set-Cookie", cookie);
}

fn get_cookie(req: &Request<AppState>, name: &str) -> Option<String> {
    let raw = req.header("Cookie")?.get(0)?.as_str();
    for pair in raw.split(';') {
        let mut kv = pair.trim().splitn(2, '=');
        let k = kv.next()?.trim();
        let v = kv.next().unwrap_or("").trim();
        if k == name {
            return Some(v.to_string());
        }
    }
    None
}

fn safe_join(base: &str, rel: &str) -> Option<String> {
    let clean = rel.trim_start_matches('/').trim_start_matches("./");
    if clean.is_empty() || clean.contains("..") || clean.contains('\\') {
        return None;
    }
    Some(format!("{}/{}", base.trim_end_matches('/'), clean))
}

fn mime_for_path(path: &str) -> &'static str {
    if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".gif") {
        "image/gif"
    } else if path.ends_with(".bmp") {
        "image/bmp"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".wasm") {
        "application/wasm"
    } else {
        "application/octet-stream"
    }
}

fn static_file_response(base: &str, rel: &str) -> tide::Result {
    let path = match safe_join(base, rel) {
        Some(v) => v,
        None => {
            return json_response(StatusCode::BadRequest, json!({"error": "invalid static path"}));
        }
    };
    let bytes = match fs::read(&path) {
        Ok(v) => v,
        Err(_) => {
            return json_response(StatusCode::NotFound, json!({"error": "file not found", "path": path}));
        }
    };
    let mut resp = Response::new(StatusCode::Ok);
    resp.set_body(bytes);
    resp.insert_header("Content-Type", mime_for_path(&path));
    Ok(resp)
}

fn text_response(body: String) -> tide::Result {
    let mut resp = Response::new(StatusCode::Ok);
    resp.set_body(body);
    resp.insert_header("Content-Type", "text/plain; charset=utf-8");
    Ok(resp)
}

fn json_response(status: StatusCode, value: serde_json::Value) -> tide::Result {
    let mut resp = Response::new(status);
    resp.set_body(value.to_string());
    resp.insert_header("Content-Type", "application/json; charset=utf-8");
    Ok(resp)
}

fn redirect_response(location: &str) -> tide::Result {
    let mut resp = Response::new(StatusCode::Found);
    resp.insert_header("Location", location);
    Ok(resp)
}

fn split_tags(tags: &str) -> Vec<String> {
    tags.split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn now_string() -> String {
    Utc::now().to_rfc3339()
}

fn render_markdown_to_html(input: &str) -> String {
    let mut options = MarkdownOptions::empty();
    options.insert(MarkdownOptions::ENABLE_TABLES);
    options.insert(MarkdownOptions::ENABLE_STRIKETHROUGH);
    options.insert(MarkdownOptions::ENABLE_FOOTNOTES);
    options.insert(MarkdownOptions::ENABLE_TASKLISTS);
    options.insert(MarkdownOptions::ENABLE_HEADING_ATTRIBUTES);
    let parser = Parser::new_ext(input, options);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

fn blog_post_body_html(post: &BlogPost) -> String {
    let mode = post.rendered_type.to_ascii_lowercase();
    if mode == "markdown" {
        let src = post
            .body_markdown
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(post.body.as_str());
        render_markdown_to_html(src)
    } else {
        post.body.clone()
    }
}

fn blog_post_page_html(user: &str, post: &BlogPost) -> String {
    let body_html = blog_post_body_html(post);
    let body = format!(
        "<article class=\"card\"><h2>{}</h2><p class=\"muted\"><b>User:</b> {} | <b>ID:</b> {} | <b>Published:</b> {}</p><div>{}</div><p class=\"muted\">tags: {}</p></article>",
        html_escape(&post.title),
        html_escape(user),
        post.id,
        html_escape(&post.timestamp),
        body_html,
        html_escape(&post.tags.join(", "))
    );
    page_shell(
        &post.title,
        &body,
        "<link rel=\"stylesheet\" type=\"text/css\" href=\"/assets/css/prism.css\">",
        "<script src=\"/assets/js/prism.js\"></script>",
    )
}

fn admin_page_html(databases: &[String], active_database: &str, admin_enabled: bool) -> String {
    let mut list = String::new();
    for db in databases {
        let _ = write!(
            &mut list,
            "<li><b>{}</b> <a href=\"/admin/remove/{}\">remove</a></li>",
            html_escape(db),
            html_escape(db)
        );
    }
    let body = format!(
        "<p class=\"muted\">admin session: {}</p><p>active db: <b>{}</b></p><section class=\"card\"><h2>Databases</h2><ul>{}</ul></section><section class=\"card\"><h2>Create Database</h2><form method=\"post\" action=\"/admin/add\"><input type=\"text\" name=\"db_name\" placeholder=\"database name\" required><button type=\"submit\">add</button></form></section><section class=\"card\"><h2>Delete Database</h2><form method=\"post\" action=\"/admin/delete\"><input type=\"text\" name=\"db_name\" placeholder=\"database name\" required><button type=\"submit\">delete</button></form></section>",
        if admin_enabled { "enabled" } else { "disabled" },
        html_escape(active_database),
        list
    );
    page_shell("Admin", &body, "", "")
}

fn blog_login_page_html() -> String {
    page_shell(
        "Blog Login",
        "<section class=\"card\"><h2>Login</h2><form method=\"post\" action=\"/blog/login\"><label>User</label><input type=\"text\" name=\"blog_user_name\" required><label>Password</label><input type=\"password\" name=\"blog_password_name\" required><label>Super Password</label><input type=\"password\" name=\"super_password\" required><button type=\"submit\">login</button></form></section>",
        "",
        "",
    )
}

fn gallery_home_page_html(users: &[String]) -> String {
    let mut items = String::new();
    for user in users {
        let _ = write!(
            &mut items,
            "<li><a href=\"/gallery/view/{}\">{}</a></li>",
            html_escape(user),
            html_escape(user)
        );
    }
    let body = format!(
        "<section class=\"card\"><h2>Gallery Users</h2><ul>{}</ul></section><section class=\"card\"><p><a href=\"/gallery/upload\">Upload binary</a> | <a href=\"/gallery/upload/url\">Upload from URL</a></p></section>",
        items
    );
    page_shell("Gallery", &body, "", "")
}

fn blog_signup_page_html() -> String {
    page_shell(
        "Blog Signup",
        "<section class=\"card\"><h2>Create Account</h2><form method=\"post\" action=\"/blog/signup\"><label>User</label><input type=\"text\" name=\"blog_user_name\" required><label>Password</label><input type=\"password\" name=\"blog_password_name\" required><button type=\"submit\">signup</button></form></section>",
        "",
        "",
    )
}

fn blog_index_page_html(user: &str, posts: &[BlogPost], private_view: bool) -> String {
    let mut cards = String::new();
    for post in posts {
        let preview = if post.body.len() > 220 {
            format!("{}...", &post.body[..220])
        } else {
            post.body.clone()
        };
        let _ = write!(
            &mut cards,
            "<article class=\"card\"><h3><a href=\"/blog/{}/view/{}\">{}</a></h3><p class=\"muted\">{}</p><p>{}</p><p class=\"muted\">tags: {}</p></article>",
            html_escape(user),
            post.id,
            html_escape(&post.title),
            html_escape(&post.timestamp),
            html_escape(&preview),
            html_escape(&post.tags.join(", "))
        );
    }
    let body = format!(
        "<div class=\"bar\"><a href=\"/blog/{}/new\">new post</a><a href=\"/blog/{}/list\">list</a><a href=\"/blog/{}/private_toggle\">toggle private ({})</a><a href=\"/blog/logout\">logout</a></div>{}",
        html_escape(user),
        html_escape(user),
        html_escape(user),
        if private_view { "on" } else { "off" },
        cards
    );
    page_shell(&format!("Blog {}", user), &body, "", "")
}

fn blog_editor_page_html(user: &str, post: Option<&BlogPost>) -> String {
    let (id, title, body, md, tags, date, author, comments, status, rendered) = if let Some(p) = post {
        (
            p.id.to_string(),
            p.title.clone(),
            p.body.clone(),
            p.body_markdown.clone().unwrap_or_default(),
            p.tags.join(", "),
            p.date.clone(),
            p.author.clone(),
            p.comments.clone(),
            p.status.clone(),
            p.rendered_type.clone(),
        )
    } else {
        (
            "new".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            user.to_string(),
            String::new(),
            String::new(),
            "wysiwyg".to_string(),
        )
    };

    let action = if id == "new" {
        format!("/blog/{}/new", html_escape(user))
    } else {
        format!("/blog/{}/edit/{}", html_escape(user), html_escape(&id))
    };

    let body_html = format!(
        "<section class=\"card\"><h2>Post Editor</h2><form method=\"post\" action=\"{}\"><label>Title</label><input type=\"text\" name=\"blog_post_title\" value=\"{}\" required><label>Body (HTML)</label><textarea name=\"blog_post_body\">{}</textarea><label>Body (Markdown)</label><textarea name=\"blog_post_body_markdown\">{}</textarea><label>Tags (comma)</label><input type=\"text\" name=\"blog_post_tags\" value=\"{}\"><label>Date</label><input type=\"text\" name=\"blog_post_date\" value=\"{}\"><label>Author</label><input type=\"text\" name=\"blog_post_author\" value=\"{}\"><label>Comments</label><input type=\"text\" name=\"blog_post_comments\" value=\"{}\"><label>Status</label><input type=\"text\" name=\"blog_post_status\" value=\"{}\"><label>Render Type</label><input type=\"text\" name=\"rendered_type\" value=\"{}\"><button type=\"submit\">save</button></form></section>",
        action,
        html_escape(&title),
        html_escape(&body),
        html_escape(&md),
        html_escape(&tags),
        html_escape(&date),
        html_escape(&author),
        html_escape(&comments),
        html_escape(&status),
        html_escape(&rendered)
    );
    page_shell(&format!("Edit Blog {}", user), &body_html, "", "")
}

fn gallery_index_page_html(user: &str, posts: &[GalleryPost]) -> String {
    let mut cards = String::new();
    for post in posts {
        let image = post
            .thumbnail_file
            .as_ref()
            .or(post.file.as_ref())
            .map(|f| format!("/public/gallery_index/{}/{}", user, f))
            .unwrap_or_default();
        let img_html = if image.is_empty() {
            String::new()
        } else {
            format!("<img src=\"{}\" style=\"max-width:260px;border-radius:10px\" alt=\"thumb\">", html_escape(&image))
        };
        let _ = write!(
            &mut cards,
            "<article class=\"card\">{}<h3><a href=\"/gallery/view/{}/id/{}\">{}</a></h3><p class=\"muted\">{}</p><p>{}</p></article>",
            img_html,
            html_escape(user),
            post.id,
            html_escape(&post.title),
            html_escape(&post.created_at),
            html_escape(&post.tags.join(", "))
        );
    }
    let body = format!(
        "<div class=\"bar\"><a href=\"/gallery/upload\">upload bytes</a><a href=\"/gallery/upload/url\">upload from url</a><a href=\"/gallery/view/{}/tags\">tags</a></div>{}",
        html_escape(user),
        cards
    );
    page_shell(&format!("Gallery {}", user), &body, "", "")
}

fn gallery_upload_page_html() -> String {
    let body = "<section class=\"card\"><h2>Binary Upload</h2><p>Send raw bytes to /gallery/upload with query params user, filename, title, description, tags.</p><p class=\"muted\">Tip: curl --data-binary @img.jpg 'https://host/gallery/upload?user=me&filename=img.jpg&title=hello&description=test&tags=a,b'</p></section>";
    page_shell("Gallery Upload", body, "", "")
}

fn gallery_upload_url_page_html() -> String {
    let body = "<section class=\"card\"><h2>URL Upload</h2><form method=\"post\" action=\"/gallery/upload/url\"><label>User</label><input type=\"text\" name=\"user\" value=\"gallery\"><label>Image URL</label><input type=\"url\" name=\"url\" required><label>Title</label><input type=\"text\" name=\"title\"><label>Description</label><input type=\"text\" name=\"description\"><label>Tags</label><input type=\"text\" name=\"tags\" placeholder=\"tag1,tag2\"><button type=\"submit\">fetch and save</button></form></section>";
    page_shell("Gallery URL Upload", body, "", "")
}

fn get_session_token(req: &Request<AppState>) -> Option<String> {
    req.header("x-blog-session")
        .and_then(|vals| vals.get(0))
        .map(|v| v.as_str().to_string())
        .or_else(|| get_cookie(req, "blog_session"))
        .or_else(|| req.url().query_pairs().find_map(|(k, v)| {
            if k == "session" {
                Some(v.to_string())
            } else {
                None
            }
        }))
}

fn current_blog_user(req: &Request<AppState>) -> Option<String> {
    let token = get_session_token(req)?;
    let store = blog_store().lock().ok()?;
    store.sessions.get(&token).cloned()
}

fn moon_text() -> String {
    const CYCLE_DAYS: f64 = 29.53;
    const PHASES: [&str; 15] = [
        "New Moon",
        "Waxing Crescent",
        "First Quarter",
        "Waxing Gibbous",
        "Full Moon",
        "Waning Gibbous",
        "Last Quarter",
        "Waning Crescent",
        "Supermoon",
        "Blue Moon",
        "Blood Moon",
        "Harvest Moon",
        "Hunter's Moon",
        "Wolf Moon",
        "Pink Moon",
    ];
    const SPECIES: [&str; 15] = [
        "Dogg",
        "Folf",
        "Aardwolf",
        "Spotted Hyena",
        "Folf Hybrid",
        "Striped Hyena",
        "Dogg Prime",
        "WolfFox",
        "Brown Hyena",
        "Dogg Celestial",
        "Folf Eclipse",
        "Aardwolf Luminous",
        "Spotted Hyena Stellar",
        "Folf Nova",
        "Brown Hyena Cosmic",
    ];
    const WERE_FORMS: [&str; 15] = [
        "WereDogg",
        "WereFolf",
        "WereAardwolf",
        "WereSpottedHyena",
        "WereFolfHybrid",
        "WereStripedHyena",
        "WereDoggPrime",
        "WereWolfFox",
        "WereBrownHyena",
        "WereDoggCelestial",
        "WereFolfEclipse",
        "WereAardwolfLuminous",
        "WereSpottedHyenaStellar",
        "WereFolfNova",
        "WereBrownHyenaCosmic",
    ];

    let reference = chrono::NaiveDate::from_ymd_opt(2000, 1, 6).expect("valid date");
    let today = Utc::now().date_naive();
    let days_since = (today - reference).num_days() as f64;
    let phase_len = CYCLE_DAYS / PHASES.len() as f64;
    let position = days_since.rem_euclid(CYCLE_DAYS);
    let phase_raw = position / phase_len;
    let phase_index = phase_raw.floor() as usize % PHASES.len();
    let consciousness = (phase_raw / (PHASES.len() - 1) as f64) * 100.0;

    format!(
        "Moon Phase: {}\nSpecies: {}\nWere-Form: {}\nConsciousness: {}/{} ({:.2}%)\n",
        PHASES[phase_index],
        SPECIES[phase_index],
        WERE_FORMS[phase_index],
        phase_raw,
        PHASES.len() - 1,
        consciousness
    )
}

fn sun_text() -> String {
    let phases = [
        (0, "Midnight Mystery", "moon"),
        (3, "Dawn's Whisper", "sunrise"),
        (5, "First Light's Murmur", "light"),
        (6, "Golden Awakening", "sun"),
        (8, "Morning Glow", "sun"),
        (12, "High Noon Radiance", "flame"),
        (15, "Afternoon Brilliance", "horizon"),
        (17, "Golden Hour Serenade", "cityscape"),
        (18, "Twilight Poetry", "crescent"),
        (19, "Dusky Secrets", "quarter"),
        (20, "Crimson Horizon", "gibbous"),
        (21, "Moon's Ascent", "full"),
        (22, "Nightfall's Caress", "sparkles"),
        (23, "Deep Celestial Silence", "night"),
    ];

    let pst = Utc::now() - Duration::hours(8);
    let hour = pst.hour() as i32;
    let mut current = phases[0];
    for phase in phases {
        if hour >= phase.0 {
            current = phase;
        }
    }

    format!("The Sun is currently in '{}' phase ({})", current.1, current.2)
}

fn parse_form(body: Result<HashMap<String, String>, tide::Error>) -> HashMap<String, String> {
    body.unwrap_or_default()
}

#[derive(Debug, Clone)]
struct MultipartFile {
    filename: String,
    content_type: Option<String>,
    data: Vec<u8>,
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn split_by_slice<'a>(haystack: &'a [u8], sep: &[u8]) -> Vec<&'a [u8]> {
    let mut out = Vec::new();
    let mut start = 0usize;
    while let Some(pos) = find_subslice(&haystack[start..], sep) {
        let at = start + pos;
        out.push(&haystack[start..at]);
        start = at + sep.len();
    }
    out.push(&haystack[start..]);
    out
}

fn extract_disposition_attr(disposition: &str, key: &str) -> Option<String> {
    let pattern = format!("{}=\"", key);
    let start = disposition.find(&pattern)? + pattern.len();
    let rest = &disposition[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn parse_multipart_form_data(
    content_type: &str,
    body: &[u8],
) -> (HashMap<String, String>, HashMap<String, MultipartFile>) {
    let mut fields = HashMap::new();
    let mut files = HashMap::new();

    let boundary = content_type
        .split(';')
        .find_map(|p| {
            let t = p.trim();
            t.strip_prefix("boundary=")
                .map(|v| v.trim_matches('"').to_string())
        })
        .unwrap_or_default();

    if boundary.is_empty() {
        return (fields, files);
    }

    let marker = format!("--{}", boundary);
    let parts = split_by_slice(body, marker.as_bytes());

    for mut part in parts {
        if part.is_empty() {
            continue;
        }
        if part.starts_with(b"\r\n") {
            part = &part[2..];
        }
        if part == b"--" || part == b"--\r\n" {
            continue;
        }
        if part.ends_with(b"\r\n") {
            part = &part[..part.len().saturating_sub(2)];
        }
        if part.ends_with(b"--") {
            part = &part[..part.len().saturating_sub(2)];
        }

        let Some(header_end) = find_subslice(part, b"\r\n\r\n") else {
            continue;
        };
        let header_bytes = &part[..header_end];
        let mut data = part[header_end + 4..].to_vec();
        while data.ends_with(b"\r") || data.ends_with(b"\n") {
            data.pop();
        }

        let headers = String::from_utf8_lossy(header_bytes);
        let mut name = None::<String>;
        let mut filename = None::<String>;
        let mut part_content_type = None::<String>;
        for line in headers.lines() {
            let lower = line.to_ascii_lowercase();
            if lower.starts_with("content-disposition:") {
                name = extract_disposition_attr(line, "name");
                filename = extract_disposition_attr(line, "filename");
            } else if lower.starts_with("content-type:") {
                part_content_type = line
                    .split_once(':')
                    .map(|(_, v)| v.trim().to_string())
                    .filter(|v| !v.is_empty());
            }
        }

        let Some(name) = name else {
            continue;
        };

        if let Some(filename) = filename {
            files.insert(
                name,
                MultipartFile {
                    filename: sanitize_filename(&filename),
                    content_type: part_content_type,
                    data,
                },
            );
        } else {
            let value = String::from_utf8_lossy(&data).trim().to_string();
            fields.insert(name, value);
        }
    }

    (fields, files)
}

fn gallery_pref_usize(
    req: &Request<AppState>,
    query: &HashMap<String, String>,
    query_key: &str,
    cookie_key: &str,
    default_value: usize,
    min_value: usize,
) -> usize {
    query
        .get(query_key)
        .and_then(|v| v.parse::<usize>().ok())
        .or_else(|| get_cookie(req, cookie_key).and_then(|v| v.parse::<usize>().ok()))
        .unwrap_or(default_value)
        .max(min_value)
}

fn sanitize_filename(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("upload.bin");
    base.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn normalize_supported_ext(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("jpg"),
        "png" => Some("png"),
        "bmp" => Some("bmp"),
        "gif" => Some("gif"),
        _ => None,
    }
}

fn image_format_for_ext(ext: &str) -> Option<ImageFormat> {
    match ext {
        "jpg" => Some(ImageFormat::Jpeg),
        "png" => Some(ImageFormat::Png),
        "bmp" => Some(ImageFormat::Bmp),
        "gif" => Some(ImageFormat::Gif),
        _ => None,
    }
}

fn mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "jpg" => "image/jpeg",
        "png" => "image/png",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

fn ext_from_content_type(content_type: &str) -> Option<&'static str> {
    let ct = content_type.to_ascii_lowercase();
    if ct.contains("image/jpeg") || ct.contains("image/jpg") {
        Some("jpg")
    } else if ct.contains("image/png") {
        Some("png")
    } else if ct.contains("image/bmp") {
        Some("bmp")
    } else if ct.contains("image/gif") {
        Some("gif")
    } else {
        None
    }
}

fn encode_image_to_ext(image: &DynamicImage, ext: &str) -> tide::Result<Vec<u8>> {
    let format = image_format_for_ext(ext).ok_or_else(|| {
        tide::Error::from_str(StatusCode::BadRequest, "unsupported output image extension")
    })?;
    let mut out = Vec::<u8>::new();
    {
        let mut cursor = Cursor::new(&mut out);
        image
            .write_to(&mut cursor, format)
            .map_err(|e| tide::Error::from_str(StatusCode::BadRequest, e.to_string()))?;
    }
    Ok(out)
}

fn resize_long_side(image: &DynamicImage, max_side: u32) -> DynamicImage {
    image.resize(max_side, max_side, FilterType::Lanczos3)
}

struct GalleryImageDerivatives {
    original_file: String,
    thumbnail_file: String,
    resized_file: String,
    extension: String,
    size: usize,
    sum_identifier: u64,
}

fn save_gallery_derivatives(user: &str, incoming_filename: &str, data: &[u8]) -> tide::Result<GalleryImageDerivatives> {
    let ext = Path::new(incoming_filename)
        .extension()
        .and_then(|s| s.to_str())
        .and_then(normalize_supported_ext)
        .ok_or_else(|| tide::Error::from_str(StatusCode::BadRequest, "unsupported image extension"))?
        .to_string();

    let decoded = image::load_from_memory(data)
        .map_err(|e| tide::Error::from_str(StatusCode::BadRequest, format!("unable to decode image bytes: {}", e)))?;

    let base_dir = format!("/root/midscore_io/public/gallery_index/{}", user);
    fs::create_dir_all(&base_dir)
        .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;

    let original_name = format!("{}_{}_original.{}", user, Utc::now().timestamp_millis(), ext);
    let thumbnail_name = format!("thumbnail_{}", original_name);
    let resized_name = format!("resized_{}", original_name);

    let thumb = resize_long_side(&decoded, 350);
    let resized = resize_long_side(&decoded, 1920);

    let original_bytes = encode_image_to_ext(&decoded, &ext)?;
    let thumb_bytes = encode_image_to_ext(&thumb, &ext)?;
    let resized_bytes = encode_image_to_ext(&resized, &ext)?;

    let original_path = format!("{}/{}", base_dir, original_name);
    let thumb_path = format!("{}/{}", base_dir, thumbnail_name);
    let resized_path = format!("{}/{}", base_dir, resized_name);

    fs::write(&original_path, &original_bytes)
        .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;
    fs::write(&thumb_path, &thumb_bytes)
        .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;
    fs::write(&resized_path, &resized_bytes)
        .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;

    let sum_identifier: u64 = original_bytes.iter().map(|b| *b as u64).sum();

    Ok(GalleryImageDerivatives {
        original_file: original_name,
        thumbnail_file: thumbnail_name,
        resized_file: resized_name,
        extension: ext,
        size: original_bytes.len(),
        sum_identifier,
    })
}

pub fn mount_roda_compat_routes(app: &mut tide::Server<AppState>) {
    app.at("/").get(|_| async { redirect_response("/gallery") });

    app.at("/assets/*path")
        .get(|req: Request<AppState>| async move {
            let rel = req.param("path").unwrap_or("");
            static_file_response("/root/midscore_io/tiade-maeepers-saerver-all/src/assets", rel)
        });

    app.at("/public/*path")
        .get(|req: Request<AppState>| async move {
            let rel = req.param("path").unwrap_or("");
            static_file_response("/root/midscore_io/public", rel)
        });

    app.at("/card").get(|_| async {
        let path = "/root/midscore_io/public/card_banner2.jpg";
        match fs::read(path) {
            Ok(bytes) => {
                let mut resp = Response::new(StatusCode::Ok);
                resp.set_body(bytes);
                resp.insert_header("Content-Type", "image/jpeg");
                Ok(resp)
            }
            Err(_) => json_response(
                StatusCode::NotFound,
                json!({"error": "card banner image missing", "path": path}),
            ),
        }
    });

    app.at("/moon").get(|_| async { text_response(moon_text()) });
    app.at("/sun").get(|_| async { text_response(sun_text()) });

    app.at("/admin/login").get(|req: Request<AppState>| async move {
        let password = req
            .url()
            .query_pairs()
            .find_map(|(k, v)| if k == "password" { Some(v.to_string()) } else { None })
            .unwrap_or_default();

        if password == ADMIN_PASSWORD {
            let mut resp = Response::new(StatusCode::Found);
            set_cookie(&mut resp, "admin", "true");
            resp.insert_header("Location", "/admin");
            Ok(resp)
        } else {
            html_response(page_shell(
                "Admin Login",
                "<section class=\"card\"><h2>Admin Login</h2><p>Use /admin/login?password=...</p></section>",
                "",
                "",
            ))
        }
    });

    app.at("/admin").get(|req: Request<AppState>| async move {
        let admin_enabled = get_cookie(&req, "admin").as_deref() == Some("true");
        if !admin_enabled {
            return redirect_response("/blog/login");
        }
        let linedb_guard = linedb().lock().map_err(|_| {
            tide::Error::from_str(StatusCode::InternalServerError, "linedb lock failed")
        })?;
        let databases = linedb_guard.list_databases();
        let active_database = linedb_guard.active_database_name().unwrap_or("none");
        html_response(admin_page_html(&databases, active_database, admin_enabled))
    });

    app.at("/admin/add").post(|mut req: Request<AppState>| async move {
        let body = parse_form(req.body_form::<HashMap<String, String>>().await);
        let db_name = body.get("db_name").cloned().unwrap_or_default();
        if db_name.is_empty() {
            return json_response(StatusCode::BadRequest, json!({"error": "db_name required"}));
        }

        let mut linedb_guard = linedb().lock().map_err(|_| {
            tide::Error::from_str(StatusCode::InternalServerError, "linedb lock failed")
        })?;
        let added = linedb_guard
            .add_db(&db_name)
            .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;

        if !added {
            return json_response(
                StatusCode::Conflict,
                json!({"error": "database already exists or invalid name", "db_name": db_name}),
            );
        }

        redirect_response("/admin")
    });

    app.at("/admin/remove/:db_name")
        .get(|req: Request<AppState>| async move {
            let db_name = req.param("db_name").unwrap_or("unknown");
            let mut linedb_guard = linedb().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "linedb lock failed")
            })?;
            let removed = linedb_guard
                .remove_db(db_name)
                .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;

            let _ = removed;
            redirect_response("/admin")
        });

    app.at("/admin/delete").post(|mut req: Request<AppState>| async move {
        let body = parse_form(req.body_form::<HashMap<String, String>>().await);
        let db_name = body.get("db_name").cloned().unwrap_or_default();
        if db_name.is_empty() {
            return json_response(StatusCode::BadRequest, json!({"error": "db_name required"}));
        }

        let mut linedb_guard = linedb().lock().map_err(|_| {
            tide::Error::from_str(StatusCode::InternalServerError, "linedb lock failed")
        })?;
        let deleted = linedb_guard
            .delete_db(&db_name)
            .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;

        let _ = deleted;
        redirect_response("/admin")
    });

    app.at("/admin/reload").get(|_| async {
        let mut linedb_guard = linedb().lock().map_err(|_| {
            tide::Error::from_str(StatusCode::InternalServerError, "linedb lock failed")
        })?;
        linedb_guard
            .reload()
            .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;
        redirect_response("/admin")
    });

    app.at("/admin/list").get(|_| async {
        let linedb_guard = linedb().lock().map_err(|_| {
            tide::Error::from_str(StatusCode::InternalServerError, "linedb lock failed")
        })?;
        json_response(
            StatusCode::Ok,
            json!({
                "databases": linedb_guard.list_databases(),
                "active_database": linedb_guard.active_database_name()
            }),
        )
    });

    app.at("/admin/rehash/:db_name")
        .post(|req: Request<AppState>| async move {
            let db_name = req.param("db_name").unwrap_or("").trim().to_string();
            if db_name.is_empty() {
                return json_response(StatusCode::BadRequest, json!({"error": "db_name required"}));
            }

            let mut linedb_guard = linedb().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "linedb lock failed")
            })?;

            match linedb_guard
                .rehash_database(&db_name)
                .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?
            {
                Some(compacted_slots) => json_response(
                    StatusCode::Ok,
                    json!({
                        "message": "database rehashed",
                        "db_name": db_name,
                        "compacted_slots": compacted_slots
                    }),
                ),
                None => json_response(
                    StatusCode::NotFound,
                    json!({"error": "database not found", "db_name": db_name}),
                ),
            }
        });

    app.at("/img/resize")
        .post(|mut req: Request<AppState>| async move {
            let query: HashMap<String, String> = req.query().unwrap_or_default();
            let filename = query
                .get("filename")
                .map(|v| sanitize_filename(v))
                .unwrap_or_default();
            if filename.is_empty() {
                return json_response(StatusCode::BadRequest, json!({"error": "Missing filename query parameter"}));
            }

            let ext = Path::new(&filename)
                .extension()
                .and_then(|s| s.to_str())
                .and_then(normalize_supported_ext)
                .ok_or_else(|| tide::Error::from_str(StatusCode::BadRequest, "Unsupported or missing file extension"))?;

            let width: u32 = query
                .get("width")
                .and_then(|s| s.parse().ok())
                .unwrap_or(800)
                .max(1);
            let height: u32 = query
                .get("height")
                .and_then(|s| s.parse().ok())
                .unwrap_or(600)
                .max(1);

            let data = req.body_bytes().await?;
            if data.is_empty() {
                return json_response(StatusCode::BadRequest, json!({"error": "empty request body"}));
            }

            let image = image::load_from_memory(&data)
                .map_err(|e| tide::Error::from_str(StatusCode::BadRequest, format!("unable to decode image bytes: {}", e)))?;
            let resized = image.resize(width, height, FilterType::Lanczos3);
            let out = encode_image_to_ext(&resized, ext)?;

            let output_dir = "/root/midscore_io/public/img_resized";
            fs::create_dir_all(output_dir)
                .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;
            let output_path = format!("{}/{}", output_dir, filename);
            fs::write(&output_path, &out)
                .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;

            let mut resp = Response::new(StatusCode::Ok);
            resp.set_body(out);
            resp.insert_header("Content-Type", mime_for_ext(ext));
            resp.insert_header("X-Resized-File", output_path);
            Ok(resp)
        });

    app.at("/blog").get(|_| async { redirect_response("/blog/login") });

    app.at("/blog/login")
        .get(|_| async { html_response(blog_login_page_html()) });

    app.at("/blog/login").post(|mut req: Request<AppState>| async move {
        let body = parse_form(req.body_form::<HashMap<String, String>>().await);
        let user = body
            .get("blog_user_name")
            .cloned()
            .unwrap_or_default()
            .to_lowercase();
        let password = body.get("blog_password_name").cloned().unwrap_or_default();
        let super_password = body.get("super_password").cloned().unwrap_or_default();

        if user.is_empty() || password.is_empty() || super_password.is_empty() {
            return json_response(
                StatusCode::BadRequest,
                json!({"error": "blog_user_name, blog_password_name, and super_password are required"}),
            );
        }

        let mut store = match blog_store().lock() {
            Ok(v) => v,
            Err(_) => {
                return json_response(
                    StatusCode::InternalServerError,
                    json!({"error": "blog store lock failed"}),
                )
            }
        };

        let valid = store
            .users
            .get(&user)
            .map(|u| u.password == password)
            .unwrap_or(false);
        if !valid || super_password != BLOG_SUPER_PASSWORD {
            return json_response(
                StatusCode::Unauthorized,
                json!({"error": "incorrect information"}),
            );
        }

        let token = Uuid::new_v4().to_string();
        store.sessions.insert(token.clone(), user.clone());
        save_json(BLOG_STORE_PATH, &*store);

        let mut resp = Response::new(StatusCode::Found);
        set_cookie(&mut resp, "blog_session", &token);
        resp.insert_header("Location", format!("/blog/{}/view", user));
        Ok(resp)
    });

    app.at("/blog/logout").get(|req: Request<AppState>| async move {
        if let Some(token) = get_session_token(&req) {
            if let Ok(mut store) = blog_store().lock() {
                store.sessions.remove(&token);
                save_json(BLOG_STORE_PATH, &*store);
            }
        }
        let mut resp = Response::new(StatusCode::Found);
        resp.append_header("Set-Cookie", "blog_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax");
        resp.insert_header("Location", "/");
        Ok(resp)
    });

    app.at("/blog/signup")
        .get(|_| async { html_response(blog_signup_page_html()) });

    app.at("/blog/signup").post(|mut req: Request<AppState>| async move {
        let body = parse_form(req.body_form::<HashMap<String, String>>().await);
        let user = body
            .get("blog_user_name")
            .cloned()
            .unwrap_or_default()
            .to_lowercase();
        let password = body.get("blog_password_name").cloned().unwrap_or_default();
        if user.is_empty() || password.is_empty() {
            return json_response(
                StatusCode::BadRequest,
                json!({"error": "blog_user_name and blog_password_name are required"}),
            );
        }

        let mut store = match blog_store().lock() {
            Ok(v) => v,
            Err(_) => {
                return json_response(
                    StatusCode::InternalServerError,
                    json!({"error": "blog store lock failed"}),
                )
            }
        };

        if store.users.contains_key(&user) {
            return json_response(StatusCode::Conflict, json!({"error": "user already exists"}));
        }

        store.users.insert(
            user.clone(),
            BlogUser {
                password,
                private_view: false,
                page_views: 0,
                pinned: None,
                posts: Vec::new(),
            },
        );
        save_json(BLOG_STORE_PATH, &*store);

        redirect_response("/blog/login")
    });

    app.at("/blog/render").get(|req: Request<AppState>| async move {
        let params: HashMap<String, String> = req.query().unwrap_or_default();
        let user = params.get("user").cloned().unwrap_or_default().to_lowercase();
        let id = params.get("id").cloned().unwrap_or_default();

        if user.is_empty() || id.is_empty() {
            return json_response(
                StatusCode::BadRequest,
                json!({"error": "user and id query params are required"}),
            );
        }

        let store = match blog_store().lock() {
            Ok(v) => v,
            Err(_) => {
                return json_response(
                    StatusCode::InternalServerError,
                    json!({"error": "blog store lock failed"}),
                )
            }
        };

        let Some(user_store) = store.users.get(&user) else {
            return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
        };

        if id == "pin" {
            if let Some(pin) = &user_store.pinned {
                return html_response(blog_post_page_html(&user, pin));
            }
            return json_response(StatusCode::NotFound, json!({"error": "no pinned post"}));
        }

        let parsed_id = match id.parse::<usize>() {
            Ok(v) => v,
            Err(_) => {
                return json_response(StatusCode::BadRequest, json!({"error": "invalid id"}));
            }
        };

        let Some(post) = user_store.posts.iter().find(|p| p.id == parsed_id) else {
            return json_response(StatusCode::NotFound, json!({"error": "post not found"}));
        };

        html_response(blog_post_page_html(&user, post))
    });

    app.at("/blog/:user")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            redirect_response(&format!("/blog/{}/view", user))
        });

    app.at("/blog/:user/pin")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;

            let Some(user_store) = store.users.get(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };
            match &user_store.pinned {
                Some(pin) => html_response(blog_post_page_html(&user, pin)),
                None => json_response(StatusCode::NotFound, json!({"error": "no pinned post"})),
            }
        })
        .post(|mut req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let actor = current_blog_user(&req);
            if actor.as_deref() != Some(user.as_str()) {
                return json_response(
                    StatusCode::Unauthorized,
                    json!({"error": "login required for this user"}),
                );
            }

            let body = parse_form(req.body_form::<HashMap<String, String>>().await);
            let post = BlogPost {
                id: 0,
                title: body.get("blog_post_title").cloned().unwrap_or_default(),
                body: body.get("blog_post_body").cloned().unwrap_or_default(),
                body_markdown: body.get("blog_post_body_markdown").cloned(),
                tags: split_tags(body.get("blog_post_tags").map(String::as_str).unwrap_or("")),
                date: body.get("blog_post_date").cloned().unwrap_or_default(),
                author: body.get("blog_post_author").cloned().unwrap_or(user.clone()),
                comments: body.get("blog_post_comments").cloned().unwrap_or_default(),
                status: body.get("blog_post_status").cloned().unwrap_or_default(),
                locked: false,
                rendered_type: body
                    .get("rendered_type")
                    .cloned()
                    .unwrap_or_else(|| "wysiwyg".to_string()),
                timestamp: now_string(),
            };

            let mut store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;
            let Some(user_store) = store.users.get_mut(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };
            user_store.pinned = Some(post);
            save_json(BLOG_STORE_PATH, &*store);
            redirect_response(&format!("/blog/{}/pin", user))
        });

    app.at("/blog/:user/tag/:tag")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let tag = req.param("tag").unwrap_or("").to_string();

            let store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;
            let Some(user_store) = store.users.get(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };
            let filtered: Vec<BlogPost> = user_store
                .posts
                .iter()
                .filter(|p| p.tags.iter().any(|t| t == &tag))
                .cloned()
                .collect();

            html_response(blog_index_page_html(&user, &filtered, user_store.private_view))
        });

    app.at("/blog/:user/edit/:id")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);

            let store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;
            let Some(user_store) = store.users.get(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };
            let post = user_store.posts.iter().find(|p| p.id == id);
            html_response(blog_editor_page_html(&user, post))
        })
        .post(|mut req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);

            let actor = current_blog_user(&req);
            if actor.as_deref() != Some(user.as_str()) {
                return json_response(
                    StatusCode::Unauthorized,
                    json!({"error": "login required for this user"}),
                );
            }

            let body = parse_form(req.body_form::<HashMap<String, String>>().await);
            let mut store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;
            let Some(user_store) = store.users.get_mut(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };

            let Some(post) = user_store.posts.iter_mut().find(|p| p.id == id) else {
                return json_response(StatusCode::NotFound, json!({"error": "post not found"}));
            };

            if let Some(v) = body.get("blog_post_title") {
                post.title = v.clone();
            }
            if let Some(v) = body.get("blog_post_body") {
                post.body = v.clone();
            }
            if let Some(v) = body.get("blog_post_body_markdown") {
                post.body_markdown = Some(v.clone());
            }
            if let Some(v) = body.get("blog_post_tags") {
                post.tags = split_tags(v);
            }
            if let Some(v) = body.get("blog_post_date") {
                post.date = v.clone();
            }
            if let Some(v) = body.get("blog_post_author") {
                post.author = v.clone();
            }
            if let Some(v) = body.get("blog_post_comments") {
                post.comments = v.clone();
            }
            if let Some(v) = body.get("blog_post_status") {
                post.status = v.clone();
            }
            if let Some(v) = body.get("rendered_type") {
                post.rendered_type = v.clone();
            }
            post.timestamp = now_string();

            save_json(BLOG_STORE_PATH, &*store);
            redirect_response(&format!("/blog/{}/edit/{}", user, id))
        });

    app.at("/blog/:user/delete")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;
            let Some(user_store) = store.users.get(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };
            html_response(blog_index_page_html(&user, &user_store.posts, user_store.private_view))
        });

    app.at("/blog/:user/delete/:id")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);
            let actor = current_blog_user(&req);
            if actor.as_deref() != Some(user.as_str()) {
                return json_response(
                    StatusCode::Unauthorized,
                    json!({"error": "login required for this user"}),
                );
            }

            let mut store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;
            let Some(user_store) = store.users.get_mut(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };

            let Some(post) = user_store.posts.iter_mut().find(|p| p.id == id) else {
                return json_response(StatusCode::NotFound, json!({"error": "post not found"}));
            };

            post.locked = !post.locked;
            save_json(BLOG_STORE_PATH, &*store);
            redirect_response(&format!("/blog/{}/delete", user))
        });

    app.at("/blog/:user/list")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;
            let Some(user_store) = store.users.get(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };
            html_response(blog_index_page_html(&user, &user_store.posts, user_store.private_view))
        });

    app.at("/blog/:user/new")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let actor = current_blog_user(&req);
            if actor.as_deref() != Some(user.as_str()) {
                return json_response(
                    StatusCode::Unauthorized,
                    json!({"error": "login required for this user"}),
                );
            }

            html_response(blog_editor_page_html(&user, None))
        })
        .post(|mut req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let actor = current_blog_user(&req);
            if actor.as_deref() != Some(user.as_str()) {
                return json_response(
                    StatusCode::Unauthorized,
                    json!({"error": "login required for this user"}),
                );
            }

            let body = parse_form(req.body_form::<HashMap<String, String>>().await);
            let title = body.get("blog_post_title").cloned().unwrap_or_default();
            let body_html = body.get("blog_post_body").cloned().unwrap_or_default();
            let body_md = body.get("blog_post_body_markdown").cloned();
            let tags = split_tags(body.get("blog_post_tags").map(String::as_str).unwrap_or(""));
            let date = body.get("blog_post_date").cloned().unwrap_or_default();
            let author = body.get("blog_post_author").cloned().unwrap_or(user.clone());
            let comments = body.get("blog_post_comments").cloned().unwrap_or_default();
            let status = body.get("blog_post_status").cloned().unwrap_or_default();
            let rendered_type = body
                .get("rendered_type")
                .cloned()
                .unwrap_or_else(|| "wysiwyg".to_string());

            if title.is_empty() || (body_html.is_empty() && body_md.as_deref().unwrap_or("").is_empty())
            {
                return json_response(
                    StatusCode::BadRequest,
                    json!({"error": "missing required blog post fields"}),
                );
            }

            let mut store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;
            let Some(user_store) = store.users.get_mut(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };

            let next_id = user_store
                .posts
                .iter()
                .map(|p| p.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1);

            let post = BlogPost {
                id: next_id,
                title,
                body: if body_html.is_empty() {
                    body_md.clone().unwrap_or_default()
                } else {
                    body_html
                },
                body_markdown: body_md,
                tags,
                date,
                author,
                comments,
                status,
                locked: false,
                rendered_type,
                timestamp: now_string(),
            };

            user_store.posts.push(post);
            save_json(BLOG_STORE_PATH, &*store);
            redirect_response(&format!("/blog/{}/view/{}", user, next_id))
        });

    app.at("/blog/:user/private_toggle")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let actor = current_blog_user(&req);
            if actor.as_deref() != Some(user.as_str()) {
                return json_response(
                    StatusCode::Unauthorized,
                    json!({"error": "login required for this user"}),
                );
            }

            let mut store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;
            let Some(user_store) = store.users.get_mut(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };

            user_store.private_view = !user_store.private_view;
            save_json(BLOG_STORE_PATH, &*store);
            redirect_response(&format!("/blog/{}/view", user))
        });

    app.at("/blog/:user/view")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;
            let Some(user_store) = store.users.get(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };

            if user_store.private_view {
                let viewer = current_blog_user(&req);
                if viewer.as_deref() != Some(user.as_str()) {
                    return json_response(StatusCode::NotFound, json!({"error": "404"}));
                }
            }

            let mut posts = user_store.posts.clone();
            let query: HashMap<String, String> = req.query().unwrap_or_default();
            if query.get("reverse").is_none() {
                posts.reverse();
            }

            html_response(blog_index_page_html(&user, &posts, user_store.private_view))
        });

    app.at("/blog/:user/view/:id")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);

            let mut store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;
            let (post, page_views) = {
                let Some(user_store) = store.users.get_mut(&user) else {
                    return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
                };

                if user_store.private_view {
                    let viewer = current_blog_user(&req);
                    if viewer.as_deref() != Some(user.as_str()) {
                        return json_response(StatusCode::NotFound, json!({"error": "404"}));
                    }
                }

                let Some(post) = user_store.posts.iter().find(|p| p.id == id).cloned() else {
                    return json_response(StatusCode::NotFound, json!({"error": "post not found"}));
                };

                user_store.page_views = user_store.page_views.saturating_add(1);
                (post, user_store.page_views)
            };
            save_json(BLOG_STORE_PATH, &*store);

            let query: HashMap<String, String> = req.query().unwrap_or_default();
            if query.get("format").map(String::as_str) == Some("json") {
                return json_response(
                    StatusCode::Ok,
                    json!({"user": user, "post": post, "locked": post.locked, "page_views": page_views}),
                );
            }

            html_response(blog_post_page_html(&user, &post))
        });

    app.at("/blog/:user/view/:month/:day/:year/:time")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("blog").to_lowercase();
            let month = req.param("month").unwrap_or("");
            let day = req.param("day").unwrap_or("");
            let year = req.param("year").unwrap_or("");
            let time = req.param("time").unwrap_or("");
            let needle = format!("{}/{}/{} {}", month, day, year, time);

            let mut store = blog_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "blog store lock failed")
            })?;
            let Some(user_store) = store.users.get_mut(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };

            let Some(post) = user_store
                .posts
                .iter()
                .find(|p| p.date.contains(&needle) || p.timestamp.contains(&needle))
                .cloned()
            else {
                return json_response(StatusCode::NotFound, json!({"error": "post not found by date/time"}));
            };

            user_store.page_views = user_store.page_views.saturating_add(1);
            save_json(BLOG_STORE_PATH, &*store);
            html_response(blog_post_page_html(&user, &post))
        });

    app.at("/gallery")
        .get(|_| async {
            let store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let mut users: Vec<String> = store.users.keys().cloned().collect();
            users.sort();
            html_response(gallery_home_page_html(&users))
        });

    app.at("/gallery/upload/url")
        .get(|_| async { html_response(gallery_upload_url_page_html()) });

    app.at("/gallery/upload/url")
        .post(|mut req: Request<AppState>| async move {
            let body = parse_form(req.body_form::<HashMap<String, String>>().await);
            let user = body
                .get("user")
                .cloned()
                .unwrap_or_else(|| "gallery".to_string())
                .to_lowercase();
            let url = body.get("url").cloned().unwrap_or_default();
            if url.is_empty() {
                return json_response(StatusCode::BadRequest, json!({"error": "url required"}));
            }

            let title = body
                .get("title")
                .cloned()
                .unwrap_or_else(|| "URL Upload".to_string());
            let description = body
                .get("description")
                .cloned()
                .unwrap_or_else(|| "no description".to_string());
            let tags = split_tags(body.get("tags").map(String::as_str).unwrap_or(""));

            let mut fetch = surf::get(&url)
                .await
                .map_err(|e| tide::Error::from_str(StatusCode::BadGateway, e.to_string()))?;
            if !fetch.status().is_success() {
                return json_response(
                    StatusCode::BadGateway,
                    json!({"error": "failed to download image url", "status": fetch.status().to_string()}),
                );
            }

            let content_type = fetch
                .header("content-type")
                .and_then(|vals| vals.get(0))
                .map(|v| v.as_str().to_string())
                .unwrap_or_default();

            let data = fetch
                .body_bytes()
                .await
                .map_err(|e| tide::Error::from_str(StatusCode::BadGateway, e.to_string()))?;
            if data.is_empty() {
                return json_response(StatusCode::BadRequest, json!({"error": "downloaded image body was empty"}));
            }

            let parsed_url = url::Url::parse(&url).ok();
            let name_from_url = parsed_url
                .as_ref()
                .and_then(|u| u.path_segments().and_then(|mut segs| segs.next_back()))
                .unwrap_or("remote_upload");
            let sanitized = sanitize_filename(name_from_url);

            let ext = Path::new(&sanitized)
                .extension()
                .and_then(|s| s.to_str())
                .and_then(normalize_supported_ext)
                .or_else(|| ext_from_content_type(&content_type));
            let Some(ext) = ext else {
                return json_response(
                    StatusCode::BadRequest,
                    json!({"error": "unsupported image type", "content_type": content_type}),
                );
            };

            let stem = Path::new(&sanitized)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("remote_upload");
            let incoming_filename = format!("{}.{}", stem, ext);
            let derivatives = save_gallery_derivatives(&user, &incoming_filename, &data)?;

            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let posts = store.users.entry(user.clone()).or_default();
            let next_id = posts.iter().map(|p| p.id).max().unwrap_or(0).saturating_add(1);
            posts.push(GalleryPost {
                id: next_id,
                title,
                body: url.clone(),
                tags,
                attachments: vec![],
                created_at: now_string(),
                views: Some(0),
                file: Some(derivatives.original_file.clone()),
                extension: Some(derivatives.extension.clone()),
                size: Some(derivatives.size),
                sum_identifier: Some(derivatives.sum_identifier),
                thumbnail_file: Some(derivatives.thumbnail_file.clone()),
                resized_file: Some(derivatives.resized_file.clone()),
                description: Some(description),
            });
            save_json(GALLERY_STORE_PATH, &*store);

            redirect_response(&format!("/gallery/view/{}/id/{}", user, next_id))
        });

    app.at("/gallery/upload")
        .get(|_| async { html_response(gallery_upload_page_html()) });

    app.at("/gallery/upload")
        .post(|mut req: Request<AppState>| async move {
            let query: HashMap<String, String> = req.query().unwrap_or_default();
            let content_type = req
                .header("content-type")
                .and_then(|vals| vals.get(0))
                .map(|v| v.as_str().to_string())
                .unwrap_or_default();

            let mut user = query
                .get("user")
                .cloned()
                .unwrap_or_else(|| "gallery".to_string())
                .to_lowercase();
            let mut filename = query
                .get("filename")
                .map(|v| sanitize_filename(v))
                .unwrap_or_else(|| "upload.jpg".to_string());
            let mut title = query
                .get("title")
                .cloned()
                .unwrap_or_else(|| "upload".to_string());
            let mut description = query
                .get("description")
                .cloned()
                .unwrap_or_else(|| "no description".to_string());
            let mut tags = split_tags(query.get("tags").map(String::as_str).unwrap_or("none"));
            let data: Vec<u8>;

            if content_type.contains("multipart/form-data") {
                let raw = req.body_bytes().await?;
                let (fields, files) = parse_multipart_form_data(&content_type, &raw);

                if let Some(v) = fields.get("user") {
                    user = v.to_lowercase();
                }
                if let Some(v) = fields.get("title") {
                    title = if v.trim().is_empty() { "untitled".to_string() } else { v.clone() };
                }
                if let Some(v) = fields.get("description") {
                    description = if v.trim().is_empty() { "no description".to_string() } else { v.clone() };
                }
                if let Some(v) = fields.get("tags") {
                    tags = split_tags(v);
                }

                if let Some(url_value) = fields.get("url").filter(|v| !v.trim().is_empty()) {
                    let mut fetch = surf::get(url_value)
                        .await
                        .map_err(|e| tide::Error::from_str(StatusCode::BadGateway, e.to_string()))?;
                    if !fetch.status().is_success() {
                        return json_response(
                            StatusCode::BadGateway,
                            json!({"error": "failed to download image url", "status": fetch.status().to_string()}),
                        );
                    }
                    let remote_content_type = fetch
                        .header("content-type")
                        .and_then(|vals| vals.get(0))
                        .map(|v| v.as_str().to_string())
                        .unwrap_or_default();
                    let remote_data = fetch
                        .body_bytes()
                        .await
                        .map_err(|e| tide::Error::from_str(StatusCode::BadGateway, e.to_string()))?;
                    if remote_data.is_empty() {
                        return json_response(StatusCode::BadRequest, json!({"error": "downloaded image body was empty"}));
                    }
                    let parsed_url = url::Url::parse(url_value).ok();
                    let name_from_url = parsed_url
                        .as_ref()
                        .and_then(|u| u.path_segments().and_then(|mut segs| segs.next_back()))
                        .unwrap_or("remote_upload");
                    let sanitized = sanitize_filename(name_from_url);
                    let ext = Path::new(&sanitized)
                        .extension()
                        .and_then(|s| s.to_str())
                        .and_then(normalize_supported_ext)
                        .or_else(|| ext_from_content_type(&remote_content_type));
                    let Some(ext) = ext else {
                        return json_response(StatusCode::BadRequest, json!({"error": "unsupported image type for uploaded url"}));
                    };
                    let stem = Path::new(&sanitized)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("remote_upload");
                    filename = format!("{}.{}", stem, ext);
                    data = remote_data;
                } else if let Some(file) = files.get("file") {
                    data = file.data.clone();
                    filename = if file.filename.is_empty() {
                        "upload.jpg".to_string()
                    } else {
                        sanitize_filename(&file.filename)
                    };
                    if Path::new(&filename).extension().is_none() {
                        if let Some(ext) = file
                            .content_type
                            .as_deref()
                            .and_then(ext_from_content_type)
                        {
                            filename = format!("{}.{}", filename, ext);
                        }
                    }
                } else {
                    return json_response(
                        StatusCode::BadRequest,
                        json!({"error": "multipart upload requires file field or url field"}),
                    );
                }
            } else {
                data = req.body_bytes().await?;
                if data.is_empty() {
                    return json_response(StatusCode::BadRequest, json!({"error": "empty upload body"}));
                }
            }

            let derivatives = save_gallery_derivatives(&user, &filename, &data)?;

            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let posts = store.users.entry(user.clone()).or_default();
            let next_id = posts.iter().map(|p| p.id).max().unwrap_or(0).saturating_add(1);
            posts.push(GalleryPost {
                id: next_id,
                title,
                body: derivatives.original_file.clone(),
                tags,
                attachments: vec![],
                created_at: now_string(),
                views: Some(0),
                file: Some(derivatives.original_file.clone()),
                extension: Some(derivatives.extension.clone()),
                size: Some(derivatives.size),
                sum_identifier: Some(derivatives.sum_identifier),
                thumbnail_file: Some(derivatives.thumbnail_file.clone()),
                resized_file: Some(derivatives.resized_file.clone()),
                description: Some(description),
            });
            save_json(GALLERY_STORE_PATH, &*store);

            redirect_response(&format!("/gallery/view/{}/id/{}", user, next_id))
        });

    app.at("/gallery/view/:user/latest")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let query: HashMap<String, String> = req.query().unwrap_or_default();
            let quantity_displayed = gallery_pref_usize(
                &req,
                &query,
                "quantity_displayed",
                "gallery_quantity_displayed",
                175,
                1,
            );
            let store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let total = store
                .users
                .get(&user)
                .map(|posts| posts.len())
                .unwrap_or(0);
            let pages = if total <= quantity_displayed {
                0
            } else {
                total.div_ceil(quantity_displayed)
            };
            let skip_by = pages.saturating_sub(1);
            redirect_response(&format!("/gallery/view/{}?skip_by={}", user, skip_by))
        });

    app.at("/gallery/reset_session/:user")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let mut resp = Response::new(StatusCode::Found);
            resp.append_header("Set-Cookie", "gallery_quantity_displayed=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax");
            resp.append_header("Set-Cookie", "gallery_modulo_display=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax");
            resp.append_header("Set-Cookie", "gallery_owo_count_rate=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax");
            resp.insert_header("Location", format!("/gallery/view/{}", user));
            Ok(resp)
        });

    app.at("/gallery/view/:user")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let query: HashMap<String, String> = req.query().unwrap_or_default();
            let quantity_displayed = gallery_pref_usize(
                &req,
                &query,
                "quantity_displayed",
                "gallery_quantity_displayed",
                175,
                1,
            );
            let modulo_display = gallery_pref_usize(
                &req,
                &query,
                "modulo_display",
                "gallery_modulo_display",
                4,
                1,
            );
            let owo_count_rate = gallery_pref_usize(
                &req,
                &query,
                "owo_count_rate",
                "gallery_owo_count_rate",
                3,
                1,
            );
            let skip_by: usize = query
                .get("skip_by")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);

            let store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let mut posts = store.users.get(&user).cloned().unwrap_or_default();
            posts.sort_by_key(|p| p.id);
            posts.reverse();

            let start = quantity_displayed.saturating_mul(skip_by);
            let end = start.saturating_add(quantity_displayed).min(posts.len());
            let paged_posts = if start < posts.len() {
                posts[start..end].to_vec()
            } else {
                Vec::new()
            };

            let total_posts = posts.len();
            let total_pages = total_posts.div_ceil(quantity_displayed);

            let _ = (total_posts, quantity_displayed, modulo_display, skip_by, total_pages, owo_count_rate);
            let body = gallery_index_page_html(&user, &paged_posts);
            let mut resp = Response::new(StatusCode::Ok);
            resp.set_body(body);
            resp.insert_header("Content-Type", "text/html; charset=utf-8");
            set_cookie(&mut resp, "gallery_quantity_displayed", &quantity_displayed.to_string());
            set_cookie(&mut resp, "gallery_modulo_display", &modulo_display.to_string());
            set_cookie(&mut resp, "gallery_owo_count_rate", &owo_count_rate.to_string());
            Ok(resp)
        });

    app.at("/gallery/view/:user/id/:id")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);
            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let post = store.users.get_mut(&user).and_then(|posts| {
                let post = posts.iter_mut().find(|p| p.id == id)?;
                let view_counter = post.views.get_or_insert(0);
                *view_counter = view_counter.saturating_add(1);
                Some(post.clone())
            });
            if post.is_some() {
                save_json(GALLERY_STORE_PATH, &*store);
            }
            match post {
                Some(post) => {
                    let file = post
                        .resized_file
                        .clone()
                        .or(post.file.clone())
                        .map(|f| format!("/public/gallery_index/{}/{}", user, f))
                        .unwrap_or_default();
                    let body = format!(
                        "<article class=\"card\"><h2>{}</h2><p class=\"muted\">views: {}</p><img src=\"{}\" style=\"max-width:100%;border-radius:12px\"><p>{}</p><p class=\"muted\">tags: {}</p><div class=\"bar\"><a href=\"/gallery/edit/{}/id/{}\">edit</a><a href=\"/gallery/delete/{}/id/{}\">delete</a><a href=\"/gallery/view/{}/id/{}/attachments\">attachments</a></div></article>",
                        html_escape(&post.title),
                        post.views.unwrap_or(0),
                        html_escape(&file),
                        html_escape(&post.body),
                        html_escape(&post.tags.join(", ")),
                        html_escape(&user),
                        post.id,
                        html_escape(&user),
                        post.id,
                        html_escape(&user),
                        post.id
                    );
                    html_response(page_shell("Gallery Item", &body, "", ""))
                }
                None => json_response(StatusCode::NotFound, json!({"error": "post not found"})),
            }
        });

    app.at("/gallery/view/:user/id/:id/attachments")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);
            let store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let attachments = store
                .users
                .get(&user)
                .and_then(|posts| posts.iter().find(|p| p.id == id))
                .map(|post| post.attachments.clone())
                .unwrap_or_default();
            let mut rows = String::new();
            for a in attachments {
                let _ = write!(
                    &mut rows,
                    "<li>{} <a href=\"/gallery/view/{}/id/{}/attachments/delete/{}\">delete</a></li>",
                    html_escape(&a.value),
                    html_escape(&user),
                    id,
                    a.id
                );
            }
            let body = format!(
                "<section class=\"card\"><h2>Attachments</h2><ul>{}</ul><form method=\"post\" action=\"/gallery/view/{}/id/{}/attachments/upload\"><input type=\"text\" name=\"value\" placeholder=\"attachment value\"><button type=\"submit\">add attachment</button></form></section>",
                rows,
                html_escape(&user),
                id
            );
            html_response(page_shell("Gallery Attachments", &body, "", ""))
        });

    app.at("/gallery/view/:user/id/:id/attachments/delete/:attachment_id")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);
            let attachment_id: usize = req
                .param("attachment_id")
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);

            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let Some(posts) = store.users.get_mut(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };
            let Some(post) = posts.iter_mut().find(|p| p.id == id) else {
                return json_response(StatusCode::NotFound, json!({"error": "post not found"}));
            };

            post.attachments.retain(|a| a.id != attachment_id);
            save_json(GALLERY_STORE_PATH, &*store);
            redirect_response(&format!("/gallery/view/{}/id/{}/attachments", user, id))
        });

    app.at("/gallery/view/:user/id/:id/attachments/upload")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);
            let body = format!(
                "<section class=\"card\"><h2>Upload Attachment</h2><form method=\"post\" action=\"/gallery/view/{}/id/{}/attachments/upload\"><label>Attachment Value</label><input type=\"text\" name=\"value\" required><button type=\"submit\">attach</button></form></section>",
                html_escape(&user),
                id
            );
            html_response(page_shell("Upload Attachment", &body, "", ""))
        })
        .post(|mut req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);
            let content_type = req
                .header("content-type")
                .and_then(|vals| vals.get(0))
                .map(|v| v.as_str().to_string())
                .unwrap_or_default();
            let mut value = "attachment".to_string();

            if content_type.contains("multipart/form-data") {
                let raw = req.body_bytes().await?;
                let (fields, files) = parse_multipart_form_data(&content_type, &raw);
                if let Some(url_value) = fields.get("url").filter(|v| !v.trim().is_empty()) {
                    let mut fetch = surf::get(url_value)
                        .await
                        .map_err(|e| tide::Error::from_str(StatusCode::BadGateway, e.to_string()))?;
                    if !fetch.status().is_success() {
                        return json_response(
                            StatusCode::BadGateway,
                            json!({"error": "failed to download attachment url", "status": fetch.status().to_string()}),
                        );
                    }
                    let remote_content_type = fetch
                        .header("content-type")
                        .and_then(|vals| vals.get(0))
                        .map(|v| v.as_str().to_string())
                        .unwrap_or_default();
                    let remote_data = fetch
                        .body_bytes()
                        .await
                        .map_err(|e| tide::Error::from_str(StatusCode::BadGateway, e.to_string()))?;
                    let ext = ext_from_content_type(&remote_content_type).unwrap_or("bin");
                    let filename = format!("{}_attachment_{}.{}", user, Utc::now().timestamp_millis(), ext);
                    let dir = format!("/root/midscore_io/public/gallery_index/{}/attachments", user);
                    fs::create_dir_all(&dir)
                        .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;
                    let path = format!("{}/{}", dir, filename);
                    fs::write(&path, remote_data)
                        .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;
                    value = format!("/public/gallery_index/{}/attachments/{}", user, filename);
                } else if let Some(file) = files.get("file") {
                    let filename = format!(
                        "{}_attachment_{}_{}",
                        user,
                        Utc::now().timestamp_millis(),
                        sanitize_filename(&file.filename)
                    );
                    let dir = format!("/root/midscore_io/public/gallery_index/{}/attachments", user);
                    fs::create_dir_all(&dir)
                        .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;
                    let path = format!("{}/{}", dir, filename);
                    fs::write(&path, &file.data)
                        .map_err(|e| tide::Error::from_str(StatusCode::InternalServerError, e.to_string()))?;
                    value = format!("/public/gallery_index/{}/attachments/{}", user, filename);
                } else if let Some(v) = fields.get("value").filter(|v| !v.trim().is_empty()) {
                    value = v.clone();
                }
            } else {
                let body = parse_form(req.body_form::<HashMap<String, String>>().await);
                value = body
                    .get("value")
                    .cloned()
                    .unwrap_or_else(|| "attachment".to_string());
            }

            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let Some(posts) = store.users.get_mut(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };
            let Some(post) = posts.iter_mut().find(|p| p.id == id) else {
                return json_response(StatusCode::NotFound, json!({"error": "post not found"}));
            };

            let next_attachment_id = post
                .attachments
                .iter()
                .map(|a| a.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            post.attachments.push(GalleryAttachment {
                id: next_attachment_id,
                value,
            });
            save_json(GALLERY_STORE_PATH, &*store);

            let _ = next_attachment_id;
            redirect_response(&format!("/gallery/view/{}/id/{}/attachments", user, id))
        });

    app.at("/gallery/delete/:user/id/:id")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);

            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let removed_post = {
                let Some(posts) = store.users.get_mut(&user) else {
                    return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
                };
                let idx = posts.iter().position(|p| p.id == id);
                idx.map(|i| posts.remove(i))
            };
            let deleted = removed_post.is_some();
            save_json(GALLERY_STORE_PATH, &*store);

            if let Some(post) = removed_post {
                let base = format!("/root/midscore_io/public/gallery_index/{}", user);
                for maybe_name in [post.file, post.thumbnail_file, post.resized_file] {
                    if let Some(name) = maybe_name {
                        let path = format!("{}/{}", base, name);
                        let _ = fs::remove_file(path);
                    }
                }
            }

            let _ = deleted;
            redirect_response(&format!("/gallery/view/{}", user))
        });

    app.at("/gallery/view/:user/tags/search")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let query: HashMap<String, String> = req.query().unwrap_or_default();
            let search_raw = query
                .get("search_tags")
                .cloned()
                .or_else(|| query.get("tag").cloned())
                .unwrap_or_default();
            let search_tokens: Vec<String> = search_raw
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            let include_tags: Vec<String> = search_tokens
                .iter()
                .filter(|t| !t.starts_with("--"))
                .cloned()
                .collect();
            let reject_tags: Vec<String> = search_tokens
                .iter()
                .filter_map(|t| t.strip_prefix("--").map(|v| v.to_string()))
                .collect();

            let store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let posts = store.users.get(&user).cloned().unwrap_or_default();
            let matched: Vec<GalleryPost> = posts
                .into_iter()
                .filter(|p| {
                    let post_tags: Vec<String> = p.tags.iter().map(|t| t.to_lowercase()).collect();
                    let includes_ok = if include_tags.is_empty() {
                        true
                    } else {
                        include_tags.iter().all(|needle| post_tags.iter().any(|t| t == needle))
                    };
                    let rejects_ok = reject_tags
                        .iter()
                        .all(|needle| !post_tags.iter().any(|t| t == needle));
                    includes_ok && rejects_ok
                })
                .collect();
            html_response(gallery_index_page_html(&user, &matched))
        });

    app.at("/gallery/view/:user/tags")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let posts = store.users.get(&user).cloned().unwrap_or_default();
            let mut tags = Vec::<String>::new();
            for post in posts {
                for tag in post.tags {
                    if !tags.iter().any(|t| t == &tag) {
                        tags.push(tag);
                    }
                }
            }
            let mut links = String::new();
            for t in tags {
                let _ = write!(
                    &mut links,
                    "<li><a href=\"/gallery/view/{}/tags/search?tag={}\">{}</a></li>",
                    html_escape(&user),
                    html_escape(&t),
                    html_escape(&t)
                );
            }
            let body = format!("<section class=\"card\"><h2>Tags for {}</h2><ul>{}</ul></section>", html_escape(&user), links);
            html_response(page_shell("Gallery Tags", &body, "", ""))
        });

    app.at("/gallery/edit/:user/id/:id")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);
            let store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let post = store
                .users
                .get(&user)
                .and_then(|posts| posts.iter().find(|p| p.id == id))
                .cloned();
            let form = if let Some(post) = post {
                format!("<section class=\"card\"><h2>Edit Gallery Post</h2><form method=\"post\" action=\"/gallery/edit/{}/id/{}\"><label>Title</label><input type=\"text\" name=\"title\" value=\"{}\"><label>Body</label><textarea name=\"body\">{}</textarea><label>Tags</label><input type=\"text\" name=\"tags\" value=\"{}\"><button type=\"submit\">save</button></form></section>", html_escape(&user), id, html_escape(&post.title), html_escape(&post.body), html_escape(&post.tags.join(", ")))
            } else {
                "<section class=\"card\"><h2>Post not found</h2></section>".to_string()
            };
            html_response(page_shell("Edit Gallery", &form, "", ""))
        })
        .post(|mut req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);
            let body = parse_form(req.body_form::<HashMap<String, String>>().await);

            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let Some(posts) = store.users.get_mut(&user) else {
                return json_response(StatusCode::NotFound, json!({"error": "user not found"}));
            };
            let Some(post) = posts.iter_mut().find(|p| p.id == id) else {
                return json_response(StatusCode::NotFound, json!({"error": "post not found"}));
            };

            if let Some(v) = body.get("title") {
                post.title = v.clone();
            }
            if let Some(v) = body.get("body") {
                post.body = v.clone();
            }
            if let Some(v) = body.get("tags") {
                post.tags = split_tags(v);
            }
            save_json(GALLERY_STORE_PATH, &*store);
            redirect_response(&format!("/gallery/view/{}/id/{}", user, id))
        });

    app.at("/gallery/uwu/view/:user")
        .get(|req: Request<AppState>| async move {
            let user = req.param("user").unwrap_or("gallery").to_lowercase();
            let store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            json_response(
                StatusCode::Ok,
                json!({"user": user, "collections": store.uwu_collections}),
            )
        });

    app.at("/gallery/uwu/view/:user/id/:id")
        .get(|req: Request<AppState>| async move {
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);
            let store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            let collection = store.uwu_collections.get(&id).cloned().unwrap_or_default();
            json_response(StatusCode::Ok, json!({"id": id, "images": collection}))
        });

    app.at("/gallery/uwu/delete/id/:id")
        .get(|req: Request<AppState>| async move {
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);
            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            store.uwu_collections.remove(&id);
            save_json(GALLERY_STORE_PATH, &*store);
            json_response(StatusCode::Ok, json!({"message": "collection deleted", "id": id}))
        });

    app.at("/gallery/uwu/new")
        .get(|_| async {
            json_response(
                StatusCode::Ok,
                json!({"message": "POST id field to /gallery/uwu/new to create collection"}),
            )
        })
        .post(|mut req: Request<AppState>| async move {
            let body = parse_form(req.body_form::<HashMap<String, String>>().await);
            let id: usize = body.get("id").and_then(|v| v.parse().ok()).unwrap_or(0);
            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            store.uwu_collections.entry(id).or_default();
            save_json(GALLERY_STORE_PATH, &*store);
            json_response(StatusCode::Created, json!({"message": "collection created", "id": id}))
        });

    app.at("/gallery/uwu/edit/id/:id")
        .post(|mut req: Request<AppState>| async move {
            let id: usize = req.param("id").unwrap_or("0").parse().unwrap_or(0);
            let body = parse_form(req.body_form::<HashMap<String, String>>().await);
            let replacement = body.get("items").cloned().unwrap_or_default();
            let pairs: Vec<(String, usize)> = replacement
                .split(',')
                .filter_map(|entry| {
                    let mut parts = entry.split(':');
                    let u = parts.next()?.trim().to_string();
                    let g = parts.next()?.trim().parse::<usize>().ok()?;
                    Some((u, g))
                })
                .collect();

            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            store.uwu_collections.insert(id, pairs);
            save_json(GALLERY_STORE_PATH, &*store);
            json_response(StatusCode::Ok, json!({"message": "collection updated", "id": id}))
        });

    app.at("/gallery/uwu/delete_image/uwu_id/:uwu_id/gallery_id/:gallery_id")
        .get(|req: Request<AppState>| async move {
            let uwu_id: usize = req.param("uwu_id").unwrap_or("0").parse().unwrap_or(0);
            let gallery_id: usize = req.param("gallery_id").unwrap_or("0").parse().unwrap_or(0);

            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            if let Some(collection) = store.uwu_collections.get_mut(&uwu_id) {
                collection.retain(|(_, id)| *id != gallery_id);
                save_json(GALLERY_STORE_PATH, &*store);
            }
            json_response(
                StatusCode::Ok,
                json!({"message": "image removed from collection", "uwu_id": uwu_id, "gallery_id": gallery_id}),
            )
        });

    app.at("/gallery/uwu/add_image/uwu_id/:uwu_id")
        .post(|mut req: Request<AppState>| async move {
            let uwu_id: usize = req.param("uwu_id").unwrap_or("0").parse().unwrap_or(0);
            let body = parse_form(req.body_form::<HashMap<String, String>>().await);
            let user = body
                .get("user")
                .cloned()
                .unwrap_or_else(|| "gallery".to_string())
                .to_lowercase();
            let gallery_id: usize = body
                .get("gallery_id")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);

            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            store
                .uwu_collections
                .entry(uwu_id)
                .or_default()
                .push((user, gallery_id));
            save_json(GALLERY_STORE_PATH, &*store);

            json_response(
                StatusCode::Ok,
                json!({"message": "image added to collection", "uwu_id": uwu_id, "gallery_id": gallery_id}),
            )
        });

    app.at("/gallery/owo/add")
        .get(|_| async {
            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            store.owo_value += 1;
            let value = store.owo_value;
            save_json(GALLERY_STORE_PATH, &*store);
            json_response(StatusCode::Ok, json!({"owo": value}))
        });

    app.at("/gallery/owo/rem")
        .get(|_| async {
            let mut store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            store.owo_value -= 1;
            let value = store.owo_value;
            save_json(GALLERY_STORE_PATH, &*store);
            json_response(StatusCode::Ok, json!({"owo": value}))
        });

    app.at("/gallery/owo/sub")
        .get(|_| async {
            let store = gallery_store().lock().map_err(|_| {
                tide::Error::from_str(StatusCode::InternalServerError, "gallery store lock failed")
            })?;
            json_response(StatusCode::Ok, json!({"owo": store.owo_value}))
        });

}
