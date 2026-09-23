// RustForth
// LSL client for Tide-style REST API endpoints.

// =========================
// ESCAPE / UNESCAPE
// (kept exactly as provided by you)
// =========================

string ESCAPE_CHARACTER = "\\";
string ESCAPE_CHARACTER_REPLACE = "\\\\";

string escape(string avatar_message)
{
       list avatar_message_string = llParseString2List(avatar_message, [""], []); 
            integer counter = 0;            
            list string_chars = [];
            for (counter = 0; counter < llStringLength(avatar_message); counter++)
            {
                string_chars = string_chars + llGetSubString(avatar_message, counter, counter);
                if (llGetSubString(avatar_message, counter, counter) == ESCAPE_CHARACTER)
                {
                    string_chars = llListReplaceList(string_chars, ["/"], counter, counter );    
                }
                
                 else if (llGetSubString(avatar_message, counter, counter) == ":")
                {
                    string_chars = llListReplaceList(string_chars, ["\\:"], counter, counter );
        
                }
                
                  else if (llGetSubString(avatar_message, counter, counter) == "[")
                {
                    string_chars = llListReplaceList(string_chars, ["\\["], counter, counter );
             
                }
                
                
                    else if (llGetSubString(avatar_message, counter, counter) == "]")
                {
                    string_chars = llListReplaceList(string_chars, ["\\]"], counter, counter );
                  //  llSay(0, (string)string_chars);
                }
                
                      else if (llGetSubString(avatar_message, counter, counter) == "{")
                {
                    string_chars = llListReplaceList(string_chars, ["\\{"], counter, counter );
                  //  llSay(0, (string)string_chars);
                }
                
                
                       else if (llGetSubString(avatar_message, counter, counter) == "}")
                {
                    string_chars = llListReplaceList(string_chars, ["\\}"], counter, counter );
                   // llSay(0, (string)string_chars);
                }
                
                        else if (llGetSubString(avatar_message, counter, counter) == "'")
                {
                    string_chars = llListReplaceList(string_chars, ["\\'"], counter, counter );
                }
                        else if (llGetSubString(avatar_message, counter, counter) == "\"")
                {
                    string_chars = llListReplaceList(string_chars, ["\\\""], counter, counter );
                }
                
                 else if (llGetSubString(avatar_message, counter, counter) == ".")
                {
                    string_chars = llListReplaceList(string_chars, ["\\."], counter, counter );
                }
                
                }
            return llDumpList2String(string_chars, "");
    }
    
string unescape(string avatar_message)
{
          list avatar_message_string = llParseString2List(avatar_message, [""], []);
            integer counter = 0;            
            list string_chars = [];
            for (counter = 0; counter < llStringLength(avatar_message); counter++)
            {
                string_chars = string_chars + llGetSubString(avatar_message, counter, counter);
                if (llGetSubString(avatar_message, counter, counter) == "\\/")
                {
                    string_chars = llListReplaceList(string_chars, [ESCAPE_CHARACTER], counter, counter );
                    //llSay(0, (string)string_chars);
                }
                
                 else if (llGetSubString(avatar_message, counter, counter) == "\\:")
                {
                    string_chars = llListReplaceList(string_chars, [":"], counter, counter );
                   // llSay(0, (string)string_chars);
                }
                
                  else if (llGetSubString(avatar_message, counter, counter) == "\\[")
                {
                    string_chars = llListReplaceList(string_chars, ["["], counter, counter );
                   // llSay(0, (string)string_chars);
                }
                
                
                    else if (llGetSubString(avatar_message, counter, counter) == "\\]")
                {
                    string_chars = llListReplaceList(string_chars, ["]"], counter, counter );
                  //  llSay(0, (string)string_chars);
                }
                
                      else if (llGetSubString(avatar_message, counter, counter) == "\\{")
                {
                    string_chars = llListReplaceList(string_chars, ["{"], counter, counter );
                  //  llSay(0, (string)string_chars);
                }
                
                
                       else if (llGetSubString(avatar_message, counter, counter) == "\\}")
                {
                    string_chars = llListReplaceList(string_chars, ["}"], counter, counter );
                  //  llSay(0, (string)string_chars);
                }
                
                
            }
            return llDumpList2String(string_chars, ""); 
}


