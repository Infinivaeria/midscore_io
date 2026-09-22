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

SetTargetFPS(60)

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
# Load textures for different directions
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

# Define a grid system
GRID_SIZE = 32

# Convert position to grid coordinates
def to_grid(x, y)
  [(x / GRID_SIZE).floor, (y / GRID_SIZE).floor]
end

# Convert grid coordinates to position
def to_position(grid_x, grid_y)
  [grid_x * GRID_SIZE, grid_y * GRID_SIZE]
end

# Initialize mascot position in grid coordinates
grid_x, grid_y = to_grid(pos_x, pos_y)

# Update camera target to center on the mascot
camera[:offset][:x] = ((screen_width / 2) - (mascot.width / 2))
camera[:offset][:y] = ((screen_height / 2) - (mascot.height / 2))
camera[:target][:x], camera[:target][:y] = to_position(grid_x, grid_y)
camera[:rotation] = BigDecimal('0.0')
camera[:zoom] = BigDecimal('1.0')

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
  # Update mascot position based on input (example)
  if IsKeyPressed(KEY_RIGHT)
    grid_x += 1
  elsif IsKeyPressed(KEY_LEFT)
    grid_x -= 1
  elsif IsKeyPressed(KEY_UP)
    grid_y -= 1
  elsif IsKeyPressed(KEY_DOWN)
    grid_y += 1
  end

  # Update camera target to follow the mascot
  camera[:target][:x], camera[:target][:y] = to_position(grid_x, grid_y)

  # Draw the scene
  BeginDrawing()
  ClearBackground(RAYWHITE)

  BeginMode2D(camera)
  DrawTexture(texture_west, *to_position(grid_x, grid_y), WHITE)
  EndMode2D()

  EndDrawing()
end




UnloadTexture(mascot)
CloseWindow()

'exit gameloop success'
