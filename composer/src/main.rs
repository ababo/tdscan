mod import_obj;

use clap::{AppSettings, Clap};

#[derive(Clap)]
#[clap(about = "Fitsme model composer")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clap)]
enum Command {
    ImportObj(import_obj::ImportObjParams),
}

fn main() {
    let opts: Opts = Opts::parse();

    let res = match opts.command {
        Command::ImportObj(params) => import_obj::import_obj(&params),
    };

    if let Err(err) = res {
        eprintln!("error: {:?}", err);
        std::process::exit(1);
    }
}
