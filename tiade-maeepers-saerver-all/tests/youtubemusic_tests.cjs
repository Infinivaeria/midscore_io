const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { test } = require('node:test');
const { webcrypto, createHash } = require('node:crypto');

const source = fs.readFileSync(path.join(__dirname, '../scripts/youtubemusic.user.js'), 'utf8');
const playerHtml = fs.readFileSync(path.join(__dirname, '../src/youtube_music_player.html'), 'utf8');
const playerSource = playerHtml.match(/<script id="music-player-code">([\s\S]*?)<\/script>/)[1];

function lslPlayback() {
    const lsl = fs.readFileSync(path.join(__dirname, '../src/youtubemusic.lsl'), 'utf8');
    const handler = lsl.slice(lsl.indexOf('    http_response('))
        .replace(/http_response\(key request_id, integer status, list metadata, string body\)/,
            'function http_response(request_id, status, metadata, body)')
        .replace(/\b(?:string|integer) (\w+) =/g, 'let $1 =')
        .replace(/\(string\)status/g, 'String(status)')
        .replace(/\n}\s*$/, '');
    const messages = [];
    let now = 100;
    const context = {
        pending_request: 'request', NULL_KEY: null, empty_playback_since: -1,
        EMPTY_PLAYBACK_SECONDS: Number(lsl.match(/EMPTY_PLAYBACK_SECONDS = (\d+)/)[1]),
        JSON_TRUE: true, JSON_FALSE: false, JSON_STRING: 'string',
        llGetUnixTime: () => now,
        llJsonGetValue: (body, keys) => JSON.parse(body)[keys[0]],
        llJsonValueType: (body, keys) => typeof JSON.parse(body)[keys[0]],
        announce(message) {
            if (messages.at(-1) !== message) messages.push(message);
        },
    };
    vm.createContext(context);
    vm.runInContext(handler, context);
    return { messages, send(seconds, playback = {}, status = 200) {
        now = 100 + seconds;
        context.pending_request = 'request';
        context.http_response('request', status, [], JSON.stringify({
            stale: false, is_playing: true, title: 'Song', artist: 'Artist', ...playback,
        }));
    } };
}

test('LSL ignores brief empty playback without reannouncing the continuing song', () => {
    const client = lslPlayback();
    client.send(0);
    client.send(1, { title: '', is_playing: false });
    client.send(2, { title: '', is_playing: false });
    client.send(3);
    assert.deepEqual(client.messages, ['Now playing: Song - Artist']);
    client.send(4, { title: 'Next song' });
    client.send(5, { title: 'Next song', is_playing: false });
    assert.deepEqual(client.messages.slice(1), ['Now playing: Next song - Artist', 'Paused: Next song - Artist']);
});

test('LSL reports sustained empty playback and resets the grace period after recovery', () => {
    const client = lslPlayback();
    client.send(0);
    client.send(1, { title: '' });
    client.send(15, { title: '' });
    assert.equal(client.messages.length, 1);
    client.send(16, { title: '' });
    assert.equal(client.messages.at(-1), 'Nothing playing on YouTube Music.');
    client.send(17);
    client.send(18, { title: '' });
    client.send(19);
    assert.equal(client.messages.length, 3);
    assert.equal(client.messages.at(-1), 'Now playing: Song - Artist');
});

test('LSL still reports stale feeds and HTTP errors immediately', () => {
    const client = lslPlayback();
    client.send(0);
    client.send(1, { title: '' });
    client.send(2, { stale: true, title: '' });
    assert.match(client.messages.at(-1), /No recent update/);
    client.send(3, {}, 401);
    assert.match(client.messages.at(-1), /token was rejected/);
});

