#!/usr/bin/env python3
# Create Fuzz Test Corpus
# Built with prayer and excellence

import os
import struct
import json
from pathlib import Path

def create_bson_corpus():
    """Create BSON test corpus files"""
    corpus_dir = Path("corpus/bson")
    corpus_dir.mkdir(parents=True, exist_ok=True)
    
    # 1. Minimal valid BSON document: {}
    minimal = bytearray([0x05, 0x00, 0x00, 0x00, 0x00])
    (corpus_dir / "minimal.bson").write_bytes(minimal)
    
    # 2. Simple document: {"hello": "world"}
    simple = bytearray()
    doc_data = b'\x02hello\x00\x06\x00\x00\x00world\x00\x00'
    size = len(doc_data) + 4
    simple.extend(struct.pack('<I', size))
    simple.extend(doc_data)
    (corpus_dir / "simple.bson").write_bytes(simple)
    
    # 3. Nested document
    nested = create_bson({
        "user": {
            "name": "test",
            "age": 42
        },
        "tags": ["rust", "golang", "typescript"]
    })
    (corpus_dir / "nested.bson").write_bytes(nested)
    
    # 4. Large document (edge case)
    large_doc = {
        f"field_{i}": f"value_{i}" * 100
        for i in range(100)
    }
    large = create_bson(large_doc)
    (corpus_dir / "large.bson").write_bytes(large)
    
    # 5. Binary data
    binary_doc = bytearray()
    binary_data = b'\x05data\x00\x10\x00\x00\x00\x00' + b'\x00' * 16 + b'\x00'
    size = len(binary_data) + 4
    binary_doc.extend(struct.pack('<I', size))
    binary_doc.extend(binary_data)
    (corpus_dir / "binary.bson").write_bytes(binary_doc)
    
    # 6. Malformed documents for negative testing
    malformed_dir = corpus_dir / "malformed"
    malformed_dir.mkdir(exist_ok=True)
    
    # Invalid size
    (malformed_dir / "invalid_size.bson").write_bytes(
        b'\xFF\xFF\xFF\xFF\x00'
    )
    
    # Size larger than data
    (malformed_dir / "truncated.bson").write_bytes(
        b'\x20\x00\x00\x00\x02test\x00'
    )
    
    # Missing null terminator
    (malformed_dir / "no_terminator.bson").write_bytes(
        b'\x05\x00\x00\x00'
    )
    
    print(f"✅ Created {len(list(corpus_dir.rglob('*.bson')))} BSON corpus files")

def create_protobuf_corpus():
    """Create Protocol Buffer test corpus files"""
    corpus_dir = Path("corpus/protobuf")
    corpus_dir.mkdir(parents=True, exist_ok=True)
    
    # Import protobuf (you'll need to generate these from .proto files)
    # For now, create raw protobuf-like data
    
    # 1. WriteDiffRequest
    write_diff_minimal = bytearray()
    # Field 1 (node_id): tag = 0x0A (field 1, wire type 2)
    write_diff_minimal.extend(b'\x0A\x04test')
    # Field 2 (diff_bson): tag = 0x12 (field 2, wire type 2)
    write_diff_minimal.extend(b'\x12\x05\x05\x00\x00\x00\x00')
    (corpus_dir / "write_diff_minimal.pb").write_bytes(write_diff_minimal)
    
    # 2. ReadSnapshotRequest
    read_snapshot = bytearray()
    # Field 1 (node_id)
    read_snapshot.extend(b'\x0A\x08snapshot')
    # Field 2 (as_of_timestamp): tag = 0x10 (field 2, wire type 0)
    read_snapshot.extend(b'\x10\x80\x80\x80\x80\x10')  # varint encoded
    (corpus_dir / "read_snapshot.pb").write_bytes(read_snapshot)
    
    # 3. SubscribeRequest
    subscribe = bytearray()
    # Field 1 (node_patterns) repeated
    subscribe.extend(b'\x0A\x05*.log')
    subscribe.extend(b'\x0A\x07*.error')
    # Field 3 (include_initial_state): tag = 0x18 (field 3, wire type 0)
    subscribe.extend(b'\x18\x01')  # true
    (corpus_dir / "subscribe.pb").write_bytes(subscribe)
    
    # 4. VectorClock
    vector_clock = bytearray()
    # Repeated ClockEntry messages
    # Entry 1
    entry1 = b'\x0A\x05node1\x10\xE8\x07'  # node_id="node1", timestamp=1000
    vector_clock.extend(b'\x0A')
    vector_clock.extend(struct.pack('B', len(entry1)))
    vector_clock.extend(entry1)
    # Entry 2
    entry2 = b'\x0A\x05node2\x10\xD0\x0F'  # node_id="node2", timestamp=2000
    vector_clock.extend(b'\x0A')
    vector_clock.extend(struct.pack('B', len(entry2)))
    vector_clock.extend(entry2)
    (corpus_dir / "vector_clock.pb").write_bytes(vector_clock)
    
    # 5. Complex message with all fields
    complex_msg = bytearray()
    # Combine multiple field types
    complex_msg.extend(b'\x0A\x0Ccomplex_node')  # node_id
    complex_msg.extend(b'\x12\x20')  # diff_bson (32 bytes)
    complex_msg.extend(b'\x00' * 32)
    complex_msg.extend(b'\x1A\x10')  # vector_clock (16 bytes)
    complex_msg.extend(vector_clock[:16])
    # Metadata map
    complex_msg.extend(b'\x22\x0E\x0A\x03key\x12\x05value')
    (corpus_dir / "complex.pb").write_bytes(complex_msg)
    
    print(f"✅ Created {len(list(corpus_dir.glob('*.pb')))} Protobuf corpus files")

