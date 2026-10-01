// Tiade.Ollama.Ruby.lsl
// Owner-controlled Second Life client for the Tide server:
//   * Ollama chat through the job-based routes (no 60 s HTTP timeouts):
//       POST /sl/ask/<team>  {"message","speaker"}  -> 202 {"job":"<id>"}
//       GET  /sl/job/<id>                          -> {"status":"pending|done|error", ...}
//   * Ruby evaluation in the server's embedded Magnus VM:
//       POST /sl/ruby/eval   {"code"}   header X-Ruby-Token
//       POST /sl/ruby/reset             header X-Ruby-Token
//     Each object gets its own Ruby session (its object key), so local
//     variables persist between `ruby` commands until `ruby-reset`.
//
// Setup: put this script in an object you own, set BASE_URL and, for Ruby,
// RUBY_TOKEN to the server's TIADE_RUBY_EVAL_TOKEN. Then chat on /7:
//   /7 ask what is a prim?     /7 ruby [1,2,3].sum     /7 notecard Demo

string  BASE_URL        = "https://stimky.info";
string  TEAM_NAME       = "secondlife";
string  RUBY_TOKEN      = "";      // must match TIADE_RUBY_EVAL_TOKEN on the server
integer CONTROL_CHANNEL = 7;       // avatars can only chat on positive channels
integer PUBLIC_REPLIES  = FALSE;   // FALSE: llOwnerSay, TRUE: llSay on channel 0
integer AUTO_ASK        = FALSE;   // relay the owner's channel-0 chat to Ollama
float   POLL_SECONDS    = 1.5;
integer ASK_DEADLINE    = 240;     // seconds before a queued ask is abandoned
integer MAX_JOBS        = 4;
integer CHUNK_CHARS     = 800;     // chat lines are limited to 1024 bytes

integer KIND_ASK   = 1;
integer KIND_POLL  = 2;
integer KIND_RUBY  = 3;
integer KIND_RESET = 4;

key     gOwner;
integer gControlListen;
integer gPublicListen;
list    gRequests;   // [request_key, kind, job_id] for in-flight HTTP requests
list    gJobs;       // [job_id, deadline_unix] for queued Ollama replies
string  gNotecardName;
integer gNotecardLine;
integer gNotecardActive;
integer gNotecardReadPending;
integer gNotecardExecuting;
key     gNotecardQuery;
key     gNotecardWaitRequest;

say(string prefix, string text)
{
    if (text == "") text = "(empty)";
    integer length = llStringLength(text);
    integer start = 0;
    while (start < length)
    {
        string chunk = llGetSubString(text, start, start + CHUNK_CHARS - 1);
        if (start == 0) chunk = prefix + chunk;
        if (PUBLIC_REPLIES) llSay(0, chunk);
        else llOwnerSay(chunk);
        start += CHUNK_CHARS;
    }
}

notify(string text)
{
    llOwnerSay("[tiade] " + text);
}

string on_off(integer flag)
{
    if (flag) return "on";
    return "off";
}

integer is_on(string word)
{
    word = llToLower(llStringTrim(word, STRING_TRIM));
    return word == "on" || word == "1" || word == "true" || word == "yes";
}

integer send(integer kind, string job_id, string url, string method, string body, integer ruby_auth)
{
    list params = [HTTP_METHOD, method, HTTP_VERIFY_CERT, TRUE, HTTP_BODY_MAXLENGTH, 16384];
    if (method == "POST") params += [HTTP_MIMETYPE, "application/json"];
    if (ruby_auth) params += [HTTP_CUSTOM_HEADER, "X-Ruby-Token", RUBY_TOKEN];
    key request = llHTTPRequest(url, params, body);
    if (request == NULL_KEY)
    {
        notify("HTTP request was throttled by the region; try again in a few seconds.");
        return FALSE;
    }
    gRequests += [request, kind, job_id];
    if (gNotecardExecuting) gNotecardWaitRequest = request;
    return TRUE;
}

integer poll_in_flight(string job_id)
{
    integer i;
    integer count = llGetListLength(gRequests);
    for (i = 0; i < count; i += 3)
    {
        if (llList2Integer(gRequests, i + 1) == KIND_POLL && llList2String(gRequests, i + 2) == job_id)
            return TRUE;
    }
    return FALSE;
}

