fn main() -> std::io::Result<()> {
    let mut config = prost_build::Config::new();
    config.type_attribute("Point2", "#[repr(C)]");
    config.type_attribute("Point3", "#[repr(C)]");
    config.compile_protos(&["src/proto/model.proto"], &["src/"])?;
    Ok(())
}
