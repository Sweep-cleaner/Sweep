import React, { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';

interface TerminalTab {
  id: string;
  name: string;
  command: string;
  lines: { text: string; type: 'prompt' | 'info' | 'success' | 'warn' | 'dim' }[];
}

const TABS: TerminalTab[] = [
  {
    id: 'preview',
    name: 'preview --all',
    command: 'sweep preview --all',
    lines: [
      { text: '$ sweep preview --all', type: 'prompt' },
      { text: '[+] Inspecting 202 registered cleaners...', type: 'dim' },
      { text: '  ▸ apt.package_cache       1.42 GB  (324 packages)', type: 'info' },
      { text: '  ▸ system.journal_vacuum   480.0 MB (older than 7d)', type: 'info' },
      { text: '  ▸ firefox.history_db      38.2 MB  (vacuumable SQLite)', type: 'info' },
      { text: '  ▸ thumbnails.cache        185.6 MB (1,842 files)', type: 'info' },
      { text: '✓ 12,611 files scanned · 2.12 GB reclaimable · 4.85s', type: 'success' }
    ]
  },
  {
    id: 'devscan',
    name: 'devscan ~',
    command: 'sweep devscan ~/projects',
    lines: [
      { text: '$ sweep devscan ~/projects', type: 'prompt' },
      { text: '[+] Walking repository manifests (package.json, Cargo.toml, venv)...', type: 'dim' },
      { text: '  ▸ web-frontend/node_modules     1.18 GB (regenerable)', type: 'info' },
      { text: '  ▸ backend-api/target/release    2.45 GB (cargo cleanable)', type: 'info' },
      { text: '  ▸ ml-service/.venv              840.0 MB (poetry ready)', type: 'info' },
      { text: '  ▸ parser/__pycache__            12.4 MB  (bytecode)', type: 'info' },
      { text: '✓ 4 directories found · 4.48 GB recoverable · 0.21s', type: 'success' }
    ]
  },
  {
    id: 'leftovers',
    name: 'leftovers',
    command: 'sweep leftovers discord',
    lines: [
      { text: '$ sweep leftovers discord', type: 'prompt' },
      { text: '[+] Querying package ownership guard (dpkg, flatpak, snap)...', type: 'dim' },
      { text: '  [✓] Verified: "discord" is not an installed system package.', type: 'info' },
      { text: '  ▸ ~/.config/discord/Cache       412.0 MB (orphaned GPU cache)', type: 'warn' },
      { text: '  ▸ ~/.local/share/discord/logs   45.2 MB  (old crash logs)', type: 'warn' },
      { text: '✓ 2 leftover paths located · 457.2 MB safe to clean · 0.08s', type: 'success' }
    ]
  },
  {
    id: 'sysinfo',
    name: 'sysinfo --health',
    command: 'sweep sysinfo --health',
    lines: [
      { text: '$ sweep sysinfo --health', type: 'prompt' },
      { text: '[+] Collecting CPU, memory, disks, uptime, temperature and SMART...', type: 'dim' },
      { text: '  CPU     Ryzen 7 5700U   16 threads   11% load', type: 'info' },
      { text: '  Memory  9.1 GB / 15.0 GB used', type: 'info' },
      { text: '  Disk    /  66.8% full', type: 'info' },
      { text: '  SMART   pass', type: 'info' },
      { text: '✓ Overall: GREEN — no critical findings', type: 'success' }
    ]
  },
  {
    id: 'residue',
    name: 'residue scan',
    command: 'sweep residue scan --confidence 0.7',
    lines: [
      { text: '$ sweep residue scan --confidence 0.7', type: 'prompt' },
      { text: '[+] Scoring uninstall residue, dead build output and version caches...', type: 'dim' },
      { text: '  [0.90] ~/.config/acme-app         (binary gone, orphan config)', type: 'warn' },
      { text: '  [0.70] ~/src/old/target           (no Cargo.toml)', type: 'warn' },
      { text: '  [1.00] ~/.local/share/acme/empty  (junk-only empty dir)', type: 'info' },
      { text: '✓ 4 findings · 812 MB reclaimable · 0.34s', type: 'success' }
    ]
  }
];

export default function TerminalAnimation() {
  const [activeTab, setActiveTab] = useState(0);
  const [visibleLines, setVisibleLines] = useState<number>(0);
  const [isCopied, setIsCopied] = useState(false);

  useEffect(() => {
    setVisibleLines(0);
    const tab = TABS[activeTab];
    const totalLines = tab.lines.length;

    let current = 0;
    const interval = setInterval(() => {
      current++;
      setVisibleLines(current);
      if (current >= totalLines) {
        clearInterval(interval);
      }
    }, 180);

    return () => clearInterval(interval);
  }, [activeTab]);

  const copyCommand = async () => {
    try {
      await navigator.clipboard.writeText(TABS[activeTab].command);
      setIsCopied(true);
      setTimeout(() => setIsCopied(false), 2000);
    } catch {
      // ignore
    }
  };

  return (
    <div className="max-w-3xl mx-auto mt-14 rounded-2xl overflow-hidden border border-border/80 bg-[#12131d] shadow-[0_20px_50px_rgba(0,0,0,0.5)] text-left">
      {/* Terminal Titlebar & Tabs */}
      <div className="flex flex-wrap items-center justify-between px-4 py-2.5 bg-[#171826] border-b border-border/70 gap-2">
        <div className="flex items-center gap-2">
          <span className="h-3 w-3 rounded-full bg-red-500/80 inline-block"></span>
          <span className="h-3 w-3 rounded-full bg-yellow-500/80 inline-block"></span>
          <span className="h-3 w-3 rounded-full bg-green-500/80 inline-block"></span>
          <span className="text-text-dim text-xs font-mono ml-2 hidden sm:inline">sweep@workstation:~</span>
        </div>

        {/* Tab Switcher */}
        <div className="flex items-center gap-1 bg-[#10111a] p-1 rounded-lg border border-border/50">
          {TABS.map((tab, idx) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(idx)}
              className={`px-3 py-1 text-xs font-mono rounded-md transition-all cursor-pointer ${
                activeTab === idx
                  ? 'bg-cyan/20 text-cyan font-semibold border border-cyan/40 shadow-sm'
                  : 'text-text-muted hover:text-white hover:bg-white/5'
              }`}
            >
              {tab.name}
            </button>
          ))}
        </div>

        {/* Quick Copy Button */}
        <button
          onClick={copyCommand}
          className="text-[11px] font-mono px-2.5 py-1 rounded bg-bg-surface hover:bg-bg-elevated border border-border text-text-dim hover:text-cyan transition-colors cursor-pointer"
        >
          {isCopied ? 'Copied!' : 'Copy Cmd'}
        </button>
      </div>

      {/* Terminal Content Body */}
      <div className="p-6 font-mono text-xs sm:text-sm min-h-[220px] space-y-2.5 bg-[#0f1018]">
        <AnimatePresence mode="wait">
          <motion.div
            key={activeTab}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="space-y-2"
          >
            {TABS[activeTab].lines.map((line, idx) => {
              if (idx > visibleLines) return null;

              if (line.type === 'prompt') {
                return (
                  <div key={idx} className="text-white font-semibold flex items-center gap-2">
                    <span className="text-rust">$</span>
                    <span>{line.text.replace('$ ', '')}</span>
                  </div>
                );
              }

              if (line.type === 'dim') {
                return (
                  <div key={idx} className="text-text-dim italic">
                    {line.text}
                  </div>
                );
              }

              if (line.type === 'warn') {
                return (
                  <div key={idx} className="text-amber-300">
                    {line.text}
                  </div>
                );
              }

              if (line.type === 'success') {
                return (
                  <div key={idx} className="text-cyan font-bold pt-2 border-t border-border/30">
                    {line.text}
                  </div>
                );
              }

              return (
                <div key={idx} className="text-text-primary pl-2">
                  {line.text}
                </div>
              );
            })}

            {visibleLines < TABS[activeTab].lines.length && (
              <motion.span
                animate={{ opacity: [1, 0, 1] }}
                transition={{ duration: 0.6, repeat: Infinity }}
                className="inline-block w-2 h-4 bg-cyan ml-1"
              />
            )}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
