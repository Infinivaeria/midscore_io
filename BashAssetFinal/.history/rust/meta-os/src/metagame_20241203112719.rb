# The Meta-Game Project

require 'raylib'
require 'json'
require 'uri'
require 'bigdecimal'
require 'fileutils'

# Draw a 3D Qubit
def DrawQubit(position, width, height, depth, color)
  DrawCube(position, width, height, depth, color)
  DrawCubeWires(position, width, height, depth, BLACK)
end

DEBUG_MODE = true

def debug(msg)
  puts msg if DEBUG_MODE
end

shared_lib_path = Gem::Specification.find_by_name('raylib-bindings').full_gem_path + '/lib/'

case RUBY_PLATFORM
when /mswin|msys|mingw|cygwin/
  Raylib.load_lib(
    shared_lib_path + 'libraylib.dll',
    raygui_libpath: shared_lib_path + 'raygui.dll',
    physac_libpath: shared_lib_path + 'physac.dll'
  )
when /darwin/
  arch = RUBY_PLATFORM.split('-')[0]
  Raylib.load_lib(
    shared_lib_path + "libraylib.#{arch}.dylib",
    raygui_libpath: shared_lib_path + "raygui.#{arch}.dylib",
    physac_libpath: shared_lib_path + "physac.#{arch}.dylib"
  )
when /linux/
  arch = RUBY_PLATFORM.split('-')[0]
  Raylib.load_lib(
    shared_lib_path + "libraylib.#{arch}.so",
    raygui_libpath: shared_lib_path + "raygui.#{arch}.so",
    physac_libpath: shared_lib_path + "physac.#{arch}.so"
  )
else
  raise "Unknown OS: #{RUBY_PLATFORM}"
end

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

GRID_SIZE = BigDecimal('32') # Grid size in pixels

# Initialize position in grid coordinates
grid_x = BigDecimal('0.0')
grid_y = BigDecimal('0.0')

# Movement speed (grid units per second)
movement_speed = BigDecimal('5.0')

mascot = LoadTexture('./assets/mascot.png')

mascot_west = LoadImage('./assets/mascot.png')
mascot_north = LoadImage('./assets/mascot.png')
mascot_east = LoadImage('./assets/mascot.png')
mascot_south = LoadImage('./assets/mascot.png')

ImageFlipHorizontal(mascot_west)
ImageRotate(mascot_north, 270.0) # -90.0
ImageRotate(mascot_south, 90.0)  # -270.0

texture_west = LoadTextureFromImage(mascot_west)
texture_north = LoadTextureFromImage(mascot_north)
texture_east = LoadTextureFromImage(mascot_east)
texture_south = LoadTextureFromImage(mascot_south)

# Initialize the camera with y-axis flipped
camera = Camera2D.new
camera.offset = Vector2.create(screen_width / 2.0, screen_height / 2.0)
camera.target = Vector2.create((grid_x * GRID_SIZE).to_f, -(grid_y * GRID_SIZE).to_f)
camera.rotation = 0.0
camera.zoom = 1.0

tick_counter = BigDecimal('0.0')
esc_timer = BigDecimal('0.0')

def x_squared(x)
  x * x
end

