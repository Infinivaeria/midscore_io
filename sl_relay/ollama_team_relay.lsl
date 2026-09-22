// Second Life LSL relay for the unified Rust Ollama relay backend.
// Backend contract:
//   POST  https://stimky.info/chat/<team>    body: {"message":"..."}
//   GET   https://stimky.info/history/<team>
//   POST  https://stimky.info/sl_logger      body: one log entry per line
//
// The Ruby backend already concatenates per-team history and the live
// Second Life text chat log on every request. This script focuses on
// reliable message delivery, in-world controls, and readable replies.

string BASE_URL = "https://stimky.info";
string RELAY_VERSION = "2026-07-31c";
string TEAM_NAME = "secondlife";
integer CONTROL_CHANNEL = -77553311;
integer ALLOW_GROUP_ACCESS = FALSE;
integer AUTO_RELAY_LOCAL_CHAT = FALSE;
integer SPEAK_REPLIES_PUBLICLY = FALSE;
integer INCLUDE_WORLD_CONTEXT = TRUE;
integer ENABLE_SL_LOG_MIRROR = TRUE;
integer DEBUG_MODE = TRUE;
integer MAX_RETRIES = 3;
float REQUEST_TIMEOUT = 90.0;
integer MAX_OUTPUT_CHARS = 900;

integer QUEUE_STRIDE = 2;
integer REQUEST_KIND_CHAT = 1;
integer REQUEST_KIND_HISTORY = 2;
integer REQUEST_KIND_LOG = 3;

key gOwner;
integer gControlListenHandle;
integer gPublicListenHandle;
list gQueue;
integer gBusy;
key gRequestId;
integer gActiveKind;
string gActivePayload;
integer gActiveRetries;
integer gRequestsSent;
integer gRepliesReceived;
integer gFailures;
string gLastReply;
string gLastError;

integer is_true_word(string value)
{
    value = llToLower(llStringTrim(value, STRING_TRIM));
    return (value == "1") || (value == "on") || (value == "true") || (value == "yes");
}

integer is_false_word(string value)
{
    value = llToLower(llStringTrim(value, STRING_TRIM));
    return (value == "0") || (value == "off") || (value == "false") || (value == "no");
}

integer is_authorized(key id)
{
    if (id == gOwner)
    {
        return TRUE;
    }

    if (ALLOW_GROUP_ACCESS && llSameGroup(id))
    {
        return TRUE;
    }

    return FALSE;
}

string bool_text(integer flag)
{
    if (flag)
    {
        return "on";
    }
    return "off";
}

integer debug(string message)
{
    if (DEBUG_MODE)
    {
        llOwnerSay("[ollama-debug] " + message);
    }
    return 0;
}

integer notify(string message)
{
    llOwnerSay("[ollama v" + RELAY_VERSION + "] " + message);
    return 0;
}

string safe_team_name(string team)
{
    team = llStringTrim(team, STRING_TRIM);
    if (team == "")
    {
        return TEAM_NAME;
    }
    return team;
}

string endpoint_url(integer request_kind)
{
    string encoded_team = llEscapeURL(TEAM_NAME);
    if (request_kind == REQUEST_KIND_HISTORY)
    {
        return BASE_URL + "/history/" + encoded_team;
    }
    if (request_kind == REQUEST_KIND_LOG)
    {
        return BASE_URL + "/sl_logger";
    }
    return BASE_URL + "/chat/" + encoded_team;
}

string json_body_for_message(string message)
{
    return llJsonSetValue("{}", ["message"], message);
}

string sl_log_entry_line(string raw_message, key speaker_id, string speaker_name)
{
    vector p = llGetPos();
    string payload = llList2Json(
        JSON_OBJECT,
        [
            "avatar_id", (string)speaker_id,
            "avatar_name", speaker_name,
            "captured_by", llKey2Name(gOwner),
            "message", raw_message,
            "sim_name", llGetRegionName(),
            "timestamp", llGetUnixTime(),
            "x_pos", p.x,
            "y_pos", p.y,
            "z_pos", p.z
        ]
    );
    return payload;
}

integer say_chunks(string prefix, string text)
{
    integer length = llStringLength(text);
    integer start = 0;
    integer first = TRUE;

    if (text == "")
    {
        text = "(empty response)";
        length = llStringLength(text);
    }

    while (start < length)
    {
        integer stop = start + MAX_OUTPUT_CHARS - 1;
        string chunk = llGetSubString(text, start, stop);
        string output = chunk;
        if (first)
        {
            output = prefix + chunk;
            first = FALSE;
        }

        if (SPEAK_REPLIES_PUBLICLY)
        {
            llSay(0, output);
        }
        else
        {
            llOwnerSay(output);
        }

        start += MAX_OUTPUT_CHARS;
    }

    return 0;
}