function hostedPlayer({ metadataFails = false, lists = new Map(), records = new Map(), failure = {}, remembered = {} } = {}) {
    const elements = new Map();
    const requests = [];
    const saved = new Map();
    const loads = [];
    const embed = { videoId: '', state: -1, volume: 70, muted: false, currentTime: 0, duration: 240,
        playlist: ['abcdefghijk', 'lmnopqrstuv'], playlistIndex: 0 };
    const makeElement = value => ({ value: value || '', textContent: '', disabled: false, children: [],
        attributes: {}, events: {}, setAttribute(name, value) { this.attributes[name] = value; },
        getAttribute(name) { return this.attributes[name]; },
        addEventListener(event, handler) { this.events[event] = handler; },
        appendChild(child) { this.children.push(child); }, replaceChildren() { this.children = []; } });
    const element = id => {
        if (!elements.has(id)) elements.set(id, makeElement(id === 'volume' ? '70' : id === 'repeat' ? 'off' : ''));
        return elements.get(id);
    };
    let tick, playerEvents;
    const context = {
        document: { getElementById: element, createElement: () => makeElement(), addEventListener() {} },
        window: {}, location: { origin: 'https://stimky.info', href: 'https://stimky.info/youtubemusic/player' },
        URL, AbortSignal,
        sessionStorage: { getItem: key => saved.get(key), setItem: (key, value) => saved.set(key, value), removeItem: key => saved.delete(key) },
        localStorage: { getItem: key => lists.get(key), setItem: (key, value) => lists.set(key, value) },
        prompt: () => 'Favorites', confirm: () => true,
        setInterval: handler => { tick = handler; },
        fetch: async (url, options) => {
            requests.push({ url, ...options });
            let status = options.headers['X-YouTubeMusic-Token'] !== 'test-token' ? 401
                : metadataFails && url.includes('/metadata?') ? 502 : 200;
            if (url === '/youtubemusic/session') {
                if (!options.method) status = 200;
                if (status === 200 && options.method === 'POST') remembered.current = {
                    username: options.headers['X-YouTubeMusic-Username'], token: options.headers['X-YouTubeMusic-Token'],
                };
                if (status === 200 && options.method === 'DELETE') remembered.current = null;
                return { ok: status === 200, status, json: async () => remembered.current || null };
            }
            let record;
            if (status === 200 && url === '/youtubemusic/library') {
                const owner = options.headers['X-YouTubeMusic-Username'];
                record = records.get(owner) || { revision: 0, library: { queue: [], saved: [], history: [], settings: {
                    volume: 70, muted: false, shuffle: false, repeat: 'off', queue_index: -1, position_seconds: 0,
                } }, playback: {} };
                if (options.method === 'PUT') {
                    const update = JSON.parse(options.body);
                    status = failure.status || (update.revision !== record.revision ? 409 : 200);
                    if (status === 200) { record = { ...record, revision: record.revision + 1, library: update.library }; records.set(owner, record); }
                }
            }
            return { ok: status === 200, status, json: async () => url.includes('/metadata?')
                ? { title: `Song ${new URL(url, 'https://stimky.info').searchParams.get('video_id')}`, artist: 'YouTube channel' }
                : record ? JSON.parse(JSON.stringify(record)) : { stale: false } };
        },
        YT: { Player: function (id, options) {
            playerEvents = options.events;
            return {
                getVideoUrl: () => `https://www.youtube.com/watch?v=${embed.videoId}`,
                getPlayerState: () => embed.state,
                getPlaylist: () => embed.playlist,
                getPlaylistIndex: () => embed.playlistIndex,
                getDuration: () => embed.duration,
                getCurrentTime: () => embed.currentTime,
                setVolume: value => { embed.volume = value; },
                mute: () => { embed.muted = true; }, unMute: () => { embed.muted = false; }, isMuted: () => embed.muted,
                setShuffle: value => { embed.shuffled = value; }, setLoop: value => { embed.loop = value; },
                seekTo: value => { embed.currentTime = value; },
                playVideo: () => { embed.state = 1; }, stopVideo: () => { embed.state = 0; },
                nextVideo: () => { embed.next = true; }, previousVideo: () => { embed.previous = true; },
                loadVideoById(videoId) { loads.push({ videoId }); embed.videoId = videoId; embed.state = 1; },
                loadPlaylist(value) { loads.push(value); embed.videoId = 'abcdefghijk'; embed.state = 1; },
                pauseVideo() { embed.state = 2; },
            };
        } },
    };
    vm.runInNewContext(playerSource, context);
    context.window.onYouTubeIframeAPIReady();
    playerEvents.onReady();
    const submit = id => element(id).events.submit({ preventDefault() {} });
    return { element, requests, saved, loads, embed, submit, lists, records, tick: () => tick(),
        flush: () => element('retryLibrary').events.click(),
        latestPlayback: () => requests.filter(request => request.method === 'POST' && request.url === '/youtubemusic').at(-1),
        playerError: code => playerEvents.onError({ data: code }),
        stateChange: state => { embed.state = state; playerEvents.onStateChange({ data: state }); } };
}

