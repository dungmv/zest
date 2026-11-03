use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;

/// Returns the shared multi-threaded Tokio runtime for async network I/O and background tasks.
pub fn global_tokio_runtime() -> Arc<Runtime> {
    static RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            Arc::new(
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create global tokio runtime"),
            )
        })
        .clone()
}
