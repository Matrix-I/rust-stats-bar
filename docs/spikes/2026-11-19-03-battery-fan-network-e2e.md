# Plan 03 — battery, fan and network end-to-end run

Run on 2026-11-21 on Ubuntu 26.04.1 LTS, GNOME Shell 50.1, Wayland, against the release build of
`statsbard` from [the Plan 03 plan](../plans/2026-11-19-rust-statsbar-03-battery-fan-network.md).
The daemon ran under `strace -f -tt -T -y -s 64 -e trace=pread64,openat,close` with
`XDG_CONFIG_HOME` set to a scratch directory, so the user's `~/.config` was untouched.

Clicks were sent as dbusmenu `Event` calls over `busctl --user`. The power profile was changed
with `powerprofilesctl`. The user only looked at the panel and pulled and replaced the power cord.

The run took two sessions with the same build. In session 1 (PID 43689), the user stopped at the
panel check because the labels were hard to read, and only the battery, fan, thread and quit
checks ran. Session 2 (PID 58354) ran every check. The raw logs are kept outside Git.

## Results

| Check | Session | Expected | Observed |
|---|---|---|---|
| Items | 1, 2 | Hub plus five items | Six items: `statsbar-{hub,cpu,memory,battery,fan,network}` |
| Panel legibility | 1 | Each label legible | **Not met.** Every label, CPU and RAM included, reads as small and broken-looking at 125 % scale; see [spike S3](2026-11-21-s3-panel-legibility.md). The order GNOME showed the items in was not recorded |
| Battery menu | 1, 2 | Matches `BAT0/uevent` and `upower -i` | `Charge 80 %`, `Status Not charging`, `Health 65 %` (5803 / 8948), `Capacity 5803 / 8948 mAh`, `Voltage 8.06 V`, `Adapter connected`; no `Cycles` row while `CYCLE_COUNT` is 0 |
| Unplug | 2 | Within 2 s: `Discharging`, `Adapter not connected`, then `Time left` | Changed at 10:29:06; `Time left 2h 14m` and `Power 16.3 W` from 10:29:07 |
| Replug | 2 | `CHG` below the 75 % start threshold, `BAT` otherwise | `Not charging` and `Adapter connected` at 10:29:26. The charge was 80 %, so the label stayed `BAT 80 %`: the pixmap hash did not change during the run. `CHG` was not seen in the panel; it is covered only by `the_battery_label_says_chg_only_while_charging` |
| Fan | 1, 2 | Speed within one sample of `fan1_input`; label `min(100, round(rpm / 5300 × 100))` | Session 1: 4996 RPM against 5002, label 94 %. Session 2: 4108 RPM, label 78 % |
| Fan reads | 2 | `pread64` on `fan1_input` every 2 s; none on `fan1_min` or `fan1_max` after open | 2.0 s apart; `fan1_min` and `fan1_max` read only at open (10:20:39) and when shown again (10:25:43) |
| Network rate | 2 | Mean of five titles over 10 s within 5 % of the `rx_bytes` rate | **Not met as planned:** 6.2 %. Met with tick-aligned sampling instead: +2.0 % over 10 ticks. See below |
| Hide | 2 | Item vanishes; its files closed; no read for 10 s; `false` in the config | All three. `close` on `BAT0/uevent` and `AC/online` at 10:24:27, on `fan1_input`, `/proc/net/route`, `rx_bytes` and `tx_bytes` at 10:24:57; no `pread64` on them until each was shown again: battery reopened at 10:25:29.7 (62 s later), network at 10:25:29.8 (33 s), fan at 10:25:43.4 (46 s); the config read `battery = false`, `fan = false`, `network = false` |
| Show again | 2 | Item back within 2 s; network totals restart | All three back within 2 s, on new bus names; `Downloaded` went from 2.0 GB to 23.5 KB |
| Power saver | 2 | Battery reads 5 s apart | `uevent` read at 10:25:53.7, 58.7, 03.7 and 08.7; back to 2 s, starting with an immediate read at 10:26:11.8, after `performance` was restored |
| Threads | 1, 2 | Same four as Plan 02b | 4 threads: `statsbard` ×2, `async-io`, `blocking-1`. Rss 5044 kB, Pss 2997 kB (Plan 02b: 2657 kB) |
| Quit | 1, 2 | Items vanish; exit code 0 | No item left at the watcher; exit code 0 |
| stderr | 1, 2 | Empty | Empty |

