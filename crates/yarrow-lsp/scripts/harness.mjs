#!/usr/bin/env node
/**
 * Minimal LSP protocol harness for yarrow-lsp.
 *
 * Starts the server with `--listen 127.0.0.1:0`, connects over TCP, runs a
 * scenario, and exits non-zero on failure.
 *
 * Usage (from repo root):
 *   node crates/yarrow-lsp/scripts/harness.mjs
 *   node crates/yarrow-lsp/scripts/harness.mjs pull-diagnostics
 *   node crates/yarrow-lsp/scripts/harness.mjs workspace-diagnostics
 *   node crates/yarrow-lsp/scripts/harness.mjs project-roots
 *   node crates/yarrow-lsp/scripts/harness.mjs project-missing-root
 *   node crates/yarrow-lsp/scripts/harness.mjs definition-require
 *   node crates/yarrow-lsp/scripts/harness.mjs on-type-format
 *   node crates/yarrow-lsp/scripts/harness.mjs rapid-edits
 *   node crates/yarrow-lsp/scripts/harness.mjs virtual-std
 *
 * Env:
 *   YARROW_LSP_BIN  - server argv prefix (default: cargo run -q -p yarrow_lsp --)
 */

import { spawn } from "node:child_process";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "../../..");
const LISTEN_RE = /listening on ([^\s]+)/;

const SCENARIOS = [
  "pull-diagnostics",
  "workspace-diagnostics",
  "project-roots",
  "project-missing-root",
  "definition-require",
  "on-type-format",
  "rapid-edits",
  "virtual-std",
];

function usage() {
  console.error(
    `usage: node crates/yarrow-lsp/scripts/harness.mjs [${SCENARIOS.join("|")}]`,
  );
  process.exit(2);
}

function serverArgv() {
  const raw = process.env.YARROW_LSP_BIN;
  if (raw && raw.trim()) {
    return raw.trim().split(/\s+/);
  }
  return ["cargo", "run", "-q", "-p", "yarrow_lsp", "--"];
}

function encode(msg) {
  const body = Buffer.from(JSON.stringify(msg), "utf8");
  return Buffer.concat([
    Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, "utf8"),
    body,
  ]);
}

class LspClient {
  constructor(socket) {
    this.socket = socket;
    this.buf = Buffer.alloc(0);
    this.pending = new Map();
    this.nextId = 1;
    this.notifications = [];
    this.socket.on("data", (chunk) => {
      this.buf = Buffer.concat([this.buf, chunk]);
      this.pump();
    });
  }

  pump() {
    while (true) {
      const headerEnd = this.buf.indexOf("\r\n\r\n");
      if (headerEnd < 0) return;
      const header = this.buf.slice(0, headerEnd).toString("utf8");
      const match = /Content-Length:\s*(\d+)/i.exec(header);
      if (!match) throw new Error(`bad LSP header: ${header}`);
      const len = Number(match[1]);
      const start = headerEnd + 4;
      if (this.buf.length < start + len) return;
      const body = this.buf.slice(start, start + len);
      this.buf = this.buf.slice(start + len);
      const msg = JSON.parse(body.toString("utf8"));
      if (msg.id !== undefined && this.pending.has(msg.id)) {
        const { resolve, reject } = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        if (msg.error) reject(new Error(JSON.stringify(msg.error)));
        else resolve(msg.result);
      } else if (msg.method) {
        this.notifications.push(msg);
      }
    }
  }

  request(method, params) {
    const id = this.nextId++;
    const msg = { jsonrpc: "2.0", id, method, params };
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket.write(encode(msg));
    });
  }

  notify(method, params) {
    this.socket.write(encode({ jsonrpc: "2.0", method, params }));
  }

  close() {
    this.socket.end();
  }
}

