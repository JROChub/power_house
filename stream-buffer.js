export class StreamBuffer {
  constructor(expectedLength = 0) {
    this.expectedLength = expectedLength;
    this.length = 0;
    this.bytes = new Uint8Array(Math.max(expectedLength, 64 * 1024));
  }

  append(chunk) {
    const required = this.length + chunk.length;
    if (required > this.bytes.length) {
      const capacity = Math.max(required, Math.ceil(this.bytes.length * 1.5));
      const expanded = new Uint8Array(capacity);
      expanded.set(this.bytes.subarray(0, this.length));
      this.bytes = expanded;
    }
    this.bytes.set(chunk, this.length);
    this.length = required;
  }

  finish() {
    if (this.expectedLength && this.length !== this.expectedLength) {
      throw new Error(
        `Artifact length mismatch: expected ${this.expectedLength}, received ${this.length}`,
      );
    }
    return this.bytes.subarray(0, this.length);
  }
}
