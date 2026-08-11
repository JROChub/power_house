"use strict";

const candidates = Object.freeze({
  "mobilenet-int8": { model: "PRETRAINED MOBILENET V2 / STATIC INT8", accuracy: "67.923567% → 66.904459%", changes: "491 / 3,925", bytes: "13,964,571 → 3,661,816", ratio: "3.813564× smaller", latency: "55.301227 → 55.437905 ms", linf: "7.575401306", note: "Calibration used 100 training images. Evaluation used all 3,925 Imagenette validation images. The candidate is smaller and lost 1.019108 percentage points of accuracy in this test." },
  "mnist-int8": { model: "PRETRAINED MNIST CNN / STATIC INT8", accuracy: "98.90% → 98.94%", changes: "7 / 10,000", bytes: "26,143 → 18,440", ratio: "1.417733× smaller", latency: "Candidate slower in recorded CPU run", linf: "5.036161422", note: "The source is the ONNX Model Zoo validated MNIST-12 CNN. Calibration used 1,000 training images; evaluation used all 10,000 test images." },
  "w16-int8": { model: "WIDTH 16 / DYNAMIC INT8", accuracy: "95.882026% → 95.882026%", changes: "0 / 1,797", bytes: "5,253 → 3,106", ratio: "1.691243× smaller", latency: "0.032763 → 0.043051 ms", linf: "0.202297449", note: "Smaller, but slower in the recorded test. Accuracy stayed within the configured limit." },
  "w32-int8": { model: "WIDTH 32 / DYNAMIC INT8", accuracy: "96.215915% → 96.215915%", changes: "0 / 1,797", bytes: "10,055 → 4,438", ratio: "2.265660× smaller", latency: "0.035006 → 0.044239 ms", linf: "0.218335152", note: "Smaller, but slower in the recorded test. Accuracy stayed within the configured limit." },
  "w64-int8": { model: "WIDTH 64 / DYNAMIC INT8", accuracy: "96.160267% → 96.160267%", changes: "2 / 1,797", bytes: "19,658 → 7,094", ratio: "2.771074× smaller", latency: "0.036781 → 0.047777 ms", linf: "0.181334734", note: "Smaller, but two classifications changed. Complete-set accuracy stayed unchanged." },
  "w16-graph": { model: "WIDTH 16 / ORT GRAPH", accuracy: "95.882026% → 95.882026%", changes: "0 / 1,797", bytes: "5,253 → 5,640", ratio: "0.931383× (larger)", latency: "0.033230 → 0.031367 ms", linf: "0", note: "Outputs matched bit for bit on the bound finite set and the candidate was slightly faster in this run, but its file is larger." },
  "w32-graph": { model: "WIDTH 32 / ORT GRAPH", accuracy: "96.215915% → 96.215915%", changes: "0 / 1,797", bytes: "10,055 → 10,442", ratio: "0.962938× (larger)", latency: "0.034000 → 0.029583 ms", linf: "0", note: "Outputs matched bit for bit on the bound finite set and the candidate was slightly faster in this run, but its file is larger." },
  "w64-graph": { model: "WIDTH 64 / ORT GRAPH", accuracy: "96.160267% → 96.160267%", changes: "0 / 1,797", bytes: "19,658 → 20,045", ratio: "0.980693× (larger)", latency: "0.032923 → 0.028638 ms", linf: "0", note: "Outputs matched bit for bit on the bound finite set and the candidate was slightly faster in this run, but its file is larger." }
});

const resultFields = Object.freeze({
  model: "result-model", accuracy: "result-accuracy", changes: "result-changes",
  bytes: "result-bytes", ratio: "result-ratio", latency: "result-latency",
  linf: "result-linf", note: "result-note"
});

document.querySelectorAll("[data-candidate]").forEach((button) => {
  button.addEventListener("click", () => {
    const result = candidates[button.dataset.candidate];
    if (!result) return;
    document.querySelectorAll("[data-candidate]").forEach((item) => item.classList.toggle("active", item === button));
    Object.entries(resultFields).forEach(([key, id]) => {
      const target = document.getElementById(id);
      if (target) target.textContent = result[key];
    });
  });
});

document.querySelectorAll("[data-copy]").forEach((button) => {
  button.addEventListener("click", async () => {
    const previous = button.textContent;
    try {
      await navigator.clipboard.writeText(button.dataset.copy);
      button.textContent = "COPIED";
    } catch {
      button.textContent = "SELECT";
    }
    window.setTimeout(() => { button.textContent = previous; }, 1500);
  });
});

let installPrompt = null;
const installButton = document.getElementById("install-app");
const installStatus = document.getElementById("install-status");

function setInstallStatus(message) {
  if (installStatus) installStatus.textContent = message;
}

window.addEventListener("beforeinstallprompt", (event) => {
  event.preventDefault();
  installPrompt = event;
  if (installButton) installButton.disabled = false;
  setInstallStatus("This browser can install CKODMK on the home screen.");
});

window.addEventListener("appinstalled", () => {
  installPrompt = null;
  if (installButton) installButton.disabled = true;
  setInstallStatus("CKODMK is installed on this device.");
});

if (installButton) {
  installButton.addEventListener("click", async () => {
    if (installPrompt) {
      await installPrompt.prompt();
      const choice = await installPrompt.userChoice;
      setInstallStatus(choice.outcome === "accepted" ? "Installation accepted." : "Installation was not completed.");
      installPrompt = null;
      return;
    }
    if (window.matchMedia("(display-mode: standalone)").matches) {
      setInstallStatus("CKODMK is already running as an installed application.");
      return;
    }
    setInstallStatus("Use the browser menu and choose Add to Home Screen or Install App.");
  });
}

if ("serviceWorker" in navigator) {
  window.addEventListener("load", async () => {
    try {
      await navigator.serviceWorker.register("sw.js", { scope: "./", updateViaCache: "none" });
      await Promise.race([
        navigator.serviceWorker.ready,
        new Promise((_, reject) => window.setTimeout(() => reject(new Error("offline installation timed out")), 30000))
      ]);
      document.documentElement.dataset.offlineReady = "true";
      setInstallStatus("CKODMK is ready for home screen installation and offline use of the included trained models.");
    } catch {
      document.documentElement.dataset.offlineReady = "false";
      setInstallStatus("Offline installation is unavailable. Browser verification still works while connected.");
    }
  });
}
