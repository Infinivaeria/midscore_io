// tide_client.lsl
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

list requests = [];
list POST_HEADERS = [
    HTTP_METHOD, "POST",
    HTTP_MIMETYPE, "application/json"
];

list kv = []; // local cache

integer CHANNEL_OUTPUT = 0; // public channel

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

debug(string msg)
{
    llOwnerSay(msg);
}

// =========================
// DEFAULT STATE
// =========================

default
{
    state_entry()
    {
        llListen(0, "", llGetOwner(), "");
        debug("Tide client ready. Commands: set key value | get key | view | delete key | clear | status | history | channel <num>");
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
            debug("Output channel set to " + (string)CHANNEL_OUTPUT);
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

            key req = llHTTPRequest(SERVERURL + "/vars/set", POST_HEADERS, json);
            requests += [(string)req, "set", keyVal];

            say("Sent set request for " + keyVal);
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

            key req = llHTTPRequest(SERVERURL + "/vars/get", POST_HEADERS, json);
            requests += [(string)req, "get", keyVal];

            say("Sent get request for " + keyVal);
            return;
        }

        // =========================
        // VIEW
        // =========================
        if (cmd == "view")
        {
            string json = llJsonSetValue("{}", ["action"], "view");

            key req = llHTTPRequest(SERVERURL + "/vars/view", POST_HEADERS, json);
            requests += [(string)req, "view", ""];

            say("Sent view request");
            return;
        }

        // =========================
        // DELETE
        // =========================
        if (cmd == "delete" && n >= 2)
        {
            string keyVal = escape(llList2String(parts, 1));

            string json = llJsonSetValue("{}", ["name"], keyVal);

            key req = llHTTPRequest(SERVERURL + "/vars/delete", POST_HEADERS, json);
            requests += [(string)req, "delete", keyVal];

            say("Sent delete request for " + keyVal);
            return;
        }

        // =========================
        // CLEAR
        // =========================
        if (cmd == "clear")
        {
            string json = llJsonSetValue("{}", ["action"], "clear");

            key req = llHTTPRequest(SERVERURL + "/vars/clear", POST_HEADERS, json);
            requests += [(string)req, "clear", ""];

            say("Sent clear request");
            return;
        }

        // =========================
        // STATUS
        // =========================
        if (cmd == "status")
        {
            string json = llJsonSetValue("{}", ["action"], "status");

            key req = llHTTPRequest(SERVERURL + "/vars/status", POST_HEADERS, json);
            requests += [(string)req, "status", ""];

            say("Sent status request");
            return;
        }

        // =========================
        // HISTORY
        // =========================
        if (cmd == "history")
        {
            string json = llJsonSetValue("{}", ["action"], "history");

            key req = llHTTPRequest(SERVERURL + "/vars/history", POST_HEADERS, json);
            requests += [(string)req, "history", ""];

            say("Sent history request");
            return;
        }

        debug("Unknown command: " + cmd);
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
            debug("HTTP response (unknown request): " + (string)status + " body: " + body);
            return;
        }

        string action = llList2String(requests, idx + 1);
        string arg    = llList2String(requests, idx + 2);

        requests = llDeleteSubList(requests, idx, idx + 2);

        say("HTTP response for " + action + " (status " + (string)status + "):");

        if (llStringLength(body) == 0)
        {
            say("Empty body");
            return;
        }

        say(body);

        if (action == "get")
        {
            string val = llJsonGetValue(body, [arg]);

            if (llStringLength(val) > 0)
            {
                kv_set(arg, unescape(val));
                debug("Cached " + arg + " = " + val);
            }
        }
    }
}