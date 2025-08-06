use ffmpeg_sidecar::{
    download::{check_latest_version, download_ffmpeg_package, ffmpeg_download_url, unpack_ffmpeg},
    version::ffmpeg_version_with_path,
};
use std::process::Command;

fn download_ffmpeg() -> Result<(), Box<dyn std::error::Error>> {
    // Checking the version number before downloading is actually not necessary,
    // but it's a good way to check that the download URL is correct.
    match check_latest_version() {
        Ok(version) => println!("Latest available version: {version}"),
        Err(_) => println!("Skipping version check on this platform."),
    }

    // These defaults will automatically select the correct download URL for your
    // platform.
    let download_url = ffmpeg_download_url()?;
    let destination = resolve_relative_path("resources/sidecar".into());

    // ensure the destination directory exists
    std::fs::create_dir_all(&destination)?;

    // The built-in download function uses `reqwest` to download the package.
    // For more advanced use cases like async streaming or download progress
    // updates, you could replace this with your own download function.
    println!("Downloading from: {download_url:?}");
    let archive_path = download_ffmpeg_package(download_url, &destination)?;
    println!("Downloaded package: {archive_path:?}");

    // Extraction uses `tar` on all platforms (available in Windows since version 1803)
    println!("Extracting...");
    unpack_ffmpeg(&archive_path, &destination)?;

    // Use the freshly installed FFmpeg to check the version number
    let version = ffmpeg_version_with_path(destination.join("ffmpeg"))?;
    println!("FFmpeg version: {version}");

    println!("Done! 🏁");

    Ok(())
}

fn resolve_relative_path(path_buf: std::path::PathBuf) -> std::path::PathBuf {
    use std::path::{Component, PathBuf};

    let mut components: Vec<PathBuf> = vec![];
    for component in path_buf.as_path().components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                components.push(component.as_os_str().into())
            }
            Component::CurDir => (),
            Component::ParentDir => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            Component::Normal(component) => components.push(component.into()),
        }
    }
    PathBuf::from_iter(components)
}

fn main() {
    download_ffmpeg().unwrap();

    if cfg!(target_os = "macos") {
        println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=10.15.7");

        // Weakly link ReplayKit to ensure BLive can be used on macOS 10.15+.
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,ReplayKit");

        // Seems to be required to enable Swift concurrency
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

        // Register exported Objective-C selectors, protocols, etc
        println!("cargo:rustc-link-arg=-Wl,-ObjC");

        // weak link to support Catalina
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,ScreenCaptureKit");
    }

    // Populate git sha environment variable if git is available
    println!("cargo:rerun-if-changed=.git/logs/HEAD");
    println!(
        "cargo:rustc-env=TARGET={}",
        std::env::var("TARGET").unwrap()
    );
    if let Ok(output) = Command::new("git").args(["rev-parse", "HEAD"]).output()
        && output.status.success()
    {
        let git_sha = String::from_utf8_lossy(&output.stdout);
        let git_sha = git_sha.trim();

        println!("cargo:rustc-env=BLIVE_COMMIT_SHA={git_sha}");

        if let Ok(build_profile) = std::env::var("PROFILE")
            && build_profile == "release"
        {
            // This is currently the best way to make `cargo build ...`'s build script
            // to print something to stdout without extra verbosity.
            println!("cargo:warning=Info: using '{git_sha}' hash for BLIVE_COMMIT_SHA env var");
        }
    }

    #[cfg(target_os = "windows")]
    {
        #[cfg(target_env = "msvc")]
        {
            // todo(windows): This is to avoid stack overflow. Remove it when solved.
            println!("cargo:rustc-link-arg=/stack:{}", 8 * 1024 * 1024);
        }

        let icon = "resources/windows/icon.ico";
        let icon = std::path::Path::new(icon);

        println!("cargo:rerun-if-changed={}", icon.display());

        let mut res = winresource::WindowsResource::new();

        res.set_icon_with_id(icon.to_str().unwrap(), "IDI_ICON_TRAY");
        // res.set_manifest_file("resources/windows/app.exe.manifest");
        res.set_icon(icon.to_str().unwrap());
        res.set("FileDescription", "BLive");
        res.set("ProductName", "BLive");

        if let Err(e) = res.compile() {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
