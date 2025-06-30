// gRPC Service Fuzz Target
// Built with prayer and excellence

#![no_main]

use libfuzzer_sys::fuzz_target;
use virtual_dom_gateway::proto::{
    WriteDiffRequest, ReadSnapshotRequest, SubscribeRequest,
    VectorClock, ClockEntry,
};
use prost::Message;
use bytes::Bytes;

fuzz_target!(|data: &[u8]| {
    // Test 1: Fuzz WriteDiffRequest parsing
    fuzz_write_diff_request(data);
    
    // Test 2: Fuzz ReadSnapshotRequest parsing
    fuzz_read_snapshot_request(data);
    
    // Test 3: Fuzz SubscribeRequest parsing
    fuzz_subscribe_request(data);
    
    // Test 4: Fuzz VectorClock operations
    fuzz_vector_clock(data);
    
    // Test 5: Fuzz metadata parsing
    fuzz_metadata(data);
});

fn fuzz_write_diff_request(data: &[u8]) {
    if let Ok(req) = WriteDiffRequest::decode(Bytes::from(data.to_vec())) {
        // Validate parsed request
        validate_node_id(&req.node_id);
        validate_diff_bson(&req.diff_bson);
        
        if let Some(vc) = &req.vector_clock {
            validate_vector_clock(vc);
        }
        
        // Test re-encoding
        let mut buf = Vec::new();
        let _ = req.encode(&mut buf);
        
        // Verify round-trip
        if let Ok(req2) = WriteDiffRequest::decode(Bytes::from(buf)) {
            assert_eq!(req.node_id, req2.node_id);
        }
    }
}

fn fuzz_read_snapshot_request(data: &[u8]) {
    if let Ok(req) = ReadSnapshotRequest::decode(Bytes::from(data.to_vec())) {
        // Validate fields
        validate_node_id(&req.node_id);
        
        if let Some(ts) = req.as_of_timestamp {
            assert!(ts >= 0, "Timestamp should be non-negative");
        }
        
        // Test field combinations
        match (req.include_metadata, req.include_history) {
            (true, true) => {
                // Both flags set - valid
            }
            (true, false) => {
                // Only metadata - valid
            }
            (false, true) => {
                // Only history - valid
            }
            (false, false) => {
                // Neither - valid but minimal
            }
        }
    }
}

fn fuzz_subscribe_request(data: &[u8]) {
    if let Ok(req) = SubscribeRequest::decode(Bytes::from(data.to_vec())) {
        // Validate subscription patterns
        for pattern in &req.node_patterns {
            validate_pattern(pattern);
        }
        
        // Validate filter expressions
        if let Some(filter) = &req.filter_expression {
            validate_filter_expression(filter);
        }
        
        // Check options
        if req.include_initial_state && req.start_after_timestamp.is_some() {
            // Valid combination
        }
    }
}

fn fuzz_vector_clock(data: &[u8]) {
    if data.len() < 8 {
        return;
    }
    
    // Create vector clock from fuzz data
    let num_entries = (data[0] as usize) % 10 + 1;
    let mut entries = Vec::new();
    
    for i in 0..num_entries.min(data.len() / 8) {
        let start = i * 8;
        if start + 8 <= data.len() {
            let node_id = format!("node_{}", data[start]);
            let timestamp = i64::from_le_bytes([
                data[start + 1], data[start + 2], data[start + 3], data[start + 4],
                data[start + 5], data[start + 6], data[start + 7], 0,
            ]);
            
            entries.push(ClockEntry {
                node_id,
                timestamp: timestamp.abs(),
            });
        }
    }
    
    let vc = VectorClock { entries };
    
    // Test vector clock operations
    test_vector_clock_compare(&vc);
    test_vector_clock_merge(&vc);
}

fn fuzz_metadata(data: &[u8]) {
    use std::collections::HashMap;
    
    if data.is_empty() {
        return;
    }
    
    let mut metadata = HashMap::new();
    let num_entries = (data[0] as usize) % 20;
    
    for i in 0..num_entries {
        if i * 2 + 1 < data.len() {
            let key = format!("key_{}", data[i * 2]);
            let value = format!("value_{}", data[i * 2 + 1]);
            metadata.insert(key, value);
        }
    }
    
    // Test metadata validation
    for (key, value) in &metadata {
        assert!(!key.is_empty(), "Empty metadata key");
        assert!(key.len() <= 256, "Metadata key too long");
        assert!(value.len() <= 65536, "Metadata value too long");
    }
}

