# The Meta-Game Project
# Using LineDB abstractions over ManagedPartitionedArray/PartitionedArray

require 'raylib'
require_relative 'line_db'  # Assuming line_db.rb is in the same directory

include Raylib

# Constants
DEBUG_MODE = true
GRID_SIZE = 32

# Movement directions
DIRECTIONS = {
  up: Vector2.create(0, -1),
  down: Vector2.create(0, 1),
  left: Vector2.create(-1, 0),
  right: Vector2.create(1, 0),
  up_left: Vector2.create(-1, -1),
  up_right: Vector2.create(1, -1),
  down_left: Vector2.create(-1, 1),
  down_right: Vector2.create(1, 1)
}

# Movement keys mapping
MOVEMENT_KEYS = {
  KEY_W => :up,
  KEY_S => :down,
  KEY_A => :left,
  KEY_D => :right,
  KEY_Q => :up_left,
  KEY_E => :up_right,
  KEY_Z => :down_left,
  KEY_C => :down_right
}

def debug(msg)
  puts msg if DEBUG_MODE
end

# Initialize LineDB
line_db = LineDB.new

# Access or create the player data
player_db = line_db.db('player_data')
player_id = 0
unless player_db.get(player_id)
  player_db.set(player_id) do |hash|
    hash[:position] = { x: 0.0, y: 0.0 }
    hash[:spin_index] = 1
  end
end

# Access or create the neutri data
neutri_db = line_db.db('neutri_data')
neutri_id = 0
unless neutri_db.get(neutri_id)
  neutri_db.set(neutri_id) do |hash|
    hash[:position] = { x: 0.0, y: 0.0, z: 0.0 }
    hash[:spin_index] = player_db.get(player_id)[:spin_index]
    hash[:rotation_angle] = 0.0
  end
end

# Initialize window
monitor = GetCurrentMonitor()
screen_width = GetMonitorWidth(monitor)
screen_height = GetMonitorHeight(monitor)

SetConfigFlags(FLAG_VSYNC_HINT)
InitWindow(screen_width, screen_height, '"The Meta-Game"')
SetWindowState(FLAG_WINDOW_UNDECORATED)
SetExitKey(KEY_HOME)
SetTargetFPS(60)

# Placeholder assets (simple shapes)
class PlaceholderAsset
  def self.draw_player(position)
    DrawCircle(position.x, position.y, 16, BLUE)
  end
end

# Camera setup
camera = Camera2D.new
camera.offset = Vector2.create(screen_width / 2.0, screen_height / 2.0)
camera.target = Vector2.create(0.0, 0.0)
camera.rotation = 0.0
camera.zoom = 1.0

# 3D camera for Neutri rendering
camera3d = Camera3D.new
camera3d.position = Vector3.create(0.0, 10.0, 10.0)
camera3d.target = Vector3.create(0.0, 0.0, 0.0)
camera3d.up = Vector3.create(0.0, 1.0, 0.0)
camera3d.fovy = 45.0
camera3d.projection = CAMERA_PERSPECTIVE

# Qubit settings
SPIN_LEVELS = [0.0, 0.5, 1.0, 1.5, 3.0]

# Main game loop
until WindowShouldClose()
  delta_time = GetFrameTime()

  # Get player data
  player = player_db.get(player_id)
  position = player[:position]
  spin_index = player[:spin_index]

  # Movement input
  movement = Vector2.create(0, 0)
  MOVEMENT_KEYS.each do |key, direction_sym|
    movement = Vector2.add(movement, DIRECTIONS[direction_sym]) if IsKeyDown(key)
  end

  # Update position
  if movement.x != 0 || movement.y != 0
    movement = Vector2Normalize(movement)
    position[:x] += movement.x * delta_time * 5
    position[:y] += movement.y * delta_time * 5
    player_db.set(player_id) do |hash|
      hash[:position] = position
    end
    camera.target = Vector2.create(
      position[:x] * GRID_SIZE + GRID_SIZE / 2,
      position[:y] * GRID_SIZE + GRID_SIZE / 2
    )
  end

  # Spin adjustment
  if IsKeyPressed(KEY_UP)
    spin_index += 1 if spin_index < SPIN_LEVELS.size - 1
    player_db.set(player_id) do |hash|
      hash[:spin_index] = spin_index
    end
  elsif IsKeyPressed(KEY_DOWN)
    spin_index -= 1 if spin_index > 0
    player_db.set(player_id) do |hash|
      hash[:spin_index] = spin_index
    end
  end

  # Update Neutri data
  neutri = neutri_db.get(neutri_id)
  neutri_spin_index = spin_index
  rotation_angle = neutri[:rotation_angle] + SPIN_LEVELS[spin_index] * delta_time * 50.0
  neutri_db.set(neutri_id) do |hash|
    hash[:spin_index] = neutri_spin_index
    hash[:rotation_angle] = rotation_angle
  end

  BeginDrawing()
    ClearBackground(RAYWHITE)

    # 2D rendering
    BeginMode2D(camera)
      # Draw grid
      (0..(screen_width / GRID_SIZE)).each do |x|
        DrawLine(
          x * GRID_SIZE, 0,
          x * GRID_SIZE, screen_height,
          LIGHTGRAY
        )
      end
      (0..(screen_height / GRID_SIZE)).each do |y|
        DrawLine(
          0, y * GRID_SIZE,
          screen_width, y * GRID_SIZE,
          LIGHTGRAY
        )
      end

      # Draw player
      player_screen_position = Vector2.create(
        position[:x] * GRID_SIZE,
        position[:y] * GRID_SIZE
      )
      PlaceholderAsset.draw_player(player_screen_position)
    EndMode2D()

    # 3D rendering for Neutri
    BeginMode3D(camera3d)
      DrawGrid(10, 1.0)
      neutri_position = Vector3.create(
        neutri[:position][:x],
        neutri[:position][:y],
        neutri[:position][:z]
      )
      spin_value = SPIN_LEVELS[neutri_spin_index]
      rotation_angle = neutri[:rotation_angle]

      if spin_value == 0.0
        DrawSphere(neutri_position, 1.0, Fade(GRAY, 0.5))
      else
        DrawSphereEx(neutri_position, 1.0, 32, 32, PURPLE)
        DrawSphereWires(neutri_position, 1.0, 16, 16, BLACK)
        end_pos = Vector3.create(
          neutri_position.x + Math.cos(rotation_angle),
          neutri_position.y + Math.sin(rotation_angle),
          neutri_position.z
        )
        DrawLine3D(neutri_position, end_pos, BLACK)
      end
    EndMode3D()

    # HUD
    DrawText("Spin Value: #{spin_value}", 10, 10, 20, BLACK)
    DrawText("Press [Up] to increase spin, [Down] to decrease spin.", 10, 40, 20, DARKGRAY)
    DrawText("Position: (#{position[:x].round(2)}, #{position[:y].round(2)})", 10, 70, 20, DARKGRAY)
    DrawFPS(screen_width - 100, 10)
  EndDrawing()

  # Save data to LineDB
  player_db.save_to_files!
  neutri_db.save_to_files!
end

# Clean up
CloseWindow()
'exit gameloop success'
