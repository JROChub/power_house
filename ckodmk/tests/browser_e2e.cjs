"use strict";

const puppeteer = require("puppeteer-core");
const fs = require("node:fs");
const crypto = require("node:crypto");
const childProcess = require("node:child_process");
const os = require("node:os");
const path = require("node:path");

async function waitForDownload(directory, pattern, label) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    const match = fs.readdirSync(directory).find((name) => pattern.test(name));
    if (match) return path.join(directory, match);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`${label} was not downloaded`);
}

async function clickControl(page, selector) {
  await page.$eval(selector, (node) => node.scrollIntoView({ behavior: "instant", block: "center", inline: "center" }));
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  await page.waitForFunction((value) => {
    const node = document.querySelector(value);
    if (!node) return false;
    const rect = node.getBoundingClientRect();
    const viewport = visualViewport;
    return rect.top >= viewport.offsetTop && rect.left >= viewport.offsetLeft && rect.bottom <= viewport.offsetTop + viewport.height && rect.right <= viewport.offsetLeft + viewport.width;
  }, { timeout: 5000 }, selector);
  const point = await page.$eval(selector, (node) => {
    const rect = node.getBoundingClientRect();
    return {
      x: rect.left + rect.width / 2 - visualViewport.offsetLeft,
      y: rect.top + rect.height / 2 - visualViewport.offsetTop
    };
  });
  await page.touchscreen.tap(point.x, point.y);
}

async function runGate(page, clickRun = true) {
  if (clickRun) await clickControl(page, "#run-gate");
  try {
    await page.waitForFunction(
      () => ["PASS", "BLOCK", "INCONCLUSIVE"].includes(document.querySelector("#gate-decision").textContent),
      { timeout: 180000 }
    );
  } catch (error) {
    const diagnostic = await page.evaluate(() => ({
      decision: document.querySelector("#gate-decision")?.textContent,
      rows: document.querySelector("#gate-rows")?.textContent,
      status: document.querySelector("#gate-status")?.textContent,
      progress: document.querySelector("#gate-progress")?.style.width,
      runDisabled: document.querySelector("#run-gate")?.disabled,
      cancelDisabled: document.querySelector("#cancel-gate")?.disabled
    }));
    throw new Error(`browser gate completion timeout: ${JSON.stringify(diagnostic)}; ${error.message}`);
  }
  return page.evaluate(() => ({
    decision: document.querySelector("#gate-decision").textContent,
    rows: document.querySelector("#gate-rows").textContent,
    changes: document.querySelector("#gate-changes").textContent,
    accuracy: document.querySelector("#gate-accuracy").textContent,
    linf: document.querySelector("#gate-linf").textContent,
    size: document.querySelector("#gate-size").textContent,
    status: document.querySelector("#gate-status").textContent,
    claims: [...document.querySelectorAll("#gate-claims li b")].map((node) => node.textContent),
    width: document.documentElement.scrollWidth,
    viewport: document.documentElement.clientWidth,
    overflow: [...document.querySelectorAll("body *")].map((node) => {
      const rect = node.getBoundingClientRect();
      return { tag: node.tagName, id: node.id, className: String(node.className || ""), left: rect.left, right: rect.right, width: rect.width };
    }).filter(({ left, right }) => left < -0.5 || right > document.documentElement.clientWidth + 0.5).slice(0, 12)
  }));
}

function assertResult(result, label, expected) {
  if (
    result.decision !== "PASS" ||
    result.rows !== "1,797 / 1,797" ||
    result.changes !== expected.changes ||
    result.accuracy !== expected.accuracy ||
    result.size !== expected.size ||
    result.claims.length !== 4 ||
    result.claims.join(",") !== expected.claims.join(",")
  ) throw new Error(`${label} gate mismatch: ${JSON.stringify(result)}`);
  if (result.width > result.viewport) throw new Error(`${label} page has horizontal overflow: ${JSON.stringify(result)}`);
}