string build_contextual_message(string raw_message, key speaker_id, string speaker_name)
{
    if (!INCLUDE_WORLD_CONTEXT)
    {
        return raw_message;
    }

    list lines = [];
    lines += ["[Second Life Team Relay]"];
    lines += ["Team: " + TEAM_NAME];
    lines += ["Speaker: " + speaker_name + " (" + (string)speaker_id + ")"];
    lines += ["Owner: " + llKey2Name(gOwner) + " (" + (string)gOwner + ")"];
    lines += ["Object: " + llGetObjectName()];
    lines += ["Region: " + llGetRegionName()];
    lines += ["Position: " + (string)llGetPos()];
    lines += ["Timestamp: " + llGetTimestamp()];
    lines += ["Message:"];
    lines += [raw_message];
    return llDumpList2String(lines, "\n");
}

integer enqueue_request(integer request_kind, string payload)
{
    gQueue += [request_kind, payload];
    debug("queued request kind=" + (string)request_kind + " queue_len=" + (string)(llGetListLength(gQueue) / QUEUE_STRIDE));
    return 0;
}

integer clear_active_request()
{
    gBusy = FALSE;
    gRequestId = NULL_KEY;
    gActiveKind = 0;
    gActivePayload = "";
    gActiveRetries = 0;
    llSetTimerEvent(0.0);
    return 0;
}

integer send_active_request()
{
    string url = endpoint_url(gActiveKind);
    list params = [HTTP_VERIFY_CERT, TRUE, HTTP_BODY_MAXLENGTH, 16384];
    string body = "";

    if (gActiveKind == REQUEST_KIND_CHAT)
    {
        params += [HTTP_METHOD, "POST", HTTP_MIMETYPE, "application/json"];
        body = json_body_for_message(gActivePayload);
    }
    else if (gActiveKind == REQUEST_KIND_LOG)
    {
        params += [HTTP_METHOD, "POST", HTTP_MIMETYPE, "text/plain"];
        body = gActivePayload;
    }
    else
    {
        params += [HTTP_METHOD, "GET"];
    }

    debug("sending request to " + url + " retry=" + (string)gActiveRetries);
    gRequestId = llHTTPRequest(url, params, body);
    gBusy = TRUE;
    gRequestsSent += 1;
    llSetTimerEvent(REQUEST_TIMEOUT);
    return 0;
}

integer pump_queue()
{
    if (gBusy)
    {
        return 0;
    }

    if (llGetListLength(gQueue) < QUEUE_STRIDE)
    {
        return 0;
    }

    gActiveKind = llList2Integer(gQueue, 0);
    gActivePayload = llList2String(gQueue, 1);
    gQueue = llDeleteSubList(gQueue, 0, QUEUE_STRIDE - 1);
    gActiveRetries = 0;
    send_active_request();
    return 0;
}

integer retry_or_fail(string reason)
{
    if (gActiveRetries < MAX_RETRIES)
    {
        gActiveRetries += 1;
        notify("Request retry " + (string)gActiveRetries + " of " + (string)MAX_RETRIES + " after: " + reason);
        send_active_request();
        return 0;
    }

    gFailures += 1;
    gLastError = reason;
    notify("Request failed: " + reason);
    clear_active_request();
    pump_queue();
    return 0;
}

integer set_public_listen(integer enabled)
{
    if (gPublicListenHandle)
    {
        llListenRemove(gPublicListenHandle);
        gPublicListenHandle = 0;
    }

    AUTO_RELAY_LOCAL_CHAT = enabled;
    if (AUTO_RELAY_LOCAL_CHAT)
    {
        gPublicListenHandle = llListen(0, "", NULL_KEY, "");
    }
    return 0;
}

