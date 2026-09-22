module JSON
  def to_json
    case self
    when Hash, Array
      JSON.generate(self)
    when String
      "\"#{self}\""
    when NilClass
      'null'
    when TrueClass
      'true'
    when FalseClass
      'false'
    when Numeric
      self.to_s
    else
      self.to_s
    end
  end

  def self.included(base)
    base.extend(ClassMethods)
  end

  module ClassMethods
    def from_json(json)
      JSONParser.parse(json)
    end
  end

  def self.generate(obj)
    case obj
    when Hash
      "\{#{obj.map { |k, v| "#{k.to_json}:#{generate(v)}" }.join(',')}\}"

    when Array
      "[#{obj.map { |v| generate(v) }.join(',')}]"
    else
      obj.to_s.to_json
    end
  end

  def self.parse(json)
    case json[0]
    when '{'
      parse_object(json)
    when '['
      parse_array(json)
    else
      parse_value(json)
    end
  end

  def self.parse_object(json)
    json = json[1..-2] # Remove the surrounding braces
    pairs = json.split(',')
    obj = {}
    pairs.each do |pair|
      key, value = pair.split(':', 2)
      obj[parse_value(key)] = parse(value)
    end
    obj
  end

  def self.parse_array(json)
    json = json[1..-2] # Remove the surrounding brackets
    elements = json.split(',')
    elements.map { |element| parse(element) }
  end

  def self.parse_value(value)
    case value
    when /^\d+$/
      value.to_i
    when /^\d+\.\d+$/
      value.to_f
    when 'true'
      true
    when 'false'
      false
    when 'null'
      nil
    else
      value[1..-2] # Remove the surrounding quotes
    end
  end
end

class Object
  include JSON
end

class Hash
  include JSON
end

class Array
  include JSON
end


p [1,0]
puts a= {a: {"q" => "lol"}, b: 0}
puts a.to_json
puts a
puts a.to_json == '{"a":1,"b":0}'
# parse
puts JSON.parse('{"a":1,"b":0}') == {"a" => 1, "b" => 0}