const MLP_EXPECTED = Object.freeze({
  mlp16: Object.freeze({ changes: "0 / 1,797", accuracy: "0/1797", size: "5,253 to 2,723", claims: ["PASS", "PASS", "PASS", "PASS"], candidate: "mlp-w16-source.ckodmk-int8.onnx" }),
  mlp32: Object.freeze({ changes: "0 / 1,797", accuracy: "0/1797", size: "10,055 to 4,055", claims: ["PASS", "PASS", "PASS", "PASS"], candidate: "mlp-w32-source.ckodmk-int8.onnx" }),
  mlp64: Object.freeze({ changes: "2 / 1,797", accuracy: "0/1797", size: "19,658 to 6,711", claims: ["PASS", "FAIL", "PASS", "PASS"], candidate: "mlp-w64-source.ckodmk-int8.onnx" })
});
const RBF_EXPECTED = Object.freeze({
  rbf40: Object.freeze({ changes: "3 / 1,797", accuracy: "3/1797", size: "12,522 to 4,778", claims: ["PASS", "FAIL", "PASS", "PASS"], candidate: "rbf-c40-source.ckodmk-int8.onnx" }),
  rbf80: Object.freeze({ changes: "3 / 1,797", accuracy: "-1/1797", size: "24,365 to 7,738", claims: ["PASS", "FAIL", "PASS", "PASS"], candidate: "rbf-c80-source.ckodmk-int8.onnx" }),
  rbf160: Object.freeze({ changes: "6 / 1,797", accuracy: "-2/1797", size: "48,047 to 13,660", claims: ["PASS", "FAIL", "PASS", "PASS"], candidate: "rbf-c160-source.ckodmk-int8.onnx" })
});

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
    const browserModules = await page.evaluate(() => ({
      gate: Boolean(window.CKODMKBrowserGate),
      optimizer: Boolean(window.CKODMKBrowserOptimizer),
      phoneStudy: Boolean(window.CKODMKPhoneStudy),
      runtime: Boolean(window.ort)
    }));
    if (Object.values(browserModules).some((loaded) => !loaded)) {
      throw new Error(`browser module initialization failed: ${JSON.stringify({ browserModules, errors })}`);
    }
    const manifest = await page.evaluate(async () => {
      const link = document.querySelector('link[rel="manifest"]');
      const value = await fetch(link.href).then((item) => item.json());
      return { display: value.display, scope: value.scope, startUrl: value.start_url, icons: value.icons.length };
    });
    if (manifest.display !== "standalone" || manifest.scope !== "./" || manifest.startUrl !== "./" || manifest.icons !== 2) {
      throw new Error(`PWA manifest mismatch: ${JSON.stringify(manifest)}`);
    }

    const mobileControls = await page.evaluate(() => {
      const ids = ["demo-model", "run-demo", "build-candidate", "create-contract", "run-gate", "cancel-gate", "download-candidate", "download-contract", "download-report", "share-report", "run-phone-study", "cancel-phone-study", "download-phone-study", "share-phone-study"];
      return ids.map((id) => {
        const rect = document.getElementById(id).getBoundingClientRect();
        return { id, width: rect.width, height: rect.height };
      });
    });
    if (mobileControls.some(({ width, height }) => width < 44 || height < 44)) {
      throw new Error(`phone controls are smaller than 44px: ${JSON.stringify(mobileControls)}`);
    }
    await clickControl(page, "#run-demo");
    const phoneResult = await runGate(page, false);
    assertResult(phoneResult, "one tap phone example", MLP_EXPECTED.mlp16);
    const selectedFiles = await page.evaluate(() => ["gate-source", "gate-candidate", "gate-dataset", "gate-contract"].map((id) => document.getElementById(id).files.length));
    if (selectedFiles.some((count) => count !== 1)) throw new Error(`one tap example did not bind all files: ${selectedFiles}`);

    await clickControl(page, "#download-candidate");
    const candidatePath = await waitForDownload(downloads, /^mlp-w16-source\.ckodmk-int8\.onnx$/, "generated candidate");
    const candidateDigest = require("node:crypto").createHash("sha256").update(fs.readFileSync(candidatePath)).digest("hex");
    if (candidateDigest !== "d3f98ad864f5aaa270834ec788dc33f5408c6d4164c56c249f1427376ab2e782") throw new Error(`downloaded candidate digest mismatch: ${candidateDigest}`);

    await clickControl(page, "#download-report");
    const reportPath = await waitForDownload(downloads, /^ckodmk-browser-report-\d+\.json$/, "browser report");
    const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
    if (report.schema !== "mfenx/ckodmk-browser-report/v1" || report.decision !== "PASS" || report.measurements.samples !== 1797 || report.claims.some((claim) => claim.result !== "PASS") || report.candidate_generation?.candidate_bytes !== 2723) {
      throw new Error(`downloaded report mismatch: ${reportPath}`);
    }
    await clickControl(page, "#run-phone-study");
    await page.waitForFunction(() => ["COMPLETE", "STOPPED"].includes(document.querySelector("#phone-study-state").textContent), { timeout: 60000 });
    const timingState = await page.evaluate(() => ({ state: document.querySelector("#phone-study-state").textContent, status: document.querySelector("#phone-study-status").textContent }));
    if (timingState.state !== "COMPLETE") throw new Error(`phone timing stopped: ${JSON.stringify(timingState)}`);
    const timingUi = await page.evaluate(() => ({
      sourceP50: document.querySelector("#phone-source-p50").textContent,
      sourceP95: document.querySelector("#phone-source-p95").textContent,
      candidateP50: document.querySelector("#phone-candidate-p50").textContent,
      candidateP95: document.querySelector("#phone-candidate-p95").textContent,
      downloadDisabled: document.querySelector("#download-phone-study").disabled
    }));
    if (timingUi.downloadDisabled || Object.values(timingUi).slice(0, 4).some((value) => !/^\d+\.\d{3} ms$/.test(value))) throw new Error(`phone timing UI mismatch: ${JSON.stringify(timingUi)}`);
    await clickControl(page, "#download-phone-study");
    const timingPath = await waitForDownload(downloads, /^ckodmk-phone-study-\d+\.json$/, "phone timing report");
    const timingReport = JSON.parse(fs.readFileSync(timingPath, "utf8"));
    if (
      timingReport.schema !== "mfenx/ckodmk-browser-phone-study/v2" ||
      timingReport.target_declaration !== null ||
      !/^[0-9a-f]{64}$/.test(timingReport.session_nonce) ||
      timingReport.execution.warmup_pairs !== 12 || timingReport.execution.measured_pairs !== 60 || timingReport.execution.inner_repetitions !== 128 ||
      timingReport.observations.pairs.length !== 60 || timingReport.privacy.files_uploaded !== false ||
      timingReport.privacy.network_result_submission !== false || timingReport.privacy.automatic_device_identifiers_collected.length !== 0 ||
      "user_agent" in timingReport || "operating_system" in timingReport || "processor" in timingReport
    ) throw new Error(`phone timing report mismatch: ${timingPath}`);
    const timingDigest = crypto.createHash("sha256").update(fs.readFileSync(timingPath)).digest("hex");
    const timingVerification = JSON.parse(childProcess.execFileSync(
      process.env.CKODMK_PYTHON || "python3",
      [path.resolve(__dirname, "../../scripts/verify_phone_study.py"), timingPath, "--sha256", `sha256:${timingDigest}`],
      { encoding: "utf8" }
    ));
    if (timingVerification.decision !== "STRUCTURE_AND_ARITHMETIC_VERIFIED" || timingVerification.target_record_status !== "ABSENT" || timingVerification.physical_target_attested !== false) throw new Error(`phone timing verifier mismatch: ${JSON.stringify(timingVerification)}`);
    await page.evaluate(() => {
      document.querySelector("#run-phone-study").click();
      setTimeout(() => document.querySelector("#cancel-phone-study").click(), 0);
    });
    await page.waitForFunction(() => document.querySelector("#phone-study-state").textContent === "STOPPED", { timeout: 30000 });
    const cancelledStudy = await page.evaluate(() => ({
      status: document.querySelector("#phone-study-status").textContent,
      downloadDisabled: document.querySelector("#download-phone-study").disabled
    }));
    if (!cancelledStudy.downloadDisabled || !cancelledStudy.status.includes("cancelled")) throw new Error(`phone timing cancellation mismatch: ${JSON.stringify(cancelledStudy)}`);

    const mlpResults = { mlp16: phoneResult };
    for (const profile of ["mlp32", "mlp64"]) {
      const expected = MLP_EXPECTED[profile];
      await page.select("#demo-model", profile);
      await clickControl(page, "#run-demo");
      const result = await runGate(page, false);
      assertResult(result, `one tap ${profile} phone example`, expected);
      const candidateName = await page.evaluate(() => window.CKODMKBrowserGate && document.querySelector("#candidate-name").textContent);
      if (candidateName !== expected.candidate) throw new Error(`${profile} candidate name mismatch: ${candidateName}`);
      mlpResults[profile] = result;
    }

    const rbfResults = {};
    for (const [profile, expected] of Object.entries(RBF_EXPECTED)) {
      await page.select("#demo-model", profile);
      await clickControl(page, "#run-demo");
      const result = await runGate(page, false);
      assertResult(result, `one tap ${profile} phone example`, expected);
      const candidateName = await page.evaluate(() => window.CKODMKBrowserGate && document.querySelector("#candidate-name").textContent);
      if (candidateName !== expected.candidate) throw new Error(`${profile} candidate name mismatch: ${candidateName}`);
      rbfResults[profile] = result;
    }
    await clickControl(page, "#create-contract");
    await page.waitForFunction(() => document.querySelector("#gate-decision").textContent === "READY" && !document.querySelector("#download-contract").disabled, { timeout: 30000 });
    const localRbfResult = await runGate(page);
    assertResult(localRbfResult, "RBF local contract workflow", RBF_EXPECTED.rbf160);

    await page.reload({ waitUntil: "networkidle0", timeout: 30000 });
    await (await page.$("#gate-source")).uploadFile(path.join(webRoot, "demo/source.onnx"));
    await (await page.$("#gate-dataset")).uploadFile(path.join(webRoot, "demo/optdigits-official-test.npz"));
    await clickControl(page, "#build-candidate");
    await page.waitForFunction(() => document.querySelector("#gate-decision").textContent === "READY" && !document.querySelector("#download-candidate").disabled, { timeout: 30000 });
    await clickControl(page, "#create-contract");
    await page.waitForFunction(() => document.querySelector("#gate-decision").textContent === "READY" && !document.querySelector("#download-contract").disabled, { timeout: 30000 });
    const localContractState = await page.evaluate(() => ({
      digest: document.querySelector("#gate-contract-digest").value,
      contractFiles: document.querySelector("#gate-contract").files.length,
      candidateFiles: document.querySelector("#gate-candidate").files.length
    }));
    if (!/^sha256:[0-9a-f]{64}$/.test(localContractState.digest) || localContractState.contractFiles !== 1 || localContractState.candidateFiles !== 1) throw new Error(`local contract workflow mismatch: ${JSON.stringify(localContractState)}`);
    const localResult = await runGate(page);
    assertResult(localResult, "phone source to candidate workflow", MLP_EXPECTED.mlp16);

    await page.setOfflineMode(true);
    await page.reload({ waitUntil: "domcontentloaded", timeout: 30000 });
    await page.select("#demo-model", "rbf160");
    await clickControl(page, "#run-demo");
    await page.waitForFunction(
      () => ["PASS", "BLOCK", "INCONCLUSIVE"].includes(document.querySelector("#gate-decision").textContent),
      { timeout: 180000 }
    );
    const offlineResult = await page.evaluate(() => ({
      decision: document.querySelector("#gate-decision").textContent,
      rows: document.querySelector("#gate-rows").textContent,
      changes: document.querySelector("#gate-changes").textContent,
      accuracy: document.querySelector("#gate-accuracy").textContent,
      linf: document.querySelector("#gate-linf").textContent,
      size: document.querySelector("#gate-size").textContent,
      status: document.querySelector("#gate-status").textContent,
      claims: [...document.querySelectorAll("#gate-claims li b")].map((node) => node.textContent),
      width: document.documentElement.scrollWidth,
      viewport: document.documentElement.clientWidth
    }));
    assertResult(offlineResult, "offline RBF phone", RBF_EXPECTED.rbf160);
    if (errors.length) throw new Error(`browser console errors: ${JSON.stringify(errors)}`);
    console.log("CKODMK_MOBILE_OFFLINE_E2E_OK", JSON.stringify({ mlpResults, rbfResults, localRbfResult, localResult, offlineResult, candidate_sha256: candidateDigest, report: path.basename(reportPath), phone_timing_report: path.basename(timingPath) }));
  } finally {
    await browser.close();
    fs.rmSync(downloads, { recursive: true, force: true });
  }
})().catch((error) => { console.error(error); process.exit(1); });
