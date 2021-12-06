use std::env;
use std::fs::{create_dir_all, set_permissions, File};
use std::io::copy;
use std::path::{Path, PathBuf};

use zip::ZipArchive;

fn target_dir() -> PathBuf {
    PathBuf::from(env::var("OUT_DIR").unwrap())
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
    println!("cargo:rerun-if-changed=src/poisson/poisson.cc");

    const COMMIT: &str = "40f2c86f75a2d202276fe6559c738d8089cdedad";
    let name = format!("PoissonRecon-{}", COMMIT);
    let proj = target_dir().join(&name);
    if !proj.exists() {
        let zip = target_dir().join("PoissonRecon.zip");
        if !zip.exists() {
            println!("Downloading PoissonRecon.zip");
            let mut file = File::create(&zip).unwrap();
            let url = format!(
                "https://github.com/ababo/PoissonRecon/archive/{}.zip",
                COMMIT
            );
            let mut resp = ureq::get(&url).call().unwrap().into_reader();
            copy(&mut resp, &mut file).unwrap();
        }
        println!("Unzipping PoissonRecon.zip");
        unzip(zip, target_dir());
    }

    println!("Building libPoissonRecon.a");
    let src = proj.join("Src_CC_wrap");
    cc::Build::new()
        .cpp(true)
        .include(proj.to_str().unwrap())
        .flag_if_supported("-std=c++14")
        .flag_if_supported("/std:c++14")
        .opt_level(3)
        .warnings(false)
        .flag_if_supported("-w")
        .file(src.join("PointData.cpp").to_str().unwrap())
        .file(src.join("PoissonReconLib.cpp").to_str().unwrap())
        .file(PathBuf::from("src").join("poisson").join("poisson.cc"))
        .compile("libPoissonRecon.a");

    println!("cargo:rustc-link-lib=static=PoissonRecon");
    if let Some(stdlib) = option_env!("CXXSTDLIB") {
        println!("cargo:rustc-link-lib=static={}", stdlib);
    }
}

fn main() {
    build_poisson_recon();
}
