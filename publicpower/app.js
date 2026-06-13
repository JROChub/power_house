import * as THREE from "./vendor/three.module.min.js";
import { StreamBuffer } from "./stream-buffer.js";
import BookOpen from "./vendor/lucide/book-open.mjs";
import Building2 from "./vendor/lucide/building-2.mjs";
import ChevronLeft from "./vendor/lucide/chevron-left.mjs";
import ChevronRight from "./vendor/lucide/chevron-right.mjs";
import Code from "./vendor/lucide/code.mjs";
import Copy from "./vendor/lucide/copy.mjs";
import Crosshair from "./vendor/lucide/crosshair.mjs";
import Download from "./vendor/lucide/download.mjs";
import Globe from "./vendor/lucide/globe.mjs";
import Pause from "./vendor/lucide/pause.mjs";
import Package from "./vendor/lucide/package.mjs";
import Play from "./vendor/lucide/play.mjs";
import RotateCcw from "./vendor/lucide/rotate-ccw.mjs";
import Search from "./vendor/lucide/search.mjs";
import Share2 from "./vendor/lucide/share-2.mjs";
import Upload from "./vendor/lucide/upload.mjs";
import Volume2 from "./vendor/lucide/volume-2.mjs";
import VolumeX from "./vendor/lucide/volume-x.mjs";
import X from "./vendor/lucide/x.mjs";

const FIELD = 1_000_000_007n;
const CONSTANT = 173n;
const EARTH_RADIUS = 1.36;
const DEG = Math.PI / 180;
const iconLibrary = {
  "book-open": BookOpen,
  "building-2": Building2,
  "chevron-left": ChevronLeft,
  "chevron-right": ChevronRight,
  code: Code,
  copy: Copy,
  crosshair: Crosshair,
  download: Download,
  globe: Globe,
  pause: Pause,
  package: Package,
  play: Play,
  "rotate-ccw": RotateCcw,
  search: Search,
  "share-2": Share2,
  upload: Upload,
  "volume-2": Volume2,
  "volume-x": VolumeX,
  x: X,
};

const modes = {
  rootprint: {
    exponent: 4,
    domainLabel: "DAG<sup>4</sup>",
    domainCaption: "PUBLIC STRUCTURE",
    dossierDomain: "4-BRANCH DAG",
    domain: "4 VERIFIED BRANCHES",
    verifierPath: "CORE-ONLY DAG REPLAY",
    allocation: "EPA STRICTLY OPTIONAL",
    dossierArtifact: "ROOTPRINT v1 JSON",
    kicker: "DETERMINISTIC PROVENANCE GRAPH",
    description:
      "Navigate, fork, merge, and verify proof history while optional external attachments remain outside core identity.",
    title: "Verify the public provenance graph",
    detail:
      "The browser recalculates every PHA fingerprint and deterministic branch identifier.",
    button: "VERIFY GRAPH",
    status: "ROOTPRINT VERIFIER READY",
    downloadHref: "artifacts/rootprint-valid.json",
    downloadName: "rootprint-valid.json",
    color: 0x45ddd2,
    unit: "BRANCHES",
    action: verifyRootprintRelease,
  },
  constant: {
    exponent: 70,
    domain: "1.18 SEXTILLION POINTS",
    verifierPath: "70 FIELD EQUATIONS",
    allocation: "NEVER ALLOCATED",
    dossierArtifact: "BROWSER TRANSCRIPT",
    kicker: "CLOSED-FORM SUM-CHECK",
    description:
      "Seventy verifier rounds close a domain larger than one sextillion Boolean points without enumerating it.",
    title: "Run the 70-round browser proof",
    detail:
      "The browser checks every round equation over the field and computes a certificate SHA-256 digest.",
    button: "RUN PROOF",
    status: "LOCAL VERIFIER READY",
    downloadHref:
      "https://github.com/JROChub/power_house/blob/main/examples/sextillion_verify.rs",
    color: 0xb9ff3d,
    action: runConstantProof,
  },
  affine: {
    exponent: 4096,
    domain: "1,234-DIGIT DOMAIN",
    verifierPath: "4,096 FIELD ROUNDS",
    allocation: "NEVER ALLOCATED",
    dossierArtifact: "SEEDED AFFINE TRACE",
    kicker: "SEEDED NON-CONSTANT MODEL",
    description:
      "A public seed defines 4,096 affine coefficients. The canonical Rust verifier replays one round per variable.",
    title: "Run a 4,096-round structural replay",
    detail:
      "This browser replay checks the affine recurrence; the release implementation uses BLAKE2b Fiat-Shamir challenges in Rust.",
    button: "RUN REPLAY",
    status: "BROWSER MODEL READY",
    downloadHref:
      "https://github.com/JROChub/power_house/blob/main/examples/hyperscale_affine.rs",
    color: 0x45ddd2,
    action: runAffineReplay,
  },
  sparse: {
    exponent: 1_000_000,
    domain: "301,030-DIGIT DOMAIN",
    verifierPath: "1,000,000 ROUNDS",
    allocation: "SPARSE INCIDENCES ONLY",
    dossierArtifact: "PHSPv1 / 16,000,171 B",
    kicker: "MILLION-ROUND CERTIFICATE",
    description:
      "A stable 16 MB PHSPv1 certificate covers a seeded sparse polynomial over one million Boolean variables.",
    title: "Verify the published PHSPv1 artifact",
    detail:
      "Downloads the immutable release asset and checks its full SHA-256 digest in this browser.",
    button: "VERIFY HASH",
    status: "RELEASE ARTIFACT READY",
    downloadHref: "artifacts/power_house_sparse_record.phsp",
    downloadName: "power_house_sparse_record.phsp",
    color: 0xffc14d,
    action: () => verifyReleaseArtifacts("sparse"),
  },
  committed: {
    exponent: 1_000_000,
    domain: "301,030-DIGIT DOMAIN",
    verifierPath: "1,000,000 ROUNDS",
    allocation: "EXTERNAL WORKLOAD",
    dossierArtifact: "PHSMv1 + PHCPv1",
    kicker: "EXTERNAL WORKLOAD BINDING",
    description:
      "The PHCPv1 proof binds a separate PHSMv1 sparse workload through a domain-separated BLAKE2b-256 commitment.",
    title: "Verify both committed release artifacts",
    detail:
      "Downloads the external workload and million-round certificate, then checks both SHA-256 digests.",
    button: "VERIFY BOTH",
    status: "TWO-FILE BINDING READY",
    downloadHref: "artifacts/external_interaction_model.phcp",
    downloadName: "external_interaction_model.phcp",
    color: 0xff7167,
    action: () => verifyReleaseArtifacts("committed"),
  },
};

const knownArtifacts = {
  rootprint: {
    size: 4_232,
    hash: "eeb33450c6473c082675b8fcdaf70abfb0e6070fe739eeda5c839070d13750a3",
    label: "ROOTPRINT v1",
  },
  phsp: {
    size: 16_000_171,
    hash: "2b219ba189c3a38f1073c7797629e9aaf44a36820abb64c7628129480eb43f3b",
    label: "PHSPv1",
  },
  phsm: {
    size: 591_464,
    hash: "c8376831f47a50a7423be6412776382bc23618b037e9fdd163594d389d68864d",
    label: "PHSMv1",
  },
  phcp: {
    size: 16_000_128,
    hash: "82045e6eb851991e08d9c4cd782abff3bb06cb8ec5f149e7c2d4287113e6a54a",
    label: "PHCPv1",
  },
};

const cities = [
  { name: "SAN FRANCISCO", code: "SFO", zone: "America/Los_Angeles", lat: 37.77, lon: -122.42 },
  { name: "VANCOUVER", code: "YVR", zone: "America/Vancouver", lat: 49.28, lon: -123.12 },
  { name: "MEXICO CITY", code: "MEX", zone: "America/Mexico_City", lat: 19.43, lon: -99.13 },
  { name: "NEW YORK", code: "NYC", zone: "America/New_York", lat: 40.71, lon: -74.0 },
  { name: "SAO PAULO", code: "SAO", zone: "America/Sao_Paulo", lat: -23.55, lon: -46.63 },
  { name: "GREENWICH", code: "UTC", zone: "Europe/London", lat: 51.48, lon: 0.0 },
  { name: "PARIS", code: "CDG", zone: "Europe/Paris", lat: 48.86, lon: 2.35 },
  { name: "AMSTERDAM", code: "AMS", zone: "Europe/Amsterdam", lat: 52.37, lon: 4.9 },
  { name: "LAGOS", code: "LOS", zone: "Africa/Lagos", lat: 6.52, lon: 3.38 },
  { name: "CAIRO", code: "CAI", zone: "Africa/Cairo", lat: 30.04, lon: 31.24 },
  { name: "NAIROBI", code: "NBO", zone: "Africa/Nairobi", lat: -1.29, lon: 36.82 },
  { name: "DUBAI", code: "DXB", zone: "Asia/Dubai", lat: 25.2, lon: 55.27 },
  { name: "DELHI", code: "DEL", zone: "Asia/Kolkata", lat: 28.61, lon: 77.21 },
  { name: "SINGAPORE", code: "SIN", zone: "Asia/Singapore", lat: 1.35, lon: 103.82 },
  { name: "BEIJING", code: "PEK", zone: "Asia/Shanghai", lat: 39.9, lon: 116.4 },
  { name: "TOKYO", code: "TYO", zone: "Asia/Tokyo", lat: 35.68, lon: 139.69 },
  { name: "SYDNEY", code: "SYD", zone: "Australia/Sydney", lat: -33.87, lon: 151.21 },
  { name: "AUCKLAND", code: "AKL", zone: "Pacific/Auckland", lat: -36.85, lon: 174.76 },
  { name: "HONOLULU", code: "HNL", zone: "Pacific/Honolulu", lat: 21.31, lon: -157.86 },
];