function browser({ data, video, artist = '', title = '', bar, playerThrows = false,
    href = 'https://music.youtube.com/', token = 'test-token', username = 'default', answers = [],
    stored = { 'music-token': token, 'music-username': username } } = {}) {
    const requests = [];
    const events = {};
    const menus = {};
    const prompts = [];
    const alerts = [];
    const elements = [];
    const timers = new Map();
    const aborted = [];
    let nextTimer = 0;
    let tick;
    vm.runInNewContext(source, {
        unsafeWindow: {
            document: { getElementById: () => ({ getVideoData: () => {
                if (playerThrows) throw new Error('Unavailable player');
                return data;
            } }) },
            navigator: { mediaSession: { metadata: { artist, title } } },
            location: { href },
        },
        document: {
            createElement: tag => ({ tag, style: {}, events: {},
                addEventListener(event, handler) { this.events[event] = handler; } }),
            documentElement: { appendChild: element => elements.push(element) },
            querySelector: selector => selector === 'video' ? video : bar,
            addEventListener: (event, handler) => { events[event] = handler; },
        },
        GM_xmlhttpRequest: request => {
            requests.push(request);
            return { abort() { aborted.push(request); request.onabort(); } };
        },
        GM_getValue: (name, fallback) => stored[name] ?? fallback,
        GM_setValue: (name, value) => { stored[name] = value; },
        GM_registerMenuCommand: (name, handler) => { menus[name] = handler; },
        prompt: (message, initial) => { prompts.push({ message, initial }); return answers.length ? answers.shift() : null; },
        alert: message => alerts.push(message),
        URL,
        crypto: webcrypto,
        TextEncoder,
        setInterval: handler => { tick = handler; },
        setTimeout: (handler, delay) => { const timer = ++nextTimer; timers.set(timer, { handler, delay }); return timer; },
        clearTimeout: timer => timers.delete(timer),
        console: { warn() {} },
    });
    return { requests, events, tick, menus, prompts, alerts, stored, elements, timers, aborted };
}

test('publishes active playback and authenticated requests without cookies', () => {
    const video = { paused: false, ended: false, readyState: 4 };
    const client = browser({ data: { title: 'Song', author: 'Channel', video_id: 'abcdefghijk' }, video, artist: 'Artist' });
    assert.deepEqual(JSON.parse(client.requests[0].data), {
        title: 'Song', artist: 'Artist', video_id: 'abcdefghijk', is_playing: true,
    });
    assert.equal(client.requests[0].headers['X-YouTubeMusic-Token'], 'test-token');
    assert.equal(client.requests[0].headers['X-YouTubeMusic-Username'], 'default');
    assert.equal(client.requests[0].anonymous, true);
    client.tick();
    assert.equal(client.requests.length, 1);
    client.requests[0].onload({ status: 200 });
    video.paused = true;
    client.events.pause();
    assert.equal(JSON.parse(client.requests[1].data).is_playing, false);
});

test('clears absent playback and retries after network errors', () => {
    const client = browser();
    assert.deepEqual(JSON.parse(client.requests[0].data), {
        title: '', artist: '', video_id: '', is_playing: false,
    });
    client.requests[0].onerror();
    client.tick();
    assert.equal(client.requests.length, 2);
    client.requests[1].ontimeout();
    client.tick();
    assert.equal(client.requests.length, 3);
});

test('bounds Unicode metadata without splitting characters', () => {
    const client = browser({
        data: { title: '\u{1f3b5}'.repeat(200), author: 'a'.repeat(120), video_id: 'abcdefghijk' },
        video: { paused: false, ended: true, readyState: 4 },
    });
    const payload = JSON.parse(client.requests[0].data);
    assert.equal(Array.from(payload.title).length, 160);
    assert.equal(payload.artist.length, 100);
    assert.equal(payload.is_playing, false);
});

