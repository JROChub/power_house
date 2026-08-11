"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { webkit } = require("playwright-core");

const serviceWorkerSource = fs.readFileSync(path.join(__dirname, "..", "sw.js"), "utf8");
const cacheNameMatch = serviceWorkerSource.match(/^const CACHE_NAME = "([A-Za-z0-9._-]+)";$/m);
if (!cacheNameMatch) throw new Error("service-worker cache name is missing or malformed");
const EXPECTED_CACHE_NAME = cacheNameMatch[1];

const EXPECTED = Object.freeze({
  mlp16: Object.freeze({ changes: "0 / 1,797", accuracy: "0/1797", size: "5,253 to 2,723", claims: ["PASS", "PASS", "PASS", "PASS"], candidate: "mlp-w16-source.ckodmk-int8.onnx" }),
  mlp32: Object.freeze({ changes: "0 / 1,797", accuracy: "0/1797", size: "10,055 to 4,055", claims: ["PASS", "PASS", "PASS", "PASS"], candidate: "mlp-w32-source.ckodmk-int8.onnx" }),
  mlp64: Object.freeze({ changes: "2 / 1,797", accuracy: "0/1797", size: "19,658 to 6,711", claims: ["PASS", "FAIL", "PASS", "PASS"], candidate: "mlp-w64-source.ckodmk-int8.onnx" }),
  rbf40: Object.freeze({ changes: "3 / 1,797", accuracy: "3/1797", size: "12,522 to 4,778", claims: ["PASS", "FAIL", "PASS", "PASS"], candidate: "rbf-c40-source.ckodmk-int8.onnx" }),
  rbf80: Object.freeze({ changes: "3 / 1,797", accuracy: "-1/1797", size: "24,365 to 7,738", claims: ["PASS", "FAIL", "PASS", "PASS"], candidate: "rbf-c80-source.ckodmk-int8.onnx" }),
  rbf160: Object.freeze({ changes: "6 / 1,797", accuracy: "-2/1797", size: "48,047 to 13,660", claims: ["PASS", "FAIL", "PASS", "PASS"], candidate: "rbf-c160-source.ckodmk-int8.onnx" })
});

async function result(page) {
  await page.waitForFunction(() => ["PASS", "BLOCK", "INCONCLUSIVE"].includes(document.querySelector("#gate-decision").textContent), null, { timeout: 180000 });
  return page.evaluate(() => ({
    decision: document.querySelector("#gate-decision").textContent,
    rows: document.querySelector("#gate-rows").textContent,
    changes: document.querySelector("#gate-changes").textContent,
    accuracy: document.querySelector("#gate-accuracy").textContent,
    size: document.querySelector("#gate-size").textContent,
    claims: [...document.querySelectorAll("#gate-claims li b")].map((node) => node.textContent),
    candidate: document.querySelector("#candidate-name").textContent,
    width: document.documentElement.scrollWidth,
    viewport: document.documentElement.clientWidth
  }));
}

function assertResult(actual, profile, candidate = null) {
  const expected = EXPECTED[profile];
  if (
    actual.decision !== "PASS" ||
    actual.rows !== "1,797 / 1,797" ||
    actual.changes !== expected.changes ||
    actual.accuracy !== expected.accuracy ||
    actual.size !== expected.size ||
    actual.claims.join(",") !== expected.claims.join(",") ||
    actual.candidate !== (candidate || expected.candidate) ||
    actual.width > actual.viewport
  ) throw new Error(`${profile} WebKit result mismatch: ${JSON.stringify(actual)}`);
}

