// The parity list of the two players, as data: the behaviour inventory of
// .agents/plans/embedded-player.md ("Behaviour inventory (the parity list)")
// and the recorded exemptions. The scenarios say which rows they stand for on
// their own default export (`rows`; tests/e2e/README.md, "What a scenario
// declares"); tests/e2e-parity.test.mjs puts the two together and fails when a
// row is shown in one player and neither shown nor exempted in the other.
//
// Nothing here is read by the runner except PLAYERS and the row ids, which the
// shared loader (scenario-loader.mjs) checks every scenario against.

export const PLAYERS = ['external', 'embedded'];

// A row is split where its halves are proven differently in the two players.
export const INVENTORY = [
  { id: '1', behaviour: 'Play starts; resume at start_seconds' },
  { id: '2', behaviour: 'Plex timeline and Jellyfin/Emby check-ins, final stop' },
  { id: '3', behaviour: 'Final position on quit; recents stamp; playback-ended' },
  { id: '4', behaviour: 'Clean end -> watched, Continue Playing modes' },
  { id: '5', behaviour: 'Playlist and server-playlist advance' },
  { id: '6', behaviour: 'Source choice / policy' },
  { id: '7', behaviour: 'Stream auth header never in argv/URL/title' },
  { id: '8a', behaviour: 'Skip markers: automatic skip' },
  { id: '8b', behaviour: 'Skip markers: the skip button' },
  { id: '9', behaviour: 'Autocrop off / manual / auto' },
  { id: '10', behaviour: 'Automatic quality step-down relaunch' },
  { id: '11', behaviour: 'Unrequested-play notice' },
  { id: '12a', behaviour: 'A new play stops the previous one' },
  { id: '12b', behaviour: 'App exit stops playback' },
  { id: '13a', behaviour: 'User mpv arguments (Advanced)' },
  { id: '13b', behaviour: 'Own-config mode' },
  { id: '14', behaviour: 'Ends at end of file rather than idling' },
];

export const ROW_IDS = INVENTORY.map((row) => row.id);

// The plan's rows that are split, and into exactly which parts (the plan's
// Slice 6 log: "Three of the plan's rows are split where their halves are
// proven differently (8, 12, 13)"). The inventory must hold every part of a
// split row and no other part, and an unsplit row whole, so that half of a
// row cannot be dropped, or a part made up, by editing INVENTORY alone
// (scenario-loader.mjs, inventoryProblems; review finding pa-3).
export const SPLITS = {
  8: ['8a', '8b'],
  12: ['12a', '12b'],
  13: ['13a', '13b'],
};

// Scenarios that fail in a player because of a recorded PRODUCT defect, which
// this test work may not fix. Such a scenario does NOT show its rows in that
// player: a row/player pair whose only scenarios are known failures is printed
// as RED by the parity test, never as shown. `failsWith` is the exact text the
// failure carries, so that a different failure of the same scenario is not
// taken for this one; `defect` is the evidence. `intermittent: true` marks a
// race that does not fail every run: a pass is then accepted too, never a
// different failure. Remove an entry once the product is fixed and the
// scenario passes.
export const KNOWN_FAILURES = [
  {
    scenario: 'embeddedownconfig',
    player: 'external',
    failsWith: "Vela's own idle setting stands, whatever the profile Vela selects has been made to include",
    defect:
      "In the external player's own-configuration mode, a user's mpv.conf that redefines the gpu-hq profile to include a file saying idle=yes wins over Vela's --idle=no, so the player idles at the end of the file and the play is never finished. Vela's external command line ends with --include=<stream-header file> after --idle=no (src-tauri/src/player/external.rs, by reading), and mpv applies the profile's include after it. Reproduced with mpv 0.41 alone: MPV_HOME=<that mpv.conf> mpv --no-ytdl --vo=null --profile=gpu-hq --idle=no --include=<any file> <clip> idles; without the trailing --include it exits at the end. The same hazard was fixed for the embedded player only (review finding ec-4). Failed every external run of Slice 6 (7 of 7 and more).",
  },
  {
    scenario: 'pagefail',
    player: 'external',
    intermittent: true,
    failsWith: 'precondition: back on the truncated grid, page 2 never arrived',
    defect:
      "The grid's onScroll in src/routes/+page.svelte loads the next page from a scroll event WebKit delivers after the grid has left the DOM (all sizes 0), so the page the scenario failed on purpose is fetched again. Recorded in .agents/state.md (E2E pagefail); fix needs a plan and a go.",
  },
  {
    scenario: 'pagefail',
    player: 'embedded',
    intermittent: true,
    failsWith: 'precondition: back on the truncated grid, page 2 never arrived',
    defect: 'As in the external player: the same product race (.agents/state.md, E2E pagefail). It passed in the full embedded run of 6787b9f, so it is a race in this player too, not a failure of every run.',
  },
];

