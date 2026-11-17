// Probe M2: evaluated in Vela's page by the probe's hook (hook.py). Plays the
// mock library's movie in Vela's own window twice, as the end-to-end
// scenarios' openLibraryGrid and openDetailAndPlay do, closes each play from
// the page, and reports every step as a "vela-probe" event, which the hook
// prints as "probe: ...". The last note is DONE or ERROR.
(async () => {
  const note = (text) =>
    window.__TAURI_INTERNALS__
      .invoke('plugin:event|emit', { event: 'vela-probe', payload: String(text) })
      .catch(() => {});
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const until = async (test, what, ms = 30000) => {
    const end = Date.now() + ms;
    for (;;) {
      let value = null;
      try {
        value = test();
      } catch {
        value = null;
      }
      if (value) return value;
      if (Date.now() > end) throw new Error(`timed out waiting for ${what}`);
      await sleep(200);
    }
  };
  const page = () => ({
    open: document.documentElement.classList.contains('player-open'),
    view: !!document.querySelector('.player'),
    body: getComputedStyle(document.body).backgroundColor,
    html: getComputedStyle(document.documentElement).backgroundColor,
    size: [innerWidth, innerHeight, devicePixelRatio],
    text: (document.querySelector('.player')?.innerText ?? '').replace(/\s+/g, ' ').slice(0, 160),
    rect: (() => {
      const r = document.querySelector('.player')?.getBoundingClientRect();
      return r ? [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)] : null;
    })(),
  });
  const playOnce = async (label, seconds) => {
    const play = await until(() => document.querySelector('.detail button.playwide'), `the Play button (${label})`);
    play.click();
    note(`${label}: Play clicked`);
    await until(() => page().open && page().view, `the player view (${label})`, 20000);
    note(`${label}: PLAYER OPEN ${JSON.stringify(page())}`);
    for (let second = 1; second <= seconds; second += 1) {
      await sleep(1000);
      note(`${label}: ${second} s ${JSON.stringify(page())}`);
    }
    const close = await until(() => document.querySelector('.player button.close'), `the Close button (${label})`);
    close.click();
    note(`${label}: Close clicked`);
    await until(() => !page().open && !page().view, `the player view to close (${label})`, 15000);
    note(`${label}: PLAYER CLOSED ${JSON.stringify(page())}`);
  };
  try {
    note(`script running, ${document.readyState}, ${JSON.stringify(page())}`);
    const side = await until(
      () => [...document.querySelectorAll('button.sideitem')].find((b) => b.textContent.trim() === 'Mock Library'),
      'the mock library in the sidebar',
    );
    side.click();
    const card = await until(() => document.querySelector('button.poster[aria-label^="Mock Movie"]'), 'the movie card');
    card.click();
    await playOnce('first play', 6);
    await sleep(2000);
    await playOnce('second play', 3);
    note('DONE');
  } catch (error) {
    note(`ERROR ${error?.message ?? error} ${JSON.stringify(page())}`);
    note(`page text: ${document.body.innerText.replace(/\s+/g, ' ').slice(0, 400)}`);
  }
})();
