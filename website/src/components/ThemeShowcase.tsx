import React, { useState } from 'react';

interface ThemePreset {
  id: string;
  name: string;
  tag: string;
  bg: string;
  surface: string;
  accent: string;
  accentLight: string;
  text: string;
  textDim: string;
  border: string;
}

// Mirrors packaging/qml/qml/ThemeManager.qml — 11 themes, in sidebar order.
const THEMES: ThemePreset[] = [
  {
    id: 'midnight-blue',
    name: 'Midnight Blue',
    tag: 'Default',
    bg: '#070B16',
    surface: '#0C1326',
    accent: '#1D4ED8',
    accentLight: '#60A5FA',
    text: '#DCE6FF',
    textDim: '#5B6B93',
    border: '#1E2B4D'
  },
  {
    id: 'gruvbox',
    name: 'Gruvbox Dark',
    tag: 'Warm Retro',
    bg: '#282828',
    surface: '#1d2021',
    accent: '#fe8019',
    accentLight: '#fabd2f',
    text: '#ebdbb2',
    textDim: '#a89984',
    border: '#504945'
  },
  {
    id: 'catppuccin',
    name: 'Catppuccin Mocha',
    tag: 'Pastel',
    bg: '#1e1e2e',
    surface: '#181825',
    accent: '#89b4fa',
    accentLight: '#b4befe',
    text: '#cdd6f4',
    textDim: '#a6adc8',
    border: '#313244'
  },
  {
    id: 'nord',
    name: 'Nord Arctic',
    tag: 'Subtle Ice',
    bg: '#2e3440',
    surface: '#242933',
    accent: '#88c0d0',
    accentLight: '#81a1c1',
    text: '#eceff4',
    textDim: '#d8dee9',
    border: '#434c5e'
  },
  {
    id: 'dracula',
    name: 'Dracula',
    tag: 'Vibrant Purple',
    bg: '#282a36',
    surface: '#21222c',
    accent: '#bd93f9',
    accentLight: '#ff79c6',
    text: '#f8f8f2',
    textDim: '#6272a4',
    border: '#44475a'
  },
  {
    id: 'tokyo-night',
    name: 'Tokyo Night',
    tag: 'Cyber Neon',
    bg: '#1a1b26',
    surface: '#16161e',
    accent: '#7aa2f7',
    accentLight: '#7dcfff',
    text: '#c0caf5',
    textDim: '#565f89',
    border: '#292e42'
  },
  {
    id: 'one-dark',
    name: 'One Dark',
    tag: 'Atom Minimal',
    bg: '#282c34',
    surface: '#21252b',
    accent: '#61afef',
    accentLight: '#98c379',
    text: '#abb2bf',
    textDim: '#5c6370',
    border: '#3e4451'
  },
  {
    id: 'solarized-dark',
    name: 'Solarized Dark',
    tag: 'Cyan Classic',
    bg: '#002b36',
    surface: '#073642',
    accent: '#268bd2',
    accentLight: '#2aa198',
    text: '#93a1a1',
    textDim: '#586e75',
    border: '#073642'
  },
  {
    id: 'monokai-pro',
    name: 'Monokai Pro',
    tag: 'High Contrast',
    bg: '#1C1B18',
    surface: '#12110E',
    accent: '#66D9EF',
    accentLight: '#A6E22E',
    text: '#F8F8F2',
    textDim: '#75715E',
    border: '#75715E'
  },
  {
    id: 'everforest',
    name: 'Everforest',
    tag: 'Serene Nature',
    bg: '#2d353b',
    surface: '#232a2e',
    accent: '#a7c080',
    accentLight: '#dbbc7f',
    text: '#d3c6aa',
    textDim: '#859289',
    border: '#3d484d'
  },
  {
    id: 'rose-pine',
    name: 'Rosé Pine',
    tag: 'Soho Pastel',
    bg: '#191724',
    surface: '#1f1d2e',
    accent: '#ebbcba',
    accentLight: '#f6c177',
    text: '#e0def4',
    textDim: '#908caa',
    border: '#26233a'
  }
];

