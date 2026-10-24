# YouTube Music to Second Life

Open **https://stimky.info/youtubemusic/player** and play music there. No
Tampermonkey, Greasemonkey, or other browser extension is required. The page
uses YouTube's embedded player API for playback and YouTube's oEmbed endpoint
for the video title and uploader/channel name. No Google API key is needed.

A normal web page cannot inspect playback in a separate `music.youtube.com`
tab. Music must play inside the Tiade player for this integration to report it.
Private playlists, account-specific queues, and videos that disallow embedding
are not supported. The reported artist is YouTube's uploader/channel name,
which may differ from the recording artist.

## Setup

KEY=d58685eea557d30eb41e04ffff4cb8d8f8715d0b9738325197133650293248bb

1. Generate a private token locally with `openssl rand -hex 32`. Set the
   `YOUTUBEMUSIC_TOKEN` environment variable in the server's launch environment,
   or add `YOUTUBEMUSIC_TOKEN=your-token` to the existing local `server.env`
   loaded by `start.sh`, then rebuild and restart the server. Never commit your real token. A missing
   or empty server token disables this endpoint with HTTP 503.
2. Open **https://stimky.info/youtubemusic/player**. Enter a music username
   such as `infinivaeria` and your music token, then click **Connect**. You can
   paste the full `YOUTUBEMUSIC_TOKEN=...` line or just its value. The page
   verifies your token before connecting and displays errors directly.
3. Paste a public YouTube or YouTube Music video/playlist URL and click
   **Load**. Use the embedded player to start playback if the browser blocks
   autoplay. Keep this page open. Credentials are remembered only in the
   current tab's session storage; **Disconnect** removes them and clears the
   feed. Disable the old userscript so it cannot overwrite the feed.
4. Put [src/youtubemusic.lsl](src/youtubemusic.lsl) into a Second Life object's
   script. It opens text boxes for the same music username and token; there is
   no need to edit script constants. It saves settings in the object's linkset
   data across script resets and clears them on ownership changes. Only the
   owner can configure it. Touch the object for **Music user**, **Token**, or
   **Refresh**. It polls every ten seconds and prints song/status changes
   privately using `llOwnerSay`.

The LSL route remains `https://stimky.info/youtubemusic`; no change to its
request format is needed. Use the same username and token in the LSL object
and the hosted player. A username is a case-insensitive feed label of 1-40
letters, numbers, dots, underscores or hyphens, not a Google login.

## Player Navigation

The Tiade home page and quick navigation link to **YouTube Music**. Within the
player, **Queue**, **Saved**, and **Recent** switch between your stored music
lists without hiding the video. **Search YouTube Music** opens YouTube Music
search in a separate tab; copy a public video or playlist URL into the Tiade
player to play it here. Search is not an embedded account/library integration.

Transport controls include play, pause, stop, previous/next, seek, volume,
mute, shuffle, and repeat (off, track, or queue). Individual videos follow the
local queue; public YouTube playlists use the embedded player's playlist
navigation. Queues support reordering and removal. **Save queue** stores a
named queue, and **Recent** records up to 50 played tracks. These lists and the
player settings are saved on the Rust server, separately for each music username.
They do not sync to your Google account. Playback and metadata updates still go to LSL.

## Partitioned Storage

The Rust `partitioned_array_rust::PartitionedArray` stores one record per music
username: the current queue, named saved queues, recent tracks, volume, mute,
shuffle, repeat, selected queue entry, resume position, and last reported song.
Connect loads that username's library from the server, so another browser can
use the same lists. Playback does not automatically start after loading; press
Play to resume the selected entry. Position is checkpointed with playback
updates, normally every ten seconds. Embedded playlist resume positions are
not restored.

The default snapshot is `youtube_music_data/music.json` inside the Rust app
directory. Set `YOUTUBEMUSIC_DATA_PATH` in the server launch environment to use
another file path. Back up this file and keep it on persistent storage. The
directory is ignored by Git. Writes use the existing atomic snapshot helper;
failed writes do not replace the in-memory array, and a corrupt snapshot causes
startup to fail rather than silently resetting your library.

The player shows library save status. **Save library** retries failed saves.
Conflicting edits from another tab return HTTP 409; **Reload library** loads the
server version after confirmation, discarding local unsaved edits. Use one
active editing tab per username. **Import browser lists** explicitly imports
queues/history saved by the older browser-local player, with confirmation
before replacing populated lists. Old browser lists are not deleted.

`GET /youtubemusic/library` returns `{username, revision, library, playback,
updated_at}`. `PUT /youtubemusic/library` accepts `{revision, library}` and
returns the updated record. Both use the same token and username headers as the
feed. A changed library increments its revision; playback updates do not.
Library bodies are limited to 1 MB, with at most 100 queue entries, 30 saved
queues of 100 entries each, and 50 recent entries. Tokens are never stored in
music records; browser credentials remain in tab session storage only.

This is not full access to YouTube Music. YouTube's public embedded API does
not expose your private library, account history, recommendations, purchases,
or subscription-only content. No general-purpose programming feature or
"Turing completeness" changes those service permissions.

## Feed API

Both `GET /youtubemusic` and `POST /youtubemusic` require the
`X-YouTubeMusic-Token` header. Clients select a feed using
`X-YouTubeMusic-Username`; older clients without that header use `default`.
GET returns:

```json
{"username":"alice","title":"Song","artist":"Artist","is_playing":true,"url":"https://music.youtube.com/watch?v=abcdefghijk","stale":false}
```

The player posts `title`, `artist`, `video_id`, and `is_playing`. Paused
tracks retain their metadata with `is_playing: false`. After 45 seconds without
updates, GET returns empty metadata, `is_playing: false`, and `stale: true`.
The server persists the last report, but live freshness timers remain in memory.
After a restart the feed is stale until a new playback report arrives; loading
saved metadata does not claim that music is currently playing. Usernames separate
feeds and libraries but are not authentication: anyone with the shared server
token can read or replace any feed or library. Keep it in
scripts and objects you control, and use HTTPS. Other scripts in the same
Second Life linkset can access saved linkset settings, including the token.

The page sends playback changes and a heartbeat every ten seconds. Browser
sleep or suspended tabs can temporarily expire the feed. Use one publisher
per username; multiple pages or old userscripts can overwrite each other.
Connection, metadata, publishing, and YouTube playback errors appear on the
page. Closing the tab expires the feed after 45 seconds.

`GET /youtubemusic/player` serves the public player page without embedding any
server secret. `GET /youtubemusic/metadata?video_id=...` requires the music token
and requests metadata only for a validated YouTube video ID. Video URLs are
never arbitrary server-side fetch targets.

## Checks

```sh
RUSTFLAGS='--cfg rustix_use_libc' cargo test youtube_music::tests
node --test tests/youtubemusic_tests.cjs
```

The Rust flag works around the current toolchain's existing `rustix` build issue.
The automated browser tests mock YouTube's embedded player and feed/library APIs.
Rust tests cover restart recovery, failed-save rollback, corrupt snapshots,
validation, token enforcement, username isolation, and revision conflicts.
Verify real playback and LSL compilation in your browser and Second Life after
configuration. The old userscript remains available for legacy clients but
is not required or recommended for the hosted-player workflow.