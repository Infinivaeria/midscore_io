# Second Life RustForth Service Guide

`Relote.Stack.Service.lsl` is an owner-controlled Second Life client for the
Tide/RustForth server. It accepts chat commands, sends HTTPS requests to the
server, and can perform allowlisted server-generated actions in the object that
contains the script.

This guide covers chat commands, Forth, RubyForth, persisted notecards,
matrices, files, and the optional browser-to-Second-Life bridge.

For a detailed explanation of the LSL implementation, its event handlers,
asynchronous requests, and polling behavior, see
[LSL_CLIENT_REFERENCE.md](LSL_CLIENT_REFERENCE.md).

For the complete HTTP/Tide route reference, see
[TIDE_ROUTE_MANUAL.md](TIDE_ROUTE_MANUAL.md).

Before using real objects or bridge jobs, run the copyable isolation and bridge
checks in the `Battle-Test Checklist` section of the route manual.

## Install and Configure

1. Create or select a Second Life object that you own.
2. Add `Relote.Stack.Service.lsl` to the object's inventory.
3. Edit these values at the top of the script:

	```lsl
	string SERVERURL = "https://your-server.example";
	string BRIDGE_TOKEN = "";
	integer CHANNEL_INPUT = 1111;
	integer CHANNEL_OUTPUT = 0;
	```

4. Save or reset the script. The owner receives a startup message.
5. Type commands in nearby chat, prefixed with the input channel. With the
	default configuration, every command begins with `/1111`.

Only the object's owner can use this script. Commands from other avatars are
ignored.

## First Successful Request

Set the output channel to public chat, then run a Forth expression:

```text
/1111 channel 0
/1111 forth 2 3 + .
```

The first command selects where ordinary results are sent. The second sends
Forth source to `POST /forth/eval`; `.` removes the top value from the Forth
stack and includes it in the response output. You should see `5` in public
chat.

Use a nonzero channel when results should not be public:

```text
/1111 channel 42
/1111 forth 6 7 * .
/1111 forth-say 42 10 20 + .
```

## Chat Command Reference

All commands below assume the default input channel `1111`.

| Command | Purpose | Example |
| --- | --- | --- |
| `channel <number>` | Set the normal output channel. | `/1111 channel 0` |
| `help` | Send owner-only syntax help. | `/1111 help` |
| `session <name>` | Select a named language session. | `/1111 session workshop_1` |
| `session-reset` | Return to this prim's UUID session. | `/1111 session-reset` |
| `forth <source>` | Run a one-line Forth program. | `/1111 forth 9 4 - .` |
| `forth-say <channel> <source>` | Run Forth on a chosen channel. | `/1111 forth-say 42 2 8 * .` |
| `ruby <source>` | Run one RubyForth statement or expression. | `/1111 ruby puts 2 + 3` |
| `ruby-say <channel> <source>` | Run RubyForth on a chosen channel. | `/1111 ruby-say 42 puts "hello"` |
| `algebra <expression>` | Analyze an integer polynomial. | `/1111 algebra 3*x^2 - 2*x + 7` |
| `algebra-say <channel> <expression>` | Send algebra output to a chosen channel. | `/1111 algebra-say 42 x^2 + 2*x + 1` |
| `set <name> <value>` | Store a server variable. | `/1111 set greeting hello` |
| `get <name>` | Read a server variable. | `/1111 get greeting` |
| `view` | List stored variables. | `/1111 view` |
| `delete <name>` | Delete one stored variable. | `/1111 delete greeting` |
| `clear` | Clear the server variable store. | `/1111 clear` |
| `status` | Request variable-store status. | `/1111 status` |
| `history` | Request variable-store history. | `/1111 history` |
| `file-write <name> <text>` | Save text in the sandboxed file store. | `/1111 file-write note.txt hello` |
| `file-read <name>` | Read a sandboxed file. | `/1111 file-read note.txt` |
| `file-read-say <channel> <name>` | Read a file on a chosen channel. | `/1111 file-read-say 42 note.txt` |
| `files` | List sandboxed files. | `/1111 files` |
| `file-delete <name>` | Delete a sandboxed file. | `/1111 file-delete note.txt` |
| `notecard <inventory-name>` | Upload an inventory notecard. | `/1111 notecard Counter` |
| `notecards` | List uploaded notecards. | `/1111 notecards` |
| `notecard-get <name>` | Fetch an uploaded notecard. | `/1111 notecard-get Counter` |
| `notecard-delete <name>` | Delete an uploaded notecard. | `/1111 notecard-delete Counter` |
| `forth-run <name>` | Run an uploaded Forth notecard. | `/1111 forth-run Counter` |
| `forth-run-say <channel> <name>` | Run a Forth notecard on a chosen channel. | `/1111 forth-run-say 42 Counter` |
| `ruby-run <name>` | Compile and run RubyForth. | `/1111 ruby-run RubyCounter` |
| `ruby-run-say <channel> <name>` | Run RubyForth on a chosen channel. | `/1111 ruby-run-say 42 RubyCounter` |
| `matrix-get <name>` | Read a persisted matrix. | `/1111 matrix-get A` |
| `matrix-get-say <channel> <name>` | Read a matrix on a chosen channel. | `/1111 matrix-get-say 42 A` |
| `bridge-on` | Start polling for browser-queued bridge jobs. | `/1111 bridge-on` |
| `bridge-off` | Stop bridge polling. | `/1111 bridge-off` |
| `bridge-poll` | Trigger one bridge poll immediately. | `/1111 bridge-poll` |

