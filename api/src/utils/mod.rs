use tokio::time::sleep;
use tokio::time::Duration;

pub mod ipencoding;

#[inline]
pub async fn wait_100ms() {
    sleep(Duration::from_millis(100)).await;
}
