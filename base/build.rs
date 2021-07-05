fn main() -> std::io::Result<()> {
    let mut config = prost_build::Config::new();
    config.type_attribute("Point2", "#[derive(Copy)] #[repr(C)]");
    config.type_attribute("Point3", "#[derive(Copy)] #[repr(C)]");
    config.compile_protos(&["src/fm/data.proto"], &["src/"])?;
    Ok(())
}