test('does not publish until a token is configured', () => {
    const client = browser({ token: '' });
    client.tick();
    assert.equal(client.requests.length, 0);
});

test('first-run prompts save normalized settings and reuse them after reload', () => {
    const client = browser({ stored: {}, answers: [' Alice.Music ', ' new-token '] });
    assert.equal(client.prompts.length, 2);
    assert.equal(client.stored['music-username'], 'alice.music');
    assert.equal(client.stored['music-token'], 'new-token');
    assert.equal(client.requests[0].headers['X-YouTubeMusic-Username'], 'alice.music');
    const reloaded = browser({ stored: client.stored });
    assert.equal(reloaded.prompts.length, 0);
    assert.equal(reloaded.requests[0].headers['X-YouTubeMusic-Token'], 'new-token');
});

test('menu setup changes username without exposing or re-entering the saved token', () => {
    const answers = ['new-user', ''];
    const client = browser({ answers });
    client.requests[0].onload({ status: 200 });
    client.menus['Music setup']();
    assert.equal(client.stored['music-username'], 'new-user');
    assert.equal(client.stored['music-token'], 'test-token');
    assert.equal(client.prompts[1].initial, '');
    assert.equal(client.requests[1].headers['X-YouTubeMusic-Username'], 'new-user');
});

test('cancelled or invalid setup does not partially save settings', () => {
    for (const answers of [['new-user', null], ['bad name'], ['valid-name', 'bad token']]) {
        const stored = {};
        const client = browser({ stored, answers });
        assert.deepEqual(stored, {});
        assert.equal(client.requests.length, 0);
    }
});

test('falls back to Media Session and watch URL when the player API is unavailable', () => {
    const client = browser({
        playerThrows: true, title: 'Fallback song', artist: 'Fallback artist',
        href: 'https://music.youtube.com/watch?v=abcdefghijk',
        video: { paused: false, ended: false, readyState: 4 },
    });
    assert.deepEqual(JSON.parse(client.requests[0].data), {
        title: 'Fallback song', artist: 'Fallback artist', video_id: 'abcdefghijk', is_playing: true,
    });
});

test('reads the visible player bar when the player API and Media Session have no title', () => {
    const values = { '.title': { textContent: 'Visible song' },
        '.byline a': { textContent: 'Visible artist' },
        'a[href*="watch?v="]': { href: 'https://music.youtube.com/watch?v=abcdefghijk' } };
    const client = browser({
        bar: { querySelector: selector => values[selector] },
        video: { paused: false, ended: false, readyState: 4 },
    });
    assert.deepEqual(JSON.parse(client.requests[0].data), {
        title: 'Visible song', artist: 'Visible artist', video_id: 'abcdefghijk', is_playing: true,
    });
});

test('status menu reports server acceptance and errors without exposing the token', async () => {
    const client = browser({ username: 'infinivaeria' });
    client.requests[0].onload({ status: 401 });
    await client.menus['Music status']();
    assert.match(client.alerts[0], /infinivaeria/);
    assert.match(client.alerts[0], /HTTP 401/);
    assert.match(client.alerts[0], /Music bridge v1.5.0/);
    assert.match(client.alerts[0], /Token length: 10/);
    assert.ok(client.alerts[0].includes(createHash('sha256').update('test-token').digest('hex').slice(0, 12)));
    assert.ok(!client.alerts[0].includes('test-token'));
    client.menus['Publish now']();
    assert.equal(client.requests.length, 2);
    client.requests[1].onload({ status: 200 });
    await client.menus['Music status']();
    assert.match(client.alerts[1], /Server accepted update/);
    assert.match(client.alerts[1], /No song detected/);
});

test('visible Music status button opens the same diagnostics without exposing the token', async () => {
    const client = browser({ username: 'infinivaeria' });
    const button = client.elements.find(element => element.id === 'second-life-music-status');
    assert.equal(button.tag, 'button');
    assert.equal(button.textContent, 'Music status');
    assert.equal(button.style.position, 'fixed');
    client.requests[0].onload({ status: 401 });
    await button.events.click();
    assert.match(client.alerts[0], /infinivaeria/);
    assert.match(client.alerts[0], /HTTP 401/);
    assert.ok(!client.alerts[0].includes('test-token'));
    await client.menus['Music status']();
    assert.equal(client.alerts[0], client.alerts[1]);
});

