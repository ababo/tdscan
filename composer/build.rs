use std::env;
use std::fs::{create_dir_all, set_permissions, File};
use std::io::copy;
use std::path::{Path, PathBuf};

use zip::ZipArchive;

fn target_dir<P: AsRef<Path>, I: IntoIterator<Item = P>>(iter: I) -> PathBuf {
    let mut path = PathBuf::from(env::var("OUT_DIR").unwrap());
    path.extend(iter);
    path
}

fn unzip<P: AsRef<Path>, P2: AsRef<Path>>(zip: P, to: P2) {
    let in_file = File::open(zip).unwrap();
    let mut archive = ZipArchive::new(in_file).unwrap();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let out_path = match file.enclosed_name() {
            Some(path) => {
                let mut parent = to.as_ref().to_owned();
                parent.extend(path);
                parent
            }
            None => continue,
        };

        if file.name().ends_with('/') {
            create_dir_all(&out_path).unwrap();
        } else {
            if let Some(p) = out_path.parent() {
                create_dir_all(&p).unwrap();
            }
            let mut out_file = File::create(&out_path).unwrap();
            copy(&mut file, &mut out_file).unwrap();
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                set_permissions(
                    &out_path,
                    std::fs::Permissions::from_mode(mode),
                )
                .unwrap();
            }
        }
    }
}

fn build_poisson_recon() {
    let lib = target_dir(["libPoissonRecon.a"]);
    if lib.exists() {
        return;
    }

    println!("Building libPoissonRecon.a");

    let src = target_dir(["PoissonRecon"]);
    if !src.exists() {
        let zip = target_dir(["PoissonRecon.zip"]);
        if !zip.exists() {
            println!("Downloading PoissonRecon.zip");
            let mut file = File::create(&zip).unwrap();
            const URL: &str = "https://github.com/mkazhdan/PoissonRecon/archive/refs/heads/master.zip";
            let mut resp = ureq::get(URL).call().unwrap().into_reader();
            copy(&mut resp, &mut file).unwrap();
        }
        println!("Unzipping PoissonRecon.zip");
        unzip(zip, src);
    }
}

fn main() {
    build_poisson_recon();
}