export default function ThemeShowcase() {
  const [theme, setTheme] = useState<ThemePreset>(THEMES[0]);

  return (
    <section id="themes" className="section">
      <div className="text-center mb-12">
        <p className="section-kicker">// QT6 / QML GUI</p>
        <h2 className="text-3xl sm:text-4xl md:text-5xl font-bold text-white mb-4">
          11 Pixel-Crafted Themes
        </h2>
        <div className="section-divider"></div>
        <p className="text-text-muted text-base sm:text-lg max-w-2xl mx-auto">
          The <code className="text-cyan font-mono">sweep-qml</code> desktop shell is a Qt6/QML app that renders on Qt's software scene graph by default, so it launches even where OpenGL is missing or broken. Set <code className="text-cyan font-mono">SWEEP_GPU=1</code> to opt into the accelerated path. Switch themes below.
        </p>
      </div>

      {/* Theme Selection Tabs */}
      <div className="flex flex-wrap justify-center gap-2 mb-10 max-w-5xl mx-auto">
        {THEMES.map((t) => (
          <button
            key={t.id}
            onClick={() => setTheme(t)}
            className={`flex items-center gap-2 px-3.5 py-2 rounded-xl text-xs font-medium transition-all cursor-pointer ${
              theme.id === t.id
                ? 'bg-bg-elevated text-white border-2 border-cyan shadow-[0_0_20px_rgba(127,219,202,0.2)] scale-105'
                : 'bg-bg-surface/70 text-text-muted hover:text-text-primary hover:bg-bg-surface border border-border/60'
            }`}
          >
            <span
              className="w-3 h-3 rounded-full inline-block shadow-sm"
              style={{ backgroundColor: t.accent }}
            />
            <span>{t.name}</span>
          </button>
        ))}
      </div>

      {/* Mock sweep-qml Window Frame */}
      <div
        className="max-w-4xl mx-auto rounded-2xl overflow-hidden border shadow-2xl transition-all duration-300"
        style={{
          backgroundColor: theme.bg,
          borderColor: theme.border,
          color: theme.text
        }}
      >
        {/* GUI Header Bar */}
        <div
          className="px-4 py-3 flex items-center justify-between border-b"
          style={{ backgroundColor: theme.surface, borderColor: theme.border }}
        >
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-1.5">
              <span className="w-3 h-3 rounded-full bg-[#ff5555] inline-block opacity-80" />
              <span className="w-3 h-3 rounded-full bg-[#f1fa8c] inline-block opacity-80" />
              <span className="w-3 h-3 rounded-full bg-[#50fa7b] inline-block opacity-80" />
            </div>
            <span className="font-mono text-xs font-bold" style={{ color: theme.accent }}>
              sweep-qml · {theme.name}
            </span>
          </div>

          <div className="flex items-center gap-2 text-[11px] font-mono">
            <span className="px-2 py-0.5 rounded" style={{ backgroundColor: theme.bg, color: theme.textDim }}>
              Qt 6 · software
            </span>
            <span className="px-2 py-0.5 rounded" style={{ backgroundColor: theme.accent, color: theme.bg, fontWeight: 'bold' }}>
              13 screens
            </span>
          </div>
        </div>

        {/* GUI Two-Column Mock Body */}
        <div className="grid md:grid-cols-12 min-h-[340px]">
          {/* Mock Sidebar Navigation */}
          <div
            className="md:col-span-4 p-4 border-r space-y-1.5"
            style={{ backgroundColor: theme.surface, borderColor: theme.border }}
          >
            <div className="text-[10px] font-mono uppercase font-bold px-2 mb-2" style={{ color: theme.textDim }}>
              Navigation
            </div>
            {[
              { label: '▦ Dashboard', active: true },
              { label: '◈ Cleaner', active: false },
              { label: '▤ Files', active: false },
              { label: '◍ Disk', active: false },
              { label: '◎ Memory', active: false },
              { label: '▶ Startup', active: false },
              { label: '◫ System Health', active: false },
              { label: '⇄ Network', active: false },
              { label: '◉ Privacy', active: false },
              { label: '◷ Schedule', active: false },
              { label: '⚙ Settings', active: false }
            ].map((item, idx) => (
              <div
                key={idx}
                className="px-3 py-2 rounded-lg text-xs font-medium flex items-center justify-between transition-colors"
                style={{
                  backgroundColor: item.active ? theme.bg : 'transparent',
                  color: item.active ? theme.accent : theme.textDim,
                  borderLeft: item.active ? `3px solid ${theme.accent}` : 'none'
                }}
              >
                <span>{item.label}</span>
                {item.active && (
                  <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: theme.accent }} />
                )}
              </div>
            ))}
          </div>

          {/* Mock Main Dashboard View */}
          <div className="md:col-span-8 p-6 space-y-5">
            {/* Storage Progress Card */}
            <div
              className="p-4 rounded-xl border"
              style={{ backgroundColor: theme.surface, borderColor: theme.border }}
            >
              <div className="flex justify-between items-center mb-2">
                <div>
                  <div className="text-xs font-semibold" style={{ color: theme.text }}>NVMe Main Partition (/)</div>
                  <div className="text-[11px]" style={{ color: theme.textDim }}>342 GB used of 512 GB</div>
                </div>
                <div className="text-sm font-mono font-bold" style={{ color: theme.accent }}>
                  66.8%
                </div>
              </div>

              {/* Progress bar */}
              <div className="w-full h-2.5 rounded-full overflow-hidden" style={{ backgroundColor: theme.bg }}>
                <div
                  className="h-full rounded-full transition-all duration-500"
                  style={{ width: '66.8%', backgroundColor: theme.accent }}
                />
              </div>
            </div>

            {/* Quick Stats Grid */}
            <div className="grid grid-cols-2 gap-3">
              <div
                className="p-3 rounded-xl border"
                style={{ backgroundColor: theme.surface, borderColor: theme.border }}
              >
                <div className="text-[10px] uppercase font-mono" style={{ color: theme.textDim }}>Reclaimable Cache</div>
                <div className="text-lg font-bold font-mono mt-1" style={{ color: theme.accent }}>
                  4.82 GB
                </div>
                <div className="text-[10px] mt-0.5" style={{ color: theme.textDim }}>Apt, Thumbnails, Docker</div>
              </div>

              <div
                className="p-3 rounded-xl border"
                style={{ backgroundColor: theme.surface, borderColor: theme.border }}
              >
                <div className="text-[10px] uppercase font-mono" style={{ color: theme.textDim }}>Dev Artifacts</div>
                <div className="text-lg font-bold font-mono mt-1" style={{ color: theme.accentLight }}>
                  2.91 GB
                </div>
                <div className="text-[10px] mt-0.5" style={{ color: theme.textDim }}>node_modules, target</div>
              </div>
            </div>

            {/* Action Bar */}
            <div className="flex items-center justify-between pt-2">
              <span className="text-xs font-mono" style={{ color: theme.textDim }}>
                202 cleaners active · Zero telemetry
              </span>

              <div className="flex gap-2">
                <button
                  className="px-4 py-2 rounded-lg text-xs font-semibold shadow-sm transition-transform active:scale-95 cursor-pointer"
                  style={{ backgroundColor: theme.accent, color: theme.bg }}
                >
                  Preview Cleanup
                </button>
                <button
                  className="px-4 py-2 rounded-lg text-xs font-semibold border transition-transform active:scale-95 cursor-pointer"
                  style={{ borderColor: theme.accent, color: theme.accent, backgroundColor: 'transparent' }}
                >
                  Quick Scan
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="text-center mt-8 text-xs text-text-dim">
        <span>Want to read the full GUI documentation? </span>
        <a href="/docs/gui" className="text-cyan hover:underline font-medium">Explore GUI &amp; Themes Guide →</a>
      </div>
    </section>
  );
}