The command parser splits on spaces. Use chat commands for short, single-line
source and use notecards for multiline Forth or RubyForth.

## Server Variables

The `set`, `get`, `view`, `delete`, `clear`, `status`, and `history` commands
use the server variable store. Values are server-persistent; the `kv` list in
the LSL script is only a local response cache.

```text
/1111 set score 125
/1111 get score
/1111 view
/1111 delete score
```

Avoid backslashes and these characters in names or values: `:`, `[`, `]`, `{`,
`}`, single quotes, double quotes, and periods. The current LSL escape/unescape
implementation does not round-trip those characters correctly. Plain names such
as `score`, `player_name`, and simple text values are safe.

## Forth Basics

Forth is stack-based. Numbers and strings are pushed onto the stack. Most words
pop their inputs from the stack and push a result.

```forth
2 3 + .
```

This pushes `2`, pushes `3`, adds them, then prints `5`.

| Group | Words |
| --- | --- |
| Arithmetic | `+`, `-`, `*`, `/`, `mod`, `abs`, `min`, `max` |
| Comparisons | `=`, `!=`, `<`, `<=`, `>`, `>=`, `0=` |
| Boolean | `true`, `false`, `nil`, `not`, `and`, `or` |
| Stack | `dup`, `drop`, `swap`, `over` |
| Output | `.`, `puts`, `p` |
| Time/randomness | `now`, `rand` |
| Control | `if ... else ... then`, `begin ... until`, `begin ... again`, `bye` |
| Variables | `let`, `variable`, `@`, `!`, `$name`, `name=`, `name+=`, `name-=` |
| Named words | `: name ... ;` |

Numbers are signed integers. `/` is integer division. Strings use double
quotes; a backslash begins a Forth comment outside a string.

### Variables

Variables persist in the same server store used by Forth, RubyForth, matrices,
and notecards.

```forth
let total
25 total !
$total 5 + total!
$total puts
```

`total` pushes a variable address for `!` and `@`. `$total` pushes its current
value. The concise forms `total!`, `total+=`, and `total-=` assign, add, and
subtract respectively.

### Named Forth Words

You can give a sequence of Forth tokens a name and use that name later in the
same program or notecard. This is the main new tool for breaking a larger
program into reusable pieces.

```forth
: square dup * ;
: announce-square square puts ;

6 announce-square
```

Read the first definition as: “define the word `square`; when it is used, copy
the top stack value with `dup`, multiply the copy and original with `*`, then
finish the definition at `;`.” The second definition calls the first, then
prints its result. The final line pushes `6` and calls `announce-square`, which
prints `36`.

