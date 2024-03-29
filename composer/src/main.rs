mod build_view;
mod combine;
mod export_to_json;
mod export_to_obj;
mod extract_scan_images;
mod import_from_obj;
mod mesh;
mod misc;
mod optimize_scan_geometry;
mod point_cloud;
mod poisson;
mod scan;
mod select;
mod texture;

use log::error;
use simplelog::{
    ColorChoice, Config as LogConfig, LevelFilter, TermLogger, TerminalMode,
};
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(about = "Fitsme model composer")]
struct Opts {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(StructOpt)]
enum Command {
    BuildView(Box<build_view::BuildViewCommand>),
    Combine(Box<combine::CombineCommand>),
    ExportToJson(Box<export_to_json::ExportToJsonCommand>),
    ExportToObj(Box<export_to_obj::ExportToObjCommand>),
    ExtractScanImages(Box<extract_scan_images::ExtractScanImagesCommand>),
    ImportFromObj(Box<import_from_obj::ImportFromObjCommand>),
    OptimizeScanGeometry(
        Box<optimize_scan_geometry::OptimizeScanGeometryCommand>,
    ),
    Select(Box<select::SelectCommand>),
}

fn main() {
    TermLogger::init(
        LevelFilter::Info,
        LogConfig::default(),
        TerminalMode::Stderr,
        ColorChoice::Auto,
    )
    .unwrap();

    let opts: Opts = Opts::from_args();

    use Command::*;
    let res = match opts.command {
        BuildView(cmd) => cmd.run(),
        Combine(cmd) => cmd.run(),
        ExportToJson(cmd) => cmd.run(),
        ExportToObj(cmd) => cmd.run(),
        ExtractScanImages(cmd) => cmd.run(),
        ImportFromObj(cmd) => cmd.run(),
        OptimizeScanGeometry(cmd) => cmd.run(),
        Select(cmd) => cmd.run(),
    };

    if let Err(err) = res {
        error!("{}", err);
        std::process::exit(1);
    }
}
