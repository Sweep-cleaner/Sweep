//! Bellek optimizasyonu — üç işletim sisteminde de yerel API'lerle.
//!
//! `sweep memopt` bu modülü kullanır; [`crate::engine::memopt`] geriye dönük
//! uyumluluk için ince bir cephedir.
//!
//! - **Linux:** `sync` + `/proc/sys/vm/drop_caches`. Normal mod yalnız sayfa
//!   önbelleğini bırakır (`1`); `--aggressive` dentries + inode'ları da ekler
//!   (`3`) ve takas doluyken `swapoff -a && swapon -a` ile takası temizler.
//!   Root ister; rootsuz çalışmada rapor `fail` ile biter, sistem değişmez.
//! - **Windows:** her sürecin çalışma kümesi `EmptyWorkingSet` ile boşaltılır
//!   (yükseltme gerekmez, erişilemeyen süreçler sessizce atlanır).
//!   `--aggressive` ayrıca `NtSetSystemInformation(SystemMemoryListInformation)`
//!   ile standby list'i boşaltır — bu adım yönetici ister, yoksa atlanır.
//! - **macOS:** `purge` ile disk belleği/önbellek temizlenir; `memory_pressure`
//!   çıktısı analiz edilerek baskı seviyesi raporlanır.
//!
//! `--dry-run` hiçbir şeyi değiştirmez; yalnız etki tahminini raporlar.

use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "memopt";

/// Bellek optimizasyonu seçenekleri.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryOptions {
    /// Takas (swap) ve standby list gibi daha derin katmanları da temizle.
    pub aggressive: bool,
    /// Hiçbir şeyi değiştirme; yalnız etki tahminini raporla.
    pub dry_run: bool,
}

impl MemoryOptions {
    /// Yalnız güvenli seviye (pagecache / working set).
    pub fn safe() -> Self {
        Self::default()
    }

    /// Güvenli seviye + takas / standby.
    pub fn aggressive() -> Self {
        Self {
            aggressive: true,
            dry_run: false,
        }
    }
}

/// Şu an kullanılan bayt (tüm platformlarda `sysinfo` ile).
///
/// Ölçüm başarısız olursa `None` döner; çağıranlar bunu "kazanç bilinmiyor"
/// olarak raporlar, hata olarak değil.
pub fn used_bytes() -> Option<u64> {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    let total = system.total_memory();
    if total == 0 {
        return None;
    }
    Some(system.used_memory())
}

/// Belleği optimize et; sonucu raporla.
pub fn run(opts: &MemoryOptions) -> Report {
    let mut report = Report::new();
    let before = used_bytes();

    #[cfg(target_os = "linux")]
    optimize_linux(opts, before, &mut report);

    #[cfg(windows)]
    optimize_windows(opts, before, &mut report);

    #[cfg(target_os = "macos")]
    optimize_macos(opts, before, &mut report);

    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        let _ = (opts, before);
        report.fail(
            CLEANER_ID,
            "optimize",
            "memory optimize is unsupported on this platform",
        );
    }

    report
}

/// `before` ile `after` arasındaki kazancı rapor satırına çevir.
///
/// Ölçüm alınamadıysa 0 bayt bildirilir (uydurma bir sayı yazmaktansa
/// "bilinmiyor" demek daha dürüsttür, ama rapor şeması sayı bekler).
fn freed(before: Option<u64>, after: Option<u64>) -> u64 {
    match (before, after) {
        (Some(b), Some(a)) => b.saturating_sub(a),
        _ => 0,
    }
}

/// Ortak rapor satırı.
fn push_entry(report: &mut Report, option: &str, label: String, bytes: u64) {
    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        option,
        label,
        None,
        bytes,
    ));
}

/// İnsan okunur bayt (rapor metinleri için).
fn human(bytes: u64) -> String {
    crate::fsutil::size::bytes_to_human(bytes, false)
}

// ---------------------------------------------------------------------------
// Linux: vm.drop_caches + takas döngüsü
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, Default)]
struct LinuxMemInfo {
    /// Bırakılabilir sayfa önbelleği (Cached + SReclaimable).
    reclaimable: u64,
    swap_total: u64,
    swap_free: u64,
}

