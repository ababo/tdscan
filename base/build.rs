fn main() -> std::io::Result<()> {
    prost_build::compile_protos(&["src/proto/model.proto"], &["src/"])?;
    Ok(())
}