# Main game loop
until WindowShouldClose()

  screen_width = GetScreenWidth()
  screen_height = GetScreenHeight()

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
    esc_timer += delta_time
    if esc_timer >= BigDecimal('5.0')
      CloseWindow()
      exit
    end
  else
    esc_timer = BigDecimal('0.0')
  end

  # Adjust camera zoom with mouse wheel
  mouse_wheel_move = GetMouseWheelMove()
  if mouse_wheel_move != 0
    if mouse_wheel_move >= 0
      camera.zoom += 0.01
      camera.zoom = [camera.zoom, 4.0].min
    else
      camera.zoom -= 0.01
      camera.zoom = [camera.zoom, 0.01].max
    end
  end

  # Update camera position based on arrow keys
  dirbits_camera = 0
  dirbits_camera |= RIGHT if IsKeyDown(KEY_RIGHT)
  dirbits_camera |= LEFT if IsKeyDown(KEY_LEFT)
  dirbits_camera |= UP if IsKeyDown(KEY_UP)
  dirbits_camera |= DOWN if IsKeyDown(KEY_DOWN)
  camera_delta = BigDecimal('100.0') * delta_time

  case dirbits_camera
  when RIGHT
    camera.target.x += camera_delta.to_f
  when LEFT
    camera.target.x -= camera_delta.to_f
  when UP
    camera.target.y -= camera_delta.to_f
  when DOWN
    camera.target.y += camera_delta.to_f
  when RIGHT | UP
    camera.target.x += camera_delta.to_f
    camera.target.y -= camera_delta.to_f
  when RIGHT | DOWN
    camera.target.x += camera_delta.to_f
    camera.target.y += camera_delta.to_f
  when LEFT | UP
    camera.target.x -= camera_delta.to_f
    camera.target.y -= camera_delta.to_f
  when LEFT | DOWN
    camera.target.x -= camera_delta.to_f
    camera.target.y += camera_delta.to_f
  end

  if IsKeyDown(KEY_C)
    camera.offset = Vector2.create(
      (screen_width / 2) - (mascot.width / 2),
      (screen_height / 2) - (mascot.height / 2)
    )
    camera.target = Vector2.create((grid_x * GRID_SIZE).to_f, -(grid_y * GRID_SIZE).to_f)
    camera.rotation = 0.0
    camera.zoom = 1.0
  end

  # Movement input
  dirbits = 0
  dirbits |= RIGHT if IsKeyDown(KEY_D)
  dirbits |= LEFT if IsKeyDown(KEY_A)
  dirbits |= UP if IsKeyDown(KEY_W)
  dirbits |= DOWN if IsKeyDown(KEY_S)
  movement_chunk = BigDecimal('200.0') * delta_time

  case dirbits
  when RIGHT
    grid_x += (movement_chunk / GRID_SIZE)
    texture = texture_east
  when LEFT
    grid_x -= (movement_chunk / GRID_SIZE)
    texture = texture_west
  when UP
    grid_y += (movement_chunk / GRID_SIZE)
    texture = texture_north
  when DOWN
    grid_y -= (movement_chunk / GRID_SIZE)
    texture = texture_south
  when RIGHT | UP
    grid_x += (movement_chunk / GRID_SIZE)
    grid_y += (movement_chunk / GRID_SIZE)
    texture = texture_north
  when RIGHT | DOWN
    grid_x += (movement_chunk / GRID_SIZE)
    grid_y -= (movement_chunk / GRID_SIZE)
    texture = texture_south
  when LEFT | UP
    grid_x -= (movement_chunk / GRID_SIZE)
    grid_y += (movement_chunk / GRID_SIZE)
    texture = texture_north
  when LEFT | DOWN
    grid_x -= (movement_chunk / GRID_SIZE)
    grid_y -= (movement_chunk / GRID_SIZE)
    texture = texture_south
  else
    texture = texture_east
  end

  # Update camera target to follow the mascot
  camera.target = Vector2.create((grid_x * GRID_SIZE).to_f, -(grid_y * GRID_SIZE).to_f)

  # Rendering phase
  BeginDrawing()
  ClearBackground(GRAY)

  BeginMode2D(camera)

  # Draw grid lines
  (-20..20).each do |x|
    DrawLine(
      (x * GRID_SIZE).to_i, -1000,
      (x * GRID_SIZE).to_i, 1000,
      Fade(GRAY, 0.5)
    )
  end
  (-20..20).each do |y|
    DrawLine(
      -1000, -(y * GRID_SIZE).to_i,
      1000, -(y * GRID_SIZE).to_i,
      Fade(GRAY, 0.5)
    )
  end

  # Draw mascot at grid position
  DrawTexture(
    texture,
    (grid_x * GRID_SIZE).to_i - (mascot.width / 2),
    -(grid_y * GRID_SIZE).to_i - (mascot.height / 2),
    WHITE
  )

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
