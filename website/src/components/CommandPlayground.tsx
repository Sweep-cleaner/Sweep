import React, { useState } from 'react';

interface PlaygroundScenario {
  id: string;
  category: string;
  title: string;
  command: string;
  guiScreen: string;
  description: string;
  safetyNote: string;
  simulatedOutput: string;
}

const SCENARIOS: PlaygroundScenario[] = [
  {
    id: 'dev',
    category: 'Development',
    title: 'Purge Developer Cruft',
    command: 'sweep devscan ~/projects --clean --yes',
    guiScreen: 'Files → Dev Cruft',
    description: 'Searches for regenerable build artifacts (node_modules, target, .venv) next to project manifests and unlinks them safely.',
    safetyNote: 'Guarded by manifest check. Generic folders like "build" require package.json or Cargo.toml present.',
    simulatedOutput: `[+] Scanning /home/user/projects...
  [x] frontend/node_modules   (1.2 GB) -> Unlinked
  [x] api/target/release      (2.8 GB) -> Unlinked
  [x] worker/.venv            (640 MB) -> Unlinked
✓ Cleaned 3 directories · 4.64 GB reclaimed · 0.28s`
  },
  {
    id: 'residue',
    category: 'Cleanup',
    title: 'Confidence-Scored Residue Scan',
    command: 'sweep residue scan --confidence 0.7',
    guiScreen: 'Files → Residue',
    description: 'The Residue Intelligence Engine flags uninstall residue, dead build output, version caches and junk-only empty dirs, each with a confidence score and a safe_to_delete flag.',
    safetyNote: 'Documents, Desktop and Pictures are never scanned, .git trees are never touched, and findings below 0.6 stay preview-only.',
    simulatedOutput: `[+] Scanning residue across 3 roots...
  [0.90] ~/.config/acme-app        (binary gone + orphan config)
  [0.70] ~/src/old/target          (build output, no Cargo.toml)
  [0.70] ~/.cache/acme-app/v1      (version cache)
  [1.00] ~/.local/share/acme/empty (junk-only empty dir)
✓ 4 findings · 812 MB reclaimable · 0.34s`
  },
  {
    id: 'leftovers',
    category: 'Privacy & State',
    title: 'Clean Uninstalled App Leftovers',
    command: 'sweep leftovers discord --clean',
    guiScreen: 'Files → Leftovers',
    description: 'Scans ~/.config, ~/.cache, and ~/.local for residual state left behind by an uninstalled program.',
    safetyNote: 'Ownership Guard: Queries dpkg, rpm, or pacman to guarantee no active package owns the directory.',
    simulatedOutput: `[+] Querying local package manager database...
  [✓] Ownership check passed: "discord" is not an active package.
  Found 2 orphaned directories:
    • ~/.config/discord/Cache (412 MB)
    • ~/.local/share/discord/GPUCache (89 MB)
✓ Cleaned 501 MB in 2 folders. Undo log written.`
  },
  {
    id: 'bigfiles',
    category: 'Disk Space',
    title: 'Find 1GB+ Space Hogs',
    command: 'sweep bigfiles --min-size 1GB --top 10',
    guiScreen: 'Files → Big Files',
    description: 'Instantly identifies the largest files eating disk capacity across user directories, sorted strictly by size.',
    safetyNote: 'Read-only by default. Requires explicit --clean flag and confirmation before unlinking.',
    simulatedOutput: `[+] Scanning mounted partitions...
  1. 8.42 GB   ~/Downloads/ubuntu-24.04-desktop.iso (modified 45d ago)
  2. 4.10 GB   ~/Videos/raw-gameplay-recording.mp4 (modified 12d ago)
  3. 2.15 GB   ~/.local/share/Steam/steamapps/shadercache (modified 2d ago)
  4. 1.30 GB   ~/VirtualBox VMs/test-vm/disk.vdi (modified 90d ago)
✓ 4 files matching threshold (>= 1GB) found.`
  },
  {
    id: 'dupes',
    category: 'Storage',
    title: 'Duplicate Photo Finder',
    command: 'sweep dupes ~/Pictures --min-size 2MB --keep newest',
    guiScreen: 'Files → Duplicates',
    description: 'Identifies exact duplicate files using two-tier filtering — size match, then a content hash — and can keep the newest, oldest, largest or smallest copy.',
    safetyNote: 'With --link, duplicates become hard links to the kept copy instead of being deleted. Overlapping scan roots are never listed twice.',
    simulatedOutput: `[+] Hashing candidate files in ~/Pictures...
  Duplicate group 1 (14.2 MB each):
    [KEEP] ~/Pictures/Exported/Copy_IMG_4021.RAW (newest)
    [DUPE] ~/Pictures/Vacation/IMG_4021.RAW
✓ Found 12 duplicate groups · 186.4 MB recoverable.`
  },
  {
    id: 'journal',
    category: 'System Logs',
    title: 'Vacuum Systemd Journals',
    command: 'sweep clean system.journal_vacuum',
    guiScreen: 'Cleaner → System Logs',
    description: 'Trims systemd journalctl storage down to the configured budget (e.g. 500M or 7 days) without breaking system logging.',
    safetyNote: 'Uses journalctl native vacuum primitives; never corrupts active journal files.',
    simulatedOutput: `[+] Interfacing with systemd-journald...
  Current journal archive size: 2.18 GB
  Target retention budget: 500 MB
  Vacuuming logs older than 7 days...
✓ Vacuumed 1.68 GB of archived journal events.`
  },
  {
    id: 'memopt',
    category: 'Memory',
    title: 'Optimize Memory',
    command: 'sweep memopt --aggressive --dry-run',
    guiScreen: 'Memory',
    description: 'Native memory maintenance per OS: Linux drops page cache and cycles swap, Windows empties working sets and purges the standby list, macOS runs purge with memory_pressure analysis.',
    safetyNote: 'Always try --dry-run first. The gentle pass needs no root; --aggressive needs root/administrator.',
    simulatedOutput: `[dry-run] Linux memory optimization
  vm.drop_caches = 1   (page cache)
  swapoff -a && swapon -a   (swap cycle)
  Estimated reclaimable estimate: 3.4 GB
✓ Nothing was changed (--dry-run).`
  },
  {
    id: 'startup',
    category: 'Boot',
    title: 'Audit Startup Impact',
    command: 'sweep startup list --impact',
    guiScreen: 'Startup',
    description: 'Enumerates XDG autostart entries, systemd user units, Windows Run/RunOnce keys and Startup folders, or launchd agents, and estimates how much each one slows boot.',
    safetyNote: 'Disable is reversible: Hidden=true on XDG entries, sweep-disabled on systemd, .disabled on launchd.',
    simulatedOutput: `[+] Inspecting startup entries (impact analysis)...
  HIGH    Steam              ~2.1s   systemd user unit
  MEDIUM  Discord            ~0.8s   XDG autostart
  LOW     Nextcloud          ~0.2s   XDG autostart
✓ 3 entries · estimated 3.1s of boot time.`
  },
  {
    id: 'sysinfo',
    category: 'Health',
    title: 'System Health Check',
    command: 'sweep sysinfo --health',
    guiScreen: 'System Health',
    description: 'Reports CPU, memory, disks, uptime and temperature, with SMART attributes rolled into a single green/yellow/red verdict.',
    safetyNote: 'Read-only. The same data feeds the dashboard cards and the System Health screen.',
    simulatedOutput: `[+] Collecting system health...
  CPU        Ryzen 7 5700U   16 threads   11% load
  Memory     9.1 GB / 15.0 GB used
  Disk (/)   66.8% full
  Temp       cpu 54°C
  SMART      pass
✓ Overall: GREEN — no critical findings.`
  },
  {
    id: 'network',
    category: 'Network',
    title: 'Flush Network Caches',
    command: 'sweep network flush --dry-run',
    guiScreen: 'Network',
    description: 'Flushes the DNS resolver cache, then optionally cleans package-manager caches (npm, pip, yarn, cargo, gem) and runs git gc / docker network prune.',
    safetyNote: 'Everything supports --dry-run and reports the standard duration_ms / bytes_affected / items_count JSON shape.',
    simulatedOutput: `[dry-run] Network maintenance
  flush  DNS resolver cache (systemd-resolved / ipconfig)
  clean  npm, pip, yarn, cargo, gem caches
  git gc --auto
  docker network prune
✓ Dry run complete — nothing changed.`
  },
  {
    id: 'privacy',
    category: 'Privacy',
    title: 'Privacy Shield',
    command: 'sweep privacy shield',
    guiScreen: 'Privacy',
    description: 'Turns OS telemetry off; the wipe verb additionally clears recent-files lists and the clipboard.',
    safetyNote: 'shield only flips telemetry flags; wipe is the destructive verb and also supports --dry-run.',
    simulatedOutput: `[+] Applying privacy shield...
  [✓] OS telemetry: disabled
  [✓] Recent-files entries: 128 cleared
  [✓] Clipboard: cleared
✓ Telemetry off · recent files and clipboard wiped.`
  },
  {
    id: 'schedule',
    category: 'Automation',
    title: 'Weekly Automated Cleanup',
    command: 'sweep schedule --enable --hour 3 -- system.journal_vacuum apt.deep_cache',
    guiScreen: 'Schedule',
    description: 'Installs a timer on the native scheduler: a systemd user timer on Linux, Task Scheduler on Windows, launchd on macOS. A companion memory-trim task is available with --memopt.',
    safetyNote: 'Non-interactive execution automatically captures an audit log and runs in non-shred safe mode.',
    simulatedOutput: `[+] Generating ~/.config/systemd/user/sweep-weekly.service...
[+] Generating ~/.config/systemd/user/sweep-weekly.timer...
[+] Reloading systemd user daemon...
✓ Enabled and started sweep-weekly.timer (Next run: Sun 03:00:00).`
  }
];

