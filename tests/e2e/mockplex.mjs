// Minimal HTTPS Plex server for the hermetic multi-Plex scenario. Each instance
// has its own token and machine identity, records every request, and serves the
// exact XML endpoints exercised by Vela's Plex source implementation.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import https from 'node:https';
import path from 'node:path';

function runOpenSsl(args) {
  const result = spawnSync('openssl', args, { encoding: 'utf8' });
  if (result.status !== 0) {
    throw new Error(`openssl ${args[0]} failed: ${result.stderr || result.stdout}`);
  }
}

export function createMockPlexTls(root) {
  const dir = path.join(root, 'plex-tls');
  fs.mkdirSync(dir, { recursive: true });
  const caKey = path.join(dir, 'ca-key.pem');
  const ca = path.join(dir, 'ca.pem');
  const serverKey = path.join(dir, 'server-key.pem');
  const request = path.join(dir, 'server.csr');
  const serverCert = path.join(dir, 'server.pem');
  const extensions = path.join(dir, 'server.ext');

  runOpenSsl([
    'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-sha256', '-days', '1',
    '-subj', '/CN=Vela E2E Plex CA', '-keyout', caKey, '-out', ca,
  ]);
  runOpenSsl([
    'req', '-newkey', 'rsa:2048', '-nodes', '-sha256',
    '-subj', '/CN=127.0.0.1', '-keyout', serverKey, '-out', request,
  ]);
  fs.writeFileSync(
    extensions,
    'subjectAltName=IP:127.0.0.1\nextendedKeyUsage=serverAuth\nbasicConstraints=critical,CA:FALSE\n',
  );
  runOpenSsl([
    'x509', '-req', '-sha256', '-days', '1', '-in', request,
    '-CA', ca, '-CAkey', caKey, '-CAcreateserial', '-out', serverCert,
    '-extfile', extensions,
  ]);

  return {
    ca,
    key: fs.readFileSync(serverKey),
    cert: fs.readFileSync(serverCert),
  };
}

function xmlEscape(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('"', '&quot;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;');
}

export async function startMockPlex({
  tls,
  name,
  machineIdentifier,
  token,
  movieTitle = 'Shared Plex Movie',
  year = 2020,
  guid = 'imdb://tt7654321',
}) {
  const state = {
    requests: [],
    served: [],
    contractViolations: [],
  };

  const server = https.createServer({ key: tls.key, cert: tls.cert }, (req, res) => {
    const url = new URL(req.url, 'https://mock');
    const request = {
      method: req.method,
      path: url.pathname,
      query: Object.fromEntries(url.searchParams),
      token: req.headers['x-plex-token'] ?? null,
      clientIdentifier: req.headers['x-plex-client-identifier'] ?? null,
    };
    state.requests.push(request);

    const send = (body, status = 200, headers = {}) => {
      res.writeHead(status, {
        'content-type': 'application/xml; charset=utf-8',
        'content-length': Buffer.byteLength(body),
        ...headers,
      });
      res.end(req.method === 'HEAD' ? undefined : body);
      state.served.push({ ...request, status });
    };

    if (request.token !== token) {
      state.contractViolations.push(`wrong token for ${request.method} ${request.path}`);
      send('<Response code="401" />', 401);
      return;
    }

    if (request.path === '/identity') {
      send(`<MediaContainer machineIdentifier="${xmlEscape(machineIdentifier)}" />`);
      return;
    }
    if (request.path === '/library/sections') {
      send(
        `<MediaContainer size="1"><Directory key="1" title="Movies" type="movie" ` +
        `agent="tv.plex.agents.movie" scanner="Plex Movie" /></MediaContainer>`,
      );
      return;
    }
    if (request.path === '/library/sections/1/all') {
      if (request.query.type !== '1') {
        state.contractViolations.push(`listing omitted movie type on ${name}`);
      }
      const start = Number(request.query['X-Plex-Container-Start'] ?? 0);
      if (start > 0) {
        send('<MediaContainer size="0" />');
        return;
      }
      send(
        `<MediaContainer size="1"><Video ratingKey="1" key="/library/metadata/1" ` +
        `title="${xmlEscape(movieTitle)}" type="movie" year="${year}" duration="10000">` +
        `<Guid id="${xmlEscape(guid)}" /></Video></MediaContainer>`,
      );
      return;
    }
    if (request.path === '/library/metadata/1') {
      send(
        `<MediaContainer size="1"><Video ratingKey="1" key="/library/metadata/1" ` +
        `title="${xmlEscape(movieTitle)}" type="movie" year="${year}" duration="10000">` +
        `<Guid id="${xmlEscape(guid)}" /><Media id="1" duration="10000" bitrate="1000" ` +
        `width="1920" height="1080" videoCodec="h264" audioCodec="aac" container="mp4">` +
        `<Part id="1" key="/library/parts/${xmlEscape(machineIdentifier)}/movie.mp4" ` +
        `duration="10000" file="/mock/movie.mp4" size="1" container="mp4" />` +
        `</Media></Video></MediaContainer>`,
      );
      return;
    }
    if (request.path.startsWith('/library/parts/')) {
      send('<Response code="404" />', 404);
      return;
    }
    if (request.path === '/hubs' || request.path === '/library/onDeck' || request.path === '/playlists') {
      send('<MediaContainer size="0" />');
      return;
    }

    state.contractViolations.push(`unmocked ${request.method} ${request.path}`);
    send('<Response code="404" />', 404);
  });

  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const port = server.address().port;

  return {
    name,
    machineIdentifier,
    token,
    port,
    state,
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

export function mockPlexSource(mock, { id }) {
  return {
    id,
    kind: 'plex',
    name: mock.name,
    base_url: `https://127.0.0.1:${mock.port}`,
    access_token: mock.token,
    api_key: null,
    user_id: null,
    device_id: `e2e-${id}`,
    machine_identifier: mock.machineIdentifier,
  };
}
