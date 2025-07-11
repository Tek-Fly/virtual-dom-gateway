fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Tell cargo to rerun this build script if the proto file changes
    println!("cargo:rerun-if-changed=../../proto/memory.proto");
    
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src")
        .compile_protos(&["../../proto/memory.proto"], &["../../proto"])?;
    Ok(())
}