Definitions are collected before execution, so a word may call another word
defined later in the same source. They are expanded before the interpreter runs,
which keeps existing loops and safety limits unchanged. Recursive definitions,
such as `: forever forever ;`, are rejected. Do not reuse the name of a built-in
word such as `if`, `puts`, or `matrix`.

### Semicolon-Separated Forth

Outside a `: name ... ;` definition, a semicolon is a statement separator. This
is useful in one-line chat commands:

```text
/1111 forth 2 3 + puts; : square dup * ; 4 square puts;
```

The output is `5 16`. The semicolon after `square` still closes the definition;
the other semicolons simply separate sequential blocks.

For a short definition in chat:

```text
/1111 forth : square dup * ; 9 square puts
```

Use a notecard for multi-line definitions.

### Explicit Execution Budgets

The server defaults to 10,000 interpreter steps and accepts at most 1,000,000.
Long loops now have direct LSL commands that set this budget explicitly:

```text
/1111 forth-steps 100000 : square dup * ; 250 square puts
/1111 ruby-steps 100000 puts 2 + 3
/1111 forth-run-steps 100000 Counter
/1111 ruby-run-steps 100000 RubyCounter
```

The second argument to each `*-steps` command must be an integer from `1` to
`1000000`. A larger budget permits more computation, but does not remove the
server's resource limit.

### Forth and RubyForth in One Program

The `ruby` Forth directive compiles its quoted RubyForth source into the
surrounding Forth program. The compiled code uses the same variables, named
words, stack, output response, and allowed Second Life calls as the containing
Forth source.

```forth
: square dup * ;
6 square puts
ruby "count = 4\nputs count * 3"
```

This prints `36` from Forth, then `12` from RubyForth. The `\n` is a newline
inside the quoted RubyForth source. In a notecard, a multiline Ruby snippet is
also allowed inside the quoted string.

RubyForth can send a quoted Forth source string back through the same expander:

```ruby
forth ": double dup + ; 21 double puts"
puts "RubyForth continues after Forth"
```

The first line defines and calls a Forth word, printing `42`; the second line
is ordinary RubyForth and prints its text afterward. A `forth` bridge statement
must contain exactly one quoted Forth source string.

For nearby chat, use the new Forth-originated alias:

```text
/1111 hybrid : square dup * ; 6 square puts ruby "puts 2 + 3"
/1111 hybrid-say 42 ruby "puts 6 * 7"
```

`hybrid` uses `/forth/eval`, so it can start with Forth and include one or more
`ruby "..."` directives. Regular `/1111 forth` accepts the same bridge syntax;
`hybrid` only makes that intent explicit in chat.

## Forth Notecard Examples

Create a notecard with the exact inventory name shown, paste its contents,
place it in the same object as the script, and upload it with:

```text
/1111 notecard <NotecardName>
```

Wait for the object to report that it is loading the notecard, then run it with
`/1111 forth-run <NotecardName>`.

### What a notecard is and what happens to it

A notecard is an inventory item inside the Second Life object. It is not a file
on your computer, and it is not automatically a program. The script reads it
one line at a time, sends its text to the server, and stores a server-side copy
under the same name.

Use this exact sequence for every example:

1. Create a new notecard in the object's inventory and name it exactly as the
	example heading says. For example, name the first one `Counter`.
2. Open the notecard and paste only the code inside its matching code block.
	Do not paste the Markdown backticks, the heading, or explanatory prose.
3. Save the notecard and close it.
4. In nearby chat, as the object owner, enter `/1111 notecard Counter`.
5. The LSL script asks Second Life for each line. When it reaches the end, it
	uploads the complete text to the server as the remote program named
	`Counter`.
6. Wait for the loading message to appear. Large notecards can take longer than
	one chat message because Second Life returns their lines asynchronously.
7. Run the uploaded copy with `/1111 forth-run Counter`.
8. Read the result on the configured output channel. Set that channel first
	with `/1111 channel 0` if you want public-chat output.

