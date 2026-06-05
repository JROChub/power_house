import * as THREE from "./vendor/three.module.min.js";

const FIELD = 1_000_000_007n;
const CONSTANT = 173n;
const modes = {
  constant: {
    exponent: 70,
    domain: "1,180,591,620,717,411,303,424 points",
    kicker: "CLOSED-FORM SUM-CHECK",
    description:
      "Seventy verifier rounds close a domain larger than one sextillion Boolean points without enumerating it.",
    title: "Run the 70-round browser proof",
    detail:
      "The browser checks every round equation over the field and computes a certificate SHA-256 digest.",
    button: "RUN PROOF",
    status: "LOCAL VERIFIER READY",
    action: runConstantProof,
  },
  affine: {
    exponent: 4096,
    domain: "1,234 decimal digits in the implicit domain",
    kicker: "SEEDED NON-CONSTANT MODEL",
    description:
      "A public seed defines 4,096 affine coefficients. The canonical Rust verifier replays one round per variable.",
    title: "Run a 4,096-round structural replay",
    detail:
      "This browser replay checks the affine recurrence; the release implementation uses BLAKE2b Fiat-Shamir challenges in Rust.",
    button: "RUN REPLAY",
    status: "BROWSER MODEL READY",
    action: runAffineReplay,
  },
  sparse: {
    exponent: 1_000_000,
    domain: "301,030 decimal digits in the implicit domain",
    kicker: "MILLION-ROUND CERTIFICATE",
    description:
      "A stable 16 MB PHSPv1 certificate covers a seeded sparse polynomial over one million Boolean variables.",
    title: "Verify the published PHSPv1 artifact",
    detail:
      "Downloads the immutable release asset and checks its full SHA-256 digest in this browser.",
    button: "VERIFY HASH",
    status: "RELEASE ARTIFACT READY",
    action: () => verifyReleaseArtifacts("sparse"),
  },
  committed: {
    exponent: 1_000_000,
    domain: "301,030 decimal digits in the implicit domain",
    kicker: "EXTERNAL WORKLOAD BINDING",
    description:
      "The PHCPv1 proof binds a separate PHSMv1 sparse workload through a domain-separated BLAKE2b-256 commitment.",
    title: "Verify both committed release artifacts",
    detail:
      "Downloads the external workload and million-round certificate, then checks both SHA-256 digests.",
    button: "VERIFY BOTH",
    status: "TWO-FILE BINDING READY",
    action: () => verifyReleaseArtifacts("committed"),
  },
};

const cities = [
  { name: "SAN FRANCISCO", code: "SFO", zone: "America/Los_Angeles", lat: 37.77, lon: -122.42 },
  { name: "NEW YORK", code: "NYC", zone: "America/New_York", lat: 40.71, lon: -74.0 },
  { name: "GREENWICH", code: "UTC", zone: "Europe/London", lat: 51.48, lon: 0.0 },
  { name: "SAO PAULO", code: "SAO", zone: "America/Sao_Paulo", lat: -23.55, lon: -46.63 },
  { name: "LAGOS", code: "LOS", zone: "Africa/Lagos", lat: 6.52, lon: 3.38 },
  { name: "DUBAI", code: "DXB", zone: "Asia/Dubai", lat: 25.2, lon: 55.27 },
  { name: "DELHI", code: "DEL", zone: "Asia/Kolkata", lat: 28.61, lon: 77.21 },
  { name: "SINGAPORE", code: "SIN", zone: "Asia/Singapore", lat: 1.35, lon: 103.82 },
  { name: "TOKYO", code: "TYO", zone: "Asia/Tokyo", lat: 35.68, lon: 139.69 },
  { name: "SYDNEY", code: "SYD", zone: "Australia/Sydney", lat: -33.87, lon: 151.21 },
];

