import assert from "node:assert/strict";
import { StreamBuffer } from "../publicpower/stream-buffer.js";

const expectedLength = 16_000_171;
const compressedContentLength = 5_456_126;
const stream = new StreamBuffer(expectedLength);

let offset = 0;
while (offset < expectedLength) {
  const size = Math.min(67_109 + (offset % 8191), expectedLength - offset);
  const chunk = new Uint8Array(size);
  chunk.fill((offset / 1024) & 0xff);
  stream.append(chunk);
  offset += size;
}

const bytes = stream.finish();
assert.equal(bytes.length, expectedLength);
assert.ok(bytes.length > compressedContentLength);

const mismatch = new StreamBuffer(32);
mismatch.append(new Uint8Array(31));
assert.throws(() => mismatch.finish(), /Artifact length mismatch/);

console.log("observatory stream buffer: ok");
