require_relative 'lib/partitioned_array/lib/line_db'

a = LineDB.new
a["test"].pad.new_table!(database_name: "test_database", database_table: "test_table")
0.upto(4000) do |i|
    a['test'].pad["test_database", "test_table"].add_at_last do |hash|
        hash[:name] = "name#{i}"
        hash[:age] = i
        p i
    end
end


