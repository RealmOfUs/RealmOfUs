#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = rou_core::glb::precheck(data);
    let _ = rou_core::glb::externalise(data);
    let _ = rou_core::image::precheck(data);
});