// A row and player that no scenario of the suite shows, and why. An exemption
// is not coverage: the parity test lists every one of them each time it runs.
// Each says why no scenario can show it in this venue without a product
// change (`reason`), and names what holds it instead: tests of other kinds
// (`heldBy`: the file and the test names; the parity test checks that the
// file exists and that each name occurs in it) and/or the step the owner takes by
// hand (`ownerStep`). An exemption for a row a scenario already shows in that
// player is refused as stale.
export const EXEMPTIONS = [
  {
    row: '10',
    player: 'external',
    reason:
      'A step-down needs a health verdict, which only genuinely degraded playback produces; nothing in this venue can degrade a play, and Vela has no development hook that forces a verdict (tests/e2e/live/transcode.mjs says the same of the live venue: it does not test Automatic). Adding such a hook is a product change.',
    heldBy: [
      {
        file: 'src-tauri/src/automatic.rs',
        tests: [
          'a_sustained_drop_storm_steps_down',
          'a_starving_cache_steps_down',
          'healthy_playback_never_steps_down',
          'a_replacement_stream_gets_the_longer_cooldown',
          'stepping_stops_at_the_cap',
          'the_step_count_survives_a_relaunch',
        ],
      },
      {
        file: 'src-tauri/src/commands.rs',
        tests: ['a_step_down_replacement_is_still_managed_by_automatic', 'a_newer_verdict_replaces_an_unhandled_one'],
      },
      { file: 'src-tauri/src/player/select.rs', tests: ['an_automatic_relaunch_neither_explains_nor_forgets'] },
    ],
    ownerStep:
      'Play a Plex film at quality Automatic in the external player over a link slow enough to starve it (or throttled), and see the play relaunched at a lower quality at about the same position, with the short quality notice on mpv\'s screen.',
  },
  {
    row: '10',
    player: 'embedded',
    reason:
      'As for the external player: no venue can force a verdict. The verdict, the ladder and the relaunch are the same code for both players (playback::play starts the health sampler on the IPC socket of whichever player runs; apply_step_down relaunches in the player it replaces), so the unit tests listed hold the decision for both; what they do not reach is an embedded play being stopped and created again by a step-down.',
    heldBy: [
      { file: 'src-tauri/src/automatic.rs', tests: ['a_sustained_drop_storm_steps_down', 'a_starving_cache_steps_down'] },
      { file: 'tests/player-selection.test.mjs', tests: ['a relaunch stays in the player it replaces'] },
      {
        file: 'tests/player-view.test.mjs',
        tests: ["a replaced play's late close does not dismiss the view of the play that replaced it"],
      },
    ],
    ownerStep:
      'As for the external player, with the embedded player selected: the relaunch must stay in the embedded player, the player view must stay up across it, and the play must go on at about the same position.',
  },
  {
    row: '11',
    player: 'external',
    reason:
      'The unrequested-play notice is shown for a play the user did not ask for, which is a step-down relaunch (src-tauri/src/playback.rs, osd_notice), and no venue can force a step-down (row 10). The D3 fallback notice that `playerfallback` checks is a different notice, for a play the user did ask for; it rides the same delivery (commands.rs, `osd_notice.or(fallback_notice)`; spawn_osd_notice sends `show-text` over the IPC socket), so that scenario is evidence that the delivery reaches the external mpv once per play, not a proof of this row.',
    heldBy: [
      {
        file: 'src-tauri/src/playback.rs',
        tests: [
          'a_notice_that_reaches_the_player_is_reported_shown_once',
          'a_notice_dropped_because_the_play_was_replaced_is_not_reported_shown',
          'a_notice_the_player_did_not_take_is_not_reported_shown',
          'a_notice_for_a_player_that_cannot_be_reached_is_not_reported_shown',
        ],
      },
      { file: 'src-tauri/src/commands.rs', tests: ['the_step_down_notice_stays_short'] },
      { file: 'tests/e2e/scenarios/playerfallback.mjs', tests: ['playerfallback'] },
    ],
    ownerStep:
      'With the external player, provoke a step-down (row 10) and see its short quality notice on mpv\'s screen for about two seconds, once.',
  },
  {
    row: '11',
    player: 'embedded',
    reason:
      'As in the external player: the notice belongs to a step-down relaunch, which no venue can force (row 10). The delivery itself is player-agnostic (spawn_osd_notice sends `show-text` over the IPC socket of whichever player runs, and the library draws mpv\'s OSD), and its rules are held by the tests listed, against a test socket. Unlike the external player, no scenario even shows the delivery reaching the embedded player: the only other notice, the D3 fallback one, by definition plays in the external player.',
    heldBy: [
      {
        file: 'src-tauri/src/playback.rs',
        tests: [
          'a_notice_that_reaches_the_player_is_reported_shown_once',
          'a_notice_dropped_because_the_play_was_replaced_is_not_reported_shown',
          'a_notice_the_player_did_not_take_is_not_reported_shown',
          'a_notice_for_a_player_that_cannot_be_reached_is_not_reported_shown',
        ],
      },
      { file: 'src-tauri/src/commands.rs', tests: ['the_step_down_notice_stays_short'] },
    ],
    ownerStep:
      'With the embedded player selected, provoke a step-down (row 10) and see its short quality notice drawn over the picture in Vela\'s window for about two seconds.',
  },
];
