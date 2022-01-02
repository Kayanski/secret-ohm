cargo wasm
RUSTFLAGS='-C link-arg=-s' cargo build --release --target wasm32-unknown-unknown --locked
wasm-opt -Oz ../build/wasm32-unknown-unknown/release/bond_depository.wasm -o ./contract.wasm