// =========================
// GLOBALS
// =========================

string SERVERURL = "https://stimky.info";
string API_KEY = "";
string BRIDGE_TOKEN = ""; // Set this to the MSSL_FORTH_BRIDGE_TOKEN value.

list requests = [];
list POST_HEADERS = [
    HTTP_METHOD, "POST",
    HTTP_MIMETYPE, "application/json"
];

list kv = []; // local cache

integer CHANNEL_OUTPUT = 0; // public channel
integer CHANNEL_INPUT = 1111; // set variables etc to a different channel
integer BRIDGE_ENABLED = FALSE;
integer BRIDGE_POLL_IN_FLIGHT = FALSE;
float BRIDGE_POLL_SECONDS = 3.0;
string SESSION_ID = "";
key notecard_query = NULL_KEY;
integer notecard_line = 0;
string notecard_name = "";
string notecard_source = "";
// =========================
// KV helpers (added — required by http_response)
// =========================

string kv_get(string keyVal)
{
    integer i = llListFindList(kv, [keyVal]);
    if (i != -1 && (i + 1) < llGetListLength(kv)) return llList2String(kv, i + 1);
    return "";
}

kv_set(string keyVal, string value)
{
    integer i = llListFindList(kv, [keyVal]);
    if (i != -1)
    {
        kv = llListReplaceList(kv, [value], i + 1, i + 1);
    }
    else
    {
        kv += [keyVal, value];
    }
}

kv_delete(string keyVal)
{
    integer i = llListFindList(kv, [keyVal]);
    if (i != -1) kv = llDeleteSubList(kv, i, i + 1);
}

// =========================
// OUTPUT HELPERS
// =========================

say(string msg)
{
    llSay(CHANNEL_OUTPUT, msg);
}

say_chunks(integer channel, string msg)
{
    integer start = 0;
    integer length = llStringLength(msg);
    while (start < length)
    {
        llSay(channel, llGetSubString(msg, start, start + 899));
        start += 900;
    }
}

debug(string msg)
{
    llOwnerSay(msg);
}

show_help()
{
    debug("RubyForth/Forth: separate sequential statements with ';'. Example: /1111 ruby count = 1; puts count; count += 1; puts count");
    debug("In a RubyForth notecard, use: forth do <Forth source> end. Both languages share variables and the active session.");
    debug("Session commands: /1111 session <safe-name> | /1111 session-reset. The default session is this prim UUID.");
}

forth_post(string endpoint, string action, string argument, string payload)
{
    payload = llJsonSetValue(payload, ["session_id"], SESSION_ID);
    key request_id = llHTTPRequest(SERVERURL + endpoint, POST_HEADERS, payload);
    requests += [(string)request_id, action, argument];
}

string forth_name_payload(string name)
{
    return llJsonSetValue("{}", ["name"], name);
}

string forth_file_payload(string name, string content)
{
    string json = llJsonSetValue("{}", ["name"], name);
    return llJsonSetValue(json, ["content"], content);
}

string forth_source_payload(string source, integer max_steps)
{
    string json = llJsonSetValue("{}", ["source"], source);
    if (max_steps > 0) json = llJsonSetValue(json, ["max_steps"], (string)max_steps);
    return json;
}

string forth_name_steps_payload(string name, integer max_steps)
{
    string json = llJsonSetValue("{}", ["name"], name);
    return llJsonSetValue(json, ["max_steps"], (string)max_steps);
}

string forth_bridge_payload()
{
    return llJsonSetValue("{}", ["token"], BRIDGE_TOKEN);
}

bridge_poll()
{
    if (!BRIDGE_ENABLED || BRIDGE_POLL_IN_FLIGHT || llStringLength(BRIDGE_TOKEN) == 0) return;
    BRIDGE_POLL_IN_FLIGHT = TRUE;
    forth_post("/forth/bridge/poll", "bridge", "", forth_bridge_payload());
}

float color_component(string value)
{
    float component = (float)value;
    if (component > 1.0) component = component / 255.0;
    if (component < 0.0) return 0.0;
    if (component > 1.0) return 1.0;
    return component;
}

