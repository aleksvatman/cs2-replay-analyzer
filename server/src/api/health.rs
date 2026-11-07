/// Liveness probe. Always public, never touches the disk.
pub async fn health() -> &'static str {
    "ok"
}