The power profile was `performance` before the run and was restored to it.

### Network rate

A download of `ubuntu-24.04.4-desktop-amd64.iso` from `releases.ubuntu.com` ran over `wlo1`. The
user consented to it.

The first attempt followed the plan: it read the title every 2 s from an arbitrary start. Its five titles averaged
32.1 MB/s, against 30.2 MB/s from `rx_bytes` over the same 10.07 s, a gap of 6.2 %. The daemon's
ticks were not aligned with the window, while the rate swung from 24 to 37 MB/s. Each title and
`Download` row still matched each other.

The plan's check therefore failed. The second attempt, a change of method, polled the menu every 50 ms and read `rx_bytes` as soon as the rows changed,
so both sides cover the same ticks. Over 10 ticks, with the rate swinging from 2.1 to 41.4 MB/s,
the titles averaged 12.75 MB/s against 12.50 MB/s from the counter: **+2.0 %**. Each tick matched on
its own, for example 22.05 MB over 2.03 s (10.86 MB/s) against the row's `10.9 MB/s`.

The aligned loop, in outline (`bus` is the network item's `org.kde.StatusNotifierItem-<pid>-<n>`):

```python
while len(marks) < 12 and time.monotonic() < deadline:
    # Interface, Download, Upload, Downloaded, Uploaded
    rows = labels(busctl("call", "--", bus, "/MenuBar", "com.canonical.dbusmenu",
                         "GetLayout", "iias", "0", "-1", "0"))
    if rows[1:] != last:  # the daemon just sampled
        rx = json.loads(run("ip", "-s", "-j", "link", "show", "wlo1"))[0]["stats64"]["rx"]["bytes"]
        marks.append((time.monotonic(), rx, rows[1]))
        last = rows[1:]
    time.sleep(0.05)
```

The first mark is dropped, since polling starts mid-tick. Each remaining mark's `Download` rate is
compared with the `rx` delta since the previous mark.

### Read times

From `-T` in session 2, over about 250 reads each:

| File | Mean | Max |
|---|---|---|
| `BAT0/uevent` | 12.96 ms | 118.57 ms |
| `AC/online` | 1.62 ms | 20.96 ms |
| `fan1_input` | 1.01 ms | 6.69 ms |
| `statistics/rx_bytes` | 0.04 ms | 1.06 ms |
| `/proc/net/route` | 0.04 ms | 1.32 ms |

Session 1 measured `BAT0/uevent` over 90 reads at a mean of 14.59 ms and a max of 95.08 ms. Its first
five fan reads took 0.69, 0.70, 0.67, 1.98 and 0.74 ms (10:01:57.9 to 10:02:05.9). The plan expected battery reads of up to 9 ms; they run on the main loop, so a
slow `uevent` delays every label on that tick by as much.

## Notes

- After the power cord went back in, the driver dropped `CURRENT_NOW` and `POWER_NOW` from
  `uevent`, so the `Power` row vanished. At the start of the run, with the cord in, it read
  `Power 0.0 W`. Both follow what the driver reports.
- Hiding or showing an item adds or removes a hub row. `ksni` then renumbers every menu id, so a
  scripted click sent with an id read before the change failed with `id not found`. The panel
  rereads the layout after `LayoutUpdated`. The hub has behaved this way since Plan 02b. In this
  run, every click sent with an id read after the last change worked.
- An item shown again registers on a new bus name, so GNOME may place it in a different position.