test('watchdog releases a request with no callbacks and ignores its late completion', async () => {
    const client = browser();
    const watchdog = Array.from(client.timers.values())[0];
    assert.equal(watchdog.delay, 12000);
    watchdog.handler();
    assert.equal(client.aborted.length, 1);
    assert.equal(client.timers.size, 0);
    await client.menus['Music status']();
    assert.match(client.alerts[0], /No response from Tampermonkey/);
    client.tick();
    assert.equal(client.requests.length, 2);
    client.requests[0].onload({ status: 200 });
    client.requests[0].onerror();
    client.tick();
    assert.equal(client.requests.length, 2);
    await client.menus['Music status']();
    assert.match(client.alerts[1], /Sending an update/);
    assert.match(client.alerts[1], /No response from Tampermonkey/);
    client.requests[1].onload({ status: 200 });
    await client.menus['Music status']();
    assert.match(client.alerts[2], /Server accepted update/);
    assert.equal(client.timers.size, 0);
});

test('aborted requests release the publisher and preserve a useful status on retry', async () => {
    const client = browser();
    client.requests[0].onabort();
    assert.equal(client.timers.size, 0);
    client.menus['Publish now']();
    assert.equal(client.requests.length, 2);
    await client.menus['Music status']();
    assert.match(client.alerts[0], /Request was aborted/);
    assert.match(client.alerts[0], /Sending an update/);
});

test('setup extracts tokens from pasted environment assignments and quoted values', () => {
    for (const input of ['YOUTUBEMUSIC_TOKEN=new-token', 'YOUTUBEMUSIC_TOKEN="new-token"', "'new-token'"]) {
        const client = browser({ stored: {}, answers: ['infinivaeria', input] });
        assert.equal(client.stored['music-token'], 'new-token');
        assert.equal(client.requests[0].headers['X-YouTubeMusic-Token'], 'new-token');
    }
});

test('hosted player validates credentials and publishes song metadata and pause state', async () => {
    const client = hostedPlayer();
    client.element('username').value = ' InfiniVaeria ';
    client.element('token').value = 'YOUTUBEMUSIC_TOKEN="test-token"';
    await client.submit('connectionForm');
    assert.equal(client.element('connectionStatus').textContent, 'Connected as infinivaeria');
    assert.equal(client.element('load').disabled, false);
    client.element('source').value = 'https://music.youtube.com/watch?v=abcdefghijk';
    client.submit('sourceForm');
    await client.tick();
    const post = client.latestPlayback();
    assert.equal(post.url, '/youtubemusic');
    assert.equal(post.headers['X-YouTubeMusic-Username'], 'infinivaeria');
    assert.deepEqual(JSON.parse(post.body), { title: 'Song abcdefghijk', artist: 'YouTube channel', video_id: 'abcdefghijk', is_playing: true });
    assert.equal(client.element('playbackState').textContent, 'Playing');
    client.embed.state = 2;
    await client.tick();
    assert.equal(JSON.parse(client.latestPlayback().body).is_playing, false);
    assert.equal(client.requests.filter(request => request.url.includes('/metadata?')).length, 1);
    client.embed.videoId = 'lmnopqrstuv';
    await client.tick();
    assert.equal(JSON.parse(client.latestPlayback().body).title, 'Song lmnopqrstuv');
    await client.element('disconnect').events.click();
    assert.equal(client.saved.size, 0);
    assert.equal(client.element('token').value, '');
    assert.equal(JSON.parse(client.requests.at(-1).body).title, '');
});

test('hosted player accepts public playlists and rejects non-YouTube URLs', async () => {
    const client = hostedPlayer();
    client.element('username').value = 'music';
    client.element('token').value = 'test-token';
    await client.submit('connectionForm');
    client.element('source').value = 'https://music.youtube.com/playlist?list=PLmVg8htmFFWIYlBWi26oy--oBwKXryjK1';
    client.submit('sourceForm');
    assert.equal(client.loads[0].listType, 'playlist');
    assert.equal(client.loads[0].list, 'PLmVg8htmFFWIYlBWi26oy--oBwKXryjK1');
    for (const url of ['https://evil.example/watch?v=abcdefghijk', 'javascript:alert(1)', 'https://www.youtube.com/watch?v=bad']) {
        client.element('source').value = url;
        client.submit('sourceForm');
        assert.notEqual(client.element('error').textContent, '');
    }
    assert.equal(client.loads.length, 1);
});

