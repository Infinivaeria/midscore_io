# By Aylon-Arlon*, 2024-12-08 10:02AM PST -7
# ######
# PURPOSE: To mirror and backup a specified file directory to be used and
# always running in the command line
# USAGE:
# ruby rustby-filemirror.exe[.rb] --source=<SOURCE_DIR> --backup=<BACKUP_DIR>
# EXAMPLE:
# ruby rustby-filemirror.exe[.rb] --source=/Users/username/Documents --backup=/Users/username/Backups
##### 
#
##!/usr/bin/env ruby
require 'pathname'
require 'fileutils'
require 'listen'
require 'optparse'

class FileMirror
  SENTINEL_FILE_NAME = '.backup_completed'.freeze
  LOCK_FILE_NAME = '.backup_lock'.freeze

  def initialize(source_dir, backup_dir)
    @source_dir = Pathname.new(source_dir).cleanpath
    @backup_dir = Pathname.new(backup_dir).cleanpath
    @sentinel_file = @backup_dir.join(SENTINEL_FILE_NAME)
    @lock_file = @backup_dir.join(LOCK_FILE_NAME)
  end

  # Perform the initial backup if it hasn't been done yet
  def initial_backup
    return if @sentinel_file.exist?

    acquire_lock do
      perform_backup unless @sentinel_file.exist?
    end
  end

  # Start monitoring the source directory for changes
  def start_monitoring
    listener = Listen.to(@source_dir.to_s, recursive: true) do |modified, added, removed|
      handle_changes(modified, added, removed)
    end
    listener.start
    sleep
  end

  private

  # Acquire an exclusive lock to prevent concurrent backups
  def acquire_lock
    begin
      lock = File.open(@lock_file.to_s, 'w')
      locked = lock.flock(File::LOCK_EX | File::LOCK_NB)

      unless locked
        # Another instance is performing the initial backup
        return
      end

      begin
        yield
      ensure
        lock.flock(File::LOCK_UN)
        lock.close
        File.delete(@lock_file) if @lock_file.exist?
      end
    rescue Errno::ENOENT
      # Backup directory does not exist
      FileUtils.mkdir_p(@backup_dir.to_s)
      retry
    rescue StandardError
      # Handle other exceptions as needed
    end
  end

  # Perform the actual backup from source to backup directory
  def perform_backup
    FileUtils.mkdir_p(@backup_dir.to_s) unless @backup_dir.directory?
    FileUtils.cp_r("#{@source_dir}/.", @backup_dir.to_s, remove_destination: true)
    @sentinel_file.touch
  rescue StandardError
    # Handle or ignore errors as needed
  end

  # Handle file system changes
  def handle_changes(modified, added, removed)
    modified.each { |file| copy_file(file) }
    added.each    { |file| copy_file(file) }
    removed.each  { |file| delete_file(file) }
  end

  # Copy a single file to the backup directory
  def copy_file(source)
    source_path = Pathname.new(source)
    rel_path = source_path.relative_path_from(@source_dir)
    backup_path = @backup_dir.join(rel_path)

    FileUtils.mkdir_p(backup_path.parent.to_s) unless backup_path.parent.directory?
    FileUtils.copy_file(source_path.to_s, backup_path.to_s)
  rescue StandardError
    # Handle or ignore errors as needed
  end

  # Delete a single file or directory from the backup directory
  def delete_file(source)
    source_path = Pathname.new(source)
    rel_path = source_path.relative_path_from(@source_dir)
    backup_path = @backup_dir.join(rel_path)

    if backup_path.file?
      File.delete(backup_path.to_s)
    elsif backup_path.directory?
      FileUtils.rm_rf(backup_path.to_s)
    end
  rescue StandardError
    # Handle or ignore errors as needed
  end
end

# ================================
# Command-Line Argument Parsing
# ================================

options = {}
OptionParser.new do |opts|
  opts.banner = "Usage: ruby rustby-filemirror.rb [options]"

  opts.on("-sSOURCE", "--source=SOURCE", "Source directory to monitor") do |s|
    options[:source] = s
  end

  opts.on("-bBACKUP", "--backup=BACKUP", "Backup directory") do |b|
    options[:backup] = b
  end

  opts.on("-h", "--help", "Prints help") do
    puts opts
    exit
  end
end.parse!

# Validate presence of required arguments
if options[:source].nil? || options[:backup].nil?
  puts "Error: Both --source and --backup options are required."
  puts "Usage: ruby rustby-filemirror.rb --source=<SOURCE_DIR> --backup=<BACKUP_DIR>"
  exit(1)
end

source_dir = options[:source]
backup_dir = options[:backup]

# Validate source directory
unless Dir.exist?(source_dir)
  puts "Error: Source directory '#{source_dir}' does not exist."
  exit(1)
end

# Initialize FileMirror
file_mirror = FileMirror.new(source_dir, backup_dir)

# Perform initial backup
file_mirror.initial_backup

# Start monitoring for changes
file_mirror.start_monitoring

