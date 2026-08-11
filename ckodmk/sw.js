"use strict";

const CACHE_NAME = "ckodmk-browser-v0.3.0-2";
const MAX_CORE_ASSET_BYTES = 16 * 1024 * 1024;
const CORE = Object.freeze([
  "./",
  "./index.html",
  "./manifest.webmanifest",
  "./styles.css?v=20260811b",
  "./app.js?v=20260810h",
  "./browser-optimizer.js?v=20260810f",
  "./browser-gate.js?v=20260811a",
  "./pcm.js?v=20260811b",
  "./phone-study.js?v=20260810f",
  "./evidence.json",
  "./assets/powerhouse-logo.svg",
  "./assets/icon-192.png",
  "./assets/icon-512.png",
  "./demo/optdigits-official-test.npz",
  "./demo/cnn/source.onnx",
  "./demo/cnn/candidate-int8.onnx",
  "./demo/cnn/browser-contract.json",
  "./pcm/optdigits-cnn-int8.pcm",
  "./trust/pcm-release-public.der",
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
        .then(async (response) => {
          if (response.ok) {
            const cache = await caches.open(CACHE_NAME);
            await cache.put("./index.html", response.clone());
          }
          return response;
        })
        .catch(() => caches.match("./index.html"))
    );
    return;
  }

  event.respondWith(
    caches.match(request)
      .then((cached) => cached || fetch(request).then(async (response) => {
        if (response.ok && response.type === "basic") {
          const cache = await caches.open(CACHE_NAME);
          await cache.put(request, response.clone());
        }
        return response;
      }))
  );
});
