// Tiade.Ollama.Ruby.lsl
// Owner-controlled Second Life client for the Tide server.
// Install Tiade.Ollama.Ruby.Chat.lsl and Tiade.Ollama.Ruby.Help.lsl in the
// same prim. Chat owns the listeners; Help owns the /7 help text.
//
// Server routes used:
//   POST /sl/ask/<team>  {"message","speaker"}  -> 202 {"job":"<id>"}
//   GET  /sl/job/<id>    -> {"status":"pending"} | {"status":"done","reply","seconds"}
//                           | {"status":"error","error"}; 404 when expired
//   POST /sl/ruby/eval   {"code"}  header X-Ruby-Token -> 200 output | 422 Ruby error
//   POST /sl/ruby/reset  {}        header X-Ruby-Token -> 200
//   GET  /ollama/health  -> 200 {"status":"ok",...} | 503
// The Ruby session defaults to this object's key. /7 session <id> selects
// another session shared with the Forth and RubyForth routes.
//
// Setup: put this script in an object you own, set BASE_URL and, for Ruby,
// RUBY_TOKEN to the server's TIADE_RUBY_EVAL_TOKEN. Then chat on /7:
//   /7 ask what is a prim?      /7 ruby [1, 2, 3].sum
//   /7 notecard Demo            /7 ruby-notecard Script
//   /7 var-set score 21          /7 var-get score
//   /7 file-write notes.txt hi  /7 matrix-get A
//
// Command notecard (`/7 notecard <name>`): one command per line, run in order.
//   # or // starts a comment; a leading "/7 " is optional.
//   ask lines wait for the Ollama reply before the next line runs.
//   wait <seconds>             pause the notecard
//   ruby-begin ... ruby-end    send the enclosed lines as ONE Ruby program
// Ruby notecard (`/7 ruby-notecard <name>`): the whole notecard is one program.

string  BASE_URL        = "https://stimky.info";
string  TEAM_NAME       = "secondlife";
string  RUBY_TOKEN      = "";      // must match TIADE_RUBY_EVAL_TOKEN on the server
integer CONTROL_CHANNEL = 7;       // avatars can only chat on positive channels
integer PUBLIC_REPLIES  = FALSE;   // FALSE: llOwnerSay, TRUE: llSay on channel 0
integer DEBUG_OUTPUT    = TRUE;    // FALSE: only explicit Ruby output, not => last value
float   TICK_SECONDS    = 2.0;     // one job poll per tick keeps under the HTTP throttle
integer ASK_DEADLINE    = 240;     // seconds before a queued ask is abandoned
integer MAX_JOBS        = 4;
integer MAX_REQUESTS    = 8;       // bound the in-flight ledger if HTTP replies never arrive
integer CHUNK_CHARS     = 800;     // chat lines are limited to 1024 bytes
integer READ_TIMEOUT    = 30;      // seconds to wait for one notecard line
integer RETRY_SECONDS   = 5;       // pause before retrying a throttled request

integer KIND_ASK    = 1;
integer KIND_POLL   = 2;
integer KIND_RUBY   = 3;
integer KIND_RESET  = 4;
integer KIND_HEALTH = 5;

integer NC_OFF      = 0;
integer NC_COMMANDS = 1;
integer NC_RUBY     = 2;
integer HELP_LINK_MESSAGE = -708641;
integer CHAT_COMMAND_MESSAGE = -708642;
string  HELP_SCRIPT = "Tiade.Ollama.Ruby.Help.lsl";

key     gOwner;
string  gSession;       // Ruby locals and shared stores use this session
list    gRequests;      // [request_key, kind, job_id] for in-flight HTTP requests
list    gJobs;          // [job_id, deadline_unix] for queued Ollama replies
integer gPollIndex;     // round-robin position in gJobs

