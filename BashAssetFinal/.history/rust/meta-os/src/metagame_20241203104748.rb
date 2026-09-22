# The Meta-Game Project
#
# * https://github.com/vaiorabbit/raylib-bindings
#
# To get more information, see:
# * https://www.raylib.com/cheatsheet/cheatsheet.html for API reference
# * https://github.com/vaiorabbit/raylib-bindings/tree/main/examples for more actual codes written in Ruby

require 'raylib'
require 'json'
require 'uri'
require 'bigdecimal'
require 'raylib'

# Version 1.0 Main
# by Aylon Arlon (2024-29-11)

# Constants
GRID_SIZE = BigDecimal('32') # Grid size in pixels
SCREEN_WIDTH = 800
SCREEN_HEIGHT = 600

# Initialize the window
InitWindow(SCREEN_WIDTH, SCREEN_HEIGHT, 'Grid System Game')
SetTargetFPS(60)

# Load images and textures
mascot_west_image = LoadImage('mascot_west.png')
mascot_east_image = LoadImage('mascot_east.png')
mascot_north_image = LoadImage('mascot_north.png')
mascot_south_image = LoadImage('mascot_south.png')

texture_west = LoadTextureFromImage(mascot_west_image)
texture_east = LoadTextureFromImage(mascot_east_image)
texture_north = LoadTextureFromImage(mascot_north_image)
texture_south = LoadTextureFromImage(mascot_south_image)

# Initial grid position (Cartesian coordinate system)
grid_x = BigDecimal('0')
grid_y = BigDecimal('0')

# Movement speed (grid units per second)
movement_speed = BigDecimal('5.0')

# Colors
ruby_red = Color.new(155, 17, 30, 255)

# Initialize the camera
camera = Camera2D.new
camera.offset = Vector2.new(SCREEN_WIDTH / 2.0, SCREEN_HEIGHT / 2.0)
camera.target = Vector2.new((grid_x * GRID_SIZE).to_f, (grid_y * GRID_SIZE).to_f)
camera.rotation = BigDecimal('0.0').to_f
camera.zoom = BigDecimal('1.0').to_f

# Main game loop
until WindowShouldClose()
  delta_time = BigDecimal(GetFrameTime().to_s)

  # Movement input
  move_x = BigDecimal('0')
  move_y = BigDecimal('0')

  move_x += movement_speed * delta_time if IsKeyDown(KEY_RIGHT)
  move_x -= movement_speed * delta_time if IsKeyDown(KEY_LEFT)
  move_y += movement_speed * delta_time if IsKeyDown(KEY_UP)
  move_y -= movement_speed * delta_time if IsKeyDown(KEY_DOWN)

  # Update grid position
  grid_x += move_x
  grid_y += move_y

  # Grid snapping
  grid_x = grid_x.floor
  grid_y = grid_y.floor

  # Update camera target
  camera.target.x = (grid_x * GRID_SIZE).to_f
  camera.target.y = (grid_y * GRID_SIZE).to_f # Positive y goes up

  # Start drawing
  BeginDrawing()
  ClearBackground(RAYWHITE)

  BeginMode2D(camera)

  # Draw grid lines
  (-20..20).each do |x|
    DrawLine(
      (x * GRID_SIZE).to_i, -640,
      (x * GRID_SIZE).to_i, 640,
      Fade(GRAY, 0.5)
    )
  end

  (-20..20).each do |y|
    DrawLine(
      -640, (y * GRID_SIZE).to_i,
      640, (y * GRID_SIZE).to_i,
      Fade(GRAY, 0.5)
    )
  end

  # Determine facing direction
  texture = if move_x > 0
              texture_east
            elsif move_x < 0
              texture_west
            elsif move_y > 0
              texture_north
            elsif move_y < 0
              texture_south
            else
              texture_east # Default texture
            end

  # Draw mascot at grid position
  DrawTexture(
    texture,
    (grid_x * GRID_SIZE).to_i - (texture.width / 2),
    (grid_y * GRID_SIZE).to_i - (texture.height / 2),
    WHITE
  )

  EndMode2D()

  # HUD display
  DrawText('Hello, World!', (SCREEN_WIDTH / 2) - 100, (SCREEN_HEIGHT / 2) - 50, 20, BLUE)
  DrawText('[Home] to exit.', 10, 10, 20, ruby_red)
  DrawText('[End] to toggle fullscreen.', 10, 40, 20, ruby_red)
  DrawText('[Insert] to toggle window decoration.', 10, 70, 20, ruby_red)
  DrawText('[C] to center camera.', 10, 100, 20, ruby_red)

  DrawFPS(SCREEN_WIDTH - 100, 10)

  EndDrawing()
end

# Clean up resources
UnloadTexture(texture_west)
UnloadTexture(texture_east)
UnloadTexture(texture_north)
UnloadTexture(texture_south)

UnloadImage(mascot_west_image)
UnloadImage(mascot_east_image)
UnloadImage(mascot_north_image)
UnloadImage(mascot_south_image)

CloseWindow()

'exit gameloop success'
