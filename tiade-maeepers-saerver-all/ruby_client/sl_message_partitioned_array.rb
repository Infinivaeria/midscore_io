require 'json'
require 'timeout'

module SlMessageStore
  def self.call(args)
    if args[2] == 'handler'
      input = JSON.parse(args.fetch(1))
      result = Timeout.timeout(2) do
        handler = eval(args.fetch(3), TOPLEVEL_BINDING, 'SL_MESSAGE_HANDLER', 1)
        raise 'Handler must return a callable' unless handler.respond_to?(:call)

        handler.call(input)
      end
      return JSON.generate(result)
    end
    require args.fetch(0)
    records = JSON.parse(args.fetch(1))
    array = PartitionedArray.new(db_size: 1, partition_amount_and_offset: 32,
                                 dynamically_allocates: false)
    array.allocate
    records.each do |record|
      array.add_partition unless array.data_arr.any?(&:empty?)
      added = array.add_nosave { |row| row.merge!(record) }
      raise 'Could not allocate prim record' if added.nil?
    end
    JSON.generate({ backend: 'ruby-partitioned-array-v1', db_size: array.db_size,
                    partition_amount_and_offset: array.partition_amount_and_offset,
                    data_arr: array.data_arr })
  end
end

SlMessageStore.method(:call)