integer gNcMode;        // NC_OFF, NC_COMMANDS or NC_RUBY
string  gNcName;
key     gNcAsset;       // inventory key, to notice edits while running
integer gNcLine;
key     gNcQuery;       // pending llGetNotecardLine, or NULL_KEY
integer gNcQueryTime;
key     gNcWaitRequest; // HTTP request the notecard is waiting on
string  gNcWaitJob;     // Ollama job the notecard is waiting on
integer gNcResumeAt;    // unix time the notecard may continue (wait/retry)
integer gNcExecuting;   // TRUE while a notecard line is being run
integer gNcThrottled;   // set by send() when a notecard request was throttled
integer gNcInBlock;     // inside ruby-begin ... ruby-end
string  gNcBuffer;      // Ruby source being collected

// ---------------------------------------------------------------- output

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

string replace_all(string text, string find, string with)
{
    return llDumpList2String(llParseStringKeepNulls(text, [find], []), with);
}

// llList2Json embeds strings that look like JSON ("[1,2]", "{}") unquoted,
// so request bodies are built with explicit string escaping instead.
// LSL has no \r escape, and \t compiles to spaces; decode those characters at runtime.
string json_string(string text)
{
    text = replace_all(text, "\\", "\\\\");
    text = replace_all(text, "\"", "\\\"");
    text = replace_all(text, "\n", "\\n");
    text = replace_all(text, llUnescapeURL("%0D"), "\\r");
    text = replace_all(text, llUnescapeURL("%09"), "\\t");
    return "\"" + text + "\"";
}

// Preserve JSON's escaping inside a single-quoted Ruby literal. Never
// interpolate chat or notecard text directly into Ruby source.
string ruby_string_arg(string text)
{
    string encoded = json_string(text);
    encoded = replace_all(encoded, "\\", "\\\\");
    encoded = replace_all(encoded, "'", "\\'");
    return "JSON.parse('" + encoded + "')";
}

integer valid_session(string name)
{
    integer length = llStringLength(name);
    if (length < 1 || length > 64) return FALSE;
    integer i;
    string allowed = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
    for (i = 0; i < length; ++i)
    {
        if (llSubStringIndex(allowed, llGetSubString(name, i, i)) == -1) return FALSE;
    }
    return TRUE;
}

// ---------------------------------------------------------------- HTTP

update_timer()
{
    if (gJobs != [] || gNcMode != NC_OFF) llSetTimerEvent(TICK_SECONDS);
    else llSetTimerEvent(0.0);
}

integer send(integer kind, string job_id, string url, string method, string body, integer ruby_auth)
{
    if (llGetListLength(gRequests) / 3 >= MAX_REQUESTS)
    {
        if (gNcExecuting) gNcThrottled = TRUE;
        else if (kind != KIND_POLL) notify("Too many outstanding HTTP requests; wait for replies or reset the script.");
        return FALSE;
    }
    list params = [HTTP_METHOD, method, HTTP_VERIFY_CERT, TRUE, HTTP_BODY_MAXLENGTH, 16384];
    if (method == "POST") params += [HTTP_MIMETYPE, "application/json"];
    if (ruby_auth) params += [HTTP_CUSTOM_HEADER, "X-Ruby-Token", RUBY_TOKEN];
    key request = llHTTPRequest(url, params, body);
    if (request == NULL_KEY)
    {
        if (gNcExecuting) gNcThrottled = TRUE;
        else if (kind != KIND_POLL) notify("HTTP request was throttled by the region; try again in a few seconds.");
        return FALSE;
    }
    gRequests += [request, kind, job_id];
    if (gNcExecuting) gNcWaitRequest = request;
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
    integer count = 0;
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
    if (job_id == gNcWaitJob) gNcWaitJob = "";
}

// ---------------------------------------------------------------- commands

