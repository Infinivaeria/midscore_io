def lagrange_four_squares(n)
  # Initialize the squares array with zeros
  squares = [0, 0, 0, 0]

  # First check if n is a perfect square
  if Math.sqrt(n) % 1 == 0
    squares[0] = Math.sqrt(n).to_i
    return squares.join('² + ') + '²'
  end

  # Check if n can be expressed as the sum of two squares
  (1..Math.sqrt(n).to_i).each do |i|
    next unless Math.sqrt(n - i * i) % 1 == 0

    squares[0] = i
    squares[1] = Math.sqrt(n - i * i).to_i
    return squares.join('² + ') + '²'
  end

  # Check if n can be expressed as the sum of three squares
  (1..Math.sqrt(n).to_i).each do |i|
    (1..Math.sqrt(n - i * i).to_i).each do |j|
      next unless Math.sqrt(n - i * i - j * j) % 1 == 0

      squares[0] = i
      squares[1] = j
      squares[2] = Math.sqrt(n - i * i - j * j).to_i
      return squares.join('² + ') + '²'
    end
  end

  # If n is not expressible as the sum of two or three squares,
  # use the four-square theorem to find the four squares
  (1..Math.sqrt(n).to_i).each do |i|
    (1..Math.sqrt(n - i * i).to_i).each do |j|
      (1..Math.sqrt(n - i * i - j * j).to_i).each do |k|
        next unless Math.sqrt(n - i * i - j * j - k * k) % 1 == 0

        squares[0] = i
        squares[1] = j
        squares[2] = k
        squares[3] = Math.sqrt(n - i * i - j * j - k * k).to_i
        return squares.join('² + ') + '²'
      end
    end
  end

  # If no combination is found, return an empty string
  ''
end

# Example usage:
loop do
  puts 'Enter a Natural Number (or press "q" to quit): '
  input = gets.chomp

  break if input.downcase == 'q'

  number = input.to_i.abs
  puts "The number #{number} can be expressed as the sum of four squares: " + lagrange_four_squares(number)
end
