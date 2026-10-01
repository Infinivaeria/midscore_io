
#![recursion_limit = "256"]

mod ruby_vm;

use std::io::{self, BufRead};
use tide::utils::After;
use tide_rustls::TlsListener;



use chrono::Utc;
use tiade_ollama_relay::{RelayConfig as OllamaRelayConfig, mount_routes as mount_ollama_routes};
use serde::Deserialize;


use std::fs;
use std::collections::{HashSet, BTreeMap};

use regex::Regex;
use chrono::{Timelike, LocalResult};
use serde::Serialize;

#[derive(Clone, Serialize)]
struct MsgEntry {
    avatar_name: String,
    sim_name: String,
    message: String,
    timestamp: i64,
    x: f64,
    y: f64,
    z: f64,
}

// v1.0.0.0


/// Evaluates Ruby code from a &str and prints the result.
/// This function initializes a Ruby VM, evaluates the code, and prints the output.
/// If evaluation fails, it prints the error.


// Helper: Create a JSON response.
pub fn json_response<T: serde::Serialize>(data: T) -> tide::Response {
    tide::Response::builder(tide::StatusCode::Ok)
        .body(serde_json::to_string(&data).unwrap())
        .content_type(tide::http::mime::JSON)
        .build()
}

// Helper: Redirect to a given URL.
pub fn redirect(url: &str) -> tide::Response {
    let mut res = tide::Response::new(tide::StatusCode::Found);
    res.insert_header("Location", url);
    res
}

const SIGIL_DECK_ROOT: &str = "/root/midscore_io/tiade-maeepers-saerver-all/sigil_deck_data";
const SIGIL_DECK_DB_PATH: &str = "/root/midscore_io/tiade-maeepers-saerver-all/sigil_deck_data/deck.json";
const SIGIL_DECK_UPLOAD_DIR: &str = "/root/midscore_io/tiade-maeepers-saerver-all/sigil_deck_data/uploads";
const SIGIL_DECK_MAX_IMAGE_SIDE: u32 = 1600;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct SigilDeckEntry {
    id: u64,
    title: String,
    description: String,
    image_file: String,
    mime_type: String,
    created_at: String,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct SigilDeckDb {
    entries: Vec<SigilDeckEntry>,
}

#[derive(Clone, Debug)]
struct MultipartFile {
    filename: String,
    content_type: Option<String>,
    data: Vec<u8>,
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn sigil_deck_now_string() -> String {
    Utc::now().to_rfc3339()
}

fn sigil_deck_ensure_storage() -> tide::Result<()> {
    std::fs::create_dir_all(SIGIL_DECK_UPLOAD_DIR)
        .map_err(|e| tide::Error::from_str(tide::StatusCode::InternalServerError, e.to_string()))?;
    Ok(())
}

fn sigil_deck_upload_path(filename: &str) -> tide::Result<std::path::PathBuf> {
  let safe_name = std::path::Path::new(filename)
    .file_name()
    .and_then(|part| part.to_str())
    .filter(|part| *part == filename)
    .ok_or_else(|| tide::Error::from_str(tide::StatusCode::BadRequest, "invalid sigil filename"))?;
  Ok(std::path::Path::new(SIGIL_DECK_UPLOAD_DIR).join(safe_name))
}

fn load_sigil_deck_entries() -> tide::Result<Vec<SigilDeckEntry>> {
    sigil_deck_ensure_storage()?;
    match std::fs::read_to_string(SIGIL_DECK_DB_PATH) {
        Ok(raw) => {
            let db: SigilDeckDb = serde_json::from_str(&raw)
                .map_err(|e| tide::Error::from_str(tide::StatusCode::InternalServerError, e.to_string()))?;
            Ok(db.entries)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(tide::Error::from_str(
            tide::StatusCode::InternalServerError,
            err.to_string(),
        )),
    }
}

fn render_flashcard_page(entries: &[SigilDeckEntry]) -> String {
    let cards = entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "id": entry.id,
                "front": entry.title,
                "back": entry.description,
                "image": format!("/sigil-deck/image/{}", entry.image_file),
                "source": "Sigil deck",
            })
        })
        .collect::<Vec<_>>();
    let cards_json = serde_json::to_string(&cards)
        .unwrap_or_else(|_| "[]".to_string())
        .replace("</", "<\\/");

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Recall Deck</title>
  <style>
    :root {{ --paper: #f4f0e6; --ink: #1c2631; --muted: #64707a; --line: #d5d2c9; --navy: #18354a; --teal: #007d79; --card: #fffdf8; --shadow: 0 18px 42px rgba(20, 43, 55, .14); }}
    * {{ box-sizing: border-box; }} body {{ min-height: 100vh; margin: 0; color: var(--ink); font-family: Georgia, 'Times New Roman', serif; background-color: var(--paper); background-image: linear-gradient(rgba(24,53,74,.035) 1px, transparent 1px), linear-gradient(90deg, rgba(24,53,74,.035) 1px, transparent 1px); background-size: 28px 28px; }} button, input, textarea {{ font: inherit; }} button {{ cursor: pointer; }}
    .shell {{ width: min(1100px, calc(100% - 32px)); margin: 0 auto; padding: 28px 0 44px; }} .topbar {{ display: flex; justify-content: space-between; align-items: center; gap: 16px; border-bottom: 1px solid var(--line); padding-bottom: 18px; }} .brand {{ display: flex; align-items: center; gap: 11px; color: var(--navy); text-decoration: none; font-size: 1.05rem; font-weight: bold; }} .brand-mark {{ display: grid; place-items: center; width: 34px; height: 34px; border: 2px solid var(--teal); color: var(--teal); font-size: 1.4rem; line-height: 1; }} .text-button {{ padding: 9px 12px; border: 1px solid var(--line); color: var(--navy); background: rgba(255,255,255,.5); border-radius: 4px; text-decoration: none; }}
    .study {{ display: grid; grid-template-columns: minmax(0, 1fr) 250px; gap: 38px; align-items: start; padding-top: 40px; }} .eyebrow {{ margin: 0 0 8px; color: var(--teal); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: .75rem; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; }} h1 {{ margin: 0; color: var(--navy); font-size: clamp(2rem, 4vw, 3.4rem); line-height: 1; }} .subtitle {{ margin: 12px 0 26px; max-width: 56ch; color: var(--muted); line-height: 1.55; }} .progress-row {{ display: flex; align-items: center; justify-content: space-between; gap: 15px; margin-bottom: 12px; color: var(--muted); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: .8rem; }} .progress {{ height: 7px; overflow: hidden; background: #d7e1df; }} .progress > div {{ width: 0; height: 100%; background: var(--teal); transition: width .25s ease; }}
    .scene {{ perspective: 1200px; margin-top: 22px; }} .flashcard {{ position: relative; min-height: 390px; transform-style: preserve-3d; transition: transform .55s cubic-bezier(.2,.7,.2,1); outline: 0; }} .flashcard.is-flipped {{ transform: rotateY(180deg); }} .face {{ position: absolute; inset: 0; display: flex; flex-direction: column; justify-content: space-between; overflow: hidden; padding: 30px; border: 1px solid #c8c3b8; background: var(--card); box-shadow: var(--shadow); backface-visibility: hidden; }} .face.back {{ color: white; background: var(--navy); transform: rotateY(180deg); }} .card-top {{ display: flex; justify-content: space-between; gap: 16px; color: var(--muted); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: .75rem; letter-spacing: .04em; text-transform: uppercase; }} .back .card-top {{ color: #bcd5d4; }} .card-content {{ display: grid; grid-template-columns: minmax(0, 1fr) 150px; gap: 24px; align-items: center; flex: 1; }} .card-content.solo {{ grid-template-columns: 1fr; }} .card-question {{ margin: 0; font-size: clamp(1.6rem, 3.2vw, 2.7rem); line-height: 1.08; overflow-wrap: anywhere; }} .card-answer {{ margin: 0; font-size: clamp(1.15rem, 2.25vw, 1.65rem); line-height: 1.45; overflow-wrap: anywhere; white-space: pre-wrap; }} .card-image {{ display: block; width: 150px; height: 150px; object-fit: cover; border: 5px solid #e1ece8; }} .hint {{ margin: 18px 0 0; color: var(--muted); font-size: .9rem; }} .back .hint {{ color: #c9d7dd; }}
    .main-controls, .rating-controls {{ display: flex; flex-wrap: wrap; gap: 10px; margin-top: 18px; }} .control {{ min-width: 44px; min-height: 44px; padding: 0 14px; border: 1px solid var(--line); border-radius: 4px; color: var(--navy); background: rgba(255,255,255,.72); font-weight: bold; }} .control:hover {{ border-color: var(--teal); color: var(--teal); }} .control.primary {{ border-color: var(--teal); color: white; background: var(--teal); }} .rate {{ min-width: 110px; min-height: 42px; padding: 0 12px; border: 0; border-radius: 4px; color: #17252d; font-weight: bold; }} .rate.again {{ background: #f4b2a7; }} .rate.hard {{ background: #f5d88c; }} .rate.got-it {{ background: #9bd8c6; }}
    .side {{ padding-top: 64px; }} .side h2 {{ margin: 0 0 11px; color: var(--navy); font-size: 1rem; }} .stat {{ display: flex; align-items: baseline; justify-content: space-between; padding: 11px 0; border-top: 1px solid var(--line); }} .stat strong {{ color: var(--teal); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 1.25rem; }} .side-actions {{ display: grid; gap: 9px; margin-top: 28px; }} .side-actions button {{ width: 100%; text-align: left; }}
    dialog {{ width: min(560px, calc(100% - 28px)); padding: 0; border: 0; box-shadow: var(--shadow); background: var(--card); }} dialog::backdrop {{ background: rgba(24,53,74,.5); }} .modal {{ padding: 24px; }} .modal-header {{ display: flex; align-items: center; justify-content: space-between; gap: 12px; }} .modal h2 {{ margin: 0; color: var(--navy); }} .form-grid {{ display: grid; gap: 14px; margin-top: 20px; }} label {{ display: grid; gap: 6px; color: var(--navy); font-size: .9rem; font-weight: bold; }} input, textarea {{ width: 100%; padding: 10px; border: 1px solid var(--line); border-radius: 3px; background: white; color: var(--ink); }} textarea {{ min-height: 105px; resize: vertical; }} .modal-actions {{ display: flex; justify-content: flex-end; gap: 10px; margin-top: 20px; }} .empty {{ display: none; padding: 32px; border: 1px dashed var(--line); color: var(--muted); text-align: center; }}
    @media (max-width: 760px) {{ .shell {{ width: min(100% - 20px, 1100px); padding-top: 16px; }} .study {{ grid-template-columns: 1fr; gap: 25px; padding-top: 28px; }} .side {{ padding-top: 0; }} .side-actions {{ grid-template-columns: repeat(2, minmax(0, 1fr)); }} .flashcard {{ min-height: 430px; }} .face {{ padding: 22px; }} .card-content {{ grid-template-columns: 1fr; }} .card-image {{ width: min(150px, 45vw); height: min(150px, 45vw); }} }}
  </style>
</head>
<body>
  <main class="shell">
    <header class="topbar"><a class="brand" href="/flashcard"><span class="brand-mark">R</span>Recall Deck</a><a class="text-button" href="/sigil-deck">Open sigil deck</a></header>
    <section class="study"><div><p class="eyebrow">Active recall study session</p><h1>Learn it. Hide it. Recall it.</h1><p class="subtitle">A focused digital flashcard routine: read the prompt, reveal only after your attempt, then rate how well it came back to you.</p><div id="studyArea"><div class="progress-row"><span id="position">Card 1 of 1</span><span id="deckLabel">Sigil deck</span></div><div class="progress"><div id="progressFill"></div></div><div class="scene"><article id="flashcard" class="flashcard" tabindex="0" aria-label="Flashcard. Press space to flip."><section class="face front"><div class="card-top"><span>Prompt</span><span id="frontSource"></span></div><div id="frontContent" class="card-content"><h2 id="frontText" class="card-question"></h2><img id="frontImage" class="card-image" alt=""></div><p class="hint">Try to answer before you reveal it.</p></section><section class="face back"><div class="card-top"><span>Answer</span><span>Recall Deck</span></div><div id="backContent" class="card-content"><p id="backText" class="card-answer"></p><img id="backImage" class="card-image" alt=""></div><p class="hint">How did that feel? Rate it, then move on.</p></section></article></div><div class="main-controls"><button id="previous" class="control" type="button">Previous</button><button id="flip" class="control primary" type="button">Reveal answer</button><button id="next" class="control" type="button">Next</button><button id="shuffle" class="control" type="button">Shuffle</button></div><div id="ratings" class="rating-controls" hidden><button class="rate again" data-rating="again" type="button">Again</button><button class="rate hard" data-rating="hard" type="button">Hard</button><button class="rate got-it" data-rating="got-it" type="button">Got it</button></div></div><div id="emptyState" class="empty">No cards yet. Add a card to start a study session.</div></div><aside class="side"><h2>Session</h2><div class="stat"><span>Reviewed</span><strong id="reviewed">0</strong></div><div class="stat"><span>Got it</span><strong id="mastered">0</strong></div><div class="stat"><span>In this deck</span><strong id="deckCount">0</strong></div><div class="side-actions"><button id="editCard" class="control" type="button">Edit current card</button><button id="deleteCard" class="control" type="button">Delete current card</button><button id="answerFirst" class="control" type="button">Start with answer</button><button id="addCard" class="control primary" type="button">Add personal card</button><button id="resetProgress" class="control" type="button">Reset session</button></div></aside></section>
  </main>
  <dialog id="cardDialog"><form id="cardForm" class="modal" method="dialog"><div class="modal-header"><h2 id="dialogTitle">Add a flashcard</h2><button id="closeDialog" class="control" type="button">Close</button></div><div class="form-grid"><label>Prompt<input name="front" required placeholder="Question, term, or cue"></label><label>Answer<textarea name="back" required placeholder="Definition, explanation, or answer"></textarea></label><label>Image URL (optional)<input name="imageUrl" type="url" placeholder="https://example.com/image.jpg"></label><label>Or choose an image (optional)<input name="imageFile" type="file" accept="image/*"></label></div><div class="modal-actions"><button class="control" type="button" id="cancelDialog">Cancel</button><button id="saveCard" class="control primary" type="submit">Add card</button></div></form></dialog>
  <script id="seedCards" type="application/json">{cards_json}</script>
  <script>
    (() => {{
      const storageKey = 'recall-deck-custom-cards', stateKey = 'recall-deck-session';
      const seedCards = JSON.parse(document.getElementById('seedCards').textContent);
      const getCustomCards = () => {{ try {{ return JSON.parse(localStorage.getItem(storageKey)) || []; }} catch {{ return []; }} }};
      let cards = [...seedCards, ...getCustomCards()], index = 0, reviewed = 0, mastered = 0, answerFirst = false, editingId = null;
      const flashcard = document.getElementById('flashcard'), frontText = document.getElementById('frontText'), backText = document.getElementById('backText'), frontImage = document.getElementById('frontImage'), backImage = document.getElementById('backImage'), ratings = document.getElementById('ratings'), dialog = document.getElementById('cardDialog');
      const updateImage = (image, url, alt) => {{ image.hidden = !url; image.src = url || ''; image.alt = alt || ''; }};
      const saveSession = () => localStorage.setItem(stateKey, JSON.stringify({{ reviewed, mastered }}));
      const render = () => {{ const card = cards[index]; document.getElementById('deckCount').textContent = cards.length; document.getElementById('studyArea').hidden = !card; document.getElementById('emptyState').style.display = card ? 'none' : 'block'; if (!card) return; flashcard.classList.toggle('is-flipped', answerFirst); ratings.hidden = !answerFirst; document.getElementById('flip').textContent = answerFirst ? 'Show prompt' : 'Reveal answer'; frontText.textContent = card.front; backText.textContent = card.back; document.getElementById('frontSource').textContent = card.source || 'Personal card'; updateImage(frontImage, card.image, card.front); updateImage(backImage, card.image, card.front); document.getElementById('frontContent').classList.toggle('solo', !card.image); document.getElementById('backContent').classList.toggle('solo', !card.image); document.getElementById('position').textContent = `Card ${{index + 1}} of ${{cards.length}}`; document.getElementById('progressFill').style.width = `${{((index + 1) / cards.length) * 100}}%`; document.getElementById('deckLabel').textContent = card.source || 'Personal card'; }};
      const flip = () => {{ if (!cards.length) return; const showingAnswer = flashcard.classList.toggle('is-flipped'); document.getElementById('flip').textContent = showingAnswer ? 'Show prompt' : 'Reveal answer'; ratings.hidden = !showingAnswer; }};
      const move = amount => {{ if (!cards.length) return; index = (index + amount + cards.length) % cards.length; render(); document.getElementById('flip').textContent = 'Reveal answer'; }};
      const restore = () => {{ try {{ const saved = JSON.parse(localStorage.getItem(stateKey)); reviewed = saved?.reviewed || 0; mastered = saved?.mastered || 0; }} catch {{}} document.getElementById('reviewed').textContent = reviewed; document.getElementById('mastered').textContent = mastered; }};
      document.getElementById('flip').addEventListener('click', flip); flashcard.addEventListener('click', flip); document.getElementById('previous').addEventListener('click', () => move(-1)); document.getElementById('next').addEventListener('click', () => move(1)); document.getElementById('shuffle').addEventListener('click', () => {{ for (let cardIndex = cards.length - 1; cardIndex > 0; cardIndex--) {{ const pick = Math.floor(Math.random() * (cardIndex + 1)); [cards[cardIndex], cards[pick]] = [cards[pick], cards[cardIndex]]; }} index = 0; render(); }}); document.getElementById('answerFirst').addEventListener('click', () => {{ answerFirst = !answerFirst; document.getElementById('answerFirst').textContent = answerFirst ? 'Start with prompt' : 'Start with answer'; render(); }});
      ratings.addEventListener('click', event => {{ const rating = event.target.dataset.rating; if (!rating) return; reviewed++; if (rating === 'got-it') mastered++; document.getElementById('reviewed').textContent = reviewed; document.getElementById('mastered').textContent = mastered; saveSession(); move(1); }}); document.getElementById('resetProgress').addEventListener('click', () => {{ reviewed = 0; mastered = 0; saveSession(); restore(); }}); document.getElementById('addCard').addEventListener('click', () => {{ editingId = null; document.getElementById('dialogTitle').textContent = 'Add a flashcard'; document.getElementById('saveCard').textContent = 'Add card'; document.getElementById('cardForm').reset(); dialog.showModal(); }}); document.getElementById('editCard').addEventListener('click', () => {{ const card = cards[index]; if (!card) return; if (card.source === 'Sigil deck') {{ window.location.href = `/sigil-deck/card/${{card.id}}`; return; }} editingId = card.id; const form = document.getElementById('cardForm'); form.elements.front.value = card.front; form.elements.back.value = card.back; form.elements.imageUrl.value = card.image || ''; document.getElementById('dialogTitle').textContent = 'Edit personal card'; document.getElementById('saveCard').textContent = 'Save changes'; dialog.showModal(); }}); document.getElementById('deleteCard').addEventListener('click', () => {{ const card = cards[index]; if (!card) return; if (card.source === 'Sigil deck') {{ window.location.href = `/sigil-deck/card/${{card.id}}`; return; }} if (!confirm('Delete this personal card?')) return; const custom = getCustomCards().filter(customCard => customCard.id !== card.id); localStorage.setItem(storageKey, JSON.stringify(custom)); cards = [...seedCards, ...custom]; index = Math.max(0, Math.min(index, cards.length - 1)); render(); }}); document.getElementById('closeDialog').addEventListener('click', () => dialog.close()); document.getElementById('cancelDialog').addEventListener('click', () => dialog.close());
      document.getElementById('cardForm').addEventListener('submit', event => {{ event.preventDefault(); const form = new FormData(event.currentTarget), file = form.get('imageFile'); const save = image => {{ const custom = getCustomCards(), card = {{ id: editingId || `custom-${{Date.now()}}`, front: form.get('front').trim(), back: form.get('back').trim(), image, source: 'Personal card' }}; const cardIndex = custom.findIndex(customCard => customCard.id === editingId); if (cardIndex >= 0) custom[cardIndex] = card; else custom.push(card); localStorage.setItem(storageKey, JSON.stringify(custom)); cards = [...seedCards, ...custom]; index = cards.findIndex(currentCard => currentCard.id === card.id); editingId = null; dialog.close(); event.currentTarget.reset(); render(); }}; if (file?.size) {{ const reader = new FileReader(); reader.onload = () => save(reader.result); reader.readAsDataURL(file); }} else {{ save(form.get('imageUrl').trim()); }} }});
      document.addEventListener('keydown', event => {{ if (dialog.open || event.target.matches('input, textarea')) return; if (event.key === ' ' || event.key === 'Enter') {{ event.preventDefault(); flip(); }} if (event.key === 'ArrowRight') move(1); if (event.key === 'ArrowLeft') move(-1); }}); restore(); render();
    }})();
  </script>
</body>
</html>"##,
        cards_json = cards_json,
    )
}

fn save_sigil_deck_entries(entries: &[SigilDeckEntry]) -> tide::Result<()> {
    sigil_deck_ensure_storage()?;
    let db = SigilDeckDb {
        entries: entries.to_vec(),
    };
    let raw = serde_json::to_string_pretty(&db)
        .map_err(|e| tide::Error::from_str(tide::StatusCode::InternalServerError, e.to_string()))?;
    std::fs::write(SIGIL_DECK_DB_PATH, raw)
        .map_err(|e| tide::Error::from_str(tide::StatusCode::InternalServerError, e.to_string()))?;
    Ok(())
}

fn sigil_mime_from_ext(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}

fn sigil_ext_from_format(format: image::ImageFormat) -> Option<&'static str> {
    match format {
        image::ImageFormat::Jpeg => Some("jpg"),
        image::ImageFormat::Png => Some("png"),
        image::ImageFormat::Gif => Some("gif"),
        image::ImageFormat::WebP => Some("webp"),
        image::ImageFormat::Bmp => Some("bmp"),
        _ => None,
    }
}

  fn sigil_normalize_image(data: &[u8]) -> tide::Result<(Vec<u8>, &'static str)> {
    if data.is_empty() {
      return Err(tide::Error::from_str(
        tide::StatusCode::BadRequest,
        "sigil image cannot be empty",
      ));
    }

    let image = image::load_from_memory(data)
      .map_err(|e| tide::Error::from_str(tide::StatusCode::BadRequest, e.to_string()))?;
    let format = image::guess_format(data)
      .map_err(|e| tide::Error::from_str(tide::StatusCode::BadRequest, e.to_string()))?;
    let ext = sigil_ext_from_format(format)
      .ok_or_else(|| tide::Error::from_str(tide::StatusCode::BadRequest, "unsupported image type"))?;
    let (width, height) = image.dimensions();

    if width <= SIGIL_DECK_MAX_IMAGE_SIDE && height <= SIGIL_DECK_MAX_IMAGE_SIDE {
      return Ok((data.to_vec(), ext));
    }

    let resized = image.resize(
      SIGIL_DECK_MAX_IMAGE_SIDE,
      SIGIL_DECK_MAX_IMAGE_SIDE,
      image::imageops::FilterType::Lanczos3,
    );
    let output_format = if format == image::ImageFormat::Gif {
      image::ImageFormat::Png
    } else {
      format
    };
    let output_ext = sigil_ext_from_format(output_format)
      .ok_or_else(|| tide::Error::from_str(tide::StatusCode::BadRequest, "unsupported image type"))?;
    let mut output = std::io::Cursor::new(Vec::new());
    resized
      .write_to(&mut output, output_format)
      .map_err(|e| tide::Error::from_str(tide::StatusCode::BadRequest, e.to_string()))?;
    Ok((output.into_inner(), output_ext))
  }

  async fn download_sigil_image(image_url: &str) -> tide::Result<Vec<u8>> {
    let parsed = url::Url::parse(image_url)
      .map_err(|_| tide::Error::from_str(tide::StatusCode::BadRequest, "invalid image URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
      return Err(tide::Error::from_str(
        tide::StatusCode::BadRequest,
        "image URL must use http or https",
      ));
    }

    let mut response = surf::get(parsed.as_str())
      .await
      .map_err(|e| tide::Error::from_str(tide::StatusCode::BadGateway, e.to_string()))?;
    if !response.status().is_success() {
      return Err(tide::Error::from_str(
        tide::StatusCode::BadGateway,
        format!("image URL returned {}", response.status()),
      ));
    }
    let data = response
      .body_bytes()
      .await
      .map_err(|e| tide::Error::from_str(tide::StatusCode::BadGateway, e.to_string()))?;
    Ok(data)
  }

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|window| window == needle)
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
) -> (std::collections::HashMap<String, String>, std::collections::HashMap<String, MultipartFile>) {
    let mut fields = std::collections::HashMap::new();
    let mut files = std::collections::HashMap::new();

    let boundary = content_type
        .split(';')
        .find_map(|part| {
            let trimmed = part.trim();
            trimmed
                .strip_prefix("boundary=")
                .map(|value| value.trim_matches('"').to_string())
        })
        .unwrap_or_default();

    if boundary.is_empty() {
        return (fields, files);
    }

    let marker = format!("--{}", boundary);
    for mut part in split_by_slice(body, marker.as_bytes()) {
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
                    .map(|(_, value)| value.trim().to_string())
                    .filter(|value| !value.is_empty());
            }
        }

        let Some(name) = name else {
            continue;
        };

        if let Some(filename) = filename {
            files.insert(
                name,
                MultipartFile {
                    filename,
                    content_type: part_content_type,
                    data,
                },
            );
        } else {
            fields.insert(name, String::from_utf8_lossy(&data).trim().to_string());
        }
    }

    (fields, files)
}

fn render_sigil_deck_page(entries: &[SigilDeckEntry]) -> String {
    let mut cards = String::new();
    if entries.is_empty() {
        cards.push_str(
            r#"<section class="empty-state"><h2>No sigils in the deck yet.</h2><p>Upload the first image to seed the tarot deck.</p></section>"#,
        );
    } else {
        for entry in entries {
            let image_src = format!("/sigil-deck/image/{}", entry.image_file);
            let _ = std::fmt::Write::write_fmt(
                &mut cards,
                format_args!(
                    r#"<a class="card" href="/sigil-deck/card/{id}">
  <img src="{image_src}" alt="{title}">
  <div class="card-body">
    <h2>{title}</h2>
    <p>{description}</p>
    <span class="meta">Drawn {created_at}</span>
  </div>
</a>"#,
                    id = entry.id,
                    image_src = image_src,
                    title = escape_html(&entry.title),
                    description = escape_html(&entry.description),
                    created_at = escape_html(&entry.created_at),
                ),
            );
        }
    }

    format!(
      r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Sigil Tarot Deck</title>
  <style>
    :root {{
      color-scheme: dark;
      --bg: #0e0c12;
      --panel: rgba(19, 17, 26, 0.86);
      --panel-strong: #1e1a29;
      --text: #f4efe6;
      --muted: #b7ad9e;
      --line: rgba(255, 241, 214, 0.12);
      --gold: #e8c06a;
      --gold-strong: #ffd98a;
      --teal: #7ed9c4;
      --shadow: 0 30px 80px rgba(0, 0, 0, 0.45);
    }}

    * {{ box-sizing: border-box; }}
    html {{ scroll-behavior: smooth; }}
    body {{
      margin: 0;
      min-height: 100vh;
      color: var(--text);
      background:
        radial-gradient(circle at top left, rgba(126, 217, 196, 0.16), transparent 28%),
        radial-gradient(circle at top right, rgba(232, 192, 106, 0.18), transparent 24%),
        linear-gradient(180deg, #17131d 0%, #0e0c12 42%, #09070b 100%);
      font-family: Georgia, 'Times New Roman', serif;
    }}

    .wrap {{
      width: min(1120px, calc(100% - 24px));
      margin: 0 auto;
      padding: 18px 0 48px;
    }}

    .hero {{
      position: relative;
      overflow: hidden;
      padding: 24px;
      border: 1px solid var(--line);
      border-radius: 28px;
      background: linear-gradient(180deg, rgba(25, 21, 32, 0.96), rgba(16, 14, 22, 0.92));
      box-shadow: var(--shadow);
    }}

    .hero::after {{
      content: '';
      position: absolute;
      inset: auto -10% -35% auto;
      width: 280px;
      height: 280px;
      border-radius: 50%;
      background: radial-gradient(circle, rgba(232, 192, 106, 0.24), transparent 68%);
      pointer-events: none;
    }}

    .eyebrow {{
      margin: 0 0 10px;
      color: var(--gold-strong);
      text-transform: uppercase;
      letter-spacing: 0.22em;
      font-size: 0.74rem;
    }}

    h1 {{
      margin: 0;
      font-size: clamp(2rem, 4vw, 4rem);
      line-height: 0.96;
      max-width: 12ch;
    }}

    .lede {{
      max-width: 62ch;
      color: var(--muted);
      font-size: 1.03rem;
      line-height: 1.6;
      margin: 14px 0 0;
    }}

    .actions {{
      display: flex;
      flex-wrap: wrap;
      gap: 12px;
      margin-top: 20px;
    }}

    .button {{
      display: inline-flex;
      align-items: center;
      justify-content: center;
      min-height: 46px;
      padding: 0 18px;
      border-radius: 999px;
      border: 1px solid rgba(255, 241, 214, 0.18);
      color: var(--text);
      text-decoration: none;
      background: rgba(255, 255, 255, 0.03);
      transition: transform 0.18s ease, border-color 0.18s ease, background 0.18s ease;
    }}

    .button.primary {{
      background: linear-gradient(135deg, var(--gold), #a67528);
      color: #181106;
      font-weight: 700;
      border-color: transparent;
    }}

    .button:hover {{ transform: translateY(-1px); border-color: rgba(255, 217, 138, 0.5); }}

    .upload {{
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 14px;
      margin-top: 22px;
      padding-top: 22px;
      border-top: 1px solid var(--line);
    }}

    .field {{ display: grid; gap: 8px; }}
    .field.full {{ grid-column: 1 / -1; }}
    label {{ color: var(--muted); font-size: 0.92rem; }}
    input, textarea {{
      width: 100%;
      border-radius: 16px;
      border: 1px solid rgba(255, 241, 214, 0.12);
      background: rgba(255, 255, 255, 0.04);
      color: var(--text);
      padding: 14px 14px;
      font: inherit;
    }}
    textarea {{ min-height: 120px; resize: vertical; }}
    input[type="file"] {{ padding: 12px; }}

    .upload button {{
      grid-column: 1 / -1;
      min-height: 48px;
      border: 0;
      border-radius: 16px;
      background: linear-gradient(135deg, var(--teal), #4e8f86);
      color: #08110f;
      font: inherit;
      font-weight: 800;
    }}

    .section {{ margin-top: 20px; }}
    .section h2 {{ margin: 0 0 12px; font-size: 1.2rem; }}
    .section p {{ margin: 0 0 16px; color: var(--muted); }}

    .grid {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 14px;
    }}

    .card {{
      display: block;
      overflow: hidden;
      border-radius: 22px;
      border: 1px solid var(--line);
      background: var(--panel);
      color: var(--text);
      text-decoration: none;
      box-shadow: var(--shadow);
      min-height: 100%;
    }}

    .card img {{
      display: block;
      width: 100%;
      aspect-ratio: 4 / 5;
      object-fit: contain;
      background: var(--panel-strong);
    }}

    .card-body {{ padding: 14px; }}
    .card-body h2 {{ margin: 0 0 8px; font-size: 1.03rem; }}
    .card-body p {{ margin: 0 0 10px; color: var(--muted); line-height: 1.5; font-size: 0.96rem; }}
    .meta {{ color: rgba(244, 239, 230, 0.7); font-size: 0.82rem; letter-spacing: 0.03em; }}

    .empty-state {{
      padding: 22px;
      border-radius: 22px;
      border: 1px dashed rgba(255, 241, 214, 0.18);
      background: rgba(255, 255, 255, 0.03);
    }}

    @media (max-width: 720px) {{
      .wrap {{ width: min(100% - 16px, 1120px); padding-top: 10px; }}
      .hero {{ padding: 18px; border-radius: 22px; }}
      .upload {{ grid-template-columns: 1fr; }}
      .field.full {{ grid-column: auto; }}
      .grid {{ grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); }}
      h1 {{ max-width: 100%; }}
    }}
  </style>
</head>
<body>
  <div class="wrap">
    <section class="hero">
      <p class="eyebrow">Sigil Tarot Deck</p>
      <h1>Draw a sigil for the game.</h1>
      <p class="lede">Upload an image, write its meaning, and keep the deck in one scrollable gallery. Tap random to pull a card from the stack, or browse the collection below.</p>
      <div class="actions">
        <a class="button primary" href="/sigil-deck/random">Draw Random Card</a>
        <a class="button" href="#deck">Browse Deck</a>
      </div>
      <form class="upload" method="post" action="/sigil-deck/upload" enctype="multipart/form-data">
        <div class="field full">
          <label for="image">Image file</label>
          <input id="image" type="file" name="image" accept="image/*">
        </div>
        <div class="field full">
          <label for="image_url">Or image URL</label>
          <input id="image_url" type="url" name="image_url" placeholder="https://example.com/sigil.jpg">
        </div>
        <div class="field">
          <label for="title">Title</label>
          <input id="title" type="text" name="title" placeholder="Sigil name" required>
        </div>
        <div class="field">
          <label for="description">Description</label>
          <textarea id="description" name="description" placeholder="Meaning, effect, lore, or gameplay trigger."></textarea>
        </div>
        <button type="submit">Save to Deck</button>
      </form>
    </section>

    <section class="section" id="deck">
      <h2>Deck Selection</h2>
      <p>Scroll through the cards or draw one at random for play.</p>
      <div class="grid">
        {cards}
      </div>
    </section>
  </div>
</body>
</html>"##,
        cards = cards
    )
}

fn render_sigil_card_page(entry: &SigilDeckEntry, deck_size: usize) -> String {
    let image_src = format!("/sigil-deck/image/{}", entry.image_file);
    format!(
      r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title} - Sigil Tarot Deck</title>
  <style>
    :root {{
      color-scheme: dark;
      --panel: rgba(19, 17, 26, 0.86);
      --text: #f4efe6;
      --muted: #b7ad9e;
      --line: rgba(255, 241, 214, 0.12);
      --gold: #e8c06a;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      min-height: 100vh;
      padding: 16px;
      color: var(--text);
      background: linear-gradient(180deg, #17131d 0%, #0e0c12 100%);
      font-family: Georgia, 'Times New Roman', serif;
    }}
    .shell {{ width: min(920px, 100%); margin: 0 auto; }}
    .panel {{
      padding: 18px;
      border-radius: 26px;
      border: 1px solid var(--line);
      background: var(--panel);
      box-shadow: 0 30px 80px rgba(0, 0, 0, 0.45);
    }}
    img {{ width: 100%; display: block; border-radius: 20px; aspect-ratio: 4 / 5; object-fit: contain; background: #1a1621; }}
    h1 {{ margin: 16px 0 10px; font-size: clamp(1.8rem, 4vw, 3rem); }}
    p {{ color: var(--muted); line-height: 1.6; }}
    .meta {{ display: grid; gap: 10px; margin-top: 12px; }}
    .actions {{ display: flex; flex-wrap: wrap; gap: 12px; margin-top: 18px; }}
    .manage {{ display: grid; gap: 12px; margin-top: 24px; padding-top: 20px; border-top: 1px solid var(--line); }}
    .manage h2 {{ margin: 0; font-size: 1.2rem; }}
    .field {{ display: grid; gap: 7px; }}
    label {{ color: var(--muted); font-size: 0.92rem; }}
    input, textarea {{ width: 100%; border: 1px solid var(--line); border-radius: 10px; background: rgba(255, 255, 255, 0.05); color: var(--text); padding: 10px 12px; font: inherit; }}
    textarea {{ min-height: 110px; resize: vertical; }}
    button {{ min-height: 42px; border: 1px solid var(--line); border-radius: 10px; padding: 0 14px; background: var(--gold); color: #181106; font: inherit; font-weight: 700; cursor: pointer; }}
    .delete button {{ background: #a64c48; color: #fff7f4; }}
    a {{
      display: inline-flex;
      align-items: center;
      justify-content: center;
      min-height: 46px;
      padding: 0 18px;
      border-radius: 999px;
      border: 1px solid rgba(255, 241, 214, 0.18);
      color: var(--text);
      text-decoration: none;
      background: rgba(255, 255, 255, 0.03);
    }}
    .primary {{ background: linear-gradient(135deg, var(--gold), #a67528); color: #181106; font-weight: 700; border-color: transparent; }}
  </style>
</head>
<body>
  <div class="shell">
    <section class="panel">
      <img src="{image_src}" alt="{title}">
      <h1>{title}</h1>
      <p>{description}</p>
      <div class="meta">
        <p>Deck size: {deck_size}</p>
        <p>Created: {created_at}</p>
      </div>
      <div class="actions">
        <a class="primary" href="/sigil-deck/random">Draw Another</a>
        <a href="/sigil-deck">Back to Deck</a>
      </div>
      <form class="manage" method="post" action="/sigil-deck/card/{id}/update" enctype="multipart/form-data">
        <h2>Edit Sigil</h2>
        <div class="field">
          <label for="title">Title</label>
          <input id="title" type="text" name="title" value="{title}" required>
        </div>
        <div class="field">
          <label for="description">Description</label>
          <textarea id="description" name="description">{description}</textarea>
        </div>
        <div class="field">
          <label for="image">Replace image</label>
          <input id="image" type="file" name="image" accept="image/*">
        </div>
        <div class="field">
          <label for="image_url">Or replace from image URL</label>
          <input id="image_url" type="url" name="image_url" placeholder="https://example.com/sigil.jpg">
        </div>
        <button type="submit">Update Sigil</button>
      </form>
      <form class="delete" method="post" action="/sigil-deck/card/{id}/delete">
        <button type="submit">Delete Sigil</button>
      </form>
    </section>
  </div>
</body>
</html>"##,
        title = escape_html(&entry.title),
        description = escape_html(&entry.description),
        created_at = escape_html(&entry.created_at),
        image_src = image_src,
        id = entry.id,
        deck_size = deck_size,
    )
}

use anyhow::Result;
use image::{DynamicImage, GenericImageView};
use std::io::Cursor;

// filepath: /path/to/helpers.rs



// filepath: /path/to/blog.rs

use std::fs::{OpenOptions};
use std::io::Write;






pub fn create_blog_post(title: &str, content: &str) -> Result<()> {
    // This is a simple example writing to a file.
    let filename = format!("posts/{}.md", title.replace(" ", "_"));
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(&filename)?;
    writeln!(file, "# {}\n\n{}", title, content)?;
    Ok(())
}

// Similar functions can be created for updating or deleting posts.
// filepath: /path/to/blog.rs

use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::path::Path;



struct LogRoute;
#[tide::utils::async_trait]
impl tide::Middleware<AppState> for LogRoute {
    async fn handle(
        &self,
        req: tide::Request<AppState>,
        next: tide::Next<'_, AppState>,
    ) -> tide::Result {
        println!("Incoming route: {}", req.url().path());
        let res = next.run(req).await;
        println!("Response status: {}", res.status());
        Ok(res)
    }
}

use std::sync::mpsc;
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;
// use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::sync_channel;
use std::thread;

#[derive(Clone)]
struct AppState;

#[derive(Debug, Deserialize)]
struct LogEntrySl {
    avatar_id: Option<String>,
    avatar_name: Option<String>,
    captured_by: Option<String>,
    message: Option<String>,
    sim_name: Option<String>,
    timestamp: Option<i64>,
    x_pos: Option<f64>,
    y_pos: Option<f64>,
    z_pos: Option<f64>,
}

use tide::{Request, Response, StatusCode};
use tide::prelude::*; // for serde_json
use async_std::fs::{read_to_string};
use async_std::prelude::*;
use serde_json::{ Map};
use serde_json::Value;
use std::sync::{Arc};
use async_std::io::prelude::*; // for WriteExt



use std::collections::HashMap;
use futures_util::TryFutureExt;
#[async_std::main]
async fn main() -> tide::Result<()> {
    // Must run before `ctrlc::set_handler`: Ruby installs signal handlers during init.
    match ruby_vm::start() {
        Ok(()) => {
            if let Err(error) = ruby_vm::eval_blocking(include_str!("../ruby_client/ollama_game_client.rb")) {
                eprintln!("Failed to load ruby_client/ollama_game_client.rb into Ruby VM: {}", error);
            }
        }
        Err(error) => eprintln!("Embedded Ruby VM unavailable; Ruby-backed routes will return errors: {}", error),
    }

    // Data directory and filenames (place near top of main.rs, after imports)

  fn new_chatlog_memory_cache() -> partitioned_array_rust::PartitionedArray {
    let mut cache = partitioned_array_rust::PartitionedArray::new(1, 1, 1, false);
    cache.allocate(false);
    cache
  }

  fn new_forth_bridge_queue() -> partitioned_array_rust::PartitionedArray {
    let mut queue = partitioned_array_rust::PartitionedArray::new(1, 64, 1, true);
    queue.allocate(false);
    queue
  }

  fn new_custom_words_store() -> partitioned_array_rust::PartitionedArray {
    let mut store = partitioned_array_rust::PartitionedArray::new(1, 256, 1, true);
    store.allocate(false);
    store
  }

  fn new_avatar_frequency_store() -> partitioned_array_rust::PartitionedArray {
    let mut store = partitioned_array_rust::PartitionedArray::new(1, 1024, 1, true);
    store.allocate(false);
    store
  }

  struct ChatlogStore {
    entries: partitioned_array_rust::PartitionedArray,
    revision: u64,
  }

  struct VarsStore {
    entries: partitioned_array_rust::PartitionedArray,
    history: partitioned_array_rust::PartitionedArray,
  }

  #[derive(Serialize, Deserialize)]
  struct PersistedMemoryStores {
    version: u8,
    chatlog_entries: partitioned_array_rust::PartitionedArray,
    chatlog_revision: u64,
    variable_entries: partitioned_array_rust::PartitionedArray,
    variable_history: partitioned_array_rust::PartitionedArray,
    #[serde(default = "new_forth_bridge_queue")]
    forth_bridge_queue: partitioned_array_rust::PartitionedArray,
    // Defaults to an empty store so snapshots saved before this feature existed still load.
    #[serde(default = "new_custom_words_store")]
    custom_words: partitioned_array_rust::PartitionedArray,
    #[serde(default = "new_avatar_frequency_store")]
    avatar_frequency: partitioned_array_rust::PartitionedArray,
  }

  fn new_chatlog_store() -> ChatlogStore {
    let mut store = partitioned_array_rust::PartitionedArray::new(1, 256, 1, true);
    store.allocate(false);
    ChatlogStore {
      entries: store,
      revision: 0,
    }
  }

  fn new_vars_store() -> VarsStore {
    let mut entries = partitioned_array_rust::PartitionedArray::new(1, 64, 1, true);
    entries.allocate(false);
    let mut history = partitioned_array_rust::PartitionedArray::new(1, 64, 1, true);
    history.allocate(false);
    VarsStore { entries, history }
  }

  fn memory_store_path() -> std::path::PathBuf {
    std::env::var("MSSL_MEMORY_STORE_PATH")
      .map(std::path::PathBuf::from)
      .unwrap_or_else(|_| {
        std::path::PathBuf::from(
          "/root/midscore_io/tiade-maeepers-saerver-all/partitioned_memory_store.json",
        )
      })
  }

  fn restore_memory_stores() -> Option<(ChatlogStore, VarsStore, partitioned_array_rust::PartitionedArray, partitioned_array_rust::PartitionedArray, partitioned_array_rust::PartitionedArray)> {
    let bytes = std::fs::read(memory_store_path()).ok()?;
    let snapshot = serde_json::from_slice::<PersistedMemoryStores>(&bytes).ok()?;
    if snapshot.version != 1 {
      return None;
    }
    Some((
      ChatlogStore {
        entries: snapshot.chatlog_entries,
        revision: snapshot.chatlog_revision,
      },
      VarsStore {
        entries: snapshot.variable_entries,
        history: snapshot.variable_history,
      },
      snapshot.forth_bridge_queue,
      snapshot.custom_words,
      snapshot.avatar_frequency,
    ))
  }

  fn persist_memory_stores(state: &AppState) -> std::io::Result<()> {
    let chatlog_store = state.chatlog_store.lock().map_err(|_| {
      std::io::Error::other("chatlog store lock poisoned")
    })?;
    let vars_store = state.vars_store.lock().map_err(|_| {
      std::io::Error::other("variable store lock poisoned")
    })?;
    let forth_bridge_queue = state.forth_bridge_queue.lock().map_err(|_| {
      std::io::Error::other("Forth bridge queue lock poisoned")
    })?;
    let custom_words = state.custom_words.lock().map_err(|_| {
      std::io::Error::other("custom word store lock poisoned")
    })?;
    let avatar_frequency = state.avatar_frequency.lock().map_err(|_| {
      std::io::Error::other("avatar frequency store lock poisoned")
    })?;
    let snapshot = PersistedMemoryStores {
      version: 1,
      chatlog_entries: chatlog_store.entries.clone(),
      chatlog_revision: chatlog_store.revision,
      variable_entries: vars_store.entries.clone(),
      variable_history: vars_store.history.clone(),
      forth_bridge_queue: forth_bridge_queue.clone(),
      custom_words: custom_words.clone(),
      avatar_frequency: avatar_frequency.clone(),
    };
    let payload = serde_json::to_vec_pretty(&snapshot)
      .map_err(std::io::Error::other)?;
    let path = memory_store_path();
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)?;
    }
    let temporary_path = path.with_extension("tmp");
    std::fs::write(&temporary_path, payload)?;
    std::fs::rename(temporary_path, path)
  }

  fn vars_entry_id(store: &VarsStore, name: &str) -> Option<usize> {
    store.entries.non_empty_ids().into_iter().find(|id| {
      store.entries.get(*id)
        .and_then(|row| row.get("name"))
        .and_then(serde_json::Value::as_str)
        == Some(name)
    })
  }

  fn vars_snapshot(store: &VarsStore) -> Map<String, Value> {
    store.entries.non_empty_ids().into_iter().filter_map(|id| {
      let row = store.entries.get(id)?;
      let name = row.get("name")?.as_str()?.to_owned();
      let value = row.get("value")?.clone();
      Some((name, value))
    }).collect()
  }

  const DEFAULT_SESSION_ID: &str = "default";

  fn session_id(value: Option<&str>) -> Result<String, String> {
    let value = value.unwrap_or(DEFAULT_SESSION_ID);
    if value.is_empty() || value.len() > 64 || !value.chars().all(|character| {
      character.is_ascii_alphanumeric() || character == '_' || character == '-'
    }) {
      return Err("session_id must be 1 to 64 ASCII letters, numbers, '_' or '-'".to_string());
    }
    Ok(value.to_string())
  }

  fn session_prefix(session: &str) -> String {
    if session == DEFAULT_SESSION_ID {
      return String::new();
    }
    let encoded = session.chars().map(|character| match character {
      '_' => "__".to_string(),
      '-' => "_d".to_string(),
      character => character.to_string(),
    }).collect::<String>();
    format!("__rf_session_{}__", encoded)
  }

  fn session_storage_key(session: &str, name: &str) -> String {
    format!("{}{}", session_prefix(session), name)
  }

  fn scoped_vars_entry_id(store: &VarsStore, session: &str, name: &str) -> Option<usize> {
    vars_entry_id(store, &session_storage_key(session, name))
  }

  fn scoped_program_value(store: &VarsStore, session: &str, name: &str) -> Option<Value> {
    program_value(store, &session_storage_key(session, name))
  }

  fn scoped_program_register(store: &VarsStore, session: &str, name: &str) -> Result<i64, String> {
    match scoped_program_value(store, session, name) {
      None => Ok(0),
      Some(value) => value.as_i64().ok_or_else(|| format!("register '{}' must contain an integer", name)),
    }
  }

  fn scoped_program_set_value(store: &mut VarsStore, session: &str, name: &str, value: Value) -> Result<(), String> {
    program_set_value(store, &session_storage_key(session, name), value)
  }

  fn scoped_program_set_register(store: &mut VarsStore, session: &str, name: &str, value: i64) -> Result<(), String> {
    scoped_program_set_value(store, session, name, Value::from(value))
  }

  fn vars_snapshot_scoped(store: &VarsStore, session: &str) -> Map<String, Value> {
    let prefix = session_prefix(session);
    store.entries.non_empty_ids().into_iter().filter_map(|id| {
      let row = store.entries.get(id)?;
      let stored_name = row.get("name")?.as_str()?;
      let logical_name = if prefix.is_empty() {
        (!stored_name.starts_with("__rf_session_")).then_some(stored_name)
      } else {
        stored_name.strip_prefix(&prefix)
      }?;
      let value = row.get("value")?.clone();
      Some((logical_name.to_string(), value))
    }).collect()
  }

  fn vars_record_history_scoped(store: &mut VarsStore, session: &str, operation: String) {
    let snapshot = vars_snapshot_scoped(store, session);
    let session = session.to_string();
    let _ = store.history.add(|row| {
      row.insert("session_id".to_string(), Value::String(session.clone()));
      row.insert("operation".to_string(), Value::String(operation.clone()));
      row.insert("recorded_at".to_string(), Value::String(Utc::now().to_rfc3339()));
      row.insert("snapshot".to_string(), Value::Object(snapshot.clone()));
    });
  }

  fn vars_set_scoped(store: &mut VarsStore, session: &str, values: &Map<String, Value>) {
    for (name, value) in values {
      let key = session_storage_key(session, name);
      if let Some(id) = vars_entry_id(store, &key) {
        let _ = store.entries.set_with(id, |row| {
          row.insert("value".to_string(), value.clone());
        });
      } else {
        let _ = store.entries.add(|row| {
          row.insert("name".to_string(), Value::String(key.clone()));
          row.insert("value".to_string(), value.clone());
        });
      }
    }
    vars_record_history_scoped(store, session, "SET".to_string());
  }

  fn vars_get_scoped(store: &mut VarsStore, session: &str, name: &str) -> Option<Value> {
    let value = scoped_vars_entry_id(store, session, name)
      .and_then(|id| store.entries.get(id))
      .and_then(|row| row.get("value"))
      .cloned();
    let operation = if value.is_some() {
      format!("GET {}", name)
    } else {
      format!("GET {} not found", name)
    };
    vars_record_history_scoped(store, session, operation);
    value
  }

  fn vars_delete_scoped(store: &mut VarsStore, session: &str, name: &str) {
    if let Some(id) = scoped_vars_entry_id(store, session, name) {
      let _ = store.entries.delete(id);
    }
    vars_record_history_scoped(store, session, format!("DELETE {}", name));
  }

  fn vars_clear_scoped(store: &mut VarsStore, session: &str) {
    let prefix = session_prefix(session);
    let ids = store.entries.non_empty_ids().into_iter().filter(|id| {
      let Some(name) = store.entries.get(*id)
        .and_then(|row| row.get("name"))
        .and_then(Value::as_str) else { return false; };
      if prefix.is_empty() { !name.starts_with("__rf_session_") } else { name.starts_with(&prefix) }
    }).collect::<Vec<_>>();
    for id in ids {
      let _ = store.entries.delete(id);
    }
    vars_record_history_scoped(store, session, "CLEAR".to_string());
  }

  fn vars_history_scoped(store: &VarsStore, session: &str) -> Vec<Value> {
    store.history.non_empty_ids().into_iter().filter_map(|id| {
      let row = store.history.get(id)?;
      let recorded_session = row.get("session_id").and_then(Value::as_str).unwrap_or(DEFAULT_SESSION_ID);
      (recorded_session == session).then_some(Value::Object(row.clone()))
    }).collect()
  }

  fn vars_record_history(store: &mut VarsStore, operation: String) {
    let snapshot = vars_snapshot(store);
    let _ = store.history.add(|row| {
      row.insert("operation".to_string(), Value::String(operation.clone()));
      row.insert("recorded_at".to_string(), Value::String(Utc::now().to_rfc3339()));
      row.insert("snapshot".to_string(), Value::Object(snapshot.clone()));
    });
  }

  fn vars_set(store: &mut VarsStore, values: &Map<String, Value>) {
    for (name, value) in values {
      if let Some(id) = vars_entry_id(store, name) {
        let _ = store.entries.set_with(id, |row| {
          row.insert("value".to_string(), value.clone());
        });
      } else {
        let _ = store.entries.add(|row| {
          row.insert("name".to_string(), Value::String(name.clone()));
          row.insert("value".to_string(), value.clone());
        });
      }
    }
    vars_record_history(store, "SET".to_string());
  }

  fn vars_get(store: &mut VarsStore, name: &str) -> Option<Value> {
    let value = vars_entry_id(store, name)
      .and_then(|id| store.entries.get(id))
      .and_then(|row| row.get("value"))
      .cloned();
    let operation = if value.is_some() {
      format!("GET {}", name)
    } else {
      format!("GET {} not found", name)
    };
    vars_record_history(store, operation);
    value
  }

  fn vars_delete(store: &mut VarsStore, name: &str) {
    if let Some(id) = vars_entry_id(store, name) {
      let _ = store.entries.delete(id);
    }
    vars_record_history(store, format!("DELETE {}", name));
  }

  fn vars_clear(store: &mut VarsStore) {
    for id in store.entries.non_empty_ids() {
      let _ = store.entries.delete(id);
    }
    vars_record_history(store, "CLEAR".to_string());
  }

  #[derive(Deserialize)]
  struct ProgramInstruction {
    op: String,
    name: Option<String>,
    value: Option<Value>,
    target: Option<String>,
  }

  #[derive(Deserialize)]
  struct ProgramRequest {
    program: Vec<ProgramInstruction>,
    max_steps: Option<usize>,
  }

  fn instruction_name(instruction: &ProgramInstruction) -> Result<&str, String> {
    instruction.name.as_deref().ok_or_else(|| {
      format!("{} requires a register name", instruction.op)
    })
  }

  fn instruction_target(instruction: &ProgramInstruction) -> Result<&str, String> {
    instruction.target.as_deref().ok_or_else(|| {
      format!("{} requires a target label", instruction.op)
    })
  }

  fn program_value(store: &VarsStore, name: &str) -> Option<Value> {
    vars_entry_id(store, name)
      .and_then(|id| store.entries.get(id))
      .and_then(|row| row.get("value"))
      .cloned()
  }

  fn program_register(store: &VarsStore, name: &str) -> Result<i64, String> {
    match program_value(store, name) {
      None => Ok(0),
      Some(value) => value.as_i64().ok_or_else(|| format!("register '{}' must contain an integer", name)),
    }
  }

  fn program_set_value(store: &mut VarsStore, name: &str, value: Value) -> Result<(), String> {
    if let Some(id) = vars_entry_id(store, name) {
      let updated = store.entries.set_with(id, |row| {
        row.insert("value".to_string(), value.clone());
      });
      if !updated {
        return Err(format!("could not update register '{}'", name));
      }
    } else {
      store.entries.add(|row| {
        row.insert("name".to_string(), Value::String(name.to_string()));
        row.insert("value".to_string(), value.clone());
      }).ok_or_else(|| "partitioned variable store is full".to_string())?;
    }
    Ok(())
  }

  fn program_set_register(store: &mut VarsStore, name: &str, value: i64) -> Result<(), String> {
    program_set_value(store, name, Value::from(value))
  }

  fn run_program(store: &mut VarsStore, request: ProgramRequest) -> Result<Value, String> {
    const DEFAULT_MAX_STEPS: usize = 10_000;
    const MAX_STEPS: usize = 1_000_000;

    let max_steps = request.max_steps.unwrap_or(DEFAULT_MAX_STEPS);
    if max_steps == 0 || max_steps > MAX_STEPS {
      return Err(format!("max_steps must be between 1 and {}", MAX_STEPS));
    }

    let mut labels = HashMap::new();
    for (index, instruction) in request.program.iter().enumerate() {
      if instruction.op == "label" {
        let label = instruction_name(instruction)?.to_string();
        if labels.insert(label.clone(), index).is_some() {
          return Err(format!("duplicate label '{}'", label));
        }
      }
    }

    let mut program_counter = 0usize;
    let mut steps = 0usize;
    let mut halted = false;
    while program_counter < request.program.len() {
      if steps == max_steps {
        return Err(format!("program exceeded max_steps ({})", max_steps));
      }
      steps += 1;

      let instruction = &request.program[program_counter];
      match instruction.op.as_str() {
        "label" => program_counter += 1,
        "set" => {
          let name = instruction_name(instruction)?;
          let value = instruction.value.as_ref()
            .and_then(Value::as_i64)
            .ok_or_else(|| "set requires an integer value".to_string())?;
          program_set_register(store, name, value)?;
          program_counter += 1;
        }
        "increment" => {
          let name = instruction_name(instruction)?;
          let value = program_register(store, name)?
            .checked_add(1)
            .ok_or_else(|| format!("register '{}' overflowed", name))?;
          program_set_register(store, name, value)?;
          program_counter += 1;
        }
        "decrement" => {
          let name = instruction_name(instruction)?;
          let value = program_register(store, name)?;
          program_set_register(store, name, value.saturating_sub(1))?;
          program_counter += 1;
        }
        "jump" => {
          let target = instruction_target(instruction)?;
          program_counter = *labels.get(target)
            .ok_or_else(|| format!("unknown label '{}'", target))?;
        }
        "jump_if_nonzero" => {
          let name = instruction_name(instruction)?;
          if program_register(store, name)? != 0 {
            let target = instruction_target(instruction)?;
            program_counter = *labels.get(target)
              .ok_or_else(|| format!("unknown label '{}'", target))?;
          } else {
            program_counter += 1;
          }
        }
        "halt" => {
          halted = true;
          break;
        }
        operation => return Err(format!("unknown operation '{}'", operation)),
      }
    }

    vars_record_history(store, format!("PROGRAM executed {} steps", steps));
    Ok(serde_json::json!({
      "halted": halted,
      "steps": steps,
      "vars": vars_snapshot(store),
    }))
  }

  #[derive(Deserialize)]
  struct ForthRunRequest {
    source: String,
    max_steps: Option<usize>,
    #[serde(default)]
    session_id: Option<String>,
  }

  #[derive(Deserialize)]
  struct RubyRunRequest {
    source: String,
    max_steps: Option<usize>,
    #[serde(default)]
    session_id: Option<String>,
  }

  #[derive(Deserialize)]
  struct ForthNotecardRequest {
    name: String,
    source: String,
    #[serde(default)]
    session_id: Option<String>,
  }

  #[derive(Deserialize)]
  struct ForthNotecardNameRequest {
    name: String,
    max_steps: Option<usize>,
    #[serde(default)]
    session_id: Option<String>,
  }

  #[derive(Deserialize)]
  struct ForthFileWriteRequest {
    name: String,
    content: String,
    #[serde(default)]
    session_id: Option<String>,
  }

  #[derive(Deserialize)]
  struct ForthFileNameRequest {
    name: String,
    #[serde(default)]
    session_id: Option<String>,
  }

  #[derive(Deserialize)]
  struct ForthAlgebraRequest {
    expression: String,
    variable: Option<String>,
    at: Option<i64>,
  }

  #[derive(Deserialize)]
  struct ForthMatrixNameRequest {
    name: String,
    #[serde(default)]
    session_id: Option<String>,
  }

  #[derive(Deserialize)]
  struct SessionRequest {
    #[serde(default)]
    session_id: Option<String>,
  }

  #[derive(Deserialize)]
  struct ForthBridgeEnqueueRequest {
    token: String,
    source: String,
    max_steps: Option<usize>,
    language: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
  }

  #[derive(Deserialize)]
  struct ForthBridgePollRequest {
    token: String,
    #[serde(default)]
    session_id: Option<String>,
  }

  #[derive(Clone, Serialize, Deserialize)]
  struct ForthMatrix {
    rows: usize,
    cols: usize,
    values: Vec<i64>,
  }

  #[derive(Clone)]
  enum ForthValue {
    Number(i64),
    Text(String),
    Json(Value),
    Address(String),
  }

  fn forth_notecard_key(name: &str) -> Result<String, String> {
    if name.is_empty() || !name.chars().all(|character| {
      character.is_ascii_alphanumeric() || character == '_' || character == '-'
    }) {
      return Err("notecard name must use only letters, numbers, '_' or '-'".to_string());
    }
    Ok(format!("forth.notecard.{}", name))
  }

  fn forth_tokens(source: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut in_string = false;
    let mut escaped = false;

    for line in source.lines() {
      for character in line.chars() {
        if in_string {
          token.push(character);
          if escaped {
            escaped = false;
          } else if character == '\\' {
            escaped = true;
          } else if character == '"' {
            tokens.push(std::mem::take(&mut token));
            in_string = false;
          }
          continue;
        }
        if character == '\\' {
          break;
        }
        if character == '"' {
          if !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
          }
          token.push(character);
          in_string = true;
        } else if character == ';' {
          if !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
          }
          tokens.push(";".to_string());
        } else if character.is_whitespace() {
          if !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
          }
        } else {
          token.push(character);
        }
      }
      if in_string {
        token.push('\n');
      } else if !token.is_empty() {
        tokens.push(std::mem::take(&mut token));
      }
    }
    if in_string {
      return Err("unterminated string literal".to_string());
    }
    Ok(tokens)
  }

  fn forth_expand_words(tokens: Vec<String>) -> Result<Vec<String>, String> {
    const MAX_EXPANDED_TOKENS: usize = 1_000_000;

    let mut definitions = HashMap::new();
    let mut program = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
      if tokens[index] != ":" {
        if tokens[index] == ";" {
          index += 1;
          continue;
        }
        program.push(tokens[index].clone());
        index += 1;
        continue;
      }

      let name = tokens.get(index + 1).ok_or_else(|| "':' requires a word name".to_string())?;
      forth_variable_name(name)?;
      if definitions.contains_key(name) {
        return Err(format!("Forth word '{}' is already defined", name));
      }

      index += 2;
      let body_start = index;
      while index < tokens.len() && tokens[index] != ";" {
        if tokens[index] == ":" {
          return Err("nested Forth word definitions are not supported".to_string());
        }
        index += 1;
      }
      if index == tokens.len() {
        return Err(format!("Forth word '{}' is missing its closing ';'", name));
      }
      definitions.insert(name.clone(), tokens[body_start..index].to_vec());
      index += 1;
    }

    fn expand(
      tokens: &[String],
      definitions: &HashMap<String, Vec<String>>,
      active_words: &mut Vec<String>,
      output: &mut Vec<String>,
      max_tokens: usize,
    ) -> Result<(), String> {
      let mut index = 0usize;
      while index < tokens.len() {
        let token = &tokens[index];
        if token == "ruby" || token == "forth" {
          let source_token = tokens.get(index + 1).ok_or_else(|| {
            format!("{} requires a quoted source string", token)
          })?;
          let source = serde_json::from_str::<String>(source_token).map_err(|_| {
            format!("{} requires a quoted source string", token)
          })?;
          let bridged_tokens = if token == "ruby" {
            forth_tokens(&ruby_compile(&source)?)?
          } else {
            forth_expand_words(forth_tokens(&source)?)?
          };
          expand(&bridged_tokens, definitions, active_words, output, max_tokens)?;
          index += 2;
          continue;
        }
        if let Some(body) = definitions.get(token) {
          if active_words.iter().any(|name| name == token) {
            return Err(format!("recursive Forth word '{}' is not supported", token));
          }
          active_words.push(token.clone());
          expand(body, definitions, active_words, output, max_tokens)?;
          active_words.pop();
        } else {
          output.push(token.clone());
          if output.len() > max_tokens {
            return Err(format!("expanded Forth program exceeds {} tokens", max_tokens));
          }
        }
        index += 1;
      }
      Ok(())
    }

    let mut expanded = Vec::new();
    expand(&program, &definitions, &mut Vec::new(), &mut expanded, MAX_EXPANDED_TOKENS)?;
    Ok(expanded)
  }

  fn forth_pop_number(stack: &mut Vec<ForthValue>) -> Result<i64, String> {
    match stack.pop() {
      Some(ForthValue::Number(value)) => Ok(value),
      Some(ForthValue::Text(_)) => Err("expected a number, found text".to_string()),
      Some(ForthValue::Json(_)) => Err("expected a number, found a structured value".to_string()),
      Some(ForthValue::Address(_)) => Err("expected a number, found a variable address".to_string()),
      None => Err("stack underflow".to_string()),
    }
  }

  fn forth_pop_address(stack: &mut Vec<ForthValue>) -> Result<String, String> {
    match stack.pop() {
      Some(ForthValue::Address(name)) => Ok(name),
      Some(ForthValue::Text(_)) => Err("expected a variable address, found text".to_string()),
      Some(ForthValue::Json(_)) => Err("expected a variable address, found a structured value".to_string()),
      Some(ForthValue::Number(_)) => Err("expected a variable address, found a number".to_string()),
      None => Err("stack underflow".to_string()),
    }
  }

  fn forth_pop_text(stack: &mut Vec<ForthValue>) -> Result<String, String> {
    match stack.pop() {
      Some(ForthValue::Text(value)) => Ok(value),
      Some(ForthValue::Number(value)) => Ok(value.to_string()),
      Some(ForthValue::Json(Value::String(value))) => Ok(value),
      Some(ForthValue::Json(_)) => Err("expected text, found a structured value".to_string()),
      Some(ForthValue::Address(_)) => Err("expected text, found a variable address".to_string()),
      None => Err("stack underflow".to_string()),
    }
  }

  fn forth_value_from_json(value: Value) -> Result<ForthValue, String> {
    match value {
      Value::Number(value) => Ok(ForthValue::Number(value.as_i64()
        .ok_or_else(|| "numbers must be signed 64-bit integers".to_string())?)),
      Value::String(value) => Ok(ForthValue::Text(value)),
      value @ (Value::Array(_) | Value::Object(_) | Value::Bool(_) | Value::Null) => Ok(ForthValue::Json(value)),
    }
  }

  fn forth_value_to_json(value: ForthValue) -> Result<Value, String> {
    match value {
      ForthValue::Number(value) => Ok(Value::from(value)),
      ForthValue::Text(value) => Ok(Value::String(value)),
      ForthValue::Json(value) => Ok(value),
      ForthValue::Address(_) => Err("cannot store a variable address as a value".to_string()),
    }
  }

  fn forth_pop_key(stack: &mut Vec<ForthValue>) -> Result<String, String> {
    let value = stack.pop().ok_or_else(|| "stack underflow".to_string())?;
    forth_value_key(value)
  }

  fn forth_value_key(value: ForthValue) -> Result<String, String> {
    match value {
      ForthValue::Text(value) => Ok(value),
      ForthValue::Number(value) => Ok(value.to_string()),
      ForthValue::Json(Value::String(value)) => Ok(value),
      ForthValue::Json(_) => Err("collection keys must be text or numbers".to_string()),
      ForthValue::Address(_) => Err("collection keys cannot be variable addresses".to_string()),
    }
  }

  fn forth_value_index(value: ForthValue) -> Result<usize, String> {
    match value {
      ForthValue::Number(value) => forth_usize(value, "array index"),
      ForthValue::Text(_) | ForthValue::Json(_) | ForthValue::Address(_) => {
        Err("array indexes must be non-negative integers".to_string())
      }
    }
  }

  fn forth_pop_collection(stack: &mut Vec<ForthValue>) -> Result<Value, String> {
    match stack.pop() {
      Some(ForthValue::Json(value @ Value::Array(_))) | Some(ForthValue::Json(value @ Value::Object(_))) => Ok(value),
      Some(_) => Err("expected an array or hash".to_string()),
      None => Err("stack underflow".to_string()),
    }
  }

  fn forth_interpolate(store: &VarsStore, session: &str, template: &str) -> Result<String, String> {
    let mut rendered = String::new();
    let mut remaining = template;
    while let Some(start) = remaining.find("#{") {
      rendered.push_str(&remaining[..start]);
      let name_start = start + 2;
      let end = remaining[name_start..].find('}')
        .ok_or_else(|| "unterminated interpolation; expected '}'".to_string())? + name_start;
      let name = &remaining[name_start..end];
      forth_variable_name(name)?;
      let value = match scoped_program_value(store, session, name) {
        None => ForthValue::Number(0),
        Some(value) => forth_value_from_json(value)?,
      };
      rendered.push_str(&forth_display(value));
      remaining = &remaining[end + 1..];
    }
    rendered.push_str(remaining);
    Ok(rendered)
  }

  fn forth_display(value: ForthValue) -> String {
    match value {
      ForthValue::Number(value) => value.to_string(),
      ForthValue::Text(value) => value,
      ForthValue::Json(value) => value.to_string(),
      ForthValue::Address(name) => format!("&{}", name),
    }
  }

  fn forth_channel(value: i64) -> Result<i32, String> {
    i32::try_from(value).map_err(|_| "Second Life channels must fit a signed 32-bit integer".to_string())
  }

  fn forth_usize(value: i64, label: &str) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| format!("{} must be a non-negative integer", label))
  }

  fn forth_memory_key(address: i64) -> String {
    format!("__forth_mem_{}", address)
  }

  fn forth_memory_key_scoped(session: &str, address: i64) -> String {
    session_storage_key(session, &forth_memory_key(address))
  }

  fn forth_memory_clear(store: &mut VarsStore) {
    let cell_ids = store.entries.non_empty_ids().into_iter().filter(|id| {
      store.entries.get(*id)
        .and_then(|row| row.get("name"))
        .and_then(Value::as_str)
        .is_some_and(|name| name.starts_with("__forth_mem_"))
    }).collect::<Vec<_>>();
    for id in cell_ids {
      let _ = store.entries.delete(id);
    }
  }

  fn forth_memory_clear_scoped(store: &mut VarsStore, session: &str) {
    let prefix = session_storage_key(session, "__forth_mem_");
    let cell_ids = store.entries.non_empty_ids().into_iter().filter(|id| {
      store.entries.get(*id)
        .and_then(|row| row.get("name"))
        .and_then(Value::as_str)
        .is_some_and(|name| name.starts_with(&prefix))
    }).collect::<Vec<_>>();
    for id in cell_ids {
      let _ = store.entries.delete(id);
    }
  }

  fn forth_push_call(calls: &mut Vec<Value>, call: Value) -> Result<(), String> {
    const MAX_HOST_CALLS: usize = 128;
    if calls.len() == MAX_HOST_CALLS {
      return Err(format!("Forth program exceeded max host calls ({})", MAX_HOST_CALLS));
    }
    calls.push(call);
    Ok(())
  }

  fn forth_variable_name(name: &str) -> Result<(), String> {
    if name.is_empty() || !name.chars().enumerate().all(|(index, character)| {
      if index == 0 {
        character == '_' || character.is_ascii_alphabetic()
      } else {
        character == '_' || character.is_ascii_alphanumeric()
      }
    }) {
      return Err(format!("invalid variable name '{}'", name));
    }
    Ok(())
  }

  fn ruby_strip_comment(line: &str) -> String {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
      if let Some(active_quote) = quote {
        if escaped {
          escaped = false;
        } else if character == '\\' {
          escaped = true;
        } else if character == active_quote {
          quote = None;
        }
      } else if character == '\'' || character == '"' {
        quote = Some(character);
      } else if character == '#' {
        return line[..index].trim().to_string();
      }
    }
    line.trim().to_string()
  }

  fn ruby_split_statements(line: &str) -> Result<Vec<String>, String> {
    let mut statements = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut square_depth = 0usize;
    let mut curly_depth = 0usize;
    let mut paren_depth = 0usize;
    for (index, character) in line.char_indices() {
      if let Some(active_quote) = quote {
        if escaped {
          escaped = false;
        } else if character == '\\' {
          escaped = true;
        } else if character == active_quote {
          quote = None;
        }
        continue;
      }
      match character {
        '\'' | '"' => quote = Some(character),
        '[' => square_depth += 1,
        ']' => square_depth = square_depth.checked_sub(1).ok_or_else(|| "unmatched ']' in Ruby statement".to_string())?,
        '{' => curly_depth += 1,
        '}' => curly_depth = curly_depth.checked_sub(1).ok_or_else(|| "unmatched '}' in Ruby statement".to_string())?,
        '(' => paren_depth += 1,
        ')' => paren_depth = paren_depth.checked_sub(1).ok_or_else(|| "unmatched ')' in Ruby statement".to_string())?,
        ';' if square_depth == 0 && curly_depth == 0 && paren_depth == 0 => {
          let statement = line[start..index].trim();
          if !statement.is_empty() {
            statements.push(statement.to_string());
          }
          start = index + character.len_utf8();
        }
        _ => {}
      }
    }
    if quote.is_some() || square_depth != 0 || curly_depth != 0 || paren_depth != 0 {
      return Err("unclosed Ruby delimiter while splitting statements".to_string());
    }
    let statement = line[start..].trim();
    if !statement.is_empty() {
      statements.push(statement.to_string());
    }
    Ok(statements)
  }

  fn ruby_expression_tokens(expression: &str) -> Result<Vec<String>, String> {
    let characters = expression.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < characters.len() {
      let character = characters[index];
      if character.is_whitespace() {
        index += 1;
      } else if character == '\'' || character == '"' {
        let quote = character;
        index += 1;
        let mut value = String::new();
        while index < characters.len() && characters[index] != quote {
          if characters[index] == '\\' {
            index += 1;
            let escaped = *characters.get(index).ok_or_else(|| "unterminated string escape".to_string())?;
            value.push(match escaped {
              'n' => '\n',
              'r' => '\r',
              't' => '\t',
              other => other,
            });
          } else {
            value.push(characters[index]);
          }
          index += 1;
        }
        if index == characters.len() {
          return Err("unterminated Ruby string".to_string());
        }
        let encoded = serde_json::to_string(&value).map_err(|error| error.to_string())?;
        if value.contains("#{") {
          tokens.push(format!("__ruby_interpolate__:{}", encoded));
        } else {
          tokens.push(encoded);
        }
        index += 1;
      } else if character.is_ascii_digit() {
        let start = index;
        while index < characters.len() && (characters[index].is_ascii_digit() || characters[index] == '_') {
          index += 1;
        }
        tokens.push(characters[start..index].iter().collect::<String>().replace('_', ""));
      } else if character.is_ascii_alphabetic() || character == '_' {
        let start = index;
        while index < characters.len() && (characters[index].is_ascii_alphanumeric() || characters[index] == '_') {
          index += 1;
        }
        tokens.push(characters[start..index].iter().collect());
      } else {
        let pair = if index + 1 < characters.len() {
          Some([character, characters[index + 1]].iter().collect::<String>())
        } else {
          None
        };
        if let Some(operator @ ("==" | "!=" | "<=" | ">=" | "&&" | "||")) = pair.as_deref() {
          tokens.push(operator.to_string());
          index += 2;
        } else if matches!(character, '+' | '-' | '*' | '/' | '%' | '<' | '>' | '!' | '(' | ')') {
          tokens.push(character.to_string());
          index += 1;
        } else {
          return Err(format!("unsupported Ruby expression character '{}'", character));
        }
      }
    }
    Ok(tokens)
  }

  fn ruby_operator_precedence(operator: &str) -> Option<u8> {
    match operator {
      "!" => Some(6),
      "*" | "/" | "%" => Some(5),
      "+" | "-" => Some(4),
      "<" | "<=" | ">" | ">=" => Some(3),
      "==" | "!=" => Some(2),
      "&&" => Some(1),
      "||" => Some(0),
      _ => None,
    }
  }

  fn ruby_forth_operator(operator: &str) -> &str {
    match operator {
      "==" => "=",
      "%" => "mod",
      "&&" => "and",
      "||" => "or",
      other => other,
    }
  }

  fn ruby_split_top_level(source: &str, separator: char) -> Result<Vec<String>, String> {
    if source.trim().is_empty() {
      return Ok(Vec::new());
    }
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut square_depth = 0usize;
    let mut curly_depth = 0usize;
    let mut paren_depth = 0usize;
    for (index, character) in source.char_indices() {
      if let Some(active_quote) = quote {
        if escaped {
          escaped = false;
        } else if character == '\\' {
          escaped = true;
        } else if character == active_quote {
          quote = None;
        }
        continue;
      }
      match character {
        '\'' | '"' => quote = Some(character),
        '[' => square_depth += 1,
        ']' => square_depth = square_depth.checked_sub(1).ok_or_else(|| "unmatched ']' in Ruby collection".to_string())?,
        '{' => curly_depth += 1,
        '}' => curly_depth = curly_depth.checked_sub(1).ok_or_else(|| "unmatched '}' in Ruby collection".to_string())?,
        '(' => paren_depth += 1,
        ')' => paren_depth = paren_depth.checked_sub(1).ok_or_else(|| "unmatched ')' in Ruby collection".to_string())?,
        _ if character == separator && square_depth == 0 && curly_depth == 0 && paren_depth == 0 => {
          let part = source[start..index].trim();
          if part.is_empty() {
            return Err("empty Ruby collection entry".to_string());
          }
          parts.push(part.to_string());
          start = index + character.len_utf8();
        }
        _ => {}
      }
    }
    if quote.is_some() || square_depth != 0 || curly_depth != 0 || paren_depth != 0 {
      return Err("unclosed Ruby collection delimiter".to_string());
    }
    let part = source[start..].trim();
    if part.is_empty() {
      return Err("empty Ruby collection entry".to_string());
    }
    parts.push(part.to_string());
    Ok(parts)
  }

  fn ruby_top_level_hash_arrow(source: &str) -> Result<Option<usize>, String> {
    let mut quote = None;
    let mut escaped = false;
    let mut square_depth = 0usize;
    let mut curly_depth = 0usize;
    let mut paren_depth = 0usize;
    for (index, character) in source.char_indices() {
      if let Some(active_quote) = quote {
        if escaped {
          escaped = false;
        } else if character == '\\' {
          escaped = true;
        } else if character == active_quote {
          quote = None;
        }
        continue;
      }
      match character {
        '\'' | '"' => quote = Some(character),
        '[' => square_depth += 1,
        ']' => square_depth = square_depth.checked_sub(1).ok_or_else(|| "unmatched ']' in Ruby hash".to_string())?,
        '{' => curly_depth += 1,
        '}' => curly_depth = curly_depth.checked_sub(1).ok_or_else(|| "unmatched '}' in Ruby hash".to_string())?,
        '(' => paren_depth += 1,
        ')' => paren_depth = paren_depth.checked_sub(1).ok_or_else(|| "unmatched ')' in Ruby hash".to_string())?,
        '=' if square_depth == 0 && curly_depth == 0 && paren_depth == 0 && source[index..].starts_with("=>") => return Ok(Some(index)),
        _ => {}
      }
    }
    if quote.is_some() || square_depth != 0 || curly_depth != 0 || paren_depth != 0 {
      return Err("unclosed Ruby hash delimiter".to_string());
    }
    Ok(None)
  }

  fn ruby_compile_collection_expression(expression: &str) -> Result<Option<Vec<String>>, String> {
    let expression = expression.trim();
    if let Some(inner) = expression.strip_prefix('[').and_then(|value| value.strip_suffix(']')) {
      let values = ruby_split_top_level(inner, ',')?;
      let count = values.len();
      let mut compiled = Vec::new();
      for value in values {
        compiled.extend(ruby_compile_expression(&value)?);
      }
      compiled.push(count.to_string());
      compiled.push("array".to_string());
      return Ok(Some(compiled));
    }
    if let Some(inner) = expression.strip_prefix('{').and_then(|value| value.strip_suffix('}')) {
      let entries = ruby_split_top_level(inner, ',')?;
      let mut compiled = Vec::new();
      for entry in &entries {
        let arrow = ruby_top_level_hash_arrow(entry)?.ok_or_else(|| "Ruby hash entries require '=>'".to_string())?;
        let key = entry[..arrow].trim();
        let value = entry[arrow + 2..].trim();
        if key.is_empty() || value.is_empty() {
          return Err("Ruby hash entries require both a key and a value".to_string());
        }
        compiled.extend(ruby_compile_expression(key)?);
        compiled.extend(ruby_compile_expression(value)?);
      }
      compiled.push(entries.len().to_string());
      compiled.push("hash".to_string());
      return Ok(Some(compiled));
    }

    let Some(open) = expression.find('(') else { return Ok(None); };
    if !expression.ends_with(')') || open == 0 {
      return Ok(None);
    }
    let name = expression[..open].trim();
    if !name.chars().all(|character| character == '_' || character.is_ascii_alphanumeric()) {
      return Ok(None);
    }
    let arguments = ruby_split_top_level(&expression[open + 1..expression.len() - 1], ',')?;
    let word = match (name, arguments.len()) {
      ("get", 2) => "c.get",
      ("set", 3) => "c.set",
      ("push", 2) => "c.push",
      ("pop", 1) => "c.pop",
      ("delete", 2) => "c.delete",
      ("keys", 1) => "c.keys",
      ("length", 1) | ("len", 1) => "c.len",
      ("has", 2) => "c.has?",
      _ => return Ok(None),
    };
    let mut compiled = Vec::new();
    for argument in arguments {
      compiled.extend(ruby_compile_expression(&argument)?);
    }
    compiled.push(word.to_string());
    Ok(Some(compiled))
  }

  fn ruby_compile_expression(expression: &str) -> Result<Vec<String>, String> {
    if let Some(compiled) = ruby_compile_collection_expression(expression)? {
      return Ok(compiled);
    }
    let tokens = ruby_expression_tokens(expression)?;
    if tokens.is_empty() {
      return Err("Ruby expression is empty".to_string());
    }
    let mut output = Vec::new();
    let mut operators = Vec::new();
    let mut index = 0usize;
    let mut expects_value = true;
    while index < tokens.len() {
      let token = &tokens[index];
      if token == "(" {
        operators.push(token.clone());
        expects_value = true;
      } else if token == ")" {
        while operators.last().is_some_and(|operator| operator != "(") {
          output.push(ruby_forth_operator(&operators.pop().unwrap()).to_string());
        }
        if operators.pop().as_deref() != Some("(") {
          return Err("unmatched ')' in Ruby expression".to_string());
        }
        expects_value = false;
      } else if let Some(precedence) = ruby_operator_precedence(token) {
        if token == "-" && expects_value {
          let next = tokens.get(index + 1).ok_or_else(|| "unary '-' requires a value".to_string())?;
          if next.chars().all(|character| character.is_ascii_digit()) {
            output.push(format!("-{}", next));
            index += 1;
            expects_value = false;
          } else {
            return Err("unary '-' is supported only for integer literals".to_string());
          }
        } else {
          while operators.last().and_then(|operator| ruby_operator_precedence(operator)).is_some_and(|top| top >= precedence) {
            output.push(ruby_forth_operator(&operators.pop().unwrap()).to_string());
          }
          operators.push(token.clone());
          expects_value = true;
        }
      } else {
        if let Some(value) = token.strip_prefix("__ruby_interpolate__:") {
          output.push(value.to_string());
          output.push("interpolate".to_string());
        } else if token == "true" || token == "false" || token == "nil" || token.starts_with('"') {
          output.push(token.clone());
        } else if token.chars().all(|character| character.is_ascii_digit()) {
          output.push(token.clone());
        } else {
          forth_variable_name(token)?;
          output.push(format!("${}", token));
        }
        expects_value = false;
      }
      index += 1;
    }
    while let Some(operator) = operators.pop() {
      if operator == "(" {
        return Err("unmatched '(' in Ruby expression".to_string());
      }
      output.push(ruby_forth_operator(&operator).to_string());
    }
    Ok(output)
  }

  fn ruby_assignment(line: &str) -> Option<(&str, &str, &str)> {
    let operators = ["+=", "-=", "*=" , "/=", "="];
    for operator in operators {
      if let Some(index) = line.find(operator) {
        let left = line[..index].trim();
        let right = line[index + operator.len()..].trim();
        let previous = line[..index].chars().last();
        let next = line[index + operator.len()..].chars().next();
        let is_comparison = operator == "=" &&
          (matches!(previous, Some('!' | '<' | '>' | '=')) || matches!(next, Some('=' | '>')));
        if !left.is_empty() && !right.is_empty() && !is_comparison {
          return Some((left, operator, right));
        }
      }
    }
    None
  }

  fn ruby_compile_statement(line: &str) -> Result<Vec<String>, String> {
    if let Some(source) = line.strip_prefix("forth ") {
      let source = source.trim();
      let mut tokens = ruby_expression_tokens(source)?;
      if tokens.len() != 1 || !tokens[0].starts_with('"') {
        return Err("forth requires one quoted Forth source string".to_string());
      }
      let source = serde_json::from_str::<String>(&tokens.remove(0))
        .map_err(|_| "forth requires one quoted Forth source string".to_string())?;
      return Ok(vec!["forth".to_string(), serde_json::to_string(&source).map_err(|error| error.to_string())?]);
    }
    if let Some(expression) = line.strip_prefix("puts ").or_else(|| line.strip_prefix("p ")) {
      let mut compiled = ruby_compile_expression(expression)?;
      compiled.push("puts".to_string());
      return Ok(compiled);
    }
    if line == "return" {
      return Ok(vec!["bye".to_string()]);
    }
    if let Some((name, operator, expression)) = ruby_assignment(line) {
      forth_variable_name(name)?;
      let mut compiled = ruby_compile_expression(expression)?;
      match operator {
        "=" => compiled.push(format!("{}=", name)),
        "+=" => compiled.push(format!("{}+=", name)),
        "-=" => compiled.push(format!("{}-=", name)),
        "*=" | "/=" => {
          compiled.insert(0, format!("${}", name));
          compiled.push(if operator == "*=" { "*" } else { "/" }.to_string());
          compiled.push(format!("{}=", name));
        }
        _ => unreachable!(),
      }
      return Ok(compiled);
    }
    ruby_compile_expression(line)
  }

  fn ruby_compile_conditional(
    lines: &[String],
    index: &mut usize,
    mut condition: Vec<String>,
  ) -> Result<Vec<String>, String> {
    let then_body = ruby_compile_block(lines, index)?;
    if let Some(elsif_condition) = lines.get(*index).and_then(|line| line.strip_prefix("elsif ")) {
      *index += 1;
      let nested = ruby_compile_conditional(lines, index, ruby_compile_expression(elsif_condition)?)?;
      condition.push("if".to_string());
      condition.extend(then_body);
      condition.push("else".to_string());
      condition.extend(nested);
      condition.push("then".to_string());
      return Ok(condition);
    }
    let else_body = if lines.get(*index).is_some_and(|line| line == "else") {
      *index += 1;
      ruby_compile_block(lines, index)?
    } else {
      Vec::new()
    };
    if lines.get(*index).is_none_or(|line| line != "end") {
      return Err("if/unless requires a matching end".to_string());
    }
    *index += 1;
    condition.push("if".to_string());
    condition.extend(then_body);
    if !else_body.is_empty() {
      condition.push("else".to_string());
      condition.extend(else_body);
    }
    condition.push("then".to_string());
    Ok(condition)
  }

  fn ruby_take_forth_block(lines: &[String], index: &mut usize) -> Result<String, String> {
    let start = *index;
    while *index < lines.len() {
      if lines[*index] == "end" {
        let source = lines[start..*index].join("\n");
        *index += 1;
        if source.trim().is_empty() {
          return Err("forth do requires at least one Forth source line".to_string());
        }
        return Ok(source);
      }
      *index += 1;
    }
    Err("forth do requires a matching end".to_string())
  }

  fn ruby_compile_block(lines: &[String], index: &mut usize) -> Result<Vec<String>, String> {
    let mut compiled = Vec::new();
    while *index < lines.len() {
      let line = &lines[*index];
      if line == "end" || line == "else" || line.starts_with("elsif ") {
        break;
      }
      if line == "forth do" {
        *index += 1;
        let source = ruby_take_forth_block(lines, index)?;
        compiled.push("forth".to_string());
        compiled.push(serde_json::to_string(&source).map_err(|error| error.to_string())?);
        continue;
      }
      if let Some(condition) = line.strip_prefix("if ") {
        *index += 1;
        compiled.extend(ruby_compile_conditional(lines, index, ruby_compile_expression(condition)? )?);
        continue;
      }
      if let Some(condition) = line.strip_prefix("unless ") {
        *index += 1;
        let mut condition = ruby_compile_expression(condition)?;
        condition.push("not".to_string());
        compiled.extend(ruby_compile_conditional(lines, index, condition)?);
        continue;
      }
      if let Some(condition) = line.strip_prefix("while ").or_else(|| line.strip_prefix("until ")) {
        let is_until = line.starts_with("until ");
        *index += 1;
        let mut condition = ruby_compile_expression(condition.trim_end_matches(" do").trim())?;
        if is_until {
          condition.push("not".to_string());
        }
        let body = ruby_compile_block(lines, index)?;
        if lines.get(*index).is_none_or(|line| line != "end") {
          return Err("while/until requires a matching end".to_string());
        }
        *index += 1;
        compiled.push("begin".to_string());
        compiled.extend(condition);
        compiled.push("if".to_string());
        compiled.extend(body);
        compiled.push("again".to_string());
        compiled.push("then".to_string());
        continue;
      }
      if let Some(count) = line.strip_suffix(".times do").or_else(|| line.strip_suffix(".times")) {
        let loop_name = format!("__ruby_times_{}", *index);
        let count = ruby_compile_expression(count.trim())?;
        *index += 1;
        let body = ruby_compile_block(lines, index)?;
        if lines.get(*index).is_none_or(|line| line != "end") {
          return Err("times requires a matching end".to_string());
        }
        *index += 1;
        compiled.extend(count);
        compiled.push(format!("{}=", loop_name));
        compiled.push("begin".to_string());
        compiled.push(format!("${}", loop_name));
        compiled.push("0".to_string());
        compiled.push(">".to_string());
        compiled.push("if".to_string());
        compiled.extend(body);
        compiled.push("1".to_string());
        compiled.push(format!("{}-=", loop_name));
        compiled.push("again".to_string());
        compiled.push("then".to_string());
        continue;
      }
      compiled.extend(ruby_compile_statement(line)?);
      *index += 1;
    }
    Ok(compiled)
  }

  fn ruby_compile(source: &str) -> Result<String, String> {
    if source.len() > 65_536 {
      return Err("Ruby source may not exceed 65536 bytes".to_string());
    }
    let mut lines = Vec::new();
    let mut in_forth_block = false;
    for raw_line in source.lines() {
      let line = ruby_strip_comment(raw_line);
      if line.is_empty() {
        continue;
      }
      if in_forth_block {
        if line == "end" {
          in_forth_block = false;
        }
        lines.push(line);
        continue;
      }
      if line == "forth do" {
        in_forth_block = true;
        lines.push(line);
        continue;
      }
      lines.extend(ruby_split_statements(&line)?);
    }
    if in_forth_block {
      return Err("forth do requires a matching end".to_string());
    }
    let mut index = 0usize;
    let compiled = ruby_compile_block(&lines, &mut index)?;
    if index != lines.len() {
      return Err(format!("unexpected Ruby block terminator '{}'", lines[index]));
    }
    Ok(compiled.join(" "))
  }

  fn forth_validate_bridge_token(token: &str) -> Result<(), String> {
    let expected = std::env::var("MSSL_FORTH_BRIDGE_TOKEN")
      .ok()
      .filter(|value| !value.is_empty())
      .ok_or_else(|| "Second Life bridge is disabled; set MSSL_FORTH_BRIDGE_TOKEN".to_string())?;
    let mut difference = (expected.len() ^ token.len()) as u8;
    for (left, right) in expected.bytes().zip(token.bytes()) {
      difference |= left ^ right;
    }
    if difference == 0 {
      Ok(())
    } else {
      Err("invalid Second Life bridge token".to_string())
    }
  }

  fn run_forth(store: &mut VarsStore, session: &str, request: ForthRunRequest) -> Result<Value, String> {
    const DEFAULT_MAX_STEPS: usize = 10_000;
    const MAX_STEPS: usize = 1_000_000;

    let max_steps = request.max_steps.unwrap_or(DEFAULT_MAX_STEPS);
    if max_steps == 0 || max_steps > MAX_STEPS {
      return Err(format!("max_steps must be between 1 and {}", MAX_STEPS));
    }
    let tokens = forth_expand_words(forth_tokens(&request.source)?)?;
    let mut begin_stack = Vec::new();
    let mut begin_to_end = HashMap::new();
    let mut end_to_begin = HashMap::new();
    let mut if_stack = Vec::new();
    let mut if_to_else_or_then = HashMap::new();
    let mut else_to_then = HashMap::new();
    for (index, token) in tokens.iter().enumerate() {
      match token.as_str() {
        "begin" => begin_stack.push(index),
        "until" | "again" => {
          let begin = begin_stack.pop().ok_or_else(|| format!("{} without begin", token))?;
          begin_to_end.insert(begin, index);
          end_to_begin.insert(index, begin);
        }
        "if" => if_stack.push(index),
        "else" => {
          let if_index = if_stack.pop().ok_or_else(|| "else without if".to_string())?;
          if_to_else_or_then.insert(if_index, index);
          if_stack.push(index);
        }
        "then" => {
          let branch = if_stack.pop().ok_or_else(|| "then without if".to_string())?;
          if tokens[branch] == "else" {
            else_to_then.insert(branch, index);
          } else {
            if_to_else_or_then.insert(branch, index);
          }
        }
        _ => {}
      }
    }
    if !begin_stack.is_empty() || !if_stack.is_empty() {
      return Err("unclosed begin or if block".to_string());
    }

    let mut stack = Vec::new();
    let mut output = Vec::new();
    let mut calls = Vec::new();
    let mut program_counter = 0usize;
    let mut steps = 0usize;
    while program_counter < tokens.len() {
      if steps == max_steps {
        return Err(format!("Forth program exceeded max_steps ({})", max_steps));
      }
      steps += 1;
      let token = &tokens[program_counter];
      if let Ok(number) = token.parse::<i64>() {
        stack.push(ForthValue::Number(number));
        program_counter += 1;
        continue;
      }
      if let Some(name) = token.strip_prefix('$') {
        forth_variable_name(name)?;
        let value = match scoped_program_value(store, session, name) {
          None => ForthValue::Number(0),
          Some(value) => forth_value_from_json(value)?,
        };
        stack.push(value);
        program_counter += 1;
        continue;
      }
      if let Some(name) = token.strip_suffix("+=") {
        forth_variable_name(name)?;
        let amount = forth_pop_number(&mut stack)?;
        let value = scoped_program_register(store, session, name)?.checked_add(amount)
          .ok_or_else(|| "integer overflow".to_string())?;
        scoped_program_set_register(store, session, name, value)?;
        program_counter += 1;
        continue;
      }
      if let Some(name) = token.strip_suffix("-=") {
        forth_variable_name(name)?;
        let amount = forth_pop_number(&mut stack)?;
        let value = scoped_program_register(store, session, name)?.checked_sub(amount)
          .ok_or_else(|| "integer overflow".to_string())?;
        scoped_program_set_register(store, session, name, value)?;
        program_counter += 1;
        continue;
      }
      if !matches!(token.as_str(), "=" | "!=" | "<=" | ">=") {
        if let Some(name) = token.strip_suffix('=') {
          forth_variable_name(name)?;
          let value = forth_value_to_json(stack.pop().ok_or_else(|| "stack underflow".to_string())?)?;
          scoped_program_set_value(store, session, name, value)?;
          program_counter += 1;
          continue;
        }
      }

      if token.starts_with('"') && token.ends_with('"') {
        let text = serde_json::from_str::<String>(token)
          .map_err(|_| "invalid string literal".to_string())?;
        stack.push(ForthValue::Text(text));
        program_counter += 1;
        continue;
      }
      match token.as_str() {
        "+" => {
          let right = stack.pop().ok_or_else(|| "stack underflow".to_string())?;
          let left = stack.pop().ok_or_else(|| "stack underflow".to_string())?;
          let value = match (left, right) {
            (ForthValue::Number(left), ForthValue::Number(right)) => ForthValue::Number(
              left.checked_add(right).ok_or_else(|| "integer overflow".to_string())?
            ),
            (ForthValue::Text(left), ForthValue::Text(right)) => ForthValue::Text(format!("{}{}", left, right)),
            _ => return Err("+ requires two integers or two strings".to_string()),
          };
          stack.push(value);
          program_counter += 1;
        }
        "=" | "!=" => {
          let right = stack.pop().ok_or_else(|| "stack underflow".to_string())?;
          let left = stack.pop().ok_or_else(|| "stack underflow".to_string())?;
          let equal = match (left, right) {
            (ForthValue::Number(left), ForthValue::Number(right)) => left == right,
            (ForthValue::Text(left), ForthValue::Text(right)) => left == right,
            (ForthValue::Json(left), ForthValue::Json(right)) => left == right,
            (ForthValue::Address(left), ForthValue::Address(right)) => left == right,
            _ => false,
          };
          stack.push(ForthValue::Number(i64::from(if token == "=" { equal } else { !equal })));
          program_counter += 1;
        }
        "-" | "*" | "/" | "mod" | "<" | ">" | "<=" | ">=" => {
          let right = forth_pop_number(&mut stack)?;
          let left = forth_pop_number(&mut stack)?;
          let value = match token.as_str() {
            "-" => left.checked_sub(right).ok_or_else(|| "integer overflow".to_string())?,
            "*" => left.checked_mul(right).ok_or_else(|| "integer overflow".to_string())?,
            "/" => left.checked_div(right).ok_or_else(|| "division by zero or overflow".to_string())?,
            "mod" => left.checked_rem(right).ok_or_else(|| "division by zero or overflow".to_string())?,
            "<" => i64::from(left < right),
            ">" => i64::from(left > right),
            "<=" => i64::from(left <= right),
            ">=" => i64::from(left >= right),
            _ => unreachable!(),
          };
          stack.push(ForthValue::Number(value));
          program_counter += 1;
        }
        "0=" => {
          let value = forth_pop_number(&mut stack)?;
          stack.push(ForthValue::Number(i64::from(value == 0)));
          program_counter += 1;
        }
        "true" => {
          stack.push(ForthValue::Number(1));
          program_counter += 1;
        }
        "false" | "nil" => {
          stack.push(ForthValue::Number(0));
          program_counter += 1;
        }
        "not" => {
          let value = forth_pop_number(&mut stack)?;
          stack.push(ForthValue::Number(i64::from(value == 0)));
          program_counter += 1;
        }
        "&&" | "and" | "||" | "or" => {
          let right = forth_pop_number(&mut stack)? != 0;
          let left = forth_pop_number(&mut stack)? != 0;
          let value = if token == "&&" || token == "and" { left && right } else { left || right };
          stack.push(ForthValue::Number(i64::from(value)));
          program_counter += 1;
        }
        "abs" => {
          let value = forth_pop_number(&mut stack)?.checked_abs()
            .ok_or_else(|| "integer overflow".to_string())?;
          stack.push(ForthValue::Number(value));
          program_counter += 1;
        }
        "min" | "max" => {
          let right = forth_pop_number(&mut stack)?;
          let left = forth_pop_number(&mut stack)?;
          stack.push(ForthValue::Number(if token == "min" { left.min(right) } else { left.max(right) }));
          program_counter += 1;
        }
        "now" => {
          stack.push(ForthValue::Number(Utc::now().timestamp()));
          program_counter += 1;
        }
        "rand" => {
          let upper_bound = forth_pop_number(&mut stack)?;
          if upper_bound <= 0 {
            return Err("rand requires a positive upper bound".to_string());
          }
          use rand::Rng;
          stack.push(ForthValue::Number(rand::thread_rng().gen_range(0..upper_bound)));
          program_counter += 1;
        }
        "dup" => {
          let value = stack.last().cloned().ok_or_else(|| "stack underflow".to_string())?;
          stack.push(value);
          program_counter += 1;
        }
        "drop" => {
          stack.pop().ok_or_else(|| "stack underflow".to_string())?;
          program_counter += 1;
        }
        "swap" => {
          if stack.len() < 2 { return Err("stack underflow".to_string()); }
          let end = stack.len() - 1;
          stack.swap(end, end - 1);
          program_counter += 1;
        }
        "over" => {
          if stack.len() < 2 { return Err("stack underflow".to_string()); }
          stack.push(stack[stack.len() - 2].clone());
          program_counter += 1;
        }
        "." | "puts" | "p" => {
          let value = stack.pop().ok_or_else(|| "stack underflow".to_string())?;
          output.push(forth_display(value));
          program_counter += 1;
        }
        "sl.say" | "sl.whisper" | "sl.shout" | "sl.region_say" => {
          let message = forth_pop_text(&mut stack)?;
          let channel = forth_channel(forth_pop_number(&mut stack)?)?;
          let operation = match token.as_str() {
            "sl.say" => "say",
            "sl.whisper" => "whisper",
            "sl.shout" => "shout",
            "sl.region_say" => "region_say",
            _ => unreachable!(),
          };
          forth_push_call(&mut calls, serde_json::json!({
            "op": operation,
            "channel": channel,
            "message": message,
          }))?;
          program_counter += 1;
        }
        "sl.owner_say" => {
          let message = forth_pop_text(&mut stack)?;
          forth_push_call(&mut calls, serde_json::json!({ "op": "owner_say", "message": message }))?;
          program_counter += 1;
        }
        "sl.set_text" => {
          let alpha = forth_pop_number(&mut stack)?;
          let blue = forth_pop_number(&mut stack)?;
          let green = forth_pop_number(&mut stack)?;
          let red = forth_pop_number(&mut stack)?;
          let text = forth_pop_text(&mut stack)?;
          forth_push_call(&mut calls, serde_json::json!({
            "op": "set_text",
            "text": text,
            "red": red,
            "green": green,
            "blue": blue,
            "alpha": alpha,
          }))?;
          program_counter += 1;
        }
        "sl.set_color" => {
          let face = forth_pop_number(&mut stack)?;
          let blue = forth_pop_number(&mut stack)?;
          let green = forth_pop_number(&mut stack)?;
          let red = forth_pop_number(&mut stack)?;
          forth_push_call(&mut calls, serde_json::json!({
            "op": "set_color",
            "face": face,
            "red": red,
            "green": green,
            "blue": blue,
          }))?;
          program_counter += 1;
        }
        "sl.set_alpha" => {
          let face = forth_pop_number(&mut stack)?;
          let alpha = forth_pop_number(&mut stack)?;
          forth_push_call(&mut calls, serde_json::json!({ "op": "set_alpha", "face": face, "alpha": alpha }))?;
          program_counter += 1;
        }
        "sl.play_sound" => {
          let volume = forth_pop_number(&mut stack)?;
          let sound = forth_pop_text(&mut stack)?;
          forth_push_call(&mut calls, serde_json::json!({ "op": "play_sound", "sound": sound, "volume": volume }))?;
          program_counter += 1;
        }
        "sl.set_timer" => {
          let seconds = forth_pop_number(&mut stack)?;
          forth_push_call(&mut calls, serde_json::json!({ "op": "set_timer", "seconds": seconds }))?;
          program_counter += 1;
        }
        "sl.set_region_pos" => {
          let z = forth_pop_number(&mut stack)?;
          let y = forth_pop_number(&mut stack)?;
          let x = forth_pop_number(&mut stack)?;
          forth_push_call(&mut calls, serde_json::json!({ "op": "set_region_pos", "x": x, "y": y, "z": z }))?;
          program_counter += 1;
        }
        "sl.link_message" => {
          let id = forth_pop_text(&mut stack)?;
          let message = forth_pop_text(&mut stack)?;
          let code = forth_pop_number(&mut stack)?;
          let link = forth_pop_number(&mut stack)?;
          forth_push_call(&mut calls, serde_json::json!({
            "op": "link_message",
            "link": link,
            "code": code,
            "message": message,
            "id": id,
          }))?;
          program_counter += 1;
        }
        "matrix" => {
          let name = tokens.get(program_counter + 1).ok_or_else(|| "matrix requires a name".to_string())?;
          let cols = forth_usize(forth_pop_number(&mut stack)?, "matrix columns")?;
          let rows = forth_usize(forth_pop_number(&mut stack)?, "matrix rows")?;
          forth_matrix_save_scoped(store, session, name, forth_matrix_new(rows, cols)?)?;
          program_counter += 2;
        }
        "m.identity" => {
          let name = tokens.get(program_counter + 1).ok_or_else(|| "m.identity requires a name".to_string())?;
          let size = forth_usize(forth_pop_number(&mut stack)?, "identity size")?;
          let mut matrix = forth_matrix_new(size, size)?;
          for index in 0..size {
            matrix.values[index * size + index] = 1;
          }
          forth_matrix_save_scoped(store, session, name, matrix)?;
          program_counter += 2;
        }
        "mget" => {
          let name = tokens.get(program_counter + 1).ok_or_else(|| "mget requires a matrix name".to_string())?;
          let col = forth_usize(forth_pop_number(&mut stack)?, "matrix column")?;
          let row = forth_usize(forth_pop_number(&mut stack)?, "matrix row")?;
          let matrix = forth_matrix_load_scoped(store, session, name)?;
          stack.push(ForthValue::Number(matrix.values[forth_matrix_index(&matrix, row, col)?]));
          program_counter += 2;
        }
        "mset" => {
          let name = tokens.get(program_counter + 1).ok_or_else(|| "mset requires a matrix name".to_string())?;
          let col = forth_usize(forth_pop_number(&mut stack)?, "matrix column")?;
          let row = forth_usize(forth_pop_number(&mut stack)?, "matrix row")?;
          let value = forth_pop_number(&mut stack)?;
          let mut matrix = forth_matrix_load_scoped(store, session, name)?;
          let index = forth_matrix_index(&matrix, row, col)?;
          matrix.values[index] = value;
          forth_matrix_save_scoped(store, session, name, matrix)?;
          program_counter += 2;
        }
        "m.fill" => {
          let name = tokens.get(program_counter + 1).ok_or_else(|| "m.fill requires a matrix name".to_string())?;
          let value = forth_pop_number(&mut stack)?;
          let mut matrix = forth_matrix_load_scoped(store, session, name)?;
          matrix.values.fill(value);
          forth_matrix_save_scoped(store, session, name, matrix)?;
          program_counter += 2;
        }
        "m.scale" => {
          let source_name = tokens.get(program_counter + 1).ok_or_else(|| "m.scale requires a source matrix".to_string())?;
          let target_name = tokens.get(program_counter + 2).ok_or_else(|| "m.scale requires a target matrix".to_string())?;
          let scalar = forth_pop_number(&mut stack)?;
          let matrix = forth_matrix_scale(&forth_matrix_load_scoped(store, session, source_name)?, scalar)?;
          forth_matrix_save_scoped(store, session, target_name, matrix)?;
          program_counter += 3;
        }
        "m.add" | "m.sub" | "m.mul" => {
          let left_name = tokens.get(program_counter + 1).ok_or_else(|| format!("{} requires a left matrix", token))?;
          let right_name = tokens.get(program_counter + 2).ok_or_else(|| format!("{} requires a right matrix", token))?;
          let target_name = tokens.get(program_counter + 3).ok_or_else(|| format!("{} requires a target matrix", token))?;
          let left = forth_matrix_load_scoped(store, session, left_name)?;
          let right = forth_matrix_load_scoped(store, session, right_name)?;
          let result = match token.as_str() {
            "m.add" => forth_matrix_add(&left, &right, false)?,
            "m.sub" => forth_matrix_add(&left, &right, true)?,
            "m.mul" => forth_matrix_multiply(&left, &right)?,
            _ => unreachable!(),
          };
          forth_matrix_save_scoped(store, session, target_name, result)?;
          program_counter += 4;
        }
        "m.solve" => {
          let left_name = tokens.get(program_counter + 1).ok_or_else(|| "m.solve requires a left matrix".to_string())?;
          let right_name = tokens.get(program_counter + 2).ok_or_else(|| "m.solve requires a right matrix".to_string())?;
          let target_name = tokens.get(program_counter + 3).ok_or_else(|| "m.solve requires a target matrix".to_string())?;
          let solution = forth_matrix_solve(
            &forth_matrix_load_scoped(store, session, left_name)?,
            &forth_matrix_load_scoped(store, session, right_name)?,
          )?;
          forth_matrix_save_scoped(store, session, target_name, solution)?;
          program_counter += 4;
        }
        "m.transpose" => {
          let source_name = tokens.get(program_counter + 1).ok_or_else(|| "m.transpose requires a source matrix".to_string())?;
          let target_name = tokens.get(program_counter + 2).ok_or_else(|| "m.transpose requires a target matrix".to_string())?;
          let matrix = forth_matrix_transpose(&forth_matrix_load_scoped(store, session, source_name)?)?;
          forth_matrix_save_scoped(store, session, target_name, matrix)?;
          program_counter += 3;
        }
        "m.det" => {
          let name = tokens.get(program_counter + 1).ok_or_else(|| "m.det requires a matrix name".to_string())?;
          stack.push(ForthValue::Number(forth_matrix_determinant(&forth_matrix_load_scoped(store, session, name)?)?));
          program_counter += 2;
        }
        "m.rows" | "m.cols" => {
          let name = tokens.get(program_counter + 1).ok_or_else(|| format!("{} requires a matrix name", token))?;
          let matrix = forth_matrix_load_scoped(store, session, name)?;
          let dimension = if token == "m.rows" { matrix.rows } else { matrix.cols };
          stack.push(ForthValue::Number(i64::try_from(dimension).map_err(|_| "matrix dimension overflow".to_string())?));
          program_counter += 2;
        }
        "m.show" => {
          let name = tokens.get(program_counter + 1).ok_or_else(|| "m.show requires a matrix name".to_string())?;
          let matrix = forth_matrix_load_scoped(store, session, name)?;
          output.push(serde_json::to_string(&matrix).map_err(|error| error.to_string())?);
          program_counter += 2;
        }
        "array" => {
          let count = forth_usize(forth_pop_number(&mut stack)?, "array item count")?;
          if count > stack.len() {
            return Err("array requires more stack values than are available".to_string());
          }
          let start = stack.len() - count;
          let values = stack.split_off(start).into_iter()
            .map(forth_value_to_json)
            .collect::<Result<Vec<_>, _>>()?;
          stack.push(ForthValue::Json(Value::Array(values)));
          program_counter += 1;
        }
        "hash" => {
          let count = forth_usize(forth_pop_number(&mut stack)?, "hash pair count")?;
          let required = count.checked_mul(2).ok_or_else(|| "hash pair count is too large".to_string())?;
          if required > stack.len() {
            return Err("hash requires key/value pairs that are not on the stack".to_string());
          }
          let start = stack.len() - required;
          let mut entries = stack.split_off(start).into_iter();
          let mut object = Map::new();
          while let Some(key) = entries.next() {
            let value = entries.next().expect("validated key/value pair count");
            object.insert(forth_value_key(key)?, forth_value_to_json(value)?);
          }
          stack.push(ForthValue::Json(Value::Object(object)));
          program_counter += 1;
        }
        "c.len" | "a.len" | "h.len" => {
          let collection = forth_pop_collection(&mut stack)?;
          let length = match collection {
            Value::Array(values) => values.len(),
            Value::Object(values) => values.len(),
            _ => unreachable!(),
          };
          stack.push(ForthValue::Number(i64::try_from(length).map_err(|_| "collection length overflow".to_string())?));
          program_counter += 1;
        }
        "c.get" | "a.get" | "h.get" => {
          let key = stack.pop().ok_or_else(|| "stack underflow".to_string())?;
          let collection = forth_pop_collection(&mut stack)?;
          let value = match collection {
            Value::Array(values) => values.get(forth_value_index(key)?)
              .cloned().ok_or_else(|| "array index is out of bounds".to_string())?,
            Value::Object(values) => values.get(&forth_value_key(key)?)
              .cloned().unwrap_or(Value::Null),
            _ => unreachable!(),
          };
          stack.push(forth_value_from_json(value)?);
          program_counter += 1;
        }
        "c.set" | "a.set" | "h.set" => {
          let value = forth_value_to_json(stack.pop().ok_or_else(|| "stack underflow".to_string())?)?;
          let key = stack.pop().ok_or_else(|| "stack underflow".to_string())?;
          let mut collection = forth_pop_collection(&mut stack)?;
          match &mut collection {
            Value::Array(values) => {
              let index = forth_value_index(key)?;
              let slot = values.get_mut(index).ok_or_else(|| "array index is out of bounds".to_string())?;
              *slot = value;
            }
            Value::Object(values) => {
              values.insert(forth_value_key(key)?, value);
            }
            _ => unreachable!(),
          }
          stack.push(ForthValue::Json(collection));
          program_counter += 1;
        }
        "c.push" | "a.push" => {
          let value = forth_value_to_json(stack.pop().ok_or_else(|| "stack underflow".to_string())?)?;
          let mut collection = forth_pop_collection(&mut stack)?;
          match &mut collection {
            Value::Array(values) => values.push(value),
            Value::Object(_) => return Err("c.push requires an array".to_string()),
            _ => unreachable!(),
          }
          stack.push(ForthValue::Json(collection));
          program_counter += 1;
        }
        "c.pop" | "a.pop" => {
          let mut collection = forth_pop_collection(&mut stack)?;
          let value = match &mut collection {
            Value::Array(values) => values.pop().unwrap_or(Value::Null),
            Value::Object(_) => return Err("c.pop requires an array".to_string()),
            _ => unreachable!(),
          };
          stack.push(forth_value_from_json(value)?);
          stack.push(ForthValue::Json(collection));
          program_counter += 1;
        }
        "c.delete" | "a.delete" | "h.delete" => {
          let key = stack.pop().ok_or_else(|| "stack underflow".to_string())?;
          let mut collection = forth_pop_collection(&mut stack)?;
          match &mut collection {
            Value::Array(values) => {
              let index = forth_value_index(key)?;
              if index >= values.len() {
                return Err("array index is out of bounds".to_string());
              }
              values.remove(index);
            }
            Value::Object(values) => {
              values.remove(&forth_value_key(key)?);
            }
            _ => unreachable!(),
          }
          stack.push(ForthValue::Json(collection));
          program_counter += 1;
        }
        "c.has?" | "h.has?" => {
          let key = stack.pop().ok_or_else(|| "stack underflow".to_string())?;
          let collection = forth_pop_collection(&mut stack)?;
          let exists = match collection {
            Value::Array(values) => forth_value_index(key).ok().is_some_and(|index| index < values.len()),
            Value::Object(values) => values.contains_key(&forth_value_key(key)?),
            _ => unreachable!(),
          };
          stack.push(ForthValue::Number(i64::from(exists)));
          program_counter += 1;
        }
        "c.keys" | "h.keys" => {
          let collection = forth_pop_collection(&mut stack)?;
          let values = match collection {
            Value::Object(values) => values.into_iter().map(|(key, _)| Value::String(key)).collect(),
            Value::Array(_) => return Err("c.keys requires a hash".to_string()),
            _ => unreachable!(),
          };
          stack.push(ForthValue::Json(Value::Array(values)));
          program_counter += 1;
        }
        "null" => {
          stack.push(ForthValue::Json(Value::Null));
          program_counter += 1;
        }
        "null?" => {
          let is_null = matches!(stack.pop(), Some(ForthValue::Json(Value::Null)));
          stack.push(ForthValue::Number(i64::from(is_null)));
          program_counter += 1;
        }
        "interpolate" | "interp" => {
          let template = forth_pop_text(&mut stack)?;
          stack.push(ForthValue::Text(forth_interpolate(store, session, &template)?));
          program_counter += 1;
        }
        "mem@" => {
          let address = forth_pop_number(&mut stack)?;
          stack.push(ForthValue::Number(program_register(store, &forth_memory_key_scoped(session, address))?));
          program_counter += 1;
        }
        "mem!" => {
          let address = forth_pop_number(&mut stack)?;
          let value = forth_pop_number(&mut stack)?;
          program_set_register(store, &forth_memory_key_scoped(session, address), value)?;
          program_counter += 1;
        }
        "mem.clear" => {
          forth_memory_clear_scoped(store, session);
          program_counter += 1;
        }
        "variable" | "let" => {
          let name = tokens.get(program_counter + 1).ok_or_else(|| "variable requires a name".to_string())?;
          forth_variable_name(name)?;
          if scoped_vars_entry_id(store, session, name).is_none() {
            scoped_program_set_register(store, session, name, 0)?;
          }
          program_counter += 2;
        }
        "@" => {
          let name = forth_pop_address(&mut stack)?;
          let value = match scoped_program_value(store, session, &name) {
            None => ForthValue::Number(0),
            Some(value) => forth_value_from_json(value)?,
          };
          stack.push(value);
          program_counter += 1;
        }
        "!" => {
          let name = forth_pop_address(&mut stack)?;
          let value = forth_value_to_json(stack.pop().ok_or_else(|| "stack underflow".to_string())?)?;
          scoped_program_set_value(store, session, &name, value)?;
          program_counter += 1;
        }
        "if" => {
          if forth_pop_number(&mut stack)? == 0 {
            program_counter = if_to_else_or_then.get(&program_counter)
              .ok_or_else(|| "if without then".to_string())? + 1;
          } else {
            program_counter += 1;
          }
        }
        "else" => {
          program_counter = else_to_then.get(&program_counter)
            .ok_or_else(|| "else without then".to_string())? + 1;
        }
        "then" | "begin" => program_counter += 1,
        "until" => {
          if forth_pop_number(&mut stack)? == 0 {
            program_counter = end_to_begin.get(&program_counter)
              .ok_or_else(|| "until without begin".to_string())? + 1;
          } else {
            program_counter += 1;
          }
        }
        "again" => {
          program_counter = end_to_begin.get(&program_counter)
            .ok_or_else(|| "again without begin".to_string())? + 1;
        }
        "bye" => break,
        name if scoped_vars_entry_id(store, session, name).is_some() => {
          stack.push(ForthValue::Address(name.to_string()));
          program_counter += 1;
        }
        word => return Err(format!("unknown Forth word '{}'", word)),
      }
    }

    vars_record_history_scoped(store, session, format!("FORTH executed {} steps", steps));
    let stack = stack.into_iter().map(|value| match value {
      ForthValue::Address(name) => Ok(Value::String(format!("&{}", name))),
      value => forth_value_to_json(value),
    }).collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::json!({
      "steps": steps,
      "output": output.join(" "),
      "calls": calls,
      "stack": stack,
      "session_id": session,
      "vars": vars_snapshot_scoped(store, session),
    }))
  }

  fn forth_save_notecard(store: &mut VarsStore, name: &str, source: String) -> Result<(), String> {
    let key = forth_notecard_key(name)?;
    let mut values = Map::new();
    values.insert(key, Value::String(source));
    vars_set(store, &values);
    Ok(())
  }

  fn forth_save_notecard_scoped(store: &mut VarsStore, session: &str, name: &str, source: String) -> Result<(), String> {
    let key = session_storage_key(session, &forth_notecard_key(name)?);
    let mut values = Map::new();
    values.insert(key, Value::String(source));
    vars_set(store, &values);
    Ok(())
  }

  fn forth_load_notecard(store: &VarsStore, name: &str) -> Result<String, String> {
    let key = forth_notecard_key(name)?;
    vars_entry_id(store, &key)
      .and_then(|id| store.entries.get(id))
      .and_then(|row| row.get("value"))
      .and_then(Value::as_str)
      .map(str::to_string)
      .ok_or_else(|| format!("notecard '{}' was not found", name))
  }

  fn forth_load_notecard_scoped(store: &VarsStore, session: &str, name: &str) -> Result<String, String> {
    let key = session_storage_key(session, &forth_notecard_key(name)?);
    vars_entry_id(store, &key)
      .and_then(|id| store.entries.get(id))
      .and_then(|row| row.get("value"))
      .and_then(Value::as_str)
      .map(str::to_string)
      .ok_or_else(|| format!("notecard '{}' was not found", name))
  }

  fn forth_file_root() -> std::path::PathBuf {
    std::env::var("MSSL_FORTH_FILE_ROOT")
      .map(std::path::PathBuf::from)
      .unwrap_or_else(|_| {
        std::path::PathBuf::from("/root/midscore_io/tiade-maeepers-saerver-all/forth_files")
      })
  }

  fn forth_file_root_scoped(session: &str) -> std::path::PathBuf {
    forth_file_root().join(session)
  }

  fn forth_file_path(name: &str) -> Result<std::path::PathBuf, String> {
    if name.is_empty() || name.len() > 128 || name.starts_with('.') || name.contains("..") ||
      !name.chars().all(|character| {
        character.is_ascii_alphanumeric() || character == '_' || character == '-' || character == '.'
      }) {
      return Err("file name must be a safe basename of up to 128 characters".to_string());
    }
    let root = forth_file_root();
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    Ok(root.join(name))
  }

  fn forth_file_path_scoped(session: &str, name: &str) -> Result<std::path::PathBuf, String> {
    let path = forth_file_path(name)?;
    let file_name = path.file_name().ok_or_else(|| "invalid file name".to_string())?;
    let root = forth_file_root_scoped(session);
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    Ok(root.join(file_name))
  }

  fn forth_write_file(name: &str, content: &str) -> Result<usize, String> {
    const MAX_FILE_BYTES: usize = 65_536;
    if content.len() > MAX_FILE_BYTES {
      return Err(format!("file content may not exceed {} bytes", MAX_FILE_BYTES));
    }
    let path = forth_file_path(name)?;
    std::fs::write(path, content).map_err(|error| error.to_string())?;
    Ok(content.len())
  }

  fn forth_read_file(name: &str) -> Result<String, String> {
    let path = forth_file_path(name)?;
    std::fs::read_to_string(path).map_err(|error| error.to_string())
  }

  fn forth_list_files() -> Result<Vec<String>, String> {
    let root = forth_file_root();
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let mut files = std::fs::read_dir(root).map_err(|error| error.to_string())?
      .filter_map(Result::ok)
      .filter_map(|entry| {
        entry.file_type().ok()?.is_file().then(|| entry.file_name().into_string().ok()).flatten()
      })
      .collect::<Vec<_>>();
    files.sort();
    Ok(files)
  }

  fn forth_delete_file(name: &str) -> Result<bool, String> {
    let path = forth_file_path(name)?;
    match std::fs::remove_file(path) {
      Ok(()) => Ok(true),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
      Err(error) => Err(error.to_string()),
    }
  }

  fn forth_write_file_scoped(session: &str, name: &str, content: &str) -> Result<usize, String> {
    const MAX_FILE_BYTES: usize = 65_536;
    if content.len() > MAX_FILE_BYTES {
      return Err(format!("file content may not exceed {} bytes", MAX_FILE_BYTES));
    }
    let path = forth_file_path_scoped(session, name)?;
    std::fs::write(path, content).map_err(|error| error.to_string())?;
    Ok(content.len())
  }

  fn forth_read_file_scoped(session: &str, name: &str) -> Result<String, String> {
    let path = forth_file_path_scoped(session, name)?;
    std::fs::read_to_string(path).map_err(|error| error.to_string())
  }

  fn forth_list_files_scoped(session: &str) -> Result<Vec<String>, String> {
    let root = forth_file_root_scoped(session);
    std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let mut files = std::fs::read_dir(root).map_err(|error| error.to_string())?
      .filter_map(Result::ok)
      .filter_map(|entry| entry.file_type().ok()?.is_file().then(|| entry.file_name().into_string().ok()).flatten())
      .collect::<Vec<_>>();
    files.sort();
    Ok(files)
  }

  fn forth_delete_file_scoped(session: &str, name: &str) -> Result<bool, String> {
    let path = forth_file_path_scoped(session, name)?;
    match std::fs::remove_file(path) {
      Ok(()) => Ok(true),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
      Err(error) => Err(error.to_string()),
    }
  }

  fn forth_matrix_key(name: &str) -> Result<String, String> {
    forth_variable_name(name)?;
    Ok(format!("forth.matrix.{}", name))
  }

  fn forth_matrix_validate(matrix: &ForthMatrix) -> Result<(), String> {
    const MAX_MATRIX_DIMENSION: usize = 64;
    const MAX_MATRIX_CELLS: usize = 4_096;
    if matrix.rows == 0 || matrix.cols == 0 || matrix.rows > MAX_MATRIX_DIMENSION || matrix.cols > MAX_MATRIX_DIMENSION {
      return Err(format!("matrix dimensions must be between 1 and {}", MAX_MATRIX_DIMENSION));
    }
    let cells = matrix.rows.checked_mul(matrix.cols).ok_or_else(|| "matrix dimensions overflow".to_string())?;
    if cells > MAX_MATRIX_CELLS || matrix.values.len() != cells {
      return Err(format!("matrix must contain exactly {} cells", cells));
    }
    Ok(())
  }

  fn forth_store_json(store: &mut VarsStore, name: &str, value: Value) -> Result<(), String> {
    if let Some(id) = vars_entry_id(store, name) {
      if !store.entries.set_with(id, |row| {
        row.insert("value".to_string(), value.clone());
      }) {
        return Err(format!("could not update '{}'", name));
      }
    } else {
      store.entries.add(|row| {
        row.insert("name".to_string(), Value::String(name.to_string()));
        row.insert("value".to_string(), value.clone());
      }).ok_or_else(|| "partitioned variable store is full".to_string())?;
    }
    Ok(())
  }

  fn forth_matrix_load(store: &VarsStore, name: &str) -> Result<ForthMatrix, String> {
    let key = forth_matrix_key(name)?;
    let value = vars_entry_id(store, &key)
      .and_then(|id| store.entries.get(id))
      .and_then(|row| row.get("value"))
      .cloned()
      .ok_or_else(|| format!("matrix '{}' was not found", name))?;
    let matrix = serde_json::from_value::<ForthMatrix>(value)
      .map_err(|_| format!("matrix '{}' is invalid", name))?;
    forth_matrix_validate(&matrix)?;
    Ok(matrix)
  }

  fn forth_matrix_save(store: &mut VarsStore, name: &str, matrix: ForthMatrix) -> Result<(), String> {
    forth_matrix_validate(&matrix)?;
    let key = forth_matrix_key(name)?;
    forth_store_json(store, &key, serde_json::to_value(matrix).map_err(|error| error.to_string())?)
  }

  fn forth_matrix_load_scoped(store: &VarsStore, session: &str, name: &str) -> Result<ForthMatrix, String> {
    let key = session_storage_key(session, &forth_matrix_key(name)?);
    let value = vars_entry_id(store, &key)
      .and_then(|id| store.entries.get(id))
      .and_then(|row| row.get("value"))
      .cloned()
      .ok_or_else(|| format!("matrix '{}' was not found", name))?;
    let matrix = serde_json::from_value::<ForthMatrix>(value)
      .map_err(|_| format!("matrix '{}' is invalid", name))?;
    forth_matrix_validate(&matrix)?;
    Ok(matrix)
  }

  fn forth_matrix_save_scoped(store: &mut VarsStore, session: &str, name: &str, matrix: ForthMatrix) -> Result<(), String> {
    forth_matrix_validate(&matrix)?;
    let key = session_storage_key(session, &forth_matrix_key(name)?);
    forth_store_json(store, &key, serde_json::to_value(matrix).map_err(|error| error.to_string())?)
  }

  fn forth_matrix_new(rows: usize, cols: usize) -> Result<ForthMatrix, String> {
    let cells = rows.checked_mul(cols).ok_or_else(|| "matrix dimensions overflow".to_string())?;
    let matrix = ForthMatrix { rows, cols, values: vec![0; cells] };
    forth_matrix_validate(&matrix)?;
    Ok(matrix)
  }

  fn forth_matrix_index(matrix: &ForthMatrix, row: usize, col: usize) -> Result<usize, String> {
    if row >= matrix.rows || col >= matrix.cols {
      return Err(format!("matrix index ({}, {}) is outside {}x{}", row, col, matrix.rows, matrix.cols));
    }
    Ok(row * matrix.cols + col)
  }

  fn forth_matrix_add(left: &ForthMatrix, right: &ForthMatrix, subtract: bool) -> Result<ForthMatrix, String> {
    if left.rows != right.rows || left.cols != right.cols {
      return Err("matrix dimensions must match".to_string());
    }
    let values = left.values.iter().zip(&right.values).map(|(left, right)| {
      if subtract { left.checked_sub(*right) } else { left.checked_add(*right) }
        .ok_or_else(|| "matrix arithmetic overflow".to_string())
    }).collect::<Result<Vec<_>, _>>()?;
    Ok(ForthMatrix { rows: left.rows, cols: left.cols, values })
  }

  fn forth_matrix_multiply(left: &ForthMatrix, right: &ForthMatrix) -> Result<ForthMatrix, String> {
    if left.cols != right.rows {
      return Err(format!("cannot multiply {}x{} by {}x{}", left.rows, left.cols, right.rows, right.cols));
    }
    let mut result = forth_matrix_new(left.rows, right.cols)?;
    for row in 0..left.rows {
      for col in 0..right.cols {
        let mut total = 0i64;
        for index in 0..left.cols {
          let product = left.values[row * left.cols + index].checked_mul(right.values[index * right.cols + col])
            .ok_or_else(|| "matrix arithmetic overflow".to_string())?;
          total = total.checked_add(product).ok_or_else(|| "matrix arithmetic overflow".to_string())?;
        }
        result.values[row * result.cols + col] = total;
      }
    }
    Ok(result)
  }

  fn forth_matrix_scale(matrix: &ForthMatrix, scalar: i64) -> Result<ForthMatrix, String> {
    let values = matrix.values.iter().map(|value| {
      value.checked_mul(scalar).ok_or_else(|| "matrix arithmetic overflow".to_string())
    }).collect::<Result<Vec<_>, _>>()?;
    Ok(ForthMatrix { rows: matrix.rows, cols: matrix.cols, values })
  }

  fn forth_matrix_transpose(matrix: &ForthMatrix) -> Result<ForthMatrix, String> {
    let mut result = forth_matrix_new(matrix.cols, matrix.rows)?;
    for row in 0..matrix.rows {
      for col in 0..matrix.cols {
        result.values[col * result.cols + row] = matrix.values[row * matrix.cols + col];
      }
    }
    Ok(result)
  }

  fn forth_matrix_determinant(matrix: &ForthMatrix) -> Result<i64, String> {
    if matrix.rows != matrix.cols {
      return Err("determinant requires a square matrix".to_string());
    }
    if matrix.rows > 16 {
      return Err("determinant is limited to 16x16 matrices".to_string());
    }
    let size = matrix.rows;
    if size == 0 { return Ok(1); }
    if size == 1 { return Ok(matrix.values[0]); }
    let mut data = matrix.values.iter().map(|value| i128::from(*value)).collect::<Vec<_>>();
    let mut sign = 1i128;
    let mut previous_pivot = 1i128;
    for pivot_index in 0..size - 1 {
      let pivot_row = (pivot_index..size).find(|row| data[row * size + pivot_index] != 0);
      let Some(pivot_row) = pivot_row else { return Ok(0); };
      if pivot_row != pivot_index {
        for col in 0..size {
          data.swap(pivot_index * size + col, pivot_row * size + col);
        }
        sign = -sign;
      }
      let pivot = data[pivot_index * size + pivot_index];
      for row in pivot_index + 1..size {
        for col in pivot_index + 1..size {
          let left = data[row * size + col].checked_mul(pivot).ok_or_else(|| "determinant overflow".to_string())?;
          let right = data[row * size + pivot_index].checked_mul(data[pivot_index * size + col])
            .ok_or_else(|| "determinant overflow".to_string())?;
          let numerator = left.checked_sub(right).ok_or_else(|| "determinant overflow".to_string())?;
          if numerator % previous_pivot != 0 {
            return Err("non-exact determinant division".to_string());
          }
          data[row * size + col] = numerator / previous_pivot;
        }
        data[row * size + pivot_index] = 0;
      }
      previous_pivot = pivot;
    }
    i64::try_from(sign.checked_mul(data[size * size - 1]).ok_or_else(|| "determinant overflow".to_string())?)
      .map_err(|_| "determinant exceeds 64-bit integer range".to_string())
  }

  fn forth_matrix_solve(left: &ForthMatrix, right: &ForthMatrix) -> Result<ForthMatrix, String> {
    if left.rows != left.cols || right.rows != left.rows {
      return Err("m.solve requires a square left matrix and a compatible right matrix".to_string());
    }
    let determinant = forth_matrix_determinant(left)?;
    if determinant == 0 {
      return Err("m.solve requires an invertible left matrix".to_string());
    }
    let mut solution = forth_matrix_new(left.cols, right.cols)?;
    for right_column in 0..right.cols {
      for solution_row in 0..left.cols {
        let mut replaced = left.clone();
        for row in 0..left.rows {
          replaced.values[row * replaced.cols + solution_row] = right.values[row * right.cols + right_column];
        }
        let numerator = forth_matrix_determinant(&replaced)?;
        if numerator % determinant != 0 {
          return Err("m.solve only returns exact integer solutions".to_string());
        }
        solution.values[solution_row * solution.cols + right_column] = numerator / determinant;
      }
    }
    Ok(solution)
  }

  fn algebra_variable(value: Option<String>) -> Result<String, String> {
    let variable = value.unwrap_or_else(|| "x".to_string());
    if variable.is_empty() || variable.len() > 16 || !variable.chars().enumerate().all(|(index, character)| {
      if index == 0 { character.is_ascii_alphabetic() } else { character.is_ascii_alphanumeric() || character == '_' }
    }) {
      return Err("variable must be an identifier up to 16 characters".to_string());
    }
    Ok(variable)
  }

  fn algebra_parse_term(term: &str, variable: &str) -> Result<(u32, i64), String> {
    if term.is_empty() {
      return Err("empty algebra term".to_string());
    }
    let Some(variable_index) = term.find(variable) else {
      return term.parse::<i64>()
        .map(|coefficient| (0, coefficient))
        .map_err(|_| format!("invalid constant '{}'", term));
    };
    if term[variable_index + variable.len()..].contains(variable) {
      return Err(format!("term '{}' contains the variable more than once", term));
    }
    let coefficient_text = term[..variable_index].strip_suffix('*').unwrap_or(&term[..variable_index]);
    let coefficient = if coefficient_text.is_empty() || coefficient_text == "+" {
      1
    } else if coefficient_text == "-" {
      -1
    } else {
      coefficient_text.parse::<i64>().map_err(|_| format!("invalid coefficient in '{}'", term))?
    };
    let suffix = &term[variable_index + variable.len()..];
    let exponent = if suffix.is_empty() {
      1
    } else if let Some(exponent) = suffix.strip_prefix('^') {
      exponent.parse::<u32>().map_err(|_| format!("invalid exponent in '{}'", term))?
    } else {
      return Err(format!("invalid variable term '{}'", term));
    };
    Ok((exponent, coefficient))
  }

  fn algebra_parse(expression: &str, variable: &str) -> Result<BTreeMap<u32, i64>, String> {
    if expression.len() > 4_096 {
      return Err("expression may not exceed 4096 characters".to_string());
    }
    let expression = expression.chars().filter(|character| !character.is_whitespace()).collect::<String>();
    if expression.is_empty() {
      return Err("expression is empty".to_string());
    }
    let mut terms = Vec::new();
    let mut start = 0usize;
    for (index, character) in expression.char_indices().skip(1) {
      if character == '+' || character == '-' {
        terms.push(&expression[start..index]);
        start = index;
      }
    }
    terms.push(&expression[start..]);

    let mut polynomial = BTreeMap::new();
    for term in terms {
      let (exponent, coefficient) = algebra_parse_term(term, variable)?;
      let combined = polynomial.get(&exponent).copied().unwrap_or(0i64).checked_add(coefficient)
        .ok_or_else(|| "coefficient overflow".to_string())?;
      if combined == 0 {
        polynomial.remove(&exponent);
      } else {
        polynomial.insert(exponent, combined);
      }
    }
    Ok(polynomial)
  }

  fn algebra_term(exponent: u32, coefficient: i64, variable: &str) -> String {
    if exponent == 0 {
      return coefficient.to_string();
    }
    let symbol = if exponent == 1 { variable.to_string() } else { format!("{}^{}", variable, exponent) };
    match coefficient {
      1 => symbol,
      -1 => format!("-{}", symbol),
      _ => format!("{}*{}", coefficient, symbol),
    }
  }

  fn algebra_format(polynomial: &BTreeMap<u32, i64>, variable: &str) -> String {
    let terms = polynomial.iter().rev().filter_map(|(exponent, coefficient)| {
      (*coefficient != 0).then(|| algebra_term(*exponent, *coefficient, variable))
    }).collect::<Vec<_>>();
    if terms.is_empty() { "0".to_string() } else {
      terms.into_iter().enumerate().map(|(index, term)| {
        if index == 0 || term.starts_with('-') { term } else { format!("+ {}", term) }
      }).collect::<Vec<_>>().join(" ")
    }
  }

  fn algebra_derivative(polynomial: &BTreeMap<u32, i64>) -> Result<BTreeMap<u32, i64>, String> {
    let mut derivative = BTreeMap::new();
    for (exponent, coefficient) in polynomial {
      if *exponent > 0 {
        let derivative_coefficient = coefficient.checked_mul(i64::from(*exponent))
          .ok_or_else(|| "coefficient overflow".to_string())?;
        derivative.insert(exponent - 1, derivative_coefficient);
      }
    }
    Ok(derivative)
  }

  fn algebra_integral(polynomial: &BTreeMap<u32, i64>, variable: &str) -> Result<String, String> {
    let mut terms = Vec::new();
    for (exponent, coefficient) in polynomial.iter().rev() {
      let next_exponent = exponent.checked_add(1).ok_or_else(|| "exponent overflow".to_string())?;
      let divisor = i64::from(next_exponent);
      let symbol = if next_exponent == 1 { variable.to_string() } else { format!("{}^{}", variable, next_exponent) };
      let term = if coefficient % divisor == 0 {
        algebra_term(next_exponent, coefficient / divisor, variable)
      } else if *coefficient == 1 {
        format!("{}/{}", symbol, divisor)
      } else if *coefficient == -1 {
        format!("-{}/{}", symbol, divisor)
      } else {
        format!("{}*{}/{}", coefficient, symbol, divisor)
      };
      terms.push(term);
    }
    if terms.is_empty() { return Ok("C".to_string()); }
    Ok(format!("{} + C", terms.into_iter().enumerate().map(|(index, term)| {
      if index == 0 || term.starts_with('-') { term } else { format!("+ {}", term) }
    }).collect::<Vec<_>>().join(" ")))
  }

  fn algebra_evaluate(polynomial: &BTreeMap<u32, i64>, at: i64) -> Result<i64, String> {
    polynomial.iter().try_fold(0i64, |total, (exponent, coefficient)| {
      let power = at.checked_pow(*exponent).ok_or_else(|| "evaluation overflow".to_string())?;
      let term = coefficient.checked_mul(power).ok_or_else(|| "evaluation overflow".to_string())?;
      total.checked_add(term).ok_or_else(|| "evaluation overflow".to_string())
    })
  }

  fn chatlog_store_snapshot(store: &Mutex<ChatlogStore>) -> (String, String) {
    let Ok(store) = store.lock() else {
      return (String::new(), String::new());
    };
    let raw = store
      .entries
      .non_empty_ids()
      .into_iter()
      .filter_map(|id| store.entries.get(id))
      .filter_map(|row| row.get("body").and_then(serde_json::Value::as_str))
      .collect::<Vec<_>>()
      .join("\n");
    (raw, store.revision.to_string())
  }

  fn chatlog_cache_get(
    cache: &Mutex<partitioned_array_rust::PartitionedArray>,
    source_len: u64,
    source_modified: &str,
    payload_key: &str,
  ) -> Option<String> {
    let cache = cache.lock().ok()?;
    let row = cache.get(0)?;
    let is_current = row
      .get("source_len")
      .and_then(serde_json::Value::as_u64)
      == Some(source_len)
      && row
        .get("source_modified")
        .and_then(serde_json::Value::as_str)
        == Some(source_modified);
    if !is_current {
      return None;
    }
    row.get(payload_key)
      .and_then(serde_json::Value::as_str)
      .map(str::to_owned)
  }

  fn chatlog_cache_store(
    cache: &Mutex<partitioned_array_rust::PartitionedArray>,
    source_len: u64,
    source_modified: &str,
    payload_key: &str,
    payload: String,
  ) {
    let Ok(mut cache) = cache.lock() else {
      return;
    };
    let preserves_current_payloads = cache.get(0).is_some_and(|row| {
      row.get("source_len")
        .and_then(serde_json::Value::as_u64)
        == Some(source_len)
        && row
          .get("source_modified")
          .and_then(serde_json::Value::as_str)
          == Some(source_modified)
    });
    let mut row = if preserves_current_payloads {
      cache.get(0).cloned().unwrap_or_default()
    } else {
      serde_json::Map::new()
    };
    row.insert("source_len".to_string(), serde_json::Value::from(source_len));
    row.insert(
      "source_modified".to_string(),
      serde_json::Value::from(source_modified.to_string()),
    );
    row.insert(payload_key.to_string(), serde_json::Value::from(payload));
    let _ = cache.set_with(0, |slot| *slot = row.clone());
  }

    // Main HTTPS server - handling all defined routes
let (chatlog_store, vars_store, forth_bridge_queue, custom_words, avatar_frequency) = restore_memory_stores()
  .unwrap_or_else(|| (new_chatlog_store(), new_vars_store(), new_forth_bridge_queue(), new_custom_words_store(), new_avatar_frequency_store()));
let state = AppState {
    queue: Mutex::new(Vec::new()),
    results: Mutex::new(Vec::new()),
  chatlog_cache: Arc::new(Mutex::new(new_chatlog_memory_cache())),
  chatlog_store: Arc::new(Mutex::new(chatlog_store)),
  vars_store: Arc::new(Mutex::new(vars_store)),
  forth_bridge_queue: Arc::new(Mutex::new(forth_bridge_queue)),
  custom_words: Arc::new(Mutex::new(custom_words)),
  avatar_frequency: Arc::new(Mutex::new(avatar_frequency)),
};
let mut app = tide::with_state(state.clone());
    let state_for_shutdown = state.clone();
    ctrlc::set_handler(move || {
        if let Err(error) = persist_memory_stores(&state_for_shutdown) {
            eprintln!("Failed to persist partitioned memory stores: {}", error);
        }
        std::process::exit(0);
    }).map_err(|error| tide::Error::from_str(
        StatusCode::InternalServerError,
        format!("failed to install shutdown handler: {}", error),
    ))?;
    // Spawn a background thread to listen for CLI input.
    let state_for_cli = state.clone();
    std::thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            if let Ok(input) = line {
                match input.trim() {
                    "exit" => {
                        if let Err(error) = persist_memory_stores(&state_for_cli) {
                        eprintln!("Failed to persist partitioned memory stores: {}", error);
                      }
                        std::process::exit(0);
                    }

                    // `rustby` evaluates a demo snippet; `rustby <code>` evaluates <code>
                    // in the embedded Ruby VM and prints the result's `to_s`.
                    command if command == "rustby" || command.starts_with("rustby ") => {
                        let code = command.strip_prefix("rustby").unwrap_or_default().trim();
                        let code = if code.is_empty() { "'RustbySpace'" } else { code };
                        match ruby_vm::eval_blocking(code) {
                            Ok(output) => println!("Ruby output: {}", output),
                            Err(e) => eprintln!("Error running Ruby code: {}", e),
                        }
                    }

                    "restart" => {
                        println!("Restarting all servers...");
                        std::process::Command::new("sh")
                            .arg("-c")
                            .arg("killall -HUP tiade-maeepers-saerver-all") // Replace with your server binary name
                            .spawn()
                            .expect("Failed to restart servers");
                    }
                    _ => {
                        println!("Unknown command: {}", input.trim());
                    }
                }
            }
        }
    });

    // ... rest of the main function (server setup, routes, etc.)
    //  Ok(())

    /*
       ///
        // Example: Spawn 3 independent Ruby interpreter threads.
        let mut handles: Vec<JoinHandle<Result<(), Error>>> = Vec::new();


        // Optionally, wait for the threads to complete.
        for handle in handles {
            match handle.join() {
                Ok(Ok(())) => println!("Ruby instance finished successfully."),
                Ok(Err(err)) => eprintln!("Ruby eval error: {}", err),
                Err(_) => eprintln!("A thread panicked."),
            }
        }
    */
    // Continue with the rest of your server setup…
    //Ok(())
    //

    use std::cell::RefCell;

// --------------------------------------------------------
// Data types
// --------------------------------------------------------

#[derive(Clone, Serialize, Deserialize)]
struct QueuedCommand {
    id: u64,
    command: String,
    stack: Vec<String>,
    memory: Vec<(String, String)>,
}

#[derive(Clone, Serialize, Deserialize)]
struct CompletedResult {
    id: u64,
    stack: Vec<String>,
    memory: Vec<(String, String)>,
    output: String,
    error: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct ForthBridgeMessage {
  id: u64,
  #[serde(default)]
  session_id: String,
  source: String,
  max_steps: Option<usize>,
  #[serde(default)]
  language: String,
  queued_at: String,
}

// --------------------------------------------------------
// AppState (Tide server state)
// --------------------------------------------------------


struct AppState {
    queue: Mutex<Vec<QueuedCommand>>,
    results: Mutex<Vec<CompletedResult>>,
  chatlog_cache: Arc<Mutex<partitioned_array_rust::PartitionedArray>>,
    chatlog_store: Arc<Mutex<ChatlogStore>>,
    vars_store: Arc<Mutex<VarsStore>>,
    forth_bridge_queue: Arc<Mutex<partitioned_array_rust::PartitionedArray>>,
    // Admin-managed words from /chatlog/words, merged into the built-in scoring dictionaries.
    custom_words: Arc<Mutex<partitioned_array_rust::PartitionedArray>>,
    avatar_frequency: Arc<Mutex<partitioned_array_rust::PartitionedArray>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
            results: Mutex::new(Vec::new()),
          chatlog_cache: Arc::new(Mutex::new(new_chatlog_memory_cache())),
          chatlog_store: Arc::new(Mutex::new(new_chatlog_store())),
          vars_store: Arc::new(Mutex::new(new_vars_store())),
          forth_bridge_queue: Arc::new(Mutex::new(new_forth_bridge_queue())),
          custom_words: Arc::new(Mutex::new(new_custom_words_store())),
          avatar_frequency: Arc::new(Mutex::new(new_avatar_frequency_store())),
        }
    }
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        let queue = self.queue.lock().unwrap().clone();
        let results = self.results.lock().unwrap().clone();
        Self {
            queue: Mutex::new(queue),
            results: Mutex::new(results),
          chatlog_cache: Arc::clone(&self.chatlog_cache),
          chatlog_store: Arc::clone(&self.chatlog_store),
          vars_store: Arc::clone(&self.vars_store),
          forth_bridge_queue: Arc::clone(&self.forth_bridge_queue),
          custom_words: Arc::clone(&self.custom_words),
          avatar_frequency: Arc::clone(&self.avatar_frequency),
        }
    }
}

 

    // Custom middleware to log which route is being handled
    struct LogRoute;
    #[tide::utils::async_trait]
    impl tide::Middleware<AppState> for LogRoute {
        async fn handle(
            &self,
            req: tide::Request<AppState>,
            next: tide::Next<'_, AppState>,
        ) -> tide::Result {
            let route = req.url().path().to_string();
            let res = next.run(req).await;
            println!("Route '{}' handled with status: {}", route, res.status());
            Ok(res)
        }
    }

    app.with(LogRoute);
    mount_ollama_routes(&mut app, OllamaRelayConfig::default())?;

   

    use std::sync::Arc;

    use std::collections::HashMap;
    use tide::{Request, Response, StatusCode};

    use std::fs::OpenOptions;
    use url::Url;
    //let rustby_eval_title = rustby_eval_title.clone();

    // Serve each directory. Tide will serve new files as they appear.
    // app.at("/css").serve_dir("./css/")?;
    // app.at("/js").serve_dir("./js/")?;
    // app.at("/img").serve_dir("./img/")?;
    // app.at("/fonts").serve_dir("./fonts/")?;
    // app.at("/public").serve_dir("./public/")?;

    #[derive(serde::Deserialize)]
    struct PraexyForm {
        content: String,
    }

    
    


    app.at("/praexy-saerver")
        .post(|mut req: tide::Request<AppState>| async move {
            let form_data: PraexyForm = req.body_form().await.unwrap_or(PraexyForm {
                content: String::new(),
            });
            Ok(format!("Received content:\n{}", form_data.content))
        });


  app.at("/analytics").get(|req: tide::Request<AppState>| async move {
    use chrono::{Datelike, FixedOffset, Timelike, Utc, NaiveDateTime};
    use serde_json::Value;
    use std::collections::{BTreeMap, HashMap, HashSet};

    const WEEKDAYS: [&str; 7] = [
        "Monday", "Tuesday", "Wednesday", "Thursday",
        "Friday", "Saturday", "Sunday",
    ];

    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April",
        "May", "June", "July", "August",
        "September", "October", "November", "December",
    ];

    // ---------------------------------------------------------------------
    // YAML → JSON parser for LSL logs
    // ---------------------------------------------------------------------
    fn parse_lsl_yaml(line: &str) -> Option<Value> {
        if let Ok(val_yaml) = serde_yaml::from_str::<serde_yaml::Value>(line) {
            let val_json = serde_json::to_value(val_yaml).ok()?;
            if val_json.is_object() || val_json.is_array() {
                return Some(val_json);
            }
        }
        None
    }

    fn parse_json(line: &str) -> Option<Value> {
        serde_json::from_str::<Value>(line).ok()
    }

    // ---------------------------------------------------------------------
    // Read the shared in-memory partitioned log store.
    // ---------------------------------------------------------------------
    let (raw, _) = chatlog_store_snapshot(req.state().chatlog_store.as_ref());
    let mut entries: Vec<Value> = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // 1. YAML (LSL logs parse as YAML)
        if let Some(val) = parse_lsl_yaml(line) {
            match val {
                Value::Array(arr) => {
                    for item in arr {
                        if item.is_object() {
                            entries.push(item);
                        }
                    }
                }
                Value::Object(_) => entries.push(val),
                _ => {}
            }
            continue;
        }

        // 2. JSON fallback
        if let Some(val) = parse_json(line) {
            match val {
                Value::Array(arr) => {
                    for item in arr {
                        if item.is_object() {
                            entries.push(item);
                        }
                    }
                }
                Value::Object(_) => entries.push(val),
                _ => {}
            }
            continue;
        }
    }

    // ---------------------------------------------------------------------
    // Deduplicate events
    // ---------------------------------------------------------------------
    fn ts(v: &Value) -> Option<i64> {
        if let Some(i) = v.as_i64() {
            return Some(i);
        }
        if let Some(f) = v.as_f64() {
            return Some(f as i64);
        }
        if let Some(s) = v.as_str() {
            return s.parse::<i64>().ok();
        }
        None
    }

    let mut unique: HashMap<(String, i64, String), Value> = HashMap::new();

    for e in &entries {
        let timestamp = match ts(&e["timestamp"]) {
            Some(t) => t,
            None => continue,
        };

        let key = (
            e["avatar_id"].as_str().unwrap_or("").to_string(),
            timestamp,
            e["message"].as_str().unwrap_or("").to_string(),
        );

        unique.entry(key).or_insert_with(|| e.clone());
    }

    let events: Vec<Value> = unique.into_values().collect();

    let markov_messages = chatlog_markov_messages(&raw);
    let markov_counts = chatlog_markov_transition_counts(&markov_messages);
    let mut markov_transitions = markov_counts.into_iter().collect::<Vec<_>>();
    markov_transitions.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let markov_transition_total = markov_transitions.iter().map(|(_, count)| *count).sum::<usize>();
    let markov_readiness = if markov_messages.len() >= 20 && markov_transitions.len() >= 50 {
      "Ready"
    } else if markov_messages.len() >= 5 {
      "Limited corpus"
    } else {
      "Needs more messages"
    };

    // ---------------------------------------------------------------------
    // Frequency counters
    // ---------------------------------------------------------------------
    let mut weekday_freq = [0usize; 7];
    let mut hour_freq = [0usize; 24];
    let mut month_freq = [0usize; 12];
    let mut year_freq: BTreeMap<i32, usize> = BTreeMap::new();
    let mut month_year_freq: BTreeMap<String, usize> = BTreeMap::new();
    let mut day_of_month_freq = [0usize; 32];

    let mut avatars: HashSet<String> = HashSet::new();
    let mut messages: HashSet<String> = HashSet::new();

    let mut earliest: Option<chrono::DateTime<FixedOffset>> = None;
    let mut latest: Option<chrono::DateTime<FixedOffset>> = None;

    let pst = FixedOffset::west_opt(7 * 3600).unwrap();

    for e in &events {
        let avatar = e["avatar_id"].as_str().unwrap_or("").trim();
        if !avatar.is_empty() {
            avatars.insert(avatar.to_string());
        }

        let msg = e["message"].as_str().unwrap_or("").trim();
        if !msg.is_empty() {
            messages.insert(msg.to_string());
        }

        let ts = match ts(&e["timestamp"]) {
            Some(ts) if ts > 0 => ts,
            _ => continue,
        };

        let dt_utc = match NaiveDateTime::from_timestamp_opt(ts, 0) {
            Some(ndt) => chrono::DateTime::<Utc>::from_utc(ndt, Utc),
            None => continue,
        };

        let dt = dt_utc.with_timezone(&pst);

        if earliest.map(|v| dt < v).unwrap_or(true) {
            earliest = Some(dt);
        }
        if latest.map(|v| dt > v).unwrap_or(true) {
            latest = Some(dt);
        }

        weekday_freq[dt.weekday().num_days_from_monday() as usize] += 1;
        hour_freq[dt.hour() as usize] += 1;
        month_freq[dt.month0() as usize] += 1;
        *year_freq.entry(dt.year()).or_insert(0) += 1;
        *month_year_freq.entry(dt.format("%Y-%m").to_string()).or_insert(0) += 1;
        day_of_month_freq[dt.day() as usize] += 1;
    }

    // ---------------------------------------------------------------------
    // Output report
    // ---------------------------------------------------------------------
    let mut out = String::new();
    out.push_str("Second Life chat frequency report (PST)\n");
    out.push_str("Source: in-memory partitioned log store\n");
    out.push_str(&format!("Raw parsed entries: {}\n", entries.len()));
    out.push_str(&format!("Total unique events: {}\n", events.len()));
    out.push_str(&format!("Unique avatar IDs: {}\n", avatars.len()));
    out.push_str(&format!("Unique message bodies: {}\n", messages.len()));
    out.push_str(&format!("Markov training messages: {}\n", markov_messages.len()));
    out.push_str(&format!("Markov word transition states: {}\n", markov_transitions.len()));
    out.push_str(&format!("Markov word transitions: {}\n", markov_transition_total));
    out.push_str(&format!("Markov readiness: {}\n", markov_readiness));
    out.push_str(&format!(
      "Markov seed suggestions: {}\n",
      chatlog_markov_seed_suggestions(&markov_messages, 12).join(", ")
    ));

    out.push_str(&format!(
        "First event (PST): {}\n",
        earliest
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S %Z").to_string())
            .unwrap_or_else(|| "N/A".to_string())
    ));

    out.push_str(&format!(
        "Last event (PST):  {}\n",
        latest
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S %Z").to_string())
            .unwrap_or_else(|| "N/A".to_string())
    ));

    out.push_str("\n=== Message Frequency by Day of Week ===\n");
    for (i, day) in WEEKDAYS.iter().enumerate() {
        out.push_str(&format!("{:<9} : {}\n", day, weekday_freq[i]));
    }

    out.push_str("\n=== Message Frequency by Hour (PST) ===\n");
    for (h, count) in hour_freq.iter().enumerate() {
        out.push_str(&format!("{:02}:00–{:02}:59 : {}\n", h, h, count));
    }

    out.push_str("\n=== Message Frequency by Month ===\n");
    for (i, month) in MONTHS.iter().enumerate() {
        out.push_str(&format!("{:<9} : {}\n", month, month_freq[i]));
    }

    out.push_str("\n=== Message Frequency by Year ===\n");
    for (year, count) in &year_freq {
        out.push_str(&format!("{} : {}\n", year, count));
    }

    out.push_str("\n=== Message Frequency by Month-Year ===\n");
    for (ym, count) in &month_year_freq {
        out.push_str(&format!("{} : {}\n", ym, count));
    }

    out.push_str("\n=== Message Frequency by Day of Month ===\n");
    for day in 1..=31 {
        out.push_str(&format!("{:02} : {}\n", day, day_of_month_freq[day]));
    }

    out.push_str("\n=== Markov Chatter Model: Top Word Transitions ===\n");
    out.push_str("Admin routes: /chatlog/markov, /chatlog/markov.json, /chatlog/markov/transitions.json\n");
    for ((left, right, next), count) in markov_transitions.into_iter().take(25) {
      let probability = if markov_transition_total == 0 {
        0.0
      } else {
        count as f64 / markov_transition_total as f64
      };
      out.push_str(&format!(
        "({}, {}) -> {} : {} ({:.3})\n",
        left, right, next, count, probability
      ));
    }

    let mut res = tide::Response::new(tide::StatusCode::Ok);
    res.set_body(out);
    res.insert_header("Content-Type", "text/plain; charset=utf-8");
    Ok(res)
});


/*
        app.at("/sl_logger").post(|mut req: tide::Request<AppState>| async move {
    // Catch all POST variables into a hashmap and print them
    let body = req.body_string().await.unwrap_or_default();
    println!("Received POST body: {}", body);

    let script_dir = "/root/midscore_io/rustby/rustby-vm/target/release/scripts";
    let file_name: String = "second_life_chat_log.txt".to_string();

    let ruby_source = format!(r######"
    body = {}
    puts Dir.pwd
    FileUtils.touch("/root/midscore_io/tiade-maeepers-saerver-all/target/release/second_life_chat_logs.txt")
    puts "logging chat message to file"
     File.open('/root/midscore_io/tiade-maeepers-saerver-all/target/release/second_life_chat_logs.txt', 'a+') do |file|
       file.write("#{{body}}\n")
     end
    puts "Chat message logged to file successfully"

    "message logged to file successfully"
    "######, body);


    if ruby_source.trim().is_empty() {
        let mut resp = tide::Response::new(tide::StatusCode::Ok);
        resp.set_body("No Ruby code supplied");
        return Ok(resp);
    }

    // Create unique .rb filename.
    let ts = Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let filename = format!("{}/sl_log_{}.rb", script_dir,ts);
    std::fs::write(&filename, &ruby_source).map_err(|e| tide::Error::new(tide::StatusCode::InternalServerError, e))?;




    let result_path = format!("/root/midscore_io/rustby/rustby-vm/target/release/scripts/sl_log_{}.txt", ts);

    // Block until the result file is available or until timeout
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(120);
    while !std::path::Path::new(&result_path).exists() {
      if start.elapsed() > timeout {
        return Ok("Timed out waiting for result file".into());
      }
      std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let output = std::fs::read_to_string(&result_path).unwrap_or_else(|_| "No output".to_string());


    // Remove script file after evaluation.

    let _ = std::fs::remove_file(&result_path);
    let _ = std::fs::remove_file(&filename);

      let output = "Log entry received and written to file successfully.";

     // Return the HTML response.
    let mut res = tide::Response::new(tide::StatusCode::Ok);
    res.set_body(output);
    res.insert_header("Content-Type", "text/plain; charset=utf-8");
    Ok(res)
    //Ok(output.into())
  });
*/




use std::fs::{create_dir_all};
use std::io::Write;
use std::path::Path;


/*
// /vars/set
app.at("/vars/set").post(|mut req: Request<AppState>| async move {
    let body: Value = req.body_json().await?;

    let mut vars = VARS.lock().unwrap();
    if let Some(obj) = body.as_object() {
        for (k,v) in obj {
            vars.insert(k.clone(),v.clone());
        }
    }
    let snapshot = vars.clone();
    drop(vars);

    save_snapshot(snapshot, "SET");

    let mut res = Response::new(StatusCode::Ok);
    res.set_body("set complete");
    Ok(res)
});

// /vars/get/:name
app.at("/vars/get/:name").post(|req: Request<AppState>| async move {
    let name: String = req.param("name")?.to_string();

    let vars = VARS.lock().unwrap();
    let val = vars.get(&name).cloned();
    let snapshot = vars.clone();
    drop(vars);

    if let Some(val) = val {
        save_snapshot(snapshot, &format!("GET {}", name));
        let mut res = Response::new(StatusCode::Ok);
        res.set_body(serde_json::to_string(&val)?);
        Ok(res)
    } else {
        save_snapshot(snapshot, &format!("GET {} not found", name));
        let mut res = Response::new(StatusCode::NotFound);
        res.set_body("not found");
        Ok(res)
    }
});

// /vars/view
app.at("/vars/view").post(|_req: Request<AppState>| async move {
    let vars = VARS.lock().unwrap();
    let snapshot = vars.clone();
    drop(vars);

    save_snapshot(snapshot.clone(), "VIEW");
    let mut res = Response::new(StatusCode::Ok);
    res.set_body(serde_json::to_string(&snapshot)?);
    Ok(res)
});

// /vars/delete/:name
app.at("/vars/delete/:name").post(|req: Request<AppState>| async move {
    let name: String = req.param("name")?.to_string();

    let mut vars = VARS.lock().unwrap();
    vars.remove(&name);
    let snapshot = vars.clone();
    drop(vars);

    save_snapshot(snapshot, &format!("DELETE {}", name));
    let mut res = Response::new(StatusCode::Ok);
    res.set_body(format!("Deleted {}", name));
    Ok(res)
});

// /vars/clear
app.at("/vars/clear").post(|_req: Request<AppState>| async move {
    let mut vars = VARS.lock().unwrap();
    vars.clear();
    let snapshot = vars.clone();
    drop(vars);

    save_snapshot(snapshot, "CLEAR");
    let mut res = Response::new(StatusCode::Ok);
    res.set_body("All variables cleared");
    Ok(res)
});

// /vars/history
app.at("/vars/history").post(|_req: Request<AppState>| async move {
    let content = read_to_string("/root/midscore_io/tiade-maeepers-saerver-all/target/release/vars_flatfile.json").await.unwrap_or_default();
    let mut res = Response::new(StatusCode::Ok);
    res.set_body(content);
    Ok(res)
});

// /vars/status
app.at("/vars/status").post(|_req: Request<AppState>| async move {
    let vars = VARS.lock().unwrap();
    let count = vars.len();
    drop(vars);

    let status = json!({
        "vars_count": count,
        "server": true,
        "debug": true,
        "public": true
    });

    let mut res = Response::new(StatusCode::Ok);
    res.set_body(status.to_string());
    Ok(res)
});

*/


   app.at("/sl_logger").post(|mut req: Request<AppState>| async move {
        // Read POST body
        let body = req.body_string().await.unwrap_or_default();
        println!("Received POST body: {}", body);

      let mut store = req.state().chatlog_store.lock().map_err(|_| {
        tide::Error::from_str(StatusCode::InternalServerError, "chatlog store lock poisoned")
      })?;
      let row_id = store.entries.add(|row| {
        row.insert("body".to_string(), Value::String(body.clone()));
        row.insert("received_at".to_string(), Value::String(Utc::now().to_rfc3339()));
      }).ok_or_else(|| {
        tide::Error::from_str(StatusCode::InsufficientStorage, "chatlog store is full")
      })?;
      store.revision = store.revision.saturating_add(1);
      drop(store);

      persist_memory_stores(req.state()).map_err(|error| {
        tide::Error::from_str(
          StatusCode::InternalServerError,
          format!("chatlog entry was not persisted: {}", error),
        )
      })?;

        // Respond
        let mut res = Response::new(StatusCode::Ok);
      res.set_body(format!("Log entry stored in memory with id {}.", row_id));
        res.insert_header("Content-Type", "text/plain; charset=utf-8");
        Ok(res)
    });

app.at("/avatarfrequency").post(|mut req: Request<AppState>| async move {
    let body: Value = req.body_json().await
      .map_err(|error| tide::Error::from_str(StatusCode::BadRequest, format!("invalid avatar frequency JSON: {}", error)))?;
    if !body.is_object() || !body.get("avatars").is_some_and(Value::is_array) {
      return Err(tide::Error::from_str(StatusCode::BadRequest, "expected an object with an avatars array"));
    }
    let mut store = req.state().avatar_frequency.lock().map_err(|_| {
      tide::Error::from_str(StatusCode::InternalServerError, "avatar frequency store lock poisoned")
    })?;
    let row_id = store.add(|row| {
      row.insert("body".to_string(), body.clone());
      row.insert("received_at".to_string(), Value::String(Utc::now().to_rfc3339()));
    }).ok_or_else(|| tide::Error::from_str(StatusCode::InsufficientStorage, "avatar frequency store is full"))?;
    drop(store);
    persist_memory_stores(req.state()).map_err(|error| {
      tide::Error::from_str(StatusCode::InternalServerError, format!("avatar frequency was not persisted: {}", error))
    })?;
    let mut response = Response::new(StatusCode::Ok);
    response.set_body(json!({"stored": true, "id": row_id}));
    Ok(response)
});

app.at("/avatarencounter").get(|req: Request<AppState>| async move {
    use std::collections::HashMap;
    let store = req.state().avatar_frequency.lock().map_err(|_| {
      tide::Error::from_str(StatusCode::InternalServerError, "avatar frequency store lock poisoned")
    })?;
    let mut current: HashMap<String, Value> = HashMap::new();
    let mut previous: HashMap<String, (i64, i64)> = HashMap::new();
    let mut sources: HashMap<String, Value> = HashMap::new();
    let mut source_previous: HashMap<String, Value> = HashMap::new();
    let mut source_encounters: HashMap<String, i64> = HashMap::new();
    let mut source_encounters_previous: HashMap<String, i64> = HashMap::new();
    for id in store.non_empty_ids() {
      let Some(row) = store.get(id) else { continue };
      let Some(body) = row.get("body") else { continue };
      let source = body.get("captured_by").and_then(Value::as_str).unwrap_or("unknown").to_string();
      if let Some(old_snapshot) = sources.get(&source) {
        source_previous.insert(source.clone(), old_snapshot.clone());
      }
      sources.insert(source.clone(), body.clone());
      // Snapshot the running total before this row's counts are folded in, so it reflects the total prior to the latest scan.
      source_encounters_previous.insert(source.clone(), source_encounters.get(&source).copied().unwrap_or(0));
      let scan_at = body.get("scan_at").and_then(Value::as_i64).unwrap_or(0);
      let Some(avatars) = body.get("avatars").and_then(Value::as_array) else { continue };
      for avatar in avatars {
        let Some(uuid) = avatar.get("uuid").and_then(Value::as_str) else { continue };
        let key = format!("{}:{}", source, uuid);
        let count = avatar.get("count").and_then(Value::as_i64).unwrap_or(0);
        // Newer scanner payloads don't send a running "total_encounters" total, so accumulate it here instead.
        *source_encounters.entry(source.clone()).or_insert(0) += count;
        if let Some(current_avatar) = current.get(&key) {
          let old_count = current_avatar.get("count").and_then(Value::as_i64).unwrap_or(0);
          let old_at = current_avatar.get("scan_at").and_then(Value::as_i64).unwrap_or(scan_at);
          previous.insert(key.clone(), (old_count, old_at));
        }
        let mut normalized = avatar.clone();
        normalized["source"] = Value::String(source.clone());
        normalized["scan_at"] = Value::from(scan_at);
        current.insert(key, normalized);
      }
    }
    drop(store);
    let total_count: i64 = current.values().map(|avatar| avatar.get("count").and_then(Value::as_i64).unwrap_or(0)).sum();
    let mut avatars = Vec::new();
    for (key, mut avatar) in current {
      let count = avatar.get("count").and_then(Value::as_i64).unwrap_or(0);
      let scan_at = avatar.get("scan_at").and_then(Value::as_i64).unwrap_or(0);
      let (previous_count, previous_at) = previous.get(&key).copied().unwrap_or((0, scan_at));
      let elapsed = (scan_at - previous_at).max(0) as f64;
      let delta = count - previous_count;
      avatar["probability"] = if total_count > 0 { Value::from(count as f64 / total_count as f64) } else { Value::from(0.0) };
      avatar["delta_count"] = Value::from(delta);
      avatar["encounter_rate_per_second"] = if elapsed > 0.0 { Value::from(delta as f64 / elapsed) } else { Value::from(0.0) };
      avatars.push(avatar);
    }
    avatars.sort_by(|left, right| right.get("count").and_then(Value::as_i64).unwrap_or(0).cmp(&left.get("count").and_then(Value::as_i64).unwrap_or(0)));
    let mut source_stats = Vec::new();
    for (source, snapshot) in sources {
      let encounters = source_encounters.get(&source).copied().unwrap_or(0);
      let scans = snapshot.get("scan_count").and_then(Value::as_i64).unwrap_or(0);
      let interval = snapshot.get("scan_interval_seconds").and_then(Value::as_f64).unwrap_or(30.0);
      let duration = (scans as f64 * interval).max(1.0);
      let previous_encounters = source_encounters_previous.get(&source).copied().unwrap_or(encounters);
      let previous_scans = source_previous.get(&source)
        .and_then(|old| old.get("scan_count"))
        .and_then(Value::as_i64)
        .unwrap_or(scans);
      let previous_rate = if previous_scans > 0 {
        previous_encounters as f64 / (previous_scans as f64 * interval).max(1.0)
      } else {
        0.0
      };
      source_stats.push(json!({
        "source": source,
        "scan_count": scans,
        "total_encounters": encounters,
        "encounter_rate_per_second": encounters as f64 / duration,
        "encounter_rate_change_per_second": encounters as f64 / duration - previous_rate,
        "delta_encounters": encounters - previous_encounters,
        "scan_interval_seconds": interval,
        "last_scan_at": snapshot.get("scan_at").cloned().unwrap_or(Value::Null)
      }));
    }
    let payload = json!({
      "source": "avatar frequency store",
      "avatar_count": avatars.len(),
      "total_current_encounters": total_count,
      "avatars": avatars,
      "sources": source_stats
    });
    let wants_json = req.url().query_pairs().any(|(key, value)| {
      (key == "format" && value == "json") || key == "json"
    });
    if !wants_json {
      let mut response = Response::new(StatusCode::Ok);
      response.set_body(r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Avatar Frequency</title><style>
:root{color-scheme:dark;--ink:#eef4f3;--muted:#9eb2b0;--line:#294342;--panel:#11211f;--accent:#65e6c5;--warn:#ffd166}
*{box-sizing:border-box}body{margin:0;background:#071211;color:var(--ink);font:15px/1.45 ui-sans-serif,system-ui,sans-serif}
main{max-width:1100px;margin:auto;padding:28px 18px 56px}header{display:flex;justify-content:space-between;align-items:end;gap:18px;border-bottom:1px solid var(--line);padding-bottom:18px}h1{margin:0;font-size:clamp(28px,5vw,48px);letter-spacing:-.02em}h2{font-size:16px;margin:0 0 12px;color:var(--accent)}p{color:var(--muted);margin:5px 0}.live{color:var(--accent);white-space:nowrap}.grid{display:grid;grid-template-columns:repeat(3,1fr);gap:12px;margin:22px 0}.panel{background:var(--panel);border:1px solid var(--line);border-radius:8px;padding:16px}.metric{font-size:30px;color:var(--accent);font-weight:700}.table-wrap{overflow:auto}table{border-collapse:collapse;width:100%;min-width:650px}th,td{text-align:left;padding:11px 10px;border-bottom:1px solid var(--line)}th{color:var(--muted);font-size:12px;text-transform:uppercase;letter-spacing:.06em}td small{display:block;color:var(--muted);word-break:break-all}.empty{color:var(--muted);padding:22px 0}@media(max-width:650px){header{display:block}.live{display:block;margin-top:8px}.grid{grid-template-columns:1fr}}
</style></head><body><main><header><div><h1>Avatar Frequency</h1><p>Nearby avatar encounter analytics</p></div><div class="live" id="status">Connecting...</div></header>
<div class="grid"><section class="panel"><h2>Avatars</h2><div class="metric" id="avatar-count">0</div></section><section class="panel"><h2>Encounters</h2><div class="metric" id="encounter-count">0</div></section><section class="panel"><h2>Sources</h2><div class="metric" id="source-count">0</div></section></div>
<section class="panel"><h2>Frequency table</h2><div class="table-wrap"><table><thead><tr><th>Name / UUID</th><th>Count</th><th>Probability</th><th>Rate/min</th><th>Change</th></tr></thead><tbody id="avatars"><tr><td colspan="5" class="empty">Waiting for scan data...</td></tr></tbody></table></div></section>
<section class="panel" style="margin-top:12px"><h2>Scanner sources</h2><div class="table-wrap"><table><thead><tr><th>Source</th><th>Scans</th><th>Encounters</th><th>Rate/min</th><th>Rate change</th></tr></thead><tbody id="sources"><tr><td colspan="5" class="empty">Waiting for scan data...</td></tr></tbody></table></div></section>
</main><script>
const esc=value=>String(value??'').replace(/[&<>\"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','\"':'&quot;',"'":'&#39;'}[c]));
async function refresh(){try{const r=await fetch('/avatarencounter?format=json',{cache:'no-store'});if(!r.ok)throw Error();const d=await r.json();document.querySelector('#avatar-count').textContent=d.avatar_count||0;document.querySelector('#encounter-count').textContent=d.total_current_encounters||0;document.querySelector('#source-count').textContent=(d.sources||[]).length;document.querySelector('#status').textContent='Updated '+new Date().toLocaleTimeString();document.querySelector('#avatars').innerHTML=(d.avatars||[]).map(a=>`<tr><td><strong>${esc(a.name||'Unknown')}</strong><small>${esc(a.uuid||'')}</small></td><td>${a.count||0}</td><td>${(Number(a.probability||0)*100).toFixed(2)}%</td><td>${(Number(a.encounter_rate_per_second||0)*60).toFixed(3)}</td><td>${a.delta_count||0}</td></tr>`).join('')||'<tr><td colspan="5" class="empty">No avatars recorded yet.</td></tr>';document.querySelector('#sources').innerHTML=(d.sources||[]).map(s=>`<tr><td>${esc(s.source)}</td><td>${s.scan_count||0}</td><td>${s.total_encounters||0}</td><td>${(Number(s.encounter_rate_per_second||0)*60).toFixed(3)}</td><td>${(Number(s.encounter_rate_change_per_second||0)*60).toFixed(3)}</td></tr>`).join('')||'<tr><td colspan="5" class="empty">No scanner sources yet.</td></tr>';}catch(e){document.querySelector('#status').textContent='Unavailable';}}refresh();setInterval(refresh,3000);
</script></body></html>"#);
      response.insert_header("Content-Type", "text/html; charset=utf-8");
      return Ok(response);
    }
    let mut response = Response::new(StatusCode::Ok);
    response.set_body(payload);
    response.insert_header("Content-Type", "application/json; charset=utf-8");
    Ok(response)
});



// Routes: set, get, view, delete (chunk 1)

// -------------------------
// Routes (corrected)
// -------------------------

// -------------------------
// Corrected routes (snapshot passed as serde_json::Map to save_snapshot)
// -------------------------

/// /vars/set
app.at("/vars/set").post(|mut req: Request<AppState>| async move {
    let body: Value = req.body_json().await
        .map_err(|e| tide::Error::from_str(StatusCode::BadRequest, format!("invalid json body: {}", e)))?;

  let Some(values) = body.as_object() else {
    let mut res = Response::new(StatusCode::BadRequest);
    res.set_body("expected JSON object");
    res.insert_header("Content-Type", "text/plain");
    return Ok(res);
  };
  let session = session_id(body.get("session_id").and_then(Value::as_str)).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut values = values.clone();
  values.remove("session_id");
  let mut store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  vars_set_scoped(&mut store, &session, &values);

    let mut res = Response::new(StatusCode::Ok);
    //res.set_body("set complete"); -- we don't need to send a body for this response
    println!("set complete");
    res.insert_header("Content-Type", "text/plain");
    Ok(res)
});

/// /vars/get  (accepts JSON body {"name":"..."} to match client)
app.at("/vars/get").post(|mut req: Request<AppState>| async move {
    let body: Value = req.body_json().await
        .map_err(|e| tide::Error::from_str(StatusCode::BadRequest, format!("invalid json body: {}", e)))?;

    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| tide::Error::from_str(StatusCode::BadRequest, "missing name"))?
        .to_string();
    let session = session_id(body.get("session_id").and_then(Value::as_str)).map_err(|error| {
      tide::Error::from_str(StatusCode::BadRequest, error)
    })?;

    let mut store = req.state().vars_store.lock().map_err(|_| {
      tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
    })?;
    let val_opt = vars_get_scoped(&mut store, &session, &name);

    if let Some(val) = val_opt {
        let mut res = Response::new(StatusCode::Ok);
        res.set_body(serde_json::to_string(&val)?);
        res.insert_header("Content-Type", "application/json; charset=utf-8");

        Ok(res)
    } else {
        let mut res = Response::new(StatusCode::NotFound);
        //res.set_body("not found"); -- we don't need to send a body for this response
        println!("GET {} not found", name);
        res.insert_header("Content-Type", "text/plain; charset=utf-8");

        Ok(res)
    }
});

/// /vars/view
app.at("/vars/view").post(|mut req: Request<AppState>| async move {
  let body: Value = req.body_json().await.unwrap_or_else(|_| Value::Object(Map::new()));
  let session = session_id(body.get("session_id").and_then(Value::as_str)).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let snapshot_map = vars_snapshot_scoped(&store, &session);
  vars_record_history_scoped(&mut store, &session, "VIEW".to_string());

    let mut res = Response::new(StatusCode::Ok);
    res.set_body(serde_json::to_string(&snapshot_map)?);
    res.insert_header("Content-Type", "application/json");
    Ok(res)
});

/// /vars/delete  (accepts JSON body {"name":"..."})
app.at("/vars/delete").post(|mut req: Request<AppState>| async move {
    let body: Value = req.body_json().await
        .map_err(|e| tide::Error::from_str(StatusCode::BadRequest, format!("invalid json body: {}", e)))?;

    let name = body
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| tide::Error::from_str(StatusCode::BadRequest, "missing name"))?
        .to_string();
    let session = session_id(body.get("session_id").and_then(Value::as_str)).map_err(|error| {
      tide::Error::from_str(StatusCode::BadRequest, error)
    })?;

    let mut store = req.state().vars_store.lock().map_err(|_| {
      tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
    })?;
    vars_delete_scoped(&mut store, &session, &name);

    let mut res = Response::new(StatusCode::Ok);
    //res.set_body(format!("Deleted {}", name)); -- we don't need to send a body for this response
    println!("Deleted {}", name);
    res.insert_header("Content-Type", "text/plain");
    Ok(res)
});

/// /vars/clear
app.at("/vars/clear").post(|mut req: Request<AppState>| async move {
  let body: Value = req.body_json().await.unwrap_or_else(|_| Value::Object(Map::new()));
  let session = session_id(body.get("session_id").and_then(Value::as_str)).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  vars_clear_scoped(&mut store, &session);

    let body = serde_json::json!({
        "result": "cleared",
        "vars_count": 0
    });

    let mut res = Response::new(StatusCode::Ok);
    //res.set_body(body.to_string()); -- dont need it
    println!("Cleared all vars");
    res.insert_header("Content-Type", "application/json");
    Ok(res)
});

/// /vars/history
app.at("/vars/history").post(|mut req: Request<AppState>| async move {
  let body: Value = req.body_json().await.unwrap_or_else(|_| Value::Object(Map::new()));
  let session = session_id(body.get("session_id").and_then(Value::as_str)).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let history = vars_history_scoped(&store, &session);
  let mut res = Response::new(StatusCode::Ok);
  res.set_body(serde_json::to_string(&history)?);
  res.insert_header("Content-Type", "application/json; charset=utf-8");
  Ok(res)
});

/// /vars/status
app.at("/vars/status").post(|mut req: Request<AppState>| async move {
  let body: Value = req.body_json().await.unwrap_or_else(|_| Value::Object(Map::new()));
  let session = session_id(body.get("session_id").and_then(Value::as_str)).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let count = vars_snapshot_scoped(&store, &session).len();

    let status = serde_json::json!({
        "vars_count": count,
        "session_id": session,
        "server": true,
        "debug": true,
        "public": true
    });

    println!("status: vars_count = {}", count);

    let mut res = Response::new(StatusCode::Ok);
    match serde_json::to_string(&status) {
        Ok(body) => {
            res.set_body(body);
            res.insert_header("Content-Type", "application/json");
        }
        Err(e) => {
            println!("status: serialization error: {}", e);
            res.set_status(StatusCode::InternalServerError);
            //res.set_body(format!("serialization error: {}", e));
            //res.insert_header("Content-Type", "text/plain");
            println!("serialization error: {}", e);
        }
    }
    Ok(res)
});

// Executes a loop-capable register program against the partitioned variable store.
app.at("/program/run").post(|mut req: Request<AppState>| async move {
  let program: ProgramRequest = req.body_json().await
    .map_err(|error| tide::Error::from_str(
      StatusCode::BadRequest,
      format!("invalid program body: {}", error),
    ))?;
  let mut store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let result = run_program(&mut store, program).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;

  let mut response = Response::new(StatusCode::Ok);
  response.set_body(result.to_string());
  response.insert_header("Content-Type", "application/json; charset=utf-8");
  Ok(response)
});

// Returns a decrementing loop suitable for POST /program/run.
app.at("/program/example").get(|_| async move {
  let mut response = Response::new(StatusCode::Ok);
  response.set_body(serde_json::json!({
    "max_steps": 100,
    "program": [
      { "op": "set", "name": "counter", "value": 3 },
      { "op": "label", "name": "loop" },
      { "op": "decrement", "name": "counter" },
      { "op": "jump_if_nonzero", "name": "counter", "target": "loop" },
      { "op": "halt" }
    ]
  }).to_string());
  response.insert_header("Content-Type", "application/json; charset=utf-8");
  Ok(response)
});

// Describes the built-in Forth/Ruby-like runtime and its public API surface.
app.at("/forth").get(|_| async move {
  Ok(json_response(serde_json::json!({
    "language": "Midscore Forth",
    "implementation": "built into src/main.rs without an external Forth or CAS runtime",
    "interface": "GET /forth/ui",
    "ruby_frontend": "GET /ruby; POST /ruby/eval",
    "storage": {
      "partitioned_array": ["session-scoped variables", "history", "notecards", "matrices", "Second Life bridge queue"],
      "sandboxed_files": "POST /forth/files/*"
    },
    "bridge": { "languages": ["forth", "ruby"], "queue": "POST /forth/bridge/enqueue" },
    "routes": [
      "POST /forth/eval",
      "POST /ruby/eval",
      "POST /sessions/:session_id/forth/eval",
      "POST /sessions/:session_id/ruby/eval",
      "GET /sessions/:session_id/status",
      "POST /ruby/notecards/run",
      "POST /forth/algebra",
      "POST /forth/notecards/save",
      "POST /forth/notecards/get",
      "POST /forth/notecards/list",
      "POST /forth/notecards/run",
      "POST /forth/notecards/delete",
      "POST /forth/files/write",
      "POST /forth/files/read",
      "POST /forth/files/list",
      "POST /forth/files/delete",
      "POST /forth/matrices/get",
      "POST /forth/bridge/enqueue",
      "POST /forth/bridge/poll"
    ],
    "words": {
      "math": ["+", "-", "*", "/", "mod", "abs", "min", "max", "rand", "now"],
      "comparison": ["=", "!=", "<", "<=", ">", ">=", "true", "false", "nil", "not", "and", "or"],
      "variables": ["variable", "let", "@", "!", "$name", "name=", "name+=", "name-=", "interpolate"],
      "collections": ["array", "hash", "c.len", "c.get", "c.set", "c.push", "c.pop", "c.delete", "c.has?", "c.keys", "a.*", "h.*", "null", "null?"],
      "definitions": [": <name> <body> ;", "non-recursive named words expanded before execution"],
      "sequencing": ["; separates Forth statements outside definitions", "; separates RubyForth statements outside strings and collections"],
      "bridging": ["ruby \"<RubyForth source>\" from Forth", "forth \"<Forth source>\" from RubyForth", "forth do ... end multiline blocks in RubyForth"],
      "control": ["if ... else ... then", "begin ... until", "begin ... again", "bye"],
      "matrices": ["matrix", "m.identity", "mget", "mset", "m.fill", "m.scale", "m.add", "m.sub", "m.mul", "m.solve", "m.transpose", "m.det", "m.rows", "m.cols", "m.show"],
      "second_life": ["sl.say", "sl.whisper", "sl.shout", "sl.region_say", "sl.owner_say", "sl.set_text", "sl.set_color", "sl.set_alpha", "sl.play_sound", "sl.set_timer", "sl.set_region_pos", "sl.link_message"]
    },
    "sessions": { "body_field": "session_id", "path_routes": "/sessions/:session_id/*", "default": "default", "lsl_default": "prim UUID" },
    "limits": { "max_steps": 1000000, "max_matrix_dimension": 64, "max_matrix_cells": 4096, "max_file_bytes": 65536 },
    "matrix_example": "2 2 matrix A 1 0 0 mset A 2 1 1 mset A m.det A puts",
    "definition_example": ": square dup * ; 6 square puts",
    "collection_example": "\"name\" \"Ada\" 1 hash \"name\" \"Lin\" 1 hash 2 array people= $people 0 c.get \"name\" c.get puts",
    "bridge_example": ": square dup * ; 6 ruby \"puts 2 + 3\" square puts",
    "sequencing_example": "2 3 + puts; : square dup * ; 4 square puts"
  })))
});

// Browser console for Forth evaluation, algebra, and authenticated bridge jobs.
app.at("/forth/ui").get(|_| async move {
  let mut response = Response::new(StatusCode::Ok);
  response.set_body(r###"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Midscore Forth</title>
<style>
:root{--ink:#17262b;--paper:#f4f0e5;--panel:#fffdf7;--line:#9eb0a7;--sea:#0d6b67;--coral:#ba4936}*{box-sizing:border-box}body{margin:0;background:repeating-linear-gradient(0deg,rgba(13,107,103,.045) 0 1px,transparent 1px 30px),var(--paper);color:var(--ink);font-family:Georgia,serif}main{max-width:1120px;margin:auto;padding:30px 18px 44px}header{display:flex;justify-content:space-between;align-items:end;border-bottom:2px solid var(--ink);padding-bottom:14px;gap:14px}h1{margin:0;font-size:30px}.route,label,button,input,textarea,pre{font-family:ui-monospace,SFMono-Regular,Consolas,monospace}.route,label{font-size:12px;color:#526863}.grid{display:grid;grid-template-columns:minmax(0,1.3fr) minmax(280px,.7fr);gap:20px;margin-top:20px}.panel{background:var(--panel);border:1px solid var(--line);padding:16px;box-shadow:5px 5px 0 rgba(23,38,43,.08)}label{display:block;text-transform:uppercase;margin:0 0 7px;font-weight:700}textarea,input{width:100%;border:1px solid var(--line);background:#fff;padding:10px;color:var(--ink);font-size:14px}textarea{height:300px;resize:vertical;line-height:1.45}.row{display:grid;grid-template-columns:1fr 120px;gap:10px;margin-top:12px}.actions{display:flex;flex-wrap:wrap;gap:8px;margin-top:12px}button{border:1px solid var(--ink);background:var(--sea);color:#fff;padding:9px 12px;font-weight:700;cursor:pointer}button.bridge{background:var(--coral)}button.plain{background:var(--panel);color:var(--ink)}pre{margin:0;min-height:200px;max-height:420px;overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere;padding:12px;background:#14272c;color:#dff1ea;font-size:13px;line-height:1.4}.status{min-height:18px;margin-top:9px;color:#526863;font:12px ui-monospace,SFMono-Regular,Consolas,monospace}@media(max-width:760px){header{align-items:start;flex-direction:column}.grid{grid-template-columns:1fr}.row{grid-template-columns:1fr}}
</style></head><body><main><header><h1>Midscore Forth</h1><div class="route">partitioned-array console</div></header><div class="grid"><section class="panel"><label for="source">Source</label><textarea id="source">let counter 3 counter ! begin $counter puts 1 counter-= $counter 0= until</textarea><div class="row"><div><label for="token">Second Life bridge token</label><input id="token" type="password" autocomplete="off"></div><div><label for="steps">Max steps</label><input id="steps" type="number" min="1" max="1000000" value="10000"></div></div><div class="actions"><button id="run">Run Forth</button><button id="ruby" class="plain">Run RubyForth</button><button id="queue" class="bridge">Queue Forth for SL</button><button id="queueRuby" class="bridge">Queue Ruby for SL</button><button id="catalog" class="plain">Catalog</button></div><div id="status" class="status"></div></section><section class="panel"><label for="expression">Integer polynomial</label><input id="expression" value="3*x^2 - 2*x + 7"><div class="actions"><button id="algebra" class="plain">Analyze</button></div><div style="margin-top:18px"><label for="output">Response</label><pre id="output">Ready.</pre></div></section></div></main><script>
const $=s=>document.querySelector(s),out=$('#output'),status=$('#status');
async function post(path,body){status.textContent='Working...';try{const r=await fetch(path,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const data=await r.json().catch(async()=>({error:await r.text()}));out.textContent=JSON.stringify(data,null,2);status.textContent=r.ok?'Done.':'Request failed.';return data}catch(e){out.textContent=String(e);status.textContent='Network error.'}}
function program(){return{source:$('#source').value,max_steps:Number($('#steps').value)||10000}}
$('#run').onclick=()=>post('/forth/eval',program());$('#ruby').onclick=()=>post('/ruby/eval',program());$('#queue').onclick=()=>post('/forth/bridge/enqueue',{...program(),token:$('#token').value,language:'forth'});$('#queueRuby').onclick=()=>post('/forth/bridge/enqueue',{...program(),token:$('#token').value,language:'ruby'});$('#algebra').onclick=()=>post('/forth/algebra',{expression:$('#expression').value});$('#catalog').onclick=async()=>{const r=await fetch('/forth');out.textContent=JSON.stringify(await r.json(),null,2);status.textContent='Catalog loaded.'};
</script></body></html>"###);
  response.insert_header("Content-Type", "text/html; charset=utf-8");
  Ok(response)
});

// Executes Forth source against integer variables in the partitioned array.
app.at("/forth/eval").post(|mut req: Request<AppState>| async move {
  let program: ForthRunRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid Forth body: {}", error))
  })?;
  let session = session_id(program.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let result = run_forth(&mut store, &session, program).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut response = Response::new(StatusCode::Ok);
  response.set_body(result.to_string());
  response.insert_header("Content-Type", "application/json; charset=utf-8");
  Ok(response)
});

// Executes Forth in the session named by the route instead of the request body.
app.at("/sessions/:session_id/forth/eval").post(|mut req: Request<AppState>| async move {
  let requested_session = req.param("session_id")?.to_string();
  let session = session_id(Some(&requested_session)).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut program: ForthRunRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid Forth body: {}", error))
  })?;
  program.session_id = Some(session.clone());
  let mut store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let result = run_forth(&mut store, &session, program).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  Ok(json_response(result))
});

// Returns the logical values visible in one durable partitioned-array session.
app.at("/sessions/:session_id/status").get(|req: Request<AppState>| async move {
  let requested_session = req.param("session_id")?.to_string();
  let session = session_id(Some(&requested_session)).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let values = vars_snapshot_scoped(&store, &session);
  Ok(json_response(serde_json::json!({
    "session_id": session,
    "value_count": values.len(),
    "values": values,
  })))
});

// Documents the Ruby-shaped frontend that compiles into the Forth runtime.
app.at("/ruby").get(|_| async move {
  Ok(json_response(serde_json::json!({
    "language": "RubyForth",
    "endpoint": "POST /ruby/eval",
    "compiles_to": "Midscore Forth",
    "notecard_runner": "POST /ruby/notecards/run",
    "supported": ["integer and string expressions", "#{name} string interpolation", "semicolon statement separation", "one-line array and hash literals", "get", "set", "push", "pop", "delete", "keys", "length", "has", "assignment", "+=", "-=", "*=", "/=", "puts", "p", "if", "unless", "elsif", "else", "while", "until", "Integer#times", "forth string bridge", "forth do ... end", "end"],
    "not_supported": ["arbitrary Ruby gems", "eval", "require", "classes", "methods", "arbitrary interpolation expressions", "multiline collection literals"],
    "example": "count = 0\nwhile count < 3\n  puts count\n  count += 1\nend"
  })))
});

// Compiles a Ruby-shaped subset into Forth and executes it against the same partitioned state.
app.at("/ruby/eval").post(|mut req: Request<AppState>| async move {
  let program: RubyRunRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid Ruby body: {}", error))
  })?;
  let session = session_id(program.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let compiled_forth = ruby_compile(&program.source).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let mut result = run_forth(&mut store, &session, ForthRunRequest {
    source: compiled_forth.clone(),
    max_steps: program.max_steps,
    session_id: None,
  }).map_err(|error| tide::Error::from_str(StatusCode::BadRequest, error))?;
  if let Value::Object(result) = &mut result {
    result.insert("compiled_forth".to_string(), Value::String(compiled_forth));
    result.insert("language".to_string(), Value::String("RubyForth".to_string()));
  }
  Ok(json_response(result))
});

// Executes RubyForth in the session named by the route instead of the request body.
app.at("/sessions/:session_id/ruby/eval").post(|mut req: Request<AppState>| async move {
  let requested_session = req.param("session_id")?.to_string();
  let session = session_id(Some(&requested_session)).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut program: RubyRunRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid Ruby body: {}", error))
  })?;
  program.session_id = Some(session.clone());
  let compiled_forth = ruby_compile(&program.source).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let mut result = run_forth(&mut store, &session, ForthRunRequest {
    source: compiled_forth.clone(),
    max_steps: program.max_steps,
    session_id: None,
  }).map_err(|error| tide::Error::from_str(StatusCode::BadRequest, error))?;
  if let Value::Object(result) = &mut result {
    result.insert("compiled_forth".to_string(), Value::String(compiled_forth));
    result.insert("language".to_string(), Value::String("RubyForth".to_string()));
  }
  Ok(json_response(result))
});

// Loads a persisted notecard, compiles its Ruby-shaped source, and executes it.
app.at("/ruby/notecards/run").post(|mut req: Request<AppState>| async move {
  let notecard: ForthNotecardNameRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid Ruby notecard body: {}", error))
  })?;
  let session = session_id(notecard.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let source = forth_load_notecard_scoped(&store, &session, &notecard.name).map_err(|error| {
    tide::Error::from_str(StatusCode::NotFound, error)
  })?;
  let compiled_forth = ruby_compile(&source).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut result = run_forth(&mut store, &session, ForthRunRequest {
    source: compiled_forth.clone(),
    max_steps: notecard.max_steps,
    session_id: None,
  }).map_err(|error| tide::Error::from_str(StatusCode::BadRequest, error))?;
  if let Value::Object(result) = &mut result {
    result.insert("notecard".to_string(), Value::String(notecard.name));
    result.insert("compiled_forth".to_string(), Value::String(compiled_forth));
    result.insert("language".to_string(), Value::String("RubyForth".to_string()));
  }
  Ok(json_response(result))
});

// Queues Forth source for the configured in-world bridge object.
app.at("/forth/bridge/enqueue").post(|mut req: Request<AppState>| async move {
  const MAX_BRIDGE_QUEUE: usize = 128;
  const MAX_BRIDGE_SOURCE_BYTES: usize = 65_536;

  let command: ForthBridgeEnqueueRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid bridge body: {}", error))
  })?;
  forth_validate_bridge_token(&command.token).map_err(|error| {
    tide::Error::from_str(StatusCode::Unauthorized, error)
  })?;
  let session = session_id(command.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  if command.source.len() > MAX_BRIDGE_SOURCE_BYTES {
    return Err(tide::Error::from_str(StatusCode::BadRequest, "bridge source exceeds 65536 bytes"));
  }
  if command.max_steps.is_some_and(|steps| steps == 0 || steps > 1_000_000) {
    return Err(tide::Error::from_str(StatusCode::BadRequest, "max_steps must be between 1 and 1000000"));
  }

  let mut queue = req.state().forth_bridge_queue.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "Forth bridge queue lock poisoned")
  })?;
  if queue.non_empty_ids().len() >= MAX_BRIDGE_QUEUE {
    return Err(tide::Error::from_str(StatusCode::ServiceUnavailable, "Forth bridge queue is full"));
  }
  let source = command.source;
  let max_steps = command.max_steps;
  let language = command.language.unwrap_or_else(|| "forth".to_string());
  if language != "forth" && language != "ruby" {
    return Err(tide::Error::from_str(StatusCode::BadRequest, "bridge language must be 'forth' or 'ruby'"));
  }
  let queued_at = Utc::now().to_rfc3339();
  let id = queue.add(|row| {
    row.insert("session_id".to_string(), Value::String(session.clone()));
    row.insert("source".to_string(), Value::String(source.clone()));
    if let Some(max_steps) = max_steps {
      row.insert("max_steps".to_string(), Value::from(max_steps as u64));
    }
    row.insert("language".to_string(), Value::String(language.clone()));
    row.insert("queued_at".to_string(), Value::String(queued_at.clone()));
  }).ok_or_else(|| tide::Error::from_str(StatusCode::InsufficientStorage, "Forth bridge queue is full"))?;
  let message = ForthBridgeMessage {
    id: id as u64,
    session_id: session,
    source,
    max_steps,
    language,
    queued_at,
  };
  Ok(json_response(serde_json::json!({ "queued": message })))
});

// Delivers one queued Forth program to an authenticated in-world bridge object.
app.at("/forth/bridge/poll").post(|mut req: Request<AppState>| async move {
  let request: ForthBridgePollRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid bridge body: {}", error))
  })?;
  forth_validate_bridge_token(&request.token).map_err(|error| {
    tide::Error::from_str(StatusCode::Unauthorized, error)
  })?;
  let session = session_id(request.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut queue = req.state().forth_bridge_queue.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "Forth bridge queue lock poisoned")
  })?;
  let message_id = queue.non_empty_ids().into_iter().find(|id| {
    queue.get(*id)
      .and_then(|row| row.get("session_id"))
      .and_then(Value::as_str)
      .map(|value| value == session)
      .unwrap_or(false)
  });
  let message = message_id.and_then(|id| {
    let message = queue.get(id).and_then(|row| {
      Some(ForthBridgeMessage {
        id: id as u64,
        session_id: session.clone(),
        source: row.get("source")?.as_str()?.to_string(),
        max_steps: row.get("max_steps").and_then(Value::as_u64).and_then(|value| usize::try_from(value).ok()),
        language: row.get("language").and_then(Value::as_str).filter(|value| !value.is_empty()).unwrap_or("forth").to_string(),
        queued_at: row.get("queued_at")?.as_str()?.to_string(),
      })
    });
    let _ = queue.delete(id);
    message
  });
  Ok(json_response(serde_json::json!({ "message": message, "pending": queue.non_empty_ids().len() })))
});

// Simplifies, differentiates, integrates, and optionally evaluates an integer polynomial.
app.at("/forth/algebra").post(|mut req: Request<AppState>| async move {
  let request: ForthAlgebraRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid algebra body: {}", error))
  })?;
  let variable = algebra_variable(request.variable).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let polynomial = algebra_parse(&request.expression, &variable).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let derivative = algebra_derivative(&polynomial).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let integral = algebra_integral(&polynomial, &variable).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let value = request.at.map(|at| algebra_evaluate(&polynomial, at)).transpose().map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  Ok(json_response(serde_json::json!({
    "variable": variable,
    "simplified": algebra_format(&polynomial, &variable),
    "derivative": algebra_format(&derivative, &variable),
    "integral": integral,
    "at": request.at,
    "value": value,
  })))
});

// Retrieves a persisted matrix created through the Forth matrix words.
app.at("/forth/matrices/get").post(|mut req: Request<AppState>| async move {
  let request: ForthMatrixNameRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid matrix body: {}", error))
  })?;
  let session = session_id(request.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let matrix = forth_matrix_load_scoped(&store, &session, &request.name).map_err(|error| {
    tide::Error::from_str(StatusCode::NotFound, error)
  })?;
  Ok(json_response(serde_json::json!({ "name": request.name, "matrix": matrix })))
});

// Saves a named Forth notecard source in the partitioned variable store.
app.at("/forth/notecards/save").post(|mut req: Request<AppState>| async move {
  let notecard: ForthNotecardRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid notecard body: {}", error))
  })?;
  let session = session_id(notecard.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  forth_save_notecard_scoped(&mut store, &session, &notecard.name, notecard.source).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  Ok(json_response(serde_json::json!({ "saved": notecard.name })))
});

// Retrieves a named Forth notecard source.
app.at("/forth/notecards/get").post(|mut req: Request<AppState>| async move {
  let notecard: ForthNotecardNameRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid notecard body: {}", error))
  })?;
  let session = session_id(notecard.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let source = forth_load_notecard_scoped(&store, &session, &notecard.name).map_err(|error| {
    tide::Error::from_str(StatusCode::NotFound, error)
  })?;
  Ok(json_response(serde_json::json!({ "name": notecard.name, "source": source })))
});

// Lists the Forth notecards currently stored in the partitioned variable store.
app.at("/forth/notecards/list").post(|mut req: Request<AppState>| async move {
  let body: Value = req.body_json().await.unwrap_or_else(|_| Value::Object(Map::new()));
  let session = session_id(body.get("session_id").and_then(Value::as_str)).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let prefix = session_storage_key(&session, "forth.notecard.");
  let notecards = store.entries.non_empty_ids().into_iter().filter_map(|id| {
    let row = store.entries.get(id)?;
    let name = row.get("name")?.as_str()?.strip_prefix(&prefix)?.to_string();
    Some(serde_json::json!({ "name": name }))
  }).collect::<Vec<_>>();
  Ok(json_response(serde_json::json!({ "notecards": notecards })))
});

// Runs a saved Forth notecard.
app.at("/forth/notecards/run").post(|mut req: Request<AppState>| async move {
  let notecard: ForthNotecardNameRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid notecard body: {}", error))
  })?;
  let session = session_id(notecard.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  let source = forth_load_notecard_scoped(&store, &session, &notecard.name).map_err(|error| {
    tide::Error::from_str(StatusCode::NotFound, error)
  })?;
  let result = run_forth(&mut store, &session, ForthRunRequest {
    source,
    max_steps: notecard.max_steps,
    session_id: None,
  }).map_err(|error| tide::Error::from_str(StatusCode::BadRequest, error))?;
  Ok(json_response(result))
});

// Removes a saved Forth notecard.
app.at("/forth/notecards/delete").post(|mut req: Request<AppState>| async move {
  let notecard: ForthNotecardNameRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid notecard body: {}", error))
  })?;
  let session = session_id(notecard.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let key = session_storage_key(&session, &forth_notecard_key(&notecard.name).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?);
  let mut store = req.state().vars_store.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "variable store lock poisoned")
  })?;
  if let Some(id) = vars_entry_id(&store, &key) {
    let _ = store.entries.delete(id);
  }
  Ok(json_response(serde_json::json!({ "deleted": notecard.name })))
});

// Writes a UTF-8 file into the server-side Forth sandbox.
app.at("/forth/files/write").post(|mut req: Request<AppState>| async move {
  let file: ForthFileWriteRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid file body: {}", error))
  })?;
  let session = session_id(file.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let bytes = forth_write_file_scoped(&session, &file.name, &file.content).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  Ok(json_response(serde_json::json!({ "name": file.name, "bytes": bytes })))
});

// Reads a UTF-8 file from the server-side Forth sandbox.
app.at("/forth/files/read").post(|mut req: Request<AppState>| async move {
  let file: ForthFileNameRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid file body: {}", error))
  })?;
  let session = session_id(file.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let content = forth_read_file_scoped(&session, &file.name).map_err(|error| {
    tide::Error::from_str(StatusCode::NotFound, error)
  })?;
  Ok(json_response(serde_json::json!({ "name": file.name, "content": content })))
});

// Lists files in the server-side Forth sandbox.
 app.at("/forth/files/list").post(|mut req: Request<AppState>| async move {
  let body: Value = req.body_json().await.unwrap_or_else(|_| Value::Object(Map::new()));
  let session = session_id(body.get("session_id").and_then(Value::as_str)).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let files = forth_list_files_scoped(&session).map_err(|error| {
    tide::Error::from_str(StatusCode::InternalServerError, error)
  })?;
  Ok(json_response(serde_json::json!({ "files": files })))
});

// Deletes a file from the server-side Forth sandbox.
app.at("/forth/files/delete").post(|mut req: Request<AppState>| async move {
  let file: ForthFileNameRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid file body: {}", error))
  })?;
  let session = session_id(file.session_id.as_deref()).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let deleted = forth_delete_file_scoped(&session, &file.name).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  Ok(json_response(serde_json::json!({ "name": file.name, "deleted": deleted })))
});

// Returns a Forth notecard that counts down and prints zero.
app.at("/forth/example").get(|_| async move {
  Ok(json_response(serde_json::json!({
    "name": "countdown",
    "source": "variable counter 3 counter ! begin counter @ 1 - dup counter ! dup 0= until ."
  })))
});

      use tide::prelude::*;
use serde_yaml;
use serde_json;
use chrono::{DateTime, FixedOffset, NaiveDateTime, Datelike, Timelike, Utc};
use std::collections::{BTreeMap, HashSet};


use std::{fs::File, io::{BufRead, BufReader}};

#[derive(serde::Deserialize)]
struct LogEntry {
    avatar_id: Option<String>,
    message: Option<String>,
    timestamp: Option<i64>,
}



   use regex::Regex;


use serde_json::Value;
use chrono::TimeZone;


use chrono_tz::America::Los_Angeles;




    use std::fs;

fn chatlog_markov_json_objects(raw: &str) -> Vec<String> {
  let mut objects = Vec::new();
  let mut buffer = String::new();
  let mut depth = 0i32;
  let mut in_string = false;
  let mut escaped = false;

  for character in raw.chars() {
    if depth > 0 || character == '{' {
      buffer.push(character);
    }

    if in_string {
      if escaped {
        escaped = false;
      } else if character == '\\' {
        escaped = true;
      } else if character == '"' {
        in_string = false;
      }
      continue;
    }

    match character {
      '"' => in_string = true,
      '{' => depth += 1,
      '}' => {
        depth -= 1;
        if depth == 0 {
          let object = std::mem::take(&mut buffer);
          if !object.trim().is_empty() {
            objects.push(object);
          }
        }
      }
      _ => {}
    }
  }

  objects
}

fn chatlog_markov_messages(raw: &str) -> Vec<String> {
  let mut messages = chatlog_markov_json_objects(raw)
    .into_iter()
    .filter_map(|object| serde_json::from_str::<Value>(&object).ok())
    .filter_map(|value| value.get("message").and_then(Value::as_str).map(str::to_string))
    .filter(|message| !message.trim().is_empty())
    .collect::<Vec<_>>();

  if messages.is_empty() {
    messages = raw
      .lines()
      .filter_map(|line| {
        let trimmed = line.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
      })
      .collect();
  }

  messages
}

fn chatlog_markov_tokens(message: &str) -> Vec<String> {
  message
    .split_whitespace()
    .map(|token| {
      token
        .trim_matches(|character: char| !character.is_alphanumeric() && character != '\'')
        .to_lowercase()
    })
    .filter(|token| token.len() > 1)
    .take(32)
    .collect()
}

fn chatlog_markov_generate(messages: &[String], seed: Option<&str>, max_words: usize, samples: usize) -> Vec<String> {
  const START: &str = "__start__";
  let mut transitions: HashMap<(String, String), Vec<String>> = HashMap::new();
  let mut starts = Vec::new();

  for message in messages {
    let tokens = chatlog_markov_tokens(message);
    if tokens.is_empty() {
      continue;
    }
    starts.push(tokens[0].clone());
    let mut left = START.to_string();
    let mut right = START.to_string();
    for token in tokens {
      transitions.entry((left.clone(), right.clone())).or_default().push(token.clone());
      left = right;
      right = token;
    }
  }

  if transitions.is_empty() || starts.is_empty() {
    return vec!["Not enough message text to build a Markov chat model yet.".to_string()];
  }

  let seed_tokens = seed.map(chatlog_markov_tokens).unwrap_or_default();
  let mut rng = rand::thread_rng();
  let sample_count = samples.clamp(1, 8);
  let word_limit = max_words.clamp(8, 80);
  let mut generated = Vec::new();

  for _ in 0..sample_count {
    let mut left = START.to_string();
    let mut right = START.to_string();
    let mut output = Vec::new();

    if let Some(seed_token) = seed_tokens.last() {
      if transitions.keys().any(|(_, next)| next == seed_token) {
        right = seed_token.clone();
        output.push(seed_token.clone());
      }
    }

    for _ in 0..word_limit {
      let options = transitions
        .get(&(left.clone(), right.clone()))
        .or_else(|| transitions.get(&(START.to_string(), START.to_string())));
      let Some(options) = options else { break; };
      if options.is_empty() {
        break;
      }
      use rand::Rng;
      let token = options[rng.gen_range(0..options.len())].clone();
      output.push(token.clone());
      left = right;
      right = token;
    }

    if output.is_empty() {
      use rand::Rng;
      output.push(starts[rng.gen_range(0..starts.len())].clone());
    }

    let mut sentence = output.join(" ");
    if let Some(first) = sentence.get_mut(0..1) {
      first.make_ascii_uppercase();
    }
    if !sentence.ends_with('.') && !sentence.ends_with('!') && !sentence.ends_with('?') {
      sentence.push('.');
    }
    generated.push(sentence);
  }

  generated
}

fn chatlog_markov_transition_counts(messages: &[String]) -> HashMap<(String, String, String), usize> {
  const START: &str = "__start__";
  let mut counts = HashMap::new();

  for message in messages {
    let tokens = chatlog_markov_tokens(message);
    let mut left = START.to_string();
    let mut right = START.to_string();
    for token in tokens {
      *counts.entry((left.clone(), right.clone(), token.clone())).or_insert(0) += 1;
      left = right;
      right = token;
    }
  }

  counts
}

fn chatlog_markov_seed_suggestions(messages: &[String], limit: usize) -> Vec<String> {
  let mut starts = HashMap::new();
  for message in messages {
    if let Some(token) = chatlog_markov_tokens(message).first() {
      *starts.entry(token.clone()).or_insert(0usize) += 1;
    }
  }
  let mut starts = starts.into_iter().collect::<Vec<_>>();
  starts.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
  starts.into_iter().take(limit).map(|(token, _)| token).collect()
}

fn render_chatlog_markov_page(messages: &[String], seed: &str, max_words: usize, samples: usize, generated: &[String]) -> String {
  let rows = generated.iter().map(|line| {
    format!("<article class=\"line\">{}</article>", escape_html(line))
  }).collect::<Vec<_>>().join("");

  format!(r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Second Life Markov Chat Console</title>
<style>
body{{margin:0;background:#0b0c10;color:#d7e4e2;font-family:system-ui,sans-serif}}main{{max-width:980px;margin:auto;padding:22px}}header{{border-bottom:1px solid #45a29e55;padding-bottom:14px}}h1{{margin:0;color:#66fcf1;font-size:22px}}a{{color:#66fcf1}}.links{{display:flex;flex-wrap:wrap;gap:9px;margin-top:12px}}.links a{{border:1px solid #45a29e66;border-radius:999px;padding:6px 10px;text-decoration:none;font-size:12px}}section{{margin-top:16px;background:#1f2833;border:1px solid #45a29e22;border-radius:8px;padding:14px}}label{{display:block;margin-bottom:6px;font-size:12px;color:#b7c7c4}}input{{width:100%;box-sizing:border-box;background:#0b0c10;color:#f4f6f7;border:1px solid #45a29e66;border-radius:6px;padding:9px}}.grid{{display:grid;grid-template-columns:1fr 120px 120px;gap:10px}}button{{margin-top:10px;background:#0d6b67;color:white;border:0;border-radius:6px;padding:10px 14px;font-weight:700;cursor:pointer}}.line{{background:#0b0c10;border-left:3px solid #66fcf1;padding:10px;margin-top:8px;line-height:1.45}}.small{{color:#9fb2ae;font-size:12px;line-height:1.45}}@media(max-width:720px){{.grid{{grid-template-columns:1fr}}}}
</style></head><body><main>
<header><h1>Second Life Markov Chat Console</h1><div class="small">Training corpus: {} messages from the in-memory chatlog store. Synthetic text for scenario review, moderation drills, and tone sampling.</div><nav class="links"><a href="/chatlog">Admin Dashboard</a><a href="/chatlog/summary">Admin JSON</a><a href="/chatlog/recent">Recent JSON</a><a href="/chatlog/markov.json?seed={}&max_words={}&samples={}">Markov JSON</a><a href="/chatlog/markov/transitions.json">Transitions JSON</a><a href="/analytics">Text Analytics</a></nav></header>
<section><form method="get" action="/chatlog/markov"><div class="grid"><div><label for="seed">Seed phrase</label><input id="seed" name="seed" value="{}" placeholder="region lag, welcome, help, music"></div><div><label for="max_words">Max words</label><input id="max_words" name="max_words" type="number" min="8" max="80" value="{}"></div><div><label for="samples">Samples</label><input id="samples" name="samples" type="number" min="1" max="8" value="{}"></div></div><button type="submit">Generate chatter</button></form></section>
<section><h2>Generated Markov Chatter</h2>{}</section>
</main></body></html>"#,
    messages.len(),
    escape_html(seed),
    max_words,
    samples,
    escape_html(seed),
    max_words,
    samples,
    rows,
  )
}

// Mirrors the four built-in dictionaries so custom words merge into the same scoring buckets.
const CUSTOM_WORD_CATEGORIES: [&str; 4] = ["hostile", "positive", "drug", "slang"];

fn custom_word_category_name(category: &str) -> Result<&'static str, String> {
  CUSTOM_WORD_CATEGORIES.iter().find(|&&name| name == category).copied()
    .ok_or_else(|| format!("category must be one of {}", CUSTOM_WORD_CATEGORIES.join(", ")))
}

// Custom words are restricted to single alphanumeric tokens so they match the same
// per-token scoring used by the built-in hostile/positive/drug/slang dictionaries.
fn custom_word_normalize(word: &str) -> Result<String, String> {
  let normalized = word.trim().to_lowercase();
  if normalized.is_empty() || normalized.len() > 32 || !normalized.chars().all(|character| character.is_ascii_alphanumeric()) {
    return Err("word must be 1 to 32 ASCII letters or numbers".to_string());
  }
  Ok(normalized)
}

// Linear scan for the (category, word) key: PartitionedArray has no secondary index.
fn custom_words_entry_id(store: &partitioned_array_rust::PartitionedArray, category: &str, word: &str) -> Option<usize> {
  store.non_empty_ids().into_iter().find(|id| {
    store.get(*id).is_some_and(|row| {
      row.get("category").and_then(Value::as_str) == Some(category)
        && row.get("word").and_then(Value::as_str) == Some(word)
    })
  })
}

// Updates the score in place if (category, word) already exists, otherwise inserts a new row.
fn custom_words_upsert(store: &mut partitioned_array_rust::PartitionedArray, category: &str, word: &str, score: i64) -> Result<(), String> {
  if let Some(id) = custom_words_entry_id(store, category, word) {
    let _ = store.set_with(id, |row| {
      row.insert("score".to_string(), Value::from(score));
    });
    return Ok(());
  }
  let category = category.to_string();
  let word = word.to_string();
  store.add(|row| {
    row.insert("category".to_string(), Value::String(category.clone()));
    row.insert("word".to_string(), Value::String(word.clone()));
    row.insert("score".to_string(), Value::from(score));
    row.insert("added_at".to_string(), Value::String(Utc::now().to_rfc3339()));
  }).ok_or_else(|| "custom word store is full".to_string())?;
  Ok(())
}

fn custom_words_delete(store: &mut partitioned_array_rust::PartitionedArray, category: &str, word: &str) -> bool {
  match custom_words_entry_id(store, category, word) {
    Some(id) => { let _ = store.delete(id); true }
    None => false,
  }
}

fn custom_words_list(store: &partitioned_array_rust::PartitionedArray) -> Vec<Value> {
  let mut entries = store.non_empty_ids().into_iter()
    .filter_map(|id| store.get(id).cloned())
    .map(Value::Object)
    .collect::<Vec<_>>();
  entries.sort_by(|left, right| {
    let left_category = left.get("category").and_then(Value::as_str).unwrap_or("");
    let right_category = right.get("category").and_then(Value::as_str).unwrap_or("");
    let left_word = left.get("word").and_then(Value::as_str).unwrap_or("");
    let right_word = right.get("word").and_then(Value::as_str).unwrap_or("");
    left_category.cmp(right_category).then_with(|| left_word.cmp(right_word))
  });
  entries
}

// Builds the word->score lookup for one category, used to extend a built-in dictionary.
fn custom_words_map(store: &partitioned_array_rust::PartitionedArray, category: &str) -> HashMap<String, i32> {
  store.non_empty_ids().into_iter().filter_map(|id| {
    let row = store.get(id)?;
    if row.get("category").and_then(Value::as_str) != Some(category) {
      return None;
    }
    let word = row.get("word")?.as_str()?.to_string();
    let score = row.get("score").and_then(Value::as_i64).unwrap_or(0) as i32;
    Some((word, score))
  }).collect()
}

// POST /chatlog/words/add body; score defaults to 2 when omitted.
#[derive(Deserialize)]
struct CustomWordRequest {
  category: String,
  word: String,
  #[serde(default)]
  score: Option<i64>,
}

// POST /chatlog/words/delete body.
#[derive(Deserialize)]
struct CustomWordKeyRequest {
  category: String,
  word: String,
}

fn render_chatlog_words_page(words: &[Value]) -> String {
  let words_json = serde_json::to_string(words)
    .unwrap_or_else(|_| "[]".to_string())
    .replace("</", "<\\/");

  let template = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Chatlog Custom Alert Words</title>
<style>
body{margin:0;background:#0b0c10;color:#d7e4e2;font-family:system-ui,sans-serif}
main{max-width:900px;margin:auto;padding:22px}
header{border-bottom:1px solid #45a29e55;padding-bottom:14px}
h1{margin:0;color:#66fcf1;font-size:22px}
a{color:#66fcf1}
.small{color:#9fb2ae;font-size:12px;line-height:1.45}
.links{display:flex;flex-wrap:wrap;gap:9px;margin-top:12px}
.links a{border:1px solid #45a29e66;border-radius:999px;padding:6px 10px;text-decoration:none;font-size:12px}
section{margin-top:16px;background:#1f2833;border:1px solid #45a29e22;border-radius:8px;padding:14px}
label{display:block;margin-bottom:6px;font-size:12px;color:#b7c7c4}
input,select{width:100%;box-sizing:border-box;background:#0b0c10;color:#f4f6f7;border:1px solid #45a29e66;border-radius:6px;padding:9px;font:inherit}
.grid{display:grid;grid-template-columns:1fr 1fr 110px 110px;gap:10px;align-items:end}
button{background:#0d6b67;color:white;border:0;border-radius:6px;padding:10px 14px;font-weight:700;cursor:pointer}
table{width:100%;border-collapse:collapse;font-size:13px;margin-top:8px}
th,td{padding:6px 8px;border-bottom:1px solid #45a29e22;text-align:left}
.del{background:#a64c48}
.status{min-height:16px;margin-top:8px;color:#9fb2ae;font-size:12px}
.empty{color:#9fb2ae;padding:10px 0}
@media(max-width:720px){.grid{grid-template-columns:1fr}}
</style></head><body><main>
<header><h1>Chatlog Custom Alert Words</h1><div class="small">Words added here are merged into the built-in hostile, positive, drug, and slang dictionaries used by the /chatlog dashboard.</div><nav class="links"><a href="/chatlog">Admin Dashboard</a><a href="/chatlog/words/list">Words JSON</a><a href="/chatlog/markov">Markov Chat</a></nav></header>
<section><h2>Add a word</h2><form id="wordForm"><div class="grid"><div><label for="category">Category</label><select id="category" name="category"><option value="hostile">Hostile</option><option value="positive">Positive</option><option value="drug">Drug/alcohol</option><option value="slang">Slang</option></select></div><div><label for="word">Word</label><input id="word" name="word" placeholder="letters or numbers only" required></div><div><label for="score">Score</label><input id="score" name="score" type="number" min="-10" max="10" value="2"></div><div><button type="submit">Save word</button></div></div><div id="status" class="status"></div></form></section>
<section><h2>Custom words</h2><table><thead><tr><th>Category</th><th>Word</th><th>Score</th><th></th></tr></thead><tbody id="wordRows"></tbody></table><div id="emptyState" class="empty" hidden>No custom words yet.</div></section>
<script id="wordsSeed" type="application/json">__WORDS_JSON__</script>
<script>
(() => {
  const seed = JSON.parse(document.getElementById('wordsSeed').textContent);
  const rows = document.getElementById('wordRows'), empty = document.getElementById('emptyState'), status = document.getElementById('status');
  const render = words => {
    rows.replaceChildren();
    empty.hidden = words.length > 0;
    for (const word of words) {
      const tr = document.createElement('tr');
      const category = document.createElement('td'); category.textContent = word.category;
      const term = document.createElement('td'); term.textContent = word.word;
      const score = document.createElement('td'); score.textContent = word.score;
      const actions = document.createElement('td');
      const del = document.createElement('button'); del.className = 'del'; del.type = 'button'; del.textContent = 'Delete';
      del.addEventListener('click', () => remove(word.category, word.word));
      actions.append(del);
      tr.append(category, term, score, actions);
      rows.append(tr);
    }
  };
  const remove = async (category, word) => {
    status.textContent = 'Removing...';
    const response = await fetch('/chatlog/words/delete', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ category, word }) });
    const data = await response.json().catch(() => ({}));
    status.textContent = response.ok ? 'Removed.' : (data.error || 'Failed to remove word.');
    if (response.ok && data.words) render(data.words);
  };
  document.getElementById('wordForm').addEventListener('submit', async event => {
    event.preventDefault();
    const form = new FormData(event.currentTarget);
    const body = { category: form.get('category'), word: form.get('word'), score: Number(form.get('score')) || 0 };
    status.textContent = 'Saving...';
    const response = await fetch('/chatlog/words/add', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
    const data = await response.json().catch(() => ({}));
    status.textContent = response.ok ? 'Saved.' : (data.error || 'Failed to save word.');
    if (response.ok && data.words) { render(data.words); event.currentTarget.reset(); document.getElementById('score').value = '2'; }
  });
  render(seed);
})();
</script>
</main></body></html>"#;

  template.replace("__WORDS_JSON__", &words_json)
}

// Adds or updates a custom word merged into the /chatlog alert dictionaries.
app.at("/chatlog/words/add").post(|mut req: Request<AppState>| async move {
  let request: CustomWordRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid custom word body: {}", error))
  })?;
  let category = custom_word_category_name(&request.category).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let word = custom_word_normalize(&request.word).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let score = request.score.unwrap_or(2).clamp(-10, 10);
  let mut store = req.state().custom_words.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "custom word store lock poisoned")
  })?;
  custom_words_upsert(&mut store, category, &word, score).map_err(|error| {
    tide::Error::from_str(StatusCode::InsufficientStorage, error)
  })?;
  let words = custom_words_list(&store);
  drop(store);
  persist_memory_stores(req.state()).map_err(|error| {
    tide::Error::from_str(StatusCode::InternalServerError, format!("custom word was not persisted: {}", error))
  })?;
  Ok(json_response(serde_json::json!({
    "saved": { "category": category, "word": word, "score": score },
    "words": words,
  })))
});

// Removes a custom word from the /chatlog alert dictionaries.
app.at("/chatlog/words/delete").post(|mut req: Request<AppState>| async move {
  let request: CustomWordKeyRequest = req.body_json().await.map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, format!("invalid custom word body: {}", error))
  })?;
  let category = custom_word_category_name(&request.category).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let word = custom_word_normalize(&request.word).map_err(|error| {
    tide::Error::from_str(StatusCode::BadRequest, error)
  })?;
  let mut store = req.state().custom_words.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "custom word store lock poisoned")
  })?;
  let deleted = custom_words_delete(&mut store, category, &word);
  let words = custom_words_list(&store);
  drop(store);
  persist_memory_stores(req.state()).map_err(|error| {
    tide::Error::from_str(StatusCode::InternalServerError, format!("custom word deletion was not persisted: {}", error))
  })?;
  Ok(json_response(serde_json::json!({ "deleted": deleted, "words": words })))
});

// Lists custom words merged into the /chatlog alert dictionaries.
app.at("/chatlog/words/list").get(|req: Request<AppState>| async move {
  let store = req.state().custom_words.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "custom word store lock poisoned")
  })?;
  Ok(json_response(serde_json::json!({
    "categories": CUSTOM_WORD_CATEGORIES,
    "words": custom_words_list(&store),
  })))
});

// Browser console for managing custom /chatlog alert words.
app.at("/chatlog/words").get(|req: Request<AppState>| async move {
  let store = req.state().custom_words.lock().map_err(|_| {
    tide::Error::from_str(StatusCode::InternalServerError, "custom word store lock poisoned")
  })?;
  let words = custom_words_list(&store);
  drop(store);
  let mut res = Response::new(StatusCode::Ok);
  res.set_body(render_chatlog_words_page(&words));
  res.insert_header("Content-Type", "text/html; charset=utf-8");
  Ok(res)
});

app.at("/chatlog/summary").get(|_| async move {
  Ok(redirect("/chatlog?format=summary"))
});

app.at("/chatlog/recent").get(|_| async move {
  Ok(redirect("/chatlog?format=recent"))
});

app.at("/chatlog/admin").get(|_| async move {
  Ok(redirect("/chatlog?format=summary"))
});

app.at("/chatlog/markov.json").get(|req: tide::Request<AppState>| async move {
  let query: HashMap<String, String> = req.query().unwrap_or_default();
  let seed = query.get("seed").map(String::as_str);
  let max_words = query.get("max_words").and_then(|value| value.parse::<usize>().ok()).unwrap_or(36);
  let samples = query.get("samples").and_then(|value| value.parse::<usize>().ok()).unwrap_or(4);
  let (raw, source_modified) = chatlog_store_snapshot(req.state().chatlog_store.as_ref());
  let messages = chatlog_markov_messages(&raw);
  let generated = chatlog_markov_generate(&messages, seed, max_words, samples);
  Ok(json_response(serde_json::json!({
    "source": "in-memory chatlog store",
    "source_modified": source_modified,
    "message_count": messages.len(),
    "seed": seed.unwrap_or(""),
    "max_words": max_words.clamp(8, 80),
    "samples": samples.clamp(1, 8),
    "generated": generated,
  })))
});

app.at("/chatlog/markov/transitions.json").get(|req: tide::Request<AppState>| async move {
  let query: HashMap<String, String> = req.query().unwrap_or_default();
  let limit = query.get("limit").and_then(|value| value.parse::<usize>().ok()).unwrap_or(50).clamp(1, 250);
  let (raw, source_modified) = chatlog_store_snapshot(req.state().chatlog_store.as_ref());
  let messages = chatlog_markov_messages(&raw);
  let counts = chatlog_markov_transition_counts(&messages);
  let mut transitions = counts.into_iter().collect::<Vec<_>>();
  transitions.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
  let total_transitions = transitions.iter().map(|(_, count)| *count).sum::<usize>();
  let unique_state_count = transitions
    .iter()
    .map(|((left, right, _), _)| (left.clone(), right.clone()))
    .collect::<HashSet<_>>()
    .len();
  let unique_transition_count = transitions.len();
  let rows = transitions
    .into_iter()
    .take(limit)
    .map(|((left, right, next), count)| {
      serde_json::json!({
        "state": [left, right],
        "next": next,
        "count": count,
        "probability": if total_transitions == 0 { 0.0 } else { count as f64 / total_transitions as f64 },
      })
    })
    .collect::<Vec<_>>();

  Ok(json_response(serde_json::json!({
    "source": "in-memory chatlog store",
    "source_modified": source_modified,
    "message_count": messages.len(),
    "state_count": unique_state_count,
    "unique_transition_count": unique_transition_count,
    "returned_transition_count": rows.len(),
    "total_transitions": total_transitions,
    "seed_suggestions": chatlog_markov_seed_suggestions(&messages, 12),
    "transitions": rows,
  })))
});

app.at("/chatlog/markov").get(|req: tide::Request<AppState>| async move {
  let query: HashMap<String, String> = req.query().unwrap_or_default();
  let seed = query.get("seed").map(String::as_str).unwrap_or("");
  let max_words = query.get("max_words").and_then(|value| value.parse::<usize>().ok()).unwrap_or(36).clamp(8, 80);
  let samples = query.get("samples").and_then(|value| value.parse::<usize>().ok()).unwrap_or(4).clamp(1, 8);
  let (raw, _) = chatlog_store_snapshot(req.state().chatlog_store.as_ref());
  let messages = chatlog_markov_messages(&raw);
  let generated = chatlog_markov_generate(&messages, Some(seed), max_words, samples);
  let mut res = Response::new(StatusCode::Ok);
  res.set_body(render_chatlog_markov_page(&messages, seed, max_words, samples, &generated));
  res.insert_header("Content-Type", "text/html; charset=utf-8");
  Ok(res)
});

app.at("/chatlog").get(|req: tide::Request<AppState>| async move {
  use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
    use std::fs;
    use chrono::{Datelike, LocalResult, Timelike, Weekday};
    use chrono_tz::America::Los_Angeles;
    use serde::Serialize;
    use serde_json::Value;
    use tide::{Response, StatusCode};

    #[derive(Clone, Serialize)]
    struct MsgEntry {
        avatar_id: String,
        avatar_name: String,
        sim_name: String,
        message: String,
        timestamp: i64,
        timestamp_valid: bool,
        x: f64,
        y: f64,
        z: f64,
        position_valid: bool,
        captured_by: String,
        hostility_score: i64,
        positive_score: i64,
        drug_score: i64,
        slang_score: i64,
        tags: Vec<String>,
        hostile_terms: Vec<String>,
        quarantined: bool,
    }

    #[derive(Default, Serialize)]
    struct IntegrityReport {
        total_lines: usize,
        candidate_objects: usize,
        parsed_objects: usize,
        parse_errors: usize,
        missing_avatar_name: usize,
        missing_sim_name: usize,
        missing_message: usize,
        bad_timestamp: usize,
        bad_position: usize,
    }

    // Splits the raw log into top-level `{...}` JSON objects, respecting
    // nested braces and braces embedded inside string values (e.g. chat text).
    fn split_json_objects(raw: &str) -> Vec<String> {
        let mut objects = Vec::new();
        let mut buf = String::new();
        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escaped = false;

        for ch in raw.chars() {
            if depth > 0 || ch == '{' {
                buf.push(ch);
            }

            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let obj = std::mem::take(&mut buf);
                        if !obj.trim().is_empty() {
                            objects.push(obj);
                        }
                    }
                }
                _ => {}
            }
        }

        objects
    }

    fn json_i64(v: &Value) -> Option<i64> {
        v.as_i64()
        .or_else(|| {
          v.as_f64().and_then(|number| {
            if number.is_finite()
              && number.fract() == 0.0
              && number >= i64::MIN as f64
              && number <= i64::MAX as f64
            {
              Some(number as i64)
            } else {
              None
            }
          })
        })
            .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
    }

    fn json_f64(v: &Value) -> Option<f64> {
      v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
        .filter(|number| number.is_finite())
    }

    fn json_string(v: &Value) -> Option<String> {
        v.as_str().map(|s| s.to_string())
    }

    let requested_format = req
      .url()
      .query_pairs()
      .find_map(|(key, value)| (key == "format").then(|| value.into_owned()));
    let recent_only = requested_format.as_deref() == Some("recent");
    let (raw, source_modified) = chatlog_store_snapshot(req.state().chatlog_store.as_ref());
    let source_len = raw.len() as u64;
    let cache_key = match requested_format.as_deref() {
      None | Some("html") => Some("html"),
      Some("recent") => Some("recent"),
      _ => None,
    };
    if let Some(cache_key) = cache_key {
      if let Some(body) = chatlog_cache_get(
        req.state().chatlog_cache.as_ref(),
        source_len,
        &source_modified,
        cache_key,
      ) {
        let mut res = Response::new(StatusCode::Ok);
        res.set_body(body);
        res.insert_header(
          "Content-Type",
          if recent_only {
            "application/json; charset=utf-8"
          } else {
            "text/html; charset=utf-8"
          },
        );
        return Ok(res);
      }
    }

    let mut integrity = IntegrityReport::default();
    let mut quarantine: Vec<MsgEntry> = Vec::new();

    integrity.total_lines = raw.lines().count();

    let objects = split_json_objects(&raw);
    integrity.candidate_objects = objects.len();

    // Words saved through /chatlog/words (see custom_words_map below) are merged
    // into each built-in dictionary so admins can extend scoring without a rebuild.
    let custom_words_store = req.state().custom_words.lock().map_err(|_| {
      tide::Error::from_str(StatusCode::InternalServerError, "custom word store lock poisoned")
    })?;

    let mut hostile_words: HashMap<String, i32> = [
        ("hate", 3), ("kill", 4), ("stupid", 2), ("idiot", 3), ("annoying", 2),
        ("terrible", 2), ("awful", 2), ("trash", 2), ("loser", 3), ("angry", 1),
        ("mad", 1), ("toxic", 2), ("cringe", 1), ("lame", 1), ("jerk", 2),
        ("bully", 3), ("rekt", 2), ("owned", 2), ("noob", 2), ("scrub", 2),
        ("clown", 2), ("dumb", 2), ("moron", 3), ("pathetic", 3), ("worthless", 4),
      ("abuse", 2), ("abusive", 3), ("aggression", 2), ("aggressive", 2),
      ("attack", 2), ("attacking", 2), ("threat", 3), ("threaten", 3),
      ("threatening", 3), ("harass", 3), ("harassment", 3), ("harassing", 3),
      ("hateful", 3), ("hostile", 2), ("rude", 1), ("mean", 1), ("nasty", 2),
      ("vile", 3), ("disgusting", 2), ("gross", 1), ("suck", 1), ("sucks", 1),
      ("liar", 2), ("lying", 2), ("fraud", 2), ("fake", 1), ("cheat", 2),
      ("cheater", 2), ("scam", 2), ("scammer", 3), ("shame", 1), ("failure", 2),
      ("useless", 3), ("garbage", 2), ("fool", 2), ("idiotic", 3),
      ("imbecile", 3), ("ignorant", 2), ("obnoxious", 2), ("disrespect", 2),
      ("disrespectful", 2), ("coward", 2), ("punch", 2), ("violent", 3),
      ("violence", 3), ("hurt", 2), ("harm", 3), ("die", 3), ("dead", 2),
      ("racist", 3), ("racism", 3), ("sexist", 3), ("sexism", 3), ("bigot", 3),
      ("bigotry", 3), ("slur", 2), ("nazi", 4), ("supremacist", 4), ("genocide", 4),
      ("terrorist", 4), ("terrorism", 4), ("predator", 4), ("predatory", 3),
      ("groomer", 4), ("grooming", 4), ("molest", 4), ("molester", 4), ("rape", 4),
      ("rapist", 4), ("assault", 3), ("murder", 4), ("murderer", 4), ("psycho", 2),
      ("psychotic", 2), ("stalker", 3), ("stalking", 3), ("doxx", 3), ("doxxing", 3),
      ("swatting", 4), ("extort", 3), ("extortion", 3), ("blackmail", 3),
      ("intimidate", 2), ("intimidation", 2), ("menace", 2), ("menacing", 2),
      ("degenerate", 2), ("scum", 2), ("filth", 2), ("vermin", 2), ("subhuman", 3),
      ("victimize", 2), ("abuser", 3), ("creep", 2), ("creepy", 2), ("pervert", 3),
      ("pervy", 2),
    ].iter().map(|&(word, score)| (word.to_string(), score)).collect();
    hostile_words.extend(custom_words_map(&custom_words_store, "hostile"));

    let mut positive_words: HashMap<String, i32> = [
        ("love", 3), ("great", 2), ("awesome", 2), ("nice", 1), ("cool", 1),
        ("fun", 1), ("good", 1), ("beautiful", 2), ("kind", 2), ("friendly", 2),
        ("sweet", 2), ("amazing", 3), ("fantastic", 3), ("wonderful", 3),
        ("thanks", 2), ("thank", 1), ("appreciate", 2), ("appreciated", 2),
        ("helpful", 2), ("excellent", 3), ("brilliant", 3), ("perfect", 3),
        ("glad", 2), ("happy", 2), ("joy", 2), ("support", 2), ("supportive", 2),
        ("welcome", 1), ("cheers", 1), ("congrats", 2), ("congratulations", 3),
        ("respect", 2), ("peace", 2), ("smile", 1), ("laugh", 1), ("yay", 1),
      ("wholesome", 2), ("generous", 2), ("generosity", 2), ("charming", 2),
      ("charismatic", 2), ("talented", 2), ("genius", 2), ("clever", 1),
      ("graceful", 2), ("elegant", 2), ("stunning", 2), ("gorgeous", 2),
      ("adorable", 2), ("cute", 1), ("funny", 1), ("hilarious", 2), ("delight", 2),
      ("delighted", 2), ("delightful", 2), ("grateful", 2), ("gratitude", 2),
      ("blessed", 2), ("blessing", 2), ("inspire", 2), ("inspiring", 2),
      ("inspired", 2), ("motivate", 1), ("motivating", 1), ("encourage", 1),
      ("encouraging", 1), ("uplift", 2), ("uplifting", 2), ("harmony", 2),
      ("harmonious", 2), ("calm", 1), ("relaxing", 1), ("soothing", 1),
      ("comfort", 1), ("comforting", 1), ("celebrate", 2), ("celebration", 2),
      ("victory", 2), ("accomplish", 2), ("accomplished", 2), ("proud", 2),
      ("pride", 1), ("honored", 2), ("sincere", 1), ("trustworthy", 2),
      ("reliable", 1), ("patient", 1), ("patience", 1),
    ].iter().map(|&(word, score)| (word.to_string(), score)).collect();
    positive_words.extend(custom_words_map(&custom_words_store, "positive"));

    let mut drug_words: HashMap<String, i32> = [
        ("drug", 1), ("drugs", 1), ("overdose", 3), ("intoxicated", 2),
        ("substance", 1), ("addiction", 2), ("rehab", 1), ("narcotic", 1),
        ("opioid", 2), ("heroin", 3), ("cocaine", 3), ("meth", 3),
        ("weed", 1), ("marijuana", 1),
        ("alcohol", 1), ("beer", 1), ("wine", 1), ("vodka", 1), ("whiskey", 1),
        ("cannabis", 1), ("cbd", 1), ("thc", 1), ("fentanyl", 3),
        ("amphetamine", 2), ("ketamine", 2), ("lsd", 2), ("mdma", 2),
        ("ecstasy", 2), ("benzodiazepine", 2), ("xanax", 2), ("valium", 2),
        ("pill", 1), ("pills", 1), ("sober", 1), ("withdrawal", 2), ("relapse", 2),
      ("opiate", 2), ("stimulant", 1), ("depressant", 1), ("hallucinogen", 2),
      ("psychedelic", 1), ("shrooms", 2), ("mushrooms", 1), ("molly", 2),
      ("crack", 3), ("dope", 1), ("junkie", 2), ("addict", 2), ("addicted", 2),
      ("overdosed", 3), ("overdosing", 3), ("vape", 1), ("vaping", 1),
      ("nicotine", 1), ("tobacco", 1), ("cigarette", 1), ("cigarettes", 1),
      ("joint", 1), ("blunt", 1), ("bong", 1), ("syringe", 2), ("needle", 1),
      ("dealer", 2), ("dealing", 1), ("trafficking", 3), ("cartel", 2),
      ("speed", 1), ("tripping", 1), ("stoned", 1), ("drunk", 1), ("wasted", 1),
      ("hammered", 1), ("buzzed", 1), ("tipsy", 1), ("booze", 1), ("liquor", 1),
      ("rum", 1), ("gin", 1), ("tequila", 1), ("brandy", 1),
    ].iter().map(|&(word, score)| (word.to_string(), score)).collect();
    drug_words.extend(custom_words_map(&custom_words_store, "drug"));

    let mut slang_words: HashMap<String, i32> = [
        ("lol", 0), ("lmao", 0), ("rofl", 0), ("bruh", 0), ("fr", 0),
        ("sus", 1), ("salty", 1), ("ratio", 1), ("based", 0), ("cap", 1),
        ("yeet", 0), ("pog", 0), ("poggers", 0), ("smh", 0), ("ngl", 0),
        ("idk", 0),
        ("omg", 0), ("wtf", 1), ("imo", 0), ("tbh", 0), ("irl", 0),
        ("btw", 0), ("gg", 0), ("ggs", 0), ("wp", 0), ("rip", 1),
        ("yikes", 1), ("fomo", 0), ("lowkey", 0), ("highkey", 0),
        ("vibe", 0), ("vibes", 0), ("goated", 0), ("cracked", 0),
        ("cope", 1), ("seethe", 1), ("savage", 1),
      ("yolo", 0), ("stan", 0), ("simp", 1), ("thirsty", 1), ("ghosted", 1),
      ("ghosting", 1), ("flex", 0), ("flexing", 0), ("clout", 0), ("cringy", 1),
      ("bet", 0), ("fam", 0), ("lit", 0), ("turnt", 0), ("extra", 0),
      ("shade", 1), ("tea", 0), ("receipts", 0), ("canceled", 1), ("cancelled", 1),
      ("triggered", 1), ("woke", 0), ("karen", 1), ("boomer", 1), ("zoomer", 0),
      ("npc", 0), ("copium", 1), ("doomer", 0), ("bloomer", 0), ("chad", 0),
      ("gigachad", 0), ("cringelord", 1), ("mood", 0), ("bigmood", 0),
    ].iter().map(|&(word, score)| (word.to_string(), score)).collect();
    slang_words.extend(custom_words_map(&custom_words_store, "slang"));

    drop(custom_words_store);

    let mut unique_keys: HashSet<String> = HashSet::new();

    // Time-derived frequency tables
    let mut freq_year: BTreeMap<i32, usize> = BTreeMap::new();
    let mut freq_month: BTreeMap<u32, usize> = BTreeMap::new();
    let mut freq_day: BTreeMap<u32, usize> = BTreeMap::new();
    let mut freq_weekday: BTreeMap<String, usize> = BTreeMap::new();
    let mut freq_hour: BTreeMap<u32, usize> = BTreeMap::new();
    let mut freq_minute: BTreeMap<u32, usize> = BTreeMap::new();
    let mut freq_second: BTreeMap<u32, usize> = BTreeMap::new();
    let mut freq_weeknum: BTreeMap<u32, usize> = BTreeMap::new();
    let mut freq_quarter: BTreeMap<u32, usize> = BTreeMap::new();
    let mut freq_day_of_year: BTreeMap<u32, usize> = BTreeMap::new();
    let mut freq_epoch_bucket: BTreeMap<i64, usize> = BTreeMap::new(); // 1-hour buckets
    let mut freq_ampm: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut freq_weekend: BTreeMap<&'static str, usize> = BTreeMap::new();

    let mut timeline: BTreeMap<String, usize> = BTreeMap::new();
    let mut sentiment_timeline: BTreeMap<String, i64> = BTreeMap::new();

    let mut avatar_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut sim_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut freq_avatar_id: BTreeMap<String, usize> = BTreeMap::new();

    let mut messages_vec: Vec<MsgEntry> = Vec::new();

    let mut word_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut avatar_hostility: BTreeMap<String, i64> = BTreeMap::new();
    let mut sim_hostility: BTreeMap<String, i64> = BTreeMap::new();
    let mut avatar_scores: BTreeMap<String, (i64, i64, i64, i64, i64)> = BTreeMap::new();

    let mut sim_transitions: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut avatar_interactions: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut topic_buckets: BTreeMap<String, usize> = BTreeMap::new();

    let mut captured_by_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut captured_by_sentiment: BTreeMap<String, i64> = BTreeMap::new();
    let mut captured_by_sim_counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut captured_by_topics: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();

    let mut rate_of_change: Vec<(i64, f64)> = Vec::new();
    let mut roc_per_sim: BTreeMap<String, Vec<(i64, f64)>> = BTreeMap::new();

    let markov_messages = chatlog_markov_messages(&raw);
    let markov_transition_counts = chatlog_markov_transition_counts(&markov_messages);
    let markov_transition_count = markov_transition_counts.values().sum::<usize>();
    let markov_state_count = markov_transition_counts.len();
    let markov_seed_suggestions = chatlog_markov_seed_suggestions(&markov_messages, 10);
    let markov_readiness = if markov_messages.len() >= 20 && markov_state_count >= 50 {
      "Ready"
    } else if markov_messages.len() >= 5 {
      "Limited corpus"
    } else {
      "Needs more messages"
    };

    let mut freq_timestamp: BTreeMap<i64, usize> = BTreeMap::new();
    let mut freq_message_len_bucket: BTreeMap<usize, usize> = BTreeMap::new();
    let mut freq_x_bucket: BTreeMap<i64, usize> = BTreeMap::new();
    let mut freq_y_bucket: BTreeMap<i64, usize> = BTreeMap::new();
    let mut freq_z_bucket: BTreeMap<i64, usize> = BTreeMap::new();

    // real 3D heatmap points: one point per message using raw x,y,z
    let mut heat3d_points: Vec<(f64, f64, f64)> = Vec::new();

    for obj in &objects {
        let parsed: Value = match serde_json::from_str(obj) {
            Ok(v) => v,
            Err(_) => {
                integrity.parse_errors += 1;
                continue;
            }
        };
        integrity.parsed_objects += 1;

        let avatar_id = parsed
            .get("avatar_id")
            .and_then(json_string)
            .unwrap_or_default();

        let avatar_name = parsed.get("avatar_name").and_then(json_string).unwrap_or_else(|| {
            integrity.missing_avatar_name += 1;
            String::new()
        });

        let message = parsed.get("message").and_then(json_string).unwrap_or_else(|| {
            integrity.missing_message += 1;
            String::new()
        });

        let sim_name = parsed.get("sim_name").and_then(json_string).unwrap_or_else(|| {
            integrity.missing_sim_name += 1;
            String::new()
        });

        let timestamp = parsed
            .get("timestamp")
          .and_then(json_i64);
        let timestamp_valid = timestamp.is_some();
        let timestamp = timestamp.unwrap_or_else(|| {
          integrity.bad_timestamp += 1;
          0
        });

        let x = parsed.get("x_pos").and_then(json_f64);
        if x.is_none() {
            integrity.bad_position += 1;
        }
        let y = parsed.get("y_pos").and_then(json_f64);
        if y.is_none() {
            integrity.bad_position += 1;
        }
        let z = parsed.get("z_pos").and_then(json_f64);
        if z.is_none() {
            integrity.bad_position += 1;
        }
        let position_valid = x.is_some() && y.is_some() && z.is_some();
        let x = x.unwrap_or(0.0);
        let y = y.unwrap_or(0.0);
        let z = z.unwrap_or(0.0);

        let captured_by = parsed
            .get("captured_by")
            .and_then(json_string)
            .unwrap_or_default();

        let key = format!("{}|{}|{}", avatar_id, timestamp, message);
        unique_keys.insert(key);

        if timestamp_valid {
          *freq_timestamp.entry(timestamp).or_insert(0) += 1;
        }

        if !avatar_id.is_empty() {
            *freq_avatar_id.entry(avatar_id.clone()).or_insert(0) += 1;
        }

        let len_bucket = (message.chars().count() / 20) * 20;
        *freq_message_len_bucket.entry(len_bucket).or_insert(0) += 1;

        if position_valid {
          let x_bucket = ((x / 10.0).round() as i64) * 10;
          let y_bucket = ((y / 10.0).round() as i64) * 10;
          let z_bucket = ((z / 10.0).round() as i64) * 10;
          *freq_x_bucket.entry(x_bucket).or_insert(0) += 1;
          *freq_y_bucket.entry(y_bucket).or_insert(0) += 1;
          *freq_z_bucket.entry(z_bucket).or_insert(0) += 1;
        }

        if timestamp_valid {
          if let LocalResult::Single(dt) = Los_Angeles.timestamp_opt(timestamp, 0) {
            let year = dt.year();
            let month = dt.month();
            let day = dt.day();
            let weekday = dt.weekday().to_string();
            let hour = dt.hour();
            let minute = dt.minute();
            let second = dt.second();
            let iso_week = dt.iso_week().week();
            let quarter = ((month - 1) / 3) + 1;
            let day_of_year = dt.ordinal();

            *freq_year.entry(year).or_insert(0) += 1;
            *freq_month.entry(month).or_insert(0) += 1;
            *freq_day.entry(day).or_insert(0) += 1;
            *freq_weekday.entry(weekday.clone()).or_insert(0) += 1;
            *freq_hour.entry(hour).or_insert(0) += 1;

            *freq_minute.entry(minute).or_insert(0) += 1;
            *freq_second.entry(second).or_insert(0) += 1;
            *freq_weeknum.entry(iso_week).or_insert(0) += 1;
            *freq_quarter.entry(quarter).or_insert(0) += 1;
            *freq_day_of_year.entry(day_of_year).or_insert(0) += 1;

            let bucket = timestamp / 3600;
            *freq_epoch_bucket.entry(bucket).or_insert(0) += 1;

            let ampm = if hour < 12 { "AM" } else { "PM" };
            *freq_ampm.entry(ampm).or_insert(0) += 1;

            let is_weekend = matches!(dt.weekday(), Weekday::Sat | Weekday::Sun);
            *freq_weekend
              .entry(if is_weekend { "Weekend" } else { "Weekday" })
              .or_insert(0) += 1;

            let date_key = dt.format("%Y-%m-%d").to_string();
            *timeline.entry(date_key).or_insert(0) += 1;
          }
        }

        if !avatar_name.is_empty() {
            *avatar_counts.entry(avatar_name.clone()).or_insert(0) += 1;
        }
        if !sim_name.is_empty() {
            *sim_counts.entry(sim_name.clone()).or_insert(0) += 1;
        }

        let msg_lower = message.to_lowercase();
        let tokens: Vec<String> = msg_lower
            .replace(|c: char| !c.is_alphanumeric(), " ")
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let mut msg_hostile: i64 = 0;
        let mut msg_positive: i64 = 0;
        let mut msg_drug: i64 = 0;
        let mut msg_slang: i64 = 0;
        let mut hostile_terms: BTreeSet<String> = BTreeSet::new();
        let mut has_slang = false;

        for w in &tokens {
            *word_counts.entry(w.clone()).or_insert(0) += 1;

            if let Some(h) = hostile_words.get(w.as_str()) {
                msg_hostile += *h as i64;
              hostile_terms.insert(w.clone());
            }
            if let Some(p) = positive_words.get(w.as_str()) {
                msg_positive += *p as i64;
            }
            if let Some(d) = drug_words.get(w.as_str()) {
                msg_drug += *d as i64;
            }
            if let Some(s) = slang_words.get(w.as_str()) {
                msg_slang += *s as i64;
              has_slang = true;
            }
        }

        let msg_total = msg_hostile + msg_drug + msg_slang - msg_positive;

        if timestamp_valid {
          if let LocalResult::Single(dt) = Los_Angeles.timestamp_opt(timestamp, 0) {
            let date_key = dt.format("%Y-%m-%d").to_string();
            *sentiment_timeline.entry(date_key).or_insert(0) += msg_positive - msg_hostile;
          }
        }

        if !avatar_name.is_empty() {
            *avatar_hostility.entry(avatar_name.clone()).or_insert(0) += msg_hostile;
            let entry = avatar_scores.entry(avatar_name.clone()).or_insert((0, 0, 0, 0, 0));
            entry.0 += msg_hostile;
            entry.1 += msg_positive;
            entry.2 += msg_drug;
            entry.3 += msg_slang;
            entry.4 += msg_total;
        }
        if !sim_name.is_empty() {
            *sim_hostility.entry(sim_name.clone()).or_insert(0) += msg_hostile + msg_drug;
        }

        for w in &tokens {
            if w.len() >= 4 {
                *topic_buckets.entry(w.clone()).or_insert(0) += 1;
            }
        }

        let captured_by_key = if captured_by.trim().is_empty() {
          "(missing)".to_string()
        } else {
          captured_by.clone()
        };
        *captured_by_counts.entry(captured_by_key.clone()).or_insert(0) += 1;
        *captured_by_sentiment.entry(captured_by_key).or_insert(0) += msg_positive - msg_hostile;

        if !captured_by.is_empty() {
            if !sim_name.is_empty() {
                *captured_by_sim_counts
                    .entry((captured_by.clone(), sim_name.clone()))
                    .or_insert(0) += 1;
            }
            let entry = captured_by_topics
                .entry(captured_by.clone())
                .or_insert(BTreeMap::new());
            for w in &tokens {
                if w.len() >= 4 {
                    *entry.entry(w.clone()).or_insert(0) += 1;
                }
            }
        }

        let hard_flags = [
          "overdose", "kill", "suicide", "self harm", "self-harm",
          "kill myself", "want to die", "end my life", "hurt myself",
          "assault", "rape", "shooting", "bomb threat", "hostage",
        ];
        let mut hard_hit = false;
        for w in &hard_flags {
            if msg_lower.contains(w) {
                hard_hit = true;
                break;
            }
        }

        let flagged = hard_hit
            || msg_hostile >= 15
            || msg_drug >= 5
            || (msg_slang >= 8 && msg_hostile >= 8)
            || msg_total >= 20;

        let quarantined = flagged;
        let mut tags = Vec::new();
        if hard_hit {
          tags.push("urgent".to_string());
        }
        if msg_hostile >= 7 {
          tags.push("high-hostility".to_string());
        } else if msg_hostile > 0 {
          tags.push("hostile".to_string());
        }
        if msg_drug > 0 {
          tags.push("substance".to_string());
        }
        if has_slang {
          tags.push("slang".to_string());
        }
        if msg_positive > msg_hostile {
          tags.push("positive".to_string());
        }
        if tags.is_empty() {
          tags.push("neutral".to_string());
        }

        if position_valid {
          heat3d_points.push((x, y, z));
        }

        let entry = MsgEntry {
            avatar_id,
            avatar_name,
            sim_name,
            message,
            timestamp,
            timestamp_valid,
            x,
            y,
            z,
            position_valid,
            captured_by,
            hostility_score: msg_hostile,
            positive_score: msg_positive,
            drug_score: msg_drug,
            slang_score: msg_slang,
            tags,
            hostile_terms: hostile_terms.into_iter().collect(),
            quarantined,
        };

        if quarantined {
            quarantine.push(entry.clone());
        }

        messages_vec.push(entry);
    }

      if recent_only {
        let mut recent_messages: Vec<&MsgEntry> = messages_vec.iter().collect();
        recent_messages.sort_by(|left, right| {
          right
            .timestamp_valid
            .cmp(&left.timestamp_valid)
            .then_with(|| right.timestamp.cmp(&left.timestamp))
        });
        recent_messages.truncate(50);

        let messages: Vec<Value> = recent_messages
          .into_iter()
          .map(|message| {
            let timestamp_label = if !message.timestamp_valid {
              "Invalid timestamp".to_string()
            } else if let LocalResult::Single(dt) = Los_Angeles.timestamp_opt(message.timestamp, 0) {
              dt.format("%Y-%m-%d %H:%M:%S").to_string()
            } else {
              message.timestamp.to_string()
            };

            serde_json::json!({
              "avatar_id": &message.avatar_id,
              "avatar_name": &message.avatar_name,
              "sim_name": &message.sim_name,
              "message": &message.message,
              "timestamp": message.timestamp,
              "timestamp_label": timestamp_label,
              "hostility_score": message.hostility_score,
              "positive_score": message.positive_score,
              "drug_score": message.drug_score,
              "slang_score": message.slang_score,
              "tags": &message.tags,
              "hostile_terms": &message.hostile_terms,
              "quarantined": message.quarantined,
            })
          })
          .collect();
        let payload = serde_json::json!({
          "refreshed_at": chrono::Utc::now().to_rfc3339(),
          "messages": messages,
        });
        let body = serde_json::to_string(&payload).map_err(|error| {
          tide::Error::from_str(StatusCode::InternalServerError, error.to_string())
        })?;

        chatlog_cache_store(
          req.state().chatlog_cache.as_ref(),
          source_len,
          &source_modified,
          "recent",
          body.clone(),
        );
        let mut res = Response::new(StatusCode::Ok);
        res.set_body(body);
        res.insert_header("Content-Type", "application/json; charset=utf-8");
        return Ok(res);
      }

  fn append_rate_samples(
    timestamp_counts: &BTreeMap<i64, usize>,
    samples: &mut Vec<(i64, f64)>,
  ) {
    let mut previous_timestamp = None;

    for (&timestamp, &count) in timestamp_counts {
      if let Some(previous_timestamp) = previous_timestamp {
        let elapsed_seconds = timestamp - previous_timestamp;
        if elapsed_seconds > 0 {
          samples.push((timestamp, count as f64 / elapsed_seconds as f64));
        }
      }
      previous_timestamp = Some(timestamp);
    }
    }

    {
    let mut global_timestamp_counts: BTreeMap<i64, usize> = BTreeMap::new();
    let mut sim_timestamp_counts: BTreeMap<String, BTreeMap<i64, usize>> = BTreeMap::new();

    for message in &messages_vec {
      if !message.timestamp_valid {
        continue;
      }
      *global_timestamp_counts.entry(message.timestamp).or_insert(0) += 1;
      if !message.sim_name.is_empty() {
        *sim_timestamp_counts
          .entry(message.sim_name.clone())
          .or_default()
          .entry(message.timestamp)
          .or_insert(0) += 1;
      }
    }

    append_rate_samples(&global_timestamp_counts, &mut rate_of_change);
    for (sim, timestamp_counts) in sim_timestamp_counts {
      let mut sim_samples = Vec::new();
      append_rate_samples(&timestamp_counts, &mut sim_samples);
      roc_per_sim.insert(sim, sim_samples);
        }
    }

    {
        let mut by_avatar: HashMap<String, Vec<&MsgEntry>> = HashMap::new();
        for m in &messages_vec {
        if m.timestamp_valid && !m.avatar_name.is_empty() && !m.sim_name.is_empty() {
        by_avatar.entry(m.avatar_name.clone()).or_default().push(m);
      }
        }

    for messages in by_avatar.values_mut() {
      messages.sort_by_key(|message| message.timestamp);
      for pair in messages.windows(2) {
        let previous = pair[0];
        let current = pair[1];
        if previous.sim_name != current.sim_name {
          *sim_transitions
            .entry((previous.sim_name.clone(), current.sim_name.clone()))
            .or_insert(0) += 1;
        }
      }
    }

    let mut by_sim: HashMap<String, Vec<&MsgEntry>> = HashMap::new();
    for m in &messages_vec {
      if m.timestamp_valid && !m.avatar_name.is_empty() && !m.sim_name.is_empty() {
        by_sim.entry(m.sim_name.clone()).or_default().push(m);
      }
    }

    for messages in by_sim.values_mut() {
      messages.sort_by_key(|message| message.timestamp);
      for pair in messages.windows(2) {
        let previous = pair[0];
        let current = pair[1];
        if previous.avatar_name != current.avatar_name {
          *avatar_interactions
            .entry((previous.avatar_name.clone(), current.avatar_name.clone()))
            .or_insert(0) += 1;
        }
            }
        }
    }

    // Markov chain style transition probabilities for sims and avatars
    let mut sim_transition_prob: BTreeMap<(String, String), f64> = BTreeMap::new();
    let mut sim_outgoing_totals: BTreeMap<String, usize> = BTreeMap::new();
    for ((from, _to), count) in &sim_transitions {
        *sim_outgoing_totals.entry(from.clone()).or_insert(0) += *count;
    }
    for ((from, to), count) in &sim_transitions {
        if let Some(total) = sim_outgoing_totals.get(from) {
            if *total > 0 {
                sim_transition_prob.insert((from.clone(), to.clone()), *count as f64 / *total as f64);
            }
        }
    }

    let mut avatar_transition_prob: BTreeMap<(String, String), f64> = BTreeMap::new();
    let mut avatar_outgoing_totals: BTreeMap<String, usize> = BTreeMap::new();
    for ((from, _to), count) in &avatar_interactions {
        *avatar_outgoing_totals.entry(from.clone()).or_insert(0) += *count;
    }
    for ((from, to), count) in &avatar_interactions {
        if let Some(total) = avatar_outgoing_totals.get(from) {
            if *total > 0 {
                avatar_transition_prob.insert((from.clone(), to.clone()), *count as f64 / *total as f64);
            }
        }
    }

    // captured_by share of total messages (probability-like)
    let total_msgs = messages_vec.len() as f64;
    let mut captured_by_share: BTreeMap<String, f64> = BTreeMap::new();
    for (capturer, count) in &captured_by_counts {
        if total_msgs > 0.0 {
            captured_by_share.insert(capturer.clone(), *count as f64 / total_msgs);
        }
    }

    let total_messages = messages_vec.len();
    let valid_timestamps = messages_vec.iter().filter(|message| message.timestamp_valid).count();
    let valid_positions = messages_vec.iter().filter(|message| message.position_valid).count();
    let quarantined_messages = messages_vec.iter().filter(|message| message.quarantined).count();
    let total_message_characters: usize = messages_vec
      .iter()
      .map(|message| message.message.chars().count())
      .sum();
    let average_message_length = if total_messages == 0 {
      0.0
    } else {
      total_message_characters as f64 / total_messages as f64
    };
    let mut message_lengths: Vec<usize> = messages_vec
      .iter()
      .map(|message| message.message.chars().count())
      .collect();
    message_lengths.sort_unstable();
    let median_message_length = match message_lengths.len() {
      0 => 0.0,
      len if len % 2 == 1 => message_lengths[len / 2] as f64,
      len => (message_lengths[len / 2 - 1] + message_lengths[len / 2]) as f64 / 2.0,
    };
    let total_sentiment: i64 = messages_vec
      .iter()
      .map(|message| message.positive_score - message.hostility_score)
      .sum();
    let mean_sentiment = if total_messages == 0 {
      0.0
    } else {
      total_sentiment as f64 / total_messages as f64
    };
    let unique_event_ratio = if integrity.parsed_objects == 0 {
      0.0
    } else {
      unique_keys.len() as f64 / integrity.parsed_objects as f64
    };
    let peak_day = timeline
      .iter()
      .max_by_key(|(_, count)| *count)
      .map(|(date, count)| (date.clone(), *count));
    let (peak_day_label, peak_day_count) = peak_day
      .clone()
      .unwrap_or_else(|| ("N/A".to_string(), 0));
    let first_timestamp = freq_timestamp.keys().next().copied();
    let last_timestamp = freq_timestamp.keys().next_back().copied();
    let observed_timespan_days = match (first_timestamp, last_timestamp) {
      (Some(first), Some(last)) if last >= first => (last - first) as f64 / 86_400.0,
      _ => 0.0,
    };
    let active_days = timeline.len();
    let observed_hours = freq_epoch_bucket.len();
    let messages_per_active_day = if active_days == 0 {
      0.0
    } else {
      total_messages as f64 / active_days as f64
    };
    let messages_per_observed_hour = if observed_hours == 0 {
      0.0
    } else {
      valid_timestamps as f64 / observed_hours as f64
    };
    let timestamp_collision_events = valid_timestamps.saturating_sub(freq_timestamp.len());
    let timestamp_collision_probability = if valid_timestamps == 0 {
      0.0
    } else {
      timestamp_collision_events as f64 / valid_timestamps as f64
    };
    let peak_hour = freq_hour
      .iter()
      .max_by_key(|(_, count)| *count)
      .map(|(hour, count)| (*hour, *count));
    let (peak_hour_label, peak_hour_count) = peak_hour
      .map(|(hour, count)| (format!("{:02}:00", hour), count))
      .unwrap_or_else(|| ("N/A".to_string(), 0));
    let peak_hour_probability = if valid_timestamps == 0 {
      0.0
    } else {
      peak_hour_count as f64 / valid_timestamps as f64
    };
    let peak_weekday = freq_weekday
      .iter()
      .max_by_key(|(_, count)| *count)
      .map(|(weekday, count)| (weekday.clone(), *count))
      .unwrap_or_else(|| ("N/A".to_string(), 0));
    let peak_weekday_probability = if valid_timestamps == 0 {
      0.0
    } else {
      peak_weekday.1 as f64 / valid_timestamps as f64
    };
    let weekend_probability = if valid_timestamps == 0 {
      0.0
    } else {
      *freq_weekend.get("Weekend").unwrap_or(&0) as f64 / valid_timestamps as f64
    };
    let pm_probability = if valid_timestamps == 0 {
      0.0
    } else {
      *freq_ampm.get("PM").unwrap_or(&0) as f64 / valid_timestamps as f64
    };
    let hourly_entropy_bits = freq_hour
      .values()
      .filter(|count| **count > 0 && valid_timestamps > 0)
      .map(|count| {
        let probability = *count as f64 / valid_timestamps as f64;
        -probability * probability.log2()
      })
      .sum::<f64>();
    let burstiest_hour_bucket = freq_epoch_bucket
      .iter()
      .max_by_key(|(_, count)| *count)
      .map(|(bucket, count)| (*bucket, *count));
    let (burstiest_hour_label, burstiest_hour_count) = burstiest_hour_bucket
      .and_then(|(bucket, count)| {
        Los_Angeles
          .timestamp_opt(bucket * 3600, 0)
          .single()
          .map(|dt| (dt.format("%Y-%m-%d %H:00").to_string(), count))
      })
      .unwrap_or_else(|| ("N/A".to_string(), 0));

    let mut captured_by_frequency: Vec<(String, usize, f64, i64, f64)> = captured_by_counts
      .iter()
      .map(|(capturer, count)| {
        let total_sentiment = *captured_by_sentiment.get(capturer).unwrap_or(&0);
        let share = *captured_by_share.get(capturer).unwrap_or(&0.0);
        let average_sentiment = if *count == 0 {
          0.0
        } else {
          total_sentiment as f64 / *count as f64
        };
        (
          capturer.clone(),
          *count,
          share,
          total_sentiment,
          average_sentiment,
        )
      })
      .collect();
    captured_by_frequency.sort_by(|left, right| {
      right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0))
    });
    let capture_entropy_bits = captured_by_frequency
      .iter()
      .filter(|(_, _, share, _, _)| *share > 0.0)
      .map(|(_, _, share, _, _)| -share * share.log2())
      .sum::<f64>();
    let (top_capturer, top_capturer_probability) = captured_by_frequency
      .first()
      .map(|(name, _, share, _, _)| (name.clone(), *share))
      .unwrap_or_else(|| ("N/A".to_string(), 0.0));
    let parse_success_rate = if integrity.candidate_objects == 0 {
      0.0
    } else {
      integrity.parsed_objects as f64 / integrity.candidate_objects as f64
    };
    let timestamp_coverage = if total_messages == 0 { 0.0 } else { valid_timestamps as f64 / total_messages as f64 };
    let position_coverage = if total_messages == 0 { 0.0 } else { valid_positions as f64 / total_messages as f64 };
    let quarantine_probability = if total_messages == 0 { 0.0 } else { quarantined_messages as f64 / total_messages as f64 };
    let hostile_messages = messages_vec.iter().filter(|message| message.hostility_score > 0).count();
    let positive_messages = messages_vec.iter().filter(|message| message.positive_score > message.hostility_score).count();
    let hostile_probability = if total_messages == 0 { 0.0 } else { hostile_messages as f64 / total_messages as f64 };
    let positive_probability = if total_messages == 0 { 0.0 } else { positive_messages as f64 / total_messages as f64 };
    let average_hostility = if total_messages == 0 {
      0.0
    } else {
      messages_vec.iter().map(|message| message.hostility_score).sum::<i64>() as f64 / total_messages as f64
    };
    let data_quality_score = ((parse_success_rate + timestamp_coverage + position_coverage + unique_event_ratio.min(1.0)) / 4.0) * 100.0;
    let capture_hhi = captured_by_frequency
      .iter()
      .map(|(_, _, share, _, _)| share * share)
      .sum::<f64>();
    let top_avatar = avatar_counts
      .iter()
      .max_by_key(|(_, count)| *count)
      .map(|(name, count)| (name.clone(), *count))
      .unwrap_or_else(|| ("N/A".to_string(), 0));
    let top_sim = sim_counts
      .iter()
      .max_by_key(|(_, count)| *count)
      .map(|(name, count)| (name.clone(), *count))
      .unwrap_or_else(|| ("N/A".to_string(), 0));
    let top_avatar_probability = if total_messages == 0 { 0.0 } else { top_avatar.1 as f64 / total_messages as f64 };
    let top_sim_probability = if total_messages == 0 { 0.0 } else { top_sim.1 as f64 / total_messages as f64 };
    let operational_risk_score = ((quarantine_probability * 45.0)
      + (hostile_probability * 25.0)
      + ((1.0 - data_quality_score / 100.0).max(0.0) * 20.0)
      + (top_capturer_probability * 10.0))
      .min(100.0);
    let operational_health = if operational_risk_score >= 55.0 {
      "Intervention recommended"
    } else if operational_risk_score >= 25.0 {
      "Monitor closely"
    } else {
      "Stable"
    };
    let logger_redundancy = captured_by_frequency.len();
    let messages_per_logger = if logger_redundancy == 0 {
      0.0
    } else {
      total_messages as f64 / logger_redundancy as f64
    };
    let invalid_record_count = integrity.parse_errors
      + integrity.missing_avatar_name
      + integrity.missing_sim_name
      + integrity.missing_message
      + integrity.bad_timestamp
      + integrity.bad_position;
    let moderation_action = if quarantined_messages > 0 {
      format!("Review {} flagged chat events", quarantined_messages)
    } else if hostile_messages > 0 {
      format!("Spot-check {} hostile chat events", hostile_messages)
    } else {
      "No moderation queue backlog".to_string()
    };
    let logger_action = if data_quality_score < 80.0 {
      "Audit in-world logger coverage"
    } else if top_capturer_probability > 0.65 {
      "Add a redundant capture source"
    } else {
      "Logger coverage acceptable"
    };
    let top_sim_transition = sim_transitions
      .iter()
      .max_by_key(|(_, count)| *count)
      .map(|((from, to), count)| {
        let probability = *sim_transition_prob.get(&(from.clone(), to.clone())).unwrap_or(&0.0);
        (format!("{} -> {}", from, to), *count, probability)
      })
      .unwrap_or_else(|| ("N/A".to_string(), 0, 0.0));
    let top_avatar_interaction = avatar_interactions
      .iter()
      .max_by_key(|(_, count)| *count)
      .map(|((from, to), count)| {
        let probability = *avatar_transition_prob.get(&(from.clone(), to.clone())).unwrap_or(&0.0);
        (format!("{} -> {}", from, to), *count, probability)
      })
      .unwrap_or_else(|| ("N/A".to_string(), 0, 0.0));
    let region_action = if top_sim_probability > 0.45 {
      format!("Watch region concentration in {}", top_sim.0)
    } else if top_sim_transition.1 > 0 {
      format!("Monitor travel corridor {}", top_sim_transition.0)
    } else {
      "No dominant region hotspot".to_string()
    };
    let mut admin_recommendations = Vec::new();
    if data_quality_score < 80.0 {
      admin_recommendations.push("Improve in-world logger coverage: timestamp, position, and parse health are reducing administrative confidence.".to_string());
    }
    if operational_risk_score >= 25.0 {
      admin_recommendations.push("Review the moderation queue before making estate staffing, ban, or escalation decisions.".to_string());
    }
    if capture_hhi > 0.45 {
      admin_recommendations.push("Capture-source concentration is high; add another in-world logger or audit the top capture source.".to_string());
    }
    if top_sim_transition.1 > 0 {
      admin_recommendations.push(format!("Watch the strongest region movement path: {} at {:.1}% transition probability.", top_sim_transition.0, top_sim_transition.2 * 100.0));
    }
    if active_days < 7 && total_messages > 0 {
      admin_recommendations.push("Treat weekly trend conclusions as provisional until at least seven active days are present.".to_string());
    }
    if admin_recommendations.is_empty() {
      admin_recommendations.push("No immediate administrative action detected from current chatlog metrics.".to_string());
    }

    fn json_for_script<T: Serialize>(value: &T) -> String {
      serde_json::to_string(value)
        .unwrap_or_else(|_| "[]".to_string())
        .replace("</", "<\\/")
    }

    let timeline_data: Vec<Value> = timeline
      .iter()
      .map(|(date, count)| serde_json::json!({ "date": date, "count": count }))
      .collect();
    let sentiment_data: Vec<Value> = sentiment_timeline
      .iter()
      .map(|(date, score)| serde_json::json!({ "date": date, "score": score }))
      .collect();
    let roc_data: Vec<Value> = rate_of_change
      .iter()
      .map(|(timestamp, rate)| serde_json::json!({ "ts": timestamp, "roc": rate }))
      .collect();
    let heat3d_data: Vec<Value> = heat3d_points
      .iter()
      .map(|(x, y, z)| serde_json::json!({ "x": x, "y": y, "z": z }))
      .collect();
    let dashboard_data_json = json_for_script(&serde_json::json!({
        "timeline": timeline_data,
        "sentiment": sentiment_data,
        "rate_of_change": roc_data,
        "heatmap": heat3d_data,
    }));

    if requested_format.as_deref() == Some("summary") {
      let captured_by_data: Vec<Value> = captured_by_frequency
        .iter()
        .map(|(capturer, count, share, sentiment_total, sentiment_average)| {
          serde_json::json!({
            "captured_by": capturer,
            "count": count,
            "probability": share,
            "sentiment_total": sentiment_total,
            "sentiment_average": sentiment_average,
          })
        })
        .collect();
      let payload = serde_json::json!({
        "refreshed_at": chrono::Utc::now().to_rfc3339(),
        "source": {
          "bytes": source_len,
          "modified": source_modified,
        },
        "integrity": &integrity,
        "statistics": {
          "total_messages": total_messages,
          "valid_timestamps": valid_timestamps,
          "valid_positions": valid_positions,
          "quarantined_messages": quarantined_messages,
          "unique_event_ratio": unique_event_ratio,
          "average_message_length": average_message_length,
          "median_message_length": median_message_length,
          "mean_sentiment": mean_sentiment,
          "capture_entropy_bits": capture_entropy_bits,
          "top_capturer": top_capturer,
          "top_capturer_probability": top_capturer_probability,
          "peak_day": peak_day,
          "observed_timespan_days": observed_timespan_days,
          "active_days": active_days,
          "observed_hours": observed_hours,
          "messages_per_active_day": messages_per_active_day,
          "messages_per_observed_hour": messages_per_observed_hour,
          "timestamp_collision_events": timestamp_collision_events,
          "timestamp_collision_probability": timestamp_collision_probability,
          "peak_hour": { "label": peak_hour_label, "count": peak_hour_count, "probability": peak_hour_probability },
          "peak_weekday": { "label": peak_weekday.0, "count": peak_weekday.1, "probability": peak_weekday_probability },
          "weekend_probability": weekend_probability,
          "pm_probability": pm_probability,
          "hourly_entropy_bits": hourly_entropy_bits,
          "burstiest_hour": { "label": burstiest_hour_label, "count": burstiest_hour_count },
          "data_quality_score": data_quality_score,
          "operational_risk_score": operational_risk_score,
          "operational_health": operational_health,
          "hostile_probability": hostile_probability,
          "positive_probability": positive_probability,
          "average_hostility": average_hostility,
          "capture_concentration_hhi": capture_hhi,
          "top_avatar": { "name": top_avatar.0.clone(), "count": top_avatar.1, "probability": top_avatar_probability },
          "top_sim": { "name": top_sim.0.clone(), "count": top_sim.1, "probability": top_sim_probability },
          "top_sim_transition": { "path": top_sim_transition.0.clone(), "count": top_sim_transition.1, "probability": top_sim_transition.2 },
          "top_avatar_interaction": { "path": top_avatar_interaction.0.clone(), "count": top_avatar_interaction.1, "probability": top_avatar_interaction.2 },
          "sim_transition_count": sim_transitions.len(),
          "avatar_interaction_count": avatar_interactions.len(),
          "logger_redundancy": logger_redundancy,
          "messages_per_logger": messages_per_logger,
          "invalid_record_count": invalid_record_count,
          "moderation_action": moderation_action,
          "logger_action": logger_action,
          "region_action": region_action,
          "markov_message_count": markov_messages.len(),
          "markov_state_count": markov_state_count,
          "markov_transition_count": markov_transition_count,
          "markov_readiness": markov_readiness,
          "markov_seed_suggestions": markov_seed_suggestions.clone(),
          "admin_recommendations": admin_recommendations.clone(),
        },
        "timeline": &timeline_data,
        "sentiment": &sentiment_data,
        "captured_by_frequency": captured_by_data,
      });
      let body = serde_json::to_string(&payload).map_err(|error| {
        tide::Error::from_str(StatusCode::InternalServerError, error.to_string())
      })?;
      let mut res = Response::new(StatusCode::Ok);
      res.set_body(body);
      res.insert_header("Content-Type", "application/json; charset=utf-8");
      return Ok(res);
    }

    let mut html = String::new();
    html.push_str(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Second Life Administrator Chatlog Console</title>
<style>
body { font-family: system-ui, sans-serif; background: #0b0c10; color: #c5c6c7; margin: 0; padding: 0; }
header { padding: 16px 24px; background: #1f2833; border-bottom: 1px solid #45a29e; }
h1 { margin: 0; font-size: 20px; color: #66fcf1; }
main { padding: 16px 24px; display: grid; grid-template-columns: minmax(0, 2fr) minmax(0, 1fr); gap: 16px; }
section { min-width: 0; background: #1f2833; border-radius: 8px; padding: 12px 16px; border: 1px solid #45a29e22; }
section h2 { margin-top: 0; font-size: 16px; color: #66fcf1; }
table { width: 100%; border-collapse: collapse; font-size: 12px; }
th, td { padding: 4px 6px; border-bottom: 1px solid #45a29e22; text-align: left; }
th { color: #c5c6c7; }
.bad { color: #ff6b6b; }
.good { color: #4cd137; }
.small { font-size: 11px; opacity: 0.8; }
.tag { display: inline-block; padding: 2px 6px; border-radius: 999px; background: #45a29e33; margin: 0 4px 4px 0; }
.quarantine { background: #ff6b6b22; }
.chart { height: 160px; background: #0b0c10; border-radius: 6px; border: 1px solid #45a29e33; position: relative; overflow: hidden; }
.chart canvas { width: 100%; height: 100%; }
#heat3dCanvas { background: #000; touch-action: none; }
.section-heading { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
.chart-range-label { display: flex; align-items: center; gap: 8px; color: #c5c6c7; font-size: 12px; }
.chart-range-label select { border: 1px solid #45a29e66; border-radius: 4px; color: #c5c6c7; background: #0b0c10; padding: 5px 7px; font: inherit; }
.recent-messages { display: grid; gap: 8px; max-height: 430px; overflow-y: auto; }
.recent-message { padding: 10px; border-left: 3px solid #45a29e; background: #0b0c10; }
.recent-message.is-quarantined { border-left-color: #ff6b6b; }
.recent-meta { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; color: #c5c6c7; font-size: 11px; }
.recent-message-text { margin-top: 6px; color: #f4f6f7; line-height: 1.45; overflow-wrap: anywhere; }
.message-tag { display: inline-block; padding: 2px 5px; border-radius: 4px; font-size: 10px; font-weight: 700; text-transform: uppercase; }
.tag-neutral { background: #45a29e22; color: #c5c6c7; }
.tag-positive { background: #4cd13722; color: #8bea9b; }
.tag-slang { background: #f4d35e22; color: #f4d35e; }
.tag-substance { background: #b58cff22; color: #d0b6ff; }
.tag-hostile, .tag-high-hostility, .tag-urgent { background: #ff6b6b22; color: #ff8b8b; }
.quick-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 10px; }
.quick-actions a { color: #66fcf1; border: 1px solid #45a29e66; border-radius: 999px; padding: 5px 10px; text-decoration: none; font-size: 12px; }
.kpi-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(130px, 1fr)); gap: 10px; }
.kpi { background: #0b0c10; border: 1px solid #45a29e33; border-radius: 7px; padding: 10px; }
.kpi span { display: block; color: #c5c6c7; font-size: 11px; }
.kpi strong { display: block; margin-top: 4px; color: #66fcf1; font-size: 20px; }
.recommendations { margin: 0; padding-left: 18px; line-height: 1.5; }
@media (max-width: 900px) { header, main { padding-left: 12px; padding-right: 12px; } main { grid-template-columns: minmax(0, 1fr); } .chart { height: 190px; } }
</style>
</head>
<body>
<header id="chatlogHeader">
  <h1>Second Life Administrator Chatlog Console</h1>
  <div class="small">In-memory partitioned log store &mdash; Parsed objects: "#);

    html.push_str(&format!(
        "{} &mdash; Unique keys: {}",
        integrity.parsed_objects,
        unique_keys.len()
    ));
    html.push_str(r#"</div>
  <nav class="quick-actions" aria-label="Chatlog data routes">
    <a href="/chatlog">Dashboard</a>
    <a href="/chatlog/summary">Admin JSON</a>
    <a href="/chatlog/recent">Recent JSON</a>
    <a href="/chatlog/admin">Admin Alias</a>
    <a href="/chatlog/markov">Markov Chat</a>
    <a href="/chatlog/markov.json">Markov JSON</a>
    <a href="/chatlog/markov/transitions.json">Markov Transitions</a>
    <a href="/chatlog/words">Custom Words</a>
    <a href="/analytics">Text Analytics</a>
    <a href="/forth/ui">Forth Console</a>
    <a href="/ruby">RubyForth API</a>
    <a href="/forth">Forth Catalog</a>
    <a href="/forth/example">Forth Example</a>
    <a href="/program/example">Program Example</a>
    <a href="/sigil-deck">Sigil Deck</a>
    <a href="/flashcard">Flashcards</a>
    <a href="/time">Time</a>
    <a href="/weather">Weather</a>
    <a href="/ae">AE Calendar</a>
    <a href="/tiade/moon">Moon</a>
    <a href="/tiade/sun">Sun</a>
    <a href="/rneutrialg">RNeutri</a>
    <a href="/random">Random</a>
    <a href="/random2">Random Ratio</a>
    <a href="/avatarencounter">Avatar Encounter</a>
    <a href="/">Home</a>
  </nav>
</header>
<main id="chatlogDashboard">
<section>
  <h2>Estate Operations KPIs</h2>
  <div class="kpi-grid">"#);

    html.push_str(&format!(
      r#"<div class="kpi"><span>Operational Health</span><strong>{}</strong></div>
<div class="kpi"><span>Risk Score</span><strong>{:.1}</strong></div>
<div class="kpi"><span>Data Quality</span><strong>{:.1}%</strong></div>
<div class="kpi"><span>Quarantine Probability</span><strong>{:.1}%</strong></div>
<div class="kpi"><span>Hostile Message Probability</span><strong>{:.1}%</strong></div>
<div class="kpi"><span>Positive Message Probability</span><strong>{:.1}%</strong></div>
<div class="kpi"><span>Logger Sources</span><strong>{}</strong></div>
<div class="kpi"><span>Invalid Record Signals</span><strong>{}</strong></div>"#,
      escape_html(operational_health),
      operational_risk_score,
      data_quality_score,
      quarantine_probability * 100.0,
      hostile_probability * 100.0,
      positive_probability * 100.0,
      logger_redundancy,
      invalid_record_count,
    ));

    html.push_str(r#"</div>
</section>
<section>
  <h2>Estate Manager Brief</h2>
  <table>
    <tr><th>Action Lane</th><th>Recommended Admin Action</th></tr>"#);

    html.push_str(&format!(
      r#"<tr><td>Moderation</td><td>{}</td></tr>
<tr><td>Logger Operations</td><td>{}</td></tr>
<tr><td>Region Operations</td><td>{}</td></tr>"#,
      escape_html(&moderation_action),
      escape_html(logger_action),
      escape_html(&region_action),
    ));

    html.push_str(r#"</table>
  <h2>Administrator Recommendations</h2>
  <ul class="recommendations">"#);

    for recommendation in &admin_recommendations {
      html.push_str(&format!("<li>{}</li>", escape_html(recommendation)));
    }

    html.push_str(r#"</ul>
</section>
<section>
  <h2>Resident, Region, and Logger Concentration</h2>
  <table>
    <tr><th>Signal</th><th>Value</th></tr>"#);

    html.push_str(&format!(
      r#"<tr><td>Top resident share</td><td>{} ({:.1}%)</td></tr>
<tr><td>Top region share</td><td>{} ({:.1}%)</td></tr>
<tr><td>Top logger share</td><td>{} ({:.1}%)</td></tr>
<tr><td>Capture-source HHI concentration</td><td>{:.3}</td></tr>
<tr><td>Average hostility score / message</td><td>{:.3}</td></tr>
<tr><td>Messages per logger source</td><td>{:.2}</td></tr>"#,
      escape_html(&top_avatar.0),
      top_avatar_probability * 100.0,
      escape_html(&top_sim.0),
      top_sim_probability * 100.0,
      escape_html(&top_capturer),
      top_capturer_probability * 100.0,
      capture_hhi,
      average_hostility,
      messages_per_logger,
    ));

    html.push_str(r#"</table>
</section>
<section>
  <h2>Resident Movement Markov Model</h2>
  <table>
    <tr><th>Signal</th><th>Value</th></tr>"#);

    html.push_str(&format!(
      r#"<tr><td>Region transition states</td><td>{}</td></tr>
<tr><td>Resident interaction states</td><td>{}</td></tr>
<tr><td>Strongest region path</td><td>{} ({}, {:.1}%)</td></tr>
<tr><td>Strongest resident interaction</td><td>{} ({}, {:.1}%)</td></tr>"#,
      sim_transitions.len(),
      avatar_interactions.len(),
      escape_html(&top_sim_transition.0),
      top_sim_transition.1,
      top_sim_transition.2 * 100.0,
      escape_html(&top_avatar_interaction.0),
      top_avatar_interaction.1,
      top_avatar_interaction.2 * 100.0,
    ));

    html.push_str(r#"</table>
</section>
<section>
  <h2>Message Chatter Markov Model</h2>
  <table>
    <tr><th>Signal</th><th>Value</th></tr>"#);

    html.push_str(&format!(
      r#"<tr><td>Model readiness</td><td>{}</td></tr>
<tr><td>Training messages</td><td>{}</td></tr>
<tr><td>Word transition states</td><td>{}</td></tr>
<tr><td>Total word transitions</td><td>{}</td></tr>
<tr><td>Suggested seeds</td><td>{}</td></tr>
<tr><td>Operator tools</td><td><a href="/chatlog/markov">Generate chatter</a> | <a href="/chatlog/markov/transitions.json">Transition JSON</a></td></tr>"#,
      escape_html(markov_readiness),
      markov_messages.len(),
      markov_state_count,
      markov_transition_count,
      escape_html(&markov_seed_suggestions.join(", ")),
    ));

    html.push_str(r#"</table>
</section>
<section>
  <h2>Integrity Report</h2>
  <table>
    <tr><th>Metric</th><th>Value</th></tr>"#);

    html.push_str(&format!(
        "<tr><td>Total lines</td><td>{}</td></tr>",
        integrity.total_lines
    ));
    html.push_str(&format!(
        "<tr><td>Candidate objects</td><td>{}</td></tr>",
        integrity.candidate_objects
    ));
    html.push_str(&format!(
        "<tr><td>Parsed objects (serde_json)</td><td>{}</td></tr>",
        integrity.parsed_objects
    ));
    html.push_str(&format!(
        "<tr><td>JSON parse errors</td><td class=\"bad\">{}</td></tr>",
        integrity.parse_errors
    ));
    html.push_str(&format!(
        "<tr><td>Missing avatar_name</td><td class=\"bad\">{}</td></tr>",
        integrity.missing_avatar_name
    ));
    html.push_str(&format!(
        "<tr><td>Missing sim_name</td><td class=\"bad\">{}</td></tr>",
        integrity.missing_sim_name
    ));
    html.push_str(&format!(
        "<tr><td>Missing message</td><td class=\"bad\">{}</td></tr>",
        integrity.missing_message
    ));
    html.push_str(&format!(
        "<tr><td>Bad timestamp</td><td class=\"bad\">{}</td></tr>",
        integrity.bad_timestamp
    ));
    html.push_str(&format!(
        "<tr><td>Bad position</td><td class=\"bad\">{}</td></tr>",
        integrity.bad_position
    ));
    html.push_str(r#"</table>
</section>
<section>
  <h2>Dashboard Statistics</h2>
  <table>
    <tr><th>Metric</th><th>Value</th></tr>"#);
    html.push_str(&format!(
        r#"<tr><td>Messages</td><td>{}</td></tr>
<tr><td>Timestamp coverage</td><td>{}/{} ({:.1}%)</td></tr>
<tr><td>Position coverage</td><td>{}/{} ({:.1}%)</td></tr>
<tr><td>Unique event ratio</td><td>{:.3}</td></tr>
<tr><td>Unique avatars / sims</td><td>{} / {}</td></tr>
<tr><td>Capture sources</td><td>{}</td></tr>
<tr><td>Average / median message length</td><td>{:.1} / {:.1}</td></tr>
<tr><td>Mean lexical sentiment</td><td>{:.3}</td></tr>
<tr><td>Quarantined messages</td><td>{} ({:.1}%)</td></tr>
<tr><td>Peak day</td><td>{} ({})</td></tr>
<tr><td>Observed timespan</td><td>{:.2} days across {} active days</td></tr>
<tr><td>Messages / active day</td><td>{:.2}</td></tr>
<tr><td>Messages / observed hour</td><td>{:.2}</td></tr>
<tr><td>Peak hour probability</td><td>{} ({}, {:.1}%)</td></tr>
<tr><td>Peak weekday probability</td><td>{} ({}, {:.1}%)</td></tr>
<tr><td>Weekend probability</td><td>{:.1}%</td></tr>
<tr><td>PM probability</td><td>{:.1}%</td></tr>
<tr><td>Hourly entropy</td><td>{:.3} bits</td></tr>
<tr><td>Burstiest observed hour</td><td>{} ({})</td></tr>
<tr><td>Repeated timestamp probability</td><td>{:.1}% ({} repeated events)</td></tr>
<tr><td>Top capture probability</td><td>{} ({:.1}%)</td></tr>
<tr><td>Capture-source entropy</td><td>{:.3} bits</td></tr>"#,
        total_messages,
        valid_timestamps,
        total_messages,
        if total_messages == 0 { 0.0 } else { valid_timestamps as f64 * 100.0 / total_messages as f64 },
        valid_positions,
        total_messages,
        if total_messages == 0 { 0.0 } else { valid_positions as f64 * 100.0 / total_messages as f64 },
        unique_event_ratio,
        avatar_counts.len(),
        sim_counts.len(),
        captured_by_frequency.len(),
        average_message_length,
        median_message_length,
        mean_sentiment,
        quarantined_messages,
        if total_messages == 0 { 0.0 } else { quarantined_messages as f64 * 100.0 / total_messages as f64 },
        escape_html(&peak_day_label),
        peak_day_count,
        observed_timespan_days,
        active_days,
        messages_per_active_day,
        messages_per_observed_hour,
        escape_html(&peak_hour_label),
        peak_hour_count,
        peak_hour_probability * 100.0,
        escape_html(&peak_weekday.0),
        peak_weekday.1,
        peak_weekday_probability * 100.0,
        weekend_probability * 100.0,
        pm_probability * 100.0,
        hourly_entropy_bits,
        escape_html(&burstiest_hour_label),
        burstiest_hour_count,
        timestamp_collision_probability * 100.0,
        timestamp_collision_events,
        escape_html(&top_capturer),
        top_capturer_probability * 100.0,
        capture_entropy_bits,
    ));
    html.push_str(r#"</table>
</section>
<section>
  <h2>Top Avatars</h2>
  <table>
    <tr><th>Avatar</th><th>Messages</th><th>Hostility</th></tr>"#);

    let mut top_avatars: Vec<(String, usize, i64)> = avatar_counts
        .iter()
        .map(|(name, count)| {
            let h = *avatar_hostility.get(name).unwrap_or(&0);
            (name.clone(), *count, h)
        })
        .collect();
    top_avatars.sort_by(|a, b| b.1.cmp(&a.1));
    top_avatars.truncate(20);

    for (name, count, hostility) in top_avatars {
        let cls = if hostility >= 50 { "bad" } else { "good" };
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td class=\"{}\">{}</td></tr>",
        escape_html(&name), count, cls, hostility
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Top Sims</h2>
  <table>
    <tr><th>Sim</th><th>Messages</th><th>Hostility</th></tr>"#);

    let mut top_sims: Vec<(String, usize, i64)> = sim_counts
        .iter()
        .map(|(name, count)| {
            let h = *sim_hostility.get(name).unwrap_or(&0);
            (name.clone(), *count, h)
        })
        .collect();
    top_sims.sort_by(|a, b| b.1.cmp(&a.1));
    top_sims.truncate(20);

    for (name, count, hostility) in top_sims {
        let cls = if hostility >= 80 { "bad" } else { "good" };
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td class=\"{}\">{}</td></tr>",
        escape_html(&name), count, cls, hostility
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <div class="section-heading">
    <h2>Timeline Range</h2>
    <label class="chart-range-label" for="chartRange">Range
      <select id="chartRange">
        <option value="all">All dates</option>
        <option value="365">Last year</option>
        <option value="90">Last 90 days</option>
        <option value="30">Last 30 days</option>
        <option value="7">Last 7 days</option>
      </select>
    </label>
  </div>
</section>
<section>
  <h2>Timeline (Messages per Day)</h2>
  <div class="chart">
    <canvas id="timelineChart"></canvas>
  </div>
  <div class="small">Volume of messages per day (Pacific time).</div>
</section>
<section>
  <h2>Sentiment Timeline</h2>
  <div class="chart">
    <canvas id="sentimentChart"></canvas>
  </div>
  <div class="small">Positive vs hostile language per day.</div>
</section>
<section>
  <h2>Time Probability Model</h2>
  <table>
    <tr><th>Signal</th><th>Value</th></tr>"#);

    html.push_str(&format!(
      r#"<tr><td>First valid timestamp</td><td>{}</td></tr>
<tr><td>Last valid timestamp</td><td>{}</td></tr>
<tr><td>Observed hour buckets</td><td>{}</td></tr>
<tr><td>Peak Pacific hour</td><td>{} ({:.1}% of timestamped messages)</td></tr>
<tr><td>Peak Pacific weekday</td><td>{} ({:.1}% of timestamped messages)</td></tr>
<tr><td>Weekend likelihood</td><td>{:.1}%</td></tr>
<tr><td>PM likelihood</td><td>{:.1}%</td></tr>
<tr><td>Hourly distribution entropy</td><td>{:.3} bits</td></tr>"#,
      first_timestamp
        .and_then(|timestamp| Los_Angeles.timestamp_opt(timestamp, 0).single())
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "N/A".to_string()),
      last_timestamp
        .and_then(|timestamp| Los_Angeles.timestamp_opt(timestamp, 0).single())
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "N/A".to_string()),
      observed_hours,
      escape_html(&peak_hour_label),
      peak_hour_probability * 100.0,
      escape_html(&peak_weekday.0),
      peak_weekday_probability * 100.0,
      weekend_probability * 100.0,
      pm_probability * 100.0,
      hourly_entropy_bits,
    ));

    html.push_str(r#"</table>
</section>
<section>
  <h2>Hour Probability (Pacific)</h2>
  <table>
    <tr><th>Hour</th><th>Count</th><th>Probability</th></tr>"#);

    for (hour, count) in &freq_hour {
        let probability = if valid_timestamps == 0 { 0.0 } else { *count as f64 * 100.0 / valid_timestamps as f64 };
        html.push_str(&format!(
            "<tr><td>{:02}:00</td><td>{}</td><td>{:.2}%</td></tr>",
            hour, count, probability
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Year Frequency</h2>
  <table>
    <tr><th>Year</th><th>Count</th></tr>"#);

    for (year, count) in &freq_year {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            year, count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Month Frequency</h2>
  <table>
    <tr><th>Month</th><th>Count</th></tr>"#);

    for (month, count) in &freq_month {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            month, count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Day of Month Frequency</h2>
  <table>
    <tr><th>Day</th><th>Count</th></tr>"#);

    for (day, count) in &freq_day {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            day, count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Weekday Frequency</h2>
  <table>
    <tr><th>Weekday</th><th>Count</th></tr>"#);

    for (weekday, count) in &freq_weekday {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
        escape_html(weekday), count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Hour of Day Frequency</h2>
  <table>
    <tr><th>Hour</th><th>Count</th></tr>"#);

    for (hour, count) in &freq_hour {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            hour, count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Minute Frequency</h2>
  <table>
    <tr><th>Minute</th><th>Count</th></tr>"#);

    for (minute, count) in &freq_minute {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            minute, count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Second Frequency</h2>
  <table>
    <tr><th>Second</th><th>Count</th></tr>"#);

    for (second, count) in &freq_second {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            second, count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>ISO Week Frequency</h2>
  <table>
    <tr><th>Week #</th><th>Count</th></tr>"#);

    for (week, count) in &freq_weeknum {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            week, count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Quarter Frequency</h2>
  <table>
    <tr><th>Quarter</th><th>Count</th></tr>"#);

    for (quarter, count) in &freq_quarter {
        html.push_str(&format!(
            "<tr><td>Q{}</td><td>{}</td></tr>",
            quarter, count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Day of Year Frequency</h2>
  <table>
    <tr><th>Day of Year</th><th>Count</th></tr>"#);

    for (doy, count) in &freq_day_of_year {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            doy, count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Epoch Buckets (Per Hour)</h2>
  <table>
    <tr><th>Epoch Hour Bucket</th><th>Count</th></tr>"#);

    for (bucket, count) in &freq_epoch_bucket {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            bucket, count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>AM/PM Frequency</h2>
  <table>
    <tr><th>Period</th><th>Count</th></tr>"#);

    for (period, count) in &freq_ampm {
        html.push_str(&format!("<tr><td>{}</td><td>{}</td></tr>", period, count));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Weekday vs Weekend Frequency</h2>
  <table>
    <tr><th>Kind</th><th>Count</th></tr>"#);

    for (kind, count) in &freq_weekend {
        html.push_str(&format!("<tr><td>{}</td><td>{}</td></tr>", kind, count));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Message Length Frequency (20-char buckets)</h2>
  <table>
    <tr><th>Length Bucket</th><th>Count</th></tr>"#);

    for (bucket, count) in &freq_message_len_bucket {
        html.push_str(&format!(
            "<tr><td>{}-{}</td><td>{}</td></tr>",
            bucket, bucket + 19, count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Position Frequency (10m buckets)</h2>
  <table>
    <tr><th>Axis</th><th>Bucket</th><th>Count</th></tr>"#);

    for (bucket, count) in &freq_x_bucket {
        html.push_str(&format!("<tr><td>X</td><td>{}</td><td>{}</td></tr>", bucket, count));
    }
    for (bucket, count) in &freq_y_bucket {
        html.push_str(&format!("<tr><td>Y</td><td>{}</td><td>{}</td></tr>", bucket, count));
    }
    for (bucket, count) in &freq_z_bucket {
        html.push_str(&format!("<tr><td>Z</td><td>{}</td><td>{}</td></tr>", bucket, count));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Avatar ID Frequency</h2>
  <table>
    <tr><th>Avatar ID</th><th>Count</th></tr>"#);

    let mut avatar_id_vec: Vec<(String, usize)> = freq_avatar_id
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    avatar_id_vec.sort_by(|a, b| b.1.cmp(&a.1));

    for (id, count) in avatar_id_vec {
      html.push_str(&format!(
        "<tr><td>{}</td><td>{}</td></tr>",
        escape_html(&id), count
      ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Word Frequency (Top 100)</h2>
  <table>
    <tr><th>Word</th><th>Count</th></tr>"#);

    let mut word_vec: Vec<(String, usize)> = word_counts
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    word_vec.sort_by(|a, b| b.1.cmp(&a.1));
    word_vec.truncate(100);

    for (word, count) in word_vec {
      html.push_str(&format!(
        "<tr><td>{}</td><td>{}</td></tr>",
        escape_html(&word), count
      ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Avatar Score Breakdown</h2>
  <table>
    <tr><th>Avatar</th><th>Hostile</th><th>Positive</th><th>Drug</th><th>Slang</th><th>Total</th></tr>"#);

    let mut avatar_scores_vec: Vec<(String, i64, i64, i64, i64, i64)> = avatar_scores
        .iter()
        .map(|(name, (h, p, d, s, t))| (name.clone(), *h, *p, *d, *s, *t))
        .collect();
    avatar_scores_vec.sort_by(|a, b| b.5.cmp(&a.5));

    for (name, hostile, positive, drug, slang, total) in avatar_scores_vec {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
        escape_html(&name), hostile, positive, drug, slang, total
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Captured By &times; Sim Crosstab</h2>
  <table>
    <tr><th>Capturer</th><th>Sim</th><th>Count</th></tr>"#);

    let mut captured_by_sim_vec: Vec<((String, String), usize)> = captured_by_sim_counts
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    captured_by_sim_vec.sort_by(|a, b| b.1.cmp(&a.1));

    for ((capturer, sim), count) in captured_by_sim_vec {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
        escape_html(&capturer), escape_html(&sim), count
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Rate of Change per Sim (Average)</h2>
  <table>
    <tr><th>Sim</th><th>Samples</th><th>Avg Rate</th></tr>"#);

    let mut roc_per_sim_vec: Vec<(String, usize, f64)> = roc_per_sim
        .iter()
        .map(|(sim, samples)| {
            let avg = if samples.is_empty() {
                0.0
            } else {
                samples.iter().map(|(_, roc)| roc).sum::<f64>() / samples.len() as f64
            };
            (sim.clone(), samples.len(), avg)
        })
        .collect();
    roc_per_sim_vec.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    for (sim, samples, avg) in roc_per_sim_vec {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{:.4}</td></tr>",
        escape_html(&sim), samples, avg
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <div class="section-heading">
    <h2>Recent Messages</h2>
    <span id="recentMessageStatus" class="small">Loading</span>
  </div>
  <div id="recentMessages" class="recent-messages" aria-live="polite"></div>
</section>
<section>
  <h2>Quarantine (Flagged Messages)</h2>
  <table>
    <tr><th>Avatar</th><th>Sim</th><th>Message</th><th>Timestamp</th></tr>"#);

    let mut quarantine_sorted = quarantine.clone();
    quarantine_sorted.sort_by_key(|m| (!m.timestamp_valid, m.timestamp));
    quarantine_sorted.truncate(50);

    for m in &quarantine_sorted {
      let ts_str = if !m.timestamp_valid {
        "Invalid timestamp".to_string()
      } else if let LocalResult::Single(dt) = Los_Angeles.timestamp_opt(m.timestamp, 0) {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
        } else {
            m.timestamp.to_string()
        };
        html.push_str(&format!(
            "<tr class=\"quarantine\"><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
          escape_html(&m.avatar_name),
          escape_html(&m.sim_name),
          escape_html(&m.message),
          ts_str
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Topics</h2>
  <div>"#);

    let mut topics_vec: Vec<(String, usize)> = topic_buckets
        .iter()
        .map(|(w, c)| (w.clone(), *c))
        .collect();
    topics_vec.sort_by(|a, b| b.1.cmp(&a.1));
    topics_vec.truncate(50);

    for (w, c) in topics_vec {
        html.push_str(&format!(
            "<span class=\"tag\">{} <span class=\"small\">{}</span></span>",
        escape_html(&w), c
        ));
    }

    html.push_str(r#"</div>
</section>
<section>
    <h2>Captured By Frequency</h2>
  <table>
    <tr><th>Capture Source</th><th>Messages</th><th>Probability</th><th>Avg Sentiment</th><th>Total Sentiment</th></tr>"#);

    for (name, count, share, sentiment_total, sentiment_average) in &captured_by_frequency {
        html.push_str(&format!(
        "<tr><td>{}</td><td>{}</td><td>{:.2}%</td><td>{:.3}</td><td>{}</td></tr>",
      escape_html(name), count, share * 100.0, sentiment_average, sentiment_total
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Captured By: Top Topics</h2>
  <table>
    <tr><th>Capturer</th><th>Top Topics</th></tr>"#);

    let mut captured_by_topics_vec: Vec<(String, Vec<(String, usize)>)> = captured_by_topics
        .iter()
        .map(|(capturer, topics)| {
            let mut topics_vec: Vec<(String, usize)> =
                topics.iter().map(|(w, c)| (w.clone(), *c)).collect();
            topics_vec.sort_by(|a, b| b.1.cmp(&a.1));
            topics_vec.truncate(8);
            (capturer.clone(), topics_vec)
        })
        .collect();
    captured_by_topics_vec.sort_by(|a, b| a.0.cmp(&b.0));

    for (capturer, topics) in captured_by_topics_vec {
        let joined = topics
            .iter()
        .map(|(w, c)| format!("{} ({})", escape_html(w), c))
            .collect::<Vec<_>>()
            .join(", ");
      html.push_str(&format!(
        "<tr><td>{}</td><td>{}</td></tr>",
        escape_html(&capturer), joined
      ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Rate of Change</h2>
  <div class="chart">
    <canvas id="rocChart"></canvas>
  </div>
  <div class="small">Approximate rate of message arrival over time.</div>
</section>
<section>
  <h2>3D Heatmap (Raw Position Coordinates)</h2>
  <div class="chart">
    <canvas id="heat3dCanvas"></canvas>
  </div>
  <div class="small">Drag or swipe to rotate. Each point uses the raw (x,y,z) coordinates from the chatlog.</div>
</section>
<section>
  <h2>Sim Markov Chain (Transitions)</h2>
  <table>
    <tr><th>From Sim</th><th>To Sim</th><th>Count</th><th>Probability</th></tr>"#);

    let mut sim_trans_vec: Vec<(String, String, usize, f64)> = sim_transitions
        .iter()
        .map(|((from, to), count)| {
            let p = *sim_transition_prob.get(&(from.clone(), to.clone())).unwrap_or(&0.0);
            (from.clone(), to.clone(), *count, p)
        })
        .collect();
    sim_trans_vec.sort_by(|a, b| b.2.cmp(&a.2));
    sim_trans_vec.truncate(50);

    for (from, to, count, prob) in sim_trans_vec {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{:.3}</td></tr>",
        escape_html(&from), escape_html(&to), count, prob
        ));
    }

    html.push_str(r#"</table>
</section>
<section>
  <h2>Avatar Markov Chain (Interactions)</h2>
  <table>
    <tr><th>From Avatar</th><th>To Avatar</th><th>Count</th><th>Probability</th></tr>"#);

    let mut avatar_trans_vec: Vec<(String, String, usize, f64)> = avatar_interactions
        .iter()
        .map(|((from, to), count)| {
            let p = *avatar_transition_prob.get(&(from.clone(), to.clone())).unwrap_or(&0.0);
            (from.clone(), to.clone(), *count, p)
        })
        .collect();
    avatar_trans_vec.sort_by(|a, b| b.2.cmp(&a.2));
    avatar_trans_vec.truncate(50);

    for (from, to, count, prob) in avatar_trans_vec {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{:.3}</td></tr>",
        escape_html(&from), escape_html(&to), count, prob
        ));
    }

    html.push_str(r#"</table>
</section>
</main>
<script id="chatlogDashboardData" type="application/json">"#);
    html.push_str(&dashboard_data_json);
    html.push_str(r#"</script>
<script>
"#);

    html.push_str(r#"
const dashboardData = JSON.parse(document.getElementById('chatlogDashboardData').textContent);
let timelineData = dashboardData.timeline || [];
let sentimentData = dashboardData.sentiment || [];
let rocData = dashboardData.rate_of_change || [];
let heat3dData = dashboardData.heatmap || [];
let chartRange = 'all';

function filterTimelineByRange(data) {
  const days = Number(chartRange);
  if (!Number.isFinite(days) || days <= 0) return data;
  const dated = data
    .map(point => ({ point, timestamp: Date.parse(`${point.date}T00:00:00Z`) }))
    .filter(({ timestamp }) => Number.isFinite(timestamp));
  if (!dated.length) return data;
  const newest = Math.max.apply(null, dated.map(({ timestamp }) => timestamp));
  const cutoff = newest - days * 24 * 60 * 60 * 1000;
  return dated
    .filter(({ timestamp }) => timestamp >= cutoff)
    .map(({ point }) => point);
}

function renderLineChart(canvasId, data, xKey, yKey, color) {
  const canvas = document.getElementById(canvasId);
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  if (w < 2 || h < 2) return;

  const pixelRatio = Math.max(window.devicePixelRatio || 1, 1);
  const pixelWidth = Math.round(w * pixelRatio);
  const pixelHeight = Math.round(h * pixelRatio);
  if (canvas.width !== pixelWidth || canvas.height !== pixelHeight) {
    canvas.width = pixelWidth;
    canvas.height = pixelHeight;
  }
  ctx.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
  ctx.clearRect(0, 0, w, h);

  const points = data
    .map((entry, index) => {
      const rawX = entry[xKey];
      const x = typeof rawX === 'string'
        ? Date.parse(`${rawX}T00:00:00Z`)
        : Number(rawX);
      const y = Number(entry[yKey]);
      return { x: Number.isFinite(x) ? x : index, y, label: String(rawX ?? '') };
    })
    .filter(point => Number.isFinite(point.y));

  if (!points.length) {
    ctx.fillStyle = '#c5c6c7';
    ctx.font = '12px system-ui';
    ctx.fillText('No data', 10, 20);
    return;
  }

  const padding = { top: 18, right: 12, bottom: 24, left: 38 };
  const plotWidth = Math.max(1, w - padding.left - padding.right);
  const plotHeight = Math.max(1, h - padding.top - padding.bottom);
  const xs = points.map(point => point.x);
  const ys = points.map(point => point.y);
  let minX = Math.min.apply(null, xs);
  let maxX = Math.max.apply(null, xs);
  let minY = Math.min.apply(null, ys);
  let maxY = Math.max.apply(null, ys);
  if (minX === maxX) {
    minX -= 1;
    maxX += 1;
  }
  if (minY === maxY) {
    minY -= 1;
    maxY += 1;
  }

  const yMin = Math.min(minY, 0);
  const yMax = Math.max(maxY, 0);
  const xToCanvas = x => padding.left + ((x - minX) / (maxX - minX)) * plotWidth;
  const yToCanvas = y => padding.top + (1 - ((y - yMin) / (yMax - yMin))) * plotHeight;

  ctx.strokeStyle = '#45a29e22';
  ctx.lineWidth = 1;
  for (let index = 0; index <= 4; index++) {
    const y = padding.top + (plotHeight * index) / 4;
    ctx.beginPath();
    ctx.moveTo(padding.left, y);
    ctx.lineTo(w - padding.right, y);
    ctx.stroke();
  }

  if (yMin < 0 && yMax > 0) {
    ctx.strokeStyle = '#c5c6c766';
    ctx.beginPath();
    ctx.moveTo(padding.left, yToCanvas(0));
    ctx.lineTo(w - padding.right, yToCanvas(0));
    ctx.stroke();
  }

  ctx.strokeStyle = color;
  ctx.lineWidth = 2;
  ctx.beginPath();

  for (let i = 0; i < points.length; i++) {
    const x = xToCanvas(points[i].x);
    const y = yToCanvas(points[i].y);
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }

  ctx.stroke();
  ctx.fillStyle = '#c5c6c7';
  ctx.font = '10px system-ui';
  ctx.fillText(yMax.toFixed(2), 4, padding.top + 3);
  ctx.fillText(yMin.toFixed(2), 4, h - padding.bottom + 3);
  ctx.fillText(points[0].label, padding.left, h - 7);
  const endLabel = points[points.length - 1].label;
  ctx.fillText(endLabel, Math.max(padding.left, w - padding.right - ctx.measureText(endLabel).width), h - 7);
}

let heat3dRot = { y: 0.6, x: 0.35 };
let heat3dNormalized = null;
let heat3dDragging = false;
let dashboardRefreshInFlight = false;
let dashboardResizeObserver = null;

function normalizeHeat3D(data) {
  if (!data.length) return [];
  const xs = data.map(d => d.x);
  const ys = data.map(d => d.y);
  const zs = data.map(d => d.z);
  const cx = (Math.min.apply(null, xs) + Math.max.apply(null, xs)) / 2;
  const cy = (Math.min.apply(null, ys) + Math.max.apply(null, ys)) / 2;
  const cz = (Math.min.apply(null, zs) + Math.max.apply(null, zs)) / 2;
  const range = Math.max(
    Math.max.apply(null, xs) - Math.min.apply(null, xs),
    Math.max.apply(null, ys) - Math.min.apply(null, ys),
    Math.max.apply(null, zs) - Math.min.apply(null, zs),
    1e-6
  );
  return data.map(d => ({
    x: (d.x - cx) / range,
    y: (d.y - cy) / range,
    z: (d.z - cz) / range,
  }));
}

function renderHeat3D(canvasId, data) {
  const canvas = document.getElementById(canvasId);
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const w = canvas.width = canvas.clientWidth;
  const h = canvas.height = canvas.clientHeight;
  ctx.clearRect(0, 0, w, h);

  if (!data.length) {
    ctx.fillStyle = '#c5c6c7';
    ctx.font = '12px system-ui';
    ctx.fillText('No data', 10, 20);
    return;
  }

  if (!heat3dNormalized) heat3dNormalized = normalizeHeat3D(data);

  const cosY = Math.cos(heat3dRot.y), sinY = Math.sin(heat3dRot.y);
  const cosX = Math.cos(heat3dRot.x), sinX = Math.sin(heat3dRot.x);
  const scale = Math.min(w, h) * 0.42;
  const cx = w / 2, cy = h / 2;

  const projected = heat3dNormalized.map(p => {
    // yaw around Y axis, then pitch around X axis
    const x1 = p.x * cosY + p.z * sinY;
    const z1 = -p.x * sinY + p.z * cosY;
    const y1 = p.y * cosX - z1 * sinX;
    const z2 = p.y * sinX + z1 * cosX;
    return { x: x1, y: y1, z: z2 };
  });

  // painter's algorithm: draw back-to-front so closer points overdraw
  projected.sort((a, b) => a.z - b.z);

  for (const p of projected) {
    const depth = (p.z + 1) / 2;
    const alpha = 0.15 + depth * 0.55;
    const size = 2 + depth * 3;
    ctx.fillStyle = `rgba(102, 252, 241, ${alpha.toFixed(2)})`;
    ctx.fillRect(cx + p.x * scale - size / 2, cy - p.y * scale - size / 2, size, size);
  }
}

function attachHeat3DRotation(canvasId, data) {
  const canvas = document.getElementById(canvasId);
  if (!canvas) return;
  let dragging = false;
  let pointerId = null;
  let lastX = 0;
  let lastY = 0;

  canvas.style.cursor = 'grab';
  canvas.addEventListener('pointerdown', event => {
    dragging = true;
    heat3dDragging = true;
    pointerId = event.pointerId;
    lastX = event.clientX;
    lastY = event.clientY;
    canvas.setPointerCapture(pointerId);
    canvas.style.cursor = 'grabbing';
  });
  const stopDragging = event => {
    if (event.pointerId !== pointerId) return;
    dragging = false;
    heat3dDragging = false;
    pointerId = null;
    canvas.style.cursor = 'grab';
  };
  canvas.addEventListener('pointerup', stopDragging);
  canvas.addEventListener('pointercancel', stopDragging);
  canvas.addEventListener('pointermove', event => {
    if (!dragging || event.pointerId !== pointerId) return;
    heat3dRot.y += (event.clientX - lastX) * 0.01;
    heat3dRot.x = Math.max(-1.5, Math.min(1.5, heat3dRot.x + (event.clientY - lastY) * 0.01));
    lastX = event.clientX;
    lastY = event.clientY;
    renderHeat3D(canvasId, data);
  });
}

function recentTagClass(tag) {
  const classes = {
    'urgent': 'tag-urgent',
    'high-hostility': 'tag-high-hostility',
    'hostile': 'tag-hostile',
    'substance': 'tag-substance',
    'slang': 'tag-slang',
    'positive': 'tag-positive',
    'neutral': 'tag-neutral',
  };
  return classes[tag] || 'tag-neutral';
}

function renderRecentMessages(messages) {
  const root = document.getElementById('recentMessages');
  if (!root) return;
  root.replaceChildren();

  if (!messages.length) {
    const empty = document.createElement('div');
    empty.className = 'small';
    empty.textContent = 'No messages yet.';
    root.append(empty);
    return;
  }

  for (const message of messages) {
    const item = document.createElement('article');
    item.className = 'recent-message';
    if (message.quarantined) item.classList.add('is-quarantined');

    const meta = document.createElement('div');
    meta.className = 'recent-meta';
    const identity = message.avatar_name || message.avatar_id || 'Unknown avatar';
    const location = message.sim_name ? ` - ${message.sim_name}` : '';
    meta.append(`${message.timestamp_label || 'Invalid timestamp'} - ${identity}${location}`);

    for (const tag of message.tags || []) {
      const badge = document.createElement('span');
      badge.className = `message-tag ${recentTagClass(tag)}`;
      badge.textContent = tag.replace('-', ' ');
      meta.append(badge);
    }

    if (message.hostility_score > 0) {
      const score = document.createElement('span');
      score.className = 'message-tag tag-hostile';
      score.textContent = `score ${message.hostility_score}`;
      meta.append(score);
    }

    const content = document.createElement('div');
    content.className = 'recent-message-text';
    content.textContent = message.message || '';
    item.append(meta, content);

    if (message.hostile_terms && message.hostile_terms.length) {
      const terms = document.createElement('div');
      terms.className = 'small';
      terms.textContent = message.hostile_terms.join(', ');
      item.append(terms);
    }

    root.append(item);
  }
}

async function refreshRecentMessages() {
  const status = document.getElementById('recentMessageStatus');
  try {
    const response = await fetch('/chatlog?format=recent', { cache: 'no-store' });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const payload = await response.json();
    renderRecentMessages(payload.messages || []);
    if (status) {
      const refreshedAt = new Date(payload.refreshed_at);
      status.textContent = Number.isNaN(refreshedAt.valueOf())
        ? 'Updated'
        : `Updated ${refreshedAt.toLocaleTimeString()}`;
    }
  } catch (_) {
    if (status) status.textContent = 'Refresh unavailable';
  }
}

function renderCharts() {
  renderLineChart('timelineChart', filterTimelineByRange(timelineData), 'date', 'count', '#66fcf1');
  renderLineChart('sentimentChart', filterTimelineByRange(sentimentData), 'date', 'score', '#ff6b6b');
  renderLineChart('rocChart', rocData, 'ts', 'roc', '#45a29e');
  renderHeat3D('heat3dCanvas', heat3dData);
}

function attachChartRangeControl() {
  const control = document.getElementById('chartRange');
  if (!control) return;
  control.value = chartRange;
  control.addEventListener('change', () => {
    chartRange = control.value;
    renderCharts();
  });
}

function observeDashboardSize() {
  const dashboard = document.getElementById('chatlogDashboard');
  if (!dashboard) return;
  if ('ResizeObserver' in window) {
    if (!dashboardResizeObserver) dashboardResizeObserver = new ResizeObserver(renderCharts);
    dashboardResizeObserver.disconnect();
    dashboardResizeObserver.observe(dashboard);
  }
}

function initializeDashboard() {
  attachChartRangeControl();
  renderCharts();
  attachHeat3DRotation('heat3dCanvas', heat3dData);
  observeDashboardSize();
  refreshRecentMessages();
}

async function refreshDashboard() {
  if (
    dashboardRefreshInFlight
    || document.hidden
    || heat3dDragging
    || window.getSelection().type === 'Range'
  ) return;

  dashboardRefreshInFlight = true;
  try {
    const response = await fetch('/chatlog', { cache: 'no-store' });
    if (!response.ok) throw new Error(`HTTP ${response.status}`);

    const nextDocument = new DOMParser().parseFromString(await response.text(), 'text/html');
    const nextHeader = nextDocument.getElementById('chatlogHeader');
    const nextDashboard = nextDocument.getElementById('chatlogDashboard');
    const nextData = nextDocument.getElementById('chatlogDashboardData');
    if (!nextHeader || !nextDashboard || !nextData) return;

    const nextDashboardData = JSON.parse(nextData.textContent);
    const scrollX = window.scrollX;
    const scrollY = window.scrollY;
    timelineData = nextDashboardData.timeline || [];
    sentimentData = nextDashboardData.sentiment || [];
    rocData = nextDashboardData.rate_of_change || [];
    heat3dData = nextDashboardData.heatmap || [];
    heat3dNormalized = null;

    document.getElementById('chatlogHeader').replaceWith(nextHeader);
    document.getElementById('chatlogDashboard').replaceWith(nextDashboard);
    document.getElementById('chatlogDashboardData').textContent = nextData.textContent;
    initializeDashboard();
    window.requestAnimationFrame(() => window.scrollTo(scrollX, scrollY));
  } catch (_) {
    // Keep the last successful dashboard view if a background refresh fails.
  } finally {
    dashboardRefreshInFlight = false;
  }
}

document.addEventListener('DOMContentLoaded', () => {
  initializeDashboard();
  if (!('ResizeObserver' in window)) window.addEventListener('resize', renderCharts);
  window.addEventListener('visibilitychange', () => {
    if (!document.hidden) refreshDashboard();
  });
  window.setInterval(refreshRecentMessages, 5000);
  window.setInterval(refreshDashboard, 5000);
});
</script>
</body>
</html>
"#);

  chatlog_cache_store(
    req.state().chatlog_cache.as_ref(),
    source_len,
    &source_modified,
    "html",
    html.clone(),
  );
    let mut res = Response::new(StatusCode::Ok);
    res.set_body(html);
    res.insert_header("Content-Type", "text/html; charset=utf-8");
    Ok(res)
});





    

    /*
      app.at("/rustby").get(|req: tide::Request<AppState>| {
          let rustby_eval_title = rustby_eval_title.clone();
          async move {
              let query: HashMap<String, String> = req.query().unwrap_or_default();
              let vlog = query
                  .get("vlog")
                  .cloned()
                  .unwrap_or_else(|| "".to_string());

              let title = rustby_eval_title.to_string();
              let base_iframe_url = format!("https://miaedscore.online:8080/{}", vlog);

              let html_content = format!(r######"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title}</title>
  <meta name="description" content="This page embeds an external webpage via an iFrame.">
  <meta name="author" content="TIADE-MAEPPERS">
  <meta name="keywords" content="HTML, iFrame, Embedded Page">
  <meta name="theme-color" content="#ffffff">
  <meta name="robots" content="index, follow">
  <meta name="googlebot" content="index, follow">
  <meta name="google" content="notranslate">
  <meta name="msapplication-TileColor" content="#ffffff">
  <meta name="msapplication-TileImage" content="https://example.com/favicon.png">
  <meta name="apple-mobile-web-app-capable" content="yes">
  <meta name="apple-mobile-web-app-status-bar-style" content="default">
  <meta name="apple-mobile-web-app-title" content="Embedded iFrame">
  <meta name="application-name" content="Embedded iFrame">
  <meta name="format-detection" content="telephone=no">
  <link rel="icon" href="https://example.com/favicon.png">
  <style>
    body {{
      margin: 0;
      padding: 0;
      font-family: sans-serif;
      background-color: #f8f8f8;
    }}
    .header {{
      background-color: #333;
      color: #fff;
      padding: 10px 20px;
      text-align: center;
    }}
    .iframe-container {{
      position: relative;
      width: 100%;
      height: calc(100vh - 120px);
      overflow: hidden;
    }}
    .iframe-container iframe {{
      position: absolute;
      top: 0;
      left: 0;
      width: 100%;
      height: 100%;
      border: none;
    }}
    .footer {{
      background-color: #333;
      color: #fff;
      text-align: center;
      padding: 10px 20px;
    }}
  </style>
  <script>
    document.addEventListener("DOMContentLoaded", function() {{
      document.body.addEventListener("click", function(event) {{
        var target = event.target.closest("a");
        if (target && target.href) {{
          event.preventDefault();
          var url = new URL(target.href);
          var newPath = url.pathname + url.search + url.hash;
          var iframe = document.getElementById("contentFrame");
          if (iframe) {{
            iframe.src = "{base_iframe_url}" + newPath;
            history.pushState(null, '', url.pathname);
          }}
        }}
      }});
    }});
  </script>
</head>
<body>
  <div class="header">
    <h1>{title}</h1>
    <nav>
      <a href="/page1">Page 1</a> |
      <a href="/page2?query=example">Page 2</a>
    </nav>
  </div>
  <div class="iframe-container">
    <iframe id="contentFrame" src="{base_iframe_url}"></iframe>
  </div>
  <div class="footer">
    <p>&copy; 2025 TIADE-MAEPPERS. All rights reserved.</p>
  </div>
</body>
</html>"######);

              let mut res = tide::Response::new(tide::StatusCode::Ok);
              res.set_body(html_content);
              res.set_content_type("text/html");
              Ok(res)
          }
      });
    */

    // Route to handle the "/bridge/*rest" path
    // This will serve an HTML page with an iframe loading the target URL.
    // The iframe will load the URL "https://miaedscore.online:8080/*rest"
    // The JavaScript snippet in the HTML will remove any query parameters from the browser URL.
    // The HTML page will be served with the content type "text/html".
    // The HTML page will be styled to take up the full width and height of the browser window.
    // The iframe will be styled to take up the full width and height of the browser window.
    // The HTML page will have a light gray background color.
    // The iframe will have no border.
    // The HTML page will have a title "Bridge Iframe".
    // The HTML page will have a meta tag for viewport settings.
    // The HTML page will have a meta tag for character set settings.
    // The HTML page will have a meta tag for theme color settings.
    // The HTML page will have a meta tag for robots settings.
    // The HTML page will have a meta tag for apple mobile web app settings.
    // The HTML page will have a meta tag for application name settings.
    // The HTML page will have a meta tag for format detection settings.
    // The HTML page will have a meta tag for ms application tile color settings.
    // The HTML page will have a meta tag for ms application tile image settings.
    // The HTML page will have a meta tag for google bot settings.
    // The HTML page will have a meta tag for google settings.
    // The HTML page will have a meta tag for favicon settings.
    // The HTML page will have a meta tag for author settings.
    // The HTML page will have a meta tag for description settings.

    app.at("/bridge/*rest")
        .get(|req: tide::Request<AppState>| async move {
            // Extract the wildcard part from the URL.
            let rest = req.param("rest").unwrap_or("");
            // Build the target URL for the 8080 server.
            let target_url = format!("https://miaedscore.online:8080/{}", rest);
            let escaped_target_url = target_url
              .replace('&', "&amp;")
              .replace('"', "&quot;")
              .replace('<', "&lt;")
              .replace('>', "&gt;");

            // Build an HTML page with an iframe loading the target URL.
            // A JavaScript snippet removes any query parameters from the browser URL.
            let html_content = format!(
                r#"<!DOCTYPE html>
  <html lang="en">
  <head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Bridge Iframe</title>
    <style>
      html, body {{
        margin: 0;
        padding: 0;
        width: 100%;
        height: 100%;
        background-color: #f8f8f8;
      }}
      iframe {{
        width: 100%;
        height: 100%;
        border: none;
      }}
    </style>
    <script>
      // Remove query parameters from address bar.
      if(window.location.search.length > 0) {{
        window.history.replaceState(null, null, window.location.pathname);
      }}
    </script>
  </head>
  <body>
    <iframe src="{0}" title="Bridge - Embedded 8080 Server"></iframe>
  </body>
  </html>"#,
                escaped_target_url
            );

            // Return the HTML response.
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            res.set_body(html_content);
            res.set_content_type("text/html");
            Ok(res)
        });

    {
        // Prelude for the Ruby-backed routes (/time, /ae, /weather, /tiade/moon, /tiade/sun).
        let contents = r######"
       require 'date'
       require 'fileutils'
       require 'time'
       require 'json'
       require 'oj'
       require 'date'
       require 'net/http'

      class ForecastByLongitude
    GRIDPOINT_FORECAST_URL = 'https://api.weather.gov/gridpoints/EKA/93,22/forecast'.freeze

    def initialize
    end

    def fetch_forecast(_lat = nil, _lon = nil)
      [
        '--- Miaedscore-Plateau, Califurnia :: Daily Forecast ---',
        print_forecast(GRIDPOINT_FORECAST_URL)
      ].compact.join("\n")
    end

    def print_forecast(url)
      return 'No forecast URL provided.' unless url

      uri = URI(url)
      response = Net::HTTP.get_response(uri)
      return "Error fetching forecast: #{response.code}" unless response.is_a?(Net::HTTPSuccess)

      data = JSON.parse(response.body)
      periods = data.dig('properties', 'periods')

      if periods && !periods.empty?
        periods.map do |period|
          name = period['name']
          temp = "#{period['temperature']} #{period['temperatureUnit']}"
          forecast = period['shortForecast']
          "#{name}: #{temp}, #{forecast}"
        end.join("\n")
      else
        'No forecast data available.'
      end
    end
  end


    # This Ruby code is designed to be evaluated by the Magnus Ruby interpreter.
      class AECalendar
    attr_reader :start_date, :year_length, :month_length

    def initialize(start_date = DateTime.new(2025, 6, 4, 0, 0, 0), month_length = 14, months_in_year = 12)
      @start_date = start_date
      @month_length = month_length
      @year_length = month_length * months_in_year
    end

    def ae_date(gregorian_date)
      days_since_start = (gregorian_date - @start_date).to_i
      ae_year = 1 + (days_since_start / @year_length)
      ae_month = 1 + ((days_since_start % @year_length) / @month_length)
      ae_day = 1 + ((days_since_start % @year_length) % @month_length)
      day_of_week = gregorian_date.strftime('%A') # Get the day name

      "AE #{ae_year}, Month #{ae_month}, Day #{ae_day} (#{day_of_week})"
    end
  end

  # Example usage
  ae_calendar = AECalendar.new
  gregorian_example = DateTime.new(2025, 7, 1)





    class MoonPhaseDetails2
      # === Constants and Definitions ===

        # Average length of a full lunar cycle (in days)
    MOON_CYCLE_DAYS = 29.53


# The 27 fabled moon rotations with emojis:
MOON_ROTATIONS = [
  'New Moon 🌑', # 1
  'Waxing Crescent 🌒',     # 2
  'First Quarter 🌓',       # 3
  'Waxing Gibbous 🌔',      # 4
  'Full Moon 🌕',           # 5
  'Waning Gibbous 🌖',      # 6
  'Last Quarter 🌗',        # 7
  'Waning Crescent 🌘',     # 8
  'Supermoon 🌝',           # 9
  'Blue Moon 🔵🌙',         # 10
  'Blood Moon 🩸🌙',        # 11
  'Harvest Moon 🍂🌕',      # 12
  "Hunter's Moon 🌙🔭",     # 13
  'Wolf Moon 🐺🌕',         # 14
  'Pink Moon 🌸🌕', # 15
  'Snow Moon 🌨️', # 16
  'Snow Moon Snow 🌨️❄️', # 17
  'Avian Moon 🦅', # 18
  'Avian Moon Snow 🦅❄️',    # 19
  'Skunk Moon 🦨',           # 20
  'Skunk Moon Snow 🦨❄️',    # 21
  'Cosmic Moon 🌌🌕', # 22
  'Celestial Moon 🌟🌕', # 23
  'Otter Moon 🐕🌌', # 24
  'Muskium Otter Muskium Stinky Stimky Otter Moon 🦨🌌', # 25
  'Light Elemental Moon 💡🌕', # 26
  'Dark Elemental Moon 🌑🌕' # 27

]
# Define 27 corresponding species with emojis.
SPECIES = [
  'Dogg 🐶', # New Moon
  'Folf 🦊🐺', # Waxing Crescent
  'Aardwolf 🐾',
  'Spotted Hyena 🐆',
  'Folf Hybrid 🦊✨',
  'Striped Hyena 🦓',
  'Dogg Prime 🐕⭐',
  'WolfFox 🐺🦊', # Waning Crescent
  'Brown Hyena 🦴',
  'Dogg Celestial 🐕🌟',
  'Folf Eclipse 🦊🌒',
  'Aardwolf Luminous 🐾✨',
  'Spotted Hyena Stellar 🐆⭐',
  'Folf Nova 🦊💥',
  'Brown Hyena Cosmic 🦴🌌',
  'Snow Leopard 🌨️', # New Moon
  'Snow Leopard Snow Snep 🌨️❄️',
  'Avian 🦅',
  'Avian Snow 🦅❄️',
  'Skunk 🦨',
  'Skunk Snow 🦨❄️',
  'Infini-Vaeria Graevity-Infini 🌌🐕',
  'Graevity-Infini Infini-Vaeria 🌟🐕',
  'Otter 🦦',
  'Muskium Otter Stinky Stimky 🦦🦨',
  'Light Elf 💡',
  'Light Elf Cosmic 🌑'

]

# Define 27 corresponding were-forms with emojis.
WERE_FORMS = [
  'WereDogg 🐶🌑',
  'WereFolf 🦊🌙',
  'WereAardwolf 🐾',
  'WereSpottedHyena 🐆',
  'WereFolfHybrid 🦊✨',
  'WereStripedHyena 🦓',
  'WereDoggPrime 🐕⭐',
  'WereWolfFox 🐺🦊', # Waning Crescent
  'WereBrownHyena 🦴',
  'WereDoggCelestial 🐕🌟',
  'WereFolfEclipse 🦊🌒',
  'WereAardwolfLuminous 🐾✨',
  'WereSpottedHyenaStellar 🐆⭐',
  'WereFolfNova 🦊💥', # Wolf Moon
  'WereBrownHyenaCosmic 🦴🌌', # Pink Moon
  'WereSnowLeopard 🐆❄️',
  'WereSnowLeopardSnow 🐆❄️❄️', # Pink Moon
  'WereAvian 🦅', # New Moon
  'WereAvianSnow 🦅❄️', # Pink Moon
  'WereSkunk 🦨', # New Moon
  'WereSkunkSnow 🦨❄️', # New Moon
  'WereInfiniVaeriaGraevity 🐕🌌',
  'WereGraevityInfiniInfiniVaeria 🌟🐕',
  'WereOtter 🦦',
  'WereMuskiumOtterStinkyStimky 🦦🦨',
  'WereLightElf 💡',
  'WereLightElfCosmic 🌑'
]

    # Each moon phase is assumed to share an equal slice of the lunar cycle.
    PHASE_COUNT  = MOON_ROTATIONS.size # 15 total phases
    PHASE_LENGTH = MOON_CYCLE_DAYS / PHASE_COUNT # Days per phase
      # === Core Function ===

      def self.current_moon_details(date)
        reference_date = Date.new(2000, 1, 6)
        days_since_reference = (date - reference_date).to_f
        lunar_position = days_since_reference % MOON_CYCLE_DAYS
        phase_index_raw = lunar_position / PHASE_LENGTH
        phase_index = phase_index_raw.floor
        conscious_percentage = (phase_index_raw / (PHASE_COUNT - 1).to_f) * 100
        current_phase     = MOON_ROTATIONS[phase_index % MOON_ROTATIONS.size]
        current_species   = SPECIES[phase_index % SPECIES.size]
        current_were_form = WERE_FORMS[phase_index % WERE_FORMS.size]
        consciousness_level = "#{phase_index_raw}/#{PHASE_COUNT - 1} (#{conscious_percentage}%)"
        [current_phase, current_species, current_were_form, consciousness_level, conscious_percentage, phase_index_raw]
      end

      # === HTML-Generating Functions ===

      def self.render_full_schedule_html
        rows = ''
        MOON_ROTATIONS.each_with_index do |phase_name, index|
          rows << <<~ROW
            <tr>
              <td>#{phase_name}</td>
              <td>#{SPECIES[index]}</td>
              <td>#{WERE_FORMS[index]}</td>
            </tr>
          ROW
        end

        <<~HTML
          <div class="container">
            <h1>Complete Moon Rotation Schedule</h1>
            <table>
              <thead>
                <tr>
                  <th>Moon Phase</th>
                  <th>Species</th>
                  <th>Were-Form</th>
                </tr>
              </thead>
              <tbody>
                #{rows}
              </tbody>
            </table>
          </div>
        HTML
      end

      def self.print_details_for_date(date)
        phase, species, were_form, consciousness, consciousness_percentage, phase_index_raw = current_moon_details(date)
        "<p>
            Moon Phase: #{phase}<br />
            Species: #{species}<br />
            Were-Form: #{were_form}<br />
            Consciousness: #{consciousness}<br />
            Miade-Score/Infini-Vaeria Consciousness: #{1 - (consciousness_percentage / 100)}% (#{1 - (phase_index_raw / PHASE_COUNT - 1)}%)<br />
          </p>"
      end

      def self.print_text_details_for_date(date)
        phase, species, were_form, consciousness, consciousness_percentage, phase_index_raw = current_moon_details(date)
        " Moon Phase: #{phase}\n
            Species: #{species}\n
            Were-Form: #{were_form}\n
            Consciousness: #{consciousness}\n"
      end
    end

    class SunPhase2
      attr_reader :name, :start_hour, :emoji

      def initialize(name, start_hour, emoji)
        @name = name
        @start_hour = start_hour
        @emoji = emoji
      end
    end

    class SolarDance2
      PHASES = [
        SunPhase2.new('Midnight Mystery', 0, '🌑'),
        SunPhase2.new('Dawn\'s Whisper', 3, '🌅'),
        SunPhase2.new('First Light’s Murmur', 5, '🔅'),
        SunPhase2.new('Golden Awakening', 6, '☀️'),
        SunPhase2.new('Morning Glow', 8, '🌞'),
        SunPhase2.new('High Noon Radiance', 12, '🔥'),
        SunPhase2.new('Afternoon Brilliance', 15, '🌇'),
        SunPhase2.new('Golden Hour Serenade', 17, '🌆'),
        SunPhase2.new('Twilight Poetry', 18, '🌒'),
        SunPhase2.new('Dusky Secrets', 19, '🌓'),
        SunPhase2.new('Crimson Horizon', 20, '🌔'),
        SunPhase2.new('Moon\'s Ascent', 21, '🌕'),
        SunPhase2.new('Nightfall\'s Caress', 22, '✨'),
        SunPhase2.new('Deep Celestial Silence', 23, '🌌'),
        SunPhase2.new('Cosmic Slumber', 24, '🌠'),
      ]

      def self.current_phase
        pst_hour = Time.now.getlocal('-08:00').hour
        PHASES.reverse.find { |phase| pst_hour >= phase.start_hour }
      end

      def self.sun_dance_message
        phase = current_phase
        "The Sun is currently in \"#{phase.name}\" phase! #{phase.emoji}"
      end
    end

    class Calendar
      attr_reader :date

      def initialize
        @date = Date.today
      end

      def gregorian
        @date.strftime('%m/%d/%Y')
      end

      def julian
        jd = @date.jd
        julian_date = Date.jd(jd, Date::JULIAN)
        julian_date.strftime('%m/%d/%Y')
      end

      def julian_primitive
        @date.jd
      end

      def formatted_pst_time
        pst_time = Time.now.getlocal('-07:00')
        pst_time.strftime('%B, %d, %Y - %I:%M:%S %p SLT/PST')
      end
    end

         def formatted_pst_time
        pst_time = Time.now.getlocal('-07:00')
        pst_time.strftime('%B, %d, %Y - %I:%M:%S %p SLT/PST')
      end







    "######;
        match ruby_vm::eval_blocking(contents) {
            Ok(_) => println!("Loaded Ruby route prelude into embedded VM"),
            Err(error) => eprintln!("Failed to load Ruby route prelude: {}", error),
        }
    }

    app.at("/time")
        .get(|_req: tide::Request<AppState>| async move {
            ruby_vm::text_response(r######"

    "Gregorian: #{Calendar.new.gregorian}\nJulian: #{Calendar.new.julian_primitive} -> #{Calendar.new.julian}\nPST+DST+SLT: #{formatted_pst_time}"

    "######).await
        });



   app.at("/random").get(|mut req: tide::Request<AppState>| async move {


    let mut res = tide::Response::new(tide::StatusCode::Ok);
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let random_value: u32 = rng.gen_range(0..4);
    let output = random_value.to_string();

     // Return the HTML response.
    let mut res = tide::Response::new(tide::StatusCode::Ok);
    res.set_body(output);
    res.insert_header("Content-Type", "text/plain; charset=utf-8");
    Ok(res)
    //Ok(output.into())
  });


     app.at("/random2").get(|mut req: tide::Request<AppState>| async move {


    use rand::Rng;
    let mut rng = rand::thread_rng();
    let output = match rng.gen_range(0..3) {
      0 => "1/2",
      1 => "1/1",
      _ => "3/2",
    };

     // Return the HTML response.
    let mut res = tide::Response::new(tide::StatusCode::Ok);
    res.set_body(output);
    res.insert_header("Content-Type", "text/plain; charset=utf-8");
    Ok(res)
    //Ok(output.into())
  });


    app.at("/sigil-deck").get(|_| async move {
      let entries = load_sigil_deck_entries()?;
      let mut res = tide::Response::new(tide::StatusCode::Ok);
      res.set_body(render_sigil_deck_page(&entries));
      res.insert_header("Content-Type", "text/html; charset=utf-8");
      Ok(res)
    });

    app.at("/flashcard").get(|_| async move {
      let entries = load_sigil_deck_entries()?;
      let mut res = tide::Response::new(tide::StatusCode::Ok);
      res.set_body(render_flashcard_page(&entries));
      res.insert_header("Content-Type", "text/html; charset=utf-8");
      Ok(res)
    });

    app.at("/sigil-deck/upload").post(|mut req: tide::Request<AppState>| async move {
      let content_type = req
        .header("content-type")
        .and_then(|values| values.get(0))
        .map(|value| value.as_str().to_string())
        .unwrap_or_default();

      if !content_type.contains("multipart/form-data") {
        return Ok(tide::Response::new(tide::StatusCode::BadRequest));
      }

      let raw = req.body_bytes().await?;
      let (fields, files) = parse_multipart_form_data(&content_type, &raw);
      let title = fields
        .get("title")
        .cloned()
        .unwrap_or_else(|| "Untitled Sigil".to_string());
      let description = fields
        .get("description")
        .cloned()
        .unwrap_or_else(|| "No description provided yet.".to_string());
      let image_data = match fields.get("image_url").map(String::as_str).map(str::trim) {
        Some(image_url) if !image_url.is_empty() => download_sigil_image(image_url).await?,
        _ => {
          let Some(upload) = files.get("image").cloned().or_else(|| files.values().next().cloned()) else {
            return Ok(tide::Response::new(tide::StatusCode::BadRequest));
          };
          upload.data
        }
      };
      let (image_data, ext) = sigil_normalize_image(&image_data)?;

      sigil_deck_ensure_storage()?;
      let mut entries = load_sigil_deck_entries()?;
      let next_id = entries
        .iter()
        .map(|entry| entry.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
      let filename = format!("sigil_{}_{}.{}", next_id, Utc::now().timestamp_millis(), ext);
      let path = format!("{}/{}", SIGIL_DECK_UPLOAD_DIR, filename);
      std::fs::write(&path, &image_data)
        .map_err(|e| tide::Error::from_str(tide::StatusCode::InternalServerError, e.to_string()))?;

      entries.push(SigilDeckEntry {
        id: next_id,
        title,
        description,
        image_file: filename.clone(),
        mime_type: sigil_mime_from_ext(ext).to_string(),
        created_at: sigil_deck_now_string(),
      });
      save_sigil_deck_entries(&entries)?;

      Ok(redirect(&format!("/sigil-deck/card/{}", next_id)))
    });

    app.at("/sigil-deck/card/:id/update").post(|mut req: tide::Request<AppState>| async move {
      let id: u64 = req.param("id").unwrap_or("0").parse().unwrap_or(0);
      let content_type = req
        .header("content-type")
        .and_then(|values| values.get(0))
        .map(|value| value.as_str().to_string())
        .unwrap_or_default();
      if !content_type.contains("multipart/form-data") {
        return Ok(tide::Response::new(tide::StatusCode::BadRequest));
      }

      let raw = req.body_bytes().await?;
      let (fields, files) = parse_multipart_form_data(&content_type, &raw);
      let mut entries = load_sigil_deck_entries()?;
      let Some(entry_index) = entries.iter().position(|entry| entry.id == id) else {
        return Ok(tide::Response::new(tide::StatusCode::NotFound));
      };
      let entry = &mut entries[entry_index];
      entry.title = fields.get("title").cloned().unwrap_or_else(|| entry.title.clone());
      entry.description = fields
        .get("description")
        .cloned()
        .unwrap_or_else(|| entry.description.clone());

      let mut old_image_path = None;
      let replacement_data = match fields.get("image_url").map(String::as_str).map(str::trim) {
        Some(image_url) if !image_url.is_empty() => Some(download_sigil_image(image_url).await?),
        _ => files.get("image").filter(|file| !file.data.is_empty()).map(|file| file.data.clone()),
      };
      if let Some(replacement_data) = replacement_data {
        let (replacement_data, ext) = sigil_normalize_image(&replacement_data)?;
        let filename = format!("sigil_{}_{}.{}", id, Utc::now().timestamp_millis(), ext);
        let new_image_path = sigil_deck_upload_path(&filename)?;
        std::fs::write(&new_image_path, &replacement_data)
          .map_err(|e| tide::Error::from_str(tide::StatusCode::InternalServerError, e.to_string()))?;
        old_image_path = Some(sigil_deck_upload_path(&entry.image_file)?);
        entry.image_file = filename;
        entry.mime_type = sigil_mime_from_ext(ext).to_string();
      }

      save_sigil_deck_entries(&entries)?;
      if let Some(path) = old_image_path {
        std::fs::remove_file(path)
          .map_err(|e| tide::Error::from_str(tide::StatusCode::InternalServerError, e.to_string()))?;
      }
      Ok(redirect(&format!("/sigil-deck/card/{}", id)))
    });

    app.at("/sigil-deck/card/:id/delete").post(|req: tide::Request<AppState>| async move {
      let id: u64 = req.param("id").unwrap_or("0").parse().unwrap_or(0);
      let mut entries = load_sigil_deck_entries()?;
      let Some(entry_index) = entries.iter().position(|entry| entry.id == id) else {
        return Ok(tide::Response::new(tide::StatusCode::NotFound));
      };
      let entry = entries.remove(entry_index);
      let image_path = sigil_deck_upload_path(&entry.image_file)?;
      std::fs::remove_file(image_path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => tide::Error::from_str(tide::StatusCode::NotFound, "sigil image not found"),
        _ => tide::Error::from_str(tide::StatusCode::InternalServerError, err.to_string()),
      })?;
      save_sigil_deck_entries(&entries)?;
      Ok(redirect("/sigil-deck"))
    });

    app.at("/sigil-deck/image/:filename").get(|req: tide::Request<AppState>| async move {
      let filename = req.param("filename").unwrap_or("");
      let safe_name = std::path::Path::new(filename)
        .file_name()
        .and_then(|part| part.to_str())
        .unwrap_or("");
      if safe_name.is_empty() {
        return Ok(tide::Response::new(tide::StatusCode::BadRequest));
      }

      let path = format!("{}/{}", SIGIL_DECK_UPLOAD_DIR, safe_name);
      let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
          return Ok(tide::Response::new(tide::StatusCode::NotFound));
        }
        Err(err) => {
          return Err(tide::Error::from_str(
            tide::StatusCode::InternalServerError,
            err.to_string(),
          ));
        }
      };

      let ext = std::path::Path::new(safe_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin");
      let mut res = tide::Response::new(tide::StatusCode::Ok);
      res.set_body(bytes);
      res.insert_header("Content-Type", sigil_mime_from_ext(ext));
      Ok(res)
    });

    app.at("/sigil-deck/card/:id").get(|req: tide::Request<AppState>| async move {
      let id: u64 = req.param("id").unwrap_or("0").parse().unwrap_or(0);
      let entries = load_sigil_deck_entries()?;
      let Some(entry) = entries.iter().find(|entry| entry.id == id).cloned() else {
        return Ok(tide::Response::new(tide::StatusCode::NotFound));
      };

      let mut res = tide::Response::new(tide::StatusCode::Ok);
      res.set_body(render_sigil_card_page(&entry, entries.len()));
      res.insert_header("Content-Type", "text/html; charset=utf-8");
      Ok(res)
    });

    app.at("/sigil-deck/random").get(|_| async move {
      let entries = load_sigil_deck_entries()?;
      if entries.is_empty() {
        let mut res = tide::Response::new(tide::StatusCode::Ok);
        res.set_body(render_sigil_deck_page(&entries));
        res.insert_header("Content-Type", "text/html; charset=utf-8");
        return Ok(res);
      }

      use rand::Rng;
      let index = rand::thread_rng().gen_range(0..entries.len());
      let entry = entries[index].clone();

      Ok(redirect(&format!("/sigil-deck/card/{}", entry.id)))
    });

    // Migrated endpoints are mounted by tiade_ollama_relay::mount_routes.

    app.at("/ae")
        .get(|_req: tide::Request<AppState>| async move {
            ruby_vm::text_response(r######"

     # Example usage
  ae_calendar = AECalendar.new
  "AE Calendar: #{ae_calendar.ae_date(DateTime.now)}"

    "######).await
        });

    app.at("/tiade/moon")
        .get(|_req: tide::Request<AppState>| async move {
            ruby_vm::text_response(r######"

    "#{MoonPhaseDetails2.print_text_details_for_date(Date.today)}"

    "######).await
        });

    app.at("/weather")
        .get(|_req: tide::Request<AppState>| async move {
            ruby_vm::text_response(r######"

    "#{ForecastByLongitude.new.fetch_forecast(39.068684, -122.781375)}"

    "######).await
        });

    //get neutri alg
    app.at("/rneutrialg")
        .get(|mut req: tide::Request<AppState>| async move {
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            //res.set_body("HTML content for /moon route");
            //res.set_content_type("text/html; charset=utf-8");
            //return Ok(res);
            // Grab Ruby code from request body.
            let query: std::collections::HashMap<String, String> = req.query().unwrap_or_default();
            let file_contents = std::fs::read_to_string("rneutri.txt")
                .map_err(|e| tide::Error::new(tide::StatusCode::InternalServerError, e))?;

            // Return the HTML response.
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            res.set_body(file_contents);
            res.insert_header("Content-Type", "text/plain; charset=utf-8");
            Ok(res)
            //Ok(output.into())
        });

    //neutri setter
    app.at("/rneutri")
        .get(|mut req: tide::Request<AppState>| async move {
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            //res.set_body("HTML content for /moon route");
            //res.set_content_type("text/html; charset=utf-8");
            //return Ok(res);
            // Grab Ruby code from request body.
            let query: std::collections::HashMap<String, String> = req.query().unwrap_or_default();
            let value = query.get("value").unwrap_or(&String::new()).to_string();
            std::fs::write("rneutri.txt", &value)
                .map_err(|e| tide::Error::new(tide::StatusCode::InternalServerError, e))?;

            // Return the HTML response.
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            res.set_body(value);
            res.insert_header("Content-Type", "text/plain; charset=utf-8");
            Ok(res)
            //Ok(output.into())
        });

    app.at("/tiade/sun")
        .get(|_req: tide::Request<AppState>| async move {
            ruby_vm::text_response(r######"

    "#{SolarDance2.sun_dance_message}"

    "######).await
        });

    app.at("/tiade-maepers/*rest")
        .get(|req: tide::Request<AppState>| async move {
            // Extract the wildcard part from the URL.
            let rest = req.param("rest").unwrap_or("");
            // Build the target URL for the 8080 server.
            let target_url = format!("https://miaedscore.online/{}", rest);

            // Build an HTML page with an iframe loading the target URL.
            // A JavaScript snippet removes any query parameters from the browser URL.
            let html_content = format!(
                r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Bridge Iframe</title>
  <style>
  /* Include style.css from the CSS folder */
  @import url('/css/style.css');



  /* Additional styling specific to this page */
    html, body {{
      margin: 0;
      padding: 0;
      width: 100%;
      height: 100%;
      background-color: #f8f8f8;
    }}
    iframe {{
      width: 100%;
      height: 100%;
      border: none;
    }}
  </style>
  <script>
    // Remove query parameters from address bar.
    if(window.location.search.length > 0) {{
      window.history.replaceState(null, null, window.location.pathname);
    }}
  </script>
</head>
<body>
  <iframe src="{0}" title="Stimky.info -> miadscore.online [B]log/Gallery"></iframe>
</body>
</html>"#,
                target_url
            );

            // Return the HTML response.
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            res.set_body(html_content);
            res.set_content_type("text/html");
            Ok(res)
        });
    app.at("/parse_plink")
        .get(|req: tide::Request<AppState>| async move {
            // Expect a query parameter "text" that includes a full URL (e.g., "https://miaedscore.online:8080/some/path?query=val")
            let query: HashMap<String, String> = req.query().unwrap_or_default();
            let input_text = query.get("text").map(|s| s.as_str()).unwrap_or("");
            if input_text.is_empty() {
                return Ok(tide::Response::new(StatusCode::BadRequest));
            }

            // Parse the provided URL string.
            let parsed_url = match Url::parse(input_text) {
                Ok(url) => url,
                Err(_) => return Ok(tide::Response::new(StatusCode::BadRequest)),
            };

            // Extract the path and query parts to form the rustby GET parameter.
            let mut vlog = parsed_url.path().to_string();
            if let Some(q) = parsed_url.query() {
                vlog.push('?');
                vlog.push_str(q);
            }

            // Construct the redirection URL to /rustby with the extracted "vlog" parameter.
            let redirect_url = format!("/rustby?vlog={}", vlog);
            let mut res = tide::Response::new(StatusCode::Found);
            res.insert_header("Location", redirect_url);
            Ok(res)
        });

    // assuming the helper is in the module

    app.at("/tiade/img/resize")
        .post(|mut req: tide::Request<AppState>| async move {
            // Extract query parameters.
            let query: HashMap<String, String> = req.query().unwrap_or_default();
            let file_name = query.get("filename").cloned().unwrap_or_default();
            if file_name.is_empty() {
                let mut res = tide::Response::new(StatusCode::BadRequest);
                res.set_body("Missing filename query parameter".to_string());
                return Ok(res);
            }

            // Check for a file extension.
            let path = Path::new(&file_name);
            let ext = path.extension().and_then(|os_str| os_str.to_str());
            if ext.is_none() {
                let mut res = tide::Response::new(StatusCode::BadRequest);
                res.set_body("File extension missing".to_string());
                return Ok(res);
            }
            let ext = ext.unwrap();

            // Optional: get desired width and height (default to 800x600).
            let width: u32 = query
                .get("width")
                .and_then(|s| s.parse().ok())
                .unwrap_or(800);
            let height: u32 = query
                .get("height")
                .and_then(|s| s.parse().ok())
                .unwrap_or(600);

            // Read the image bytes from the request body.
            let data = req.body_bytes().await?;

            let mut res = tide::Response::new(tide::StatusCode::Ok);
            res.set_body("Image resized (placeholder)".to_string());
            Ok(res)
        });

    app.at("/").get(|_| async {
        let mut res = tide::Response::new(tide::StatusCode::Ok);        
        res.set_body("<!DOCTYPE html>\n<html>\n<head>\n  <title>Home</title>\n</head>\n<body>\n  <h1>Welcome</h1>\n</body>\n</html>".to_string());
      res.insert_header("Content-Type", "text/html; charset=utf-8");
        Ok(res)
    });
    /*
        app.at("/paema").get(move |req: Request<AppState>| {
            let rustby_eval_title = rustby_eval_title.clone();
            async move {
                let query: HashMap<String, String> = req.query().unwrap_or_default();
                let vlog = query
                    .get("vlog")
                    .cloned()
                    .unwrap_or_else(|| "".to_string());

                let title = rustby_eval_title.to_string();
                let base_iframe_url = format!("https://miaedscore.online:8080/{}", vlog);

                let html_content = format!(r######"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title}</title>
  <meta name="description" content="This page embeds an external webpage via an iFrame.">
  <meta name="author" content="TIADE-MAEPPERS">
  <meta name="keywords" content="HTML, iFrame, Embedded Page">
  <meta name="theme-color" content="#ffffff">
  <meta name="robots" content="index, follow">
  <meta name="googlebot" content="index, follow">
  <meta name="google" content="notranslate">
  <meta name="msapplication-TileColor" content="#ffffff">
  <meta name="msapplication-TileImage" content="https://example.com/favicon.png">
  <meta name="apple-mobile-web-app-capable" content="yes">
  <meta name="apple-mobile-web-app-status-bar-style" content="default">
  <meta name="apple-mobile-web-app-title" content="Embedded iFrame">
  <meta name="application-name" content="Embedded iFrame">
  <meta name="format-detection" content="telephone=no">
  <link rel="icon" href="https://example.com/favicon.png">
  <style>
    body {{
      margin: 0;
      padding: 0;
      font-family: sans-serif;
      background-color: #f8f8f8;
    }}
    .header {{
      background-color: #333;
      color: #fff;
      padding: 10px 20px;
      text-align: center;
    }}
    .iframe-container {{
      position: relative;
      width: 100%;
      height: calc(100vh - 120px);
      overflow: hidden;
    }}
    .iframe-container iframe {{
      position: absolute;
      top: 0;
      left: 0;
      width: 100%;
      height: 100%;
      border: none;
    }}
    .footer {{
      background-color: #333;
      color: #fff;
      text-align: center;
      padding: 10px 20px;
    }}
  </style>

  <script>
    document.addEventListener("DOMContentLoaded", function() {{
      document.body.addEventListener("click", function(event) {{
        var target = event.target.closest("a");
        if (target && target.href) {{
          event.preventDefault();
          var url = new URL(target.href);
          var newPath = url.pathname + url.search + url.hash;
          var iframe = document.getElementById("contentFrame");
          if (iframe) {{
            iframe.src = "{base_iframe_url}" + newPath;
            history.pushState(null, '', url.pathname);
          }}
        }}
      }});
    }});
  </script>
</head>
<body>
  <div class="header">
    <h1>{title}</h1>
    <nav>
      <a href="/page1">Page 1</a> |
      <a href="/page2?query=example">Page 2</a>
    </nav>
  </div>
  <div class="iframe-container">
    <iframe id="contentFrame" src="{base_iframe_url}"></iframe>
  </div>
  <div class="footer">
    <p>&copy; 2025 TIADE-MAEPPERS. All rights reserved.</p>
  </div>
</body>
</html>"######);

                let mut res = tide::Response::new(tide::StatusCode::Ok);
                res.set_body(html_content);
                res.set_content_type("text/html");
                Ok(res)
            }
        });
    */

    // A simple POST endpoint
    app.at("/echo")
        .post(|mut req: Request<AppState>| async move {
            let body = req.body_string().await.unwrap_or_default();
            Ok(format!("You sent: {}", body))
        });

    // Route to restart all spawned servers
    app.at("/restart-servers").post(|req: Request<AppState>| async move {
      persist_memory_stores(req.state()).map_err(|error| {
        tide::Error::from_str(
          StatusCode::InternalServerError,
          format!("failed to persist partitioned memory stores: {}", error),
        )
      })?;
        println!("Restarting all servers...");
        std::process::Command::new("sh")
            .arg("-c")
            .arg("killall -HUP tiade-maeepers-saerver-all") // Replace with your server binary name
            .spawn()
            .expect("Failed to restart servers");
        Ok("Servers are restarting")
    });

    // Add a file
    app.at("/file/add")
        .post(|mut req: Request<AppState>| async move {
            let contents = req.body_bytes().await.unwrap_or_default();
            std::fs::write("/tmp/new_file.txt", &contents)?;
            Ok("File added")
        });

    // Delete a file
    app.at("/file/delete").delete(|_| async {
        std::fs::remove_file("/tmp/new_file.txt")?;
        Ok("File deleted")
    });

    //roda_tide_rewrite::mount_roda_compat_routes(&mut app);
  

    // Listen on all interfaces over standard HTTPS (TLS) port by default.
    let addresses = vec![std::env::var("MSSL_ADDRESS")
      .unwrap_or_else(|_| "0.0.0.0:443".to_string())];
    let cert_path = std::env::var("MSSL_CERT_PATH")
      .unwrap_or_else(|_| "/etc/letsencrypt/live/stimky.info/fullchain.pem".to_string());
    let key_path = std::env::var("MSSL_KEY_PATH")
      .unwrap_or_else(|_| "/etc/letsencrypt/live/stimky.info/privkey.pem".to_string());

    let mut tasks = vec![];
    for addr in addresses {
        let app_clone = app.clone();
        let c = cert_path.clone();
        let k = key_path.clone();
      let listen_addr = addr.clone();
        println!("Spawning server on address: {}", addr); // Debug message
        tasks.push(async_std::task::spawn(async move {
        let listener = TlsListener::build().addrs(listen_addr.clone()).cert(c).key(k);
        println!("Server is starting on address: {}", listen_addr); // Debug message
            app_clone.listen(listener).await
        }));
    }

    for t in tasks {
        if let Err(e) = t.await {
            println!("Error while running server: {}", e); // Debug message
        }
    }
    if let Err(error) = persist_memory_stores(&state) {
      eprintln!("Failed to persist partitioned memory stores: {}", error);
    }
    println!("All servers have been spawned successfully."); // Debug message
    Ok(())
}
