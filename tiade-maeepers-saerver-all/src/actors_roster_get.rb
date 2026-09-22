#!/usr/bin/env ruby
# actors_pairs_hardcoded.rb
# Usage:
#   ruby actors_pairs_hardcoded.rb --pair "Duke"
#   ruby actors_pairs_hardcoded.rb --random 6
#   ruby actors_pairs_hardcoded.rb --random-pairs 3

require 'optparse'
require 'securerandom'

# Hardcoded roster as provided by the user.
# Each entry is a two-element array [left_side_string, right_side_string].
PAIRS = [
  ['Duke: ArityWolf :: AydenAardWolf/AxelShep', 'Luke: KintyWolf :: LukeReinhard/RhuxShep'],
  ['TIDE', 'MEEPERS'],
  ['Protheorem: YeenStank', 'Theorempro: StankYeen'],
  ['Spacey: SpaceYeen', 'Meek: LightYeen'],
  ['PsiCorpVoxel :: RistWolf', 'CorpPsiVoxel:: IsoWolf'],
  ['ReefRuff', 'SpeakEasy'],
  ['SkuruWolf', 'WolfSkuru'],
  ['MontBarque', 'DaFoxy'],
  ['StanleyBorzoi', 'BorzoiStanley'],
  ['DogNorf', 'NorfDog'],
  ['TamaYote', 'YoteTama'],
  ['MidnightSix3', '3SixMidnight'],
  ['MasterOkami', 'Vann (Vivanne)'],
  ['PredYeen', 'Chatai'],
  ['CoreyCoyote', 'Kusaki'],
  ['MerleKoz', 'KozMerle'],
  ['WastelandCanid', 'JordiFox'],
  ['Esaio', 'Osaio'],
  ['Manzsters', 'MtKanjon'],
  ['NIIC', 'RocketFocksYT'],
  ['ThatDogCoda', 'CodaThatDog'],
  ['Setsu', 'TonTonah'],
  ['GrimmrFade', 'Communist_Industry (Fox)'],
  ['SnowWaveWolf', 'NUBBS'],
  ['CorbinWusky', 'Oku:: CaptGrowlarBear'],
  ['FaceLessPupper', 'RearSilver'],
  ['ShadowWolf :: NickRochefort', 'ShadeWolf :: RocheFortNick'],
  ['EsiWolf', 'OsiWolf'],
  ['ThatWolfKoda', 'KodaThatWolf'],
  ['MistyTheCanine', 'TheMistyCanine'],
  ['WTFolf :: WrenchTattooWolf :: AndrewXLR', 'LusuLuju ::  PreyYeen :: XLRAndrew'],
  ['IanFox', 'IaenFox'],
  ['FlukeHusky', 'ArtemisWishfoot'],
  ['VinceWuff :: ArityWolf2', 'AussieRigby :: AustralianChef Board (ACB)'],
  ['CosmicWuffy', 'Xuen'],
  ['AardWolfEssex', 'HuskyPupGaming'],
  ['MangoFoxy', 'PureKoor :: Tiger/Raccoon Hybrid'],
  ['ChanceMoonRay :; Wolf', 'Vulpe :: Grey Fox'],
  ['WhiskeyDingo', 'SkaiFox'],
  ['Yxillo :: Hyena', 'JTWusky'],
  ['Juiceps', 'Andre_Moraph'],
  ['superTULER', 'Kajohtie'],
  ["Tirox :: Fluke's Boyfriend :: TigerFox", 'GenericDefault :: West :: Coyote'],
  ["Mattel :: MerleKoz's bf :: Gray Wolf :: ArityWolf3", 'BigNuggetBox :: Spotted/Striped Hyena'],
  ['LuckFox033 :: AyloFolf2', 'RaxxVR'],
  ['MyntyPaws :: Snep', 'Nukorio :: folf'],
  ['AdenWolf', 'AdaenWolf'],
  ['FrostOwOBone', 'WolfWill'],
  ['Scrub :: ScrubbyFox', 'Lykey'],
  ['GhostFawx', 'BaldurWoof'],
  ['Nova :: Malamute', 'Rocky :: GoldenRetriever'],
  ['Ranger :: Akita', 'River :: GermanShepherd'],
  ['EzraCoyote', 'EzraenCoyote'],
  ['Snoww_Boy', 'Tako_Wolf'],
  ['Veles :: Malamute', 'Geri :: Golden Retriever mix, etc'],
  ['Hati :: Akita Mix', 'Arktos :: German Shepherd'],
  ['Ailini', 'Ailiny'],

  # Dogs
  ['DivaBlackStar', 'DovaBlackstar'],
  ['BowTheBad', 'BadTheBow'],

  # Contactees (store as a single left/right pair for convenience)
  [
    'Contactees: The Other Spiritualists; The Other Wild Animals; The Musicians; The AAA [Indie[ Game Developers; The Corpos (CEOs, Linux/Linus Torvalds/OSS, etc); The Other People (musicians, actors, AAA game developers (AAA indie)); Our Ancestors', 'Contactees: (mirror)'
  ],

  # Fairy Realm / LightBodies / TulpaSpace
  ['Kejento{1..120}', 'NovaFox{1..120}'],
  ['LycanRoc :: SnoutLocks{1..120}', 'Arcanine :: Muskdrips{1..120}'],

  # Extra note
  ['x Aylon Arlon Ayrlon', '(note)']
].freeze

