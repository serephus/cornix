# cornix

Small from-scratch Rust firmware for the Corne v4.1 (RP2040).

- One UF2 for both halves; on v4.1 either half can be the main board.
- v4.1 split LED data is sent only when it changes; ordinary polls stay small.
- 42 keys; the 4 extra keys are ignored.
- Static Norman keymap in `src/keymap.rs`.
- Per-key backlight scaled by lifetime key frequency.
- Persistent counters in the last 16 KB of flash.
- Green `#00ff30` lighting, total-normalized with `B_MAX = 40`.

## Build

Requires Nix with flakes.

```sh
# v4.1 default
nix run .#uf2

# explicit v4.1
nix run .#uf2-v41
```

Outputs:

```text
target/thumbv6m-none-eabi/release/cornix-v41.uf2
```

v4.0 fallback:

```sh
nix run .#uf2-v40
```

```text
target/thumbv6m-none-eabi/release/cornix-v40.uf2
```

## Flash

Enter the RP2040 bootloader on the half you are flashing:

- hold its bootmagic key while plugging USB (left: Escape; right: Backspace, the outermost top-row key)
- or double-tap RESET
- or hold the physical BOOT button

Then:

```sh
nix shell nixpkgs#picotool -c picotool load -x -v \
  target/thumbv6m-none-eabi/release/cornix-v41.uf2
```

Flash the same file to both halves.

On v4.1, the half with USB plugged in becomes the main/central board; plug only one half into USB. The v4.0 fallback still uses the left half as main, so use the left USB port.

Counters are stored on the active main half. Switching which half is main uses that half's stored counters; they are not merged across halves.

## Keymap

Edit `src/keymap.rs`, then rebuild. Layers 0–3 are used; layers 4–5 and tap dance are ignored.

## Configuration

Edit `src/config.rs`:

- `B_MAX` — maximum per-key brightness
- `LED_COLOR` — RGB color
- `HALF_DUPLEX_BAUD` — v4.1 split speed
- `HALF_DUPLEX_TURNAROUND_US` — v4.1 poll turnaround delay (default 1000)
- `FLUSH_INTERVAL_SECS` — counter save interval

## Recovery

Official Corne Vial recovery UF2s are used from the local, untracked `recovery/` directory. Flash the matching revision with `picotool`, then load your Vial keymap.