test('playlist boundaries advance between queued playlists and preserve internal playback', async () => {
    const client = hostedPlayer();
    client.element('username').value = 'music'; client.element('token').value = 'test-token';
    await client.submit('connectionForm');
    const first = 'PLfirstplaylist', second = 'PLsecondplaylist';
    client.element('source').value = `https://music.youtube.com/playlist?list=${first}`;
    client.submit('sourceForm');
    client.element('source').value = `https://music.youtube.com/playlist?list=${second}`;
    client.element('enqueue').events.click();
    client.stateChange(0);
    assert.equal(client.loads.length, 1);
    assert.equal(client.embed.next, undefined);
    client.element('next').events.click();
    assert.equal(client.embed.next, true);
    assert.equal(client.loads.length, 1);
    client.embed.playlistIndex = 1;
    client.stateChange(0);
    assert.equal(client.loads.at(-1).list, second);
    client.embed.playlistIndex = 0;
    client.element('previous').events.click();
    assert.equal(client.loads.at(-1).list, first);
    client.embed.playlistIndex = 1;
    client.element('next').events.click();
    assert.equal(client.loads.at(-1).list, second);
    client.stateChange(0);
    assert.equal(client.loads.length, 4);
    await client.flush();
    assert.equal(client.records.get('music').library.settings.queue_index, 1);
});

test('playlist completion respects Stop, repeat track, and repeat queue', async () => {
    const client = hostedPlayer();
    client.element('username').value = 'music'; client.element('token').value = 'test-token';
    await client.submit('connectionForm');
    for (const list of ['PLfirstplaylist', 'PLsecondplaylist']) {
        client.element('source').value = `https://music.youtube.com/playlist?list=${list}`;
        client.submit('sourceForm');
    }
    client.element('repeat').value = 'queue'; client.element('repeat').events.change();
    assert.equal(client.embed.loop, false);
    client.embed.playlistIndex = 1;
    client.element('stop').events.click(); client.stateChange(0);
    assert.equal(client.loads.length, 2);
    client.element('play').events.click(); client.stateChange(0);
    assert.equal(client.loads.at(-1).list, 'PLfirstplaylist');
    assert.equal(client.embed.loop, false);
    client.element('repeat').value = 'track'; client.element('repeat').events.change();
    client.stateChange(0);
    assert.equal(client.loads.length, 3);
    assert.equal(client.embed.currentTime, 0);
    assert.equal(client.embed.state, 1);
});

test('hosted player shows rejected tokens and metadata failures without claiming success', async () => {
    const rejected = hostedPlayer();
    rejected.element('username').value = 'music';
    rejected.element('token').value = 'wrong-token';
    await rejected.submit('connectionForm');
    assert.match(rejected.element('error').textContent, /Token rejected/);
    assert.equal(rejected.saved.size, 0);
    const client = hostedPlayer({ metadataFails: true });
    client.element('username').value = 'music';
    client.element('token').value = 'test-token';
    await client.submit('connectionForm');
    client.element('source').value = 'abcdefghijk';
    client.submit('sourceForm');
    await client.tick();
    assert.match(client.element('publishStatus').textContent, /HTTP 502/);
    assert.equal(client.requests.filter(request => request.method === 'POST' && request.url === '/youtubemusic').length, 0);
});

