"use strict";

const puppeteer = require("puppeteer-core");

async function waitForResult(page) {
  await page.waitForFunction(
    () => ["PASS", "BLOCK", "INCONCLUSIVE"].includes(document.querySelector("#gate-decision")?.textContent),
    { timeout: 180000 }
  );
  return page.evaluate(async () => {
    const digest = async (file) => {
      const value = await crypto.subtle.digest("SHA-256", await file.arrayBuffer());
      return [...new Uint8Array(value)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
    };
    const source = document.querySelector("#gate-source").files[0];
    const candidate = document.querySelector("#gate-candidate").files[0];
    return {
      decision: document.querySelector("#gate-decision").textContent,
      status: document.querySelector("#gate-status").textContent,
      rows: document.querySelector("#gate-rows").textContent,
      changes: document.querySelector("#gate-changes").textContent,
      accuracy: document.querySelector("#gate-accuracy").textContent,
      size: document.querySelector("#gate-size").textContent,
      sourceBytes: source.size,
      candidateBytes: candidate.size,
      sourceDigest: await digest(source),
      candidateDigest: await digest(candidate),
      width: document.documentElement.scrollWidth,
      viewport: document.documentElement.clientWidth
    };
  });
}

function assertMobileNet(result, phase) {
  const expected = {
    decision: "BLOCK",
    rows: "1 / 1",
    changes: "1 / 1",
    accuracy: "-1/1",
    size: "13,964,571 to 3,655,033",
    sourceBytes: 13964571,
    candidateBytes: 3655033,
    sourceDigest: "c0c3f76d93fa3fd6580652a45618618a220fced18babf65774ed169de0432ad5",
    candidateDigest: "cc028fe6cae7bc11a4ff53cfc9b79c920e8be65ce33a904ec3e2a8f66d77f95f"
  };
  for (const [key, value] of Object.entries(expected)) {
    if (result[key] !== value) throw new Error(`${phase} MobileNet ${key} mismatch: ${JSON.stringify(result)}`);
  }
  if (!result.status.includes("required claim failed") || result.width > result.viewport) throw new Error(`${phase} MobileNet UI mismatch: ${JSON.stringify(result)}`);
}

async function runExample(page) {
  await page.select("#demo-model", "mobilenet");
  await page.$eval("#run-demo", (node) => node.scrollIntoView({ block: "center" }));
  await page.click("#run-demo");
  return waitForResult(page);
}

(async () => {
  const executablePath = process.env.CKODMK_CHROME;
  if (!executablePath) throw new Error("CKODMK_CHROME is required");
  const baseUrl = process.env.CKODMK_BASE_URL || "http://127.0.0.1:8765";
  const route = process.env.CKODMK_ROUTE || "/ckodmk/";
  const browser = await puppeteer.launch({ headless: true, executablePath, args: ["--no-sandbox"] });
  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 390, height: 844, isMobile: true, hasTouch: true, deviceScaleFactor: 2 });
    const errors = [];
    page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
    page.on("pageerror", (error) => errors.push(error.message));
    const url = new URL(route, baseUrl).href;
    const response = await page.goto(url, { waitUntil: "networkidle0", timeout: 60000 });
    if (!response || response.status() !== 200) throw new Error(`page HTTP status ${response?.status()}`);
    await page.waitForFunction(() => document.documentElement.dataset.offlineReady === "true", { timeout: 120000 });
    assertMobileNet(await runExample(page), "online");

    await page.setOfflineMode(true);
    await page.reload({ waitUntil: "domcontentloaded", timeout: 60000 });
    await page.waitForFunction(() => Boolean(window.CKODMKBrowserGate && window.ort), { timeout: 30000 });
    assertMobileNet(await runExample(page), "offline");
    if (errors.length) throw new Error(`browser errors: ${JSON.stringify(errors)}`);
    console.log("CKODMK_MOBILENET_MOBILE_OFFLINE_OK");
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
