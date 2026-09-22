require 'ollama_game_client_magnus'

client = OllamaGameClientNative::PartitionedClient.new(
  server_url: 'https://your-host',
  team: 'arcade',
  player: 'player-42',
  storage_path: './save_data'
)

game = OllamaGameClientNative::RuleRuntime.new(client)
game.before_turn do |state, action|
  next state.merge('last_action' => action, 'turns' => state.fetch('turns', 0) + 1)
end
game.after_turn do |state, directive|
  inventory = Array(state['inventory']).uniq
  state.merge('inventory' => inventory, 'game_over' => directive.fetch('game_over', false))
end

# After restarting the game process, restore the last local partitioned state:
# game.restore!

# This can be called from a raylib frame/update loop. Ruby supplies arbitrary
# algorithms, control flow, data structures, and game-specific validation.
next_state = game.turn('open the north door', game_prompt: 'Fantasy dungeon. Keep choices concise.')
puts next_state
