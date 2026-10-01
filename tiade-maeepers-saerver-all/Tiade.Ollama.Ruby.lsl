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
//   /7 ask what is a prim?     /7 ruby [1,2,3].sum     /7 help

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

drop_job(string job_id)
{
    integer index = llListFindList(gJobs, [job_id]);
    if (index != -1) gJobs = llDeleteSubList(gJobs, index, index + 1);
    if (gJobs == []) llSetTimerEvent(0.0);
}

ask(string message, string speaker)
{
    message = llStringTrim(message, STRING_TRIM);
    if (message == "") { notify("Usage: ask <message>"); return; }
    if (llGetListLength(gJobs) / 2 >= MAX_JOBS)
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

help()
{
    notify(llDumpList2String([
        "Commands on /" + (string)CONTROL_CHANNEL + " (owner only):",
        "ask <message>    - ask Ollama (team '" + TEAM_NAME + "')",
        "ruby <code>      - evaluate Ruby on the server (locals persist)",
        "ruby-reset       - clear this object's Ruby session",
        "team <name>      - switch Ollama team/history",
        "public on|off    - reply in local chat instead of owner-only",
        "auto on|off      - send your local chat to Ollama automatically",
        "url <https://..> - change the server",
        "status | help"], "\n"));
}

show_status()
{
    notify(llDumpList2String([
        "Server: " + BASE_URL,
        "Team: " + TEAM_NAME,
        "Ruby: " + on_off(RUBY_TOKEN != ""),
        "Public replies: " + on_off(PUBLIC_REPLIES),
        "Auto ask: " + on_off(AUTO_ASK),
        "Waiting replies: " + (string)(llGetListLength(gJobs) / 2),
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
    else
    {
        notify("Ollama error: " + llJsonGetValue(body, ["error"]));
    }
}

default
{
    state_entry()
    {
        gOwner = llGetOwner();
        gRequests = [];
        gJobs = [];
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

    http_response(key request, integer status, list metadata, string body)
    {
        integer index = llListFindList(gRequests, [request]);
        if (index == -1) return;
        integer kind = llList2Integer(gRequests, index + 1);
        string job_id = llList2String(gRequests, index + 2);
        gRequests = llDeleteSubList(gRequests, index, index + 2);

        if (kind == KIND_ASK)
        {
            string id = llJsonGetValue(body, ["job"]);
            if (status == 202 && id != JSON_INVALID)
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
            notify("Ruby: " + body);
        }
    }

    timer()
    {
        integer now = llGetUnixTime();
        integer i = 0;
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
        if (gJobs == []) llSetTimerEvent(0.0);
    }
}
