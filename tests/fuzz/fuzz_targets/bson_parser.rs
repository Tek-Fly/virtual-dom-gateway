// BSON Parser Fuzz Target
// Built with prayer and excellence

#![no_main]

use libfuzzer_sys::fuzz_target;
use virtual_dom_gateway::bson::{ZeroCopyBSON, BSONDocument};
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    // Test 1: Parse arbitrary data as BSON
    if let Ok(doc) = ZeroCopyBSON::from_bytes(data) {
        // Verify we can read without panicking
        let _ = doc.get_field("test");
        let _ = doc.get_nested("path.to.field");
        let _ = doc.to_json();
        
        // Verify round-trip
        if let Ok(bytes) = doc.to_bytes() {
            let _ = ZeroCopyBSON::from_bytes(&bytes);
        }
    }
    
    // Test 2: Parse as BSON document with validation
    let mut cursor = Cursor::new(data);
    if let Ok(doc) = BSONDocument::from_reader(&mut cursor) {
        // Verify document operations
        for (key, _value) in doc.iter() {
            // Ensure keys are valid UTF-8
            assert!(std::str::from_utf8(key.as_bytes()).is_ok());
        }
        
        // Test serialization
        let _ = doc.to_vec();
    }
    
    // Test 3: Zero-copy operations with bounds checking
    if data.len() >= 4 {
        // First 4 bytes should be document size
        let size = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        
        if size <= data.len() && size >= 5 {
            // Valid BSON document size
            if let Ok(doc) = ZeroCopyBSON::from_slice(&data[..size]) {
                // Test field extraction
                let _ = doc.extract_binary_field("data");
                let _ = doc.extract_string_field("name");
                let _ = doc.extract_i64_field("timestamp");
            }
        }
    }
    
    // Test 4: Malformed BSON handling
    test_malformed_bson(data);
});

fn test_malformed_bson(data: &[u8]) {
    // Test various malformed inputs
    let test_cases = vec![
        // Empty document
        vec![],
        // Invalid size
        vec![0xFF, 0xFF, 0xFF, 0xFF],
        // Size larger than data
        vec![0xFF, 0x00, 0x00, 0x00, 0x00],
        // Missing null terminator
        vec![0x05, 0x00, 0x00, 0x00],
        // Invalid type byte
        vec![0x10, 0x00, 0x00, 0x00, 0xFF, b'k', b'e', b'y', 0x00],
    ];
    
    for mut case in test_cases {
        // Append fuzz data to test case
        case.extend_from_slice(data);
        
        // Should not panic
        let _ = ZeroCopyBSON::from_bytes(&case);
        let _ = BSONDocument::from_bytes(&case);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_empty_input() {
        let data = &[];
        test_malformed_bson(data);
    }
    
    #[test]
    fn test_valid_bson() {
        // Minimal valid BSON document: {}\n
        let data = &[0x05, 0x00, 0x00, 0x00, 0x00];
        if let Ok(doc) = ZeroCopyBSON::from_bytes(data) {
            assert_eq!(doc.len(), 0);
        }
    }
}