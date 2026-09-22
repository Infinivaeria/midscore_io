require 'digest/md5'

# File name to check
filename = 'C:/BashAssetFinal-build/rustby-vm.zip'

unless File.exist?(filename)
  puts "File #{filename} not found!"
  exit 1
end

md5 = Digest::MD5.new
File.open(filename, 'rb') do |file|
  while chunk = file.read(1024)
    md5.update(chunk)
  end
end

puts "Rustby-VM MD5 Checksum: #{md5.hexdigest}"

# write md5 to disk
File.write('C:/BashAssetFinal-build/rustby-vm.md5', md5.hexdigest)