integer show_status()
{
    list lines = [];
    lines += ["Team: " + TEAM_NAME];
    lines += ["Base URL: " + BASE_URL];
    lines += ["Control channel: " + (string)CONTROL_CHANNEL];
    lines += ["Auto relay local chat: " + bool_text(AUTO_RELAY_LOCAL_CHAT)];
    lines += ["Public replies: " + bool_text(SPEAK_REPLIES_PUBLICLY)];
    lines += ["Include world context: " + bool_text(INCLUDE_WORLD_CONTEXT)];
    lines += ["Mirror to /sl_logger: " + bool_text(ENABLE_SL_LOG_MIRROR)];
    lines += ["Debug mode: " + bool_text(DEBUG_MODE)];
    lines += ["Request timeout (s): " + (string)REQUEST_TIMEOUT];
    lines += ["Max retries: " + (string)MAX_RETRIES];
    lines += ["Busy: " + bool_text(gBusy)];
    lines += ["Queued requests: " + (string)(llGetListLength(gQueue) / QUEUE_STRIDE)];
    lines += ["Requests sent: " + (string)gRequestsSent];
    lines += ["Replies received: " + (string)gRepliesReceived];
    lines += ["Failures: " + (string)gFailures];
    if (gLastError != "")
    {
        lines += ["Last error: " + gLastError];
    }
    notify(llDumpList2String(lines, "\n"));
    return 0;
}

integer show_help()
{
    list lines = [];
    lines += ["Commands on /" + (string)CONTROL_CHANNEL + ":"];
    lines += ["ask <message>  - send a prompt to the current team"]; 
    lines += ["say <message>  - alias for ask"]; 
    lines += ["history        - fetch server-side team history"]; 
    lines += ["team <name>    - change the current team name"]; 
    lines += ["url <https://host:port> - change the backend base URL"]; 
    lines += ["auto on|off    - relay owner local chat on channel 0"]; 
    lines += ["public on|off  - say replies on channel 0 instead of owner chat"]; 
    lines += ["context on|off - include avatar/object/region metadata"]; 
    lines += ["log on|off     - mirror prompts to /sl_logger as JSON lines"]; 
    lines += ["debug on|off   - toggle debug output"]; 
    lines += ["(Current timeout/retries shown in status)"];
    lines += ["status         - show current configuration and counters"]; 
    lines += ["help           - show this help"]; 
    lines += ["reset          - reset the script"]; 
    lines += ["Touch the object to see status/help again."];
    notify(llDumpList2String(lines, "\n"));
    return 0;
}

integer handle_chat_submission(string raw_message, key speaker_id, string speaker_name)
{
    raw_message = llStringTrim(raw_message, STRING_TRIM);
    if (raw_message == "")
    {
        notify("Refusing to send an empty message.");
        return 0;
    }

    if (ENABLE_SL_LOG_MIRROR)
    {
        enqueue_request(REQUEST_KIND_LOG, sl_log_entry_line(raw_message, speaker_id, speaker_name));
    }
    enqueue_request(REQUEST_KIND_CHAT, build_contextual_message(raw_message, speaker_id, speaker_name));
    pump_queue();
    return 0;
}

integer handle_command(key speaker_id, string speaker_name, string message)
{
    list tokens = llParseString2List(message, [" "], []);
    string command = llToLower(llList2String(tokens, 0));
    string remainder = llStringTrim(llDeleteSubString(message, 0, llStringLength(command) - 1), STRING_TRIM);

    if (command == "ask" || command == "say")
    {
        handle_chat_submission(remainder, speaker_id, speaker_name);
        return 0;
    }

    if (command == "history")
    {
        enqueue_request(REQUEST_KIND_HISTORY, "");
        pump_queue();
        return 0;
    }

    if (command == "team")
    {
        remainder = safe_team_name(remainder);
        TEAM_NAME = remainder;
        notify("Team set to '" + TEAM_NAME + "'.");
        return 0;
    }

    if (command == "url")
    {
        if (remainder == "")
        {
            notify("Current URL: " + BASE_URL);
            return 0;
        }
        BASE_URL = remainder;
        notify("Base URL set to " + BASE_URL);
        return 0;
    }

    if (command == "auto")
    {
        if (is_true_word(remainder))
        {
            set_public_listen(TRUE);
        }
        else if (is_false_word(remainder))
        {
            set_public_listen(FALSE);
        }
        notify("Auto relay local chat is " + bool_text(AUTO_RELAY_LOCAL_CHAT) + ".");
        return 0;
    }

    if (command == "public")
    {
        if (is_true_word(remainder))
        {
            SPEAK_REPLIES_PUBLICLY = TRUE;
        }
        else if (is_false_word(remainder))
        {
            SPEAK_REPLIES_PUBLICLY = FALSE;
        }
        notify("Public replies are " + bool_text(SPEAK_REPLIES_PUBLICLY) + ".");
        return 0;
    }

    if (command == "context")
    {
        if (is_true_word(remainder))
        {
            INCLUDE_WORLD_CONTEXT = TRUE;
        }
        else if (is_false_word(remainder))
        {
            INCLUDE_WORLD_CONTEXT = FALSE;
        }
        notify("World context is " + bool_text(INCLUDE_WORLD_CONTEXT) + ".");
        return 0;
    }

    if (command == "log")
    {
        if (is_true_word(remainder))
        {
            ENABLE_SL_LOG_MIRROR = TRUE;
        }
        else if (is_false_word(remainder))
        {
            ENABLE_SL_LOG_MIRROR = FALSE;
        }
        notify("Mirror to /sl_logger is " + bool_text(ENABLE_SL_LOG_MIRROR) + ".");
        return 0;
    }

    if (command == "debug")
    {
        if (is_true_word(remainder))
        {
            DEBUG_MODE = TRUE;
        }
        else if (is_false_word(remainder))
        {
            DEBUG_MODE = FALSE;
        }
        notify("Debug mode is " + bool_text(DEBUG_MODE) + ".");
        return 0;
    }

    if (command == "status")
    {
        show_status();
        return 0;
    }

    if (command == "help")
    {
        show_help();
        return 0;
    }

    if (command == "reset")
    {
        llResetScript();
        return 0;
    }

    notify("Unknown command '" + command + "'. Use 'help'.");
    return 0;
}