export default function CommandPlayground() {
  const [selected, setSelected] = useState<PlaygroundScenario>(SCENARIOS[0]);
  const [isCopied, setIsCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(selected.command);
      setIsCopied(true);
      setTimeout(() => setIsCopied(false), 2000);
    } catch {
      // ignore
    }
  };

  return (
    <section id="demo" className="section">
      <div className="text-center mb-12">
        <p className="section-kicker">// INTERACTIVE PLAYGROUND</p>
        <h2 className="text-3xl sm:text-4xl md:text-5xl font-bold text-white mb-4">
          Explore Common Cleanups
        </h2>
        <div className="section-divider"></div>
        <p className="text-text-muted text-base sm:text-lg max-w-2xl mx-auto">
          Click a cleaning scenario below to preview the exact CLI command, safety boundaries, and simulated output.
        </p>
      </div>

      {/* Scenario Selector Chips */}
      <div className="flex flex-wrap justify-center gap-2.5 mb-10 max-w-4xl mx-auto">
        {SCENARIOS.map((s) => (
          <button
            key={s.id}
            onClick={() => setSelected(s)}
            className={`px-4 py-2 rounded-xl text-xs sm:text-sm font-medium transition-all cursor-pointer ${
              selected.id === s.id
                ? 'bg-gradient-to-r from-rust to-rust-glow text-bg-base font-semibold shadow-[0_0_20px_rgba(255,122,69,0.3)] scale-105'
                : 'bg-[#181926] text-text-muted hover:text-white hover:bg-white/5 border border-border/70'
            }`}
          >
            {s.title}
          </button>
        ))}
      </div>

      {/* Interactive Card Display */}
      <div className="max-w-4xl mx-auto rounded-2xl bg-[#141522] border border-border/80 overflow-hidden shadow-2xl">
        {/* Header Bar */}
        <div className="p-6 bg-[#181a2b] border-b border-border/70 flex flex-col sm:flex-row justify-between sm:items-center gap-4">
          <div>
            <div className="flex items-center gap-2 mb-1">
              <span className="text-xs font-mono px-2 py-0.5 rounded bg-cyan/15 border border-cyan/30 text-cyan">
                {selected.category}
              </span>
              <span className="text-xs text-text-dim">|</span>
              <span className="text-xs text-text-dim font-mono">{selected.guiScreen}</span>
            </div>
            <h3 className="text-lg sm:text-xl font-bold text-white">{selected.title}</h3>
          </div>

          <button
            onClick={handleCopy}
            className="self-start sm:self-auto inline-flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-mono font-semibold bg-cyan/10 border border-cyan/40 text-cyan hover:bg-cyan/20 hover:border-cyan transition-all cursor-pointer"
          >
            <span>{isCopied ? '✓ Copied' : 'Copy Command'}</span>
          </button>
        </div>

        {/* Details & Output */}
        <div className="p-6 space-y-6">
          {/* Command Banner */}
          <div className="p-4 rounded-xl bg-[#0e0f17] border border-border flex items-center justify-between font-mono text-xs sm:text-sm overflow-x-auto">
            <div className="flex items-center gap-2">
              <span className="text-rust select-none">$</span>
              <span className="text-cyan font-semibold">{selected.command}</span>
            </div>
          </div>

          {/* Description & Safety Note */}
          <div className="grid sm:grid-cols-2 gap-4 text-xs sm:text-sm">
            <div className="p-4 rounded-xl bg-bg-surface/50 border border-border/50">
              <div className="text-text-dim font-mono uppercase text-[11px] mb-1 font-semibold">How It Works</div>
              <p className="text-text-muted leading-relaxed">{selected.description}</p>
            </div>
            <div className="p-4 rounded-xl bg-rust/5 border border-rust/25">
              <div className="text-rust font-mono uppercase text-[11px] mb-1 font-semibold">Safety Boundary</div>
              <p className="text-text-primary/90 leading-relaxed">{selected.safetyNote}</p>
            </div>
          </div>

          {/* Simulated Terminal Output */}
          <div>
            <div className="text-xs font-mono text-text-dim mb-2 uppercase tracking-wider">Simulated Terminal Response</div>
            <pre className="p-4 rounded-xl bg-[#0a0b12] border border-border/80 font-mono text-xs text-text-primary overflow-x-auto leading-relaxed shadow-inner">
              <code>{selected.simulatedOutput}</code>
            </pre>
          </div>
        </div>

        {/* Footer Link */}
        <div className="p-4 bg-[#161725] border-t border-border/50 text-center text-xs text-text-dim">
          <span>Need full flag details? Check the </span>
          <a href="/docs/cli" className="text-cyan hover:underline font-medium">CLI Reference Guide →</a>
        </div>
      </div>
    </section>
  );
}