ask(string message, string speaker)
{
    message = llStringTrim(message, STRING_TRIM);
    if (message == "") { notify("Usage: ask <message>"); return; }
    if (llGetListLength(gJobs) / 2 + pending_asks() >= MAX_JOBS)
    {
        if (gNcExecuting) { gNcThrottled = TRUE; return; }
        notify("Still waiting on " + (string)MAX_JOBS + " replies; try again shortly.");
        return;
    }
    string body = "{\"message\":" + json_string(message) + ",\"speaker\":" + json_string(speaker) + "}";
    send(KIND_ASK, "", BASE_URL + "/sl/ask/" + llEscapeURL(TEAM_NAME), "POST", body, FALSE);
}

integer ruby_ready()
{
    if (RUBY_TOKEN == "") { notify("Set RUBY_TOKEN in the script to use Ruby."); return FALSE; }
    return TRUE;
}

integer ruby_in_flight()
{
    integer i;
    integer count = llGetListLength(gRequests);
    for (i = 0; i < count; i += 3)
    {
        integer kind = llList2Integer(gRequests, i + 1);
        if (kind == KIND_RUBY || kind == KIND_RESET) return TRUE;
    }
    return FALSE;
}

integer ruby(string code)
{
    if (!ruby_ready()) return FALSE;
    if (llStringTrim(code, STRING_TRIM) == "") { notify("Usage: ruby <code>"); return FALSE; }
    if (ruby_in_flight())
    {
        if (gNcExecuting) gNcThrottled = TRUE;
        else notify("Ruby is still running; wait for its reply before sending another Ruby command.");
        return FALSE;
    }
    if (llGetListLength(gRequests) / 3 >= MAX_REQUESTS)
    {
        if (gNcExecuting) gNcThrottled = TRUE;
        else notify("Too many outstanding HTTP requests; wait for replies or reset the script.");
        return FALSE;
    }
    integer required_memory = llStringLength(code) * 4 + 2048;
    integer free_memory = llGetFreeMemory();
    if (free_memory < required_memory)
    {
        notify("Not enough script memory to send Ruby (" + (string)free_memory + " free, "
            + (string)required_memory + " needed); use a smaller program or reset the script.");
        return FALSE;
    }
    string include_result = "false";
    if (DEBUG_OUTPUT) include_result = "true";
    return send(KIND_RUBY, "", BASE_URL + "/sl/ruby/eval", "POST",
        "{\"code\":" + json_string(code) + ",\"session\":" + json_string(gSession)
            + ",\"include_result\":" + include_result + "}", TRUE);
}

ruby_reset()
{
    if (ruby_in_flight())
    {
        notify("Ruby is still running; wait for its reply before resetting the session.");
        return;
    }
    if (ruby_ready()) send(KIND_RESET, "", BASE_URL + "/sl/ruby/reset", "POST",
        "{\"session\":" + json_string(gSession) + "}", TRUE);
}

store_command(string verb, string rest)
{
    if (!ruby_ready()) return;
    string name = rest;
    string value = "";
    if (verb == "var-set" || verb == "file-write")
    {
        integer space = llSubStringIndex(rest, " ");
        if (space == -1)
        {
            notify("Usage: " + verb + " <name> <value>");
            return;
        }
        name = llGetSubString(rest, 0, space - 1);
        value = llStringTrim(llGetSubString(rest, space + 1, -1), STRING_TRIM);
        if (value == "")
        {
            notify("Usage: " + verb + " <name> <value>");
            return;
        }
    }
    if (verb != "var-view" && verb != "file-list" && (name == "" || llSubStringIndex(name, " ") != -1))
    {
        notify("Usage: " + verb + " <name>");
        return;
    }
    if ((verb == "var-view" || verb == "file-list") && rest != "")
    {
        notify("Usage: " + verb);
        return;
    }

    string code;
    if (verb == "var-view") code = "var_view";
    else if (verb == "file-list") code = "file_list";
    else if (verb == "var-set") code = "var_set(" + ruby_string_arg(name) + ", " + ruby_string_arg(value) + ")";
    else if (verb == "file-write") code = "file_write(" + ruby_string_arg(name) + ", " + ruby_string_arg(value) + ")";
    else
    {
        string method = "";
        if (verb == "var-get") method = "var_get";
        else if (verb == "var-delete") method = "var_delete";
        else if (verb == "file-read") method = "file_read";
        else if (verb == "file-delete") method = "file_delete";
        else if (verb == "matrix-get") method = "matrix_get";
        code = method + "(" + ruby_string_arg(name) + ")";
    }
    ruby(code);
}

