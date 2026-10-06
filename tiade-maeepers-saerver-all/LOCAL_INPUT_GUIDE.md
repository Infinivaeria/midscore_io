# Local Input Guide

The combined `Tiade.Ollama.Ruby.lsl` captures ordinary nearby Second Life chat
on channel `0`. The main controller keeps only the newest message in its
`gLastLocalChat` variable. Capturing never executes chat as a command and
never sends it to Ollama or Ruby automatically.

## Install

Put only these two scripts in the same prim and save or reset them:

- `Tiade.Ollama.Ruby.lsl`
- `Tiade.Ollama.Ruby.Help.lsl`

Remove the old Chat and LocalInput scripts; both listeners are built into the
main script now. `/7 help` or `/7 help all` lists every command and alias.

Nearby avatars may speak normally on channel `0`. Only the object owner can
run controller commands on `/7` or send captured input to Ruby.

## Inspecting input

Have someone nearby speak, then use either command as the object owner:

```text
/7 status
/7 local-input
```

`status` includes a short preview. `local-input` returns the complete current
value privately to the owner, even when public replies are enabled. If no
message has been heard since the main script reset, it reports that no input is
available.

The value is volatile: every newly heard message replaces the old value, and
the value is cleared when the main script resets, recompiles, changes owner, or
is re-rezzed.

## Inline Ruby

Use `/7 local-input-ruby <Ruby code>` to bind the latest captured message to
the Ruby variable `local_input` and run code against it.

```text
/7 local-input-ruby puts local_input
/7 local-input-ruby puts local_input.strip.upcase
/7 local-input-ruby puts local_input.split.length
/7 local-input-ruby puts local_input.reverse
```

The controller safely escapes the captured text and passes it as Ruby data; a
chat message is not inserted into source as executable Ruby code. The Ruby
session's normal `RUBY_TOKEN`, one-request-at-a-time, and public-replies rules
still apply.

## Ruby-only notecard

Use this for a multi-line Ruby program that needs `local_input`.

1. Create an object notecard named `LocalInputRuby`.
2. Paste the content of `sl_notecards/LocalInputRuby.txt`.
3. Run:

   ```text
   /7 local-input-ruby-notecard LocalInputRuby
   ```

Before the card runs, the controller assigns the captured message to
`local_input`. The card must contain Ruby source only. Do not include `/7`,
`local-input`, `local-input-ruby`, `ruby-begin`, or `ruby-end` in it.

### Input/output example

`sl_notecards/LocalInputOutput.txt` is a Ruby-only example that prints:

- `INPUT:` followed by the captured message;
- `OUTPUT (trimmed uppercase):` followed by its transformed value.

Upload that source as a notecard named `LocalInputOutput`, then run:

```text
/7 local-input-ruby-notecard LocalInputOutput
```

## Command notecard

Use this mode when a card should issue Tiade controller commands rather than
contain Ruby source. `sl_notecards/LocalInputCommands.txt` displays the current
input and then runs an inline Ruby example.

1. Upload it as a notecard named `LocalInputCommands`.
2. Run:

   ```text
   /7 notecard LocalInputCommands
   ```

Never run this card with `/7 ruby-notecard LocalInputCommands`. Ruby will see
`local-input` and `local-input-ruby` as invalid Ruby syntax.

## Choosing a variation

| Need | Use |
| --- | --- |
| See the latest value only | `/7 local-input` |
| One small transformation | `/7 local-input-ruby <Ruby code>` |
| Multi-line Ruby processing | `/7 local-input-ruby-notecard <name>` |
| Demonstrate input and output | `LocalInputOutput` with `local-input-ruby-notecard` |
| Run controller commands from a card | `/7 notecard LocalInputCommands` |

## Privacy and output

- Local chat capture covers nearby channel-0 speakers, not just the owner.
- The object retains one message only, in memory; it is not a persistent chat
  history or server log.
- Ruby receives local input only after the owner explicitly invokes one of the
  Ruby-local-input commands.
- Ruby output follows the controller's public-replies setting. Keep it off for
  private messages, or use `/7 local-input` for owner-only inspection.
- Tell nearby people that their chat is being captured before using this
  feature for an interactive experience.

## Troubleshooting

**No local input captured** — Confirm the combined `Tiade.Ollama.Ruby.lsl` is
compiled and running, reset it, and have someone speak on channel `0`.

**“No local channel-0 input”** — Speak nearby after the main script starts,
then retry. A reset clears the previous value.

**Ruby syntax error mentioning `local-input`** — A command notecard was run as
a Ruby notecard. Run it with `/7 notecard <name>`, or use a Ruby-only card with
`/7 local-input-ruby-notecard <name>`.

**“Not enough script memory to read notecard”** — Recompile/reset the current
main script, use `/7 status` to check free memory, and prefer a small Ruby-only
card. The controller reserves 2 KB while buffering a card and verifies memory
again before sending Ruby.

**Ruby token error** — Set `RUBY_TOKEN` in `Tiade.Ollama.Ruby.lsl` to the
server's `TIADE_RUBY_EVAL_TOKEN`, then save/reset the script.
