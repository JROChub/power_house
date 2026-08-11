"use strict";

const CACHE_NAME = "ckodmk-browser-v0.2.0-23";
const MAX_CORE_ASSET_BYTES = 16 * 1024 * 1024;
const CORE = Object.freeze([
  "./",
  "./index.html",
  "./manifest.webmanifest",
  "./styles.css?v=20260810h",
  "./app.js?v=20260810e",
  "./browser-optimizer.js?v=20260810f",
  "./browser-gate.js?v=20260810i",
  "./phone-study.js?v=20260810d",
  "./evidence.json",
  "./assets/powerhouse-logo.svg",
  "./assets/icon-192.png",
  "./assets/icon-512.png",
  "./demo/source.onnx",
  "./demo/candidate-int8.onnx",
  "./demo/optdigits-official-test.npz",
  "./demo/browser-contract.json",
  "./reports/ACCEPTANCE_RUBRIC.md",
  "./reports/AUDITOR_HANDOFF.md",
  "./reports/BLIND_CAMPAIGN_V3_RESULTS.md",
  "./reports/BROWSER_PHONE_STUDY_V2.md",
  "./reports/EXTERNAL_EVALUATION_RETURN_V1.md",
  "./reports/GENERATIVE_CAMPAIGN_V1.md",
  "./reports/GENERATIVE_CAMPAIGN_V1.json",
  "./reports/GENERATIVE_CAMPAIGN_V1_SPEC.md",
  "./reports/GOVERNANCE.md",
  "./reports/INTERNAL_READINESS_V1.md",
  "./reports/INTERNAL_READINESS_V1.json",
  "./reports/REAL_MODEL_RESULTS.md",
  "./reports/SECOND_TOOLCHAIN_RESULTS.md",
  "./reports/THREAT_MODEL.md",
  "./downloads/external-evaluation-pending.json",
  "./downloads/mfenx_ckodmk-0.2.0-py3-none-any.whl",
  "./downloads/mfenx_ckodmk-0.2.0-py3-none-any.whl.sha256",
  "./downloads/verify_external_evaluation.py",
  "./downloads/verify_external_evaluation.py.sha256",
  "./downloads/verify_phone_study.py",
  "./downloads/verify_phone_study.py.sha256",
  "./demo/mlp-w32/source.onnx",
  "./demo/mlp-w32/candidate-int8.onnx",
  "./demo/mlp-w32/browser-contract.json",
  "./demo/mlp-w32/training-record.json",
  "./demo/mlp-w64/source.onnx",
  "./demo/mlp-w64/candidate-int8.onnx",
  "./demo/mlp-w64/browser-contract.json",
  "./demo/mlp-w64/training-record.json",
  "./demo/rbf/source.onnx",
  "./demo/rbf/candidate-int8.onnx",
  "./demo/rbf/browser-contract.json",
  "./demo/rbf/training-record.json",
  "./demo/rbf-c40/source.onnx",
  "./demo/rbf-c40/candidate-int8.onnx",
  "./demo/rbf-c40/browser-contract.json",
  "./demo/rbf-c40/training-record.json",
  "./demo/rbf-c160/source.onnx",
  "./demo/rbf-c160/candidate-int8.onnx",
  "./demo/rbf-c160/browser-contract.json",
  "./demo/rbf-c160/training-record.json",
  "./vendor/ort/ort.wasm.min.js",
  "./vendor/ort/ort-wasm-simd-threaded.mjs",
  "./vendor/ort/ort-wasm-simd-threaded.wasm"
]);

async function cacheCoreAsset(cache, path) {
  const request = new Request(path, { cache: "no-store", credentials: "same-origin" });
  const response = await fetch(request);
  if (!response.ok || response.type !== "basic") throw new Error(`offline asset failed: ${path}`);
  const declaredLength = Number(response.headers.get("content-length"));
  if (Number.isFinite(declaredLength) && declaredLength > MAX_CORE_ASSET_BYTES) throw new Error(`offline asset too large: ${path}`);
  const body = await response.arrayBuffer();
  if (body.byteLength < 1 || body.byteLength > MAX_CORE_ASSET_BYTES) throw new Error(`offline asset size rejected: ${path}`);
  const headers = new Headers(response.headers);
  headers.delete("content-encoding");
  headers.delete("content-range");
  headers.delete("transfer-encoding");
  headers.set("content-length", String(body.byteLength));
  await cache.put(request, new Response(body, { status: response.status, statusText: response.statusText, headers }));
}

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME)
      .then(async (cache) => {
        for (const path of CORE) await cacheCoreAsset(cache, path);
      })
      .then(() => self.skipWaiting())
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys()
      .then((names) => Promise.all(
        names.filter((name) => name.startsWith("ckodmk-browser-") && name !== CACHE_NAME)
          .map((name) => caches.delete(name))
      ))
      .then(() => self.clients.claim())
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin || !url.pathname.startsWith(new URL("./", self.registration.scope).pathname)) return;

  if (request.mode === "navigate") {
    event.respondWith(
      fetch(request)
        .then((response) => {
          if (response.ok) caches.open(CACHE_NAME).then((cache) => cache.put("./index.html", response.clone()));
          return response;
        })
        .catch(() => caches.match("./index.html"))
    );
    return;
  }

  event.respondWith(
    caches.match(request)
      .then((cached) => cached || fetch(request).then((response) => {
        if (response.ok && response.type === "basic") {
          caches.open(CACHE_NAME).then((cache) => cache.put(request, response.clone()));
        }
        return response;
      }))
  );
});
