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
require 'fileutils'

# Constants for 3D setup
QUBIT_SIZE = 99.0
ROTATION_SPEED = 150.0

# Draw a 3d Qubit
# DrawQubit(Vector3.create(0.0, 0.0, 0.0), 1.0, 1.0, 1.0, ruby_red)
def DrawQubit(position, width, height, depth, color) # rubocop:disable Naming/MethodName
  DrawCube(position, width, height, depth, color)
  DrawCubeWires(position, width, height, depth, BLACK)
end

# A note on coordinate systems and unit circles in raylib, where the y-axis is inverted,
#
# In raylib, as in most computer graphics libraries, the coordinate system is such that the origin (0, 0) is at the top-left corner of the screen. The x-axis increases to the right, and the y-axis increases downward. This is different from the traditional Cartesian coordinate system used in mathematics, where the y-axis increases upward.
#
# In Trigonometry and with degrees and radians, there are counter-clockwise and clockwise rotations; clockwise rotations are considered negative degrees/radians.
#
# Unit Circle in raylib Coordinate System
# In the raylib coordinate system:
#
# The positive x-axis points to the right.
# The positive y-axis points downward.
# The negative x-axis points to the left.
# The negative y-axis points upward.
# Visual Representation of the Unit Circle in raylib
#
#        (-1, -1)       (0, -1)       (1, -1)
#           +-------------+-------------+
#           |             |             |
#           |             |             |
# (-1, 0)---+-------------+-------------+---(1, 0)
#           |             |             |
#           |             |             |
#           +-------------+-------------+
#        (-1, 1)        (0, 1)        (1, 1)
#
# Rotations on the Unit Circle in raylib
# 0 degrees (0 radians): Positive x-axis (right)
# 90 degrees (π/2 radians): Positive y-axis (downward)
# 180 degrees (π radians): Negative x-axis (left)
# 270 degrees (3π/2 radians): Negative y-axis (upward)
# 360 degrees (2π radians): Completes the circle back to the positive x-axis

DEBUG_MODE = true

def debug(msg)
  puts msg if DEBUG_MODE == true
end

shared_lib_path = Gem::Specification.find_by_name('raylib-bindings').full_gem_path + '/lib/'

case RUBY_PLATFORM
when /mswin|msys|mingw|cygwin/

  Raylib.load_lib(shared_lib_path + 'libraylib.dll', raygui_libpath: shared_lib_path + 'raygui.dll',
                                                     physac_libpath: shared_lib_path + 'physac.dll')
when /darwin/
  arch = RUBY_PLATFORM.split('-')[0]
  Raylib.load_lib(shared_lib_path + "libraylib.#{arch}.dylib",
                  raygui_libpath: shared_lib_path + "raygui.#{arch}.dylib", physac_libpath: shared_lib_path + "physac.#{arch}.dylib")
when /linux/
  arch = RUBY_PLATFORM.split('-')[0]
  Raylib.load_lib(shared_lib_path + "libraylib.#{arch}.so", raygui_libpath: shared_lib_path + "raygui.#{arch}.so",
                                                            physac_libpath: shared_lib_path + "physac.#{arch}.so")
else
  raise "Unknown OS: #{RUBY_PLATFORM}"
end

##

include Raylib

FULL_SCREEN = true
NO_ESC_QUIT = 0

monitor = GetCurrentMonitor()
screen_width = GetMonitorWidth(monitor)
screen_height = GetMonitorHeight(monitor)

# Enable Vertical Sync (Vsync)
SetConfigFlags(FLAG_VSYNC_HINT)
InitWindow(screen_width, screen_height, '"The Meta-Game" :: 🚂📜BashAsset_engine BUILD v0.1.1')
SetWindowState(FLAG_WINDOW_UNDECORATED)
SetExitKey(KEY_HOME) # Disables the ESC key from closing the window

screen_width = GetScreenWidth()
screen_height = GetScreenHeight()

SetTargetFPS(1024)
decorated_window = true
full_screen_mode = true

ruby_red = Color.from_u8(155, 17, 30, 255)

LEFT = 0x8
RIGHT = 0x4
UP = 0x2
DOWN = 0x1

pos_x = BigDecimal('0.0')
pos_y = BigDecimal('0.0')

mascot = LoadTexture('./assets/mascot.png')

mascot_west ||= LoadImage('./assets/mascot.png')
mascot_north ||= LoadImage('./assets/mascot.png')
mascot_east ||= LoadImage('./assets/mascot.png')
mascot_south ||= LoadImage('./assets/mascot.png')

ImageFlipHorizontal(mascot_west)
ImageRotate(mascot_north, BigDecimal('270.0')) # -90.0
ImageRotate(mascot_south, BigDecimal('90.0'))  # -270.0

