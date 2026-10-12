apt update && apt upgrade
rustup update
gem update --system && gem update
cd ./wasm-client
cargo update
cargo build --release
cd ..
cd ./tiade-maeepers-saerver-all
cargo update
cargo build --release
./start-with-wasm.sh