function startServer(extraEnv = {}) {
  const prefix = serverArgv();
  const args = [...prefix.slice(1), "--listen", "127.0.0.1:0", "--log-level", "info"];
  const child = spawn(prefix[0], args, {
    cwd: REPO_ROOT,
    stdio: ["ignore", "ignore", "pipe"],
    env: { ...process.env, ...extraEnv },
  });

  let stderr = "";
  child.stderr.setEncoding("utf8");

  const ready = new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`timeout waiting for listen line; stderr:\n${stderr}`));
    }, 120_000);

    child.stderr.on("data", (chunk) => {
      stderr += chunk;
      const m = LISTEN_RE.exec(stderr);
      if (m) {
        clearTimeout(timer);
        resolve(m[1]);
      }
    });

    child.on("error", (err) => {
      clearTimeout(timer);
      reject(err);
    });
    child.on("exit", (code, signal) => {
      if (code && code !== 0) {
        clearTimeout(timer);
        reject(
          new Error(
            `server exited early code=${code} signal=${signal}; stderr:\n${stderr}`,
          ),
        );
      }
    });
  });

  return { child, ready };
}

function connect(addr) {
  const [host, portStr] = addr.split(":");
  const port = Number(portStr);
  return new Promise((resolve, reject) => {
    const socket = net.connect({ host, port }, () => resolve(socket));
    socket.on("error", reject);
  });
}

/** Full pull report is `{ kind: "full", resultId?, items: [...] }`. */
function digDiagnostics(report) {
  if (!report) return [];
  if (Array.isArray(report.items)) return report.items;
  if (report.full && Array.isArray(report.full.items)) return report.full.items;
  if (report.report) return digDiagnostics(report.report);
  return [];
}

function hasCode(items, want) {
  return items.some((d) => {
    const code = d && d.code;
    return (
      code === want || code === Number(want.slice(1)) || String(code) === want
    );
  });
}

function errorItems(items) {
  return items.filter((d) => d && d.severity === 1);
}

function fileUri(fixture) {
  return "file://" + fixture;
}