// ---------------------------------------------------------------- notecards

start_notecard(string name, integer mode)
{
    name = llStringTrim(name, STRING_TRIM);
    if (name == "") { notify("Usage: notecard <name>  or  ruby-notecard <name>"); return; }
    if (gNcMode != NC_OFF) { notify("Already running notecard '" + gNcName + "'. Use notecard-stop first."); return; }
    if (llGetInventoryType(name) != INVENTORY_NOTECARD)
    {
        notify("Notecard '" + name + "' was not found in this object's inventory (names are case-sensitive).");
        return;
    }
    if (mode == NC_RUBY && !ruby_ready()) return;
    gNcMode = mode;
    gNcName = name;
    gNcAsset = llGetInventoryKey(name);
    gNcLine = 0;
    gNcQuery = NULL_KEY;
    gNcWaitRequest = NULL_KEY;
    gNcWaitJob = "";
    gNcResumeAt = 0;
    gNcThrottled = FALSE;
    gNcInBlock = FALSE;
    gNcBuffer = "";
    if (mode == NC_RUBY) notify("Sending notecard '" + name + "' to Ruby as one program.");
    else notify("Running notecard '" + name + "'.");
    update_timer();
    notecard_next();
}

end_notecard(string message)
{
    gNcMode = NC_OFF;
    gNcQuery = NULL_KEY;
    gNcWaitRequest = NULL_KEY;
    gNcWaitJob = "";
    gNcInBlock = FALSE;
    gNcBuffer = "";
    notify(message);
    update_timer();
}

// Reads the next line unless the notecard is waiting for something.
notecard_next()
{
    if (gNcMode == NC_OFF || gNcQuery != NULL_KEY) return;
    if (gNcWaitRequest != NULL_KEY || gNcWaitJob != "") return;
    if (llGetUnixTime() < gNcResumeAt) return;
    if (llGetInventoryType(gNcName) != INVENTORY_NOTECARD)
    {
        end_notecard("Notecard '" + gNcName + "' was removed; stopped.");
        return;
    }
    gNcQuery = llGetNotecardLine(gNcName, gNcLine);
    gNcQueryTime = llGetUnixTime();
}

// Appends a Ruby source line unless there is insufficient script memory.
integer buffer_ruby(string line)
{
    if (llGetFreeMemory() < llStringLength(gNcBuffer) + llStringLength(line) + 8192)
    {
        end_notecard("Not enough script memory to read notecard '" + gNcName + "'; stopped.");
        return FALSE;
    }
    gNcBuffer += line + "\n";
    return TRUE;
}

// Sends the collected Ruby; on throttle the same line is read again later.
integer flush_ruby()
{
    if (llStringTrim(gNcBuffer, STRING_TRIM) == "") return TRUE;
    gNcExecuting = TRUE;
    gNcThrottled = FALSE;
    integer sent = ruby(gNcBuffer);
    gNcExecuting = FALSE;
    if (gNcThrottled) return FALSE;
    if (!sent)
    {
        end_notecard("Could not send Ruby in notecard '" + gNcName + "'; stopped.");
        return FALSE;
    }
    gNcBuffer = "";
    return TRUE;
}

