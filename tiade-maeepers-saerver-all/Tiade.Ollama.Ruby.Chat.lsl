// Legacy adapter for older split controllers only. Do not install beside the
// combined Tiade.Ollama.Ruby.lsl, which now owns this listener directly.
// It owns the control-channel listener and forwards owner commands to the main controller.

integer CONTROL_CHANNEL = 7;       // must match Tiade.Ollama.Ruby.lsl
integer CHAT_COMMAND_MESSAGE = -708642;

key gOwner;

default
{
    state_entry()
    {
        gOwner = llGetOwner();
        llListen(CONTROL_CHANNEL, "", gOwner, "");
    }

    on_rez(integer param) { llResetScript(); }

    changed(integer change)
    {
        if (change & CHANGED_OWNER) llResetScript();
    }

    listen(integer channel, string name, key id, string message)
    {
        if (id != gOwner) return;
        if (channel == CONTROL_CHANNEL)
            llMessageLinked(LINK_THIS, CHAT_COMMAND_MESSAGE, message, id);
    }
}
