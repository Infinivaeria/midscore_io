def num_num(char)
  letters = {
    "a" => 1, "b" => 2, "c" => 3, "d" => 4, "e" => 5, "f" => 6,
    "g" => 7, "h" => 8, "i" => 9, "j" => 1, "k" => 2, "l" => 3,
    "m" => 4, "n" => 5, "o" => 6, "p" => 7, "q" => 8, "r" => 9,
    "s" => 1, "t" => 2, "u" => 3, "v" => 4, "w" => 5, "x" => 6,
    "y" => 7, "z" => 8
  }
  letters[char]
end

def mczv(y_string)
  n = Math.sqrt(y_string.length).to_i
  raise "Input string length must be a perfect square" unless n * n == y_string.length

  matrix = Array.new(n) { Array.new(n) }

  y_string.chars.each_with_index do |char, idx|
    row = idx / n
    col = idx % n
    matrix[row][col] = num_num(char)
  end

  # Print the resulting matrix
  matrix.each { |row| puts row.join(" ") }
end

# Input: y-sized string (e.g., "wolfmoonbolt")
print "Enter the y-sized string (letters only): "
y_string = gets.chomp
mczv(y_string)
