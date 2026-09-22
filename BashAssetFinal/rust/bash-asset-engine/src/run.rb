Dir.chdir('THE-META_GAME-Magi-Tek_Tek-Magi-Engiane') do
  system('.\compile.bat')
end

system('cargo build --release')
system('cargo run --release'