float alpha_component(string value)
{
    return color_component(value);
}

run_forth_calls(string body)
{
    integer index = 0;
    string call = llJsonGetValue(body, ["calls", index]);

    while (call != JSON_INVALID)
    {
        string operation = llJsonGetValue(call, ["op"]);
        if (operation == "say")
        {
            llSay((integer)llJsonGetValue(call, ["channel"]), llJsonGetValue(call, ["message"]));
        }
        else if (operation == "whisper")
        {
            llWhisper((integer)llJsonGetValue(call, ["channel"]), llJsonGetValue(call, ["message"]));
        }
        else if (operation == "shout")
        {
            llShout((integer)llJsonGetValue(call, ["channel"]), llJsonGetValue(call, ["message"]));
        }
        else if (operation == "region_say")
        {
            llRegionSay((integer)llJsonGetValue(call, ["channel"]), llJsonGetValue(call, ["message"]));
        }
        else if (operation == "owner_say")
        {
            llOwnerSay(llJsonGetValue(call, ["message"]));
        }
        else if (operation == "set_text")
        {
            llSetText(
                llJsonGetValue(call, ["text"]),
                <color_component(llJsonGetValue(call, ["red"])), color_component(llJsonGetValue(call, ["green"])), color_component(llJsonGetValue(call, ["blue"]))>,
                alpha_component(llJsonGetValue(call, ["alpha"]))
            );
        }
        else if (operation == "set_color")
        {
            llSetColor(
                <color_component(llJsonGetValue(call, ["red"])), color_component(llJsonGetValue(call, ["green"])), color_component(llJsonGetValue(call, ["blue"]))>,
                (integer)llJsonGetValue(call, ["face"])
            );
        }
        else if (operation == "set_alpha")
        {
            llSetAlpha(alpha_component(llJsonGetValue(call, ["alpha"])), (integer)llJsonGetValue(call, ["face"]));
        }
        else if (operation == "play_sound")
        {
            llPlaySound(llJsonGetValue(call, ["sound"]), alpha_component(llJsonGetValue(call, ["volume"])));
        }
        else if (operation == "set_timer")
        {
            llSetTimerEvent((float)llJsonGetValue(call, ["seconds"]));
        }
        else if (operation == "set_region_pos")
        {
            llSetRegionPos(<(float)llJsonGetValue(call, ["x"]), (float)llJsonGetValue(call, ["y"]), (float)llJsonGetValue(call, ["z"])>);
        }
        else if (operation == "link_message")
        {
            llMessageLinked(
                (integer)llJsonGetValue(call, ["link"]),
                (integer)llJsonGetValue(call, ["code"]),
                llJsonGetValue(call, ["message"]),
                (key)llJsonGetValue(call, ["id"])
            );
        }
        index++;
        call = llJsonGetValue(body, ["calls", index]);
    }
}

// =========================
// DEFAULT STATE
// =========================

