require 'rubygems'
require 'bundler/setup'
require 'listen'
require 'time'
require 'wdm' # Ensure wdm gem is installed for Windows support

# Directory to monitor
MONITOR_DIRECTORY = 'C:\\BashAssetFinal'

unless Dir.exist?(MONITOR_DIRECTORY)
  puts "The directory '#{MONITOR_DIRECTORY}' does not exist."
  exit
else
  puts "Monitoring the directory: #{MONITOR_DIRECTORY}"
end

# Initialize the listener
listener = Listen.to(
  MONITOR_DIRECTORY,
  recursive: true
) do |modified, added, removed|
  timestamp = Time.now.strftime('%Y-%m-%d %H:%M:%S')

  modified.each do |file|
    action = 'Modified'
    puts "[#{timestamp}] #{action}: #{file}"
  end

  added.each do |file|
    action = 'Added'
    puts "[#{timestamp}] #{action}: #{file}"
  end

  removed.each do |file|
    action = 'Removed'
    puts "[#{timestamp}] #{action}: #{file}"
  end
end

# Start the listener
begin
  listener.start
  puts 'Monitoring the BashAssetFinal directory for changes. Press Ctrl+C to stop.'
  sleep
rescue Interrupt
  puts "\nMonitoring stopped by user."
ensure
  listener.stop
end