/// `/proc/meminfo`'yu oku (yalnız Linux).
#[cfg(target_os = "linux")]
fn linux_meminfo() -> Option<LinuxMemInfo> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut info = LinuxMemInfo::default();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let key = fields.next()?;
        // Değerler kB cinsindendir.
        let value = fields.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) * 1024;
        match key {
            "Cached:" | "SReclaimable:" => info.reclaimable += value,
            "SwapTotal:" => info.swap_total = value,
            "SwapFree:" => info.swap_free = value,
            _ => {}
        }
    }
    Some(info)
}

/// `drop_caches` seviyesini yaz (1 = pagecache, 2 = dentries/inodes, 3 = her ikisi).
#[cfg(target_os = "linux")]
fn drop_caches(level: u8) -> Result<(), String> {
    crate::platform::sync_filesystems();
    std::fs::write("/proc/sys/vm/drop_caches", format!("{level}\n"))
        .map_err(|_| crate::i18n::t(&crate::i18n::Lang::detect(&[]), "needs root (skipped)"))
}

/// Takası boşalt: `swapoff -a` sonra `swapon -a`.
///
/// Yalnız takas gerçekten doluyken çağrılır; kullanılan takas yoksa
/// `swapoff` gereksiz yere sayfa taşımaz.
#[cfg(target_os = "linux")]
fn swap_cycle() -> Result<(), String> {
    let (code, _, err) = crate::fsutil::run_command("swapoff", &["-a"], true)
        .map_err(|e| format!("swapoff: {e}"))?;
    if code != 0 {
        return Err(format!("swapoff exited {code}: {}", err.trim()));
    }
    let (code, _, err) = crate::fsutil::run_command("swapon", &["-a"], true)
        .map_err(|e| format!("swapon: {e}"))?;
    if code != 0 {
        return Err(format!("swapon exited {code}: {}", err.trim()));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn optimize_linux(opts: &MemoryOptions, before: Option<u64>, report: &mut Report) {
    let info = linux_meminfo();
    let level = if opts.aggressive { 3 } else { 1 };

    if opts.dry_run {
        let estimate = info.map(|i| i.reclaimable).unwrap_or(0);
        push_entry(
            report,
            "optimize",
            format!(
                "would drop caches (level {level}) — up to ~{} reclaimable",
                human(estimate)
            ),
            0,
        );
        if opts.aggressive {
            let swap_used = info.map(|i| i.swap_total.saturating_sub(i.swap_free)).unwrap_or(0);
            if swap_used > 0 {
                push_entry(
                    report,
                    "swap",
                    format!("would cycle swap (~{} in use)", human(swap_used)),
                    0,
                );
            } else {
                push_entry(
                    report,
                    "swap",
                    "would cycle swap (nothing swapped out)".to_string(),
                    0,
                );
            }
        }
        return;
    }

    match drop_caches(level) {
        Ok(()) => {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let gained = freed(before, used_bytes());
            push_entry(
                report,
                "optimize",
                format!("page cache dropped (level {level}, ~{} freed)", human(gained)),
                gained,
            );
        }
        Err(err) => {
            report.fail(CLEANER_ID, "optimize", err);
            return; // rootsuz: takas döngüsü de başarısız olur, tekrar denemeyelim
        }
    }

    if opts.aggressive {
        let swap_used = info
            .map(|i| i.swap_total.saturating_sub(i.swap_free))
            .unwrap_or(0);
        if swap_used == 0 {
            push_entry(
                report,
                "swap",
                "swap cycle skipped (nothing swapped out)".to_string(),
                0,
            );
        } else {
            match swap_cycle() {
                Ok(()) => push_entry(
                    report,
                    "swap",
                    format!("swap cycled (~{} returned to RAM)", human(swap_used)),
                    swap_used,
                ),
                Err(err) => report.fail(CLEANER_ID, "swap", err),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Windows: EmptyWorkingSet + standby list
// ---------------------------------------------------------------------------

#[cfg(windows)]
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
#[cfg(windows)]
const PROCESS_SET_QUOTA: u32 = 0x0100;

#[cfg(windows)]
#[link(name = "psapi")]
extern "system" {
    fn EnumProcesses(process_ids: *mut u32, cb: u32, cb_needed: *mut u32) -> i32;
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(
        dw_desired_access: u32,
        b_inherit_handle: i32,
        dw_process_id: u32,
    ) -> *mut std::ffi::c_void;
    fn CloseHandle(h_object: *mut std::ffi::c_void) -> i32;
    fn EmptyWorkingSet(h_process: *mut std::ffi::c_void) -> i32;
}

/// `NtSetSystemInformation` — standby list boşaltma.
///
/// `SystemMemoryListInformation` (0x50) sınıfı, `SYSTEM_MEMORY_LIST_COMMAND`
/// (`i32`) alır; `MemoryPurgeStandbyList = 4` bayat önbelleği serbest bırakır.
/// `SeProfileSingleProcessPrivilege` gerektirir; yönetici değilsek çağrı
/// `STATUS_PRIVILEGE_NOT_HELD` (0xC0000061) döner ve biz bunu "atlandı"
/// olarak raporlarız — sistem zarar görmez.
#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    #[link(name = "ntdll")]
    extern "system" {
        fn NtSetSystemInformation(
            system_information_class: u32,
            system_information: *mut c_void,
            system_information_length: u32,
        ) -> i32;
    }

    const SYSTEM_MEMORY_LIST_INFORMATION: u32 = 0x50;
    const MEMORY_PURGE_STANDBY_LIST: i32 = 4;
    /// `STATUS_PRIVILEGE_NOT_HELD` — yönetici yetkisi yok.
    pub const STATUS_PRIVILEGE_NOT_HELD: i32 = 0xC000_0061u32 as i32;

    pub fn purge_standby_list() -> Result<(), i32> {
        let mut command: i32 = MEMORY_PURGE_STANDBY_LIST;
        let status = unsafe {
            NtSetSystemInformation(
                SYSTEM_MEMORY_LIST_INFORMATION,
                &mut command as *mut i32 as *mut c_void,
                std::mem::size_of::<i32>() as u32,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(status)
        }
    }
}

/// Her sürecin çalışma kümesini boşalt; kaç sürece dokunulduğunu döndür.
#[cfg(windows)]
fn trim_working_sets() -> Result<u32, String> {
    let mut pids = [0u32; 4096];
    let mut needed: u32 = 0;
    let ok = unsafe {
        EnumProcesses(
            pids.as_mut_ptr(),
            (pids.len() * std::mem::size_of::<u32>()) as u32,
            &mut needed,
        )
    };
    if ok == 0 {
        return Err("EnumProcesses failed".to_string());
    }
    let count = ((needed as usize) / 4).min(pids.len());
    let mut trimmed: u32 = 0;
    for &pid in &pids[..count] {
        if pid == 0 {
            continue;
        }
        // SYSTEM / yükseltilmiş süreçler için OpenProcess başarısız olur —
        // sessizce atlanır, kullanıcı süreçleri yine de boşaltılır.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA, 0, pid) };
        if handle.is_null() {
            continue;
        }
        let rc = unsafe { EmptyWorkingSet(handle) };
        unsafe { CloseHandle(handle) };
        if rc != 0 {
            trimmed += 1;
        }
    }
    Ok(trimmed)
}

#[cfg(windows)]
fn optimize_windows(opts: &MemoryOptions, before: Option<u64>, report: &mut Report) {
    if opts.dry_run {
        // Gerçek boşaltma yapmadan süreç sayısını ölçmek için yalnız
        // EnumProcesses çağrılır (yan etkisiz).
        let mut pids = [0u32; 4096];
        let mut needed: u32 = 0;
        let ok = unsafe {
            EnumProcesses(
                pids.as_mut_ptr(),
                (pids.len() * std::mem::size_of::<u32>()) as u32,
                &mut needed,
            )
        };
        let count = if ok == 0 { 0 } else { (needed as usize) / 4 };
        push_entry(
            report,
            "optimize",
            format!("would trim working sets across up to {count} processes"),
            0,
        );
        if opts.aggressive {
            push_entry(
                report,
                "standby",
                "would purge the standby list (needs administrator)".to_string(),
                0,
            );
        }
        return;
    }

    match trim_working_sets() {
        Ok(trimmed) => {
            let gained = freed(before, used_bytes());
            push_entry(
                report,
                "optimize",
                format!("working sets trimmed across {trimmed} processes (~{} freed)", human(gained)),
                gained,
            );
        }
        Err(err) => report.fail(CLEANER_ID, "optimize", err),
    }

    if opts.aggressive {
        match win::purge_standby_list() {
            Ok(()) => {
                let gained = freed(before, used_bytes());
                push_entry(
                    report,
                    "standby",
                    "standby list purged".to_string(),
                    gained,
                );
            }
            Err(win::STATUS_PRIVILEGE_NOT_HELD) => push_entry(
                report,
                "standby",
                "standby purge skipped (needs administrator)".to_string(),
                0,
            ),
            Err(status) => report.fail(
                CLEANER_ID,
                "standby",
                format!("NtSetSystemInformation failed (status 0x{:08X})", status as u32),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// macOS: purge + memory_pressure
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn memory_pressure_summary() -> Option<String> {
    let out = crate::deep::command_output("memory_pressure", &[])?;
    // Çıktı "System-wide memory free percentage: NN%" satırını içerir.
    out.lines()
        .find(|line| line.to_lowercase().contains("free percentage"))
        .map(|line| line.trim().to_string())
}

#[cfg(target_os = "macos")]
fn optimize_macos(opts: &MemoryOptions, before: Option<u64>, report: &mut Report) {
    let pressure = memory_pressure_summary();
    if let Some(line) = &pressure {
        push_entry(report, "analyze", format!("memory pressure: {line}"), 0);
    }

    if opts.dry_run {
        push_entry(
            report,
            "optimize",
            "would run `purge` (needs root)".to_string(),
            0,
        );
        return;
    }

    if !crate::deep::have("purge") {
        report.fail(CLEANER_ID, "optimize", "purge not found");
        return;
    }
    match crate::fsutil::run_command("purge", &[], true) {
        Ok((0, _, _)) => {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let gained = freed(before, used_bytes());
            push_entry(
                report,
                "optimize",
                format!("inactive memory purged (~{} freed)", human(gained)),
                gained,
            );
        }
        Ok((code, _, err)) => report.fail(
            CLEANER_ID,
            "optimize",
            format!("purge exited {code}: {}", err.trim()),
        ),
        Err(err) => report.fail(CLEANER_ID, "optimize", format!("purge: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_changes_nothing() {
        let opts = MemoryOptions {
            aggressive: false,
            dry_run: true,
        };
        let report = run(&opts);
        // Dry-run hiçbir zaman hata üretmez ve en az bir "ne yapardı" satırı yazar.
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        assert!(!report.entries.is_empty());
        // Hiçbir satır bayt geri kazanımı iddia etmez.
        assert_eq!(report.reclaimed(), 0);
    }

    #[test]
    fn aggressive_dry_run_reports_extra_steps() {
        let opts = MemoryOptions {
            aggressive: true,
            dry_run: true,
        };
        let report = run(&opts);
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        assert!(report.entries.len() >= 2, "entries: {}", report.entries.len());
        assert!(report
            .entries
            .iter()
            .any(|e| e.option == "optimize" || e.option == "standby" || e.option == "swap"));
    }

    #[test]
    fn freed_is_saturating_and_tolerates_missing_measurements() {
        assert_eq!(freed(Some(1000), Some(400)), 600);
        // Ölçüm ters dönerse (başka bir süreç bellek aldı) negatife düşmez.
        assert_eq!(freed(Some(400), Some(1000)), 0);
        assert_eq!(freed(None, Some(1)), 0);
        assert_eq!(freed(Some(1), None), 0);
    }

    #[test]
    fn options_constructors_are_consistent() {
        assert_eq!(MemoryOptions::safe(), MemoryOptions::default());
        assert!(MemoryOptions::aggressive().aggressive);
        assert!(!MemoryOptions::aggressive().dry_run);
    }

    #[test]
    fn used_bytes_reports_plausible_total() {
        // sysinfo bir toplam bellek görebiliyorsa kullanılan da toplamı aşmamalı.
        if let Some(used) = used_bytes() {
            let mut system = sysinfo::System::new();
            system.refresh_memory();
            assert!(used <= system.total_memory());
        }
    }
}
