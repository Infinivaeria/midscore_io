// Put this script beside the combined Tiade.Ollama.Ruby.lsl in the same prim.
// Help requests arrive by link message so the HTTP/notecard script does not
// need to hold the help text in its own limited script memory.

integer CONTROL_CHANNEL = 7;       // must match Tiade.Ollama.Ruby.lsl
integer HELP_LINK_MESSAGE = -708641;
integer gCollectingHelp;
string gHelpOutput;

flush_help()
{
    integer length = llStringLength(gHelpOutput);
    integer start = 0;
    while (start < length)
    {
        llOwnerSay(llGetSubString(gHelpOutput, start, start + 899));
        start += 900;
    }
    gHelpOutput = "";
}

notify(string text)
{
    string line = "[tiade] " + text + "\n";
    if (gCollectingHelp)
    {
        if (llStringLength(gHelpOutput) + llStringLength(line) > 900)
            flush_help();
        gHelpOutput += line;
        if (llStringLength(gHelpOutput) >= 900) flush_help();
    }
    else llOwnerSay("[tiade] " + text);
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
    notify("HELP: help or help all (complete command list and all eight detailed topics) | help <1-8> (one detailed topic). Command-card-only directives: wait <seconds>, ruby-begin, ruby-end; blank lines and # or // comments are skipped outside Ruby blocks.");
    notify("RUBY HELPERS: var_set(name, value), var_get(name), var_delete(name), var_view(), file_write(name, text), file_read(name), file_delete(name), file_list(), matrix_get(name). Storage uses the selected session; ruby-reset does not erase it. With debug off, use puts/print to see values.");
}

