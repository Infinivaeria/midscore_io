
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

// Helper: convert HashMap<String, Value> -> serde_json::Map<String, Value>
fn hashmap_to_map(src: &std::collections::HashMap<String, serde_json::Value>) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    for (k, v) in src.iter() {
        m.insert(k.clone(), v.clone());
    }
    m
}

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



lazy_static! {
    static ref VARS: Arc<Mutex<Map<String,Value>>> = Arc::new(Mutex::new(Map::new()));
}
use lazy_static::lazy_static;
use async_std::io::WriteExt;

 // Ensure these constants are defined at module top-level (not inside this function).
    const DATA_DIR: &str = "/midscore_io/tiade-maeepers-saerver-all/";
  const FLAT_FILE: &str = "vars_flatfile.json";
    const HISTORY_FILE: &str = "vars_history.log";
use std::collections::HashMap;
use futures_util::TryFutureExt;
/// Async save_snapshot that accepts a serde_json::Map (the type you are passing from handlers).
/// Writes DATA_DIR/FLAT_FILE and appends HISTORY_FILE. Returns tide::Result so callers can handle errors.
async fn save_snapshot(snapshot: Map<String, Value>, tag: &str) -> tide::Result<()> {
   

    let dir = Path::new(DATA_DIR);

    // create_dir_all is async; await it and map errors to tide::Error
    fs::create_dir_all(dir).map_err(|e| {
        tide::Error::from_str(
            StatusCode::InternalServerError,
            format!("create_dir_all failed: {}", e),
        )
    })?;

    let flat_path = dir.join(FLAT_FILE);

    // Serialize the serde_json::Map directly
    let flat_json = serde_json::to_string_pretty(&snapshot).map_err(|e| {
        tide::Error::from_str(
            StatusCode::InternalServerError,
            format!("serialize failed: {}", e),
        )
    })?;

    // Write file asynchronously
    fs::write(&flat_path, flat_json).map_err(|e| {
        tide::Error::from_str(
            StatusCode::InternalServerError,
            format!("write flat file failed: {}", e),
        )
    });
    println!("save_snapshot: wrote {}", flat_path.display());

    // Append history entry
    let history_path = dir.join(HISTORY_FILE);
    let entry = format!("{} - {}\n", Utc::now().to_rfc3339(), tag);

    let mut f = async_std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&history_path)
        .await
        .map_err(|e| {
            tide::Error::from_str(
                StatusCode::InternalServerError,
                format!("open history failed: {}", e),
            )
        })?;

    f.write_all(entry.as_bytes()).await.map_err(|e| {
        tide::Error::from_str(
            StatusCode::InternalServerError,
            format!("write history failed: {}", e),
        )
    })?;

    Ok(())
  }