texture_west = LoadTextureFromImage(mascot_west)
texture_north = LoadTextureFromImage(mascot_north)
texture_east = LoadTextureFromImage(mascot_east)
texture_south = LoadTextureFromImage(mascot_south)

# Initialize the camera
camera = Camera2D.new
camera[:offset][:x] = 0.0
camera[:offset][:y] = 0.0
camera[:target][:x] = pos_x
camera[:target][:y] = pos_y
camera[:rotation] = 0.0
camera[:zoom] = 1.0

tick_counter = 0.0
esc_timer = 0.0

def x_squared(x)
  x * x
end

camera[:offset][:x] = ((screen_width / 2) - (mascot.width / 2))
camera[:offset][:y] = ((screen_height / 2) - (mascot.height / 2))
camera[:target][:x] = pos_x
camera[:target][:y] = pos_y
camera[:rotation] = BigDecimal('0.0')
camera[:zoom] = BigDecimal('1.0')

camera3d = Camera3D.new
camera3d[:position] = Vector3.create(10.0, 10.0, 10.0)
camera3d[:target] = Vector3.create(0.0, 0.0, 0.0)
camera3d[:up] = Vector3.create(0.0, 1.0, 0.0)
camera3d[:fovy] = 45.0
camera3d[:projection] = CAMERA_PERSPECTIVE

# Qubit rendering function
def draw_qubit(position, size, rotation, delta_time: Time.now.to_i)
  # Base cube
  DrawCube(position, size, size, size, RED)
  DrawCubeWires(position, size, size, size, BLACK)

  # Transform positions for quantum state indicators
  up_sphere = Vector3.create(
    position[:x] + (Math.sin(rotation) * size),
    position[:y] + size,
    position[:z] + (Math.cos(rotation) * size)
  )

  Vector3.create(
    position[:x] - (Math.sin(rotation) * size),
    position[:y] - size,
    position[:z] - (Math.cos(rotation) * size)
  )

  # Draw quantum state indicators
  DrawSphere(up_sphere, size * 0.2, BLUE)
  DrawSphere(down_sphere, size * 0.2, GREEN)
end

# Partitioned Array/LineDB Database

db_manager = FileMethodsManager.new
db_manager.make_db
db_manager.write_line('test')
db_manager.write_line('test_database')
db_manager.write_line('test_table')

puts 'created db directory and list'
line_db = LineDatabaseFactory.line_db

line_db['test'].pad.new_table!(database_name: 'test_database', database_table: 'test_table')
line_db['test'].pad['test_database', 'test_table'].save_everything_to_files!

