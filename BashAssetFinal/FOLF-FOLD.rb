# Enochian Fold Ontological Mathematics Language (EFOML) Implementation in Ruby
# Description:
# This program implements EFOML, allowing encoding and decoding between English and EFOML shorthand.
# It includes definitions of the alphabet, numerals, mathematical symbols, and ontological concepts.

# Module for EFOML
module EFOML
  # Alphabet: Mapping between English letters/phonemes and EFOML symbols
  ALPHABET = {
    'A' => '⍺',
    'B' => '⍲',
    'C' => '⍳',
    'D' => '⍴',
    'E' => '⍵',
    'F' => '⍶',
    'G' => '⍷',
    'H' => '⍸',
    'I' => '⍹',
    'J' => '⍺⍺',
    'K' => '⍳⍳',
    'L' => '⍴⍴',
    'M' => '⍵⍵',
    'N' => '⍶⍶',
    'O' => '⍷⍷',
    'P' => '⍸⍸',
    'Q' => '⍹⍹',
    'R' => '⍺⍳',
    'S' => '⍺⍴',
    'T' => '⍺⍵',
    'U' => '⍺⍶',
    'V' => '⍺⍷',
    'W' => '⍺⍸',
    'X' => '⍺⍹',
    'Y' => '⍳⍺',
    'Z' => '⍴⍹',
    # Extended Phonemes
    'CH' => '⍸⍳',
    'SH' => '⍸⍴',
    'TH' => '⍸⍵',
    'NG' => '⍸⍶',
    'PH' => '⍸⍷',
    'WH' => '⍺⍸⍸',
    'KN' => '⍳⍶',
    'GH' => '⍷⍸',
    'AE' => '⍶⍺',
    'OE' => '⍷⍺',
  }

  # Numerals: Mapping between numbers and EFOML numeral symbols
  NUMERALS = {
    0 => '⬦',
    1 => '⬧',
    2 => '⬨',
    3 => '⬩',
    4 => '⬪',
    5 => '⬫',
    6 => '⬬',
    7 => '⬭',
    8 => '⬮',
    9 => '⬯',
    10 => '⭐',
    11 => '⭑',
    12 => '⭒',
    13 => '✶',
    14 => '✷',
    15 => '✸',
    16 => '✹',
    17 => '✺',
    18 => '✻',
    19 => '✼',
    20 => '✽',
    21 => '❂',
    22 => '❃',
    23 => '❄',
    24 => '❅',
    25 => '❆',
    26 => '✙',
    27 => '✚',
    28 => '✛',
    29 => '✜',
    30 => '✝',
    31 => '✞',
    32 => '✟',
    33 => '✠',
    34 => '⛨',
    35 => '⛧',
    36 => '⛦',
    37 => '⚕',
    38 => '☤',
    39 => '⚚',
    40 => '⚖',
    41 => '☯',
    42 => '☮',
    43 => '♾',
    44 => '⚛',
  }

  # Mathematical Symbols
  MATH_SYMBOLS = {
    'FOLD_EFFECT' => '⊿',  # Delta symbol representing the Fold Effect
    'SUMMATION' => '∑',
    'INFINITY' => '∞',
    'ZERO' => '0',          # Can also use 'Ø' if desired
  }

  # Helper method to match extended phonemes in a word
  def self.match_phonemes(word)
    phonemes = ALPHABET.keys.select { |k| k.length > 1 }
    regex = Regexp.union(phonemes)
    word.upcase.scan(regex)
  end

  # Encode English text to EFOML shorthand
  def self.encode(text)
    words = text.strip.split(/\s+/)
    encoded_words = words.map do |word|
      # Initialize index and encoded word
      index = 0
      encoded_word = ''

      while index < word.length
        # Try to match extended phonemes first
        match = nil
        ALPHABET.keys.select { |k| k.length > 1 }.each do |phoneme|
          if word[index, phoneme.length].upcase == phoneme
            match = phoneme
            break
          end
        end

        # If extended phoneme matched
        if match
          encoded_word += ALPHABET[match]
          index += match.length
        else
          # Match single character
          char = word[index].upcase
          if ALPHABET.key?(char)
            encoded_word += ALPHABET[char]
          elsif NUMERALS.key?(char.to_i)
            # Encode numbers if present in the text
            encoded_word += NUMERALS[char.to_i]
          else
            # Keep character as is if not in ALPHABET or NUMERALS (e.g., punctuation)
            encoded_word += word[index]
          end
          index += 1
        end
      end

      encoded_word
    end

    encoded_words.join(' ')
  end

  # Decode EFOML shorthand to English text
  def self.decode(shorthand)
    # Build reverse mapping for decoding
    reverse_alphabet = ALPHABET.invert
    reverse_numerals = NUMERALS.invert
    # Sort EFOML symbols by length in descending order to match longest symbols first
    efoml_symbols = reverse_alphabet.keys.sort_by { |s| -s.length }

    words = shorthand.strip.split(/\s+/)
    decoded_words = words.map do |word|
      index = 0
      decoded_word = ''

      while index < word.length
        match = nil
        symbol = nil

        # Check for numerals first
        NUMERALS.values.each do |num_sym|
          if word[index, num_sym.length] == num_sym
            match = reverse_numerals[num_sym]
            decoded_word += match.to_s
            index += num_sym.length
            break
          end
        end

        next if match

        # Check for alphabet symbols
        efoml_symbols.each do |alph_sym|
          if word[index, alph_sym.length] == alph_sym
            symbol = alph_sym
            break
          end
        end

        if symbol
          decoded_word += reverse_alphabet[symbol]
          index += symbol.length
        else
          # Keep character as is if not in EFOML symbols (e.g., punctuation)
          decoded_word += word[index]
          index += 1
        end
      end

      decoded_word.capitalize
    end

    decoded_words.join(' ')
  end

  # Encode numbers to EFOML numeral symbols
  def self.encode_number(number)
    NUMERALS[number] || number.to_s
  end

  # Decode EFOML numeral symbols to numbers
  def self.decode_number(symbol)
    NUMERALS.key(symbol) || symbol.to_s
  end

  # Example method to demonstrate encoding and decoding
  def self.example_usage
    puts "EFOML Example Usage:\n\n"

    # Example text
    text = "HELLO, WORLD!"
    puts "Original Text: #{text}"

    # Encode text
    encoded_text = encode(text)
    puts "Encoded Text: #{encoded_text}"

    # Decode text
    decoded_text = decode(encoded_text)
    puts "Decoded Text: #{decoded_text}\n\n"

    # Example mathematical expression
    expression = "The Fold Effect reveals hidden layers."
    puts "Original Expression: #{expression}"

    # Encode expression
    encoded_expression = encode(expression.gsub('Fold Effect', MATH_SYMBOLS['FOLD_EFFECT']))
    puts "Encoded Expression: #{encoded_expression}"

    # Decode expression
    decoded_expression = decode(encoded_expression.gsub(MATH_SYMBOLS['FOLD_EFFECT'], '⊿'))
    puts "Decoded Expression: #{decoded_expression.gsub('⊿', 'Fold Effect')}\n\n"

    # Encode number
    number = 42
    encoded_number = encode_number(number)
    puts "Original Number: #{number}"
    puts "Encoded Number: #{encoded_number}"

    # Decode number
    decoded_number = decode_number(encoded_number)
    puts "Decoded Number: #{decoded_number}\n\n"

    # Example using Fold Effect symbol in a sentence
    sentence = "By applying the Fold Effect, we uncover deeper truths."
    puts "Original Sentence: #{sentence}"

    # Encode sentence
    encoded_sentence = encode(sentence.gsub('Fold Effect', MATH_SYMBOLS['FOLD_EFFECT']))
    puts "Encoded Sentence: #{encoded_sentence}"

    # Decode sentence
    decoded_sentence = decode(encoded_sentence.gsub(MATH_SYMBOLS['FOLD_EFFECT'], '⊿'))
    puts "Decoded Sentence: #{decoded_sentence.gsub('⊿', 'Fold Effect')}"
  end
end

# Run the example usage
EFOML.example_usage
