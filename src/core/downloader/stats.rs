// 下载统计信息
#[derive(Debug, Clone, Default)]
pub struct DownloadStats {
    pub bytes_downloaded: u64,
    pub download_speed_kbps: f32,
    pub duration_ms: u64,
    pub reconnect_count: u32,
    pub last_error: Option<String>,
}
