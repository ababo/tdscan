fn main() -> std::io::Result<()> {
    let mut config = prost_build::Config::new();

    config.type_attribute("Point2", "#[derive(Copy)] #[repr(C)]");
    config.type_attribute("Point3", "#[derive(Copy)] #[repr(C)]");

    config.type_attribute("Point2", "#[derive(serde::Serialize)]");
    config.type_attribute("Point3", "#[derive(serde::Serialize)]");
    config.type_attribute("Image", "#[derive(serde::Serialize)]");
    config.type_attribute("ElementView", "#[derive(serde::Serialize)]");
    config.type_attribute("ElementView.Face", "#[derive(serde::Serialize)]");
    config.type_attribute("ElementViewState", "#[derive(serde::Serialize)]");
    config.type_attribute("Scan", "#[derive(serde::Serialize)]");
    config.type_attribute("ScanFrame", "#[derive(serde::Serialize)]");
    config.type_attribute("Record", "#[derive(serde::Serialize)]");
    config.type_attribute("Record.type", "#[derive(serde::Serialize)]");

    config.compile_protos(&["src/fm/data.proto"], &["src/"])?;

    Ok(())
}
