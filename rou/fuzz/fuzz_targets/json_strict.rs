#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = rou_core::json::strict(data) {
        let encoded =
            rou_core::json::canonical(&value).expect("parsed JSON has a canonical encoding");
        // RFC 8785 may round integers to binary64. The canonical bytes must be stable.
        if let Ok(reparsed) = rou_core::json::strict(&encoded) {
            assert_eq!(encoded, rou_core::json::canonical(&reparsed).unwrap());
        }
    }
});
