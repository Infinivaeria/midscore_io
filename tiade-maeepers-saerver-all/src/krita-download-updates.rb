require 'net/http'
require 'uri'

# Function to download files from a given URL
def download_files(directory_url)
  uri = URI(directory_url)
  response = Net::HTTP.get_response(uri)

  unless response.is_a?(Net::HTTPSuccess)
    puts "Failed to retrieve directory listing: #{response.message}"
    return
  end

  # Extract file links from the directory listing
  files = response.body.scan(%r{<a href="([^"?/]+)">}).flatten
  puts "Files found: #{files.join(', ')}"

  files.each do |file|
    file_url = URI.join(directory_url, file).to_s
    file_name = file

    File.open(file_name, 'wb') do |f|
      puts "Downloading #{file_url}..."
      f.write(Net::HTTP.get(URI(file_url)))
    end
    puts "Downloaded: #{file_name}"
  end
end

# Example usage
directory_url = 'https://cdn.kde.org/ci-builds/graphics/krita/master/windows/' # Replace with the target URL
download_files(directory_url)
