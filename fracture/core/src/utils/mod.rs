use tokio::time::sleep;
use tokio::time::Duration;

pub mod ipencoding;

#[inline]
pub async fn wait_update_time() {
    //used in update loops
    sleep(Duration::from_millis(crate::conf::UPDATE_TIME)).await;
}
