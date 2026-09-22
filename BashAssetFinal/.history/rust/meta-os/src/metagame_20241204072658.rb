# The Meta-Game Project
# Using LineDB abstractions over ManagedPartitionedArray/PartitionedArray

require 'raylib'
require_relative 'line_db'  # Make sure line_db.rb is accessible

include Raylib

# Constants and initializations remain the same...

# Initialize LineDB
line_db = LineDB.new

# Access or create the player database
player_db = line_db.db('player_data')
player_id = 0
player_db.set(player_id) do |hash|
  hash[:position] ||= { x: 0.0, y: 0.0 }
  hash[:spin_index] ||= 1
end

# Access or create the neutri database
neutri_db = line_db.db('neutri_data')
neutri_id = 0
neutri_db.set(neutri_id) do |hash|
  hash[:position] ||= { x: 0.0, y: 0.0, z: 0.0 }
  hash[:spin_index] ||= player_db.get(player_id)[:spin_index]
  hash[:rotation_angle] ||= 0.0
end

# Main game loop
until WindowShouldClose()
  delta_time = GetFrameTime()

  # Retrieve player data
  player = player_db.get(player_id)
  position = player[:position]
  spin_index = player[:spin_index]

  # Movement and game logic remain the same...

  # Update player data
  player_db.set(player_id) do |hash|
    hash[:position] = position
    hash[:spin_index] = spin_index
  end

  # Update neutri data
  neutri_db.set(neutri_id) do |hash|
    hash[:spin_index] = spin_index
    hash[:rotation_angle] += SPIN_LEVELS[spin_index] * delta_time * 50.0
  end

  # Rendering and drawing code remain the same...

end

# Save data before exiting
player_db.save_everything_to_files!
neutri_db.save_everything_to_files!

# Clean up
CloseWindow()
'exit gameloop success'