#[async_std::main]
async fn main() -> tide::Result<()> {
    // Data directory and filenames (place near top of main.rs, after imports)

    // Main HTTPS server - handling all defined routes
let mut app = tide::with_state(AppState {
    queue: Mutex::new(Vec::new()),
    results: Mutex::new(Vec::new()),
});
    // Spawn a background thread to listen for CLI input.
    std::thread::spawn(|| {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            if let Ok(input) = line {
                match input.trim() {
                    "exit" => {
                        println!("Exiting server abruptly.");
                        std::process::exit(0);
                    }

                    // When the "rustby" command is input, write the Ruby code to a .rb file
                    // in a shared directory ("./rustby_scripts"). Then, immediately load (evaluate)
                    // the file using Magnus. The file is deleted after evaluation. The Ruby code in
                    // the file is expected to return a string.
                    "rustby" => {
                        println!("Running Ruby code via named pipe sharing system...");
                        let script_dir = "./rustby_scripts";
                        if let Err(e) = std::fs::create_dir_all(script_dir) {
                            println!("Failed to create script directory: {}", e);
                            continue;
                        }
                        let filename = format!(
                            "{}/script_{}.rb",
                            script_dir,
                            Utc::now().timestamp_nanos_opt().unwrap_or(0)
                        );
                        // Replace the Ruby code below as needed. It must return a string value.
                        let ruby_code = r#"nil
       'RustbySpace'
      "#;
                        if let Err(e) = std::fs::write(&filename, ruby_code) {
                            println!("Error writing script file: {}", e);
                            continue;
                        }
                        println!("Script file written: {}", filename);

                        // Instead of calling the Ruby evaluator directly (which cannot be done in a thread),
                        // write the Ruby load command to a named pipe for external processing.
                        let pipe_path = "/tmp/ruby_pipe";
                        if let Err(e) = std::fs::write(pipe_path, format!("load '{}'\n", filename))
                        {
                            println!("Error writing to named pipe: {}", e);
                        } else {
                            println!("Command sent to Ruby evaluator via pipe: {}", pipe_path);
                        }

                        // Wait briefly for the external process to evaluate the script and write the result.
                        std::thread::sleep(std::time::Duration::from_millis(100));

                        // Read the evaluation result from an output file.
                        let result_path = "/tmp/ruby_output.txt";
                        let script_result = match std::fs::read_to_string(result_path) {
                            Ok(output) => Ok(output),
                            Err(e) => {
                                eprintln!("Error reading Ruby output: {}", e);
                            Err(format!("Error reading Ruby output: {}", e))
                            }
                        };

                        // Remove the script file after evaluation.
                        if let Err(e) = std::fs::remove_file(&filename) {
                            eprintln!("Failed to remove script file: {}", e);
                        }

                        match script_result {
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

// --------------------------------------------------------
// AppState (Tide server state)
// --------------------------------------------------------


struct AppState {
    queue: Mutex<Vec<QueuedCommand>>,
    results: Mutex<Vec<CompletedResult>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
            results: Mutex::new(Vec::new()),
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


   app.at("/analytics").get(|_req: tide::Request<AppState>| async move {
    use chrono::{Datelike, FixedOffset, Timelike, Utc, NaiveDateTime};
    use serde_json::Value;
    use std::collections::{BTreeMap, HashMap, HashSet};

    const PATH: &str =
        "/root/midscore_io/tiade-maeepers-saerver-all/second_life_chat_logs.txt";

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
    // Read raw log file
    // ---------------------------------------------------------------------
    let raw = std::fs::read_to_string(PATH).unwrap_or_default();
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
    out.push_str(&format!("Source file: {}\n", PATH));
    out.push_str(&format!("Raw parsed entries: {}\n", entries.len()));
    out.push_str(&format!("Total unique events: {}\n", events.len()));
    out.push_str(&format!("Unique avatar IDs: {}\n", avatars.len()));
    out.push_str(&format!("Unique message bodies: {}\n", messages.len()));

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

        // Log file path (adjust to a writable path for your process)
        let log_path = "/root/midscore_io/tiade-maeepers-saerver-all/second_life_chat_logs.txt";

        // Ensure parent directory exists
        if let Some(parent) = Path::new(log_path).parent() {
            create_dir_all(parent).map_err(|e| {
                println!("Failed to create directory {}: {}", parent.display(), e);
                tide::Error::new(StatusCode::InternalServerError, e)
            })?;
        }

        // Append to file (create if missing)
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|e| {
                println!("Failed to open log file {}: {}", log_path, e);
                tide::Error::new(StatusCode::InternalServerError, e)
            })?;

        writeln!(file, "{}", body).map_err(|e| {
            println!("Failed to write to log file: {}", e);
            tide::Error::new(StatusCode::InternalServerError, e)
        })?;

        // Respond
        let mut res = Response::new(StatusCode::Ok);
        res.set_body("Log entry received and written to file successfully.");
        res.insert_header("Content-Type", "text/plain; charset=utf-8");
        Ok(res)
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

    {
        let mut vars = VARS.lock().unwrap();
        if let Some(obj) = body.as_object() {
            for (k, v) in obj {
                vars.insert(k.clone(), v.clone());
            }
        } else {
            let mut res = Response::new(StatusCode::BadRequest);
            res.set_body("expected JSON object");
            res.insert_header("Content-Type", "text/plain");
            return Ok(res);
        }
    }

    // Build snapshot as serde_json::Map
    let snapshot_map = {
        let vars = VARS.lock().unwrap();
        let mut map = Map::new();
        for (k, v) in vars.iter() {
            map.insert(k.clone(), v.clone());
        }
        map
    };

    if let Err(e) = save_snapshot(snapshot_map, "SET").await {
        println!("save_snapshot error (SET): {}", e);
        let mut res = Response::new(StatusCode::InternalServerError);
        res.set_body(format!("set failed: {}", e));
        res.insert_header("Content-Type", "text/plain");
        return Ok(res);
    }

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

    let val_opt = {
        let vars = VARS.lock().unwrap();
        vars.get(&name).cloned()
    };

    // Build snapshot_map from current in-memory vars
    let snapshot_map = {
        let vars = VARS.lock().unwrap();
        let mut map = Map::new();
        for (k, v) in vars.iter() {
            map.insert(k.clone(), v.clone());
        }
        map
    };

    if let Some(val) = val_opt {
        if let Err(e) = save_snapshot(snapshot_map, &format!("GET {}", name)).await {
            println!("save_snapshot error (GET {}): {}", name, e);
        }
        let mut res = Response::new(StatusCode::Ok);
        res.set_body(serde_json::to_string(&val)?);
        res.insert_header("Content-Type", "application/json; charset=utf-8");

        Ok(res)
    } else {
        // rebuild snapshot for the "not found" case (snapshot_map was moved above)
        let snapshot_map = {
            let vars = VARS.lock().unwrap();
            let mut map = Map::new();
            for (k, v) in vars.iter() {
                map.insert(k.clone(), v.clone());
            }
            map
        };
        if let Err(e) = save_snapshot(snapshot_map, &format!("GET {} not found", name)).await {
            println!("save_snapshot error (GET not found {}): {}", name, e);
        }
        let mut res = Response::new(StatusCode::NotFound);
        //res.set_body("not found"); -- we don't need to send a body for this response
        println!("GET {} not found", name);
        res.insert_header("Content-Type", "text/plain; charset=utf-8");

        Ok(res)
    }
});

/// /vars/view
app.at("/vars/view").post(|_req: Request<AppState>| async move {
    let snapshot_map = {
        let vars = VARS.lock().unwrap();
        let mut map = Map::new();
        for (k, v) in vars.iter() {
            map.insert(k.clone(), v.clone());
        }
        map
    };

    if let Err(e) = save_snapshot(snapshot_map.clone(), "VIEW").await {
        println!("save_snapshot error (VIEW): {}", e);
    }

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

    {
        let mut vars = VARS.lock().unwrap();
        vars.remove(&name);
    }

    let snapshot_map = {
        let vars = VARS.lock().unwrap();
        let mut map = Map::new();
        for (k, v) in vars.iter() {
            map.insert(k.clone(), v.clone());
        }
        map
    };

    if let Err(e) = save_snapshot(snapshot_map, &format!("DELETE {}", name)).await {
        println!("save_snapshot error (DELETE {}): {}", name, e);
    }

    let mut res = Response::new(StatusCode::Ok);
    //res.set_body(format!("Deleted {}", name)); -- we don't need to send a body for this response
    println!("Deleted {}", name);
    res.insert_header("Content-Type", "text/plain");
    Ok(res)
});

/// /vars/clear
app.at("/vars/clear").post(|_req: Request<AppState>| async move {
    {
        let mut vars = VARS.lock().unwrap();
        vars.clear();
    }

    let snapshot_map = {
        let vars = VARS.lock().unwrap();
        let mut map = Map::new();
        for (k, v) in vars.iter() {
            map.insert(k.clone(), v.clone());
        }
        map
    };

    if let Err(e) = save_snapshot(snapshot_map, "CLEAR").await{
        println!("save_snapshot error (CLEAR): {}", e);
        let mut res = Response::new(StatusCode::InternalServerError);
        //res.set_body(format!("clear failed: {}", e));
        //res.insert_header("Content-Type", "text/plain");
        println!("clear failed: {}", e);
        return Ok(res);
    }

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

/// /vars/history (robust, uses DATA_DIR and FLAT_FILE)
app.at("/vars/history").post(|_req: Request<AppState>| async move {
    let path = format!("{}/{}", DATA_DIR, FLAT_FILE);

    match fs::metadata(&path) {
        Ok(meta) => {
            if meta.len() == 0 {
                println!("history: file exists but is empty: {}", path);
                let mut res = Response::new(StatusCode::Ok);
                res.set_body("[]");
                res.insert_header("Content-Type", "application/json");
                return Ok(res);
            }
        }
        Err(e) => {
            println!("history: metadata error for {}: {}", path, e);
            let history_path = format!("{}/{}", DATA_DIR, HISTORY_FILE);
            if let Ok(content) = fs::read_to_string(&history_path) {
                let mut res = Response::new(StatusCode::Ok);
                res.set_body(content);
                res.insert_header("Content-Type", "application/json; charset=utf-8");
                return Ok(res);
            }
            let mut res = Response::new(StatusCode::NotFound);
            //res.set_body(format!("history file not found: {} (error: {})", path, e));
            println!("history file not found: {} (error: {})", path, e);
            res.insert_header("Content-Type", "text/plain");
            return Ok(res);
        }
    }

    match fs::read_to_string(&path) {
        Ok(content) => {
            let mut res = Response::new(StatusCode::Ok);
            res.set_body(content);
            res.insert_header("Content-Type", "application/json");
            Ok(res)
        }
        Err(e) => {
            println!("history: read error for {}: {}", path, e);
            let mut res = Response::new(StatusCode::InternalServerError);
            //res.set_body(format!("failed to read history file: {}", e));
            //res.insert_header("Content-Type", "text/plain");
            println!("failed to read history file: {}", e);
            Ok(res)
        }
    }
});

/// /vars/status
app.at("/vars/status").post(|_req: Request<AppState>| async move {
    let count = {
        let vars = VARS.lock().unwrap();
        vars.len()
    };

    let status = serde_json::json!({
        "vars_count": count,
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

    let mut integrity = IntegrityReport::default();
    let mut quarantine: Vec<MsgEntry> = Vec::new();

    let path = "/root/midscore_io/tiade-maeepers-saerver-all/second_life_chat_logs.txt";
    let raw = match fs::read_to_string(path) {
        Ok(s) => {
            integrity.total_lines = s.lines().count();
            s
        }
        Err(e) => {
            eprintln!("Failed to read {}: {}", path, e);
            String::new()
        }
    };

    let objects = split_json_objects(&raw);
    integrity.candidate_objects = objects.len();

    let hostile_words: HashMap<&'static str, i32> = [
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
    ].iter().cloned().collect();

    let positive_words: HashMap<&'static str, i32> = [
        ("love", 3), ("great", 2), ("awesome", 2), ("nice", 1), ("cool", 1),
        ("fun", 1), ("good", 1), ("beautiful", 2), ("kind", 2), ("friendly", 2),
        ("sweet", 2), ("amazing", 3), ("fantastic", 3), ("wonderful", 3),
        ("thanks", 2), ("thank", 1), ("appreciate", 2), ("appreciated", 2),
        ("helpful", 2), ("excellent", 3), ("brilliant", 3), ("perfect", 3),
        ("glad", 2), ("happy", 2), ("joy", 2), ("support", 2), ("supportive", 2),
        ("welcome", 1), ("cheers", 1), ("congrats", 2), ("congratulations", 3),
        ("respect", 2), ("peace", 2), ("smile", 1), ("laugh", 1), ("yay", 1),
    ].iter().cloned().collect();

    let drug_words: HashMap<&'static str, i32> = [
        ("drug", 1), ("drugs", 1), ("overdose", 3), ("intoxicated", 2),
        ("substance", 1), ("addiction", 2), ("rehab", 1), ("narcotic", 1),
        ("opioid", 2), ("heroin", 3), ("cocaine", 3), ("meth", 3),
        ("weed", 1), ("marijuana", 1),
        ("alcohol", 1), ("beer", 1), ("wine", 1), ("vodka", 1), ("whiskey", 1),
        ("cannabis", 1), ("cbd", 1), ("thc", 1), ("fentanyl", 3),
        ("amphetamine", 2), ("ketamine", 2), ("lsd", 2), ("mdma", 2),
        ("ecstasy", 2), ("benzodiazepine", 2), ("xanax", 2), ("valium", 2),
        ("pill", 1), ("pills", 1), ("sober", 1), ("withdrawal", 2), ("relapse", 2),
    ].iter().cloned().collect();

    let slang_words: HashMap<&'static str, i32> = [
        ("lol", 0), ("lmao", 0), ("rofl", 0), ("bruh", 0), ("fr", 0),
        ("sus", 1), ("salty", 1), ("ratio", 1), ("based", 0), ("cap", 1),
        ("yeet", 0), ("pog", 0), ("poggers", 0), ("smh", 0), ("ngl", 0),
        ("idk", 0),
        ("omg", 0), ("wtf", 1), ("imo", 0), ("tbh", 0), ("irl", 0),
        ("btw", 0), ("gg", 0), ("ggs", 0), ("wp", 0), ("rip", 1),
        ("yikes", 1), ("fomo", 0), ("lowkey", 0), ("highkey", 0),
        ("vibe", 0), ("vibes", 0), ("goated", 0), ("cracked", 0),
        ("cope", 1), ("seethe", 1), ("savage", 1),
    ].iter().cloned().collect();

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
    let mut captured_by_sim_counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut captured_by_topics: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();

    let mut rate_of_change: Vec<(i64, f64)> = Vec::new();
    let mut roc_per_sim: BTreeMap<String, Vec<(i64, f64)>> = BTreeMap::new();

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

        if !captured_by.is_empty() {
            *captured_by_counts.entry(captured_by.clone()).or_insert(0) += 1;
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

        let hard_flags = ["overdose", "kill", "suicide", "self harm"];
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

      let recent_only = req
        .url()
        .query_pairs()
        .any(|(key, value)| key == "format" && value == "recent");
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

    let mut html = String::new();
    html.push_str(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Second Life Chatlog Dashboard</title>
<style>
body { font-family: system-ui, sans-serif; background: #0b0c10; color: #c5c6c7; margin: 0; padding: 0; }
header { padding: 16px 24px; background: #1f2833; border-bottom: 1px solid #45a29e; }
h1 { margin: 0; font-size: 20px; color: #66fcf1; }
main { padding: 16px 24px; display: grid; grid-template-columns: 2fr 1fr; gap: 16px; }
section { background: #1f2833; border-radius: 8px; padding: 12px 16px; border: 1px solid #45a29e22; }
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
</style>
</head>
<body>
<header id="chatlogHeader">
  <h1>Second Life Chatlog Dashboard</h1>
  <div class="small">File: second_life_chat_logs.txt &mdash; Parsed objects: "#);

    html.push_str(&format!(
        "{} &mdash; Unique keys: {}",
        integrity.parsed_objects,
        unique_keys.len()
    ));
    html.push_str(r#"</div>
</header>
<main id="chatlogDashboard">
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
  <h2>Timestamp Frequency (Raw UNIX, Full)</h2>
  <table>
    <tr><th>Timestamp</th><th>Count</th></tr>"#);

    let mut ts_vec: Vec<(i64, usize)> = freq_timestamp
        .iter()
        .map(|(ts, c)| (*ts, *c))
        .collect();
    ts_vec.sort_by(|a, b| a.0.cmp(&b.0));

    for (ts, c) in ts_vec {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td></tr>",
            ts, c
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
  <h2>Captured By (Counts & Share)</h2>
  <table>
    <tr><th>Capturer</th><th>Messages</th><th>Share</th></tr>"#);

    let mut captured_vec: Vec<(String, usize, f64)> = captured_by_counts
        .iter()
        .map(|(k, v)| {
            let share = *captured_by_share.get(k).unwrap_or(&0.0);
            (k.clone(), *v, share)
        })
        .collect();
    captured_vec.sort_by(|a, b| b.1.cmp(&a.1));
    captured_vec.truncate(20);

    for (name, count, share) in captured_vec {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{:.3}</td></tr>",
        escape_html(&name), count, share
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

function renderLineChart(canvasId, data, xKey, yKey, color) {
  const canvas = document.getElementById(canvasId);
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const w = canvas.width = canvas.clientWidth;
  const h = canvas.height = canvas.clientHeight;

  if (!data.length) {
    ctx.fillStyle = '#c5c6c7';
    ctx.font = '12px system-ui';
    ctx.fillText('No data', 10, 20);
    return;
  }

  const ys = data.map(d => d[yKey]);
  let minY = Math.min.apply(null, ys);
  let maxY = Math.max.apply(null, ys);
  if (minY === maxY) {
    minY -= 1;
    maxY += 1;
  }

  ctx.clearRect(0, 0, w, h);
  ctx.strokeStyle = color;
  ctx.lineWidth = 1.5;
  ctx.beginPath();

  for (let i = 0; i < data.length; i++) {
    const t = i / Math.max(1, data.length - 1);
    const x = 16 + t * (w - 32);
    const yNorm = (ys[i] - minY) / (maxY - minY);
    const y = h - 16 - yNorm * (h - 32);

    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }

  ctx.stroke();
  ctx.fillStyle = '#c5c6c7';
  ctx.font = '10px system-ui';
  ctx.fillText('min: ' + minY.toFixed(2), 8, h - 8);
  ctx.fillText('max: ' + maxY.toFixed(2), w - 80, 12);
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
  renderLineChart('timelineChart', timelineData, 'date', 'count', '#66fcf1');
  renderLineChart('sentimentChart', sentimentData, 'date', 'score', '#ff6b6b');
  renderLineChart('rocChart', rocData, 'ts', 'roc', '#45a29e');
  renderHeat3D('heat3dCanvas', heat3dData);
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
  window.setInterval(refreshDashboard, 15000);
});
</script>
</body>
</html>
"#);

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
        std::fs::create_dir_all("/root/midscore_io/rustby/rustby-vm/target/release/scripts").ok();
        let ts = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let filename =
            format!("/root/midscore_io/rustby/rustby-vm/target/release/scripts/script_{ts}.rb");
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
        std::fs::write(&filename, contents)?;
        println!("Created script file: {}", filename);
    }

    app.at("/time").get(|mut req: tide::Request<AppState>| async move {

    let script_dir = "/root/midscore_io/rustby/rustby-vm/target/release/scripts";
    //td::fs::create_dir_all(script_dir).ok();
    let mut res = tide::Response::new(tide::StatusCode::Ok);
    //res.set_body("HTML content for /moon route");
    //res.set_content_type("text/html; charset=utf-8");
    //return Ok(res);
    // Grab Ruby code from request body.
    let ruby_source = r######"

    "Gregorian: #{Calendar.new.gregorian}\nJulian: #{Calendar.new.julian_primitive} -> #{Calendar.new.julian}\nPST+DST+SLT: #{formatted_pst_time}"

    "######;
    if ruby_source.trim().is_empty() {
        let mut resp = tide::Response::new(tide::StatusCode::Ok);
        resp.set_body("No Ruby code supplied");
        return Ok(resp);
    }

    // Create unique .rb filename.
    let ts = Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let filename = format!("{}/moon_{}.rb", script_dir,ts);
    std::fs::write(&filename, &ruby_source).map_err(|e| tide::Error::new(tide::StatusCode::InternalServerError, e))?;




    let result_path = format!("/root/midscore_io/rustby/rustby-vm/target/release/scripts/moon_{}.txt", ts);

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


     // Return the HTML response.
    let mut res = tide::Response::new(tide::StatusCode::Ok);
    res.set_body(output);
    res.insert_header("Content-Type", "text/plain; charset=utf-8");
    Ok(res)
    //Ok(output.into())
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
        .get(|mut req: tide::Request<AppState>| async move {
            let script_dir = "/root/midscore_io/rustby/rustby-vm/target/release/scripts";
            //td::fs::create_dir_all(script_dir).ok();
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            //res.set_body("HTML content for /moon route");
            //res.set_content_type("text/html; charset=utf-8");
            //return Ok(res);
            // Grab Ruby code from request body.
            let ruby_source = r######"

     # Example usage
  ae_calendar = AECalendar.new
  "AE Calendar: #{ae_calendar.ae_date(DateTime.now)}"

    "######;
            if ruby_source.trim().is_empty() {
                let mut resp = tide::Response::new(tide::StatusCode::Ok);
                resp.set_body("No Ruby code supplied");
                return Ok(resp);
            }

            // Create unique .rb filename.
            let ts = Utc::now().timestamp_nanos_opt().unwrap_or(0);
            let filename = format!("{}/ae_{}.rb", script_dir, ts);
            std::fs::write(&filename, &ruby_source)
                .map_err(|e| tide::Error::new(tide::StatusCode::InternalServerError, e))?;

            let result_path = format!(
                "/root/midscore_io/rustby/rustby-vm/target/release/scripts/ae_{}.txt",
                ts
            );

            // Block until the result file is available or until timeout
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(120);
            while !std::path::Path::new(&result_path).exists() {
                if start.elapsed() > timeout {
                    return Ok("Timed out waiting for result file".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let output =
                std::fs::read_to_string(&result_path).unwrap_or_else(|_| "No output".to_string());

            // Remove script file after evaluation.

            let _ = std::fs::remove_file(&result_path);
            let _ = std::fs::remove_file(&filename);

            // Return the HTML response.
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            res.set_body(output);
            res.insert_header("Content-Type", "text/plain; charset=utf-8");
            Ok(res)
            //Ok(output.into())
        });

    app.at("/tiade/moon")
        .get(|mut req: tide::Request<AppState>| async move {
            let script_dir = "/root/midscore_io/rustby/rustby-vm/target/release/scripts";
            //td::fs::create_dir_all(script_dir).ok();
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            //res.set_body("HTML content for /moon route");
            //res.set_content_type("text/html; charset=utf-8");
            //return Ok(res);
            // Grab Ruby code from request body.
            let ruby_source = r######"

    "#{MoonPhaseDetails2.print_text_details_for_date(Date.today)}"

    "######;
            if ruby_source.trim().is_empty() {
                let mut resp = tide::Response::new(tide::StatusCode::Ok);
                resp.set_body("No Ruby code supplied");
                return Ok(resp);
            }

            // Create unique .rb filename.
            let ts = Utc::now().timestamp_nanos_opt().unwrap_or(0);
            let filename = format!("{}/moon_{}.rb", script_dir, ts);
            std::fs::write(&filename, &ruby_source)
                .map_err(|e| tide::Error::new(tide::StatusCode::InternalServerError, e))?;

            let result_path = format!(
                "/root/midscore_io/rustby/rustby-vm/target/release/scripts/moon_{}.txt",
                ts
            );

            // Block until the result file is available or until timeout
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(120);
            while !std::path::Path::new(&result_path).exists() {
                if start.elapsed() > timeout {
                    return Ok("Timed out waiting for result file".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let output =
                std::fs::read_to_string(&result_path).unwrap_or_else(|_| "No output".to_string());

            // Remove script file after evaluation.

            let _ = std::fs::remove_file(&result_path);
            let _ = std::fs::remove_file(&filename);

            // Return the HTML response.
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            res.set_body(output);
            res.insert_header("Content-Type", "text/plain; charset=utf-8");
            Ok(res)
            //Ok(output.into())
        });

    app.at("/weather")
        .get(|mut req: tide::Request<AppState>| async move {
            let script_dir = "/root/midscore_io/rustby/rustby-vm/target/release/scripts";
            //td::fs::create_dir_all(script_dir).ok();
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            //res.set_body("HTML content for /moon route");
            //res.set_content_type("text/html; charset=utf-8");
            //return Ok(res);
            // Grab Ruby code from request body.
            let ruby_source = r######"

    "#{ForecastByLongitude.new.fetch_forecast(39.068684, -122.781375)}"

    "######;
            if ruby_source.trim().is_empty() {
                let mut resp = tide::Response::new(tide::StatusCode::Ok);
                resp.set_body("No Ruby code supplied");
                return Ok(resp);
            }

            // Create unique .rb filename.
            let ts = Utc::now().timestamp_nanos_opt().unwrap_or(0);
            let filename = format!("{}/weather_{}.rb", script_dir, ts);
            std::fs::write(&filename, &ruby_source)
                .map_err(|e| tide::Error::new(tide::StatusCode::InternalServerError, e))?;

            let result_path = format!(
                "/root/midscore_io/rustby/rustby-vm/target/release/scripts/weather_{}.txt",
                ts
            );

            // Block until the result file is available or until timeout
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(120);
            while !std::path::Path::new(&result_path).exists() {
                if start.elapsed() > timeout {
                    return Ok("Timed out waiting for result file".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let output =
                std::fs::read_to_string(&result_path).unwrap_or_else(|_| "No output".to_string());

            // Remove script file after evaluation.

            let _ = std::fs::remove_file(&result_path);
            let _ = std::fs::remove_file(&filename);

            // Return the HTML response.
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            res.set_body(output);
            res.insert_header("Content-Type", "text/plain; charset=utf-8");
            Ok(res)
            //Ok(output.into())
        });

    //get neutri alg
    app.at("/rneutrialg")
        .get(|mut req: tide::Request<AppState>| async move {
            let script_dir = "/root/midscore_io/rustby/rustby-vm/target/release/scripts";
            //td::fs::create_dir_all(script_dir).ok();
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
            let script_dir = "/root/midscore_io/rustby/rustby-vm/target/release/scripts";
            //td::fs::create_dir_all(script_dir).ok();
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
        .get(|mut req: tide::Request<AppState>| async move {
            let script_dir = "/root/midscore_io/rustby/rustby-vm/target/release/scripts";
            //td::fs::create_dir_all(script_dir).ok();
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            //res.set_body("HTML content for /moon route");
            res.set_content_type("text/html; charset=utf-8");
            //return Ok(res);
            // Grab Ruby code from request body.
            let ruby_source = r######"

    "#{SolarDance2.sun_dance_message}"

    "######;
            if ruby_source.trim().is_empty() {
                let mut resp = tide::Response::new(tide::StatusCode::Ok);
                resp.set_body("No Ruby code supplied");
                return Ok(resp);
            }

            // Create unique .rb filename.
            let ts = Utc::now().timestamp_nanos_opt().unwrap_or(0);
            let filename = format!("{}/sun_{}.rb", script_dir, ts);
            std::fs::write(&filename, &ruby_source)
                .map_err(|e| tide::Error::new(tide::StatusCode::InternalServerError, e))?;

            let result_path = format!(
                "/root/midscore_io/rustby/rustby-vm/target/release/scripts/sun_{}.txt",
                ts
            );

            // Block until the result file is available or until timeout
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(120);
            while !std::path::Path::new(&result_path).exists() {
                if start.elapsed() > timeout {
                    return Ok("Timed out waiting for result file".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let output =
                std::fs::read_to_string(&result_path).unwrap_or_else(|_| "No output".to_string());

            // Remove script file after evaluation.

            let _ = std::fs::remove_file(&result_path);
            let _ = std::fs::remove_file(&filename);

            // Return the HTML response.
            let mut res = tide::Response::new(tide::StatusCode::Ok);
            res.set_body(output);
            res.insert_header("Content-Type", "text/plain; charset=utf-8");
            Ok(res)
            //Ok(output.into())
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
    app.at("/restart-servers").post(|_| async move {
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
    println!("All servers have been spawned successfully."); // Debug message
    Ok(())
}