integer pending_asks()
{
    integer i;
    integer count;
    integer length = llGetListLength(gRequests);
    for (i = 0; i < length; i += 3)
    {
        if (llList2Integer(gRequests, i + 1) == KIND_ASK) count++;
    }
    return count;
}

drop_job(string job_id)
{
    integer index = llListFindList(gJobs, [job_id]);
    if (index != -1) gJobs = llDeleteSubList(gJobs, index, index + 1);
    if (gJobs == [] && !gNotecardActive) llSetTimerEvent(0.0);
}

ask(string message, string speaker)
{
    message = llStringTrim(message, STRING_TRIM);
    if (message == "") { notify("Usage: ask <message>"); return; }
    if (llGetListLength(gJobs) / 2 + pending_asks() >= MAX_JOBS)
    {
        notify("Still waiting on " + (string)MAX_JOBS + " replies; try again shortly.");
        return;
    }
    string body = llList2Json(JSON_OBJECT, ["message", message, "speaker", speaker]);
    send(KIND_ASK, "", BASE_URL + "/sl/ask/" + llEscapeURL(TEAM_NAME), "POST", body, FALSE);
}

ruby(string code)
{
    if (RUBY_TOKEN == "") { notify("Set RUBY_TOKEN in the script to use Ruby."); return; }
    if (llStringTrim(code, STRING_TRIM) == "") { notify("Usage: ruby <code>"); return; }
    send(KIND_RUBY, "", BASE_URL + "/sl/ruby/eval", "POST",
        llList2Json(JSON_OBJECT, ["code", code]), TRUE);
}

request_notecard_line()
{
    key query = llGetNotecardLine(gNotecardName, gNotecardLine);
    if (query == NULL_KEY)
    {
        gNotecardActive = FALSE;
        gNotecardReadPending = FALSE;
        notify("Could not read notecard '" + gNotecardName + "'.");
    }
    else
    {
        gNotecardQuery = query;
        gNotecardReadPending = TRUE;
    }
}

start_notecard(string name)
{
    name = llStringTrim(name, STRING_TRIM);
    if (name == "") { notify("Usage: notecard <inventory name>"); return; }
    if (gNotecardActive) { notify("Already reading notecard '" + gNotecardName + "'."); return; }
    if (llGetInventoryType(name) != INVENTORY_NOTECARD)
    {
        notify("Notecard '" + name + "' was not found in this object's inventory.");
        return;
    }
    gNotecardName = name;
    gNotecardLine = 0;
    gNotecardActive = TRUE;
    gNotecardReadPending = FALSE;
    gNotecardWaitRequest = NULL_KEY;
    notify("Running notecard '" + gNotecardName + "' (one command per line).");
    llSetTimerEvent(POLL_SECONDS);
    request_notecard_line();
}

stop_notecard()
{
    if (!gNotecardActive) { notify("No notecard is running."); return; }
    gNotecardActive = FALSE;
    gNotecardReadPending = FALSE;
    gNotecardQuery = NULL_KEY;
    notify("Stopped notecard '" + gNotecardName + "'.");
    if (gJobs == []) llSetTimerEvent(0.0);
}