default
{
    
    on_rez(integer start_parameter)
    {
     llOwnerSay("Reloading variable functions for the tide to second life server interaction... Input on /1111 ..");   
    }
    
    state_entry()
    {
        SESSION_ID = (string)llGetKey();
        llListen(CHANNEL_INPUT, "", NULL_KEY, "");
        debug("Tide stack ready. Use /1111 help for RubyForth, Forth, session, and bridge syntax.");
    }

    listen(integer chan, string name, key id, string msg)
    {
        if (id != llGetOwner()) return;

        list parts = llParseString2List(msg, [" "], []);
        integer n = llGetListLength(parts);
        if (n == 0) return;

        string cmd = llToLower(llList2String(parts, 0));

        // =========================
        // CHANNEL SWITCH
        // =========================
        if (cmd == "channel" && n >= 2)
        {
            CHANNEL_OUTPUT = (integer)llList2String(parts, 1);
            //debug("Output channel set to " + (string)CHANNEL_OUTPUT);
            return;
        }

        if (cmd == "help")
        {
            show_help();
            return;
        }

        if (cmd == "session" && n == 2)
        {
            SESSION_ID = llList2String(parts, 1);
            debug("Active language session: " + SESSION_ID);
            return;
        }

        if (cmd == "session-reset")
        {
            SESSION_ID = (string)llGetKey();
            debug("Active language session reset to prim UUID: " + SESSION_ID);
            return;
        }

        if (cmd == "bridge-on")
        {
            if (llStringLength(BRIDGE_TOKEN) == 0)
            {
                debug("Set BRIDGE_TOKEN in this script before enabling the Second Life bridge.");
                return;
            }
            BRIDGE_ENABLED = TRUE;
            llSetTimerEvent(BRIDGE_POLL_SECONDS);
            bridge_poll();
            return;
        }

        if (cmd == "bridge-off")
        {
            BRIDGE_ENABLED = FALSE;
            BRIDGE_POLL_IN_FLIGHT = FALSE;
            llSetTimerEvent(0.0);
            return;
        }

        if (cmd == "bridge-poll")
        {
            bridge_poll();
            return;
        }

        // =========================
        // SET
        // =========================
        if (cmd == "set" && n >= 3)
        {
            string keyVal = escape(llList2String(parts, 1));
            string value  = escape(llDumpList2String(llList2List(parts, 2, -1), " "));

            string json = "{}";
            json = llJsonSetValue(json, [keyVal], value);
            json = llJsonSetValue(json, ["session_id"], SESSION_ID);

            key req = llHTTPRequest(SERVERURL + "/vars/set", POST_HEADERS, json);
            requests += [(string)req, "set", keyVal];

            //say("Sent set request for " + keyVal);
            return;
        }

        // =========================
        // GET
        // =========================
        if (cmd == "get" && n >= 2)
        {
            string keyVal = escape(llList2String(parts, 1));

            string json = "{}";
            json = llJsonSetValue(json, ["name"], keyVal);
            json = llJsonSetValue(json, ["session_id"], SESSION_ID);

            key req = llHTTPRequest(SERVERURL + "/vars/get", POST_HEADERS, json);
            requests += [(string)req, "get", keyVal];

            //say("Sent get request for " + keyVal);
            return;
        }

        // =========================
        // VIEW
        // =========================
        if (cmd == "view")
        {
            string json = llJsonSetValue("{}", ["action"], "view");
            json = llJsonSetValue(json, ["session_id"], SESSION_ID);

            key req = llHTTPRequest(SERVERURL + "/vars/view", POST_HEADERS, json);
            requests += [(string)req, "view", ""];

            //say("Sent view request");
            return;
        }

        // =========================
        // DELETE
        // =========================
        if (cmd == "delete" && n >= 2)
        {
            string keyVal = escape(llList2String(parts, 1));

            string json = llJsonSetValue("{}", ["name"], keyVal);
            json = llJsonSetValue(json, ["session_id"], SESSION_ID);

            key req = llHTTPRequest(SERVERURL + "/vars/delete", POST_HEADERS, json);
            requests += [(string)req, "delete", keyVal];

           // say("Sent delete request for " + keyVal);
            return;
        }

        // =========================
        // CLEAR
        // =========================
        if (cmd == "clear")
        {
            string json = llJsonSetValue("{}", ["action"], "clear");
            json = llJsonSetValue(json, ["session_id"], SESSION_ID);

            key req = llHTTPRequest(SERVERURL + "/vars/clear", POST_HEADERS, json);
            requests += [(string)req, "clear", ""];

            //say("Sent clear request");
            return;
        }

        // =========================
        // STATUS
        // =========================
        if (cmd == "status")
        {
            string json = llJsonSetValue("{}", ["action"], "status");
            json = llJsonSetValue(json, ["session_id"], SESSION_ID);

            key req = llHTTPRequest(SERVERURL + "/vars/status", POST_HEADERS, json);
            requests += [(string)req, "status", ""];

            //say("Sent status request");
            return;
        }

        // =========================
        // HISTORY
        // =========================
        if (cmd == "history")
        {
            string json = llJsonSetValue("{}", ["action"], "history");
            json = llJsonSetValue(json, ["session_id"], SESSION_ID);

            key req = llHTTPRequest(SERVERURL + "/vars/history", POST_HEADERS, json);
            requests += [(string)req, "history", ""];

            //say("Sent history request");
            return;
        }

        // Run Forth and announce its output on the selected output channel.
        if (cmd == "forth-steps" && n >= 3)
        {
            integer max_steps = (integer)llList2String(parts, 1);
            if (max_steps < 1 || max_steps > 1000000)
            {
                debug("max_steps must be between 1 and 1000000.");
                return;
            }
            string source = llDumpList2String(llList2List(parts, 2, -1), " ");
            forth_post("/forth/eval", "forth", (string)CHANNEL_OUTPUT, forth_source_payload(source, max_steps));
            return;
        }

        if (cmd == "forth" && n >= 2)
        {
            string source = llDumpList2String(llList2List(parts, 1, -1), " ");
            string json = llJsonSetValue("{}", ["source"], source);
            forth_post("/forth/eval", "forth", (string)CHANNEL_OUTPUT, json);
            return;
        }

        if (cmd == "hybrid" && n >= 2)
        {
            string source = llDumpList2String(llList2List(parts, 1, -1), " ");
            forth_post("/forth/eval", "forth", (string)CHANNEL_OUTPUT, forth_source_payload(source, 0));
            return;
        }

        // Run Forth and announce only this result on an arbitrary SL channel.
        if (cmd == "forth-say" && n >= 3)
        {
            integer reply_channel = (integer)llList2String(parts, 1);
            string source = llDumpList2String(llList2List(parts, 2, -1), " ");
            string json = llJsonSetValue("{}", ["source"], source);
            forth_post("/forth/eval", "forth", (string)reply_channel, json);
            return;
        }

        if (cmd == "hybrid-say" && n >= 3)
        {
            integer reply_channel = (integer)llList2String(parts, 1);
            string source = llDumpList2String(llList2List(parts, 2, -1), " ");
            forth_post("/forth/eval", "forth", (string)reply_channel, forth_source_payload(source, 0));
            return;
        }

        // Run Ruby-shaped source through the server's RubyForth compiler.
        if (cmd == "ruby-steps" && n >= 3)
        {
            integer max_steps = (integer)llList2String(parts, 1);
            if (max_steps < 1 || max_steps > 1000000)
            {
                debug("max_steps must be between 1 and 1000000.");
                return;
            }
            string source = llDumpList2String(llList2List(parts, 2, -1), " ");
            forth_post("/ruby/eval", "forth", (string)CHANNEL_OUTPUT, forth_source_payload(source, max_steps));
            return;
        }

        if (cmd == "ruby" && n >= 2)
        {
            string source = llDumpList2String(llList2List(parts, 1, -1), " ");
            string json = llJsonSetValue("{}", ["source"], source);
            forth_post("/ruby/eval", "forth", (string)CHANNEL_OUTPUT, json);
            return;
        }

        if (cmd == "ruby-say" && n >= 3)
        {
            integer reply_channel = (integer)llList2String(parts, 1);
            string source = llDumpList2String(llList2List(parts, 2, -1), " ");
            string json = llJsonSetValue("{}", ["source"], source);
            forth_post("/ruby/eval", "forth", (string)reply_channel, json);
            return;
        }

        if (cmd == "algebra" && n >= 2)
        {
            string expression = llDumpList2String(llList2List(parts, 1, -1), " ");
            string json = llJsonSetValue("{}", ["expression"], expression);
            forth_post("/forth/algebra", "algebra", (string)CHANNEL_OUTPUT, json);
            return;
        }

        if (cmd == "algebra-say" && n >= 3)
        {
            integer reply_channel = (integer)llList2String(parts, 1);
            string expression = llDumpList2String(llList2List(parts, 2, -1), " ");
            string json = llJsonSetValue("{}", ["expression"], expression);
            forth_post("/forth/algebra", "algebra", (string)reply_channel, json);
            return;
        }

        if (cmd == "matrix-get" && n == 2)
        {
            string matrix_name = llList2String(parts, 1);
            forth_post("/forth/matrices/get", "matrix", (string)CHANNEL_OUTPUT, forth_name_payload(matrix_name));
            return;
        }

        if (cmd == "matrix-get-say" && n == 3)
        {
            integer reply_channel = (integer)llList2String(parts, 1);
            string matrix_name = llList2String(parts, 2);
            forth_post("/forth/matrices/get", "matrix", (string)reply_channel, forth_name_payload(matrix_name));
            return;
        }

        if (cmd == "file-write" && n >= 3)
        {
            string file_name = llList2String(parts, 1);
            string content = llDumpList2String(llList2List(parts, 2, -1), " ");
            forth_post("/forth/files/write", "file-write", (string)CHANNEL_OUTPUT, forth_file_payload(file_name, content));
            return;
        }

        if (cmd == "file-read" && n == 2)
        {
            string file_name = llList2String(parts, 1);
            forth_post("/forth/files/read", "file-read", (string)CHANNEL_OUTPUT, forth_name_payload(file_name));
            return;
        }

        if (cmd == "file-read-say" && n == 3)
        {
            integer reply_channel = (integer)llList2String(parts, 1);
            string file_name = llList2String(parts, 2);
            forth_post("/forth/files/read", "file-read", (string)reply_channel, forth_name_payload(file_name));
            return;
        }

        if (cmd == "files")
        {
            forth_post("/forth/files/list", "files", (string)CHANNEL_OUTPUT, "{}");
            return;
        }

        if (cmd == "file-delete" && n == 2)
        {
            string file_name = llList2String(parts, 1);
            forth_post("/forth/files/delete", "file-delete", (string)CHANNEL_OUTPUT, forth_name_payload(file_name));
            return;
        }

        // Upload an inventory notecard as a named remote Forth program.
        if (cmd == "notecard" && n == 2)
        {
            string card_name = llList2String(parts, 1);
            if (llGetInventoryType(card_name) != INVENTORY_NOTECARD)
            {
                say("No notecard named " + card_name + " in this object.");
                return;
            }
            notecard_name = card_name;
            notecard_source = "";
            notecard_line = 0;
            notecard_query = llGetNotecardLine(notecard_name, notecard_line);
            say("Loading " + notecard_name + " into the remote Forth database.");
            return;
        }

        if (cmd == "forth-run" && n == 2)
        {
            string card_name = llList2String(parts, 1);
            forth_post("/forth/notecards/run", "forth", (string)CHANNEL_OUTPUT, forth_name_payload(card_name));
            return;
        }

        if (cmd == "forth-run-steps" && n == 3)
        {
            integer max_steps = (integer)llList2String(parts, 1);
            if (max_steps < 1 || max_steps > 1000000)
            {
                debug("max_steps must be between 1 and 1000000.");
                return;
            }
            string card_name = llList2String(parts, 2);
            forth_post("/forth/notecards/run", "forth", (string)CHANNEL_OUTPUT, forth_name_steps_payload(card_name, max_steps));
            return;
        }

        if (cmd == "forth-run-say" && n == 3)
        {
            integer reply_channel = (integer)llList2String(parts, 1);
            string card_name = llList2String(parts, 2);
            forth_post("/forth/notecards/run", "forth", (string)reply_channel, forth_name_payload(card_name));
            return;
        }

        if (cmd == "ruby-run" && n == 2)
        {
            string card_name = llList2String(parts, 1);
            forth_post("/ruby/notecards/run", "forth", (string)CHANNEL_OUTPUT, forth_name_payload(card_name));
            return;
        }

        if (cmd == "ruby-run-steps" && n == 3)
        {
            integer max_steps = (integer)llList2String(parts, 1);
            if (max_steps < 1 || max_steps > 1000000)
            {
                debug("max_steps must be between 1 and 1000000.");
                return;
            }
            string card_name = llList2String(parts, 2);
            forth_post("/ruby/notecards/run", "forth", (string)CHANNEL_OUTPUT, forth_name_steps_payload(card_name, max_steps));
            return;
        }

        if (cmd == "ruby-run-say" && n == 3)
        {
            integer reply_channel = (integer)llList2String(parts, 1);
            string card_name = llList2String(parts, 2);
            forth_post("/ruby/notecards/run", "forth", (string)reply_channel, forth_name_payload(card_name));
            return;
        }

        if (cmd == "notecards")
        {
            forth_post("/forth/notecards/list", "notecards", "", "{}");
            return;
        }

        if (cmd == "notecard-get" && n == 2)
        {
            string card_name = llList2String(parts, 1);
            forth_post("/forth/notecards/get", "notecard-get", card_name, forth_name_payload(card_name));
            return;
        }

        if (cmd == "notecard-delete" && n == 2)
        {
            string card_name = llList2String(parts, 1);
            forth_post("/forth/notecards/delete", "notecard-delete", card_name, forth_name_payload(card_name));
            return;
        }

        //debug("Unknown command: " + cmd);
    }

    // =========================
    // HTTP RESPONSE
    // =========================

    http_response(key request_id, integer status, list metadata, string body)
    {
        string rid = (string)request_id;
        integer idx = llListFindList(requests, [rid]);

        if (idx == -1)
        {
            //debug("HTTP response (unknown request): " + (string)status + " body: " + body);
            return;
        }

        string action = llList2String(requests, idx + 1);
        string arg    = llList2String(requests, idx + 2);

        requests = llDeleteSubList(requests, idx, idx + 2);

        //say("HTTP response for " + action + " (status " + (string)status + "):");

        if (llStringLength(body) == 0)
        {
            if (action == "bridge") BRIDGE_POLL_IN_FLIGHT = FALSE;
            //say("Empty body");
            return;
        }

        if (action == "bridge")
        {
            BRIDGE_POLL_IN_FLIGHT = FALSE;
            string source = llJsonGetValue(body, ["message", "source"]);
            if (source != JSON_INVALID)
            {
                string payload = llJsonSetValue("{}", ["source"], source);
                string max_steps = llJsonGetValue(body, ["message", "max_steps"]);
                if (max_steps != JSON_INVALID && max_steps != JSON_NULL)
                {
                    payload = llJsonSetValue(payload, ["max_steps"], max_steps);
                }
                string language = llJsonGetValue(body, ["message", "language"]);
                string endpoint = "/forth/eval";
                if (language == "ruby") endpoint = "/ruby/eval";
                forth_post(endpoint, "forth", (string)CHANNEL_OUTPUT, payload);
            }
        }
        else if (action == "forth")
        {
            string output = llJsonGetValue(body, ["output"]);
            integer reply_channel = (integer)arg;
            run_forth_calls(body);
            if (output != JSON_INVALID && llStringLength(output) > 0)
            {
                llSay(reply_channel, output);
            }
            else
            {
                llSay(reply_channel, body);
            }
        }
        else if (action == "file-read")
        {
            string content = llJsonGetValue(body, ["content"]);
            integer reply_channel = (integer)arg;
            if (content != JSON_INVALID)
            {
                say_chunks(reply_channel, content);
            }
            else
            {
                say_chunks(reply_channel, body);
            }
        }
        else if (action == "algebra")
        {
            integer reply_channel = (integer)arg;
            string simplified = llJsonGetValue(body, ["simplified"]);
            string derivative = llJsonGetValue(body, ["derivative"]);
            string integral = llJsonGetValue(body, ["integral"]);
            if (simplified != JSON_INVALID)
            {
                say_chunks(reply_channel, "f(x) = " + simplified + " | f'(x) = " + derivative + " | integral = " + integral);
            }
            else
            {
                say_chunks(reply_channel, body);
            }
        }
        else if (action == "matrix" || action == "files" || action == "file-write" || action == "file-delete")
        {
            say_chunks((integer)arg, body);
        }
        else
        {
            say_chunks(CHANNEL_OUTPUT, body);
        }

        if (action == "get")
        {
            string val = llJsonGetValue(body, [arg]);

            if (llStringLength(val) > 0)
            {
                kv_set(arg, unescape(val));
                //debug("Cached " + arg + " = " + val);
            }
        }
    }

    timer()
    {
        bridge_poll();
    }

    dataserver(key query_id, string data)
    {
        if (query_id != notecard_query) return;

        if (data != EOF)
        {
            notecard_source += data + "\n";
            ++notecard_line;
            notecard_query = llGetNotecardLine(notecard_name, notecard_line);
            return;
        }

        string json = llJsonSetValue("{}", ["name"], notecard_name);
        json = llJsonSetValue(json, ["source"], notecard_source);
        forth_post("/forth/notecards/save", "notecard-save", notecard_name, json);
        notecard_query = NULL_KEY;
        notecard_source = "";
    }
}
