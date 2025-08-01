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