const el = Object.fromEntries(
  [
    "orbital-canvas",
    "boot-screen",
    "boot-progress",
    "mission-state",
    "network-indicator",
    "network-state",
    "network-block",
    "network-validators",
    "network-peers",
    "network-console-state",
    "node-sfo-state",
    "node-nyc-state",
    "node-ams-state",
    "utc-date",
    "utc-time",
    "city-list",
    "city-search",
    "stage-city",
    "stage-time",
    "stage-date",
    "stage-zone",
    "solar-state",
    "solar-altitude",
    "sunrise-value",
    "sunset-value",
    "moon-phase",
    "moon-light",
    "solar-position",
    "solar-arc",
    "observatory-mode",
    "time-offset-label",
    "time-slider",
    "time-back",
    "time-live",
    "time-forward",
    "observatory-toggle",
    "observatory-close",
    "evaluation-toggle",
    "evaluation-close",
    "domain-label",
    "domain-caption",
    "domain-detail",
    "verifier-path",
    "allocation-value",
    "orbit-kicker",
    "orbit-description",
    "event-phase",
    "event-value",
    "dossier-mode",
    "dossier-domain",
    "dossier-work",
    "dossier-artifact",
    "verification-status",
    "verification-title",
    "verification-detail",
    "verify-button",
    "artifact-button",
    "artifact-input",
    "download-button",
    "share-button",
    "proof-trace",
    "progress-bar",
    "round-value",
    "claim-value",
    "digest-value",
    "mode-value",
    "status-seal",
    "seal-value",
    "seal-unit",
    "toast",
    "sound-toggle",
    "motion-toggle",
    "focus-toggle",
    "network-toggle",
    "zoom-in",
    "zoom-out",
    "view-reset",
    "install-command",
    "globe-tooltip",
    "monument-index",
    "network-console",
  ].map((id) => [camelCase(id), document.querySelector(`#${id}`)]),
);
el.canvas = el.orbitalCanvas;

const state = {
  mode: "rootprint",
  activeCity: 5,
  running: false,
  motion: !window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  sound: false,
  visible: !document.hidden,
  timeOffsetHours: 0,
  targetRotationX: cities[5].lat * DEG,
  targetRotationY: -Math.PI / 2 - cities[5].lon * DEG,
  zoom: window.innerWidth < 760 ? 4.85 : 4.55,
  toastTimer: 0,
  proofProgress: 0,
  lastResult: null,
  pointerDown: null,
  userInteracted: false,
};

const formatterCache = new Map();
const cityRows = [];
const cityMarkers = [];
const orbitEntries = [];
const interactiveObjects = [];
const raycaster = new THREE.Raycaster();
const pointer = new THREE.Vector2();
const hitWorldPosition = new THREE.Vector3();

let renderer;
let scene;
let camera;
let earthGroup;
let earthMaterial;
let atmosphereMaterial;
let orbitGroup;
let moon;
let moonOrbit;
let subsolarMarker;
let proofShell;
let proofShellMaterial;
let proofRingGroup;
let proofParticles;
let proofParticlesMaterial;
let selectedCityHalo;
let selectedCityBeam;
let networkGroup;
let animationFrame;
let audioContext;
let latestSolar = null;
let autoProofTimer = 0;
const networkLinks = [];
const networkCityIndexes = [0, 3, 7];

function camelCase(value) {
  return value.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
}

function mountIcon(target, iconName) {
  if (!target || !iconLibrary[iconName]) return;
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  for (const [tag, attributes] of iconLibrary[iconName]) {
    const node = document.createElementNS("http://www.w3.org/2000/svg", tag);
    for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, value);
    svg.append(node);
  }
  target.replaceChildren(svg);
  target.dataset.icon = iconName;
}

function mountIcons() {
  document.querySelectorAll("[data-icon]").forEach((target) => {
    mountIcon(target, target.dataset.icon);
  });
}

function setBootProgress(percent) {
  el.bootProgress.style.width = `${Math.max(8, Math.min(100, percent))}%`;
}

function finishBoot() {
  setBootProgress(100);
  window.setTimeout(() => el.bootScreen.classList.add("hidden"), 220);
}

function getFormatter(zone, kind) {
  const key = `${zone}:${kind}`;
  if (formatterCache.has(key)) return formatterCache.get(key);
  const options =
    kind === "clock"
      ? { timeZone: zone, hour: "2-digit", minute: "2-digit", hourCycle: "h23" }
      : kind === "clockSeconds"
        ? {
            timeZone: zone,
            hour: "2-digit",
            minute: "2-digit",
            second: "2-digit",
            hourCycle: "h23",
          }
        : kind === "stageDate"
          ? { timeZone: zone, month: "short", day: "2-digit" }
          : { timeZone: zone, month: "short", day: "2-digit", year: "numeric" };
  const formatter = new Intl.DateTimeFormat("en-US", options);
  formatterCache.set(key, formatter);
  return formatter;
}

function simulationDate() {
  return new Date(Date.now() + state.timeOffsetHours * 3_600_000);
}

function formatClock(date, zone, seconds = false) {
  return getFormatter(zone, seconds ? "clockSeconds" : "clock").format(date);
}

function normalizeDegrees(value) {
  return ((value + 180) % 360 + 360) % 360 - 180;
}

function julianDay(date) {
  return date.getTime() / 86_400_000 + 2_440_587.5;
}

function solarCoordinates(date) {
  const centuries = (julianDay(date) - 2_451_545) / 36_525;
  const meanLongitude =
    ((280.46646 + centuries * (36_000.76983 + centuries * 0.0003032)) % 360 + 360) % 360;
  const meanAnomaly = 357.52911 + centuries * (35_999.05029 - 0.0001537 * centuries);
  const eccentricity = 0.016708634 - centuries * (0.000042037 + 0.0000001267 * centuries);
  const equationOfCenter =
    Math.sin(meanAnomaly * DEG) * (1.914602 - centuries * (0.004817 + 0.000014 * centuries)) +
    Math.sin(2 * meanAnomaly * DEG) * (0.019993 - 0.000101 * centuries) +
    Math.sin(3 * meanAnomaly * DEG) * 0.000289;
  const trueLongitude = meanLongitude + equationOfCenter;
  const omega = 125.04 - 1934.136 * centuries;
  const apparentLongitude = trueLongitude - 0.00569 - 0.00478 * Math.sin(omega * DEG);
  const meanObliquity =
    23 +
    (26 +
      (21.448 -
        centuries * (46.815 + centuries * (0.00059 - centuries * 0.001813))) /
        60) /
      60;
  const obliquity = meanObliquity + 0.00256 * Math.cos(omega * DEG);
  const declination =
    Math.asin(Math.sin(obliquity * DEG) * Math.sin(apparentLongitude * DEG)) / DEG;
  const y = Math.tan((obliquity * DEG) / 2) ** 2;
  const equationOfTime =
    (4 / DEG) *
    (y * Math.sin(2 * meanLongitude * DEG) -
      2 * eccentricity * Math.sin(meanAnomaly * DEG) +
      4 *
        eccentricity *
        y *
        Math.sin(meanAnomaly * DEG) *
        Math.cos(2 * meanLongitude * DEG) -
      0.5 * y * y * Math.sin(4 * meanLongitude * DEG) -
      1.25 * eccentricity * eccentricity * Math.sin(2 * meanAnomaly * DEG));
  const utcMinutes =
    date.getUTCHours() * 60 +
    date.getUTCMinutes() +
    date.getUTCSeconds() / 60 +
    date.getUTCMilliseconds() / 60_000;
  const longitude = normalizeDegrees(180 - (utcMinutes + equationOfTime) / 4);
  return { lat: declination, lon: longitude, declination, equationOfTime };
}

function solarAltitude(city, date, solar) {
  const utcMinutes =
    date.getUTCHours() * 60 +
    date.getUTCMinutes() +
    date.getUTCSeconds() / 60 +
    date.getUTCMilliseconds() / 60_000;
  const trueSolarMinutes =
    ((utcMinutes + solar.equationOfTime + 4 * city.lon) % 1440 + 1440) % 1440;
  const hourAngle = (trueSolarMinutes / 4 - 180) * DEG;
  const latitude = city.lat * DEG;
  const declination = solar.declination * DEG;
  const sine =
    Math.sin(latitude) * Math.sin(declination) +
    Math.cos(latitude) * Math.cos(declination) * Math.cos(hourAngle);
  return Math.asin(THREE.MathUtils.clamp(sine, -1, 1)) / DEG;
}

function solarEvents(city, date, solar) {
  const latitude = city.lat * DEG;
  const declination = solar.declination * DEG;
  const cosineHourAngle =
    Math.cos(90.833 * DEG) / (Math.cos(latitude) * Math.cos(declination)) -
    Math.tan(latitude) * Math.tan(declination);
  if (cosineHourAngle > 1) return { sunrise: "POLAR NIGHT", sunset: "POLAR NIGHT" };
  if (cosineHourAngle < -1) return { sunrise: "MIDNIGHT SUN", sunset: "MIDNIGHT SUN" };
  const hourAngle = Math.acos(cosineHourAngle) / DEG;
  const solarNoon = 720 - 4 * city.lon - solar.equationOfTime;
  return {
    sunrise: formatEventTime(date, solarNoon - hourAngle * 4, city.zone),
    sunset: formatEventTime(date, solarNoon + hourAngle * 4, city.zone),
  };
}

function formatEventTime(date, utcMinutes, zone) {
  const midnight = Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate());
  return formatClock(new Date(midnight + utcMinutes * 60_000), zone);
}

function moonData(date) {
  const synodicMonth = 29.53058867;
  const knownNewMoon = Date.UTC(2000, 0, 6, 18, 14);
  const age =
    ((((date.getTime() - knownNewMoon) / 86_400_000) % synodicMonth) + synodicMonth) %
      synodicMonth;
  const phase = age / synodicMonth;
  const illumination = (1 - Math.cos(phase * Math.PI * 2)) / 2;
  const names = [
    "NEW",
    "WAXING CRESCENT",
    "FIRST QUARTER",
    "WAXING GIBBOUS",
    "FULL",
    "WANING GIBBOUS",
    "LAST QUARTER",
    "WANING CRESCENT",
  ];
  return {
    age,
    phase,
    illumination,
    name: names[Math.round(phase * 8) % 8],
  };
}

function latLonVector(lat, lon, radius = 1) {
  const phi = (90 - lat) * DEG;
  const theta = (lon + 180) * DEG;
  return new THREE.Vector3(
    -radius * Math.sin(phi) * Math.cos(theta),
    radius * Math.cos(phi),
    radius * Math.sin(phi) * Math.sin(theta),
  );
}

function coordinateLabel(value, positive, negative) {
  return `${Math.abs(value).toFixed(1)} ${value >= 0 ? positive : negative}`;
}

