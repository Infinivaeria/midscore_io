// Legacy adapter for older split controllers only. Do not install beside the
// combined Tiade.Ollama.Ruby.lsl, which now owns this listener directly.
//
// It receives ordinary nearby chat (channel 0) from any avatar and sends it
// to the controller as a link message. The controller stores the latest value
// in gLastLocalChat. It is deliberately not interpreted as a command and is
// not sent to Ollama/Ruby automatically, so everyday local conversation stays
// local. The owner may explicitly process it with /7 local-input-ruby <code>
// or /7 local-input-ruby-notecard <name>.

integer LOCAL_CHAT_INPUT_MESSAGE = -708643;

integer gListenHandle;

start_listening()
{
    if (gListenHandle) llListenRemove(gListenHandle);
    // NULL_KEY receives messages from every nearby avatar.
    gListenHandle = llListen(0, "", NULL_KEY, "");
}

default
{
    state_entry()
    {
        start_listening();
    }

    on_rez(integer param) { llResetScript(); }

    changed(integer change)
    {
        if (change & CHANGED_OWNER) llResetScript();
    }

    listen(integer channel, string name, key id, string message)
    {
        if (channel != 0) return;
        llMessageLinked(LINK_THIS, LOCAL_CHAT_INPUT_MESSAGE, message, id);
    }
}