notecard_line(string data)
{
    if (gNcMode == NC_RUBY)
    {
        if (buffer_ruby(data)) gNcLine++;
        return;
    }

    string line = llStringTrim(data, STRING_TRIM);
    string lower = llToLower(line);
    if (gNcInBlock)
    {
        if (lower == "ruby-end")
        {
            if (!flush_ruby()) { gNcResumeAt = llGetUnixTime() + RETRY_SECONDS; return; }
            gNcInBlock = FALSE;
        }
        else if (!buffer_ruby(data)) return;   // keep indentation inside Ruby
        gNcLine++;
        return;
    }

    // Accept lines written exactly as typed in chat: "/7 ask ...".
    string prefix = "/" + (string)CONTROL_CHANNEL + " ";
    if (llSubStringIndex(line, prefix) == 0)
    {
        line = llStringTrim(llDeleteSubString(line, 0, llStringLength(prefix) - 1), STRING_TRIM);
        lower = llToLower(line);
    }

    if (line == "" || llGetSubString(line, 0, 0) == "#" || llGetSubString(line, 0, 1) == "//")
    {
        gNcLine++;
        return;
    }
    if (lower == "ruby-begin")
    {
        if (!ruby_ready()) { end_notecard("Notecard '" + gNcName + "' stopped at ruby-begin."); return; }
        gNcInBlock = TRUE;
        gNcBuffer = "";
        gNcLine++;
        return;
    }
    if (llSubStringIndex(lower, "wait ") == 0)
    {
        gNcResumeAt = llGetUnixTime() + (integer)llGetSubString(line, 5, -1);
        gNcLine++;
        return;
    }
    if (lower == "notecard-stop") { end_notecard("Notecard '" + gNcName + "' stopped itself."); return; }

    gNcExecuting = TRUE;
    gNcThrottled = FALSE;
    command(line, llKey2Name(gOwner));
    gNcExecuting = FALSE;
    if (gNcThrottled) gNcResumeAt = llGetUnixTime() + RETRY_SECONDS;   // same line again
    else gNcLine++;
}

notecard_eof()
{
    if (gNcMode == NC_RUBY)
    {
        if (!flush_ruby()) { gNcResumeAt = llGetUnixTime() + RETRY_SECONDS; return; }
        end_notecard("Sent notecard '" + gNcName + "' (" + (string)gNcLine + " lines) to Ruby.");
        return;
    }
    if (gNcInBlock) { end_notecard("Notecard '" + gNcName + "' ended inside ruby-begin without ruby-end."); return; }
    if (gNcWaitRequest != NULL_KEY || gNcWaitJob != "") return;   // finish after the last reply
    end_notecard("Finished notecard '" + gNcName + "'.");
}

// ---------------------------------------------------------------- UI

show_help(integer page)
{
    if (llGetInventoryType(HELP_SCRIPT) != INVENTORY_SCRIPT)
    {
        notify("Install " + HELP_SCRIPT + " in this prim to use /" + (string)CONTROL_CHANNEL + " help.");
        return;
    }
    llMessageLinked(LINK_THIS, HELP_LINK_MESSAGE, (string)page, gOwner);
}