# Build a flattened list of individual names by splitting each side on common separators.
def extract_individual_names(pairs)
  names = []
  pairs.each do |left, right|
    [left, right].each do |side|
      # Remove surrounding parentheses and trim
      s = side.to_s.gsub(/^\(|\)$/, '').strip
      # Split on semicolons, commas, double colons, slashes, '::', '::', ' / ', ' :: ', ' - ', '(', ')'
      parts = s.split(%r{;|,|\s::\s|::|/|\s/\s|\s-\s|\(|\)|\.\.\.| \.\.| \.\.| \.\.\.|\s\|\s})
      parts.each do |p|
        p = p.strip
        next if p.empty?

        # Keep compound tokens like "Luke: KintyWolf" intact, but also split "Duke: ArityWolf :: AydenAardWolf/AxelShep" into tokens
        # Further split on colon if it separates a label from a name
        if p.include?(':')
          label, rest = p.split(':', 2).map(&:strip)
          # include both label and rest if they look like names
          names << label unless label.empty?
          # split rest on slashes
          rest.split('/').map(&:strip).each { |r| names << r unless r.empty? }
        else
          p.split('/').map(&:strip).each { |r| names << r unless r.empty? }
        end
      end
    end
  end
  names.map(&:strip).uniq
end

NAMES = extract_individual_names(PAIRS).freeze

# Find the pair (left/right) that contains the query string (case-insensitive substring match).
def find_pair_for(query)
  q = query.to_s.strip.downcase
  PAIRS.each do |left, right|
    return [left, right] if left.downcase.include?(q) || right.downcase.include?(q)
  end
  nil
end

# Print a pair in the "A <-> B" format
def print_pair(a, b)
  puts "#{a} <-> #{b}"
end

# Return x random unique PAIRS entries.
def random_pairs(x)
  x = x.to_i
  raise ArgumentError, 'x must be a positive integer' if x <= 0
  raise ArgumentError, "x (#{x}) is larger than pair roster size (#{PAIRS.size})" if x > PAIRS.size

  PAIRS.sample(x, random: SecureRandom)
end

# Return x random unique names from the flattened NAMES list joined by " <-> "
def random_chain(x)
  x = x.to_i
  raise ArgumentError, 'x must be a positive integer' if x <= 0
  raise ArgumentError, "x (#{x}) is larger than roster size (#{NAMES.size})" if x > NAMES.size

  chosen = NAMES.sample(x, random: SecureRandom)
  chosen.join(' <-> ')
end

# CLI parsing
options = {}
OptionParser.new do |opts|
  opts.banner = 'Usage: actors_pairs_hardcoded.rb [options]'

  opts.on('--pair NAME', 'Find and print the twin pair or group containing NAME') do |name|
    options[:pair] = name
  end

  opts.on('--random N', Integer, "Print a chain of N random names joined by '<->'") do |n|
    options[:random] = n
  end

  opts.on('--random-pairs N', Integer, "Print N random pair entries as 'LHS <--> RHS'") do |n|
    options[:random_pairs] = n
  end

  opts.on('-l', '--list', 'List all individual names in the roster') do
    options[:list] = true
  end

  opts.on('-h', '--help', 'Show this help') do
    puts opts
    exit
  end
end.parse!

begin
  if options[:pair]
    result = find_pair_for(options[:pair])
    if result
      print_pair(result[0], result[1])
    else
      puts "No pair or group found containing '#{options[:pair]}'."
    end
  elsif options[:random]
    puts random_chain(options[:random])
  elsif options[:random_pairs]
    random_pairs(options[:random_pairs]).each do |left, right|
      puts "#{left} <--> #{right}"
    end
  elsif options[:list]
    puts "Roster names (#{NAMES.size}):"
    puts NAMES.join(', ')
  else
    puts 'No action specified. Use --pair NAME, --random N, --random-pairs N, or --list. Run with -h for help.'
  end
rescue ArgumentError => e
  warn "Error: #{e.message}"
  exit 1
end