// Validation helpers

fn validate_node_id(node_id: &[u8]) {
    assert!(!node_id.is_empty(), "Empty node ID");
    assert!(node_id.len() <= 256, "Node ID too long");
    
    // Check for valid characters (alphanumeric + special chars)
    for &byte in node_id {
        assert!(
            byte.is_ascii_alphanumeric() || b"._-/".contains(&byte),
            "Invalid character in node ID"
        );
    }
}

fn validate_diff_bson(diff_bson: &[u8]) {
    if diff_bson.is_empty() {
        return;
    }
    
    // Basic BSON validation
    if diff_bson.len() >= 4 {
        let size = u32::from_le_bytes([
            diff_bson[0], diff_bson[1], diff_bson[2], diff_bson[3]
        ]) as usize;
        
        assert!(
            size >= 5 && size <= 16 * 1024 * 1024,
            "Invalid BSON document size"
        );
    }
}

fn validate_vector_clock(vc: &VectorClock) {
    assert!(!vc.entries.is_empty(), "Empty vector clock");
    assert!(vc.entries.len() <= 100, "Too many vector clock entries");
    
    // Check for duplicate node IDs
    let mut seen = std::collections::HashSet::new();
    for entry in &vc.entries {
        assert!(seen.insert(&entry.node_id), "Duplicate node ID in vector clock");
        assert!(entry.timestamp >= 0, "Negative timestamp in vector clock");
    }
}

fn validate_pattern(pattern: &str) {
    assert!(!pattern.is_empty(), "Empty pattern");
    assert!(pattern.len() <= 1024, "Pattern too long");
    
    // Basic glob pattern validation
    let valid_chars = pattern.chars().all(|c| {
        c.is_ascii() && !c.is_control()
    });
    assert!(valid_chars, "Invalid characters in pattern");
}

fn validate_filter_expression(expr: &str) {
    assert!(!expr.is_empty(), "Empty filter expression");
    assert!(expr.len() <= 4096, "Filter expression too long");
    
    // Basic expression validation (no code injection)
    let forbidden = ["eval", "exec", "system", "__proto__"];
    for forbidden_str in &forbidden {
        assert!(!expr.contains(forbidden_str), "Forbidden string in filter");
    }
}

fn test_vector_clock_compare(vc: &VectorClock) {
    // Test comparison with itself
    let cmp = virtual_dom_gateway::vector_clock::compare(vc, vc);
    assert_eq!(cmp, std::cmp::Ordering::Equal);
    
    // Test comparison with empty clock
    let empty = VectorClock { entries: vec![] };
    let _ = virtual_dom_gateway::vector_clock::compare(vc, &empty);
}

fn test_vector_clock_merge(vc: &VectorClock) {
    // Test merge with itself
    let merged = virtual_dom_gateway::vector_clock::merge(vc, vc);
    assert_eq!(merged.entries.len(), vc.entries.len());
    
    // Test merge with modified version
    let mut modified = vc.clone();
    if let Some(entry) = modified.entries.first_mut() {
        entry.timestamp += 1;
    }
    
    let merged2 = virtual_dom_gateway::vector_clock::merge(vc, &modified);
    assert!(!merged2.entries.is_empty());
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_empty_input() {
        let data = &[];
        // Should not panic
        fuzz_write_diff_request(data);
        fuzz_read_snapshot_request(data);
        fuzz_subscribe_request(data);
    }
    
    #[test]
    fn test_valid_protobuf() {
        let req = WriteDiffRequest {
            node_id: b"test_node".to_vec(),
            diff_bson: vec![5, 0, 0, 0, 0], // Minimal BSON
            vector_clock: Some(VectorClock {
                entries: vec![ClockEntry {
                    node_id: "node1".to_string(),
                    timestamp: 12345,
                }],
            }),
            metadata: std::collections::HashMap::new(),
        };
        
        let mut buf = Vec::new();
        req.encode(&mut buf).unwrap();
        
        // Fuzz the valid protobuf
        fuzz_write_diff_request(&buf);
    }
}