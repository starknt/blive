// 下载统计信息
#[derive(Debug, Clone, Default)]
pub struct DownloadStats {
    pub bytes_downloaded: u64,
    pub download_speed_kbps: f32,
    pub duration_ms: u64,
    pub reconnect_count: u32,
    pub last_error: Option<String>,
}

impl DownloadStats {
    pub fn reset(&mut self) {
        self.bytes_downloaded = 0;
        self.download_speed_kbps = 0.0;
        self.duration_ms = 0;
        self.last_error = None;
    }
}
