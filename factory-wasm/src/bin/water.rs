//! Runs the deterministic disturbed-water measurement recorded in `docs/BENCHMARKS.md`.

fn main() {
    let report = factory_wasm::water_bench::run();
    println!("{}", factory_wasm::water_bench::format(&report));
}