function updateAstronomy() {
  const date = simulationDate();
  latestSolar = solarCoordinates(date);
  const moonState = moonData(date);
  const city = cities[state.activeCity];
  const altitude = solarAltitude(city, date, latestSolar);
  const events = solarEvents(city, date, latestSolar);

  el.utcDate.textContent = getFormatter("UTC", "date").format(date).toUpperCase();
  el.utcTime.textContent = formatClock(date, "UTC", true);
  el.missionState.textContent =
    state.timeOffsetHours === 0
      ? "LIVE ORBIT"
      : `TIME SHIFT ${state.timeOffsetHours > 0 ? "+" : ""}${state.timeOffsetHours}H`;
  el.observatoryMode.textContent = state.timeOffsetHours === 0 ? "LIVE" : "SIM";
  el.timeOffsetLabel.textContent =
    state.timeOffsetHours === 0
      ? "LIVE"
      : `${state.timeOffsetHours > 0 ? "+" : ""}${state.timeOffsetHours}H`;

  cityRows.forEach((row, index) => {
    const rowAltitude = solarAltitude(cities[index], date, latestSolar);
    row.clock.textContent = formatClock(date, cities[index].zone);
    row.solar.className = `city-solar-dot ${rowAltitude > -0.833 ? "day" : "night"}`;
  });

  el.stageTime.textContent = formatClock(date, city.zone, true);
  el.stageDate.textContent = getFormatter(city.zone, "stageDate").format(date).toUpperCase();
  el.solarState.textContent =
    altitude > 0 ? "DAYLIGHT" : altitude > -6 ? "CIVIL TWILIGHT" : "NIGHT";
  el.solarState.classList.toggle("night", altitude <= 0);
  el.solarAltitude.textContent = `${altitude >= 0 ? "+" : ""}${altitude.toFixed(1)}°`;
  el.solarArc.style.setProperty(
    "--solar-position",
    `${THREE.MathUtils.clamp((altitude + 90) / 180, 0, 1) * 100}%`,
  );
  el.sunriseValue.textContent = events.sunrise;
  el.sunsetValue.textContent = events.sunset;
  el.moonPhase.textContent = moonState.name;
  el.moonLight.textContent = `${Math.round(moonState.illumination * 100)}%`;
  el.solarPosition.textContent = `${coordinateLabel(latestSolar.lat, "N", "S")} / ${coordinateLabel(
    latestSolar.lon,
    "E",
    "W",
  )}`;

  if (earthMaterial) {
    earthMaterial.uniforms.sunDirection.value.copy(
      latLonVector(latestSolar.lat, latestSolar.lon).normalize(),
    );
  }
  if (subsolarMarker) {
    const position = latLonVector(latestSolar.lat, latestSolar.lon, EARTH_RADIUS + 0.025);
    subsolarMarker.position.copy(position);
    subsolarMarker.lookAt(position.clone().multiplyScalar(2));
  }
  if (moonOrbit && moon) {
    moonOrbit.rotation.z = 5.14 * DEG;
    const angle = moonState.phase * Math.PI * 2 + Math.PI;
    moon.position.set(Math.cos(angle) * 2.65, 0.16 * Math.sin(angle * 2), Math.sin(angle) * 2.65);
    moon.rotation.y = -angle;
  }
}

function buildCityList() {
  const fragment = document.createDocumentFragment();
  cities.forEach((city, index) => {
    const button = document.createElement("button");
    const solar = document.createElement("i");
    const name = document.createElement("b");
    const clock = document.createElement("strong");
    const metadata = document.createElement("small");
    button.className = `city-row${index === state.activeCity ? " active" : ""}`;
    button.type = "button";
    button.dataset.index = String(index);
    solar.className = "city-solar-dot night";
    name.textContent = city.name;
    clock.textContent = "00:00";
    metadata.textContent = `${city.code} / ${city.zone}`;
    button.append(solar, name, clock, metadata);
    button.addEventListener("click", () => selectCity(index));
    cityRows.push({ button, solar, clock });
    fragment.append(button);
  });
  el.cityList.append(fragment);
}

function filterCities(query) {
  const normalized = query.trim().toLowerCase();
  let visible = 0;
  cityRows.forEach(({ button }, index) => {
    const city = cities[index];
    const matches =
      !normalized ||
      city.name.toLowerCase().includes(normalized) ||
      city.code.toLowerCase().includes(normalized) ||
      city.zone.toLowerCase().includes(normalized);
    button.hidden = !matches;
    if (matches) visible += 1;
  });
  let empty = el.cityList.querySelector(".city-empty");
  if (!visible && !empty) {
    empty = document.createElement("div");
    empty.className = "city-empty";
    empty.textContent = "NO OBSERVATORY MATCH";
    el.cityList.append(empty);
  } else if (visible && empty) {
    empty.remove();
  }
}

function selectCity(index, focus = true) {
  state.activeCity = index;
  const city = cities[index];
  cityRows.forEach(({ button }, rowIndex) => {
    button.classList.toggle("active", rowIndex === index);
    button.setAttribute("aria-pressed", rowIndex === index ? "true" : "false");
  });
  cityMarkers.forEach((marker, markerIndex) => {
    marker.material.color.setHex(markerIndex === index ? 0xb9ff3d : 0x45ddd2);
    marker.scale.setScalar(markerIndex === index ? 1.7 : 1);
  });
  el.stageCity.textContent = city.name;
  el.stageZone.textContent = city.zone.toUpperCase();
  updateSelectedCityGeometry();
  if (focus) focusSelectedCity();
  updateAstronomy();
  if (window.innerWidth <= 760) document.body.classList.remove("observatory-open");
  tone(520, 0.04);
}

function focusSelectedCity() {
  const city = cities[state.activeCity];
  state.targetRotationX = THREE.MathUtils.clamp(city.lat * DEG, -1.18, 1.18);
  state.targetRotationY = -Math.PI / 2 - city.lon * DEG;
  state.zoom = window.innerWidth < 760 ? 4.45 : 4.2;
}

function seededRandom() {
  let seed = 0x504f5745;
  return () => {
    seed ^= seed << 13;
    seed ^= seed >>> 17;
    seed ^= seed << 5;
    return (seed >>> 0) / 4_294_967_296;
  };
}

function createStars() {
  const random = seededRandom();
  const count = window.innerWidth < 760 ? 850 : 1800;
  const positions = new Float32Array(count * 3);
  const colors = new Float32Array(count * 3);
  for (let index = 0; index < count; index += 1) {
    const radius = 8 + random() * 18;
    const theta = random() * Math.PI * 2;
    const phi = Math.acos(2 * random() - 1);
    positions[index * 3] = radius * Math.sin(phi) * Math.cos(theta);
    positions[index * 3 + 1] = radius * Math.cos(phi);
    positions[index * 3 + 2] = radius * Math.sin(phi) * Math.sin(theta);
    const brightness = 0.42 + random() * 0.58;
    colors[index * 3] = brightness * 0.76;
    colors[index * 3 + 1] = brightness * 0.92;
    colors[index * 3 + 2] = brightness * 0.86;
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));
  const material = new THREE.PointsMaterial({
    size: 0.018,
    vertexColors: true,
    transparent: true,
    opacity: 0.72,
    sizeAttenuation: true,
  });
  scene.add(new THREE.Points(geometry, material));
}

function loadTexture(url, progress) {
  return new Promise((resolve, reject) => {
    new THREE.TextureLoader().load(
      url,
      (texture) => {
        texture.colorSpace = THREE.SRGBColorSpace;
        texture.anisotropy = Math.min(renderer.capabilities.getMaxAnisotropy(), 8);
        setBootProgress(progress);
        resolve(texture);
      },
      undefined,
      reject,
    );
  });
}

function createEarthGrid() {
  const group = new THREE.Group();
  const material = new THREE.LineBasicMaterial({
    color: 0x8bd7cd,
    transparent: true,
    opacity: 0.085,
    depthWrite: false,
  });
  for (let lat = -60; lat <= 60; lat += 30) {
    const points = [];
    for (let lon = -180; lon <= 180; lon += 3) {
      points.push(latLonVector(lat, lon, EARTH_RADIUS + 0.006));
    }
    group.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(points), material));
  }
  for (let lon = -150; lon <= 180; lon += 30) {
    const points = [];
    for (let lat = -90; lat <= 90; lat += 3) {
      points.push(latLonVector(lat, lon, EARTH_RADIUS + 0.006));
    }
    group.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(points), material));
  }
  return group;
}

function elevatedArc(start, end, height = 0.28) {
  const points = [];
  for (let index = 0; index <= 72; index += 1) {
    const progress = index / 72;
    const point = start
      .clone()
      .lerp(end, progress)
      .normalize()
      .multiplyScalar(EARTH_RADIUS + 0.035 + Math.sin(progress * Math.PI) * height);
    points.push(point);
  }
  return new THREE.CatmullRomCurve3(points);
}

function createNetworkTopology() {
  networkGroup = new THREE.Group();
  const pairs = [
    [0, 3],
    [3, 7],
    [7, 0],
  ];
  pairs.forEach(([fromIndex, toIndex], index) => {
    const curve = elevatedArc(
      latLonVector(cities[fromIndex].lat, cities[fromIndex].lon),
      latLonVector(cities[toIndex].lat, cities[toIndex].lon),
      0.22 + index * 0.045,
    );
    const material = new THREE.LineBasicMaterial({
      color: index === 1 ? 0xffc15a : 0x45ddd2,
      transparent: true,
      opacity: 0.56,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });
    const line = new THREE.Line(
      new THREE.BufferGeometry().setFromPoints(curve.getPoints(96)),
      material,
    );
    const pulse = new THREE.Mesh(
      new THREE.SphereGeometry(0.025, 12, 8),
      new THREE.MeshBasicMaterial({
        color: index === 1 ? 0xffc15a : 0xb9ff3d,
        transparent: true,
        opacity: 0.95,
      }),
    );
    networkGroup.add(line, pulse);
    networkLinks.push({ curve, line, pulse, phase: index / pairs.length });
  });

  networkCityIndexes.forEach((cityIndex) => {
    const city = cities[cityIndex];
    const position = latLonVector(city.lat, city.lon, EARTH_RADIUS + 0.045);
    const ring = new THREE.Mesh(
      new THREE.RingGeometry(0.04, 0.057, 28),
      new THREE.MeshBasicMaterial({
        color: 0xb9ff3d,
        transparent: true,
        opacity: 0.86,
        side: THREE.DoubleSide,
        depthWrite: false,
      }),
    );
    ring.position.copy(position);
    ring.quaternion.setFromUnitVectors(
      new THREE.Vector3(0, 0, 1),
      position.clone().normalize(),
    );
    networkGroup.add(ring);
  });
  earthGroup.add(networkGroup);
}

