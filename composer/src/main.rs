mod animate;
mod build_view;
mod combine;
mod export_to_json;
mod import_obj;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(about = "Fitsme model composer")]
struct Opts {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(StructOpt)]
enum Command {
    Animate(animate::AnimateParams),
    BuildView(build_view::BuildViewParams),
    Combine(combine::CombineParams),
    ExportToJson(export_to_json::ExportToJsonParams),
    ImportObj(import_obj::ImportObjParams),
}

fn main() {
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
    };

    if let Err(err) = res {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}
