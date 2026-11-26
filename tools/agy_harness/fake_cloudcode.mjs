#!/usr/bin/env node
/*
 * fake_cloudcode.mjs — servidor HTTP/2 local que imita los endpoints
 * `v1internal` del Language Server de Antigravity.
 *
 * OBJETIVO
 * --------
 * Capturar, byte a byte, lo que el LS REAL envía y espera recibir, sin
 * tocar ningún servidor de Google: todo el tráfico se queda en loopback.
 *
 * DISEÑO
 * ------
 *  - Node >= 18 con `http2` nativo: negocia HTTP/2 (h2c con prior knowledge)
 *    y también acepta HTTP/1.1 por si el cliente no usa h2.
 *  - Cada petición se vuelca en un JSONL (`--out`), con cabeceras, cuerpo
 *    en hex y una vista imprimible. Eso permite decodificar después los
 *    campos protobuf sin depender de un .proto.
 *  - Las respuestas se construyen como tramas gRPC (5 bytes: flag + longitud
 *    big-endian) con un protobuf vacío o con campos mínimos.
 *  - `--require-auth-header` devuelve 401 si no llega `Authorization`, para
 *    poder observar la ruta de autenticación sin tener credenciales.
 *
 * USO
 * ---
 *   node fake_cloudcode.mjs --port 8085 --out captures/run.jsonl
 */
import http2 from 'node:http2';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';

const argv = process.argv.slice(2);
function arg(name, def) {
  const i = argv.indexOf('--' + name);
  return i >= 0 && argv[i + 1] ? argv[i + 1] : def;
}
const PORT = parseInt(arg('port', '8085'), 10);
const OUT = arg('out', 'captures/run.jsonl');
const REQUIRE_AUTH = argv.includes('--require-auth-header');
const QUIET = argv.includes('--quiet');

fs.mkdirSync(path.dirname(OUT), { recursive: true });
const fh = fs.createWriteStream(OUT, { flags: 'a' });

let seq = 0;
function log(obj) {
  fh.write(JSON.stringify(obj) + '\n');
}

function printable(buf) {
  let s = '';
  for (const b of buf) s += b >= 32 && b < 127 ? String.fromCharCode(b) : '.';
  return s;
}

/** Decodifica un protobuf "crudo" (sin .proto): lista campo# -> valor. */
function decodeProto(buf) {
  const fields = [];
  let i = 0;
  while (i < buf.length) {
    let key = 0, shift = 0;
    while (i < buf.length) {
      const b = buf[i++];
      key |= (b & 0x7f) << shift;
      if (!(b & 0x80)) break;
      shift += 7;
      if (shift > 28) break;
    }
    const num = key >>> 3, wire = key & 7;
    if (num === 0) break;
    let val = null;
    if (wire === 0) {
      let v = 0n, sh = 0n;
      while (i < buf.length) {
        const b = buf[i++];
        v |= BigInt(b & 0x7f) << sh;
        if (!(b & 0x80)) break;
        sh += 7n;
      }
      val = v.toString();
    } else if (wire === 2) {
      let len = 0, shift = 0;
      while (i < buf.length) {
        const b = buf[i++];
        len |= (b & 0x7f) << shift;
        if (!(b & 0x80)) break;
        shift += 7;
      }
      const sub = buf.subarray(i, i + len); i += len;
      val = { len, hex: sub.toString('hex').slice(0, 400), ascii: printable(sub).slice(0, 300) };
    } else if (wire === 5) { val = buf.readUInt32LE(i); i += 4; }
    else if (wire === 1) { val = buf.readBigUInt64LE(i).toString(); i += 8; }
    else break;
    fields.push({ field: num, wire, value: val });
  }
  return fields;
}

/** Trama gRPC: [compressed:1][length:4 big-endian][payload] */
function grpcFrame(payload) {
  const head = Buffer.alloc(5);
  head.writeUInt8(0, 0);
  head.writeUInt32BE(payload.length, 1);
  return Buffer.concat([head, payload]);
}

/** Protobuf mínimo: campo 1 (string) y campo 2 (string). */
function simpleProto(a, b) {
  const parts = [];
  const enc = (field, str) => {
    const s = Buffer.from(str, 'utf8');
    const h = Buffer.from([field << 3 | 2]);
    const l = Buffer.alloc(1); l.writeUInt8(s.length);
    parts.push(h, l, s);
  };
  if (a) enc(1, a);
  if (b) enc(2, b);
  return Buffer.concat(parts);
}