help()
{
    notify("HELP 1/4 - CHAT AND SETTINGS");
    notify("Commands use /" + (string)CONTROL_CHANNEL + " and are owner-only.");
    notify("/" + (string)CONTROL_CHANNEL + " ask <message> - ask Ollama using team '" + TEAM_NAME + "'.");
    notify("Plain text on the control channel is also sent as an Ollama question.");
    notify("/" + (string)CONTROL_CHANNEL + " team <name> - change the team and its shared chat history.");
    notify("/" + (string)CONTROL_CHANNEL + " url <https://host> - change the HTTPS server URL (no trailing slash).");
    notify("/" + (string)CONTROL_CHANNEL + " status - show server, team, options, and pending replies.");
    notify("/" + (string)CONTROL_CHANNEL + " help - show this extended help.");

    notify("HELP 2/4 - RUBY");
    notify("/" + (string)CONTROL_CHANNEL + " ruby <code> - run Ruby on the server; session locals persist for this object.");
    notify("/" + (string)CONTROL_CHANNEL + " ruby-reset - clear this object's Ruby session.");
    notify("Set RUBY_TOKEN in the script to the server's TIADE_RUBY_EVAL_TOKEN. Empty token disables Ruby.");
    notify("Ruby is remote code execution with server privileges. Only use a server and token you trust; never share the token.");
    notify("Ruby output/errors are returned to the owner. Server timeouts and code-length limits also apply.");

    notify("HELP 3/4 - NOTECARD RUNNER");
    notify("Put a notecard in this object's inventory, then use /" + (string)CONTROL_CHANNEL + " notecard <exact name>.");
    notify("Each nonblank line is one command. Lines starting with # are comments. Example lines:");
    notify("team secondlife");
    notify("ask What is a prim?");
    notify("ruby [1, 2, 3].sum");
    notify("/" + (string)CONTROL_CHANNEL + " notecard-stop - stop reading more lines. Only one notecard runs at a time.");
    notify("HTTP commands are submitted in sequence. Ollama replies finish asynchronously; up to " + (string)MAX_JOBS + " can be pending.");

    notify("HELP 4/4 - REPLIES AND TROUBLESHOOTING");
    notify("/" + (string)CONTROL_CHANNEL + " public on|off - show replies in local chat or only to the owner (default off).");
    notify("/" + (string)CONTROL_CHANNEL + " auto on|off - forward your own channel-0 chat to Ollama (default off).");
    notify("If requests fail, check status, the HTTPS URL, server availability, and Ruby token configuration.");
    notify("Ollama asks can take time; jobs are polled automatically and abandoned after " + (string)ASK_DEADLINE + " seconds.");
}

show_status()
{
    string notecard_status = "none";
    if (gNotecardActive) notecard_status = gNotecardName;
    notify(llDumpList2String([
        "Server: " + BASE_URL,
        "Team: " + TEAM_NAME,
        "Ruby: " + on_off(RUBY_TOKEN != ""),
        "Public replies: " + on_off(PUBLIC_REPLIES),
        "Auto ask: " + on_off(AUTO_ASK),
        "Waiting replies: " + (string)(llGetListLength(gJobs) / 2),
        "Notecard: " + notecard_status,
        "Free memory: " + (string)llGetFreeMemory()], "\n"));
}

set_auto(integer enabled)
{
    AUTO_ASK = enabled;
    if (gPublicListen) llListenRemove(gPublicListen);
    gPublicListen = 0;
    if (AUTO_ASK) gPublicListen = llListen(0, "", gOwner, "");
}

command(string message, string speaker)
{
    message = llStringTrim(message, STRING_TRIM);
    integer space = llSubStringIndex(message, " ");
    string verb = llToLower(message);
    string rest = "";
    if (space != -1)
    {
        verb = llToLower(llGetSubString(message, 0, space - 1));
        rest = llStringTrim(llGetSubString(message, space + 1, -1), STRING_TRIM);
    }

    if (verb == "ask") ask(rest, speaker);
    else if (verb == "ruby") ruby(rest);
    else if (verb == "ruby-reset")
    {
        if (RUBY_TOKEN == "") notify("Set RUBY_TOKEN in the script to use Ruby.");
        else send(KIND_RESET, "", BASE_URL + "/sl/ruby/reset", "POST", "{}", TRUE);
    }
    else if (verb == "notecard") start_notecard(rest);
    else if (verb == "notecard-stop") stop_notecard();
    else if (verb == "team" && rest != "") { TEAM_NAME = rest; notify("Team: " + TEAM_NAME); }
    else if (verb == "public") { PUBLIC_REPLIES = is_on(rest); notify("Public replies " + on_off(PUBLIC_REPLIES)); }
    else if (verb == "auto") { set_auto(is_on(rest)); notify("Auto ask " + on_off(AUTO_ASK)); }
    else if (verb == "url" && llSubStringIndex(rest, "https://") == 0) { BASE_URL = rest; notify("Server: " + BASE_URL); }
    else if (verb == "status") show_status();
    else if (verb == "help") help();
    else ask(message, speaker);   // plain text is treated as a question
}

handle_job_status(string job_id, string body)
{
    string job_state = llJsonGetValue(body, ["status"]);
    if (job_state == "pending") return;
    drop_job(job_id);
    if (job_state == "done")
    {
        say("[ollama " + llJsonGetValue(body, ["seconds"]) + "s] ", llJsonGetValue(body, ["reply"]));
    }
    else if (job_state == "error")
    {
        notify("Ollama error: " + llJsonGetValue(body, ["error"]));
    }
    else
    {
        notify("Invalid Ollama job response: " + body);
    }
}

