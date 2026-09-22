require 'json'
require 'net/http'
require 'uri'

module OllamaGameClient
  class Error < StandardError; end

  class Client
    def initialize(server_url:, team:, player:)
      @server_url = server_url.sub(%r{/+$}, '')
      @team = team
      @player = player
    end

    attr_reader :server_url, :team, :player

    def routes
      request('GET', '/ollama')
    end

    def health
      request('GET', '/ollama/health')
    end

    def chat(message)
      request('POST', "/chat/#{escape(team)}", { message: message })
    end

    def turn(action:, game_prompt: nil, state: nil)
      payload = { action: action }
      payload[:game_prompt] = game_prompt unless game_prompt.nil? || game_prompt.empty?
      payload[:state] = state unless state.nil?
      request('POST', game_path('turn'), payload)
    end

    def state
      request('GET', game_path('state')).fetch('state')
    end

    def reset
      request('POST', game_path('reset')).fetch('state')
    end

    private

    def game_path(action)
      "/game/#{escape(team)}/#{escape(player)}/#{action}"
    end

    def escape(value)
      URI.encode_uri_component(value)
    end

    def request(method, path, payload = nil)
      uri = URI.parse("#{server_url}#{path}")
      request_class = Net::HTTP.const_get(method.capitalize)
      request = request_class.new(uri)
      request['Accept'] = 'application/json'
      if payload
        request['Content-Type'] = 'application/json'
        request.body = JSON.generate(payload)
      end

      response = Net::HTTP.start(uri.hostname, uri.port, use_ssl: uri.scheme == 'https') do |http|
        http.request(request)
      end
      body = JSON.parse(response.body)
      return body if response.is_a?(Net::HTTPSuccess)

      raise Error, "relay returned HTTP #{response.code}: #{body.fetch('error', response.body)}"
    rescue JSON::ParserError => e
      raise Error, "relay returned invalid JSON: #{e.message}"
    end
  end
end
