// ==UserScript==
// @name         YouTube Music to Second Life
// @namespace    midscore.io
// @version      1.5.0
// @description  Send the active YouTube Music song to your private playback bridge.
// @match        https://music.youtube.com/*
// @connect      stimky.info
// @grant        GM_xmlhttpRequest
// @grant        GM_getValue
// @grant        GM_setValue
// @grant        GM_registerMenuCommand
// @grant        unsafeWindow
// @run-at       document-idle
// @noframes
// ==/UserScript==

(() => {
    'use strict';

    const ROUTE_URL = 'https://stimky.info/youtubemusic';
    let token = GM_getValue('music-token', '');
    let username = GM_getValue('music-username', '');
    const clip = (text, length) => Array.from(String(text || '')).slice(0, length).join('');
    let pending = null;
    let lastStatus = 'No update attempted yet.';

    function readPlayback() {
        const video = document.querySelector('video');
        const bar = document.querySelector('ytmusic-player-bar');
        const metadata = unsafeWindow.navigator.mediaSession?.metadata;
        let data;
        try {
            data = unsafeWindow.document.getElementById('movie_player')?.getVideoData?.();
        } catch (error) {
            console.warn('YouTube player API unavailable; using player bar metadata.');
        }
        const validId = value => typeof value === 'string' && /^[A-Za-z0-9_-]{11}$/.test(value);
        const apiAvailable = Boolean(data?.title && validId(data.video_id));
        const title = apiAvailable ? data.title
            : bar?.querySelector('.title')?.textContent?.trim() || metadata?.title;
        const link = bar?.querySelector('a[href*="watch?v="]')?.href;
        const videoId = apiAvailable ? data.video_id
            : new URL(link || unsafeWindow.location.href).searchParams.get('v');
        const available = Boolean(video && title && validId(videoId));
        return {
            title: available ? clip(title, 160) : '',
            artist: available ? clip(metadata?.artist || data?.author
                || bar?.querySelector('.byline a')?.textContent?.trim(), 100) : '',
            video_id: available ? videoId : '',
            is_playing: available && !video.paused && !video.ended && video.readyState >= 2,
        };
    }

    function configured() {
        return /^[a-z0-9_.-]{1,40}$/.test(username) && /^[\x21-\x7e]{1,256}$/.test(token);
    }

    function setup() {
        const enteredName = prompt('Music username (a label you choose, not your Google login):', username || 'default');
        if (enteredName === null) return;
        const nextUsername = enteredName.trim().toLowerCase();
        if (!/^[a-z0-9_.-]{1,40}$/.test(nextUsername)) {
            alert('Use 1-40 letters, numbers, dots, underscores or hyphens for the music username.');
            return;
        }
        const enteredToken = prompt(token
            ? 'Music token (leave blank to keep the saved token):'
            : 'Paste the music token configured on your server:', '');
        if (enteredToken === null) return;
        let nextToken = enteredToken.trim();
        nextToken = nextToken.replace(/^YOUTUBEMUSIC_TOKEN\s*=\s*/, '');
        if ((nextToken.startsWith('"') && nextToken.endsWith('"'))
            || (nextToken.startsWith("'") && nextToken.endsWith("'"))) {
            nextToken = nextToken.slice(1, -1);
        }
        nextToken = nextToken || token;
        if (!/^[\x21-\x7e]{1,256}$/.test(nextToken)) {
            alert('Enter a music token without spaces (up to 256 characters).');
            return;
        }
        GM_setValue('music-username', nextUsername);
        GM_setValue('music-token', nextToken);
        username = nextUsername;
        token = nextToken;
        publish();
    }

    function finish(attempt, status) {
        if (pending !== attempt) return false;
        clearTimeout(attempt.watchdog);
        pending = null;
        lastStatus = status;
        return true;
    }

    function publish() {
        if (pending || !configured()) return;
        let attempt;
        try {
            const playback = readPlayback();
            attempt = {};
            pending = attempt;
            attempt.watchdog = setTimeout(() => {
                if (finish(attempt, 'No response from Tampermonkey after 12 seconds. Check for a permission prompt for stimky.info and allow that connection, then try Publish now.')) {
                    attempt.handle?.abort?.();
                }
            }, 12000);
            attempt.handle = GM_xmlhttpRequest({
                method: 'POST',
                url: ROUTE_URL,
                headers: {
                    'Content-Type': 'application/json',
                    'X-YouTubeMusic-Token': token,
                    'X-YouTubeMusic-Username': username,
                },
                data: JSON.stringify(playback),
                anonymous: true,
                timeout: 8000,
                onload(response) {
                    const status = response.status === 200
                        ? `Server accepted update at ${new Date().toLocaleTimeString()}.\nPlayback: ${playback.title || 'No song detected; keep a song open in this tab.'}`
                        : `Server rejected update: HTTP ${response.status}. Check the token and server URL in Music setup.`;
                    if (!finish(attempt, status)) return;
                    if (response.status !== 200) console.warn('YouTube Music bridge HTTP status:', response.status,
                        'Use Tampermonkey > Music setup to check your settings.');
                },
                onerror() {
                    if (!finish(attempt, 'Network request failed. Check server access and Tampermonkey permission for stimky.info.')) return;
                    console.warn('YouTube Music bridge request failed.');
                },
                ontimeout() {
                    if (!finish(attempt, 'Server request timed out. Check the connection and try Publish now.')) return;
                    console.warn('YouTube Music bridge request timed out.');
                },
                onabort() {
                    finish(attempt, 'Request was aborted. Check Tampermonkey permission for stimky.info, then try Publish now.');
                },
            });
        } catch (error) {
            const status = `Could not read or send playback: ${error.message}`;
            if (attempt) finish(attempt, status);
            else lastStatus = status;
            console.warn('YouTube Music bridge could not read playback:', error.message);
        }
    }

    async function showStatus() {
        const savedToken = token;
        let fingerprint = 'not set';
        if (savedToken) {
            try {
                const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(savedToken));
                fingerprint = Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('').slice(0, 12);
            } catch (error) {
                fingerprint = 'unavailable in this browser';
            }
        }
        alert(`Music bridge v1.5.0\nMusic feed: ${username || '(not set)'}\nServer: ${ROUTE_URL}\nToken length: ${savedToken.length}\nToken fingerprint: ${fingerprint}\n${pending ? 'Sending an update...\n' : ''}${configured() ? lastStatus : 'Not configured. Choose Music setup.'}`);
    }

    GM_registerMenuCommand('Music setup', setup);
    GM_registerMenuCommand('Music status', showStatus);
    GM_registerMenuCommand('Publish now', publish);
    const statusButton = document.createElement('button');
    statusButton.id = 'second-life-music-status';
    statusButton.type = 'button';
    statusButton.textContent = 'Music status';
    statusButton.title = 'Second Life music publishing status';
    Object.assign(statusButton.style, {
        position: 'fixed', right: '16px', bottom: '100px', zIndex: '2147483647',
        width: '140px', maxWidth: 'calc(100vw - 32px)', height: '40px',
        background: '#212121', color: '#ffffff', border: '1px solid #777777',
        borderRadius: '4px', font: '14px sans-serif', cursor: 'pointer',
    });
    statusButton.addEventListener('click', showStatus);
    document.documentElement.appendChild(statusButton);
    document.addEventListener('play', publish, true);
    document.addEventListener('pause', publish, true);
    document.addEventListener('ended', publish, true);
    document.addEventListener('yt-navigate-finish', publish);
    setInterval(publish, 10000);
    if (configured()) publish();
    else setup();
})();