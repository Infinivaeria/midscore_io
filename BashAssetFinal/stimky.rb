require 'pathname'
require 'fileutils'
require 'listen'
require 'time'

# ================================
# Configuration
# ================================

# Source directory to monitor
SOURCE_DIR = Pathname.new('C:/BashAssetFinal').cleanpath

# Backup directory (can be a network path or local)
BACKUP_DIR = Pathname.new('\\\\Stimky-mini-nuk\\nas\\BashAssetFinalBak') # Example for network
# BACKUP_DIR = Pathname.new('C:/Path/To/Backup') # Example for local

# Log file path
LOG_FILE = SOURCE_DIR.join('backup_log.txt')

# Sentinel file to indicate initial backup completion
SENTINEL_FILE = BACKUP_DIR.join('.backup_completed')

# Lock file to prevent concurrent initial backups
LOCK_FILE = BACKUP_DIR.join('.backup_lock')

# ================================
# Helper Methods
# ================================

def log(message)
  timestamp = Time.now.strftime('%Y-%m-%d %H:%M:%S')
  log_message = "[#{timestamp}] #{message}"
  puts log_message
  begin
    File.open(LOG_FILE.to_s, 'a') { |file| file.puts log_message }
  rescue StandardError => e
    puts "[#{timestamp}] Failed to write to log file: #{e.message}"
  end
end

def copy_file(source, destination)
  FileUtils.mkdir_p(destination.parent.to_s) unless destination.parent.directory?
  FileUtils.copy_file(source.to_s, destination.to_s)
  log "Copied: #{source} to #{destination}"
rescue StandardError => e
  log "Error copying #{source} to #{destination}: #{e.message}"
end

def delete_file(destination)
  if destination.file?
    File.delete(destination.to_s)
    log "Deleted file: #{destination}"
  elsif destination.directory?
    FileUtils.rm_rf(destination.to_s)
    log "Deleted directory: #{destination}"
  end
rescue StandardError => e
  log "Error deleting #{destination}: #{e.message}"
end

# ================================
# Initial Backup
# ================================

def initial_backup
  # Check if initial backup has already been completed
  if SENTINEL_FILE.exist?
    log 'Initial backup already completed. Skipping.'
    return
  end

  # Acquire a lock to prevent concurrent backups
  lock = File.open(LOCK_FILE.to_s, 'w')
  locked = lock.flock(File::LOCK_EX | File::LOCK_NB)

  unless locked
    log 'Another instance is performing the initial backup. Skipping.'
    return
  end

  begin
    log "Starting initial backup from #{SOURCE_DIR} to #{BACKUP_DIR}"
    FileUtils.mkdir_p(BACKUP_DIR.to_s) unless BACKUP_DIR.directory?
    FileUtils.cp_r("#{SOURCE_DIR}/.", BACKUP_DIR.to_s, remove_destination: true)
    log 'Initial backup completed.'

    # Create the sentinel file to indicate completion
    SENTINEL_FILE.touch
    log "Sentinel file created at #{SENTINEL_FILE}"
  rescue StandardError => e
    log "Error during initial backup: #{e.message}"
  ensure
    # Release the lock and close the lock file
    lock.flock(File::LOCK_UN)
    lock.close
    # Optionally, delete the lock file
    File.delete(LOCK_FILE) if LOCK_FILE.exist?
  end
end

# ================================
# Monitor and Sync
# ================================

def start_listener
  listener = Listen.to(SOURCE_DIR.to_s, recursive: true) do |modified, added, removed|
    modified.each do |file|
      source_path = Pathname.new(file)
      rel_path = source_path.relative_path_from(SOURCE_DIR)
      backup_path = BACKUP_DIR.join(rel_path)
      copy_file(source_path, backup_path)
    end

    added.each do |file|
      source_path = Pathname.new(file)
      rel_path = source_path.relative_path_from(SOURCE_DIR)
      backup_path = BACKUP_DIR.join(rel_path)
      copy_file(source_path, backup_path)
    end

    removed.each do |file|
      source_path = Pathname.new(file)
      rel_path = source_path.relative_path_from(SOURCE_DIR)
      backup_path = BACKUP_DIR.join(rel_path)
      delete_file(backup_path)
    end
  end

  listener.start
  log 'Monitoring directory for changes. Press Ctrl+C to stop.'
  sleep
end

# ================================
# Main Execution
# ================================

# Ensure the source directory exists
if SOURCE_DIR.directory?
  log "Monitoring directory: #{SOURCE_DIR}"
else
  log "Error: Source directory #{SOURCE_DIR} does not exist."
  exit
end

# Ensure the backup directory exists or attempt to create it
begin
  if BACKUP_DIR.directory?
    log "Backup directory exists: #{BACKUP_DIR}"
  else
    log "Backup directory does not exist. Attempting to create: #{BACKUP_DIR}"
    FileUtils.mkdir_p(BACKUP_DIR.to_s)
    log 'Backup directory created successfully.'
  end
rescue StandardError => e
  log "Error creating backup directory: #{e.message}"
  exit
end

# Perform the initial backup if not already done
initial_backup

# Start monitoring for changes
start_listener
