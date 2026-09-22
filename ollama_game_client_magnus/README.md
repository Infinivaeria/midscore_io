# Ollama Game Client Magnus

Independent Rust crate for raylib-ruby or other Ruby game clients. Rust performs HTTP transport to the Ollama game relay through Magnus. The included Ruby adapter persists returned directives through the local Ruby `ManagedPartitionedArray` implementation. Set `OLLAMA_GAME_CLIENT_PARTITIONED_ARRAY_PATH` when the partitioned-array library is installed outside this workspace's default location.

## Build

```bash
./build-ruby-extension.sh
```

Copy or link `target/release/ollama_game_client_magnus.so` where Ruby can load native extensions, then load it with:

```ruby
require "ollama_game_client_magnus"
client = OllamaGameClientNative::PartitionedClient.new(
  server_url: "https://your-host",
  team: "arcade",
  player: "player-42",
  storage_path: "./save_data"
)
result = client.turn(action: "open the north door", game_prompt: "Fantasy dungeon.")
```

`PartitionedClient#turn` saves each server directive locally. `latest_directive` returns the most recent directive for the same team/player key, and `RuleRuntime#restore!` reapplies its state after a client restart.

## Programmable Game Rules

Ruby is the Turing-complete game runtime: it owns raylib rendering, input, physics, validation, loops, and custom algorithms. `OllamaGameClientNative::RuleRuntime` provides `before_turn` and `after_turn` callbacks for arbitrary local Ruby rules around the relay request. Model output stays JSON data and is never evaluated as code.

See [ruby/rule_runtime_example.rb](ruby/rule_runtime_example.rb) for a stateful rule setup suitable for a raylib update loop.