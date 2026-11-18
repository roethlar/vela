# Reviewer's report, verbatim: ios plan, design round 2

Dispatch: label=r2-ios base=fe7fea2 head=16c787e start=2026-11-18T15:28:16Z end=2026-11-18T15:32:36Z rc=0

Both capability checks succeeded. I read `AGENTS.md`, and `git rev-parse 16c787e` returned `16c787e66a3bd1f594466796fc4d95b5a9946b8b`.

**Six repairs close their recorded design failures. Four test-only rows close under the owner’s ruling. Two new findings affect iPhone memory management.** `git diff --check fe7fea2 16c787e` passed.

### Pending iPhone rows

| Row | Verdict | Reason at `16c787e` |
|---|---|---|
| iph-3 | closed | I5 adopts shared rows 15–17, including Dolby Vision 8.4, conditional profile 10, live profile 7 conversion, the minimum OS, and route-specific audio results. A result the available phone, OS, or route cannot establish stays pending. [iPhone plan:208](/Users/michael/Dev/vela/.agents/plans/ios-port.md:208) · [Pending results:225](/Users/michael/Dev/vela/.agents/plans/ios-port.md:225) |
| iph-8 | closed | An iPad-only ruling leaves the phone decision open before I0 enables the family. The shared plan blocks the slice that would hide the feature until the owner rules. [iPhone plan:61](/Users/michael/Dev/vela/.agents/plans/ios-port.md:61) · [iPad plan:114](/Users/michael/Dev/vela/.agents/plans/ipados-port.md:114) · [Approval gate:120](/Users/michael/Dev/vela/.agents/plans/ipados-port.md:120) |
| iph-12 | closed | I0 can install a development-signed build or an internal TestFlight build. Public release remains the owner’s decision after I5. [iPhone plan:172](/Users/michael/Dev/vela/.agents/plans/ios-port.md:172) · [Distribution ruling:2723](/Users/michael/Dev/vela/.agents/decisions.md:2723) |
| iph-13 | closed | I1 uses an original profile 7 remux, and I4 retains it in sustained playback with live conversion active. A preconverted file can no longer stand in for the converter’s memory load. [iPhone plan:180](/Users/michael/Dev/vela/.agents/plans/ios-port.md:180) · [I4:201](/Users/michael/Dev/vela/.agents/plans/ios-port.md:201) |
| iph-14 | closed | Exact byte and frame limits now apply to queues Vela owns. For Apple’s sample-buffer renderer, the engine acts on readiness. This closes the original demand to count Apple’s internal queue. [iPhone plan:144](/Users/michael/Dev/vela/.agents/plans/ios-port.md:144) |
| iph-15 | closed | Test-only row, closed under the [owner’s ruling:2704](/Users/michael/Dev/vela/.agents/decisions.md:2704). |
| iph-16 | closed | Each desktop parity row gets its own phone disposition. Excluding `mpvargs` leaves row 13a open until the owner rules on that feature for iPhone. [iPhone plan:193](/Users/michael/Dev/vela/.agents/plans/ios-port.md:193) · [Existing row mapping:45](/Users/michael/Dev/vela/tests/e2e/scenarios/mpvargs.mjs:45) |
| iph-17 | closed | Test-only row, closed under the [owner’s ruling:2704](/Users/michael/Dev/vela/.agents/decisions.md:2704). |
| iph-18 | closed | The binding landscape scope reaches 440 pt. The remaining test detail falls under the owner’s ruling. [iPhone scope:41](/Users/michael/Dev/vela/.agents/plans/ios-port.md:41) · [Compact layout:94](/Users/michael/Dev/vela/.agents/plans/ios-port.md:94) |
| iph-19 | closed | Test-only row, closed under the [owner’s ruling:2704](/Users/michael/Dev/vela/.agents/decisions.md:2704). |

### New findings

1. **MEDIUM — The HLS loopback producer has no stated memory limit.** If the shared engine adopts the Dolby Vision loopback after P2, Vela produces fragmented MP4 segments and AVPlayer requests them over HTTP. The repaired rule names demux, decode, and subtitle queues, then relies on sample-buffer renderer readiness. That signal cannot throttle an HTTP segment producer. During a pause or slow read, Vela could retain growing segment data while satisfying every named queue limit. Give Vela’s loopback muxer and segment store explicit byte and retention limits, and throttle production when those limits are reached. Keep renderer readiness on the sample-buffer path. [iPhone buffer rule:144](/Users/michael/Dev/vela/.agents/plans/ios-port.md:144) · [iPhone loopback assumption:71](/Users/michael/Dev/vela/.agents/plans/ios-port.md:71) · [Shared fallback and decision gate:159](/Users/michael/Dev/vela/.agents/plans/ipados-port.md:159) · [Apple’s HLS request sequence](https://developer.apple.com/videos/play/wwdc2024/10113/?time=106)

2. **MEDIUM — The plan overstates what the app’s memory footprint measures.** The repair says the app footprint “includes Apple’s share” and uses that measurement to size buffers. When iOS charges playback-related work to a separate system process, Vela’s footprint alone cannot establish the combined cost. An implementer could record a stable app peak and overestimate the headroom available for Vela’s queues. Label the measurement as memory charged to Vela’s process. Where a budget decision depends on wider memory pressure, use available device and related-process evidence and leave unattributed renderer memory unknown. [iPhone plan:149](/Users/michael/Dev/vela/.agents/plans/ios-port.md:149) · [Apple’s per-process memory accounting and related-process guidance](https://developer.apple.com/documentation/xcode/identifying-high-memory-use-with-jetsam-event-reports)