default
{
    state_entry()
    {
        gOwner = llGetOwner();
        gQueue = [];
        gBusy = FALSE;
        gRequestId = NULL_KEY;
        gActiveKind = 0;
        gActivePayload = "";
        gActiveRetries = 0;
        gRequestsSent = 0;
        gRepliesReceived = 0;
        gFailures = 0;
        gLastReply = "";
        gLastError = "";

        if (gControlListenHandle)
        {
            llListenRemove(gControlListenHandle);
        }
        gControlListenHandle = llListen(CONTROL_CHANNEL, "", NULL_KEY, "");

        set_public_listen(AUTO_RELAY_LOCAL_CHAT);
        notify("Ready. Team='" + TEAM_NAME + "' URL='" + BASE_URL + "' control channel=/" + (string)CONTROL_CHANNEL);
        show_help();
    }

    on_rez(integer start_param)
    {
        llResetScript();
    }

    changed(integer change)
    {
        if (change & CHANGED_OWNER)
        {
            llResetScript();
        }
    }

    touch_start(integer total_number)
    {
        key toucher = llDetectedKey(0);
        if (!is_authorized(toucher))
        {
            llRegionSayTo(toucher, 0, "This relay is owner-controlled.");
            return;
        }

        show_status();
        show_help();
    }

    listen(integer channel, string name, key id, string message)
    {
        if (!is_authorized(id))
        {
            return;
        }

        if (channel == CONTROL_CHANNEL)
        {
            handle_command(id, name, message);
            return;
        }

        if (channel == 0 && AUTO_RELAY_LOCAL_CHAT)
        {
            handle_chat_submission(message, id, name);
        }
    }

    http_response(key request_id, integer status, list metadata, string body)
    {
        if (request_id != gRequestId)
        {
            debug("ignoring stale http response");
            return;
        }

        llSetTimerEvent(0.0);

        if (status < 200 || status >= 300)
        {
            retry_or_fail("HTTP " + (string)status + ": " + body);
            return;
        }

        gRepliesReceived += 1;
        gLastError = "";

        if (gActiveKind == REQUEST_KIND_HISTORY)
        {
            string history = llJsonGetValue(body, ["history"]);
            if (history == JSON_INVALID)
            {
                history = body;
            }
            gLastReply = history;
            say_chunks("[ollama history] ", history);
        }
        else if (gActiveKind == REQUEST_KIND_LOG)
        {
            debug("log mirror accepted: HTTP " + (string)status);
        }
        else
        {
            string reply = llJsonGetValue(body, ["response"]);
            if (reply == JSON_INVALID)
            {
                string err = llJsonGetValue(body, ["error"]);
                if (err != JSON_INVALID)
                {
                    retry_or_fail(err);
                    return;
                }
                reply = body;
            }

            reply = llStringTrim(reply, STRING_TRIM);
            if (reply == "")
            {
                string err2 = llJsonGetValue(body, ["error"]);
                if (err2 != JSON_INVALID)
                {
                    reply = "Backend error: " + err2;
                }
                else
                {
                    reply = "Model returned no text output. Please retry your message.";
                }
            }

            gLastReply = reply;
            say_chunks("[ollama v" + RELAY_VERSION + " " + TEAM_NAME + "] ", reply);
        }

        clear_active_request();
        pump_queue();
    }

    timer()
    {
        if (gBusy)
        {
            retry_or_fail("request timed out after " + (string)REQUEST_TIMEOUT + " seconds");
        }
        else
        {
            llSetTimerEvent(0.0);
        }
    }
}