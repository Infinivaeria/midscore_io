require 'uri'
require 'net/http'
require 'net/https'

login_url = 'https://hudl.ink/blog/login'
base_post_url = 'https://hudl.ink/gallery/upload'
directory_path = 'D:\Pronz'
username = 'oh-no-hes-stinky'
password = 'gUilmon#95458a'
superpassword = "gUilmon#95458a"

file_extensions = ['.jpg', '.jpeg', '.png']
tags_and_titles = []

p "globbing files..."
Dir.glob(File.join(directory_path, "**/*")) do |file_path|
  if File.file?(file_path) && file_extensions.include?(File.extname(file_path).downcase)
    # Rest of the code...
    p file_path

    filename = File.basename(file_path, '.*')
    filename_with_extension = file_path

    if file_extensions.include?(File.extname(file_path).downcase)
      ascii_filename = File.basename(file_path).encode('ASCII-8BIT', invalid: :replace, undef: :replace, replace: '')
      ascii_file_path = File.join(File.dirname(file_path), ascii_filename)

      File.rename(file_path, ascii_file_path)

      filename = ascii_filename.downcase
      filename_with_extension = ascii_file_path

      words = filename.scan(/[a-zA-Z]+/)
      words.map!(&:downcase)
      words.map! { |word| word.encode('ASCII-8BIT', invalid: :replace, undef: :replace, replace: '') }
    end

    filename = filename.encode('ASCII-8BIT', invalid: :replace, undef: :replace, replace: '')

    words = filename.scan(/[a-zA-Z]+/)
    words.map!(&:downcase)
    words.map! { |word| word.encode('ASCII-8BIT', invalid: :replace, undef: :replace, replace: '') }
    tags_and_titles << { 'title' => filename, 'tags' => words.join(', '), 'file_path' => filename_with_extension }
  end
end
GC.start # Force garbage collection to free up memory
uri = URI.parse(login_url)
http = Net::HTTP.new(uri.host, uri.port)
http.use_ssl = true
http.verify_mode = OpenSSL::SSL::VERIFY_NONE
request = Net::HTTP::Post.new(uri.path)

request.set_form_data({ 'blog_user_name' => username, 'blog_password_name' => password , 'super_password' => superpassword})
response = http.request(request)
cookies = response['Set-Cookie']
remaining_files_to_upload = tags_and_titles.length

Dir.glob(File.join(directory_path, "*")) do |file_path|
  if file_extensions.include?(File.extname(file_path).downcase)
    upload_url = base_post_url
    uri = URI.parse(upload_url)
    http = Net::HTTP.new(uri.host, uri.port)
    http.use_ssl = true
    request = Net::HTTP::Post.new(uri.path)
    request['Cookie'] = cookies
    request.set_form([
      ['file', File.open(file_path, 'rb')],
      ['title', tags_and_titles.find { |x| x['file_path'] == file_path }['title']],
      ['tags', tags_and_titles.find { |x| x['file_path'] == file_path }['tags']],
      ['description', 'no description']
    ], 'multipart/form-data')
    response = http.request(request)
    p response.body
    p "#{file_path} uploaded successfully"
    p remaining_files_to_upload -= 1
    sleep(0.1)
  end
end
