#!/usr/bin/env node
/*
 * fake_cloudcode.mjs — servidor local que emula la API v1internal de
 * Antigravity para poder probar el Language Server sin tocar Google.
 *
 * CONTRATO MEDIDO (no supuesto): captura del LS real, 2026-11-26
 * ------------------------------------------------------------------
 *   POST /v1internal:<metodo>   HTTP/1.1
 *   Content-Type: application/json
 *   User-Agent: antigravity/hub/2.4.1 (aidev_client; os_type=linux; arch=arm64; cl=998057262)
 *   Authorization: Bearer <token>
 *   Body: JSON camelCase, p.ej. {"metadata":{"ideType":"ANTIGRAVITY"}}
 *   Body: {"metadata":{"updateChannel":"stable"}}
 * ------------------------------------------------------------------
 * El LS habla HTTP/1.1 con protobuf-JSON (estilo Connect), NO protobuf
 * binario y NO gRPC con trama de 5 bytes. Por eso el listener principal es
 * HTTP/1.1 plano.
 *
 * Cada petición se graba en un JSONL. Los métodos sin respuesta definida se
 * responden con `{}` y se marcan como "sin fixture": así el arnés ENUMERA la
 * superficie real que el LS usa, método a método.
 *
 * USO
 *   node fake_cloudcode.mjs --port 8085 --out captures/run.jsonl
 *   node fake_cloudcode.mjs --port 8085 --require-auth   # 401 sin Authorization
 *   node fake_cloudcode.mjs --port 8085 --scenario authfail
 */
import http from 'node:http';
import http2 from 'node:http2';
import fs from 'node:fs';
import path from 'node:path';

const argv = process.argv.slice(2);
const arg = (n, d) => { const i = argv.indexOf('--' + n); return i >= 0 && argv[i + 1] ? argv[i + 1] : d; };
const PORT = parseInt(arg('port', '8085'), 10);
const OUT = arg('out', 'captures/run.jsonl');
const REQUIRE_AUTH = argv.includes('--require-auth');
const SCENARIO = arg('scenario', 'ok');
const QUIET = argv.includes('--quiet');

fs.mkdirSync(path.dirname(OUT), { recursive: true });
const fh = fs.createWriteStream(OUT, { flags: 'a' });
let seq = 0;
const log = o => fh.write(JSON.stringify(o) + '\n');

/* ------------------------------------------------------------------ */
/* Fixtures: formas JSON de los protos de la comunidad                */
/* (ecosystem/antigravity-grpc-schemas/protos/exa/google/internal/   */
/*  cloud/code/v1internal/*.proto)                                    */
/* ------------------------------------------------------------------ */
const USER_TIER_FREE = {
  id: 'free-tier', name: 'Free', upgradeType: 'UPGRADE_TYPE_UNSPECIFIED',
};
const USER_TIER_GOOGLE_ONE = {
  id: 'legacy-tier-google-one', name: 'Google One',
  upgradeType: 'GOOGLE_ONE',
};

const RESPONSES = {
  // onboarding.proto -> LoadCodeAssistResponse
  'v1internal:loadCodeAssist': {
    currentTier: USER_TIER_FREE,
    allowedTiers: [USER_TIER_FREE, USER_TIER_GOOGLE_ONE],
    cloudaicompanionProject: 'local-harness-project',
    gcpManaged: false,
    releaseChannel: { type: 'STABLE', name: 'stable' },
    paidTier: USER_TIER_FREE,
  },
  // jetski_service.proto -> ListExperimentsResponse
  'v1internal:listExperiments': { experiments: {} },
  // model_configs.proto -> FetchAvailableModelsResponse
  'v1internal:fetchAvailableModels': {
    models: {
      'local-model': {
        modelId: 'local-model',
        name: 'Local Harness Model',
        supportedMethods: ['generateContent', 'streamGenerateContent'],
        userFacingModelId: 'local-model',
      },
    },
  },
  // jetski_service.proto -> FetchUserInfoResponse
  'v1internal:fetchUserInfo': {
    userSettings: {}, userTags: [], regionCode: 'US',
  },
  'v1internal:onboardUser': {
    cloudaicompanionProject: 'local-harness-project',
  },
};

/* ------------------------------------------------------------------ */
function methodOf(url) {
  const p = (url || '').split('?')[0].replace(/^\//, '');
  return p;
}

function scenarioOverride(method, body) {
  if (SCENARIO === 'authfail') {
    return { status: 401, payload: { error: { code: 401, message: 'UNAUTHENTICATED (local)' } } };
  }
  if (SCENARIO === 'quota' && method === 'v1internal:loadCodeAssist') {
    return { status: 429, payload: { error: { code: 429, message: 'RESOURCE_EXHAUSTED (local)' } } };
  }
  return null;
}

function handle(req, res, proto) {
  const chunks = [];
  req.on('data', c => chunks.push(c));
  req.on('end', () => {
    const body = Buffer.concat(chunks);
    const hdrs = req.headers || req.reqHeaders || {};
    const method = req.method || hdrs[':method'] || 'UNKNOWN';
    const url = req.url || hdrs[':path'] || 'UNKNOWN';
    const m = methodOf(url);
    const id = ++seq;

    let parsed = null;
    try { parsed = JSON.parse(body.toString('utf8')); } catch { /* no json */ }

    log({
      kind: 'request', seq: id, t: new Date().toISOString(),
      method, path: url, rpc: m, http: proto, http_version: req.httpVersion || null,
      headers: hdrs, body_len: body.length,
      body_ascii: body.toString('latin1').slice(0, 2000),
      body_json: parsed,
      has_authorization: !!hdrs['authorization'],
      user_agent: hdrs['user-agent'] || null,
    });
    if (!QUIET) {
      console.log(`[${id}] ${method} ${url} len=${body.length} auth=${hdrs['authorization'] ? 'si' : 'no'} ${JSON.stringify(parsed)}`);
    }

    const ov = scenarioOverride(m, parsed);
    if (ov) {
      const payload = JSON.stringify(ov.payload);
      res.writeHead(ov.status, { 'content-type': 'application/json' });
      res.end(payload);
      log({ kind: 'response', seq: id, path: url, status: ov.status, scenario: SCENARIO });
      return;
    }
    if (REQUIRE_AUTH && !hdrs['authorization']) {
      const payload = JSON.stringify({ error: { code: 401, message: 'UNAUTHENTICATED (local)' } });
      res.writeHead(401, { 'content-type': 'application/json' });
      res.end(payload);
      log({ kind: 'response', seq: id, path: url, status: 401 });
      return;
    }
    const has = Object.prototype.hasOwnProperty.call(RESPONSES, m);
    const payload = JSON.stringify(has ? RESPONSES[m] : {});
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end(payload);
    log({ kind: 'response', seq: id, path: url, status: 200, fixture: has ? m : null });
  });
}

const h1 = http.createServer((req, res) => handle(req, res, 'http/1.1'));
const server = http2.createServer();
server.on('stream', stream => handle(stream, stream, 'h2'));
server.on('error', e => process.stderr.write('h2 error: ' + e.message + '\n'));

h1.listen(PORT, '127.0.0.1', () => {
  server.listen(PORT + 1, '127.0.0.1', () => {
    process.stdout.write(
      `fake_cloudcode: http/1.1 http://127.0.0.1:${PORT} | h2c http://127.0.0.1:${PORT + 1}` +
      ` | escenario=${SCENARIO} -> ${OUT}\n`);
  });
});
