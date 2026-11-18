# Spike S1 — tray label pixmap in the GNOME panel

Run on 2026-11-18 with `spikes/tray/target/release/label`. The GNOME watcher listed all five items.

**Machine:** Ubuntu 26.04.1 LTS, GNOME Shell 50.1, Wayland. Ubuntu AppIndicators was active.
The extension's metadata does not declare a version. Its Ubuntu package,
`gnome-shell-ubuntu-extensions`, is version `50.26.04.8ubuntu`. Mutter reported a display scale of
125% on the built-in 1920×1080 display.

**Question:** does a wide glyph and percentage pixmap stay wide and legible in the panel in both
styles?

| Variant | Offered pixmap(s), as printed | Dark shape | Dark legible | Light shape | Light legible |
|---|---|---|---|---|---|
| A — CPU, cycling 7/37/100%, white | 64×16 | Kept | No | Kept | No |
| B — memory, 100%, white | 128×32 | Kept | No | Kept | No |
| C — CPU, 100%, white | 64×16 and 128×32 | Kept | No | Kept | No |
| D — memory, 100%, black | 64×16 | Kept | No, low contrast | Kept | No, low contrast |
| E — CPU, 100%, white | 32×16 | Kept | No | Kept | No |

The user reported the labels were too small to read in Dark. The Light "legible" cells are not the
user's answer: they are inferred from Light captures that matched the Dark ones pixel for pixel. At
native size, the screenshots show wide, visibly unclipped ink in both styles. The ink occupied about
20–21 pixels in width and 4–6 pixels in height. The white values could be distinguished in magnified
crops. The black dots of D were barely visible next to the dark background. Matching frames showed
the same tray appearance in both styles.

Item A's width was steady across 7%, 37%, and 100%. The glyph and the right edge of the percentage
stayed in place. The top bar background was dark (`#131313`) in both styles. Dark was restored after
capture (the executor, not the user, switched the GNOME style, with the user's approval; this
departs from the plan's rule that the user switches it), and the watcher confirmed the removal of
all five S1 items.

## Dependencies

Package metadata and release dates were checked on 2026-11-18 with `cargo info` and the crates.io API.

| Dependency | Licence | Repository | Pinned release | Newest release |
|---|---|---|---|---|
| `ksni` | Unlicense | [iovxw/ksni](https://github.com/iovxw/ksni) | 0.3.7, 2026-10-20 | 0.3.7, 2026-10-20 |
| `zbus` | MIT | [z-galaxy/zbus](https://github.com/z-galaxy/zbus/) | 5.19.0, 2026-08-09 | 6.3.0, 2026-11-13 |
| `async-io` | Apache-2.0 OR MIT | [smol-rs/async-io](https://github.com/smol-rs/async-io) | 2.6.0, 2025-09-14 | 2.6.0, 2025-09-14 |
| `futures-lite` | Apache-2.0 OR MIT | [smol-rs/futures-lite](https://github.com/smol-rs/futures-lite) | 2.6.1, 2025-08-04 | 2.6.1, 2025-08-04 |

The newest releases of `async-io` and `futures-lite` were both over 12 months old.

**Audit:** `cargo audit --file spikes/tray/Cargo.lock` exited 0 on 2026-11-18. No vulnerabilities
were reported. RustSec reported three unmaintained transitive dependencies, with no fixed upgrade
listed for these packages:

- `zbus_names` 4.3.5 ([RUSTSEC-2026-0371](https://rustsec.org/advisories/RUSTSEC-2026-0371))
- `zvariant` 5.15.1 ([RUSTSEC-2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370))
- `zvariant_derive` 5.15.1 ([RUSTSEC-2026-0372](https://rustsec.org/advisories/RUSTSEC-2026-0372))

The user approved running S1 and S2 with this lockfile after reviewing the warnings.

**Decision:** “Shape kept, but no variant legible” in Plan 02a. The user chose to trial fallback 1
in Plan 02b. Test a rasterised system font for panel legibility and measure Pss before choosing a
renderer.
