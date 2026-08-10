"use strict";

const candidates = Object.freeze({
  "w16-int8": { model: "WIDTH 16 / DYNAMIC INT8", accuracy: "95.882026% → 95.882026%", changes: "0 / 1,797", bytes: "5,253 → 3,106", ratio: "1.691243× smaller", latency: "0.032763 → 0.043051 ms", linf: "0.202297449", note: "Smaller, but slower on the recorded private test host. The required accuracy and absolute latency claims passed." },
  "w32-int8": { model: "WIDTH 32 / DYNAMIC INT8", accuracy: "96.215915% → 96.215915%", changes: "0 / 1,797", bytes: "10,055 → 4,438", ratio: "2.265660× smaller", latency: "0.035006 → 0.044239 ms", linf: "0.218335152", note: "Smaller, but slower on the recorded private test host. The required accuracy and absolute latency claims passed." },
  "w64-int8": { model: "WIDTH 64 / DYNAMIC INT8", accuracy: "96.160267% → 96.160267%", changes: "2 / 1,797", bytes: "19,658 → 7,094", ratio: "2.771074× smaller", latency: "0.036781 → 0.047777 ms", linf: "0.181334734", note: "Smaller, but slower on the recorded private test host. Decision invariance failed as an advisory claim; the required finite set accuracy delta remained exactly zero." },
  "w16-graph": { model: "WIDTH 16 / ORT GRAPH", accuracy: "95.882026% → 95.882026%", changes: "0 / 1,797", bytes: "5,253 → 5,640", ratio: "0.931383× (larger)", latency: "0.033230 → 0.031367 ms", linf: "0", note: "Bit-identical on the bound finite set and slightly faster in this run, but the candidate file is larger." },
  "w32-graph": { model: "WIDTH 32 / ORT GRAPH", accuracy: "96.215915% → 96.215915%", changes: "0 / 1,797", bytes: "10,055 → 10,442", ratio: "0.962938× (larger)", latency: "0.034000 → 0.029583 ms", linf: "0", note: "Bit-identical on the bound finite set and slightly faster in this run, but the candidate file is larger." },
  "w64-graph": { model: "WIDTH 64 / ORT GRAPH", accuracy: "96.160267% → 96.160267%", changes: "0 / 1,797", bytes: "19,658 → 20,045", ratio: "0.980693× (larger)", latency: "0.032923 → 0.028638 ms", linf: "0", note: "Bit-identical on the bound finite set and slightly faster in this run, but the candidate file is larger." }
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
