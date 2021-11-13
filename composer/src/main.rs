mod animate;
mod build_view;
mod combine;
mod export_to_json;
mod import_obj;
mod misc;
mod optimize_scan_geometry;
mod point_cloud;
mod poisson;
mod select;

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

#[allow(clippy::large_enum_variant)]
#[derive(StructOpt)]
enum Command {
    Animate(animate::AnimateParams),
    BuildView(build_view::BuildViewParams),
    Combine(combine::CombineParams),
    ExportToJson(export_to_json::ExportToJsonParams),
    ImportObj(import_obj::ImportObjParams),
    OptimizeScanGeometry(optimize_scan_geometry::OptimizeScanGeometryParams),
    Select(select::SelectParams),
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

    let res = match opts.command {
        Command::Animate(params) => animate::animate_with_params(&params),
        Command::BuildView(params) => {
            build_view::build_view_with_params(&params)
        }
        Command::Combine(params) => combine::combine_with_params(&params),
        Command::ExportToJson(params) => {
            export_to_json::export_to_json_with_params(&params)
        }
        Command::ImportObj(params) => {
            import_obj::import_obj_with_params(&params)
        }
        Command::OptimizeScanGeometry(params) => {
            optimize_scan_geometry::optimize_scan_geometry_with_params(&params)
        }
        Command::Select(params) => select::select_with_params(&params),
    };

    if let Err(err) = res {
        error!("{}", err);
        std::process::exit(1);
    }
}
