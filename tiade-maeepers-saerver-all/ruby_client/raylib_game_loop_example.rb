require_relative 'ollama_game_client'

# Call this from a raylib-ruby update loop or from Ruby evaluated through Magnus.
# The game owns rendering, input, physics, and validation; Ollama supplies turn data.
class GameDirector
  def initialize(client)
    @client = client
    @state = client.state
    @choices = []
    @narrative = ''
    @game_over = false
  end

  attr_reader :state, :choices, :narrative, :game_over

  def apply_player_action(action, game_prompt:)
    result = @client.turn(action: action, game_prompt: game_prompt, state: @state)
    directive = result.fetch('directive')
    @narrative = directive.fetch('narrative', result.fetch('response'))
    @state = directive.fetch('state', result.fetch('state'))
    @choices = directive.fetch('choices', [])
    @game_over = directive.fetch('game_over', false)
    directive
  end
end

# Example setup for a raylib-ruby application:
# client = OllamaGameClient::Client.new(
#   server_url: "https://your-host",
#   team: "arcade",
#   player: "player-42"
# )
# director = GameDirector.new(client)
#
# Raylib.init_window(1280, 720, "Ollama Game")
# until Raylib.window_should_close?
#   if Raylib.is_key_pressed(Raylib::KEY_ENTER)
#     director.apply_player_action("open the north door", game_prompt: "A fantasy dungeon. Keep choices concise.")
#   end
#   Raylib.begin_drawing
#   Raylib.clear_background(Raylib::BLACK)
#   Raylib.draw_text(director.narrative, 32, 32, 24, Raylib::RAYWHITE)
#   Raylib.end_drawing
# end
