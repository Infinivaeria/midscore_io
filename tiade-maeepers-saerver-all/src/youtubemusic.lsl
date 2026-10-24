string ROUTE_URL = "https://stimky.info/youtubemusic";
string music_token;
string music_username;
float POLL_SECONDS = 10.0;
integer EMPTY_PLAYBACK_SECONDS = 15;
integer empty_playback_since = -1;

key pending_request = NULL_KEY;
integer requested_at;
string last_message = "";
integer dialog_channel;
integer dialog_listener;
integer dialog_expires;
string input_mode = "";

close_dialog()
{
    if (dialog_listener) llListenRemove(dialog_listener);
    dialog_listener = 0;
    input_mode = "";
}

open_dialog(string mode)
{
    close_dialog();
    dialog_channel = -1000000 - (integer)llFrand(1000000000.0);
    dialog_listener = llListen(dialog_channel, "", llGetOwner(), "");
    dialog_expires = llGetUnixTime() + 120;
    input_mode = mode;
    if (mode == "username")
        llTextBox(llGetOwner(), "Music feed username (not your Google login). Use the same name on the Tiade music player.\n1-40 letters, numbers, dots, underscores or hyphens.", dialog_channel);
    else if (mode == "token")
        llTextBox(llGetOwner(), "Paste your music token. It must match the server and Tiade music player.\nLeave blank to keep the current token.", dialog_channel);
    else
        llDialog(llGetOwner(), "YouTube Music\nUsername: " + music_username + "\nToken saved: " + (string)(music_token != ""),
            ["Music user", "Token", "Refresh", "Close"], dialog_channel);
}

integer valid_username(string value)
{
    if (llStringLength(value) < 1 || llStringLength(value) > 40) return FALSE;
    integer index;
    for (index = 0; index < llStringLength(value); index++)
        if (llSubStringIndex("abcdefghijklmnopqrstuvwxyz0123456789_.-", llGetSubString(value, index, index)) < 0) return FALSE;
    return TRUE;
}

integer valid_token(string value)
{
    if (llStringLength(value) < 1 || llStringLength(value) > 256) return FALSE;
    integer index;
    for (index = 0; index < llStringLength(value); index++)
        if (llOrd(value, index) < 33 || llOrd(value, index) > 126) return FALSE;
    return TRUE;
}

save_setting(string name, string value)
{
    if (llLinksetDataWrite(name, value) != LINKSETDATA_OK)
        llOwnerSay("Could not save music settings in this object; they may be lost on reset.");
}

announce(string message)
{
    if (message != last_message)
    {
        last_message = message;
        llSetText(message, <1.0, 1.0, 1.0>, 1.0);
        llOwnerSay(llGetSubString(message, 0, 239));
    }
}

poll_song()
{
    if (music_token == "" || music_username == "")
    {
        announce("Touch this object to set your music username and token.");
        return;
    }
    if (pending_request != NULL_KEY && llGetUnixTime() - requested_at < 40) return;
    requested_at = llGetUnixTime();
    pending_request = llHTTPRequest(ROUTE_URL, [
        HTTP_METHOD, "GET",
        HTTP_CUSTOM_HEADER, "X-YouTubeMusic-Token", music_token,
        HTTP_CUSTOM_HEADER, "X-YouTubeMusic-Username", music_username,
        HTTP_BODY_MAXLENGTH, 4096,
        HTTP_PRAGMA_NO_CACHE, TRUE
    ], "");
    if (pending_request == NULL_KEY) announce("YouTube Music request was throttled; retrying shortly.");
}

