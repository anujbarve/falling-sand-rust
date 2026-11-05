//! Device creation test; skips when the machine has no GPU adapter.

use sandbox_v2_gpu::Gpu;

#[test]
fn headless_device_or_skip() {
    match Gpu::new_headless() {
        Some(gpu) => {
            let info = gpu.adapter.get_info();
            assert!(!info.name.is_empty());
        }
        None => eprintln!("SKIPPED: no GPU adapter available"),
    }
}
