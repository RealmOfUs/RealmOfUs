#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(Some(pointer)) = rou_core::lfs::pointer(data) {
        let encoded = rou_core::lfs::encode(&pointer).unwrap();
        assert_eq!(rou_core::lfs::pointer(&encoded).unwrap(), Some(pointer));
    }
});
