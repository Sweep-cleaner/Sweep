#!/bin/bash
# Head-to-head: BleachBit vs Sweep on identical fixtures, Stacer headless probe.
set -u
cd /bench

echo "########## versions ##########"
bleachbit --version 2>&1 | head -1
sweep --version
dpkg -l stacer 2>/dev/null | tail -1 || echo "stacer: NOT INSTALLED"

echo "########## cleaner inventory ##########"
echo -n "bleachbit cleaners: "; bleachbit --list-cleaners 2>/dev/null | wc -l
echo -n "sweep cleaners:     "; sweep list 2>/dev/null | head -1

echo "########## BLEACHBIT (fixture 1) ##########"
/bench/fixture.sh
BB_CLEANERS="$(bleachbit --list-cleaners 2>/dev/null | grep -iE '^(system\.tmp|thumbnails\.cache)$' | tr '\n' ' ')"
echo "using cleaners: $BB_CLEANERS"
echo "--- preview ---"
time bleachbit --preview $BB_CLEANERS 2>&1 | tail -6
echo "--- clean ---"
time bleachbit --clean $BB_CLEANERS 2>&1 | tail -4
echo "leftover tmp: $(ls /tmp/sweep-bench-*.tmp 2>/dev/null | wc -l) files"
echo "leftover thumbs: $(find "$HOME/.cache/thumbnails" -type f 2>/dev/null | wc -l) files"

echo "########## SWEEP (fixture 2, identical) ##########"
/bench/fixture.sh
echo "--- preview ---"
time sweep preview system.tmp thumbnails firefox.cache evolution.cache browsers.chrome_cache 2>&1 | tail -14
echo "--- clean ---"
time sweep --yes clean system.tmp thumbnails firefox.cache evolution.cache browsers.chrome_cache 2>&1 | tail -12
echo "leftover tmp: $(ls /tmp/sweep-bench-*.tmp 2>/dev/null | wc -l) files"
echo "leftover thumbs: $(find "$HOME/.cache/thumbnails" -type f 2>/dev/null | wc -l) files"
echo "leftover ff: $(find "$HOME/.mozilla/firefox" "$HOME/.cache/mozilla" -type f 2>/dev/null | wc -l) files"
echo "leftover evo: $(find "$HOME/.cache/evolution" -type f 2>/dev/null | wc -l) files"
echo "leftover chrome: $(find "$HOME/.config/google-chrome" -type f 2>/dev/null | wc -l) files"

echo "########## STACER headless probe ##########"
which stacer || echo "no stacer binary on PATH"
timeout 25 xvfb-run -a stacer --no-sandbox > /tmp/stacer.log 2>&1
echo "stacer exit=$? (124=timeout/killed, 0=exited)"
tail -5 /tmp/stacer.log
echo "########## done ##########"
