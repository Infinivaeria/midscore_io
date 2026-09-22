require 'webrick'
require 'net/http'
require 'uri'

# Define the Puma server URL
PUMA_URL = 'http://0.0.0.0:8080'

# Create a simple reverse proxy using WEBrick
class ReverseProxy < WEBrick::HTTPServlet::AbstractServlet
  def do_GET(request, response)
    uri = URI.parse("#{PUMA_URL}#{request.path}")
    proxy_response = Net::HTTP.get_response(uri)

    response.status = proxy_response.code.to_i
    response.body = proxy_response.body
    proxy_response.each_header do |key, value|
      response[key] = value
    end
  end

  def do_POST(request, response)
    uri = URI.parse("#{PUMA_URL}#{request.path}")
    proxy_response = Net::HTTP.post_form(uri, request.query)

    response.status = proxy_response.code.to_i
    response.body = proxy_response.body
    proxy_response.each_header do |key, value|
      response[key] = value
    end
  end
end

# Set up the WEBrick server
server = WEBrick::HTTPServer.new(Port: 8080)
server.mount '/', ReverseProxy

# Start the WEBrick server
trap('INT') { server.shutdown }
server.start