test('hosted player restores server-saved username and key after reopening and forgets on disconnect', async () => {
    const remembered = {};
    const client = hostedPlayer({ remembered });
    client.element('username').value = 'alice';
    client.element('token').value = 'test-token';
    await client.submit('connectionForm');
    assert.deepEqual(remembered.current, { username: 'alice', token: 'test-token' });
    assert.equal(client.saved.size, 0);
    const reopened = hostedPlayer({ remembered });
    await new Promise(setImmediate);
    assert.equal(reopened.element('username').value, 'alice');
    assert.equal(reopened.element('token').value, 'test-token');
    assert.equal(reopened.element('connectionStatus').textContent, 'Connected as alice');
    assert.equal(reopened.loads.length, 0);
    await reopened.element('disconnect').events.click();
    assert.equal(remembered.current, null);
    const forgotten = hostedPlayer({ remembered });
    await new Promise(setImmediate);
    assert.equal(forgotten.element('token').value, '');
    assert.notEqual(forgotten.element('connectionStatus').textContent, 'Connected as alice');
});

test('hosted player never saves rejected credentials', async () => {
    const remembered = {};
    const client = hostedPlayer({ remembered });
    client.element('username').value = 'alice';
    client.element('token').value = 'wrong-token';
    await client.submit('connectionForm');
    assert.equal(remembered.current, undefined);
    assert.equal(client.requests.some(request => request.url === '/youtubemusic/session' && request.method === 'POST'), false);
});

test('hosted player transport controls operate on the embedded player', async () => {
    const client = hostedPlayer();
    client.element('username').value = 'music'; client.element('token').value = 'test-token';
    await client.submit('connectionForm');
    client.element('play').events.click(); assert.equal(client.embed.state, 1);
    client.element('pause').events.click(); assert.equal(client.embed.state, 2);
    client.element('volume').value = '35'; client.element('volume').events.input(); assert.equal(client.embed.volume, 35);
    client.element('seek').value = '87'; client.element('seek').events.change(); assert.equal(client.embed.currentTime, 87);
    client.element('mute').events.click(); assert.equal(client.embed.muted, true);
    client.element('mute').events.click(); assert.equal(client.embed.muted, false);
    client.element('shuffle').events.click(); assert.equal(client.embed.shuffled, true);
    client.element('repeat').value = 'queue'; client.element('repeat').events.change(); assert.equal(client.embed.loop, true);
    client.element('next').events.click(); assert.equal(client.embed.next, true);
    client.element('previous').events.click(); assert.equal(client.embed.previous, true);
});

test('server queue persists, reorders, removes and saves entries without storing credentials', async () => {
    const client = hostedPlayer();
    client.element('username').value = 'music'; client.element('token').value = 'test-token';
    await client.submit('connectionForm');
    for (const value of ['abcdefghijk', 'lmnopqrstuv']) {
        client.element('source').value = value; client.element('enqueue').events.click();
    }
    assert.equal(client.element('queueList').children.length, 2);
    let second = client.element('queueList').children[1];
    second.children.find(button => button.attributes['aria-label'] === 'Move up').events.click();
    await client.flush();
    assert.equal(client.records.get('music').library.queue[0].videoId, 'lmnopqrstuv');
    client.element('saveQueue').events.click();
    await client.flush();
    assert.equal(client.records.get('music').library.saved[0].name, 'Favorites');
    assert.equal(client.element('savedTab').attributes['aria-selected'], 'true');
    const reloaded = hostedPlayer({ records: client.records });
    reloaded.element('username').value = 'music'; reloaded.element('token').value = 'test-token';
    await reloaded.submit('connectionForm');
    assert.equal(reloaded.element('queueList').children.length, 2);
    const first = reloaded.element('queueList').children[0];
    first.children.find(button => button.attributes['aria-label'] === 'Remove').events.click();
    await reloaded.flush();
    assert.equal(client.records.get('music').library.queue.length, 1);
    assert.ok(!JSON.stringify([...client.records]).includes('test-token'));
    assert.equal(client.lists.size, 0);
});

test('manual queue next/previous and history follow selected videos', async () => {
    const client = hostedPlayer();
    client.element('username').value = 'music'; client.element('token').value = 'test-token';
    await client.submit('connectionForm');
    client.element('source').value = 'abcdefghijk'; client.submit('sourceForm');
    await client.tick();
    client.element('source').value = 'lmnopqrstuv'; client.element('enqueue').events.click();
    client.element('next').events.click(); assert.equal(client.embed.videoId, 'lmnopqrstuv');
    await client.tick();
    await client.flush();
    assert.equal(client.records.get('music').library.history[0].videoId, 'lmnopqrstuv');
    client.embed.currentTime = 0; client.element('previous').events.click();
    assert.equal(client.embed.videoId, 'abcdefghijk');
    client.element('clearHistory').events.click(); await client.flush();
    assert.equal(client.records.get('music').library.history.length, 0);
});