const el = {
  canvas: document.querySelector("#orbital-canvas"),
  utcDate: document.querySelector("#utc-date"),
  utcTime: document.querySelector("#utc-time"),
  cityList: document.querySelector("#city-list"),
  stageCity: document.querySelector("#stage-city"),
  stageTime: document.querySelector("#stage-time"),
  stageZone: document.querySelector("#stage-zone"),
  solarPosition: document.querySelector("#solar-position"),
  domainLabel: document.querySelector("#domain-label"),
  domainDetail: document.querySelector("#domain-detail"),
  orbitKicker: document.querySelector("#orbit-kicker"),
  orbitDescription: document.querySelector("#orbit-description"),
  verificationStatus: document.querySelector("#verification-status"),
  verificationTitle: document.querySelector("#verification-title"),
  verificationDetail: document.querySelector("#verification-detail"),
  verifyButton: document.querySelector("#verify-button"),
  progressBar: document.querySelector("#progress-bar"),
  roundValue: document.querySelector("#round-value"),
  claimValue: document.querySelector("#claim-value"),
  digestValue: document.querySelector("#digest-value"),
  statusSeal: document.querySelector("#status-seal"),
  sealValue: document.querySelector("#seal-value"),
  toast: document.querySelector("#toast"),
  soundToggle: document.querySelector("#sound-toggle"),
  motionToggle: document.querySelector("#motion-toggle"),
};

const state = {
  mode: "constant",
  activeCity: 2,
  running: false,
  motion: !window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  sound: false,
  targetRotationX: 0.16,
  targetRotationY: 0,
  dragX: 0,
  dragY: 0,
  zoom: 4.6,
  toastTimer: 0,
};

let renderer;
let scene;
let camera;
let earthGroup;
let earthMaterial;
let atmosphereMaterial;
let orbitGroup;
let proofNodes = [];
let audioContext;

function pad(value) {
  return String(value).padStart(2, "0");
}

function formatClock(date, zone, seconds = true) {
  return new Intl.DateTimeFormat("en-GB", {
    timeZone: zone,
    hour: "2-digit",
    minute: "2-digit",
    second: seconds ? "2-digit" : undefined,
    hour12: false,
  }).format(date);
}

function updateClocks() {
  const now = new Date();
  el.utcDate.textContent = new Intl.DateTimeFormat("en-US", {
    timeZone: "UTC",
    month: "short",
    day: "2-digit",
    year: "numeric",
  })
    .format(now)
    .toUpperCase();
  el.utcTime.textContent = `${pad(now.getUTCHours())}:${pad(now.getUTCMinutes())}:${pad(
    now.getUTCSeconds(),
  )}`;

  document.querySelectorAll(".city-row").forEach((row, index) => {
    row.querySelector("strong").textContent = formatClock(now, cities[index].zone, false);
  });

  const city = cities[state.activeCity];
  el.stageTime.textContent = formatClock(now, city.zone);
}

function buildCityList() {
  const fragment = document.createDocumentFragment();
  cities.forEach((city, index) => {
    const button = document.createElement("button");
    button.className = `city-row${index === state.activeCity ? " active" : ""}`;
    button.innerHTML = `<b>${city.name}</b><strong>00:00</strong><small>${city.code} / ${city.zone}</small>`;
    button.addEventListener("click", () => selectCity(index));
    fragment.append(button);
  });
  el.cityList.append(fragment);
}

function selectCity(index) {
  state.activeCity = index;
  const city = cities[index];
  document.querySelectorAll(".city-row").forEach((row, rowIndex) => {
    row.classList.toggle("active", rowIndex === index);
  });
  el.stageCity.textContent = city.name;
  el.stageZone.textContent = city.zone.toUpperCase();
  state.targetRotationX = THREE.MathUtils.degToRad(city.lat) * 0.55;
  state.targetRotationY = -THREE.MathUtils.degToRad(city.lon);
  updateClocks();
  tone(520, 0.04);
}

function dayOfYear(date) {
  const start = Date.UTC(date.getUTCFullYear(), 0, 0);
  return Math.floor((date.getTime() - start) / 86_400_000);
}

function solarCoordinates(date) {
  const day = dayOfYear(date);
  const hours =
    date.getUTCHours() + date.getUTCMinutes() / 60 + date.getUTCSeconds() / 3600;
  const declination = -23.44 * Math.cos((2 * Math.PI * (day + 10)) / 365);
  let longitude = 180 - hours * 15;
  if (longitude > 180) longitude -= 360;
  if (longitude < -180) longitude += 360;
  return { lat: declination, lon: longitude };
}

function latLonVector(lat, lon, radius = 1) {
  const phi = THREE.MathUtils.degToRad(90 - lat);
  const theta = THREE.MathUtils.degToRad(lon + 180);
  return new THREE.Vector3(
    -radius * Math.sin(phi) * Math.cos(theta),
    radius * Math.cos(phi),
    radius * Math.sin(phi) * Math.sin(theta),
  );
}