function respond(res, method, path_, body) {
  const h2 = typeof res.respond === 'function';
  const hdrs = res.reqHeaders || res.headers || {};
  const auth = hdrs['authorization'];
  if (REQUIRE_AUTH && !auth) {
    const payload = JSON.stringify({ error: { code: 401, message: 'UNAUTHENTICATED (servidor local)' } });
    if (h2) {
      res.respond({ ':status': 401, 'content-type': 'application/json' });
      res.end(payload);
    } else {
      res.writeHead(401, { 'content-type': 'application/json' });
      res.end(payload);
    }
    log({ kind: 'response', method, path: path_, status: 401 });
    return;
  }
  // Los endpoints v1internal se responden como gRPC (application/grpc+proto).
  const grpcLike = /v1internal|exa\.|LanguageServer/i.test(path_ || '');
  if (grpcLike) {
    const payload = simpleProto('ok', `local:${method}`);
    if (h2) {
      res.respond({
        ':status': 200,
        'content-type': 'application/grpc+proto',
        'grpc-status': '0',
        'grpc-message': 'ok',
      });
      res.end(grpcFrame(payload));
    } else {
      res.writeHead(200, { 'content-type': 'application/grpc+proto', 'grpc-status': '0', 'grpc-message': 'ok' });
      res.end(grpcFrame(payload));
    }
    log({ kind: 'response', method, path: path_, status: 200, grpc: true, payload_len: payload.length });
    return;
  }
  const payload = JSON.stringify({ ok: true, local: true, path: path_ });
  if (h2) {
    res.respond({ ':status': 200, 'content-type': 'application/json' });
    res.end(payload);
  } else {
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end(payload);
  }
  log({ kind: 'response', method, path: path_, status: 200 });
}

function handleRequest(req, res, proto) {
  const chunks = [];
  req.on('data', c => chunks.push(c));
  req.on('end', () => {
    const body = Buffer.concat(chunks);
    const id = ++seq;
    // En HTTP/2 el método y la ruta van en las pseudo-cabeceras.
    const hdrs = req.headers || req.reqHeaders || {};
    const method = req.method || hdrs[':method'] || 'UNKNOWN';
    const path_ = req.url || hdrs[':path'] || 'UNKNOWN';
    const rec = {
      kind: 'request', seq: id, t: new Date().toISOString(),
      method, path: path_, http: proto,
      headers: hdrs,
      body_len: body.length,
      body_hex: body.toString('hex').slice(0, 4000),
      body_ascii: printable(body).slice(0, 2000),
      proto_fields: body.length ? decodeProto(body) : [],
    };
    log(rec);
    if (!QUIET) {
      process.stdout.write(
        `[${id}] ${method} ${path_} len=${body.length} ` +
        `fields=${rec.proto_fields.length} ${rec.body_ascii.slice(0, 120)}\n`);
    }
    respond(res, method, path_, body);
  });
}

// La API de compatibilidad ('request') es la unica que expone cabeceras de
// forma fiable en Node 22 (el stream crudo de http2 no expone .headers).
// allowHTTP1 hace que el MISMO puerto acepte:
//   - HTTP/2 cleartext (h2c, prior knowledge): lo que usa el cliente gRPC
//     de Go cuando la URL es http:// (sin TLS).
//   - HTTP/1.1 plano, por si el cliente cae a REST.
const server = http2.createServer({ allowHTTP1: true });
server.on('request', (req, res) => {
  handleRequest(req, res, res.stream ? 'h2' : 'http/1.1');
});
server.on('error', e => { process.stderr.write('h2 error: ' + e.message + '\n'); });

// Node no multiplexa HTTP/1.1 correctamente en el socket de http2, asi que
// se abre un puerto contiguo en paralelo para REST plano.
const h1 = http.createServer((req, res) => handleRequest(req, res, 'http/1.1'));

server.listen(PORT, '127.0.0.1', () => {
  h1.listen(PORT + 1, '127.0.0.1', () => {
    process.stdout.write(
      `fake_cloudcode: h2c http://127.0.0.1:${PORT}  |  http/1.1 http://127.0.0.1:${PORT + 1}` +
      `  ->  ${OUT}\n`);
  });
});
