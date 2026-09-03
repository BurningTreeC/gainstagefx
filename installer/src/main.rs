//! Installs the plugin bundles that sit next to this program.
//!
//! Ships inside the release archives so that installing does not mean reading
//! a README and copying folders by hand. It looks for the bundles beside
//! itself, works out where this platform keeps plugins, and copies them there.
//!
//! Deliberately free of dependencies: this binary is distributed to users, so
//! anything it linked would need accounting for in the licence notices.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Plugins are installed into a folder named after the vendor, which keeps the
/// plugin directories tidy and is what most vendors do.
const VENDOR: &str = "BurningTreeC";

const BUNDLES: [(&str, &str); 2] = [("GainStageFx.clap", "CLAP"), ("GainStageFx.vst3", "VST3")];

fn main() {
    let code = match run() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("\nInstallation failed: {error}");
            1
        }
    };
    // Double clicking in a file manager gives a window that closes the moment
    // the program ends, so hold it open long enough to read.
    if cfg!(windows) {
        print!("\nPress Enter to close. ");
        let _ = io::stdout().flush();
        let _ = io::stdin().read(&mut [0u8]);
    }
    std::process::exit(code);
}

fn run() -> io::Result<()> {
    let source = beside_this_program()?;
    println!("Installing from {}\n", source.display());

    let mut installed = 0;
    for (bundle, kind) in BUNDLES {
        let from = source.join(bundle);
        if !from.exists() {
            println!("{bundle} is not in this folder, skipping it");
            continue;
        }

        let dest = plugin_dir(kind)?.join(VENDOR);
        fs::create_dir_all(&dest)?;

        let to = dest.join(bundle);
        if to.exists() {
            remove(&to)?;
        }
        copy(&from, &to)?;
        println!("{kind:<5} {bundle}  ->  {}", dest.display());
        installed += 1;
    }

    if installed == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no plugin bundles were found next to this program",
        ));
    }
    println!("\nDone. Rescan plugins in your DAW.");
    Ok(())
}

/// The directory this program was started from, which is where the archive was
/// extracted and so where the bundles are.
fn beside_this_program() -> io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "cannot locate this program"))
}

/// Where this platform keeps plugins of a given kind.
fn plugin_dir(kind: &str) -> io::Result<PathBuf> {
    if cfg!(windows) {
        // The shared location needs administrator rights. Fall back to the
        // per-user one rather than failing, since every host reads both.
        if let Some(common) = env_path("COMMONPROGRAMFILES") {
            let dir = common.join(kind);
            if writable(&dir) {
                return Ok(dir);
            }
        }
        let local = env_path("LOCALAPPDATA").ok_or_else(|| missing("LOCALAPPDATA"))?;
        Ok(local.join("Programs").join("Common").join(kind))
    } else if cfg!(target_os = "macos") {
        let home = env_path("HOME").ok_or_else(|| missing("HOME"))?;
        Ok(home.join("Library").join("Audio").join("Plug-Ins").join(kind))
    } else {
        let home = env_path("HOME").ok_or_else(|| missing("HOME"))?;
        Ok(home.join(format!(".{}", kind.to_lowercase())))
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn missing(name: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("the {name} environment variable is not set"),
    )
}

/// Whether a directory can be created and written to, which on Windows is how
/// we find out if the program is running with administrator rights.
fn writable(dir: &Path) -> bool {
    if fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".gainstagefx-write-test");
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// A CLAP is a single shared library on Linux and Windows but a bundle
/// directory on macOS, and a VST3 is a directory everywhere, so both shapes
/// have to be handled.
fn copy(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(from, to).map(|_| ())
    }
}

fn remove(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}