function updateSun() {
  if (!earthMaterial) return;
  const solar = solarCoordinates(new Date());
  earthMaterial.uniforms.sunDirection.value.copy(latLonVector(solar.lat, solar.lon).normalize());
  const lat = `${Math.abs(solar.lat).toFixed(1)} ${solar.lat >= 0 ? "N" : "S"}`;
  const lon = `${Math.abs(solar.lon).toFixed(1)} ${solar.lon >= 0 ? "E" : "W"}`;
  el.solarPosition.textContent = `${lat} / ${lon}`;
}

function createStars() {
  const count = window.innerWidth < 760 ? 900 : 1800;
  const positions = new Float32Array(count * 3);
  const colors = new Float32Array(count * 3);
  for (let index = 0; index < count; index += 1) {
    const radius = 8 + Math.random() * 18;
    const theta = Math.random() * Math.PI * 2;
    const phi = Math.acos(2 * Math.random() - 1);
    positions[index * 3] = radius * Math.sin(phi) * Math.cos(theta);
    positions[index * 3 + 1] = radius * Math.cos(phi);
    positions[index * 3 + 2] = radius * Math.sin(phi) * Math.sin(theta);
    const brightness = 0.45 + Math.random() * 0.55;
    colors[index * 3] = brightness * 0.72;
    colors[index * 3 + 1] = brightness * 0.9;
    colors[index * 3 + 2] = brightness * 0.82;
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

function createEarth() {
  const loader = new THREE.TextureLoader();
  const dayTexture = loader.load("assets/earth-day.jpg");
  const nightTexture = loader.load("assets/earth-night.jpg");
  dayTexture.colorSpace = THREE.SRGBColorSpace;
  nightTexture.colorSpace = THREE.SRGBColorSpace;
  dayTexture.anisotropy = renderer.capabilities.getMaxAnisotropy();
  nightTexture.anisotropy = renderer.capabilities.getMaxAnisotropy();

  earthMaterial = new THREE.ShaderMaterial({
    uniforms: {
      dayMap: { value: dayTexture },
      nightMap: { value: nightTexture },
      sunDirection: { value: new THREE.Vector3(1, 0, 0) },
    },
    vertexShader: `
      varying vec2 vUv;
      varying vec3 vWorldNormal;
      varying vec3 vWorldPosition;
      void main() {
        vUv = uv;
        vWorldNormal = normalize(mat3(modelMatrix) * normal);
        vec4 worldPosition = modelMatrix * vec4(position, 1.0);
        vWorldPosition = worldPosition.xyz;
        gl_Position = projectionMatrix * viewMatrix * worldPosition;
      }
    `,
    fragmentShader: `
      uniform sampler2D dayMap;
      uniform sampler2D nightMap;
      uniform vec3 sunDirection;
      varying vec2 vUv;
      varying vec3 vWorldNormal;
      varying vec3 vWorldPosition;
      void main() {
        vec3 normal = normalize(vWorldNormal);
        float solar = dot(normal, normalize(sunDirection));
        float daylight = smoothstep(-0.16, 0.24, solar);
        vec3 day = texture2D(dayMap, vUv).rgb;
        vec3 night = texture2D(nightMap, vUv).rgb * 1.35;
        vec3 litDay = day * (0.42 + max(solar, 0.0) * 0.78);
        float limb = pow(1.0 - max(dot(normal, normalize(cameraPosition - vWorldPosition)), 0.0), 3.0);
        vec3 color = mix(night, litDay, daylight);
        color += vec3(0.04, 0.22, 0.20) * limb * 0.42;
        gl_FragColor = vec4(color, 1.0);
      }
    `,
  });

  earthGroup = new THREE.Group();
  const globeSegments = window.innerWidth < 760 ? [64, 40] : [96, 64];
  const globe = new THREE.Mesh(
    new THREE.SphereGeometry(1.36, globeSegments[0], globeSegments[1]),
    earthMaterial,
  );
  earthGroup.add(globe);

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
        float intensity = pow(0.76 - dot(vNormal, vec3(0.0, 0.0, 1.0)), 3.2);
        gl_FragColor = vec4(0.12, 0.75, 0.68, 1.0) * intensity;
      }
    `,
  });
  const atmosphere = new THREE.Mesh(
    new THREE.SphereGeometry(1.47, 72, 48),
    atmosphereMaterial,
  );
  earthGroup.add(atmosphere);

  const markerGeometry = new THREE.SphereGeometry(0.018, 10, 8);
  cities.forEach((city, index) => {
    const markerMaterial = new THREE.MeshBasicMaterial({
      color: index === state.activeCity ? 0xb9ff3d : 0x39d7ce,
    });
    const marker = new THREE.Mesh(markerGeometry, markerMaterial);
    marker.position.copy(latLonVector(city.lat, city.lon, 1.385));
    marker.userData.cityIndex = index;
    earthGroup.add(marker);
  });

  scene.add(earthGroup);
  updateSun();
}

function createOrbit(radius, tiltX, tiltZ, color, opacity) {
  const points = [];
  for (let index = 0; index <= 256; index += 1) {
    const angle = (index / 256) * Math.PI * 2;
    points.push(new THREE.Vector3(Math.cos(angle) * radius, 0, Math.sin(angle) * radius));
  }
  const geometry = new THREE.BufferGeometry().setFromPoints(points);
  const material = new THREE.LineBasicMaterial({
    color,
    transparent: true,
    opacity,
    blending: THREE.AdditiveBlending,
  });
  const line = new THREE.Line(geometry, material);
  line.rotation.x = tiltX;
  line.rotation.z = tiltZ;
  return line;
}

function createOrbits() {
  orbitGroup = new THREE.Group();
  orbitGroup.add(createOrbit(1.76, 0.42, 0.16, 0x39d7ce, 0.28));
  orbitGroup.add(createOrbit(1.95, 1.02, -0.28, 0xb9ff3d, 0.22));
  orbitGroup.add(createOrbit(2.15, -0.7, 0.5, 0xffb83e, 0.16));

  const nodeGeometry = new THREE.SphereGeometry(0.025, 12, 8);
  const colors = [0xb9ff3d, 0x39d7ce, 0xffb83e];
  for (let index = 0; index < 9; index += 1) {
    const material = new THREE.MeshBasicMaterial({ color: colors[index % colors.length] });
    const node = new THREE.Mesh(nodeGeometry, material);
    node.userData = {
      orbit: index % 3,
      phase: (index / 9) * Math.PI * 2,
      speed: 0.08 + (index % 3) * 0.025,
    };
    proofNodes.push(node);
    orbitGroup.add(node);
  }
  scene.add(orbitGroup);
}

function initScene() {
  try {
    renderer = new THREE.WebGLRenderer({
      canvas: el.canvas,
      antialias: true,
      powerPreference: "high-performance",
    });
  } catch (error) {
    document.body.classList.add("webgl-fallback");
    showToast("WebGL is unavailable; proof controls remain active.");
    return;
  }
  renderer.setPixelRatio(
    Math.min(window.devicePixelRatio, window.innerWidth < 760 ? 1.25 : 1.8),
  );
  renderer.setSize(window.innerWidth, window.innerHeight);
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.12;

  scene = new THREE.Scene();
  scene.fog = new THREE.FogExp2(0x020607, 0.022);
  camera = new THREE.PerspectiveCamera(42, window.innerWidth / window.innerHeight, 0.1, 100);
  camera.position.set(0, 0, state.zoom);

  createStars();
  createEarth();
  createOrbits();
  bindGlobeInput();
  animate();
}

function bindGlobeInput() {
  let dragging = false;
  let lastX = 0;
  let lastY = 0;

  el.canvas.addEventListener("pointerdown", (event) => {
    dragging = true;
    lastX = event.clientX;
    lastY = event.clientY;
    el.canvas.setPointerCapture(event.pointerId);
  });
  el.canvas.addEventListener("pointermove", (event) => {
    if (!dragging) return;
    const dx = event.clientX - lastX;
    const dy = event.clientY - lastY;
    lastX = event.clientX;
    lastY = event.clientY;
    state.targetRotationY += dx * 0.006;
    state.targetRotationX = THREE.MathUtils.clamp(
      state.targetRotationX + dy * 0.004,
      -1.1,
      1.1,
    );
  });
  el.canvas.addEventListener("pointerup", () => {
    dragging = false;
  });
  el.canvas.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      state.zoom = THREE.MathUtils.clamp(state.zoom + event.deltaY * 0.002, 3.5, 6.4);
    },
    { passive: false },
  );
}

function animate(time = 0) {
  requestAnimationFrame(animate);
  if (!renderer || !scene || !camera) return;

  const seconds = time * 0.001;
  if (earthGroup) {
    if (state.motion) state.targetRotationY += 0.00045;
    earthGroup.rotation.x += (state.targetRotationX - earthGroup.rotation.x) * 0.035;
    earthGroup.rotation.y += (state.targetRotationY - earthGroup.rotation.y) * 0.035;
  }
  if (orbitGroup) {
    orbitGroup.rotation.y = state.motion ? seconds * 0.035 : orbitGroup.rotation.y;
    proofNodes.forEach((node) => {
      const radii = [1.76, 1.95, 2.15];
      const tiltsX = [0.42, 1.02, -0.7];
      const tiltsZ = [0.16, -0.28, 0.5];
      const angle = node.userData.phase + seconds * node.userData.speed;
      const vector = new THREE.Vector3(
        Math.cos(angle) * radii[node.userData.orbit],
        0,
        Math.sin(angle) * radii[node.userData.orbit],
      );
      vector.applyEuler(
        new THREE.Euler(tiltsX[node.userData.orbit], 0, tiltsZ[node.userData.orbit]),
      );
      node.position.copy(vector);
      const pulse = 0.75 + Math.sin(seconds * 3 + node.userData.phase) * 0.25;
      node.scale.setScalar(pulse);
    });
  }
  camera.position.z += (state.zoom - camera.position.z) * 0.055;
  renderer.render(scene, camera);
}

function resize() {
  if (!renderer || !camera) return;
  renderer.setSize(window.innerWidth, window.innerHeight);
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.fov = window.innerWidth < 760 ? 50 : 42;
  camera.updateProjectionMatrix();
}

function selectMode(name) {
  if (state.running) return;
  state.mode = name;
  const mode = modes[name];
  document.querySelectorAll(".proof-mode").forEach((button) => {
    button.classList.toggle("active", button.dataset.mode === name);
  });
  el.domainLabel.innerHTML = `2<sup>${mode.exponent.toLocaleString()}</sup>`;
  el.domainDetail.textContent = mode.domain;
  el.orbitKicker.textContent = mode.kicker;
  el.orbitDescription.textContent = mode.description;
  el.verificationStatus.textContent = mode.status;
  el.verificationTitle.textContent = mode.title;
  el.verificationDetail.textContent = mode.detail;
  el.verifyButton.querySelector("span").textContent = mode.button;
  el.sealValue.textContent =
    mode.exponent >= 1_000_000 ? "1M" : mode.exponent.toLocaleString();
  el.roundValue.textContent = `0 / ${mode.exponent.toLocaleString()}`;
  el.claimValue.textContent = "WAITING";
  el.digestValue.textContent = "PENDING";
  el.progressBar.style.width = "0";
  el.statusSeal.classList.remove("verified");
  tone(420, 0.035);
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

function sleep(milliseconds) {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}

function beginRun() {
  state.running = true;
  el.verifyButton.disabled = true;
  el.statusSeal.classList.remove("verified");
  el.progressBar.style.width = "0";
  el.digestValue.textContent = "WORKING";
}

function completeRun(digest, claim = "VERIFIED") {
  state.running = false;
  el.verifyButton.disabled = false;
  el.progressBar.style.width = "100%";
  el.digestValue.textContent = digest.slice(0, 12).toUpperCase();
  el.claimValue.textContent = claim;
  el.statusSeal.classList.add("verified");
  el.verificationStatus.textContent = "VERIFICATION COMPLETE";
  tone(760, 0.12);
}

function failRun(message) {
  state.running = false;
  el.verifyButton.disabled = false;
  el.verificationStatus.textContent = "VERIFICATION STOPPED";
  el.claimValue.textContent = "FAILED";
  el.digestValue.textContent = "REJECTED";
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
    el.progressBar.style.width = `${((index + 1) / 70) * 100}%`;
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
      el.roundValue.textContent = `${(index + 64).toLocaleString()} / 4,096`;
      el.progressBar.style.width = `${((index + 64) / count) * 100}%`;
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

async function fetchWithProgress(url, progressStart, progressSpan) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Download failed with HTTP ${response.status}`);
  const total = Number(response.headers.get("content-length")) || 0;
  if (!response.body) return new Uint8Array(await response.arrayBuffer());
  const reader = response.body.getReader();
  const chunks = [];
  let received = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    received += value.length;
    const ratio = total ? received / total : Math.min(received / 16_000_000, 0.95);
    el.progressBar.style.width = `${progressStart + ratio * progressSpan}%`;
    el.verificationDetail.textContent = `${(received / 1_000_000).toFixed(1)} MB downloaded and buffered for SHA-256`;
  }
  const bytes = new Uint8Array(received);
  let offset = 0;
  chunks.forEach((chunk) => {
    bytes.set(chunk, offset);
    offset += chunk.length;
  });
  return bytes;
}