The inventory name and the command argument are case-sensitive in practice:
use the same spelling every time. Editing the local notecard does not update the
server copy. Run `/1111 notecard Counter` again after every edit, then rerun
`/1111 forth-run Counter`.

`forth-run` executes source as Forth. `ruby-run` executes the same uploaded
source as RubyForth. Do not run a Forth example with `ruby-run`, and do not run
a RubyForth example with `forth-run`.

### `Counter`

```forth
let counter
3 counter !

begin
	$counter puts
	1 counter-=
	$counter 0=
until
```

Expected output: `3`, `2`, `1`.

### `Condition`

```forth
let temperature
23 temperature !

$temperature 20 > if
	"warm" puts
else
	"cool" puts
then
```

### `StackTools`

```forth
10 20 over . . .
```

`over` copies the second stack item. This example outputs `10`, `20`, then
`10`.

### `RandomNumber`

```forth
100 rand puts
```

Outputs an integer from `0` through `99`.

### `Greeting`

```forth
"Hello, Second Life" puts
"Hello, " "RustForth" + puts
```

### `MatrixDeterminant`

```forth
2 2 matrix A
1 0 0 mset A
2 0 1 mset A
3 1 0 mset A
4 1 1 mset A

m.show A
m.det A puts
```

This creates a two-by-two matrix and prints its serialized form followed by
determinant `-2`.

### `MatrixScale`

```forth
2 m.identity I
5 m.scale I FiveI
m.show FiveI
```

### `MatrixSolve`

```forth
2 2 matrix A
2 0 0 mset A
0 0 1 mset A
2 1 0 mset A
1 1 1 mset A

2 1 matrix B
4 0 0 mset B
6 1 0 mset B

m.solve A B X
m.show X
```

This solves $A X = B$ when the solution contains exact integers.

## Matrix Reference

Matrices persist server-side. The maximum dimension is `64` and the maximum
number of cells is `4096`.

| Word | Stack/source form | Result |
| --- | --- | --- |
| `matrix` | `<rows> <cols> matrix <name>` | Create a zero matrix. |
| `m.identity` | `<size> m.identity <name>` | Create an identity matrix. |
| `mset` | `<value> <row> <col> mset <name>` | Set one cell. |
| `mget` | `<row> <col> mget <name>` | Push one cell. |
| `m.fill` | `<value> m.fill <name>` | Fill every cell. |
| `m.scale` | `<scalar> m.scale <source> <target>` | Scale a matrix. |
| `m.add` | `m.add <left> <right> <target>` | Add matrices. |
| `m.sub` | `m.sub <left> <right> <target>` | Subtract matrices. |
| `m.mul` | `m.mul <left> <right> <target>` | Multiply matrices. |
| `m.solve` | `m.solve <left> <right> <target>` | Solve $A X = B$ with exact integers. |
| `m.transpose` | `m.transpose <source> <target>` | Transpose a matrix. |
| `m.det` | `m.det <name>` | Push determinant. |
| `m.rows` | `m.rows <name>` | Push row count. |
| `m.cols` | `m.cols <name>` | Push column count. |
| `m.show` | `m.show <name>` | Emit serialized matrix output. |

Use `/1111 matrix-get <name>` to retrieve a stored matrix outside a Forth
program.

## Forth-Controlled Second Life Actions

The Forth runtime does not directly run arbitrary LSL. Instead, it returns an
allowlisted `calls` list, and this script performs each call in the object.
These actions work with `forth`, `forth-run`, and Forth bridge jobs. At most 128
host calls are allowed in one Forth program.

The action is performed when this LSL client receives the evaluation response,
so a direct command such as `/1111 forth ...` can cause the action immediately.

### Speak in chat

```forth
0 "Hello from the object" sl.say
0 "A quiet hello" sl.whisper
0 "Attention" sl.shout
42 "Private channel message" sl.region_say
"Only the owner sees this" sl.owner_say
```

The stack order for the first four forms is `<channel> <message>`.

### Change floating text, color, and alpha

```forth
"Ready" 0 255 0 255 sl.set_text
255 0 0 -1 sl.set_color
128 -1 sl.set_alpha
```

