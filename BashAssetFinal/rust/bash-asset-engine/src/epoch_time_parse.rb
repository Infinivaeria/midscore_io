def epoch_to_date(epoch_time)
  Time.at(epoch_time).strftime("%Y-%m-%d %H:%M:%S")
end

# Example usage
epoch_time = 1629878400
converted_date = epoch_to_date(epoch_time)
puts converted_date
require 'date'

def epoch_to_date(epoch_time)
  Time.at(epoch_time).strftime("%Y-%m-%d %H:%M:%S")
end

# Example usage
epoch_time = 1629878400
converted_date = epoch_to_date(epoch_time)
puts converted_date