(async () => {
  const baseUrl = process.env.CKODMK_BASE_URL || "http://127.0.0.1:8765";
  const route = process.env.CKODMK_ROUTE || "/";
  const webRoot = path.resolve(process.env.CKODMK_WEB_ROOT || path.join(__dirname, ".."));
  const output = fs.mkdtempSync(path.join(os.tmpdir(), "ckodmk-webkit-"));
  const browser = await webkit.launch({ headless: true });
  try {
    const context = await browser.newContext({
      ignoreHTTPSErrors: true,
      viewport: { width: 390, height: 844 },
      deviceScaleFactor: 3,
      hasTouch: true,
      isMobile: true,
      userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Mobile/15E148 Safari/604.1"
    });
    const page = await context.newPage();
    const errors = [];
    page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
    page.on("pageerror", (error) => errors.push(error.message));
    const response = await page.goto(new URL(route, baseUrl).href, { waitUntil: "networkidle", timeout: 30000 });
    if (!response || response.status() !== 200) throw new Error(`WebKit page HTTP status ${response?.status()}`);
    await page.waitForFunction(async () => document.documentElement.dataset.offlineReady === "true" && Boolean(await navigator.serviceWorker.getRegistration()), null, { timeout: 30000 });

    for (const profile of Object.keys(EXPECTED)) {
      await page.locator("#demo-model").selectOption(profile);
      await page.locator("#run-demo").click();
      assertResult(await result(page), profile);
    }

    await page.reload({ waitUntil: "networkidle", timeout: 30000 });
    await page.locator("#gate-source").setInputFiles(path.join(webRoot, "demo/source.onnx"));
    await page.locator("#gate-dataset").setInputFiles(path.join(webRoot, "demo/optdigits-official-test.npz"));
    await page.locator("#build-candidate").click();
    await page.waitForFunction(() => document.querySelector("#gate-decision").textContent === "READY" && !document.querySelector("#download-candidate").disabled, null, { timeout: 30000 });
    await page.locator("#create-contract").click();
    await page.waitForFunction(() => document.querySelector("#gate-decision").textContent === "READY" && !document.querySelector("#download-contract").disabled, null, { timeout: 30000 });
    await page.locator("#run-gate").click();
    const custom = await result(page);
    assertResult(custom, "mlp16", "source.ckodmk-int8.onnx");
    const downloadPromise = page.waitForEvent("download");
    await page.locator("#download-report").click();
    const download = await downloadPromise;
    const reportPath = path.join(output, download.suggestedFilename());
    await download.saveAs(reportPath);
    const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
    if (report.schema !== "mfenx/ckodmk-browser-report/v1" || report.decision !== "PASS" || report.measurements.samples !== 1797) throw new Error("WebKit report download is invalid");
    await page.locator("#run-phone-study").click();
    await page.waitForFunction(() => ["COMPLETE", "STOPPED"].includes(document.querySelector("#phone-study-state").textContent), null, { timeout: 60000 });
    const timingState = await page.evaluate(() => ({ state: document.querySelector("#phone-study-state").textContent, status: document.querySelector("#phone-study-status").textContent }));
    if (timingState.state !== "COMPLETE") throw new Error(`WebKit phone timing stopped: ${JSON.stringify(timingState)}`);
    const timingDownloadPromise = page.waitForEvent("download");
    await page.locator("#download-phone-study").click();
    const timingDownload = await timingDownloadPromise;
    const timingPath = path.join(output, timingDownload.suggestedFilename());
    await timingDownload.saveAs(timingPath);
    const timing = JSON.parse(fs.readFileSync(timingPath, "utf8"));
    if (timing.schema !== "mfenx/ckodmk-browser-phone-study/v2" || timing.target_declaration !== null || !/^[0-9a-f]{64}$/.test(timing.session_nonce) || timing.execution.inner_repetitions !== 128 || timing.observations.pairs.length !== 60 || timing.privacy.automatic_device_identifiers_collected.length !== 0) throw new Error("WebKit phone timing report is invalid");

    const cached = await page.evaluate(async () => {
      const paths = ["./index.html", "./browser-gate.js?v=20260810i", "./phone-study.js?v=20260810d", "./demo/optdigits-official-test.npz", "./demo/mlp-w64/source.onnx", "./demo/rbf-c160/source.onnx", "./vendor/ort/ort-wasm-simd-threaded.wasm"];
      const values = await Promise.all(paths.map(async (item) => ({ item, present: Boolean(await caches.match(item)) })));
      return { cacheNames: await caches.keys(), values };
    });
    if (cached.cacheNames.length !== 1 || cached.cacheNames[0] !== EXPECTED_CACHE_NAME || cached.values.some((item) => !item.present)) throw new Error(`WebKit offline cache is incomplete: ${JSON.stringify(cached)}`);
    if (errors.length) throw new Error(`WebKit console errors: ${JSON.stringify(errors)}`);
    console.log("CKODMK_WEBKIT_MOBILE_E2E_OK", JSON.stringify({ profiles: Object.keys(EXPECTED).length, custom: custom.decision, phone_timing: "complete", cache: "complete" }));
    await context.close();
  } finally {
    await browser.close();
    fs.rmSync(output, { recursive: true, force: true });
  }
})().catch((error) => { console.error(error); process.exit(1); });
