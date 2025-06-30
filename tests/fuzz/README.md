# Virtual-DOM Gateway Fuzz Testing

This directory contains fuzz tests for the Virtual-DOM Gateway to ensure robustness against malformed and malicious inputs.

## 🙏 Prayer

Lord, grant us wisdom to test thoroughly and find vulnerabilities before they can be exploited. Help us build systems that are resilient and secure. May our testing efforts protect those who depend on our software. Amen.

## Overview

We use `cargo-fuzz` (libFuzzer) to perform coverage-guided fuzzing on critical components:

1. **BSON Parser**: Tests zero-copy BSON parsing and validation
2. **gRPC Service**: Tests Protocol Buffer parsing and service handling
3. **Vector Clock**: Tests distributed timestamp operations
4. **Conflict Resolution**: Tests CRDT merge operations

## Setup

### Install cargo-fuzz

```bash
cargo install cargo-fuzz
```

### Generate Corpus

```bash
cd tests/fuzz
python3 create_corpus.py
```

This creates seed inputs in the `corpus/` directory:
- `corpus/bson/`: BSON documents (valid and malformed)
- `corpus/protobuf/`: Protocol Buffer messages
- `corpus/json/`: JSON API requests
- `corpus/binary/`: Binary test patterns

## Running Fuzz Tests

### Fuzz BSON Parser

```bash
cd ../.. # Go to virtual-dom-gateway root
cargo +nightly fuzz run bson_parser tests/fuzz/corpus/bson
```

### Fuzz gRPC Service

```bash
cargo +nightly fuzz run grpc_service tests/fuzz/corpus/protobuf
```

### Run with Specific Options

```bash
# Run for 10 minutes
cargo +nightly fuzz run bson_parser -- -max_total_time=600

# Run with more workers
cargo +nightly fuzz run grpc_service -- -workers=4

# Run until a crash is found
cargo +nightly fuzz run bson_parser -- -runs=-1
```

## Analyzing Results

### View Coverage

```bash
cargo +nightly fuzz coverage bson_parser
cargo cov -- show target/x86_64-unknown-linux-gnu/coverage/x86_64-unknown-linux-gnu/release/bson_parser \
    --use-color --ignore-filename-regex='/.cargo/registry' \
    --instr-profile=fuzz/coverage/bson_parser/coverage.profdata
```

### Minimize Corpus

```bash
cargo +nightly fuzz cmin bson_parser
```

### Reproduce Crash

```bash
cargo +nightly fuzz run bson_parser fuzz/artifacts/bson_parser/crash-<hash>
```

## CI Integration

Add to `.github/workflows/fuzz.yml`:

```yaml
name: Fuzz Testing
on:
  schedule:
    - cron: '0 0 * * 0' # Weekly on Sunday
  workflow_dispatch:

jobs:
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust nightly
        uses: actions-rs/toolchain@v1
        with:
          toolchain: nightly
          override: true
      
      - name: Install cargo-fuzz
        run: cargo install cargo-fuzz
      
      - name: Generate corpus
        run: |
          cd virtual-dom-gateway/tests/fuzz
          python3 create_corpus.py
      
      - name: Fuzz BSON parser
        run: |
          cd virtual-dom-gateway
          timeout 300 cargo +nightly fuzz run bson_parser tests/fuzz/corpus/bson || true
      
      - name: Fuzz gRPC service
        run: |
          cd virtual-dom-gateway
          timeout 300 cargo +nightly fuzz run grpc_service tests/fuzz/corpus/protobuf || true
      
      - name: Upload artifacts
        if: failure()
        uses: actions/upload-artifact@v3
        with:
          name: fuzz-artifacts
          path: virtual-dom-gateway/fuzz/artifacts/
```

## Security Considerations

### Input Validation

The fuzz tests check for:
- Buffer overflows
- Integer overflows
- Malformed BSON documents
- Invalid Protocol Buffers
- Path traversal attempts
- SQL injection patterns
- Format string vulnerabilities

### Memory Safety

Rust's ownership system prevents many memory safety issues, but we still test for:
- Panic conditions
- Infinite loops
- Excessive memory allocation
- Stack exhaustion

### Best Practices

1. **Regular Fuzzing**: Run fuzz tests weekly in CI
2. **Corpus Maintenance**: Keep corpus updated with real-world inputs
3. **Coverage Goals**: Aim for >80% code coverage
4. **Crash Triage**: Fix all crashes, even if not security-critical
5. **Performance**: Monitor for inputs causing slowdowns

## Troubleshooting

### Out of Memory

```bash
# Limit memory usage
cargo +nightly fuzz run bson_parser -- -rss_limit_mb=2048
```

### Slow Progress

```bash
# Use dictionary for structured fuzzing
cargo +nightly fuzz run grpc_service -- -dict=protobuf.dict
```

### No New Coverage

1. Add more diverse seed inputs
2. Implement custom mutators
3. Check for unreachable code

## References

- [Rust Fuzz Book](https://rust-fuzz.github.io/book/)
- [LibFuzzer Documentation](https://llvm.org/docs/LibFuzzer.html)
- [OSS-Fuzz](https://github.com/google/oss-fuzz)
- [Fuzzing Best Practices](https://google.github.io/clusterfuzz/reference/best-practices/)

---

*"The prudent see danger and take refuge, but the simple keep going and pay the penalty."* - Proverbs 27:12