default
{
    state_entry()
    {
        llSetText("YouTube Music\nWaiting for playback...", <1.0, 1.0, 1.0>, 1.0);
        if (llLinksetDataRead("ytm.owner") != (string)llGetOwner())
        {
            llLinksetDataDelete("ytm.token");
            llLinksetDataDelete("ytm.username");
            save_setting("ytm.owner", (string)llGetOwner());
        }
        music_token = llLinksetDataRead("ytm.token");
        music_username = llLinksetDataRead("ytm.username");
        llSetTimerEvent(POLL_SECONDS);
        if (music_username == "") open_dialog("username");
        else if (music_token == "") open_dialog("token");
        else poll_song();
    }

    on_rez(integer start_parameter)
    {
        llResetScript();
    }

    timer()
    {
        if (input_mode != "" && llGetUnixTime() >= dialog_expires) close_dialog();
        poll_song();
    }

    touch_start(integer count)
    {
        if (llDetectedKey(0) != llGetOwner()) return;
        if (music_username == "") open_dialog("username");
        else if (music_token == "") open_dialog("token");
        else open_dialog("menu");
    }

    changed(integer change)
    {
        if (change & CHANGED_OWNER)
        {
            llLinksetDataDelete("ytm.token");
            llLinksetDataDelete("ytm.username");
            llLinksetDataDelete("ytm.owner");
            llResetScript();
        }
    }

    listen(integer channel, string name, key speaker, string message)
    {
        if (speaker != llGetOwner() || channel != dialog_channel || input_mode == "") return;
        string mode = input_mode;
        close_dialog();
        if (mode == "menu")
        {
            if (message == "Music user") open_dialog("username");
            else if (message == "Token") open_dialog("token");
            else if (message == "Refresh")
            {
                last_message = "";
                poll_song();
            }
            return;
        }
        string value = llStringTrim(message, STRING_TRIM);
        if (value == "") return;
        if (mode == "username")
        {
            value = llToLower(value);
            if (!valid_username(value))
            {
                llOwnerSay("Invalid music username.");
                open_dialog("username");
                return;
            }
            music_username = value;
            save_setting("ytm.username", value);
        }
        else if (mode == "token")
        {
            if (!valid_token(value))
            {
                llOwnerSay("Invalid music token: use a token without spaces.");
                open_dialog("token");
                return;
            }
            music_token = value;
            save_setting("ytm.token", value);
        }
        pending_request = NULL_KEY;
        last_message = "";
        empty_playback_since = -1;
        if (music_token == "") open_dialog("token");
        else poll_song();
    }

    http_response(key request_id, integer status, list metadata, string body)
    {
        if (request_id != pending_request) return;
        pending_request = NULL_KEY;
        if (status != 200)
        {
            empty_playback_since = -1;
            if (status == 401) announce("Music token was rejected. Touch this object and choose Token.");
            else announce("YouTube Music route returned HTTP " + (string)status + ".");
            return;
        }
        string stale = llJsonGetValue(body, ["stale"]);
        string playing = llJsonGetValue(body, ["is_playing"]);
        if ((stale != JSON_TRUE && stale != JSON_FALSE)
            || (playing != JSON_TRUE && playing != JSON_FALSE)
            || llJsonValueType(body, ["title"]) != JSON_STRING
            || llJsonValueType(body, ["artist"]) != JSON_STRING)
        {
            empty_playback_since = -1;
            announce("YouTube Music route returned invalid playback JSON.");
            return;
        }
        if (stale == JSON_TRUE)
        {
            empty_playback_since = -1;
            announce("No recent update. Open stimky.info/youtubemusic/player and play a song.");
            return;
        }
        string title = llJsonGetValue(body, ["title"]);
        string artist = llJsonGetValue(body, ["artist"]);
        if (title == "")
        {
            integer now = llGetUnixTime();
            if (empty_playback_since == -1) empty_playback_since = now;
            if (now - empty_playback_since < EMPTY_PLAYBACK_SECONDS) return;
            announce("Nothing playing on YouTube Music.");
            return;
        }
        empty_playback_since = -1;
        string message = "Now playing: " + title;
        if (playing == JSON_FALSE) message = "Paused: " + title;
        if (artist != "") message += " - " + artist;
        announce(message);
    }
}