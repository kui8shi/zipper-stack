cargo build --release --target wasm32-unknown-unknown
wasm-bindgen ./target/wasm32-unknown-unknown/release/riscv_emu_rust_wasm.wasm --out-dir ./ --target web --no-typescript