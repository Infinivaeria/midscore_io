Dir.chdir('THE-META_GAME-Magi-Tek_Tek-Magi-Engiane') do
  system('compile.bat') or abort('Failed to run compile.bat')
  system('cargo build --release') or abort('cargo build failed')
end