test('Stop does not advance the queue, while natural completion does', async () => {
    const client = hostedPlayer();
    client.element('username').value = 'music'; client.element('token').value = 'test-token';
    await client.submit('connectionForm');
    client.element('source').value = 'abcdefghijk'; client.submit('sourceForm');
    await client.tick();
    client.element('source').value = 'lmnopqrstuv'; client.element('enqueue').events.click();
    client.element('stop').events.click(); client.stateChange(0);
    assert.equal(client.embed.videoId, 'abcdefghijk');
    client.element('play').events.click(); client.stateChange(0);
    assert.equal(client.embed.videoId, 'lmnopqrstuv');
});

test('settings and position restore without autoplay, and usernames have isolated libraries', async () => {
    const client = hostedPlayer();
    client.element('username').value = 'alice'; client.element('token').value = 'test-token';
    await client.submit('connectionForm');
    client.element('source').value = 'abcdefghijk'; client.submit('sourceForm');
    client.element('volume').value = '32'; client.element('volume').events.input();
    client.element('mute').events.click(); client.element('shuffle').events.click();
    client.element('repeat').value = 'queue'; client.element('repeat').events.change();
    client.embed.currentTime = 78; await client.tick(); await client.flush();
    const restored = hostedPlayer({ records: client.records });
    restored.element('username').value = 'alice'; restored.element('token').value = 'test-token';
    await restored.submit('connectionForm');
    assert.equal(restored.loads.length, 0);
    assert.equal(restored.embed.volume, 32); assert.equal(restored.embed.muted, true);
    assert.equal(restored.embed.shuffled, true); assert.equal(restored.embed.loop, true);
    restored.element('play').events.click(); restored.stateChange(1);
    assert.equal(restored.embed.videoId, 'abcdefghijk'); assert.equal(restored.embed.currentTime, 78);
    await restored.tick(); await restored.flush();
    restored.element('username').value = 'bob'; await restored.submit('connectionForm');
    assert.equal(restored.element('queueList').children[0].textContent, 'Queue is empty');
    assert.equal(restored.embed.volume, 70); assert.equal(restored.embed.muted, false);
    assert.equal(client.records.get('alice').library.queue.length, 1);
});

test('failed saves retain edits for retry and stale tabs cannot overwrite server data', async () => {
    const failure = { status: 500 }, client = hostedPlayer({ failure });
    client.element('username').value = 'alice'; client.element('token').value = 'test-token';
    await client.submit('connectionForm');
    client.element('source').value = 'abcdefghijk'; client.element('enqueue').events.click();
    await client.flush(); assert.match(client.element('libraryStatus').textContent, /Not saved/);
    assert.equal(client.records.size, 0); assert.equal(client.element('queueList').children.length, 1);
    failure.status = 0; await client.flush();
    const stale = hostedPlayer({ records: client.records });
    stale.element('username').value = 'alice'; stale.element('token').value = 'test-token';
    await stale.submit('connectionForm');
    client.element('source').value = 'lmnopqrstuv'; client.element('enqueue').events.click(); await client.flush();
    stale.element('clearQueue').events.click(); await stale.flush();
    assert.match(stale.element('libraryStatus').textContent, /changed in another tab/);
    assert.equal(client.records.get('alice').library.queue.length, 2);
    await stale.element('reloadLibrary').events.click();
    assert.equal(stale.element('queueList').children.length, 2);
});

test('legacy browser lists import only on explicit request', async () => {
    const lists = new Map([['tiade-music-queue', JSON.stringify([{ videoId: 'abcdefghijk' }])]]);
    const client = hostedPlayer({ lists });
    client.element('username').value = 'alice'; client.element('token').value = 'test-token';
    await client.submit('connectionForm');
    assert.equal(client.element('queueList').children[0].textContent, 'Queue is empty');
    client.element('importLocal').events.click(); await client.flush();
    assert.equal(client.records.get('alice').library.queue[0].videoId, 'abcdefghijk');
});