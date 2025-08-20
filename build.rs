#[cfg(feature = "ffmpeg")]
use ffmpeg_sidecar::{
    download::{check_latest_version, download_ffmpeg_package, ffmpeg_download_url, unpack_ffmpeg},
    version::ffmpeg_version_with_path,
};
use std::process::Command;

#[cfg(feature = "ffmpeg")]
fn download_ffmpeg() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(debug_assertions)]
    {
        use ffmpeg_sidecar::command::ffmpeg_is_installed;

        if ffmpeg_is_installed() {
            println!("FFmpeg is already installed, skip download");
            return Ok(());
        }
    }

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

    // clean the destination directory
    if destination.exists() {
        std::fs::remove_dir_all(&destination)?;
    }

    // ensure the destination directory exists
    std::fs::create_dir_all(&destination)?;

    // The built-in download function uses `reqwest` to download the package.
    // For more advanced use cases like async streaming or download progress
    // updates, you could replace this with your own download function.
    println!("Downloading from: {download_url:?}");
    let archive_path = download_ffmpeg_package(download_url, &destination)?;

    // Extraction uses `tar` on all platforms (available in Windows since version 1803)
    println!("Extracting... {archive_path:?} -> {destination:?}");
    match unpack_ffmpeg(&archive_path, &destination) {
        Ok(_) => println!("Extracted successfully"),
        Err(e) => eprintln!("Extraction failed: {e:?}"),
    }

    // Use the freshly installed FFmpeg to check the version number
    let version = ffmpeg_version_with_path(destination.join("ffmpeg"))?;
    println!("FFmpeg version: {version}");

    println!("Done! 🏁");

    Ok(())
}

#[cfg(not(feature = "ffmpeg"))]
fn download_ffmpeg() -> Result<(), Box<dyn std::error::Error>> {
    // 在未启用 ffmpeg feature 时，不下载 sidecar，仅确保资源目录存在，避免 cargo-bundle 报错
    let destination = resolve_relative_path("resources/sidecar".into());
    let ffmpeg_dir = destination.join("ffmpeg");
    if ffmpeg_dir.exists() {
        if ffmpeg_dir.is_file() {
            // 如果存在同名文件，移除后创建目录
            std::fs::remove_file(&ffmpeg_dir)?;
            std::fs::create_dir_all(&ffmpeg_dir)?;
        }
        // 目录已存在则跳过
    } else {
        std::fs::create_dir_all(&ffmpeg_dir)?;
    }
    println!(
        "Skipping FFmpeg sidecar download (feature 'ffmpeg' not enabled). Created {:?}",
        ffmpeg_dir
    );
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

        let lite = if cfg!(feature = "lite") { "1" } else { "0" };
        let env_iss = "resources/windows/env.iss";
        let env_iss = std::path::Path::new(env_iss);
        println!("cargo:rerun-if-changed={}", env_iss.display());
        let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap();
        std::fs::remove_file(env_iss)
            .unwrap_or_else(|_| println!("No previous env.iss file found, creating a new one."));
        // Generate the Inno Setup script, overwriting the env.iss file.
        // This is used to set the application name and version dynamically.
        std::fs::write(
            env_iss,
            format!(
                r#"
#define MyAppName "BLive"
#define MyAppVersion "{pkg_version}"
#define LITE {lite}
"#
            ),
        )
        .expect("Failed to read env.iss");

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