help_page(integer page)
{
    string c = "/" + (string)CONTROL_CHANNEL + " ";
    gHelpOutput = "";
    gCollectingHelp = TRUE;
    if (page == 0)
    {
        notify("START HERE: Type these in Nearby Chat as the bracelet owner: " + c + "status checks settings; " + c + "health checks the server; " + c + "ask Hello, what can you do? sends a test question.");
        notify("Replies are private to you by default. " + c + "public on makes Ollama/Ruby replies visible nearby; " + c + "public off makes them private again.");
        notify("To try Ruby (if enabled on the server): " + c + "ruby puts 'Hello from Ruby'. All eight detailed topics follow the command list below.");
        command_list();
        notify("HELP: " + c + "help and " + c + "help all show the complete reference. " + c + "help <1-8> shows just one topic. " + c + "status shows current settings.");
        notify("1 Setup | 2 Ollama | 3 Public replies | 4 Ruby | 5 Variables | 6 Files/matrices | 7 Command cards | 8 Ruby cards/troubleshooting");
    }
    else if (page == 1)
    {
        notify("HELP 1/8 - SETUP (only the object owner can use /" + (string)CONTROL_CHANNEL + ")");
        notify("If " + c + "help prints only QUICK HELP without this reference: open this prim's Contents, add Tiade.Ollama.Ruby.Help.lsl beside Tiade.Ollama.Ruby.lsl, and ensure both scripts are compiled and running. Then say " + c + "help again. Remove old Chat and LocalInput scripts.");
        notify("Set BASE_URL to your HTTPS server and RUBY_TOKEN to its TIADE_RUBY_EVAL_TOKEN in the main script. CONTROL_CHANNEL and HELP_LINK_MESSAGE must match in both scripts. Never post the token in chat.");
        notify("The combined main script owns both listeners. It captures channel-0 chat from nearby speakers and stores only the latest message in gLastLocalChat. It does not send chat to the server or run it as a command.");
        notify(c + "health checks server/Ollama; " + c + "status shows server, team, session, Ruby enabled, replies, notecard progress and a preview of the latest local input. " + c + "local-input shows the complete stored local-input value to the owner. Touch the object for status and the complete help reference.");
        notify("Ruby executes on your server with its privileges. Only use a trusted server and keep the token private; changing a script requires recompiling it in-world.");
    }
    else if (page == 2)
    {
        notify("HELP 2/8 - OLLAMA QUESTIONS AND SETTINGS");
        notify(c + "ask <message> - send a question; example: " + c + "ask What is a prim? Plain unrecognized /" + (string)CONTROL_CHANNEL + " text is also sent as a question.");
        notify(c + "team <name> - choose a team and its shared Ollama history (default: secondlife). Team does not change the Ruby session.");
        notify(c + "url <https://host> - switch server for this script, without a trailing slash; " + c + "health checks connectivity.");
        notify(c + "health - report the server's configured Ollama model and installed models, or an HTTP failure. This does not test the Ruby token. " + c + "status - show client settings, pending replies, notecard progress, local-input preview and free script memory without contacting the server.");
        notify("Ollama replies are queued; one pending job is polled every 2s, rotating between jobs. At most 4 questions may be pending. A reply is abandoned after 240s. These limits reflect the main script's default settings.");
    }
    else if (page == 3)
    {
        notify("HELP 3/8 - PUBLIC REPLIES");
        notify(c + "public on - say Ollama answers and Ruby output in nearby public chat (channel 0). Anyone nearby may see them; do not use for private data.");
        notify(c + "public off - send Ollama answers and Ruby output to the owner only. Errors from Ruby evaluation follow the same reply setting.");
        notify("Status, help, settings confirmations, HTTP diagnostics and Ruby reset messages remain owner-only. The public setting resets to off when the main script resets, recompiles or re-rezzes.");
        notify("Use " + c + "status to check the current reply setting. Nearby channel-0 chat is stored only in gLastLocalChat by the main script. " + c + "local-input returns the full value to the owner. It is cleared when the main script resets and is never relayed automatically.");
        notify("LOCAL-INPUT TEST: Say Hello nearby on channel 0, then run " + c + "local-input to inspect it privately, or " + c + "local-input-ruby puts local_input.upcase to print HELLO. Ruby output follows public on|off; keep public off for private text.");
        notify("Every newly heard channel-0 message replaces the previous value; this is not a chat history. Nearby speakers can change it even though only the owner can run commands. Let people know their chat is being captured.");
        notify("If no input is captured, confirm the main script is running, have someone speak on channel 0, then retry. No extra LocalInput script or help notecard is required.");
    }
    else if (page == 4)
    {
        notify("HELP 4/8 - MAGNUS RUBY AND SESSIONS");
        notify(c + "ruby <code> - run one Ruby program. Example: " + c + "ruby x = 21; puts x * 2. Locals persist in the selected session.");
        notify(c + "local-input-ruby <code> - run Ruby with the latest nearby channel-0 message in local_input. Example: " + c + "local-input-ruby puts local_input.upcase. Chat text is passed as escaped data, not executable source.");
        notify(c + "ruby-local-input <code> is an alias for local-input-ruby. To persist captured text explicitly: " + c + "local-input-ruby var_set('last_chat', local_input).");
        notify("This client allows one Ruby evaluation/reset request at a time. Wait for its reply before sending another Ruby command or ruby-reset. Long-running server code can still exceed the simulator's HTTP timeout.");
        notify(c + "session <id> - use a shared Ruby/storage session (1-64 ASCII letters, digits, _ or -). Default is this object's UUID; choose the same ID as a Forth client to share stored data.");
        notify(c + "session-reset - select this object's UUID again. " + c + "ruby-reset - clear Ruby locals in the selected session, NOT its shared variables, files or matrices.");
        notify("Resetting/recompiling/re-rezzing the main script selects this object's UUID again and restores configured server/team/debug/public settings. It does not request a server-side ruby-reset or erase storage. If saved data seems missing, select its original session ID again; a different object has a different default session.");
        notify(c + "debug off - hide automatic => last value replies; explicit puts/print and Ruby errors remain visible. " + c + "debug on - show last values again (default).");
        notify("With debug off, a successful Ruby command without puts/print stays silent, including var-get and file-read; use debug on to inspect their values. The Ruby route requires a matching token.");
    }
    else if (page == 5)
    {
        notify("HELP 5/8 - SHARED VARIABLES (Ruby token required)");
        notify(c + "var-set <name> <text> - save a STRING; example: " + c + "var-set greeting Hello world");
        notify(c + "var-get <name> - read one value; " + c + "var-view - show stored values; " + c + "var-delete <name> - remove it (false if absent).");
        notify("For numbers, arrays or other JSON-compatible values, use Ruby: " + c + "ruby var_set('score', 21); puts var_get('score'). Missing reads raise Ruby errors.");
        notify("Variable names in chat commands cannot contain spaces. var-set requires nonempty text and trims surrounding whitespace. For empty text use " + c + "ruby var_set('greeting', ''); to preserve spaces use " + c + "ruby var_set('greeting', ' Hello '). To inspect all values with debug off: " + c + "ruby puts var_view().inspect.");
        notify("Successful var-set/var-delete operations persist in Rust's partitioned array snapshot before the server replies. Values survive server restarts and ruby-reset. Reuse the same session ID to read them; ordinary Ruby locals are only in server memory.");
    }
    else if (page == 6)
    {
        notify("HELP 6/8 - SANDBOXED FILES AND FORTH MATRICES");
        notify(c + "file-write <name> <text> - write a UTF-8 STRING; " + c + "file-read <name> - read it; " + c + "file-list - list names; " + c + "file-delete <name> - remove it.");
        notify("File names must be safe basenames (letters, digits, _, -, .; no '..', leading dot or path). Files are limited to 65536 bytes on the server; use a Ruby notecard for multiline content.");
        notify("FILE EXAMPLE (run each line separately and wait for its reply):\n" + c + "file-write notes.txt Hello world\n" + c + "file-read notes.txt");
        notify("With debug off, use " + c + "ruby puts file_read('notes.txt') or " + c + "ruby puts file_list().inspect. Use " + c + "ruby file_write('notes.txt', '') to write empty content.");
        notify(c + "matrix-get <name> - read a matrix made by Forth in this session. Returns rows, cols and row-major values; Ruby can inspect it with " + c + "ruby matrix_get('A')['values'].inspect.");
    }
    else if (page == 7)
    {
        notify("HELP 7/8 - COMMAND NOTECARDS");
        notify("Create a notecard named Demo in this object's inventory and say " + c + "notecard Demo. Each line is one command, without /" + (string)CONTROL_CHANNEL + " (a leading /" + (string)CONTROL_CHANNEL + " is optional).");
        notify("Example Demo contents (one command per line):\n# comment\nteam secondlife\nask What is a prim?\nwait 2\nvar-set greeting Hello\nvar-get greeting");
        notify("Lines starting # or // are comments. ask waits for its Ollama reply; wait <seconds> pauses; ruby-begin through ruby-end sends the enclosed lines as ONE multiline Ruby program.");
        notify("RUBY BLOCK EXAMPLE (command-card contents):\nruby-begin\ntotal = 21 * 2\nputs total\nruby-end");
        notify("Ruby indentation and comments inside the block are preserved; always close it with ruby-end. wait takes an integer number of seconds and is a command-card directive, not a chat command. Commands are separate lines; semicolons are only for statements within Ruby code, not for separating controller commands.");
        notify(c + "notecard-stop - stop the active notecard. A throttled request is retried; editing/removing a running card or timing out while reading it stops the run.");
    }
    else if (page == 8)
    {
        notify("HELP 8/8 - RUBY NOTECARDS AND TROUBLESHOOTING");
        notify(c + "ruby-notecard RubyScript - send the ENTIRE notecard as Ruby. It must contain Ruby only: no team, ask, " + c + "ruby or ruby-begin lines. Script memory still limits large notecards.");
        notify(c + "local-input-ruby-notecard LocalInputRuby - send a Ruby-only card with local_input set to the latest nearby channel-0 message. Use this for Ruby that processes captured chat; the card must not contain local-input or local-input-ruby commands.");
        notify("Local-input Ruby cards use the latest captured message when the completed card is sent to Ruby, not when reading starts. New chat during reading can replace that message. Ruby-only card example:\nputs local_input\nputs local_input.upcase");
        notify(c + "ruby-local-input-notecard <name> is an alias for local-input-ruby-notecard. " + c + "notecard-stop stops reading a card but does not cancel work already sent to the server.");
        notify("Use " + c + "notecard Demo for command cards, NOT ruby-notecard Demo. Use ruby-notecard only for Ruby-only cards. Example Ruby line: puts var_get('greeting').");
        notify("If Ruby says set RUBY_TOKEN, configure it in the main script; HTTP 401 means it does not match the server. HTTP 503 means the server Ruby route is disabled. Check " + c + "status and " + c + "health for setup.");
        notify("HTTP 422 is a Ruby evaluation error: read the error and check the code. HTTP 499 means the simulator timed out; the server may still be executing it. Wait before retrying, especially writes or other side effects. Stopping a card or resetting the client does not cancel a request already sent.");
        notify("If HTTP is throttled or too many requests are outstanding, wait for replies before retrying. Command cards retry throttled requests automatically. An expired or abandoned Ollama reply requires a new ask; it does not mean the server never processed the original question.");
        notify("If a notecard cannot be found, check its exact case-sensitive inventory name. If local-input, local-input-ruby or ruby-begin reaches Ruby, the command card was run with ruby-notecard instead of notecard.");
        notify("If another card is already running, use " + c + "notecard-stop before starting a new one. For script-memory errors, check free memory with " + c + "status and use a smaller Ruby program/card; resetting the main script clears captured chat and restores its configured settings.");
    }
    gCollectingHelp = FALSE;
    flush_help();
}

help(integer page)
{
    if (page == 0 || page == 9)
    {
        help_page(0);
        integer topic;
        for (topic = 1; topic <= 8; ++topic) help_page(topic);
    }
    else help_page(page);
    notify("END OF HELP. Use /" + (string)CONTROL_CHANNEL + " help <1-8> to revisit one topic.");
}

default
{
    state_entry()
    {
        notify("Help ready. Type /" + (string)CONTROL_CHANNEL + " help in Nearby Chat.");
    }

    link_message(integer sender_num, integer num, string message, key id)
    {
        if (num != HELP_LINK_MESSAGE || id != llGetOwner()) return;
        if (message == "0") help(0);
        else if (message == "9") help(9);
        else if (llStringLength(message) == 1 && llSubStringIndex("12345678", message) != -1)
            help((integer)message);
    }
}