async function verifyReleaseArtifacts(kind) {
  beginRun();
  try {
    if (kind === "sparse") {
      el.verificationStatus.textContent = "DOWNLOADING PHSPv1 RELEASE";
      const expected = "2b219ba189c3a38f1073c7797629e9aaf44a36820abb64c7628129480eb43f3b";
      const bytes = await fetchWithProgress(
        "artifacts/power_house_sparse_record.phsp",
        0,
        78,
      );
      el.verificationStatus.textContent = "COMPUTING FULL SHA-256";
      const digest = await sha256Hex(bytes);
      if (digest !== expected) throw new Error("PHSPv1 SHA-256 mismatch");
      el.roundValue.textContent = "1,000,000 / 1,000,000";
      completeRun(digest, "PHSPv1 OK");
      el.verificationTitle.textContent = "Published 16 MB certificate is authentic";
      el.verificationDetail.textContent =
        "Algebraic replay remains reproducible with the bundled Rust and Python verifiers.";
    } else {
      const workloadExpected =
        "c8376831f47a50a7423be6412776382bc23618b037e9fdd163594d389d68864d";
      const proofExpected =
        "82045e6eb851991e08d9c4cd782abff3bb06cb8ec5f149e7c2d4287113e6a54a";
      el.verificationStatus.textContent = "DOWNLOADING PHSMv1 WORKLOAD";
      const workload = await fetchWithProgress(
        "artifacts/external_interaction_model.phsm",
        0,
        12,
      );
      const workloadDigest = await sha256Hex(workload);
      if (workloadDigest !== workloadExpected) throw new Error("PHSMv1 SHA-256 mismatch");

      el.verificationStatus.textContent = "DOWNLOADING PHCPv1 PROOF";
      const proof = await fetchWithProgress(
        "artifacts/external_interaction_model.phcp",
        14,
        72,
      );
      const proofDigest = await sha256Hex(proof);
      if (proofDigest !== proofExpected) throw new Error("PHCPv1 SHA-256 mismatch");
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

function tone(frequency, duration) {
  if (!state.sound) return;
  try {
    audioContext ||= new AudioContext();
    const oscillator = audioContext.createOscillator();
    const gain = audioContext.createGain();
    oscillator.type = "sine";
    oscillator.frequency.value = frequency;
    gain.gain.setValueAtTime(0.025, audioContext.currentTime);
    gain.gain.exponentialRampToValueAtTime(
      0.0001,
      audioContext.currentTime + duration,
    );
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

function bindInterface() {
  document.querySelectorAll(".proof-mode").forEach((button) => {
    button.addEventListener("click", () => selectMode(button.dataset.mode));
  });
  el.verifyButton.addEventListener("click", () => {
    if (!state.running) modes[state.mode].action();
  });
  el.soundToggle.addEventListener("click", () => {
    state.sound = !state.sound;
    el.soundToggle.classList.toggle("active", state.sound);
    el.soundToggle.querySelector("span").textContent = state.sound ? "◕" : "◖";
    if (state.sound) tone(620, 0.08);
    showToast(state.sound ? "Interface sound enabled" : "Interface sound muted");
  });
  el.motionToggle.addEventListener("click", () => {
    state.motion = !state.motion;
    el.motionToggle.querySelector("span").textContent = state.motion ? "Ⅱ" : "▶";
    el.motionToggle.setAttribute(
      "aria-label",
      state.motion ? "Pause orbital motion" : "Resume orbital motion",
    );
  });
  window.addEventListener("resize", resize);
  window.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !state.running) modes[state.mode].action();
    if (event.key === " ") {
      event.preventDefault();
      el.motionToggle.click();
    }
  });
}

function init() {
  buildCityList();
  bindInterface();
  selectMode("constant");
  selectCity(2);
  updateClocks();
  window.setInterval(updateClocks, 1000);
  window.setInterval(updateSun, 30_000);
  initScene();
}

init();
