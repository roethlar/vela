// Minimal W3C WebDriver client over Node's built-in fetch. The harness
// deliberately does not use WebdriverIO — see .agents/plans/e2e-harness.md
// (deviation 2026-07-05).
import fs from 'node:fs/promises';

export class WebDriverError extends Error {}

export class Driver {
  constructor(baseUrl) {
    this.baseUrl = baseUrl;
    this.sessionId = null;
  }

  async #cmd(method, path, body, { timeoutMs = 30000 } = {}) {
    const t0 = Date.now();
    let res;
    try {
      res = await fetch(`${this.baseUrl}${path}`, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: body === undefined ? undefined : JSON.stringify(body),
        signal: AbortSignal.timeout(timeoutMs),
      });
    } catch (err) {
      throw new WebDriverError(
        `${method} ${path} → ${err.name === 'TimeoutError' ? `no response within ${timeoutMs}ms` : err.cause?.code ?? err.message} (after ${Date.now() - t0}ms)`,
      );
    } finally {
      if (process.env.VELA_E2E_DEBUG) {
        console.error(`[driver] ${method} ${path} ${Date.now() - t0}ms`);
      }
    }
    const json = await res.json();
    if (!res.ok || json.value?.error) {
      throw new WebDriverError(
        `${method} ${path} → ${json.value?.error ?? res.status}: ${json.value?.message ?? ''}`,
      );
    }
    return json.value;
  }

  async newSession(applicationPath) {
    const value = await this.#cmd(
      'POST',
      '/session',
      {
        capabilities: {
          alwaysMatch: { 'tauri:options': { application: applicationPath } },
        },
      },
      { timeoutMs: 60000 }, // covers the app launch
    );
    this.sessionId = value.sessionId;
    return value;
  }

  #s(path) {
    return `/session/${this.sessionId}${path}`;
  }

  async deleteSession() {
    if (!this.sessionId) return;
    const id = this.sessionId;
    this.sessionId = null;
    await this.#cmd('DELETE', `/session/${id}`);
  }

  async exec(script, args = []) {
    return this.#cmd('POST', this.#s('/execute/sync'), { script, args });
  }

  // Polls `script` until it returns a truthy value, which is returned.
  async waitFor(script, what, { timeoutMs = 15000, intervalMs = 250 } = {}) {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const value = await this.exec(script);
      if (value) return value;
      if (Date.now() > deadline) {
        throw new WebDriverError(`timed out after ${timeoutMs}ms waiting for ${what}`);
      }
      await new Promise((r) => setTimeout(r, intervalMs));
    }
  }

  // `using`: "css selector" | "xpath" | "link text" | ... Returns an element id.
  async find(using, value) {
    const el = await this.#cmd('POST', this.#s('/element'), { using, value });
    return Object.values(el)[0];
  }

  async click(elementId) {
    await this.#cmd('POST', this.#s(`/element/${elementId}/click`), {});
  }

  async screenshotTo(filePath) {
    const b64 = await this.#cmd('GET', this.#s('/screenshot'));
    await fs.writeFile(filePath, Buffer.from(b64, 'base64'));
  }
}