Color and alpha components accept either `0` through `1` or `0` through `255`.
`-1` means all faces for `sl.set_color` and `sl.set_alpha`.

### Play a sound

Replace the UUID with a sound asset UUID in the object's inventory or a sound
that the object is permitted to play:

```forth
"01234567-89ab-cdef-0123-456789abcdef" 1 sl.play_sound
```

The stack order is `<sound> <volume>`.

### Object timer, position, and linked messages

```forth
5 sl.set_timer
128 128 25 sl.set_region_pos
```

`sl.set_timer` invokes `llSetTimerEvent`; it does not add custom timer handling
to this script. The existing `timer` event is used for bridge polling, so do not
use `sl.set_timer` while bridge polling is enabled unless changing the poll
interval is intentional.

`sl.set_region_pos` requires the usual Second Life permissions and parcel
conditions. `sl.link_message` has stack order `<link> <code> <message> <id>`;
`-1` is `LINK_SET`.

### `ObjectWelcome`

```forth
"RustForth online" 0 200 100 255 sl.set_text
0 "Object is ready" sl.say
"Object is ready" sl.owner_say
```

### `PulseColor`

```forth
0 128 255 -1 sl.set_color
255 -1 sl.set_alpha
0 "Color changed" sl.whisper
```

## RubyForth

RubyForth is a small Ruby-shaped language compiled into the Forth runtime. It
shares persistent variables, arrays, hashes, matrices, notecards, and execution
budgets with Forth, but it is not MRI Ruby. It has no gems, `require`, `eval`,
classes, arbitrary host methods, arbitrary file access, or system commands.

Supported constructs are integer and string expressions, `#{name}`
interpolation, assignment, `+=`, `-=`, `*=`, `/=`, `puts`, `p`, `if`,
`unless`, `elsif`, `else`, `while`, `until`, `n.times do`, `end`, `return`,
one-line arrays/hashes, and collection helpers such as `get`, `set`, `push`,
`delete`, `keys`, `length`, and `has`.

Semicolons separate RubyForth statements outside strings, arrays, hashes, and
parentheses. This makes multi-step direct chat commands possible:

```text
/1111 ruby count = 1; puts count; count += 1; puts count
```

Use a notecard for multiline blocks, not merely because of semicolons.

```text
/1111 ruby puts 2 + 3
/1111 ruby puts "Hello from RubyForth"
/1111 ruby-say 42 puts 6 * 7
```

RubyForth currently cannot call `sl.say`, `sl.set_text`, or any other `sl.*`
operation. Use a Forth notecard when the program must change the in-world
object.

### Mixing RubyForth and Forth

Use a quoted one-line bridge when the Forth fragment is short:

```ruby
count = 2
forth "$count 3 + count="
puts count
```

For a larger Forth fragment, use a multiline `forth do ... end` block:

```ruby
count = 2; puts count

forth do
	$count 3 + count=
	$count puts
end

puts count
```

This emits `2 5 5`: RubyForth initializes and prints `count`, Forth adds three
and prints it, and RubyForth reads the same persisted value afterward. Forth
inside this block can use named words, matrices, arrays, hashes, or allowed
`sl.*` calls. RubyForth resumes after the block using the same execution budget.

Forth can call a short RubyForth fragment in the other direction:

```forth
ruby "count = 4; puts count * 3"
```

### Per-Prim Sessions

At startup, the LSL client sets its language session to the current prim UUID.
That means Forth/RubyForth state from two objects using the same server can be
isolated. Change the active scope only when you intentionally want to share
state:

```text
/1111 session workshop_1
/1111 ruby score = 10; puts score
/1111 session-reset
```

Session names must use letters, numbers, `_`, or `-`, and be at most 64
characters. Existing API callers that omit `session_id` keep using the legacy
`default` session.

## RubyForth Notecard Examples

Upload each notecard with `/1111 notecard <NotecardName>`, then run it with
`/1111 ruby-run <NotecardName>`.

### `RubyCounter`

```ruby
count = 0
while count < 3
	puts count
	count += 1
end
```

Expected output: `0`, `1`, `2`.