show_status()
{
    string nc = "none";
    if (gNcMode != NC_OFF) nc = gNcName + " (line " + (string)(gNcLine + 1) + ")";
    notify(llDumpList2String([
        "Server: " + BASE_URL,
        "Team: " + TEAM_NAME,
        "Session: " + gSession,
        "Ruby: " + on_off(RUBY_TOKEN != ""),
        "Debug output: " + on_off(DEBUG_OUTPUT),
        "Public replies: " + on_off(PUBLIC_REPLIES),
        "Waiting replies: " + (string)(llGetListLength(gJobs) / 2),
        "Notecard: " + nc,
        "Free memory: " + (string)llGetFreeMemory()], "\n"));
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
    else if (verb == "ruby-reset") ruby_reset();
    else if (verb == "debug")
    {
        string setting = llToLower(rest);
        if (setting == "on" || setting == "off")
        {
            DEBUG_OUTPUT = (setting == "on");
            notify("Debug output " + on_off(DEBUG_OUTPUT));
        }
        else notify("Usage: debug on|off");
    }
    else if (verb == "session")
    {
        if (valid_session(rest)) { gSession = rest; notify("Ruby session: " + gSession); }
        else notify("Session must be 1-64 ASCII letters, digits, '_' or '-'.");
    }
    else if (verb == "session-reset") { gSession = (string)llGetKey(); notify("Ruby session: " + gSession); }
    else if (verb == "var-set" || verb == "var-get" || verb == "var-delete" || verb == "var-view"
        || verb == "file-write" || verb == "file-read" || verb == "file-delete" || verb == "file-list"
        || verb == "matrix-get") store_command(verb, rest);
    else if (verb == "notecard") start_notecard(rest, NC_COMMANDS);
    else if (verb == "ruby-notecard") start_notecard(rest, NC_RUBY);
    else if (verb == "notecard-stop")
    {
        if (gNcMode == NC_OFF) notify("No notecard is running.");
        else end_notecard("Stopped notecard '" + gNcName + "'.");
    }
    else if (verb == "team" && rest != "") { TEAM_NAME = rest; notify("Team: " + TEAM_NAME); }
    else if (verb == "public") { PUBLIC_REPLIES = is_on(rest); notify("Public replies " + on_off(PUBLIC_REPLIES)); }
    else if (verb == "url")
    {
        if (llSubStringIndex(rest, "https://") == 0)
        {
            if (llGetSubString(rest, -1, -1) == "/") rest = llDeleteSubString(rest, -1, -1);
            BASE_URL = rest;
            notify("Server: " + BASE_URL);
        }
        else notify("Usage: url https://host");
    }
    else if (verb == "health") send(KIND_HEALTH, "", BASE_URL + "/ollama/health", "GET", "", FALSE);
    else if (verb == "status") show_status();
    else if (verb == "help")
    {
        if (rest == "") show_help(0);
        else if (llStringLength(rest) == 1 && llSubStringIndex("12345678", rest) != -1)
            show_help((integer)rest);
        else notify("Usage: /" + (string)CONTROL_CHANNEL + " help <1-8>");
    }
    else ask(message, speaker);   // plain /7 text is treated as a question
}

handle_job_status(string job_id, string body)
{
    string job_state = llJsonGetValue(body, ["status"]);
    if (job_state == "pending") return;
    drop_job(job_id);
    if (job_state == "done")
        say("[ollama " + llJsonGetValue(body, ["seconds"]) + "s] ", llJsonGetValue(body, ["reply"]));
    else if (job_state == "error")
        notify("Ollama error: " + llJsonGetValue(body, ["error"]));
    else
        notify("Invalid Ollama job response: " + body);
}

// ---------------------------------------------------------------- events