function createSelectedCityGeometry() {
  selectedCityHalo = new THREE.Mesh(
    new THREE.RingGeometry(0.052, 0.083, 36),
    new THREE.MeshBasicMaterial({
      color: 0xb9ff3d,
      transparent: true,
      opacity: 0.82,
      side: THREE.DoubleSide,
      depthWrite: false,
    }),
  );
  selectedCityBeam = new THREE.Line(
    new THREE.BufferGeometry(),
    new THREE.LineBasicMaterial({
      color: 0xb9ff3d,
      transparent: true,
      opacity: 0.42,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    }),
  );
  earthGroup.add(selectedCityHalo, selectedCityBeam);
  updateSelectedCityGeometry();
}

function updateSelectedCityGeometry() {
  if (!selectedCityHalo || !selectedCityBeam) return;
  const city = cities[state.activeCity];
  const surface = latLonVector(city.lat, city.lon, EARTH_RADIUS + 0.045);
  selectedCityHalo.position.copy(surface);
  selectedCityHalo.quaternion.setFromUnitVectors(
    new THREE.Vector3(0, 0, 1),
    surface.clone().normalize(),
  );
  selectedCityBeam.geometry.dispose();
  selectedCityBeam.geometry = new THREE.BufferGeometry().setFromPoints([
    surface,
    surface.clone().normalize().multiplyScalar(EARTH_RADIUS + 0.46),
  ]);
}