def create_json_corpus():
    """Create JSON test corpus for API testing"""
    corpus_dir = Path("corpus/json")
    corpus_dir.mkdir(parents=True, exist_ok=True)
    
    test_cases = [
        # Valid requests
        {
            "name": "write_diff_request.json",
            "data": {
                "node_id": "users/123",
                "diff": {
                    "op": "update",
                    "path": "/profile/name",
                    "value": "Test User"
                },
                "metadata": {
                    "user_id": "user123",
                    "timestamp": "2025-06-30T12:00:00Z"
                }
            }
        },
        {
            "name": "subscribe_request.json",
            "data": {
                "patterns": ["users/*", "posts/*"],
                "filter": "type == 'update'",
                "include_initial": True
            }
        },
        {
            "name": "conflict_resolution.json",
            "data": {
                "node_id": "doc/456",
                "local_version": {
                    "vector_clock": {"node1": 100, "node2": 50},
                    "data": {"title": "Local Version"}
                },
                "remote_version": {
                    "vector_clock": {"node1": 90, "node2": 60},
                    "data": {"title": "Remote Version"}
                },
                "strategy": "vector_clock"
            }
        },
        # Edge cases
        {
            "name": "empty_request.json",
            "data": {}
        },
        {
            "name": "large_metadata.json",
            "data": {
                "node_id": "test",
                "metadata": {
                    f"key_{i}": f"value_{i}" * 10
                    for i in range(100)
                }
            }
        },
        # Invalid requests for negative testing
        {
            "name": "invalid_node_id.json",
            "data": {
                "node_id": "../../../etc/passwd",
                "diff": {}
            }
        },
        {
            "name": "sql_injection.json",
            "data": {
                "node_id": "'; DROP TABLE users; --",
                "filter": "1=1"
            }
        }
    ]
    
    for test_case in test_cases:
        filepath = corpus_dir / test_case["name"]
        filepath.write_text(json.dumps(test_case["data"], indent=2))
    
    print(f"✅ Created {len(test_cases)} JSON corpus files")

def create_binary_corpus():
    """Create binary test patterns"""
    corpus_dir = Path("corpus/binary")
    corpus_dir.mkdir(parents=True, exist_ok=True)
    
    # 1. All zeros
    (corpus_dir / "zeros.bin").write_bytes(b'\x00' * 1024)
    
    # 2. All ones
    (corpus_dir / "ones.bin").write_bytes(b'\xFF' * 1024)
    
    # 3. Alternating pattern
    (corpus_dir / "alternating.bin").write_bytes(b'\xAA\x55' * 512)
    
    # 4. Random-like data
    import hashlib
    random_data = bytearray()
    seed = b"prayer_for_security"
    for i in range(64):
        hash_data = hashlib.sha256(seed + str(i).encode()).digest()
        random_data.extend(hash_data)
    (corpus_dir / "random.bin").write_bytes(random_data)
    
    # 5. UTF-8 stress test
    utf8_stress = "🙏 Test 测试 テスト 🚀" * 100
    (corpus_dir / "utf8_stress.bin").write_bytes(utf8_stress.encode('utf-8'))
    
    # 6. Common attack patterns
    attack_patterns = [
        b'A' * 10000,  # Buffer overflow attempt
        b'%s' * 100,   # Format string
        b'\x00\x00\x00\x00' * 256,  # Null bytes
        b'../../../../etc/passwd\x00',  # Path traversal
    ]
    
    for i, pattern in enumerate(attack_patterns):
        (corpus_dir / f"attack_{i}.bin").write_bytes(pattern)
    
    print(f"✅ Created {len(list(corpus_dir.glob('*.bin')))} binary corpus files")

def create_bson(obj):
    """Simple BSON encoder for testing"""
    # This is a simplified version - real implementation would be more complex
    import bson
    return bson.dumps(obj)

def main():
    print("🙏 Creating fuzz test corpus with prayer for robust testing...")
    print()
    
    create_bson_corpus()
    create_protobuf_corpus()
    create_json_corpus()
    create_binary_corpus()
    
    print()
    print("📊 Corpus Summary:")
    corpus_root = Path("corpus")
    total_files = len(list(corpus_root.rglob('*'))) - len(list(corpus_root.rglob('*/')))
    total_size = sum(f.stat().st_size for f in corpus_root.rglob('*') if f.is_file())
    
    print(f"   Total files: {total_files}")
    print(f"   Total size: {total_size:,} bytes")
    print()
    print("✅ Fuzz corpus creation complete!")
    print()
    print("To run fuzzing:")
    print("   cargo +nightly fuzz run bson_parser corpus/bson")
    print("   cargo +nightly fuzz run grpc_service corpus/protobuf")

if __name__ == "__main__":
    main()