default
{
    state_entry()
    {
        gOwner = llGetOwner();
        gSession = (string)llGetKey();
        gRequests = [];
        gJobs = [];
        gNcMode = NC_OFF;
        gNcQuery = NULL_KEY;
        gNcWaitRequest = NULL_KEY;
        update_timer();
        notify("Ready on /" + (string)CONTROL_CHANNEL + ". Install Chat and Help scripts, then say /"
            + (string)CONTROL_CHANNEL + " help");
    }

    on_rez(integer param) { llResetScript(); }

    changed(integer change)
    {
        if (change & CHANGED_OWNER) llResetScript();
        if ((change & CHANGED_INVENTORY) && gNcMode != NC_OFF
            && llGetInventoryKey(gNcName) != gNcAsset)
            end_notecard("Notecard '" + gNcName + "' was edited or removed; stopped. Run it again.");
    }

    touch_start(integer count)
    {
        if (llDetectedKey(0) == gOwner) { show_status(); show_help(0); }
    }

    link_message(integer sender_num, integer num, string message, key id)
    {
        if (id != gOwner) return;
        string speaker = llKey2Name(id);
        if (num == CHAT_COMMAND_MESSAGE) command(message, speaker);
    }

    dataserver(key query_id, string data)
    {
        if (gNcMode == NC_OFF || query_id != gNcQuery) return;
        gNcQuery = NULL_KEY;
        if (data == EOF) notecard_eof();
        else notecard_line(data);
        notecard_next();
    }

    http_response(key request, integer status, list metadata, string body)
    {
        integer index = llListFindList(gRequests, [request]);
        if (index == -1) return;
        integer kind = llList2Integer(gRequests, index + 1);
        string job_id = llList2String(gRequests, index + 2);
        gRequests = llDeleteSubList(gRequests, index, index + 2);
        integer for_notecard = (request == gNcWaitRequest);
        if (for_notecard) gNcWaitRequest = NULL_KEY;

        if (kind == KIND_ASK)
        {
            string id = llJsonGetValue(body, ["job"]);
            if (status == 202 && id != JSON_INVALID && id != JSON_NULL && id != "")
            {
                gJobs += [id, llGetUnixTime() + ASK_DEADLINE];
                if (for_notecard && gNcMode != NC_OFF) gNcWaitJob = id;
            }
            else notify("Ask failed (HTTP " + (string)status + "): " + body);
        }
        else if (kind == KIND_POLL)
        {
            if (status == 200) handle_job_status(job_id, body);
            else if (status == 404) { drop_job(job_id); notify("Reply expired on the server."); }
            // other statuses (e.g. 499 region timeout) are retried on a later tick
        }
        else if (kind == KIND_RUBY)
        {
            if (status == 200)
            {
                if (body != "") say("[ruby] ", body);
            }
            else if (status == 422) say("[ruby error] ", body);
            else if (status == 401) notify("Ruby token rejected (HTTP 401): RUBY_TOKEN must equal TIADE_RUBY_EVAL_TOKEN.");
            else if (status == 499) notify("Ruby response timed out (HTTP 499). The server may still be running it; wait before sending another Ruby command.");
            else notify("Ruby request failed (HTTP " + (string)status + "): " + body);
        }
        else if (kind == KIND_RESET)
        {
            if (status == 200) notify("Ruby: " + body);
            else notify("Ruby reset failed (HTTP " + (string)status + "): " + body);
        }
        else if (kind == KIND_HEALTH)
        {
            if (status == 200)
                notify("Server OK. Model: " + llJsonGetValue(body, ["configured_model"])
                    + ". Installed: " + llList2CSV(llJson2List(llJsonGetValue(body, ["available_models"]))));
            else notify("Health check failed (HTTP " + (string)status + "): " + body);
        }

        update_timer();
        notecard_next();
    }

    timer()
    {
        integer now = llGetUnixTime();
        integer i = 0;
        while (i < llGetListLength(gJobs))
        {
            if (now > llList2Integer(gJobs, i + 1))
            {
                drop_job(llList2String(gJobs, i));
                notify("Gave up waiting for an Ollama reply.");
            }
            else i += 2;
        }

        // One poll per tick, round-robin, to stay under the region HTTP throttle.
        integer count = llGetListLength(gJobs) / 2;
        if (count > 0)
        {
            if (gPollIndex >= count) gPollIndex = 0;
            string job_id = llList2String(gJobs, gPollIndex * 2);
            if (!poll_in_flight(job_id))
                send(KIND_POLL, job_id, BASE_URL + "/sl/job/" + job_id, "GET", "", FALSE);
            gPollIndex++;
        }

        if (gNcMode != NC_OFF)
        {
            if (gNcQuery != NULL_KEY && now - gNcQueryTime > READ_TIMEOUT)
                end_notecard("Timed out reading notecard '" + gNcName + "' line " + (string)(gNcLine + 1)
                    + ". An empty, never-saved notecard cannot be read.");
            else notecard_next();
        }
        update_timer();
    }
}