async function createEarth() {
  const mobile = window.innerWidth <= 760;
  const [dayTexture, nightTexture] = await Promise.all([
    loadTexture(`assets/earth-day${mobile ? "-mobile" : ""}.jpg`, 48),
    loadTexture(`assets/earth-night${mobile ? "-mobile" : ""}.jpg`, 70),
  ]);

  earthMaterial = new THREE.ShaderMaterial({
    uniforms: {
      dayMap: { value: dayTexture },
      nightMap: { value: nightTexture },
      sunDirection: { value: new THREE.Vector3(1, 0, 0) },
    },
    vertexShader: `
      varying vec2 vUv;
      varying vec3 vObjectNormal;
      varying vec3 vViewNormal;
      varying vec3 vViewPosition;
      void main() {
        vUv = uv;
        vObjectNormal = normalize(normal);
        vViewNormal = normalize(normalMatrix * normal);
        vec4 viewPosition = modelViewMatrix * vec4(position, 1.0);
        vViewPosition = viewPosition.xyz;
        gl_Position = projectionMatrix * viewPosition;
      }
    `,
    fragmentShader: `
      uniform sampler2D dayMap;
      uniform sampler2D nightMap;
      uniform vec3 sunDirection;
      varying vec2 vUv;
      varying vec3 vObjectNormal;
      varying vec3 vViewNormal;
      varying vec3 vViewPosition;
      void main() {
        float solar = dot(normalize(vObjectNormal), normalize(sunDirection));
        float daylight = smoothstep(-0.22, 0.16, solar);
        vec3 day = texture2D(dayMap, vUv).rgb;
        vec3 night = texture2D(nightMap, vUv).rgb * 2.05;
        vec3 litDay = day * (0.68 + max(solar, 0.0) * 0.72);
        vec3 viewDirection = normalize(-vViewPosition);
        float limb = pow(1.0 - max(dot(normalize(vViewNormal), viewDirection), 0.0), 3.0);
        float terminator = 1.0 - smoothstep(0.0, 0.18, abs(solar));
        vec3 color = mix(night, litDay, daylight);
        color += day * 0.08;
        color += vec3(0.05, 0.34, 0.3) * limb * 0.78;
        color += vec3(0.11, 0.42, 0.36) * terminator * 0.11;
        gl_FragColor = vec4(color, 1.0);
      }
    `,
  });

  earthGroup = new THREE.Group();
  const globeSegments = mobile ? [64, 40] : [112, 72];
  const globe = new THREE.Mesh(
    new THREE.SphereGeometry(EARTH_RADIUS, globeSegments[0], globeSegments[1]),
    earthMaterial,
  );
  earthGroup.add(globe, createEarthGrid());

  atmosphereMaterial = new THREE.ShaderMaterial({
    transparent: true,
    side: THREE.BackSide,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
    vertexShader: `
      varying vec3 vNormal;
      void main() {
        vNormal = normalize(normalMatrix * normal);
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: `
      varying vec3 vNormal;
      void main() {
        float intensity = pow(max(0.72 - dot(vNormal, vec3(0.0, 0.0, 1.0)), 0.0), 3.0);
        gl_FragColor = vec4(0.12, 0.82, 0.73, 1.0) * intensity;
      }
    `,
  });
  earthGroup.add(
    new THREE.Mesh(
      new THREE.SphereGeometry(EARTH_RADIUS + 0.11, mobile ? 56 : 80, mobile ? 36 : 52),
      atmosphereMaterial,
    ),
  );

  const markerGeometry = new THREE.SphereGeometry(0.021, 12, 8);
  const markerHitGeometry = new THREE.SphereGeometry(0.055, 10, 8);
  cities.forEach((city, index) => {
    const marker = new THREE.Mesh(
      markerGeometry,
      new THREE.MeshBasicMaterial({ color: index === state.activeCity ? 0xb9ff3d : 0x45ddd2 }),
    );
    marker.position.copy(latLonVector(city.lat, city.lon, EARTH_RADIUS + 0.03));
    marker.scale.setScalar(index === state.activeCity ? 1.7 : 1);
    marker.userData = { type: "city", cityIndex: index };
    const hitMarker = new THREE.Mesh(
      markerHitGeometry,
      new THREE.MeshBasicMaterial({
        transparent: true,
        opacity: 0,
        depthWrite: false,
      }),
    );
    hitMarker.position.copy(marker.position);
    hitMarker.userData = { type: "city", cityIndex: index };
    cityMarkers.push(marker);
    interactiveObjects.push(hitMarker);
    earthGroup.add(marker, hitMarker);
  });

  createNetworkTopology();
  createSelectedCityGeometry();

  const solarGroup = new THREE.Group();
  const solarRing = new THREE.Mesh(
    new THREE.RingGeometry(0.043, 0.059, 24),
    new THREE.MeshBasicMaterial({
      color: 0xffc14d,
      transparent: true,
      opacity: 0.92,
      side: THREE.DoubleSide,
      depthWrite: false,
    }),
  );
  const solarCore = new THREE.Mesh(
    new THREE.SphereGeometry(0.012, 10, 8),
    new THREE.MeshBasicMaterial({ color: 0xfff0b8 }),
  );
  solarGroup.add(solarRing, solarCore);
  subsolarMarker = solarGroup;
  earthGroup.add(solarGroup);
  scene.add(earthGroup);
  updateAstronomy();
}

function createOrbitTrack(name, index) {
  const radius = [1.78, 1.98, 2.17, 2.36][index];
  const tiltX = [0.4, 1.04, -0.68, 0.78][index];
  const tiltZ = [0.16, -0.3, 0.52, -0.62][index];
  const points = [];
  for (let point = 0; point <= 256; point += 1) {
    const angle = (point / 256) * Math.PI * 2;
    points.push(new THREE.Vector3(Math.cos(angle) * radius, 0, Math.sin(angle) * radius));
  }
  const material = new THREE.LineBasicMaterial({
    color: modes[name].color,
    transparent: true,
    opacity: index === 0 ? 0.66 : 0.16,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
  });
  const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints(points), material);
  line.rotation.set(tiltX, 0, tiltZ);
  const beacon = new THREE.Mesh(
    new THREE.SphereGeometry(0.035, 14, 10),
    new THREE.MeshBasicMaterial({ color: modes[name].color, transparent: true, opacity: 0.95 }),
  );
  beacon.userData = {
    type: "proof",
    mode: name,
    radius,
    tiltX,
    tiltZ,
    phase: index * 1.63,
    speed: 0.055 + index * 0.014,
  };
  const hitBeacon = new THREE.Mesh(
    new THREE.SphereGeometry(0.085, 10, 8),
    new THREE.MeshBasicMaterial({ transparent: true, opacity: 0, depthWrite: false }),
  );
  hitBeacon.userData = { type: "proof", mode: name };
  beacon.add(hitBeacon);
  orbitGroup.add(line, beacon);
  orbitEntries.push({ name, line, beacon });
  interactiveObjects.push(hitBeacon);
}

function createOrbits() {
  orbitGroup = new THREE.Group();
  Object.keys(modes).forEach(createOrbitTrack);
  scene.add(orbitGroup);
}

function createProofField() {
  proofShellMaterial = new THREE.MeshBasicMaterial({
    color: modes[state.mode].color,
    transparent: true,
    opacity: 0.04,
    wireframe: true,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
  });
  proofShell = new THREE.Mesh(
    new THREE.SphereGeometry(EARTH_RADIUS + 0.2, 32, 20),
    proofShellMaterial,
  );

  proofRingGroup = new THREE.Group();
  [1.62, 1.78, 1.96].forEach((radius, index) => {
    const ring = new THREE.Mesh(
      new THREE.TorusGeometry(radius, 0.004 + index * 0.001, 5, 180),
      new THREE.MeshBasicMaterial({
        color: modes[state.mode].color,
        transparent: true,
        opacity: 0.08,
        blending: THREE.AdditiveBlending,
        depthWrite: false,
      }),
    );
    ring.rotation.set(0.38 + index * 0.48, index * 0.36, -0.24 + index * 0.42);
    proofRingGroup.add(ring);
  });

  const pointCount = window.innerWidth <= 760 ? 900 : 2400;
  const positions = new Float32Array(pointCount * 3);
  const goldenAngle = Math.PI * (3 - Math.sqrt(5));
  for (let index = 0; index < pointCount; index += 1) {
    const y = 1 - (index / (pointCount - 1)) * 2;
    const radial = Math.sqrt(1 - y * y);
    const angle = goldenAngle * index;
    const shell = 1.58 + ((index * 17) % 23) * 0.006;
    positions[index * 3] = Math.cos(angle) * radial * shell;
    positions[index * 3 + 1] = y * shell;
    positions[index * 3 + 2] = Math.sin(angle) * radial * shell;
  }
  const particleGeometry = new THREE.BufferGeometry();
  particleGeometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  proofParticlesMaterial = new THREE.PointsMaterial({
    color: modes[state.mode].color,
    size: window.innerWidth <= 760 ? 0.012 : 0.009,
    transparent: true,
    opacity: 0.2,
    blending: THREE.AdditiveBlending,
    depthWrite: false,
    sizeAttenuation: true,
  });
  proofParticles = new THREE.Points(particleGeometry, proofParticlesMaterial);
  scene.add(proofShell, proofRingGroup, proofParticles);
}

function createMoon() {
  moonOrbit = new THREE.Group();
  const material = new THREE.MeshStandardMaterial({
    color: 0xc8d0c8,
    roughness: 0.9,
    metalness: 0,
  });
  moon = new THREE.Mesh(new THREE.SphereGeometry(0.105, 28, 18), material);
  moonOrbit.add(moon);
  scene.add(moonOrbit);
  scene.add(new THREE.AmbientLight(0x203132, 0.7));
  const sunLight = new THREE.DirectionalLight(0xffefd0, 2.4);
  sunLight.position.set(5, 2, 4);
  scene.add(sunLight);
}

async function initScene() {
  try {
    renderer = new THREE.WebGLRenderer({
      canvas: el.canvas,
      antialias: true,
      alpha: true,
      powerPreference: "high-performance",
    });
  } catch {
    document.body.classList.add("webgl-fallback");
    finishBoot();
    showToast("WebGL is unavailable; proof controls remain active.");
    scheduleAutoProof();
    return;
  }
  renderer.setPixelRatio(
    Math.min(window.devicePixelRatio, window.innerWidth <= 760 ? 1.15 : 1.5),
  );
  renderer.setSize(window.innerWidth, window.innerHeight);
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.28;
  renderer.setClearColor(0x010405, 0.42);

  scene = new THREE.Scene();
  scene.fog = new THREE.FogExp2(0x020607, 0.022);
  camera = new THREE.PerspectiveCamera(42, window.innerWidth / window.innerHeight, 0.1, 100);
  camera.position.set(0, 0, state.zoom);

  createStars();
  createOrbits();
  createProofField();
  createMoon();
  try {
    await createEarth();
  } catch {
    document.body.classList.add("webgl-fallback");
    showToast("Earth textures could not load; verification remains available.");
  }
  bindGlobeInput();
  selectMode(state.mode, false);
  selectCity(state.activeCity, false);
  setBootProgress(90);
  animate();
  finishBoot();
  scheduleAutoProof();
}

function pointerPosition(event) {
  const bounds = el.canvas.getBoundingClientRect();
  pointer.x = ((event.clientX - bounds.left) / bounds.width) * 2 - 1;
  pointer.y = -((event.clientY - bounds.top) / bounds.height) * 2 + 1;
}

function raycast(event) {
  if (!camera || !scene) return null;
  pointerPosition(event);
  raycaster.setFromCamera(pointer, camera);
  return (
    raycaster.intersectObjects(interactiveObjects, false).find((hit) => {
      if (hit.object.userData.type !== "city") return true;
      hit.object.getWorldPosition(hitWorldPosition);
      return hitWorldPosition.z > 0;
    }) || null
  );
}

function showGlobeTooltip(event, object) {
  const data = object.userData;
  if (data.type === "city") {
    const city = cities[data.cityIndex];
    el.globeTooltip.innerHTML = `<b>${city.name}</b><span>${formatClock(
      simulationDate(),
      city.zone,
      true,
    )} / ${city.code}</span>`;
  } else {
    const mode = modes[data.mode];
    el.globeTooltip.innerHTML = `<b>${data.mode.toUpperCase()}</b><span>2^${mode.exponent.toLocaleString()} DOMAIN</span>`;
  }
  el.globeTooltip.style.left = `${Math.min(event.clientX + 14, window.innerWidth - 190)}px`;
  el.globeTooltip.style.top = `${Math.min(event.clientY + 14, window.innerHeight - 72)}px`;
  el.globeTooltip.classList.add("visible");
}

function hideGlobeTooltip() {
  el.globeTooltip.classList.remove("visible");
}

function activateSceneObject(object) {
  if (object.userData.type === "city") {
    selectCity(object.userData.cityIndex);
    showToast(`${cities[object.userData.cityIndex].name} observatory selected`);
  } else if (object.userData.type === "proof") {
    selectMode(object.userData.mode);
    showToast(`${object.userData.mode.toUpperCase()} proof orbit selected`);
  }
}

function bindGlobeInput() {
  el.canvas.addEventListener("pointerdown", (event) => {
    const hit = raycast(event);
    state.pointerDown = {
      id: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      lastX: event.clientX,
      lastY: event.clientY,
      moved: false,
      hitObject: hit?.object || null,
    };
    el.canvas.setPointerCapture(event.pointerId);
  });
  el.canvas.addEventListener("pointermove", (event) => {
    if (state.pointerDown) {
      const dx = event.clientX - state.pointerDown.lastX;
      const dy = event.clientY - state.pointerDown.lastY;
      state.pointerDown.lastX = event.clientX;
      state.pointerDown.lastY = event.clientY;
      if (
        Math.hypot(
          event.clientX - state.pointerDown.startX,
          event.clientY - state.pointerDown.startY,
        ) > 5
      ) {
        state.pointerDown.moved = true;
      }
      state.targetRotationY += dx * 0.006;
      state.targetRotationX = THREE.MathUtils.clamp(
        state.targetRotationX + dy * 0.004,
        -1.18,
        1.18,
      );
      hideGlobeTooltip();
      return;
    }
    const hit = raycast(event);
    el.canvas.style.cursor = hit ? "pointer" : "grab";
    if (hit) showGlobeTooltip(event, hit.object);
    else hideGlobeTooltip();
  });
  el.canvas.addEventListener("pointerup", (event) => {
    if (state.pointerDown && !state.pointerDown.moved) {
      const hitObject = state.pointerDown.hitObject || raycast(event)?.object;
      if (hitObject) activateSceneObject(hitObject);
    }
    state.pointerDown = null;
  });
  el.canvas.addEventListener("pointercancel", () => {
    state.pointerDown = null;
  });
  el.canvas.addEventListener("pointerleave", hideGlobeTooltip);
  el.canvas.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      state.zoom = THREE.MathUtils.clamp(state.zoom + event.deltaY * 0.002, 3.45, 6.2);
    },
    { passive: false },
  );
}

function animate(time = 0) {
  animationFrame = requestAnimationFrame(animate);
  if (!renderer || !scene || !camera || !state.visible) return;
  const seconds = time * 0.001;
  if (earthGroup) {
    if (state.motion && !state.pointerDown) state.targetRotationY += 0.00024;
    earthGroup.rotation.x += (state.targetRotationX - earthGroup.rotation.x) * 0.045;
    earthGroup.rotation.y += (state.targetRotationY - earthGroup.rotation.y) * 0.045;
  }
  if (orbitGroup) {
    orbitGroup.rotation.y = state.motion ? seconds * 0.022 : orbitGroup.rotation.y;
    orbitEntries.forEach(({ name, beacon }, index) => {
      const data = beacon.userData;
      const angle = data.phase + seconds * (state.motion ? data.speed : 0);
      const position = new THREE.Vector3(
        Math.cos(angle) * data.radius,
        0,
        Math.sin(angle) * data.radius,
      );
      position.applyEuler(new THREE.Euler(data.tiltX, 0, data.tiltZ));
      beacon.position.copy(position);
      const selected = name === state.mode;
      const pulse = selected ? 1.1 + Math.sin(seconds * 4) * 0.24 : 0.72;
      const progressBoost = selected ? state.proofProgress * 0.55 : 0;
      beacon.scale.setScalar(pulse + progressBoost);
    });
  }
  if (networkGroup) {
    networkLinks.forEach(({ curve, pulse, line, phase }, index) => {
      const progress = (phase + seconds * (0.055 + index * 0.008)) % 1;
      pulse.position.copy(curve.getPoint(progress));
      pulse.scale.setScalar(0.85 + Math.sin(seconds * 7 + index) * 0.24);
      line.material.opacity = 0.38 + Math.sin(seconds * 1.4 + index) * 0.13;
    });
  }
  if (selectedCityHalo) {
    selectedCityHalo.scale.setScalar(1 + Math.sin(seconds * 4.5) * 0.18);
    selectedCityHalo.material.opacity = 0.66 + Math.sin(seconds * 4.5) * 0.16;
  }
  if (subsolarMarker) {
    subsolarMarker.scale.setScalar(1 + Math.sin(seconds * 3) * 0.12);
  }
  if (proofShell && proofRingGroup) {
    const activity = state.running ? 0.12 + state.proofProgress * 0.34 : state.lastResult ? 0.13 : 0.035;
    proofShellMaterial.opacity = activity + (state.running ? Math.sin(seconds * 7) * 0.025 : 0);
    proofShellMaterial.color.setHex(modes[state.mode].color);
    proofShell.scale.setScalar(1 + state.proofProgress * 0.18);
    proofShell.rotation.y = seconds * 0.035;
    proofShell.rotation.x = seconds * -0.018;
    proofRingGroup.children.forEach((ring, index) => {
      ring.material.color.setHex(modes[state.mode].color);
      ring.material.opacity = activity * (0.8 - index * 0.12);
      ring.rotation.y += state.motion ? 0.0008 * (index + 1) : 0;
      ring.rotation.z += state.motion ? 0.00045 * (index % 2 ? -1 : 1) : 0;
    });
  }
  if (proofParticles) {
    proofParticles.rotation.y = seconds * -0.018;
    proofParticles.rotation.z = seconds * 0.006;
    proofParticlesMaterial.color.setHex(modes[state.mode].color);
    proofParticlesMaterial.opacity =
      0.12 + state.proofProgress * 0.52 + (state.running ? 0.08 : 0);
    proofParticles.scale.setScalar(1 + state.proofProgress * 0.12);
  }
  if (moon) moon.rotation.y += state.motion ? 0.0015 : 0;
  camera.position.z += (state.zoom - camera.position.z) * 0.06;
  renderer.render(scene, camera);
}

function resize() {
  if (!renderer || !camera) return;
  renderer.setPixelRatio(
    Math.min(window.devicePixelRatio, window.innerWidth <= 760 ? 1.15 : 1.5),
  );
  renderer.setSize(window.innerWidth, window.innerHeight);
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.fov = window.innerWidth <= 760 ? 50 : 42;
  camera.updateProjectionMatrix();
}

function updateOrbitSelection() {
  orbitEntries.forEach(({ name, line, beacon }) => {
    const selected = name === state.mode;
    line.material.opacity = selected ? 0.68 : 0.14;
    beacon.material.opacity = selected ? 1 : 0.58;
  });
}

function buildProofTrace() {
  const fragment = document.createDocumentFragment();
  for (let index = 0; index < 96; index += 1) {
    const segment = document.createElement("i");
    const height = 18 + ((index * 37 + index * index * 11) % 78);
    segment.style.setProperty("--height", `${height}%`);
    fragment.append(segment);
  }
  el.proofTrace.append(fragment);
}

function updateProofTrace(percent) {
  const activeCount = Math.round((percent / 100) * el.proofTrace.children.length);
  [...el.proofTrace.children].forEach((segment, index) => {
    segment.classList.toggle("active", index < activeCount && !state.lastResult);
    segment.classList.toggle("complete", index < activeCount && Boolean(state.lastResult));
  });
}

function setProgress(percent) {
  const bounded = Math.max(0, Math.min(100, percent));
  state.proofProgress = bounded / 100;
  el.progressBar.style.width = `${bounded}%`;
  updateProofTrace(bounded);
  if (state.running) {
    el.eventPhase.textContent = `VERIFYING ${state.mode.toUpperCase()}`;
    el.eventValue.textContent = `${Math.round(bounded)}% CLOSED`;
  }
}

function selectMode(name, withTone = true) {
  if (state.running || !modes[name]) return;
  state.mode = name;
  state.lastResult = null;
  const mode = modes[name];
  document.documentElement.style.setProperty(
    "--mode-color",
    `#${mode.color.toString(16).padStart(6, "0")}`,
  );
  document.querySelectorAll(".proof-mode").forEach((button) => {
    const active = button.dataset.mode === name;
    button.classList.toggle("active", active);
    button.setAttribute("aria-pressed", active ? "true" : "false");
  });
  el.domainLabel.innerHTML =
    mode.domainLabel ?? `2<sup>${mode.exponent.toLocaleString()}</sup>`;
  el.domainCaption.textContent = mode.domainCaption ?? "IMPLICIT DOMAIN";
  el.domainDetail.textContent = mode.domain;
  el.verifierPath.textContent = mode.verifierPath;
  el.allocationValue.textContent = mode.allocation;
  el.orbitKicker.textContent = mode.kicker;
  el.orbitDescription.textContent = mode.description;
  el.verificationStatus.textContent = mode.status;
  el.verificationTitle.textContent = mode.title;
  el.verificationDetail.textContent = mode.detail;
  el.verifyButton.querySelector("span").textContent = mode.button;
  el.sealValue.textContent =
    mode.exponent >= 1_000_000 ? "1M" : mode.exponent.toLocaleString();
  el.sealUnit.textContent = mode.unit ?? "ROUNDS";
  el.roundValue.textContent = `0 / ${mode.exponent.toLocaleString()}`;
  el.claimValue.textContent = "WAITING";
  el.digestValue.textContent = "PENDING";
  el.modeValue.textContent = name.toUpperCase();
  el.dossierMode.textContent = name.toUpperCase();
  el.dossierDomain.textContent =
    mode.dossierDomain ?? `2^${mode.exponent.toLocaleString()}`;
  el.dossierWork.textContent = mode.verifierPath;
  el.dossierArtifact.textContent = mode.dossierArtifact;
  el.monumentIndex.textContent = String(Object.keys(modes).indexOf(name) + 1).padStart(2, "0");
  el.downloadButton.href = mode.downloadHref;
  el.downloadButton.target = mode.downloadName ? "" : "_blank";
  if (mode.downloadName) el.downloadButton.setAttribute("download", mode.downloadName);
  else el.downloadButton.removeAttribute("download");
  el.shareButton.disabled = true;
  el.statusSeal.classList.remove("verified");
  el.eventPhase.textContent = "FIELD ARMED";
  el.eventValue.textContent = `${mode.exponent.toLocaleString()} ROUNDS READY`;
  document.body.classList.remove("proof-running", "proof-verified");
  setProgress(0);
  updateOrbitSelection();
  if (withTone) tone(420, 0.035);
}

function mod(value) {
  const result = value % FIELD;
  return result < 0n ? result + FIELD : result;
}

function modPow(base, exponent) {
  let result = 1n;
  let factor = mod(base);
  let power = BigInt(exponent);
  while (power > 0n) {
    if (power & 1n) result = mod(result * factor);
    factor = mod(factor * factor);
    power >>= 1n;
  }
  return result;
}

async function sha256Hex(value) {
  const bytes =
    value instanceof Uint8Array ? value : new TextEncoder().encode(String(value));
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function assertBrowserCanonicalNumbers(value) {
  if (typeof value === "number" && !Number.isSafeInteger(value)) {
    throw new Error("Browser verification requires safe integer JSON numbers");
  }
  if (Array.isArray(value)) {
    value.forEach(assertBrowserCanonicalNumbers);
  } else if (value && typeof value === "object") {
    Object.values(value).forEach(assertBrowserCanonicalNumbers);
  }
}

async function domainSeparatedHash(domain, value) {
  const domainBytes = new TextEncoder().encode(`${domain}\0`);
  const valueBytes = new TextEncoder().encode(canonicalJson(value));
  const combined = new Uint8Array(domainBytes.length + valueBytes.length);
  combined.set(domainBytes);
  combined.set(valueBytes, domainBytes.length);
  return `sha256:${await sha256Hex(combined)}`;
}

async function verifyPhaArtifact(artifact) {
  if (artifact?.schema !== "power-house/pha/v1") {
    throw new Error("Unsupported PHA schema");
  }
  const embedded = artifact.embedded_proof;
  if (!embedded || typeof embedded.protocol !== "string" || !embedded.protocol.trim()) {
    throw new Error("Invalid embedded Power House proof");
  }
  assertBrowserCanonicalNumbers(artifact.provenance);
  assertBrowserCanonicalNumbers(embedded.public_inputs);
  assertBrowserCanonicalNumbers(embedded.proof);
  const core = {
    embedded_proof: {
      proof: embedded.proof,
      protocol: embedded.protocol,
      public_inputs: embedded.public_inputs,
    },
    provenance: artifact.provenance,
    schema: artifact.schema,
  };
  const expected = await domainSeparatedHash(
    "power-house:pha:v1:phx-fingerprint",
    core,
  );
  if (artifact.phx_fingerprint !== expected) {
    throw new Error("PHA core fingerprint mismatch");
  }
}

async function verifyRootprintGraph(graph) {
  if (graph?.schema !== "power-house/rootprint/v1") {
    throw new Error("Unsupported Rootprint schema");
  }
  const branches = graph.branches;
  const root = branches?.[graph.root_branch];
  if (!root || root.sequence !== 0 || root.parents.length !== 0) {
    throw new Error("Invalid Rootprint root branch");
  }
  const branchEntries = Object.entries(branches);
  for (let index = 0; index < branchEntries.length; index += 1) {
    const [key, branch] = branchEntries[index];
    if (key !== branch.id) throw new Error("Rootprint branch key mismatch");
    await verifyPhaArtifact(branch.artifact);
    const parents = branch.parents;
    if (
      parents.length > 2 ||
      parents.some((parent, parentIndex) => parentIndex > 0 && parents[parentIndex - 1] >= parent)
    ) {
      throw new Error("Rootprint parents are not sorted and unique");
    }
    if (branch.id !== graph.root_branch && parents.length === 0) {
      throw new Error("Rootprint non-root branch has no parent");
    }
    for (const parentId of parents) {
      const parent = branches[parentId];
      if (!parent || parent.sequence >= branch.sequence) {
        throw new Error("Rootprint parent ordering failed");
      }
    }
    const expectedId = await domainSeparatedHash("power-house:rootprint:v1:branch-id", {
      artifact_phx_fingerprint: branch.artifact.phx_fingerprint,
      label: branch.label,
      parents,
    });
    if (branch.id !== expectedId) throw new Error("Rootprint branch identifier mismatch");
    el.roundValue.textContent = `${index + 1} / ${branchEntries.length}`;
    setProgress(35 + ((index + 1) / branchEntries.length) * 55);
  }

  const reachable = new Set([graph.root_branch]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const branch of Object.values(branches)) {
      if (!reachable.has(branch.id) && branch.parents.some((parent) => reachable.has(parent))) {
        reachable.add(branch.id);
        changed = true;
      }
    }
  }
  if (reachable.size !== branchEntries.length) {
    throw new Error("Rootprint contains an unreachable branch");
  }
  return branchEntries.length;
}

function sleep(milliseconds) {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

function beginRun() {
  state.running = true;
  state.lastResult = null;
  document.body.classList.add("proof-running");
  document.body.classList.remove("proof-verified");
  el.verifyButton.disabled = true;
  el.shareButton.disabled = true;
  el.statusSeal.classList.remove("verified");
  el.digestValue.textContent = "WORKING";
  setProgress(0);
}

function completeRun(digest, claim = "VERIFIED") {
  state.running = false;
  state.lastResult = {
    mode: state.mode,
    digest: digest.toUpperCase(),
    claim,
    rounds: modes[state.mode].exponent,
  };
  el.verifyButton.disabled = false;
  el.shareButton.disabled = false;
  el.digestValue.textContent = digest.slice(0, 12).toUpperCase();
  el.claimValue.textContent = claim;
  el.statusSeal.classList.add("verified");
  el.verificationStatus.textContent = "VERIFICATION COMPLETE";
  el.eventPhase.textContent = "CERTIFICATE ACCEPTED";
  el.eventValue.textContent = digest.slice(0, 16).toUpperCase();
  document.body.classList.remove("proof-running");
  document.body.classList.add("proof-verified");
  setProgress(100);
  tone(760, 0.12);
}

function failRun(message) {
  state.running = false;
  document.body.classList.remove("proof-running", "proof-verified");
  el.verifyButton.disabled = false;
  el.verificationStatus.textContent = "VERIFICATION STOPPED";
  el.claimValue.textContent = "FAILED";
  el.digestValue.textContent = "REJECTED";
  el.eventPhase.textContent = "VERIFICATION REJECTED";
  el.eventValue.textContent = "INSPECT FAILURE";
  showToast(message);
  tone(180, 0.14);
}

async function runConstantProof() {
  beginRun();
  const rounds = [];
  let running = mod(CONSTANT * modPow(2n, 70));
  el.claimValue.textContent = running.toString();
  el.verificationStatus.textContent = "REPLAYING FIELD EQUATIONS";

  for (let index = 0; index < 70; index += 1) {
    const remaining = 70 - index;
    const b = mod(CONSTANT * modPow(2n, remaining - 1));
    const a = 0n;
    const equation = mod(b + (a + b));
    if (equation !== running) {
      failRun(`Round ${index + 1} failed the g(0) + g(1) equation.`);
      return;
    }
    rounds.push([a.toString(), b.toString(), running.toString()]);
    running = b;
    el.roundValue.textContent = `${index + 1} / 70`;
    setProgress(((index + 1) / 70) * 100);
    if (index % 7 === 0) tone(300 + index * 4, 0.018);
    if (index % 5 === 4) await sleep(12);
  }

  if (running !== CONSTANT) {
    failRun("Final evaluation did not equal the public constant.");
    return;
  }
  const digest = await sha256Hex(
    JSON.stringify({
      format: "MFENX_BROWSER_CONSTANT_V1",
      field: FIELD.toString(),
      variables: 70,
      constant: CONSTANT.toString(),
      rounds,
      final: running.toString(),
    }),
  );
  completeRun(digest, running.toString());
  el.verificationTitle.textContent = "All 70 round equations accepted";
  el.verificationDetail.textContent =
    "The canonical Rust example also derives Fiat-Shamir challenges and trace metadata.";
}

function seededWord(index) {
  let value = BigInt(index + 1) ^ 0x504f574552484f55n;
  value ^= value << 13n;
  value ^= value >> 7n;
  value ^= value << 17n;
  return mod(value);
}

async function runAffineReplay() {
  beginRun();
  const count = 4096;
  const constant = seededWord(0);
  const coefficients = Array.from({ length: count }, (_, index) => seededWord(index + 1));
  const sum = coefficients.reduce((accumulator, value) => mod(accumulator + value), 0n);
  let running = mod(constant * modPow(2n, count) + sum * modPow(2n, count - 1));
  let prefix = constant;
  let suffix = sum;
  const digestWords = [];
  el.claimValue.textContent = running.toString();
  el.verificationStatus.textContent = "CHECKING 4,096 AFFINE ROUNDS";

  for (let index = 0; index < count; index += 1) {
    const coefficient = coefficients[index];
    suffix = mod(suffix - coefficient);
    const remaining = count - index - 1;
    const scale = modPow(2n, remaining);
    const a = mod(coefficient * scale);
    const later = remaining === 0 ? 0n : mod(suffix * modPow(2n, remaining - 1));
    const b = mod(prefix * scale + later);
    if (mod(b + a + b) !== running) {
      failRun(`Affine recurrence failed at round ${index + 1}.`);
      return;
    }
    const challenge = seededWord(index + count + 1);
    prefix = mod(prefix + coefficient * challenge);
    running = mod(a * challenge + b);
    if (index % 64 === 0) {
      digestWords.push(`${a}:${b}:${running}`);
      const completed = Math.min(index + 64, count);
      el.roundValue.textContent = `${completed.toLocaleString()} / 4,096`;
      setProgress((completed / count) * 100);
      tone(350 + (index / 64) * 3, 0.012);
      await sleep(7);
    }
  }

  if (running !== prefix) {
    failRun("Affine final evaluation mismatch.");
    return;
  }
  const digest = await sha256Hex(digestWords.join("|"));
  completeRun(digest, running.toString());
  el.verificationTitle.textContent = "4,096 structural rounds accepted";
  el.verificationDetail.textContent =
    "Use the Rust example for the canonical BLAKE2b Fiat-Shamir proof and exact release digest.";
}

async function fetchWithProgress(url, expectedLength, progressStart, progressSpan) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Download failed with HTTP ${response.status}`);
  if (!response.body) {
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (bytes.length !== expectedLength) {
      throw new Error(
        `Artifact length mismatch: expected ${expectedLength}, received ${bytes.length}`,
      );
    }
    return bytes;
  }
  const reader = response.body.getReader();
  const stream = new StreamBuffer(expectedLength);
  let received = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    stream.append(value);
    received += value.length;
    const ratio = Math.min(received / expectedLength, 0.99);
    setProgress(progressStart + ratio * progressSpan);
    el.verificationDetail.textContent = `${(received / 1_000_000).toFixed(
      1,
    )} MB streamed for SHA-256`;
  }
  return stream.finish();
}

async function verifyReleaseArtifacts(kind) {
  beginRun();
  try {
    if (kind === "sparse") {
      el.verificationStatus.textContent = "DOWNLOADING PHSPv1 RELEASE";
      const bytes = await fetchWithProgress(
        "artifacts/power_house_sparse_record.phsp",
        knownArtifacts.phsp.size,
        0,
        78,
      );
      el.verificationStatus.textContent = "COMPUTING FULL SHA-256";
      const digest = await sha256Hex(bytes);
      if (digest !== knownArtifacts.phsp.hash) throw new Error("PHSPv1 SHA-256 mismatch");
      el.roundValue.textContent = "1,000,000 / 1,000,000";
      completeRun(digest, "PHSPv1 OK");
      el.verificationTitle.textContent = "Published 16 MB certificate is authentic";
      el.verificationDetail.textContent =
        "Algebraic replay remains reproducible with the bundled Rust and Python verifiers.";
    } else {
      el.verificationStatus.textContent = "DOWNLOADING PHSMv1 WORKLOAD";
      const workload = await fetchWithProgress(
        "artifacts/external_interaction_model.phsm",
        knownArtifacts.phsm.size,
        0,
        12,
      );
      const workloadDigest = await sha256Hex(workload);
      if (workloadDigest !== knownArtifacts.phsm.hash) {
        throw new Error("PHSMv1 SHA-256 mismatch");
      }

      el.verificationStatus.textContent = "DOWNLOADING PHCPv1 PROOF";
      const proof = await fetchWithProgress(
        "artifacts/external_interaction_model.phcp",
        knownArtifacts.phcp.size,
        14,
        72,
      );
      const proofDigest = await sha256Hex(proof);
      if (proofDigest !== knownArtifacts.phcp.hash) throw new Error("PHCPv1 SHA-256 mismatch");
      el.roundValue.textContent = "1,000,000 / 1,000,000";
      completeRun(proofDigest, "BOTH FILES OK");
      el.verificationTitle.textContent = "Workload and certificate hashes accepted";
      el.verificationDetail.textContent =
        "The release verifier additionally checks the BLAKE2b workload commitment and every sum-check round.";
    }
  } catch (error) {
    failRun(`${error.message}. The release files remain available from GitHub.`);
  }
}

async function verifyRootprintRelease() {
  beginRun();
  try {
    el.verificationStatus.textContent = "DOWNLOADING ROOTPRINT v1";
    const bytes = await fetchWithProgress(
      "artifacts/rootprint-valid.json",
      knownArtifacts.rootprint.size,
      0,
      28,
    );
    const digest = await sha256Hex(bytes);
    if (digest !== knownArtifacts.rootprint.hash) {
      throw new Error("Rootprint release SHA-256 mismatch");
    }
    el.verificationStatus.textContent = "REPLAYING CORE IDENTITIES";
    const graph = JSON.parse(new TextDecoder().decode(bytes));
    const branchCount = await verifyRootprintGraph(graph);
    completeRun(digest, `${branchCount} BRANCHES OK`);
    el.verificationTitle.textContent = "Rootprint graph and every PHA core accepted";
    el.verificationDetail.textContent =
      "EPA data was transported but deliberately excluded from all core identities.";
  } catch (error) {
    failRun(error.message);
  }
}

function artifactType(file) {
  return file.name.toLowerCase().split(".").pop();
}

async function readLocalFile(file, start, span) {
  const expected = knownArtifacts[artifactType(file)];
  if (!expected) throw new Error(`Unsupported artifact: ${file.name}`);
  if (file.size !== expected.size) {
    throw new Error(
      `${expected.label} length mismatch: expected ${expected.size}, received ${file.size}`,
    );
  }
  if (!file.stream) return new Uint8Array(await file.arrayBuffer());
  const reader = file.stream().getReader();
  const stream = new StreamBuffer(expected.size);
  let received = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    stream.append(value);
    received += value.length;
    setProgress(start + (received / expected.size) * span);
    el.verificationDetail.textContent = `${expected.label}: ${(
      received / 1_000_000
    ).toFixed(1)} MB read locally`;
  }
  return stream.finish();
}

async function verifyLocalArtifacts(fileList) {
  const files = [...fileList];
  const byType = new Map(files.map((file) => [artifactType(file), file]));
  const portable = byType.get("json") ?? byType.get("pha");
  if (portable) {
    if (portable.size > 2_000_000) {
      showToast("PHA and Rootprint JSON files must be 2 MB or smaller.");
      return;
    }
    selectMode("rootprint");
    beginRun();
    try {
      const bytes = new Uint8Array(await portable.arrayBuffer());
      const value = JSON.parse(new TextDecoder().decode(bytes));
      if (value.schema === "power-house/rootprint/v1") {
        const branchCount = await verifyRootprintGraph(value);
        completeRun(await sha256Hex(bytes), `${branchCount} BRANCHES OK`);
      } else {
        await verifyPhaArtifact(value);
        completeRun(await sha256Hex(bytes), "PHA CORE OK");
      }
      el.verificationTitle.textContent = "Local Power House identity accepted";
      el.verificationDetail.textContent =
        "Verification used only Power House core fields; EPA remained optional.";
    } catch (error) {
      failRun(error.message);
    } finally {
      el.artifactInput.value = "";
    }
    return;
  }
  const committed = byType.has("phsm") || byType.has("phcp");
  const required = committed ? ["phsm", "phcp"] : ["phsp"];
  const missing = required.filter((type) => !byType.has(type));
  if (missing.length) {
    showToast(`Select ${missing.map((type) => type.toUpperCase()).join(" + ")} together.`);
    return;
  }

  selectMode(committed ? "committed" : "sparse");
  beginRun();
  try {
    let finalDigest = "";
    for (let index = 0; index < required.length; index += 1) {
      const type = required[index];
      const artifact = knownArtifacts[type];
      const start = (index / required.length) * 90;
      const span = 90 / required.length;
      el.verificationStatus.textContent = `HASHING LOCAL ${artifact.label}`;
      const bytes = await readLocalFile(byType.get(type), start, span);
      const digest = await sha256Hex(bytes);
      if (digest !== artifact.hash) throw new Error(`${artifact.label} SHA-256 mismatch`);
      finalDigest = digest;
    }
    el.roundValue.textContent = "LOCAL BYTES / ACCEPTED";
    completeRun(finalDigest, committed ? "LOCAL PAIR OK" : "LOCAL PHSP OK");
    el.verificationTitle.textContent = committed
      ? "Local workload and certificate match the release"
      : "Local certificate matches the published release";
    el.verificationDetail.textContent =
      "The selected bytes were hashed locally and matched the immutable release digest.";
  } catch (error) {
    failRun(error.message);
  } finally {
    el.artifactInput.value = "";
  }
}

function scheduleAutoProof() {
  window.clearTimeout(autoProofTimer);
  autoProofTimer = window.setTimeout(() => {
    autoProofTimer = 0;
    if (!state.userInteracted && !state.running && state.mode === "rootprint") {
      verifyRootprintRelease();
    }
  }, 2400);
}

function cancelAutoProof() {
  state.userInteracted = true;
  window.clearTimeout(autoProofTimer);
  autoProofTimer = 0;
}

function tone(frequency, duration) {
  if (!state.sound) return;
  try {
    audioContext ||= new AudioContext();
    const oscillator = audioContext.createOscillator();
    const gain = audioContext.createGain();
    oscillator.type = "sine";
    oscillator.frequency.value = frequency;
    gain.gain.setValueAtTime(0.025, audioContext.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.0001, audioContext.currentTime + duration);
    oscillator.connect(gain);
    gain.connect(audioContext.destination);
    oscillator.start();
    oscillator.stop(audioContext.currentTime + duration);
  } catch {
    state.sound = false;
  }
}

function showToast(message) {
  window.clearTimeout(state.toastTimer);
  el.toast.textContent = message;
  el.toast.classList.add("show");
  state.toastTimer = window.setTimeout(() => el.toast.classList.remove("show"), 3600);
}

function updateNetworkNodeStates(healthy) {
  const nodeFields = [el.nodeSfoState, el.nodeNycState, el.nodeAmsState];
  nodeFields.forEach((field, index) => {
    const online = index < healthy;
    field.textContent = online ? "ONLINE" : "CHECK";
    field.closest("button").classList.toggle("online", online);
  });
}

async function refreshNetworkStatus() {
  try {
    const response = await fetch("https://rpc.mfenx.com/network-status.json", {
      cache: "no-store",
    });
    const data = await response.json();
    if (!response.ok) throw new Error(data.error || `HTTP ${response.status}`);

    const networkState = data.status || "degraded";
    const healthy = Number(data.validators_healthy) || 0;
    document.body.dataset.network = networkState;
    el.networkState.textContent = networkState.toUpperCase();
    el.networkConsoleState.textContent = networkState.toUpperCase();
    el.networkBlock.textContent = Number(data.block_height).toLocaleString("en-US");
    el.networkValidators.textContent = `${healthy} / ${Number(data.validators_total) || 3}`;
    el.networkPeers.textContent = Number(data.peer_connections).toLocaleString("en-US");
    updateNetworkNodeStates(healthy);
  } catch {
    document.body.dataset.network = "unknown";
    el.networkState.textContent = "FEED CHECK";
    el.networkConsoleState.textContent = "FEED CHECK";
    updateNetworkNodeStates(0);
  }
}

async function copyText(value, successMessage) {
  try {
    await navigator.clipboard.writeText(value);
    showToast(successMessage);
  } catch {
    showToast("Clipboard access is unavailable in this browser.");
  }
}

async function shareVerification() {
  if (!state.lastResult) return;
  const result = state.lastResult;
  const text = `MFENX Power House ${result.mode.toUpperCase()} verification accepted: ${result.rounds.toLocaleString()} rounds, digest ${result.digest.slice(
    0,
    16,
  )}…`;
  if (navigator.share) {
    try {
      await navigator.share({ title: "MFENX Power House Verification", text, url: location.href });
      return;
    } catch (error) {
      if (error.name === "AbortError") return;
    }
  }
  await copyText(`${text} ${location.href}`, "Verification result copied");
}

function setTimeOffset(hours) {
  state.timeOffsetHours = THREE.MathUtils.clamp(Number(hours), -24, 24);
  el.timeSlider.value = String(state.timeOffsetHours);
  updateAstronomy();
}

function setMotion(enabled) {
  state.motion = enabled;
  mountIcon(el.motionToggle.querySelector("[data-icon]"), enabled ? "pause" : "play");
  el.motionToggle.classList.toggle("active", !enabled);
  el.motionToggle.setAttribute(
    "aria-label",
    enabled ? "Pause orbital motion" : "Resume orbital motion",
  );
  el.motionToggle.title = enabled ? "Pause orbital motion" : "Resume orbital motion";
}

function setSound(enabled) {
  state.sound = enabled;
  mountIcon(el.soundToggle.querySelector("[data-icon]"), enabled ? "volume-2" : "volume-x");
  el.soundToggle.classList.toggle("active", enabled);
  el.soundToggle.setAttribute(
    "aria-label",
    enabled ? "Mute interface sound" : "Enable interface sound",
  );
  if (enabled) tone(620, 0.08);
  showToast(enabled ? "Interface sound enabled" : "Interface sound muted");
}

function bindInterface() {
  document.addEventListener(
    "pointerdown",
    cancelAutoProof,
    { capture: true, once: true },
  );
  document.querySelectorAll(".proof-mode").forEach((button) => {
    button.addEventListener("click", () => selectMode(button.dataset.mode));
  });
  el.verifyButton.addEventListener("click", () => {
    if (!state.running) modes[state.mode].action();
  });
  el.artifactButton.addEventListener("click", () => el.artifactInput.click());
  el.artifactInput.addEventListener("change", () => {
    if (el.artifactInput.files.length) verifyLocalArtifacts(el.artifactInput.files);
  });
  el.shareButton.addEventListener("click", shareVerification);
  el.installCommand.addEventListener("click", () =>
    copyText("cargo add power_house", "Install command copied"),
  );
  el.citySearch.addEventListener("input", (event) => filterCities(event.target.value));
  el.observatoryToggle.addEventListener("click", () => {
    document.body.classList.remove("evaluation-open");
    document.body.classList.add("observatory-open");
  });
  el.observatoryClose.addEventListener("click", () =>
    document.body.classList.remove("observatory-open"),
  );
  el.evaluationToggle.addEventListener("click", () => {
    document.body.classList.remove("observatory-open");
    document.body.classList.add("evaluation-open");
  });
  el.evaluationClose.addEventListener("click", () =>
    document.body.classList.remove("evaluation-open"),
  );
  el.timeSlider.addEventListener("input", (event) => setTimeOffset(event.target.value));
  el.timeBack.addEventListener("click", () => setTimeOffset(state.timeOffsetHours - 1));
  el.timeForward.addEventListener("click", () => setTimeOffset(state.timeOffsetHours + 1));
  el.timeLive.addEventListener("click", () => setTimeOffset(0));
  el.focusToggle.addEventListener("click", () => {
    focusSelectedCity();
    showToast(`${cities[state.activeCity].name} centered`);
  });
  el.networkToggle.addEventListener("click", () => {
    document.body.classList.remove("evaluation-open");
    document.body.classList.add("observatory-open");
    window.setTimeout(
      () => el.networkConsole.scrollIntoView({ behavior: "smooth", block: "end" }),
      300,
    );
  });
  el.zoomIn.addEventListener("click", () => {
    state.zoom = Math.max(3.45, state.zoom - 0.32);
  });
  el.zoomOut.addEventListener("click", () => {
    state.zoom = Math.min(6.2, state.zoom + 0.32);
  });
  el.viewReset.addEventListener("click", () => {
    setTimeOffset(0);
    focusSelectedCity();
    state.zoom = window.innerWidth < 760 ? 4.85 : 4.55;
    showToast("Orbital view reset");
  });
  document.querySelectorAll("[data-network-city]").forEach((button) => {
    button.addEventListener("click", () => selectCity(Number(button.dataset.networkCity)));
  });
  el.soundToggle.addEventListener("click", () => setSound(!state.sound));
  el.motionToggle.addEventListener("click", () => setMotion(!state.motion));
  window.addEventListener("resize", resize);
  document.addEventListener("visibilitychange", () => {
    state.visible = !document.hidden;
  });
  window.addEventListener("keydown", (event) => {
    cancelAutoProof();
    const target = event.target;
    const typing =
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target instanceof HTMLSelectElement;
    if (event.key === "Escape") {
      document.body.classList.remove("observatory-open", "evaluation-open");
      hideGlobeTooltip();
      return;
    }
    if (typing) return;
    if (event.key === "Enter" && !state.running) modes[state.mode].action();
    if (event.key === " ") {
      event.preventDefault();
      setMotion(!state.motion);
    }
    if (event.key === "ArrowLeft") state.targetRotationY -= 0.12;
    if (event.key === "ArrowRight") state.targetRotationY += 0.12;
    if (event.key === "ArrowUp") {
      state.targetRotationX = THREE.MathUtils.clamp(state.targetRotationX - 0.08, -1.18, 1.18);
    }
    if (event.key === "ArrowDown") {
      state.targetRotationX = THREE.MathUtils.clamp(state.targetRotationX + 0.08, -1.18, 1.18);
    }
    if (event.key === "+" || event.key === "=") state.zoom = Math.max(3.45, state.zoom - 0.2);
    if (event.key === "-") state.zoom = Math.min(6.2, state.zoom + 0.2);
  });
}

function applyUrlState() {
  const params = new URLSearchParams(window.location.search);
  const requestedMode = params.get("mode");
  if (requestedMode && modes[requestedMode]) state.mode = requestedMode;

  const requestedCity = params.get("city")?.toUpperCase();
  if (requestedCity) {
    const cityIndex = cities.findIndex(
      (city) => city.code === requestedCity || city.name === requestedCity,
    );
    if (cityIndex >= 0) state.activeCity = cityIndex;
  }

  const requestedOffset = Number(params.get("time"));
  if (Number.isFinite(requestedOffset)) {
    state.timeOffsetHours = THREE.MathUtils.clamp(requestedOffset, -24, 24);
  }

  const panel = params.get("panel");
  if (panel === "observatory") document.body.classList.add("observatory-open");
  if (panel === "evaluation") document.body.classList.add("evaluation-open");
}

function init() {
  mountIcons();
  setBootProgress(22);
  buildProofTrace();
  buildCityList();
  bindInterface();
  applyUrlState();
  selectMode(state.mode, false);
  selectCity(state.activeCity, false);
  focusSelectedCity();
  setTimeOffset(state.timeOffsetHours);
  setMotion(state.motion);
  updateAstronomy();
  refreshNetworkStatus();
  window.setInterval(updateAstronomy, 1000);
  window.setInterval(refreshNetworkStatus, 15_000);
  initScene();
}

window.addEventListener("beforeunload", () => {
  window.clearTimeout(autoProofTimer);
  if (animationFrame) cancelAnimationFrame(animationFrame);
});

init();
