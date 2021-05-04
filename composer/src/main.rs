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
    ImportObj(import_obj::ImportObjParams),
}

fn main() {
    let opts: Opts = Opts::from_args();

    let res = match opts.command {
        Command::ImportObj(params) => {
            import_obj::import_obj_with_params(&params)
        }
    };

    if let Err(err) = res {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}