### `RubyTimes`

```ruby
5.times do
	puts "tick"
end
```

### `RubyCondition`

```ruby
score = 80
if score >= 90
	puts "A"
elsif score >= 80
	puts "B"
else
	puts "Needs work"
end
```

### `RubyUnless`

```ruby
enabled = false
unless enabled
	puts "Feature is disabled"
end
```

### `RubyArithmetic`

```ruby
total = 4
total *= 3
total += 2
puts total
```

Expected output: `14`.

### `RubyStrings`

```ruby
first = "Second"
second = "Life"
puts first + " " + second
```

### `RubyUntil`

```ruby
remaining = 3
until remaining == 0
	puts remaining
	remaining -= 1
end
```

## Files and Notecard Management

The server has two separate persistence mechanisms:

- **Notecards** store source programs in the partitioned server store. Upload
	inventory notecards with `notecard`; run them with `forth-run` or `ruby-run`.
- **Files** are UTF-8 text documents in a sandboxed file store. Manage them
	with `file-write`, `file-read`, `files`, and `file-delete`. They are not
	executable unless their contents are submitted as program source.

```text
/1111 file-write ideas.txt build a counter notecard
/1111 file-read ideas.txt
/1111 files
/1111 file-delete ideas.txt

/1111 notecard Counter
/1111 notecards
/1111 forth-run Counter
/1111 notecard-delete Counter
```

## Browser-to-Second-Life Bridge

The bridge lets the browser console queue Forth or RubyForth source. A Second
Life object with this script polls the queue and evaluates one queued job at a
time.

### Server Setup

Set a nonempty secret before starting the server:

```sh
export MSSL_FORTH_BRIDGE_TOKEN='replace-with-a-long-random-token'
```

Set that exact same value in the script:

```lsl
string BRIDGE_TOKEN = "replace-with-a-long-random-token";
```

Do not publish the token in a public script. Anyone who knows it can enqueue
jobs for a bridge-enabled object.

### Object Setup

After saving or resetting the configured script:

```text
/1111 bridge-on
```

The object polls every three seconds. Stop polling with:

```text
/1111 bridge-off
```

`bridge-poll` sends one immediate poll request, which is useful while
diagnosing connectivity.

### Queue a Job

Open `https://your-server.example/forth/ui`. Paste Forth source, enter the
bridge token, and choose **Queue Forth for SL**. For example:

```forth
"Queued job reached Second Life" sl.owner_say
```

Use **Queue Ruby for SL** only for RubyForth computation or output. RubyForth
does not currently produce `sl.*` calls, so it will not modify the in-world
object.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| No startup message | Confirm the object contains a saved script and inspect the Second Life script error window. |
| No command response | Use `/1111`, confirm you own the object, and verify `SERVERURL` is reachable over HTTPS. |
| Output appears in the wrong place | Send `/1111 channel 0`, or use a `*-say` command with an explicit channel. |
| Forth reports stack underflow | Supply the word's inputs. For `sl.say`, push channel and then message. |
| Ruby parsing fails | Use one statement per line, omit semicolons, and use only the documented RubyForth subset. |
| Notecard upload fails | Ensure the notecard is in the same object's inventory and its name exactly matches the command. |
| Bridge refuses to start | Set a nonempty `BRIDGE_TOKEN` in the script. |
| Bridge receives no job | Ensure the server and script use the same `MSSL_FORTH_BRIDGE_TOKEN`, then use `/1111 bridge-poll`. |
| Variable text is malformed | Avoid special characters listed in the variable warning until escaping is corrected. |
| Object action is ignored | Confirm it came from Forth rather than RubyForth, and check normal Second Life permissions and parcel restrictions. |

## Practical Workflow

1. Use `/1111 forth ...` for quick arithmetic or a short test.
2. Move multiline logic into a named inventory notecard.
3. Upload with `/1111 notecard Name` after every notecard edit.
4. Use `forth-run` for object actions and `ruby-run` for compact control-flow
	 programs and server-side calculations.
5. Enable the bridge only when browser-queued work must reach a specific
	 in-world object.
