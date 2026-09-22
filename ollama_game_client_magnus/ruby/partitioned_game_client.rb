require 'json'
require 'fileutils'

partitioned_array_path = ENV.fetch(
  'OLLAMA_GAME_CLIENT_PARTITIONED_ARRAY_PATH',
  '/root/midscore_io/lib/partitioned_array/lib'
)
require File.join(partitioned_array_path, 'managed_partitioned_array')

module OllamaGameClientNative
  class PartitionedClient
    def initialize(server_url:, team:, player:, storage_path: './ollama_game_client_state')
      @transport = Transport.new(server_url, team, player)
      @key = "#{team}:#{player}"
      FileUtils.mkdir_p(storage_path)
      @store = ManagedPartitionedArray.new(
        db_path: storage_path,
        db_name: 'ollama_game_directives',
        endless_add: true,
        dynamically_allocates: true,
        has_capacity: false
      )
      marker = File.join(storage_path, 'ollama_game_directives[0]', 'last_entry.json')
      if File.exist?(marker)
        @store.load_everything_from_files!
      else
        @store.allocate
        @store.save_everything_to_files!
      end
    end

    def routes
      JSON.parse(@transport.routes_json)
    end

    def health
      JSON.parse(@transport.health_json)
    end

    def state
      JSON.parse(@transport.state_json).fetch('state')
    end

    def reset
      result = JSON.parse(@transport.reset_json)
      persist(result)
      result.fetch('state')
    end

    def turn(action:, game_prompt: '', state: nil)
      state_json = state.nil? ? '' : JSON.generate(state)
      result = JSON.parse(@transport.turn_json(action, game_prompt, state_json))
      persist(result)
      result
    end

    def latest_directive
      latest = nil
      0.upto(@store.latest_id) do |index|
        entry = @store.get(index, hash: true)
        data = entry['data'] if entry.is_a?(Hash)
        latest = data['directive'] if data.is_a?(Hash) && data['key'] == @key
      end
      latest
    end

    private

    def persist(result)
      @store.add do |entry|
        entry['key'] = @key
        entry['directive'] = result
      end
      @store.save_everything_to_files!
    end
  end

  # Ruby owns the complete local game program. The relay only returns data.
  class RuleRuntime
    def initialize(client, initial_state: nil)
      @client = client
      @state = initial_state || client.state
      @before_turn = []
      @after_turn = []
    end

    attr_reader :state

    def before_turn(&rule)
      raise ArgumentError, 'a rule block is required' unless rule

      @before_turn << rule
      self
    end

    def after_turn(&rule)
      raise ArgumentError, 'a rule block is required' unless rule

      @after_turn << rule
      self
    end

    def turn(action, game_prompt: '')
      @before_turn.each { |rule| @state = require_state!(rule.call(@state, action)) }
      result = @client.turn(action: action, game_prompt: game_prompt, state: @state)
      directive = result.fetch('directive')
      @state = require_state!(directive.fetch('state', result.fetch('state')))
      @after_turn.each { |rule| @state = require_state!(rule.call(@state, directive)) }
      @state
    end

    def reset
      @state = @client.reset
    end

    def restore!
      result = @client.latest_directive
      return @state if result.nil?

      directive = result.fetch('directive', result)
      @state = require_state!(directive.fetch('state', result.fetch('state', @state)))
    end

    private

    def require_state!(value)
      raise TypeError, 'game rules must return a Hash state object' unless value.is_a?(Hash)

      value
    end
  end
end
