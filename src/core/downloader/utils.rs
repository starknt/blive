// pretty bytes
pub fn pretty_bytes(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut i = 0;
    let mut value = bytes as f64;

    while value >= 1024.0 && i < units.len() - 1 {
        value /= 1024.0;
        i += 1;
    }

    format!("{:.2} {}", value, units[i])
}

// pretty kb
pub fn pretty_kb(kb: f32) -> String {
    let units = ["MB", "GB", "TB"];
    let mut i = 0;
    let mut value = kb as f64;

    while value >= 1024.0 && i < units.len() - 1 {
        value /= 1024.0;
        i += 1;
    }

    format!("{:.2} {}", value, units[i])
}

pub fn pretty_duration(duration: u64) -> String {
    let hours = duration / 3600;
    let minutes = (duration % 3600) / 60;
    let seconds = duration % 60;

    format!("{hours:02}:{minutes:02}:{seconds:02}")
}
