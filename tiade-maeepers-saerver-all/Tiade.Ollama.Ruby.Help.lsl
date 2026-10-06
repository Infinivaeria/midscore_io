// Put this script beside the combined Tiade.Ollama.Ruby.lsl in the same prim.
// Help requests arrive by link message so the HTTP/notecard script does not
// need to hold the help text in its own limited script memory.

integer CONTROL_CHANNEL = 7;       // must match Tiade.Ollama.Ruby.lsl
integer HELP_LINK_MESSAGE = -708641;

notify(string text)
{
    llOwnerSay("[tiade] " + text);
}

command_list()
{
    string prefix = "/" + (string)CONTROL_CHANNEL + " ";
    notify("ALL COMMANDS - owner only; prefix each command with " + prefix + ". <...> means your value, not literal brackets. Unknown command text asks Ollama.");
    notify("OLLAMA: ask <message> | team <name> | url <https://host> | public on|off | health | status");
    notify("RUBY: ruby <code> | ruby-reset | debug on|off | session <id> | session-reset");
    notify("LOCAL CHAT: local-input | local-input-ruby <code> | ruby-local-input <code> (alias). Channel-0 text is captured only; never auto-executed or sent.");
    notify("PERSISTENT VARIABLES: var-set <name> <text> | var-get <name> | var-delete <name> | var-view. var-set stores text; typed values: " + prefix + "ruby var_set('score', 21). Successful writes/deletes persist in Rust's partitioned array on disk; Ruby locals do not.");
    notify("FILES/MATRICES: file-write <name> <text> | file-read <name> | file-delete <name> | file-list | matrix-get <name>");
    notify("CARDS: notecard <name> | ruby-notecard <name> | local-input-ruby-notecard <name> | ruby-local-input-notecard <name> (alias) | notecard-stop");
    notify("HELP: help (commands and index) | help all (command list) | help <1-8> (detailed topic). Command-card-only directives: wait <seconds>, ruby-begin, ruby-end; blank lines and # or // comments are skipped outside Ruby blocks.");
    notify("RUBY HELPERS: var_set(name, value), var_get(name), var_delete(name), var_view(), file_write(name, text), file_read(name), file_delete(name), file_list(), matrix_get(name). Storage uses the selected session; ruby-reset does not erase it. With debug off, use puts/print to see values.");
}

