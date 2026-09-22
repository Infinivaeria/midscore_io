# The Meta-Game Project
# Using LineDB abstractions over ManagedPartitionedArray/PartitionedArray

require 'raylib'

include Raylib

# Constants
DEBUG_MODE = true
GRID_SIZE = 32 # Size of each grid cell in pixels

# Movement directions mapped to discrete grid steps
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

# Movement keys mapping to directions
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

# Debugging method
def debug(msg)
  puts msg if DEBUG_MODE
end

# Initialize LineDB
line_db = LineDB.new

# Access or create the player database using [] notation
player_db = line_db['player_data']
player_id = 0
player_db[player_id] ||= { position: { x: 0, y: 0 }, spin_index: 1 } # Discrete grid positions

# Access or create the neutri database using [] notation
neutri_db = line_db['neutri_data']
neutri_id = 0
neutri_db[neutri_id] ||= { position: { x: 0, y: 0, z: 0 }, spin_index: player_db[player_id][:spin_index],
                           rotation_angle: 0 }

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

# Camera setup for 2D rendering
camera = Camera2D.new
camera.offset = Vector2.create(screen_width / 2.0, screen_height / 2.0)
camera.target = Vector2.create(0, 0)
camera.rotation = 0.0
camera.zoom = 1.0

# 3D camera setup for Neutri rendering
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

  # Retrieve player data from LineDB
  player = player_db[player_id]
  position = player[:position]
  spin_index = player[:spin_index]

  # Handle movement input
  movement = Vector2.create(0, 0)
  MOVEMENT_KEYS.each do |key, direction_sym|
    movement = Vector2.add(movement, DIRECTIONS[direction_sym]) if IsKeyDown(key)
  end

  # Update position based on movement
  if movement.x != 0 || movement.y != 0
    # Normalize movement to ensure consistent speed
    direction = Vector2Normalize(movement)
    # Move one grid cell per key press
    position[:x] += direction.x.round
    position[:y] += direction.y.round
    # Update player position in LineDB
    player_db[player_id][:position] = position
    # Update camera target to follow the player
    camera.target = Vector2.create(
      (position[:x] * GRID_SIZE) + (GRID_SIZE / 2),
      (position[:y] * GRID_SIZE) + (GRID_SIZE / 2)
    )
  end

  # Handle spin adjustments
  if IsKeyPressed(KEY_UP)
    spin_index += 1 if spin_index < SPIN_LEVELS.size - 1
    player_db[player_id][:spin_index] = spin_index
  elsif IsKeyPressed(KEY_DOWN)
    spin_index -= 1 if spin_index > 0
    player_db[player_id][:spin_index] = spin_index
  end

  # Update Neutri data in LineDB
  neutri_db[neutri_id][:spin_index] = spin_index
  neutri_db[neutri_id][:rotation_angle] += SPIN_LEVELS[spin_index] * delta_time * 50.0

  BeginDrawing()
  ClearBackground(RAYWHITE)

  # 2D Rendering
  BeginMode2D(camera)
  # Draw grid lines
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

  # Draw player at discrete grid position
  player_screen_position = Vector2.create(
    position[:x] * GRID_SIZE,
    position[:y] * GRID_SIZE
  )
  PlaceholderAsset.draw_player(player_screen_position)
  EndMode2D()

  # 3D Rendering for Neutri
  BeginMode3D(camera3d)
  DrawGrid(10, 1.0)
  neutri_position = Vector3.create(
    neutri_db[neutri_id][:position][:x],
    neutri_db[neutri_id][:position][:y],
    neutri_db[neutri_id][:position][:z]
  )
  spin_value = SPIN_LEVELS[neutri_db[neutri_id][:spin_index]]
  rotation_angle = neutri_db[neutri_id][:rotation_angle]

  if spin_value.zero?
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

  # Heads-Up Display (HUD)
  DrawText("Spin Value: #{spin_value}", 10, 10, 20, BLACK)
  DrawText('Press [Up] to increase spin, [Down] to decrease spin.', 10, 40, 20, DARKGRAY)
  DrawText("Position: (#{position[:x]}, #{position[:y]})", 10, 70, 20, DARKGRAY)
  DrawFPS(screen_width - 100, 10)
  EndDrawing()

  # Persist data changes to LineDB
  line_db.save_databases!
end

# Clean up resources before exiting
CloseWindow()

'exit gameloop success'