function sameFileUri(a, b) {
  if (a === b) return true;
  try {
    const pa = decodeURIComponent(String(a).replace(/^file:\/\//, ""));
    const pb = decodeURIComponent(String(b).replace(/^file:\/\//, ""));
    return path.resolve(pa) === path.resolve(pb);
  } catch {
    return false;
  }
}

async function openFile(client, fixture, version = 1) {
  const text = fs.readFileSync(fixture, "utf8");
  const uri = fileUri(fixture);
  client.notify("textDocument/didOpen", {
    textDocument: {
      uri,
      languageId: "yarrow",
      version,
      text,
    },
  });
  return uri;
}

async function scenarioPullDiagnostics(client) {
  const fixture = path.join(
    REPO_ROOT,
    "docs/examples/invalid/01_use_after_move.yar",
  );

  const init = await client.request("initialize", {
    processId: null,
    clientInfo: { name: "yarrow-lsp-harness", version: "0.1" },
    capabilities: {},
    rootUri: null,
  });
  if (!init || !init.capabilities) {
    throw new Error("initialize missing capabilities");
  }
  if (!init.capabilities.diagnosticProvider) {
    throw new Error("initialize missing diagnosticProvider");
  }

  client.notify("initialized", {});
  const uri = await openFile(client, fixture);

  const report = await client.request("textDocument/diagnostic", {
    textDocument: { uri },
  });
  const flat = digDiagnostics(report);
  if (!hasCode(flat, "E373")) {
    throw new Error(
      `expected E373 in pull diagnostics; got: ${JSON.stringify(report)}`,
    );
  }

  await client.request("shutdown", null);
  client.notify("exit", null);
}

async function scenarioWorkspaceDiagnostics(client) {
  const fixture = path.join(
    REPO_ROOT,
    "docs/examples/invalid/01_use_after_move.yar",
  );

  const init = await client.request("initialize", {
    processId: null,
    clientInfo: { name: "yarrow-lsp-harness", version: "0.1" },
    capabilities: {},
    rootUri: null,
  });
  if (!init?.capabilities?.diagnosticProvider?.workspaceDiagnostics) {
    throw new Error(
      `expected diagnosticProvider.workspaceDiagnostics true; got: ${JSON.stringify(init?.capabilities?.diagnosticProvider)}`,
    );
  }

  client.notify("initialized", {});
  const uri = await openFile(client, fixture);

  // Pull immediately (do not wait for publishDiagnostics).
  const report = await client.request("workspace/diagnostic", {
    previousResultIds: [],
  });
  const items = report?.items;
  if (!Array.isArray(items) || items.length < 1) {
    throw new Error(
      `expected workspace/diagnostic items; got: ${JSON.stringify(report)}`,
    );
  }
  const entry = items.find((it) => sameFileUri(it.uri, uri));
  if (!entry) {
    throw new Error(
      `expected report for ${uri}; got: ${JSON.stringify(report)}`,
    );
  }
  const flat = Array.isArray(entry.items) ? entry.items : [];
  if (!hasCode(flat, "E373")) {
    throw new Error(
      `expected E373 in workspace pull; got: ${JSON.stringify(entry)}`,
    );
  }

  await client.request("shutdown", null);
  client.notify("exit", null);
}

async function scenarioProjectRoots(client) {
  const rootA = path.join(REPO_ROOT, "docs/examples/project/root_a.yar");
  const rootB = path.join(REPO_ROOT, "docs/examples/project/root_b.yar");

  const init = await client.request("initialize", {
    processId: null,
    clientInfo: { name: "yarrow-lsp-harness", version: "0.1" },
    capabilities: {},
    rootUri: null,
    initializationOptions: {
      projectRoots: [rootA, rootB],
    },
  });
  if (!init?.capabilities?.diagnosticProvider) {
    throw new Error("initialize missing diagnosticProvider");
  }
  if (!init.capabilities.diagnosticProvider.interFileDependencies) {
    throw new Error("expected interFileDependencies true in project mode");
  }

  client.notify("initialized", {});
  const uriA = await openFile(client, rootA, 1);
  const uriB = await openFile(client, rootB, 1);

  const reportA = await client.request("textDocument/diagnostic", {
    textDocument: { uri: uriA },
  });
  const reportB = await client.request("textDocument/diagnostic", {
    textDocument: { uri: uriB },
  });
  const itemsA = digDiagnostics(reportA);
  const itemsB = digDiagnostics(reportB);
  if (errorItems(itemsA).length || errorItems(itemsB).length) {
    throw new Error(
      `expected no errors for project roots; A=${JSON.stringify(itemsA)} B=${JSON.stringify(itemsB)}`,
    );
  }

  await client.request("shutdown", null);
  client.notify("exit", null);
}

async function scenarioProjectMissingRoot(client) {
  const rootA = path.join(REPO_ROOT, "docs/examples/project/root_a.yar");
  const missing = path.join(
    REPO_ROOT,
    "docs/examples/project/does_not_exist.yar",
  );
  const missingUri = fileUri(missing);

  await client.request("initialize", {
    processId: null,
    clientInfo: { name: "yarrow-lsp-harness", version: "0.1" },
    capabilities: {},
    rootUri: null,
    initializationOptions: {
      projectRoots: [rootA, missing],
    },
  });
  client.notify("initialized", {});
  await openFile(client, rootA, 1);

  const deadline = Date.now() + 10_000;
  let hit = null;
  while (Date.now() < deadline) {
    hit = client.notifications.find(
      (n) =>
        n.method === "textDocument/publishDiagnostics" &&
        n.params &&
        sameFileUri(n.params.uri, missingUri) &&
        hasCode(n.params.diagnostics || [], "E383"),
    );
    if (hit) break;
    await new Promise((r) => setTimeout(r, 50));
  }
  if (!hit) {
    const pubs = client.notifications.filter(
      (n) => n.method === "textDocument/publishDiagnostics",
    );
    throw new Error(
      `expected publishDiagnostics E383 for missing root; got=${JSON.stringify(pubs)}`,
    );
  }

  await client.request("shutdown", null);
  client.notify("exit", null);
}

async function scenarioDefinitionRequire(client) {
  const fixture = path.join(
    REPO_ROOT,
    "docs/examples/valid/12_modules.yar",
  );
  const greetPath = path.join(
    REPO_ROOT,
    "docs/examples/valid/helpers/greet.yar",
  );
  const text = fs.readFileSync(fixture, "utf8");
  // Alias on: `"helpers.greet" greet require`
  const aliasOffset = text.indexOf("greet require");
  if (aliasOffset < 0) {
    throw new Error("fixture missing 'greet require'");
  }
  const before = text.slice(0, aliasOffset);
  const line = (before.match(/\n/g) || []).length;
  const character = aliasOffset - (before.lastIndexOf("\n") + 1);

  await client.request("initialize", {
    processId: null,
    clientInfo: { name: "yarrow-lsp-harness", version: "0.1" },
    capabilities: {
      general: { positionEncodings: ["utf-8"] },
    },
    rootUri: null,
  });
  client.notify("initialized", {});
  const uri = await openFile(client, fixture);

  const result = await client.request("textDocument/definition", {
    textDocument: { uri },
    position: { line, character },
  });

  const loc = Array.isArray(result) ? result[0] : result;
  if (!loc || !loc.uri) {
    throw new Error(
      `expected definition Location for greet require; got: ${JSON.stringify(result)}`,
    );
  }
  const target = decodeURIComponent(String(loc.uri).replace(/^file:\/\//, ""));
  if (path.resolve(target) !== path.resolve(greetPath)) {
    throw new Error(
      `expected definition -> ${greetPath}; got uri=${loc.uri}`,
    );
  }

  await client.request("shutdown", null);
  client.notify("exit", null);
}

async function scenarioOnTypeFormat(client) {
  // Nested `end` at one tab; client over-indented the following blank line.
  const text = "main function do\n\tinner function do\n\t\t1 drop\n\tend\n\t\t\t";
  const uri = "file:///tmp/yarrow-lsp-on-type.yar";
  // Cursor after typed newline + over-indent (end of buffer).
  const line = 4;
  const character = 3;

  const init = await client.request("initialize", {
    processId: null,
    clientInfo: { name: "yarrow-lsp-harness", version: "0.1" },
    capabilities: {
      general: { positionEncodings: ["utf-8"] },
    },
    rootUri: null,
  });
  const onType = init?.capabilities?.documentOnTypeFormattingProvider;
  if (!onType || onType.firstTriggerCharacter !== "\n") {
    throw new Error(
      `expected documentOnTypeFormattingProvider firstTriggerCharacter=\\n; got: ${JSON.stringify(onType)}`,
    );
  }

  client.notify("initialized", {});
  client.notify("textDocument/didOpen", {
    textDocument: {
      uri,
      languageId: "yarrow",
      version: 1,
      text,
    },
  });

  const edits = await client.request("textDocument/onTypeFormatting", {
    textDocument: { uri },
    position: { line, character },
    ch: "\n",
    options: { tabSize: 4, insertSpaces: false },
  });

  if (!Array.isArray(edits) || edits.length !== 1) {
    throw new Error(
      `expected one indent TextEdit; got: ${JSON.stringify(edits)}`,
    );
  }
  const edit = edits[0];
  if (edit.newText !== "\t") {
    throw new Error(
      `expected newText one tab (match end indent); got: ${JSON.stringify(edit)}`,
    );
  }
  if (
    edit.range?.start?.line !== 4 ||
    edit.range?.end?.line !== 4 ||
    edit.range?.start?.character !== 0 ||
    edit.range?.end?.character !== 3
  ) {
    throw new Error(
      `expected range covering over-indent on line 4; got: ${JSON.stringify(edit.range)}`,
    );
  }

  await client.request("shutdown", null);
  client.notify("exit", null);
}

async function scenarioRapidEdits(client) {
  // Gate: two quick full-document changes; last publish/pull match latest version only.
  const validPath = path.join(REPO_ROOT, "docs/examples/valid/01_hello.yar");
  const invalidPath = path.join(
    REPO_ROOT,
    "docs/examples/invalid/01_use_after_move.yar",
  );
  const valid = fs.readFileSync(validPath, "utf8");
  const invalid = fs.readFileSync(invalidPath, "utf8");
  const uri = fileUri(validPath);

  await client.request("initialize", {
    processId: null,
    clientInfo: { name: "yarrow-lsp-harness", version: "0.1" },
    capabilities: {},
    rootUri: null,
  });
  client.notify("initialized", {});

  client.notify("textDocument/didOpen", {
    textDocument: {
      uri,
      languageId: "yarrow",
      version: 1,
      text: valid,
    },
  });
  // Supersede with invalid then valid before debounce settles.
  client.notify("textDocument/didChange", {
    textDocument: { uri, version: 2 },
    contentChanges: [{ text: invalid }],
  });
  client.notify("textDocument/didChange", {
    textDocument: { uri, version: 3 },
    contentChanges: [{ text: valid }],
  });

  const deadline = Date.now() + 10_000;
  let sawV3 = false;
  while (Date.now() < deadline) {
    const pubs = client.notifications.filter(
      (n) =>
        n.method === "textDocument/publishDiagnostics" &&
        n.params &&
        sameFileUri(n.params.uri, uri),
    );
    if (pubs.some((n) => n.params.version === 3)) {
      sawV3 = true;
      break;
    }
    await new Promise((r) => setTimeout(r, 50));
  }
  if (!sawV3) {
    const pubs = client.notifications.filter(
      (n) => n.method === "textDocument/publishDiagnostics",
    );
    throw new Error(
      `expected publishDiagnostics version 3; got=${JSON.stringify(pubs)}`,
    );
  }

  // Allow any in-flight stale task a chance to mis-publish, then assert order.
  await new Promise((r) => setTimeout(r, 500));

  const pubs = client.notifications.filter(
    (n) =>
      n.method === "textDocument/publishDiagnostics" &&
      n.params &&
      sameFileUri(n.params.uri, uri),
  );
  let maxSeen = -1;
  for (const n of pubs) {
    const v = n.params.version;
    if (typeof v !== "number") continue;
    if (v < maxSeen) {
      throw new Error(
        `torn diagnostics: publish v${v} after v${maxSeen}; pubs=${JSON.stringify(pubs)}`,
      );
    }
    maxSeen = Math.max(maxSeen, v);
  }
  if (maxSeen !== 3) {
    throw new Error(
      `expected last publish version 3; maxSeen=${maxSeen} pubs=${JSON.stringify(pubs)}`,
    );
  }
  const last = pubs[pubs.length - 1];
  if (last.params.version !== 3) {
    throw new Error(
      `expected final publish version 3; got=${JSON.stringify(last)}`,
    );
  }
  if (hasCode(last.params.diagnostics || [], "E373")) {
    throw new Error(
      `expected no E373 on latest valid buffer; got=${JSON.stringify(last)}`,
    );
  }

  const report = await client.request("textDocument/diagnostic", {
    textDocument: { uri },
  });
  const flat = digDiagnostics(report);
  if (hasCode(flat, "E373")) {
    throw new Error(
      `expected pull diagnostics without E373 for v3; got=${JSON.stringify(report)}`,
    );
  }
  const resultId = report?.resultId ?? report?.full?.resultId;
  if (resultId != null && resultId !== "v3") {
    throw new Error(
      `expected pull resultId v3; got=${JSON.stringify(report)}`,
    );
  }

  await client.request("shutdown", null);
  client.notify("exit", null);
}

async function scenarioVirtualStd(client) {
  const fixture = path.join(REPO_ROOT, "docs/examples/valid/01_hello.yar");
  const text = fs.readFileSync(fixture, "utf8");
  // Alias on: `"std.io" io require`
  const aliasOffset = text.indexOf("io require");
  if (aliasOffset < 0) {
    throw new Error("fixture missing 'io require'");
  }
  const before = text.slice(0, aliasOffset);
  const line = (before.match(/\n/g) || []).length;
  const character = aliasOffset - (before.lastIndexOf("\n") + 1);

  const init = await client.request("initialize", {
    processId: null,
    clientInfo: { name: "yarrow-lsp-harness", version: "0.1" },
    capabilities: {
      general: { positionEncodings: ["utf-8"] },
    },
    rootUri: null,
  });
  const schemes =
    init?.capabilities?.experimental?.textDocumentContent?.schemes;
  if (!Array.isArray(schemes) || !schemes.includes("yarrow-std")) {
    throw new Error(
      `expected experimental.textDocumentContent.schemes to include yarrow-std; got: ${JSON.stringify(init?.capabilities?.experimental)}`,
    );
  }

  client.notify("initialized", {});
  const uri = await openFile(client, fixture);

  const result = await client.request("textDocument/definition", {
    textDocument: { uri },
    position: { line, character },
  });
  const loc = Array.isArray(result) ? result[0] : result;
  if (!loc || !loc.uri) {
    throw new Error(
      `expected definition Location for std.io require; got: ${JSON.stringify(result)}`,
    );
  }
  const targetUri = String(loc.uri);
  if (!targetUri.startsWith("yarrow-std:")) {
    throw new Error(
      `expected virtual yarrow-std: URI (FORCE_VIRTUAL_STD); got uri=${targetUri}`,
    );
  }

  const content = await client.request("workspace/textDocumentContent", {
    uri: targetUri,
  });
  const body = content?.text ?? content;
  if (typeof body !== "string" || !body.includes("write_line")) {
    throw new Error(
      `expected embedded std.io source via textDocumentContent; got: ${JSON.stringify(content)?.slice(0, 200)}`,
    );
  }

  await client.request("shutdown", null);
  client.notify("exit", null);
}

async function main() {
  const scenario = process.argv[2] || "pull-diagnostics";
  if (scenario === "-h" || scenario === "--help") usage();
  if (!SCENARIOS.includes(scenario)) {
    console.error(`unknown scenario: ${scenario}`);
    usage();
  }

  const extraEnv =
    scenario === "virtual-std" ? { YARROW_LSP_FORCE_VIRTUAL_STD: "1" } : {};
  const { child, ready } = startServer(extraEnv);
  let client;
  try {
    const addr = await ready;
    const socket = await connect(addr);
    client = new LspClient(socket);

    if (scenario === "pull-diagnostics") {
      await scenarioPullDiagnostics(client);
    } else if (scenario === "workspace-diagnostics") {
      await scenarioWorkspaceDiagnostics(client);
    } else if (scenario === "project-roots") {
      await scenarioProjectRoots(client);
    } else if (scenario === "project-missing-root") {
      await scenarioProjectMissingRoot(client);
    } else if (scenario === "definition-require") {
      await scenarioDefinitionRequire(client);
    } else if (scenario === "on-type-format") {
      await scenarioOnTypeFormat(client);
    } else if (scenario === "rapid-edits") {
      await scenarioRapidEdits(client);
    } else if (scenario === "virtual-std") {
      await scenarioVirtualStd(client);
    }

    console.log(`harness ok: ${scenario} via tcp ${addr}`);
  } finally {
    if (client) client.close();
    child.kill("SIGTERM");
    await new Promise((r) => setTimeout(r, 200));
    try {
      child.kill("SIGKILL");
    } catch {
      /* ignore */
    }
  }
}

main().catch((err) => {
  console.error(err && err.stack ? err.stack : err);
  process.exit(1);
});
