lambda do |prim|
  state = prim.fetch('state')
  message = prim.fetch('message')
  if state['last_message'] == message
    { 'state' => state }
  else
    total = state.fetch('total', 0)
    3.times { total += 1 }
    { 'state' => { 'total' => total, 'last_message' => message },
      'display' => { 'text' => "Count: #{total}", 'color' => [0, 1, 0], 'alpha' => 1 } }
  end
end
