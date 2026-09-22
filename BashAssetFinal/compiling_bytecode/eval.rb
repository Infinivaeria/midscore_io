arguments = ARGV
from = arguments[0]
byte_code = File.binread(from)
iseq - RubyVM::InstructionSequence.load_from_binary(Marshal.load(byte_code))
iseq.eval

