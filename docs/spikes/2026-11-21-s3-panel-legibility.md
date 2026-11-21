# Spike S3 — tray label legibility at 125% scale

Run on 2026-11-21 on Ubuntu 26.04.1 LTS, GNOME Shell 50.1, Wayland, with Ubuntu AppIndicators
(`gnome-shell-ubuntu-extensions` 50.26.04.8ubuntu). The built-in 1920×1080 display ran at a
logical scale of 1.25, with Mutter's `scale-monitor-framebuffer` on. The extension's `icon-size`
was 0, so it used the default of 16 logical pixels.

Screenshots came from the Desktop Screenshot portal, at 1920×1080 device pixels. The panel crops
are kept outside Git.

## Why the labels look broken

During the Plan 03 end-to-end run, the user reported every label as small and broken-looking. That
included CPU and RAM, which Plan 03 did not change. The same happened with the `main` build
(297d8aa), so Plan 03 did not cause it.

The extension draws a pixmap item in a box of `icon-size × scale_factor` logical pixels. Under
fractional scaling, `scale_factor` is 1, so the box is 16 logical pixels, or 20 device pixels.
`PixmapsUtils.getBestPixmap` asks for at least `16 × ceil(1.25) = 32` pixels. When no pixmap is
that large, it takes the largest one offered, which here is the 20×20 label.

The label's ink measured about 20 device pixels, so it is drawn at 1:1. Each 1-pixel stroke still
covered two device columns, at about 80 and 180 out of 255. Full white (255) did not appear in the
CPU label. This pattern means the box sits at a fractional device position and is sampled
bilinearly. Our pixmap is correct; the blur comes from the shell.

## Larger pixmaps

The probe `spikes/tray/src/bin/sharp.rs` drew `label::render(Cpu, 37)` and
`label::render_rates(12 000, 1 500 000)`, scaled up by pixel duplication.

"Sharp" is the share of ink pixels at 230 or above, among those at 230 or above plus those from 40
to 200.

| Variant | Offered | Sharp |
|---|---|---|
| A, CPU as shipped | 20×20 | 0.02 |
| B, CPU ×2 | 40×40 | 0.09 |
| C, CPU ×4 | 80×80 | 0.29 |
| D, CPU ×4, strokes one texel wider | 80×80 | 0.50 |
| E, network as shipped | 20×20 | 0.07 |
| F, network ×4 | 80×80 | 0.34 |

The larger pixmaps were sharper. The user still found them too small to read, because the box stays
20 device pixels. Variant D was the only bold attempt. A bolder 20×20 font was not drawn, since
even the sharpest variant was too small to read.

Plan 03b removed the renderer and this probe. To rerun it, check out a30a747 and build
`spikes/tray` there.

## Text drawn by the shell

The extension reads the optional `XAyatanaLabel` property and shows it as an `St.Label` next to the
icon, in the panel font. The probe `spikes/tray/text_label_probe.py` (Python and Gio, no new
packages) registered items that set it. The extension refused an item without `Menu`. Once the
probe set every StatusNotifierItem property, the items appeared.

| Variant | User's verdict |
|---|---|
| `CPU 37%` and `↑12K ↓1.5M`, text only | Legible but too wide; the network item pushed into the clock |
| A Yaru symbolic icon and a short number: `computer-chip-symbolic` + `37%`, `drive-harddisk-solidstate-symbolic` + `36%`, `network-transmit-receive-symbolic` + `1.5M` | **Chosen** |
| One combined item, `CPU 37  RAM 36  ↓1.5M` | Not chosen |

The text looked as crisp as the battery indicator's own `80%`.

## Constraint

`ksni` 0.3.7 defines the `org.kde.StatusNotifierItem` interface itself, with a fixed property
list. It cannot publish `XAyatanaLabel`. Using the chosen design needs either a patched `ksni`
or our own StatusNotifierItem service on `zbus`, which the daemon already depends on.

**Decision (2026-11-21):** Plan 03b publishes the label through a vendored `ksni` 0.3.7 patched
with `XAyatanaLabel` (`vendor/ksni/PATCHES.md`), and removes the pixmap labels of Plans 02b
and 03.
