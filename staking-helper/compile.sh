RUSTFLAGS='-C link-arg=-s' cargo build --release --target wasm32-unknown-unknown --locked
wasm-opt -Oz ../build/wasm32-unknown-unknown/release/staking_helper.wasm -o ./contract.wasm