default
{
    state_entry()
    {
        gOwner = llGetOwner();
        gRequests = [];
        gJobs = [];
        gNotecardActive = FALSE;
        gNotecardReadPending = FALSE;
        gNotecardExecuting = FALSE;
        gNotecardQuery = NULL_KEY;
        gNotecardWaitRequest = NULL_KEY;
        gControlListen = llListen(CONTROL_CHANNEL, "", gOwner, "");
        set_auto(AUTO_ASK);
        notify("Ready on /" + (string)CONTROL_CHANNEL + ". Say /" + (string)CONTROL_CHANNEL + " help");
    }

    on_rez(integer param) { llResetScript(); }

    changed(integer change)
    {
        if (change & CHANGED_OWNER) llResetScript();
    }

    touch_start(integer count)
    {
        if (llDetectedKey(0) == gOwner) { show_status(); help(); }
    }

    listen(integer channel, string name, key id, string message)
    {
        if (id != gOwner) return;
        if (channel == CONTROL_CHANNEL) command(message, name);
        else if (channel == 0 && AUTO_ASK) ask(message, name);
    }

    dataserver(key query_id, string data)
    {
        if (!gNotecardActive || query_id != gNotecardQuery) return;
        gNotecardReadPending = FALSE;
        gNotecardQuery = NULL_KEY;
        if (data == EOF)
        {
            gNotecardActive = FALSE;
            notify("Finished notecard '" + gNotecardName + "'.");
            if (gJobs == []) llSetTimerEvent(0.0);
            return;
        }

        data = llStringTrim(data, STRING_TRIM);
        if (data != "" && llGetSubString(data, 0, 0) != "#")
        {
            gNotecardLine++;
            gNotecardExecuting = TRUE;
            command(data, "notecard");
            gNotecardExecuting = FALSE;
        }
        else gNotecardLine++;
    }

    http_response(key request, integer status, list metadata, string body)
    {
        integer index = llListFindList(gRequests, [request]);
        if (index == -1) return;
        integer kind = llList2Integer(gRequests, index + 1);
        string job_id = llList2String(gRequests, index + 2);
        gRequests = llDeleteSubList(gRequests, index, index + 2);
        if (request == gNotecardWaitRequest) gNotecardWaitRequest = NULL_KEY;

        if (kind == KIND_ASK)
        {
            string id = llJsonGetValue(body, ["job"]);
            if (status == 202 && id != JSON_INVALID && id != JSON_NULL && id != "")
            {
                gJobs += [id, llGetUnixTime() + ASK_DEADLINE];
                llSetTimerEvent(POLL_SECONDS);
            }
            else notify("Ask failed (HTTP " + (string)status + "): " + body);
        }
        else if (kind == KIND_POLL)
        {
            if (status == 200) handle_job_status(job_id, body);
            else if (status == 404) { drop_job(job_id); notify("Reply expired on the server."); }
            // other statuses (e.g. 499 region timeout) are retried on the next tick
        }
        else if (kind == KIND_RUBY)
        {
            if (status == 200) say("[ruby] ", body);
            else if (status == 422) say("[ruby error] ", body);
            else notify("Ruby request failed (HTTP " + (string)status + "): " + body);
        }
        else if (kind == KIND_RESET)
        {
            if (status == 200) notify("Ruby: " + body);
            else notify("Ruby reset failed (HTTP " + (string)status + "): " + body);
        }
    }

    timer()
    {
        integer now = llGetUnixTime();
        integer i = 0;
        if (gNotecardActive && !gNotecardReadPending && gNotecardWaitRequest == NULL_KEY)
            request_notecard_line();
        while (i < llGetListLength(gJobs))
        {
            string job_id = llList2String(gJobs, i);
            if (now > llList2Integer(gJobs, i + 1))
            {
                gJobs = llDeleteSubList(gJobs, i, i + 1);
                notify("Gave up waiting for an Ollama reply.");
            }
            else
            {
                if (!poll_in_flight(job_id))
                    send(KIND_POLL, job_id, BASE_URL + "/sl/job/" + job_id, "GET", "", FALSE);
                i += 2;
            }
        }
        if (gJobs == [] && !gNotecardActive) llSetTimerEvent(0.0);
    }
}
