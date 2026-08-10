"use strict";

const puppeteer = require("puppeteer-core");

(async () => {
  const executablePath = process.env.CKODMK_CHROME;
  if (!executablePath) throw new Error("CKODMK_CHROME is required");
  const baseUrl = process.env.CKODMK_BASE_URL || "http://127.0.0.1:8765";
  const route = process.env.CKODMK_ROUTE || "/ckodmk/";
  const browser = await puppeteer.launch({ headless: true, executablePath, args: ["--no-sandbox"] });
  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 1440, height: 1000 });
    const errors = [];
    page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
    page.on("pageerror", (error) => errors.push(error.message));
    const response = await page.goto(new URL(route, baseUrl).href, { waitUntil: "networkidle0", timeout: 30000 });
    if (!response || response.status() !== 200) throw new Error(`page HTTP status ${response?.status()}`);
    await page.click("#load-demo");
    await page.waitForFunction(() => document.querySelector("#gate-decision").textContent === "READY", { timeout: 30000 });
    await page.click("#run-gate");
    await page.waitForFunction(() => ["PASS", "BLOCK", "INCONCLUSIVE"].includes(document.querySelector("#gate-decision").textContent), { timeout: 180000 });
    const result = await page.evaluate(() => ({
      decision: document.querySelector("#gate-decision").textContent,
      rows: document.querySelector("#gate-rows").textContent,
      changes: document.querySelector("#gate-changes").textContent,
      accuracy: document.querySelector("#gate-accuracy").textContent,
      linf: document.querySelector("#gate-linf").textContent,
      claims: [...document.querySelectorAll("#gate-claims li b")].map((node) => node.textContent),
      width: document.documentElement.scrollWidth,
      viewport: document.documentElement.clientWidth
    }));
    if (result.decision !== "PASS" || result.rows !== "1,797 / 1,797" || result.changes !== "0 / 1,797" || result.accuracy !== "0/1797" || result.claims.some((claim) => claim !== "PASS") || errors.length) throw new Error(`browser gate mismatch: ${JSON.stringify({ result, errors })}`);
    if (result.width > result.viewport) throw new Error("desktop page has horizontal overflow");

    await page.setViewport({ width: 390, height: 844, isMobile: true });
    await page.reload({ waitUntil: "networkidle0" });
    const mobile = await page.evaluate(() => ({ width: document.documentElement.scrollWidth, viewport: document.documentElement.clientWidth, gateVisible: Boolean(document.querySelector("#gate-form").getBoundingClientRect().width) }));
    if (mobile.width > mobile.viewport || !mobile.gateVisible) throw new Error(`mobile layout failed: ${JSON.stringify(mobile)}`);
    console.log("CKODMK_BROWSER_E2E_OK", JSON.stringify(result));
  } finally {
    await browser.close();
  }
})().catch((error) => { console.error(error); process.exit(1); });
