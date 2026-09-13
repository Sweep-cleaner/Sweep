//! Bellek optimizasyonu (geriye dönük uyumluluk cephesi).
//!
//! Gerçek motor [`crate::engine::memory`] içindedir; bu modül yalnız
//! `memopt::run()` çağrılarını güvenli (agresif olmayan) varsayılanlarla
//! oraya yönlendirir, böylece eski çağıranlar ve testler değişmeden çalışır.
//!
//! Kullanılan yerel API'ler: Linux'ta `/proc/sys/vm/drop_caches`, Windows'ta
//! `EmptyWorkingSet` / `EnumProcesses` / `OpenProcess` (ve `--aggressive` ile
//! `NtSetSystemInformation`), macOS'ta `purge`.

use crate::core::report::Report;

pub use crate::engine::memory::{MemoryOptions, CLEANER_ID};

/// Belleği optimize et (güvenli seviye: pagecache / working set).
pub fn run() -> Report {
    crate::engine::memory::run(&MemoryOptions::safe())
}

#[cfg(test)]
mod tests {
    #[test]
    fn runs_without_panic() {
        let r = super::run();
        // Root yoksa fail, varsa tek command girdisi — ikisi de geçerli.
        assert!(r.entries.len() <= 1);
    }
}
