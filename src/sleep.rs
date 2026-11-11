//! Host timers for the shared interpreter.

use std::time::Duration;

pub(crate) async fn sleep(mut duration: Duration) {
    // Small chunks avoid platform timer overflow for otherwise valid durations.
    // Browser runs can still be cancelled by terminating their worker.
    while !duration.is_zero() {
        let chunk = duration.min(Duration::from_secs(86_400));
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::sleep(chunk);
        #[cfg(target_arch = "wasm32")]
        sleep_millis(chunk.as_secs_f64() * 1000.0).await;
        duration -= chunk;
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "
export async function sleep_millis(milliseconds) {
    const deadline = performance.now() + milliseconds;
    do {
        await new Promise(resolve => setTimeout(resolve, Math.ceil(milliseconds)));
        milliseconds = deadline - performance.now();
    } while (milliseconds > 0);
}
")]
extern "C" {
    async fn sleep_millis(milliseconds: f64);
}