# Main game loop
until WindowShouldClose()

  screen_width = GetScreenWidth()
  screen_height = GetScreenHeight()

  # Calculate qubit rotation
  qubit_rotation = (GetTime() * ROTATION_SPEED).to_f 

  if IsKeyPressed(KEY_END)
    if full_screen_mode
      full_screen_mode = !full_screen_mode
      ClearWindowState(FLAG_FULLSCREEN_MODE)
      puts 'Setting full screen mode'
      screen_width = GetMonitorWidth(monitor)
      screen_height = GetMonitorHeight(monitor)
      puts "Screen width: #{screen_width}, Screen height: #{screen_height}"
      SetWindowSize(screen_width, screen_height)
      SetWindowState(FLAG_FULLSCREEN_MODE)
    else
      full_screen_mode = !full_screen_mode
      ClearWindowState(FLAG_FULLSCREEN_MODE)
      SetWindowSize(1920, 1080)
      puts 'Setting windowed mode'
      puts 'DEFAULT to 1080p (resizable)'
      SetWindowState(FLAG_WINDOW_RESIZABLE)
    end
  end

  if IsKeyPressed(KEY_INSERT)
    if decorated_window
      SetWindowState(FLAG_WINDOW_UNDECORATED)
      decorated_window = !decorated_window
    else
      ClearWindowState(FLAG_WINDOW_UNDECORATED)
      decorated_window = !decorated_window
    end
  end

  delta_time = BigDecimal(GetFrameTime().to_s)
  tick_counter += delta_time

  if IsKeyDown(KEY_ESCAPE)
    esc_timer ||= 0
    esc_timer += delta_time
    if esc_timer >= 5.0
      CloseWindow()
      exit
    end
  else
    esc_timer = 0
  end

  # Adjust camera zoom with mouse wheel
  if GetMouseWheelMove() != 0
    if GetMouseWheelMove() >= 0
      camera[:zoom] += BigDecimal('0.01')
      camera[:zoom] = camera[:zoom].clamp(BigDecimal('0.01'), BigDecimal('4.0'))

    else
      camera[:zoom] -= BigDecimal('0.01')
      camera[:zoom] = camera[:zoom].clamp(BigDecimal('0.01'), BigDecimal('4.0'))

    end
  end

  # center the camera around pos_x and pos_y

  # Update camera position based on arrow keys
  dirbits_camera = 0
  dirbits_camera |= RIGHT if IsKeyDown(KEY_RIGHT)
  dirbits_camera |= LEFT if IsKeyDown(KEY_LEFT)
  dirbits_camera |= UP if IsKeyDown(KEY_UP)
  dirbits_camera |= DOWN if IsKeyDown(KEY_DOWN)
  camera_delta = BigDecimal('100.0')

  case dirbits_camera
  when RIGHT # correct
    camera[:target][:x] -= BigDecimal(camera_delta.to_s)
  when LEFT # correct
    camera[:target][:x] += BigDecimal(camera_delta.to_s)
  when UP # correct
    camera[:target][:y] += BigDecimal(camera_delta.to_s)
  when DOWN # correct
    camera[:target][:y] -= BigDecimal(camera_delta.to_s)
  when RIGHT | UP # correct
    camera[:target][:x] -= BigDecimal(camera_delta.to_s)
    camera[:target][:y] += BigDecimal(camera_delta.to_s)
  when RIGHT | DOWN # correct
    camera[:target][:x] -= BigDecimal(camera_delta.to_s)
    camera[:target][:y] -= BigDecimal(camera_delta.to_s)
  when LEFT | UP
    camera[:target][:x] -= BigDecimal(camera_delta.to_s)
    camera[:target][:y] += BigDecimal(camera_delta.to_s)
  when LEFT | DOWN
    camera[:target][:x] += BigDecimal(camera_delta.to_s)
    camera[:target][:y] -= BigDecimal(camera_delta.to_s)
  end

  if IsKeyDown(KEY_C)
    camera[:offset] = Vector2.create((screen_width / 2) - (mascot.width / 2), (screen_height / 2) - (mascot.height / 2))
    camera[:target][:x] = pos_x
    camera[:target][:y] = pos_y
    camera[:rotation] = BigDecimal('0.0')
    camera[:zoom] = BigDecimal('1.0')
  end

  # Rendering phase
  BeginDrawing()
  ClearBackground(GRAY)

  # In main game loop:
  BeginMode3D(camera3d)
  DrawGrid(10, 1.0)
  draw_qubit(
    Vector3.create(0.0, 0.0, 0.0),
    QUBIT_SIZE,
    qubit_rotation,
    
  )
  EndMode3D()

  BeginMode2D(camera)

  movement_chunk ||= BigDecimal('200.0')

  dirbits = 0
  dirbits |= RIGHT if IsKeyDown(KEY_D)
  dirbits |= LEFT if IsKeyDown(KEY_A)
  dirbits |= UP if IsKeyDown(KEY_W)
  dirbits |= DOWN if IsKeyDown(KEY_S)

  case dirbits
  when RIGHT
    pos_x += (movement_chunk * delta_time)

    DrawTexture(texture_east, pos_x, pos_y, WHITE)

  when LEFT
    pos_x -= (movement_chunk * delta_time)
    DrawTexture(texture_west, pos_x, pos_y, WHITE)

  when UP
    pos_y -= (movement_chunk * delta_time)
    DrawTexture(texture_north, pos_x, pos_y, WHITE)

  when DOWN
    pos_y += (movement_chunk * delta_time)
    DrawTexture(texture_south, pos_x, pos_y, WHITE)

  when RIGHT | UP
    pos_x += (movement_chunk * delta_time)
    pos_y -= (movement_chunk * delta_time)
    DrawTexture(texture_north, pos_x, pos_y, WHITE)

  when RIGHT | DOWN
    pos_x += (movement_chunk * delta_time)
    pos_y += (movement_chunk * delta_time)
    DrawTexture(texture_south, pos_x, pos_y, WHITE)

  when LEFT | UP
    pos_x -= (movement_chunk * delta_time)
    pos_y -= (movement_chunk * delta_time)
    DrawTexture(texture_north, pos_x, pos_y, WHITE)

  when LEFT | DOWN
    pos_x -= (movement_chunk * delta_time)
    pos_y += (movement_chunk * delta_time)
    DrawTexture(texture_south, pos_x, pos_y, WHITE)

  else
    DrawTexture(texture_east, pos_x, pos_y, WHITE)
  end
  EndMode2D()

  # HUD
  DrawText('Hello, World!', screen_width / 2, screen_height / 2, 100, BLUE)
  DrawText('[Home] to exit.', 10, 10, 30, ruby_red)
  DrawText('[End] to toggle fullscreen.', 10, 100, 30, ruby_red)
  DrawText('[Insert] to toggle window decoration.', 10, 130, 30, ruby_red)
  DrawText('[C] to center camera.', 10, 170, 30, ruby_red)

  DrawFPS(screen_width - 100, 16)

  EndDrawing()
end

UnloadTexture(mascot)
CloseWindow()

'exit gameloop success'