help(integer page)
{
    string c = "/" + (string)CONTROL_CHANNEL + " ";
    if (page == 0)
    {
        command_list();
        notify("HELP: " + c + "help <1-8> shows one page. " + c + "help all lists every command. " + c + "status shows current settings.");
        notify("1 Setup | 2 Ollama | 3 Public replies | 4 Ruby | 5 Variables | 6 Files/matrices | 7 Command cards | 8 Ruby cards/troubleshooting");
    }
    else if (page == 1)
    {
        notify("HELP 1/8 - SETUP (only the object owner can use /" + (string)CONTROL_CHANNEL + ")");
        notify("Install only Tiade.Ollama.Ruby.lsl and Tiade.Ollama.Ruby.Help.lsl in the same prim, using these exact names. Remove the old Chat and LocalInput scripts. Set BASE_URL and RUBY_TOKEN in the main script; CONTROL_CHANNEL must match in both scripts.");
        notify("The combined main script owns both listeners. It captures channel-0 chat from nearby speakers and stores only the latest message in gLastLocalChat. It does not send chat to the server or run it as a command.");
        notify(c + "health checks server/Ollama; " + c + "status shows server, team, session, Ruby enabled, replies, notecard progress and a preview of the latest local input. " + c + "local-input shows the complete stored local-input value to the owner. Touch the object for status and the help index.");
        notify("Ruby executes on your server with its privileges. Only use a trusted server and keep the token private; changing a script requires recompiling it in-world.");
    }
    else if (page == 2)
    {
        notify("HELP 2/8 - OLLAMA QUESTIONS AND SETTINGS");
        notify(c + "ask <message> - send a question; example: " + c + "ask What is a prim? Plain unrecognized /" + (string)CONTROL_CHANNEL + " text is also sent as a question.");
        notify(c + "team <name> - choose a team and its shared Ollama history (default: secondlife). Team does not change the Ruby session.");
        notify(c + "url <https://host> - switch server for this script, without a trailing slash; " + c + "health checks connectivity.");
        notify("Ollama replies are queued, then polled every 2s; at most 4 may be pending. A reply is abandoned after 240s.");
    }
    else if (page == 3)
    {
        notify("HELP 3/8 - PUBLIC REPLIES");
        notify(c + "public on - say Ollama answers and Ruby output in nearby public chat (channel 0). Anyone nearby may see them; do not use for private data.");
        notify(c + "public off - send Ollama answers and Ruby output to the owner only. Errors from Ruby evaluation follow the same reply setting.");
        notify("Status, help, settings confirmations, HTTP diagnostics and Ruby reset messages remain owner-only. The public setting resets to off when the main script resets, recompiles or re-rezzes.");
        notify("Use " + c + "status to check the current reply setting. Nearby channel-0 chat is stored only in gLastLocalChat by the main script. " + c + "local-input returns the full value to the owner. It is cleared when the main script resets and is never relayed automatically.");
        notify("For a complete local-input setup and Ruby guide, upload LocalInputHelp.txt as a notecard in this object.");
    }
    else if (page == 4)
    {
        notify("HELP 4/8 - MAGNUS RUBY AND SESSIONS");
        notify(c + "ruby <code> - run one Ruby program. Example: " + c + "ruby x = 21; puts x * 2. Locals persist in the selected session.");
        notify(c + "local-input-ruby <code> - run Ruby with the latest nearby channel-0 message in local_input. Example: " + c + "local-input-ruby puts local_input.upcase. Chat text is passed as escaped data, not executable source.");
        notify(c + "ruby-local-input <code> is an alias for local-input-ruby. To persist captured text explicitly: " + c + "local-input-ruby var_set('last_chat', local_input).");
        notify("Ruby runs one request at a time. Wait for its reply before sending another ruby command or ruby-reset; this avoids simulator HTTP timeouts.");
        notify(c + "session <id> - use a shared Ruby/storage session (1-64 ASCII letters, digits, _ or -). Default is this object's UUID; choose the same ID as a Forth client to share stored data.");
        notify(c + "session-reset - select this object's UUID again. " + c + "ruby-reset - clear Ruby locals in the selected session, NOT its shared variables, files or matrices.");
        notify(c + "debug off - hide automatic => last value replies; explicit puts/print and Ruby errors remain visible. " + c + "debug on - show last values again (default).");
        notify("With debug off, a successful Ruby command without puts/print stays silent, including var-get and file-read; use debug on to inspect their values. The Ruby route requires a matching token.");
    }
    else if (page == 5)
    {
        notify("HELP 5/8 - SHARED VARIABLES (Ruby token required)");
        notify(c + "var-set <name> <text> - save a STRING; example: " + c + "var-set greeting Hello world");
        notify(c + "var-get <name> - read one value; " + c + "var-view - show stored values; " + c + "var-delete <name> - remove it (false if absent).");
        notify("For numbers, arrays or other JSON-compatible values, use Ruby: " + c + "ruby var_set('score', 21); puts var_get('score'). Missing reads raise Ruby errors.");
        notify("Successful var-set/var-delete operations persist in Rust's partitioned array snapshot before the server replies. Values survive server restarts and ruby-reset. Reuse the same session ID to read them; ordinary Ruby locals are only in server memory.");
    }
    else if (page == 6)
    {
        notify("HELP 6/8 - SANDBOXED FILES AND FORTH MATRICES");
        notify(c + "file-write <name> <text> - write a UTF-8 STRING; " + c + "file-read <name> - read it; " + c + "file-list - list names; " + c + "file-delete <name> - remove it.");
        notify("File names must be safe basenames (letters, digits, _, -, .; no '..', leading dot or path). Files are limited to 65536 bytes on the server; use a Ruby notecard for multiline content.");
        notify(c + "matrix-get <name> - read a matrix made by Forth in this session. Returns rows, cols and row-major values; Ruby can inspect it with " + c + "ruby matrix_get('A')['values'].inspect.");
    }
    else if (page == 7)
    {
        notify("HELP 7/8 - COMMAND NOTECARDS");
        notify("Create a notecard named Demo in this object's inventory and say " + c + "notecard Demo. Each line is one command, without /" + (string)CONTROL_CHANNEL + " (a leading /" + (string)CONTROL_CHANNEL + " is optional).");
        notify("Example Demo lines: # comment | team secondlife | ask What is a prim? | wait 2 | var-set greeting Hello | var-get greeting. Write each command on its own line.");
        notify("Lines starting # or // are comments. ask waits for its Ollama reply; wait <seconds> pauses; ruby-begin through ruby-end sends the enclosed lines as ONE multiline Ruby program.");
        notify(c + "notecard-stop - stop the active notecard. A throttled request is retried; editing/removing a running card or timing out while reading it stops the run.");
    }
    else if (page == 8)
    {
        notify("HELP 8/8 - RUBY NOTECARDS AND TROUBLESHOOTING");
        notify(c + "ruby-notecard RubyScript - send the ENTIRE notecard as Ruby. It must contain Ruby only: no team, ask, /7 ruby or ruby-begin lines. Script memory still limits large notecards.");
        notify(c + "local-input-ruby-notecard LocalInputRuby - send a Ruby-only card with local_input set to the latest nearby channel-0 message. Use this for Ruby that processes captured chat; the card must not contain local-input or local-input-ruby commands.");
        notify(c + "ruby-local-input-notecard <name> is an alias for local-input-ruby-notecard. " + c + "notecard-stop stops reading a card but does not cancel work already sent to the server.");
        notify("Use " + c + "notecard Demo for command cards, NOT ruby-notecard Demo. Use ruby-notecard only for Ruby-only cards. Example Ruby line: puts var_get('greeting').");
        notify("If Ruby says set RUBY_TOKEN, configure it in the main script; HTTP 401 means it does not match the server. HTTP 503 means the server Ruby route is disabled. Check " + c + "status and " + c + "health for setup.");
        notify("If a notecard cannot be found, check its exact case-sensitive inventory name. If local-input, local-input-ruby or ruby-begin reaches Ruby, the command card was run with ruby-notecard instead of notecard.");
    }
}

default
{
    link_message(integer sender_num, integer num, string message, key id)
    {
        if (num != HELP_LINK_MESSAGE || id != llGetOwner()) return;
        if (message == "0") help(0);
        else if (message == "9") command_list();
        else if (llStringLength(message) == 1 && llSubStringIndex("12345678", message) != -1)
            help((integer)message);
    }
}
