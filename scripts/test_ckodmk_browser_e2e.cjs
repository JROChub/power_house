"use strict";

const puppeteer = require("puppeteer-core");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

async function waitForDownloadedReport(directory) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    const match = fs.readdirSync(directory).find((name) => /^ckodmk-browser-report-\d+\.json$/.test(name));
    if (match) return path.join(directory, match);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("browser report was not downloaded");
}

async function runGate(page) {
  await page.click("#run-gate");
  await page.waitForFunction(
    () => ["PASS", "BLOCK", "INCONCLUSIVE"].includes(document.querySelector("#gate-decision").textContent),
    { timeout: 180000 }
  );
  return page.evaluate(() => ({
    decision: document.querySelector("#gate-decision").textContent,
    rows: document.querySelector("#gate-rows").textContent,
    changes: document.querySelector("#gate-changes").textContent,
    accuracy: document.querySelector("#gate-accuracy").textContent,
    linf: document.querySelector("#gate-linf").textContent,
    status: document.querySelector("#gate-status").textContent,
    claims: [...document.querySelectorAll("#gate-claims li b")].map((node) => node.textContent),
    width: document.documentElement.scrollWidth,
    viewport: document.documentElement.clientWidth
  }));
}

function assertPassingResult(result, label) {
  if (
    result.decision !== "PASS" ||
    result.rows !== "1,797 / 1,797" ||
    result.changes !== "0 / 1,797" ||
    result.accuracy !== "0/1797" ||
    result.claims.length !== 4 ||
    result.claims.some((claim) => claim !== "PASS")
  ) throw new Error(`${label} gate mismatch: ${JSON.stringify(result)}`);
  if (result.width > result.viewport) throw new Error(`${label} page has horizontal overflow`);
}

(async () => {
  const executablePath = process.env.CKODMK_CHROME;
  if (!executablePath) throw new Error("CKODMK_CHROME is required");
  const baseUrl = process.env.CKODMK_BASE_URL || "http://127.0.0.1:8765";
  const route = process.env.CKODMK_ROUTE || "/ckodmk/";
  const webRoot = path.resolve(process.env.CKODMK_WEB_ROOT || path.join(__dirname, ".."));
  const downloads = fs.mkdtempSync(path.join(os.tmpdir(), "ckodmk-browser-downloads-"));
  const browser = await puppeteer.launch({ headless: true, executablePath, args: ["--no-sandbox"] });
  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 390, height: 844, isMobile: true, hasTouch: true, deviceScaleFactor: 2 });
    const cdp = await page.createCDPSession();
    await cdp.send("Browser.setDownloadBehavior", { behavior: "allow", downloadPath: downloads });
    const errors = [];
    page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
    page.on("pageerror", (error) => errors.push(error.message));
    const response = await page.goto(new URL(route, baseUrl).href, { waitUntil: "networkidle0", timeout: 30000 });
    if (!response || response.status() !== 200) throw new Error(`page HTTP status ${response?.status()}`);

    await page.waitForFunction(
      async () => document.documentElement.dataset.offlineReady === "true" && Boolean(await navigator.serviceWorker.getRegistration()),
      { timeout: 30000 }
    );
    const manifest = await page.evaluate(async () => {
      const link = document.querySelector('link[rel="manifest"]');
      const value = await fetch(link.href).then((item) => item.json());
      return { display: value.display, scope: value.scope, startUrl: value.start_url, icons: value.icons.length };
    });
    if (manifest.display !== "standalone" || manifest.scope !== "./" || manifest.startUrl !== "./" || manifest.icons !== 2) {
      throw new Error(`PWA manifest mismatch: ${JSON.stringify(manifest)}`);
    }

    const files = [
      ["#gate-source", "demo/source.onnx"],
      ["#gate-candidate", "demo/candidate-int8.onnx"],
      ["#gate-dataset", "demo/optdigits-official-test.npz"],
      ["#gate-contract", "demo/browser-contract.json"]
    ];
    for (const [selector, relative] of files) {
      await (await page.$(selector)).uploadFile(path.join(webRoot, relative));
    }
    await page.type("#gate-contract-digest", "sha256:f03b6dc284ecd140ea9d7a95c88711079eb585482eb3993173527ceab4a18cf5");
    const selectedFiles = await page.evaluate(() => ["gate-source", "gate-candidate", "gate-dataset", "gate-contract"].map((id) => document.getElementById(id).files.length));
    if (selectedFiles.some((count) => count !== 1)) throw new Error(`phone file selection failed: ${selectedFiles}`);
    const phoneResult = await runGate(page);
    assertPassingResult(phoneResult, "phone file selection");

    await page.click("#download-report");
    const reportPath = await waitForDownloadedReport(downloads);
    const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
    if (report.schema !== "mfenx/ckodmk-browser-report/v1" || report.decision !== "PASS" || report.measurements.samples !== 1797 || report.claims.some((claim) => claim.result !== "PASS")) {
      throw new Error(`downloaded report mismatch: ${reportPath}`);
    }

    await page.setOfflineMode(true);
    await page.reload({ waitUntil: "domcontentloaded", timeout: 30000 });
    await page.click("#load-demo");
    await page.waitForFunction(
      () =>
        document.querySelector("#gate-decision").textContent === "READY" &&
        document.querySelector("#gate-status").textContent.startsWith("Real trained-model demo loaded") &&
        ["gate-source", "gate-candidate", "gate-dataset", "gate-contract"].every((id) => document.getElementById(id).files.length === 1),
      { timeout: 30000 }
    );
    const controlsEnabled = await page.evaluate(() => !document.querySelector("#load-demo").disabled && !document.querySelector("#run-gate").disabled);
    if (!controlsEnabled) throw new Error("retained model controls stayed disabled after loading");
    const offlineResult = await runGate(page);
    assertPassingResult(offlineResult, "offline phone");
    if (errors.length) throw new Error(`browser console errors: ${JSON.stringify(errors)}`);
    console.log("CKODMK_MOBILE_OFFLINE_E2E_OK", JSON.stringify({ phoneResult, offlineResult, report: path.basename(reportPath) }));
  } finally {
    await browser.close();
    fs.rmSync(downloads, { recursive: true, force: true });
  }
})().catch((error) => { console.error(error); process.exit(1); });
