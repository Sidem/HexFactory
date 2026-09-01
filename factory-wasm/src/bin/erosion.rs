//! Runs the deterministic accelerated geomorphic-epoch measurement.

fn main() {
    let report = factory_wasm::erosion_bench::run();
    println!("{}", factory_wasm::erosion_bench::format(&report));
}
