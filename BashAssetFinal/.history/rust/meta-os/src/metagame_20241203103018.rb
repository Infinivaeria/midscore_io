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

# Constants
GRID_SIZE = 32
SCREEN_WIDTH = 800
SCREEN_HEIGHT = 600

# Coordinate system abstraction
class GridCoordinate
  attr_reader :grid_x, :grid_y, :pixel_x, :pixel_y

  def initialize(grid_x, grid_y)
    @grid_x = grid_x
    @grid_y = grid_y
    @pixel_x = grid_to_pixel(grid_x)
    @pixel_y = grid_to_pixel(grid_y)
  end

  def self.from_pixels(pixel_x, pixel_y)
    new(pixel_to_grid(pixel_x), pixel_to_grid(pixel_y))
  end

  private

  def self.pixel_to_grid(pixel)
    (pixel / GRID_SIZE).floor
  end

  def grid_to_pixel(grid)
    grid * GRID_SIZE
  end
end

# Camera handling with grid alignment
class GridCamera
  attr_reader :camera

  def initialize
    @camera = Camera2D.new
    reset_camera
  end

  def reset_camera
    @camera[:offset][:x] = SCREEN_WIDTH / 2.0
    @camera[:offset][:y] = SCREEN_HEIGHT / 2.0
    @camera[:rotation] = 0.0
    @camera[:zoom] = 1.0
  end

  def follow(grid_coordinate)
    @camera[:target][:x] = grid_coordinate.pixel_x
    @camera[:target][:y] = grid_coordinate.pixel_y
  end
end

# Game state management
class GameState
  attr_reader :position, :camera, :textures

  def initialize
    @position = GridCoordinate.new(0, 0)
    @camera = GridCamera.new
    load_textures
    init_database
  end

  def load_textures
    @textures = {
      west: LoadTextureFromImage(mascot_west),
      north: LoadTextureFromImage(mascot_north),
      east: LoadTextureFromImage(mascot_east),
      south: LoadTextureFromImage(mascot_south)
    }
  end

  def init_database
    @db_manager = FileMethodsManager.new
    @db_manager.make_db
    @db_manager.write_line('test')
    @db_manager.write_line('test_database')
    @db_manager.write_line('test_table')

    @line_db = LineDatabaseFactory.line_db
    @line_db['test'].pad.new_table!(
      database_name: 'test_database',
      database_table: 'test_table'
    )
    @line_db['test'].pad['test_database', 'test_table'].save_everything_to_files!
  end

  def move(dx, dy)
    @position = GridCoordinate.new(@position.grid_x + dx, @position.grid_y + dy)
    @camera.follow(@position)
  end

  def update
    if IsKeyPressed(KEY_RIGHT)
      move(1, 0)
    elsif IsKeyPressed(KEY_LEFT)
      move(-1, 0)
    elsif IsKeyPressed(KEY_UP)
      move(0, -1)
    elsif IsKeyPressed(KEY_DOWN)
      move(0, 1)
    end
  end

  def draw
    BeginDrawing()
    ClearBackground(RAYWHITE)

    BeginMode2D(@camera.camera)

    # Draw grid (optional, for debugging)
    draw_grid

    # Draw mascot
    DrawTexture(
      @textures[:west],
      @position.pixel_x,
      @position.pixel_y,
      WHITE
    )

    EndMode2D()
    EndDrawing()
  end

  private

  def draw_grid
    (-10..10).each do |x|
      (-10..10).each do |y|
        DrawRectangleLines(
          x * GRID_SIZE,
          y * GRID_SIZE,
          GRID_SIZE,
          GRID_SIZE,
          GRAY
        )
      end
    end
  end
end

# Main game loop
game = GameState.new

until WindowShouldClose()
  game.update
  game.draw
end
