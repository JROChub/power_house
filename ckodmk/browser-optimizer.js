"use strict";

(function initBrowserOptimizer(global) {
  const MAX_MODEL_BYTES = 64 * 1024 * 1024;
  const MAX_FIELDS = 100000;
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const encoder = new TextEncoder();

  class OptimizerError extends Error {}

  function concat(parts) {
    const size = parts.reduce((total, part) => total + part.length, 0);
    if (size < 1 || size > MAX_MODEL_BYTES) throw new OptimizerError("generated model exceeds the browser size limit");
    const output = new Uint8Array(size);
    let offset = 0;
    for (const part of parts) { output.set(part, offset); offset += part.length; }
    return output;
  }

  function varint(value) {
    let current = BigInt(value);
    if (current < 0n) throw new OptimizerError("negative protobuf integer is unsupported");
    const bytes = [];
    do {
      let byte = Number(current & 0x7fn);
      current >>= 7n;
      if (current) byte |= 0x80;
      bytes.push(byte);
    } while (current);
    return Uint8Array.from(bytes);
  }

  function readVarint(bytes, start) {
    let value = 0n;
    let shift = 0n;
    let offset = start;
    for (let count = 0; count < 10; count += 1) {
      if (offset >= bytes.length) throw new OptimizerError("truncated protobuf varint");
      const byte = bytes[offset++];
      value |= BigInt(byte & 0x7f) << shift;
      if (!(byte & 0x80)) {
        if (count > 0 && byte === 0) throw new OptimizerError("protobuf varint is not minimally encoded");
        return { value, offset };
      }
      shift += 7n;
    }
    throw new OptimizerError("protobuf varint exceeds 10 bytes");
  }

  function safeNumber(value, label) {
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) throw new OptimizerError(`${label} exceeds the safe integer limit`);
    return Number(value);
  }

  function fields(bytes) {
    const result = [];
    let offset = 0;
    while (offset < bytes.length) {
      if (result.length >= MAX_FIELDS) throw new OptimizerError("protobuf field count exceeds limit");
      const start = offset;
      const tag = readVarint(bytes, offset);
      offset = tag.offset;
      const number = safeNumber(tag.value >> 3n, "protobuf field number");
      const wire = Number(tag.value & 7n);
      if (number < 1) throw new OptimizerError("invalid protobuf field number");
      let payload;
      let value;
      if (wire === 0) {
        const item = readVarint(bytes, offset); value = item.value; offset = item.offset;
      } else if (wire === 1) {
        if (offset + 8 > bytes.length) throw new OptimizerError("truncated protobuf fixed64");
        payload = bytes.subarray(offset, offset + 8); offset += 8;
      } else if (wire === 2) {
        const length = readVarint(bytes, offset); offset = length.offset;
        const size = safeNumber(length.value, "protobuf field length");
        if (offset + size > bytes.length) throw new OptimizerError("truncated protobuf message");
        payload = bytes.subarray(offset, offset + size); offset += size;
      } else if (wire === 5) {
        if (offset + 4 > bytes.length) throw new OptimizerError("truncated protobuf fixed32");
        payload = bytes.subarray(offset, offset + 4); offset += 4;
      } else {
        throw new OptimizerError(`unsupported protobuf wire type ${wire}`);
      }
      result.push({ number, wire, payload, value, raw: bytes.subarray(start, offset) });
    }
    return result;
  }

  function field(number, wire, payload) {
    const tag = varint(BigInt(number * 8 + wire));
    if (wire === 0) return concat([tag, varint(payload)]);
    if (wire === 2) return concat([tag, varint(payload.length), payload]);
    throw new OptimizerError("encoder supports only varint and message fields");
  }

  function textField(number, value) { return field(number, 2, encoder.encode(value)); }
  function messageField(number, value) { return field(number, 2, value); }

  function repeatedText(items, number) {
    const values = items.filter((item) => item.number === number && item.wire === 2).map((item) => decoder.decode(item.payload));
    if (items.some((item) => item.number === number && item.wire !== 2)) throw new OptimizerError("protobuf string field has the wrong wire type");
    return values;
  }

  function oneText(items, number, label) {
    const values = repeatedText(items, number);
    if (values.length !== 1) throw new OptimizerError(`${label} must occur exactly once`);
    return values[0];
  }

  function oneVarint(items, number, label) {
    const values = items.filter((item) => item.number === number);
    if (values.length !== 1 || values[0].wire !== 0) throw new OptimizerError(`${label} must be one protobuf integer`);
    return safeNumber(values[0].value, label);
  }

  function packedIntegers(payload, label) {
    const values = [];
    let offset = 0;
    while (offset < payload.length) {
      const item = readVarint(payload, offset); values.push(safeNumber(item.value, label)); offset = item.offset;
    }
    return values;
  }

  function parseTensor(message) {
    const items = fields(message);
    const allowed = new Set([1, 2, 8, 9]);
    if (items.some((item) => !allowed.has(item.number))) throw new OptimizerError("source tensor uses an unsupported representation");
    const dimsItems = items.filter((item) => item.number === 1);
    let dims = [];
    for (const item of dimsItems) {
      if (item.wire === 0) dims.push(safeNumber(item.value, "tensor dimension"));
      else if (item.wire === 2) dims.push(...packedIntegers(item.payload, "tensor dimension"));
      else throw new OptimizerError("tensor dimensions have the wrong wire type");
    }
    if (!dims.length || dims.some((value) => value < 1 || value > 65536)) throw new OptimizerError("tensor dimensions are outside the supported profile");
    const name = oneText(items, 8, "tensor name");
    const dataType = oneVarint(items, 2, "tensor data type");
    const rawItems = items.filter((item) => item.number === 9);
    if (rawItems.length !== 1 || rawItems[0].wire !== 2) throw new OptimizerError("tensor must use one raw_data field");
    const elements = dims.reduce((total, value) => total * value, 1);
    if (!Number.isSafeInteger(elements) || elements > 16000000) throw new OptimizerError("tensor element count exceeds the supported profile");
    const view = new DataView(rawItems[0].payload.buffer, rawItems[0].payload.byteOffset, rawItems[0].payload.byteLength);
    if (dataType === 1) {
      if (rawItems[0].payload.length !== elements * 4) throw new OptimizerError("Float32 tensor raw data has the wrong length");
      const data = new Float32Array(elements);
      for (let index = 0; index < elements; index += 1) {
        const value = view.getFloat32(index * 4, true);
        if (!Number.isFinite(value)) throw new OptimizerError("tensor contains a nonfinite coefficient");
        data[index] = value;
      }
      return { name, dims, dataType, data };
    }
    if (dataType === 7 && elements <= 16) {
      if (rawItems[0].payload.length !== elements * 8) throw new OptimizerError("Int64 tensor raw data has the wrong length");
      const data = [];
      for (let index = 0; index < elements; index += 1) {
        const value = view.getBigInt64(index * 8, true);
        if (value < BigInt(Number.MIN_SAFE_INTEGER) || value > BigInt(Number.MAX_SAFE_INTEGER)) throw new OptimizerError("Int64 tensor exceeds the safe integer range");
        data.push(Number(value));
      }
      return { name, dims, dataType, data };
    }
    throw new OptimizerError("source tensor is outside the supported Float32 or bounded Int64 profile");
  }

  function parseNode(message) {
    const items = fields(message);
    const allowed = new Set([1, 2, 3, 4, 6, 7]);
    if (items.some((item) => !allowed.has(item.number))) throw new OptimizerError("source node uses attributes or metadata outside the supported profile");
    const inputs = repeatedText(items, 1);
    const outputs = repeatedText(items, 2);
    const opType = oneText(items, 4, "node operation");
    const domains = repeatedText(items, 7);
    if (domains.some((domain) => domain !== "")) throw new OptimizerError("source node uses a nondefault ONNX domain");
    if (outputs.length !== 1 || !inputs.length || inputs.some((name) => !name) || !outputs[0]) throw new OptimizerError("source node has an unsupported input or output list");
    return { opType, inputs, output: outputs[0] };
  }

  function validateMlpTopology(nodes, tensors) {
    const operations = ["MatMul", "Add", "Relu", "MatMul", "Add"];
    if (nodes.length !== operations.length || nodes.some((node, index) => node.opType !== operations[index])) throw new OptimizerError("source graph is not the supported two layer MLP profile");
    const [mm1, add1, relu, mm2, add2] = nodes;
    if (mm1.inputs.length !== 2 || add1.inputs.length !== 2 || relu.inputs.length !== 1 || mm2.inputs.length !== 2 || add2.inputs.length !== 2) throw new OptimizerError("source graph arity is outside the supported profile");
    if (add1.inputs[0] !== mm1.output || relu.inputs[0] !== add1.output || mm2.inputs[0] !== relu.output || add2.inputs[0] !== mm2.output) throw new OptimizerError("source graph connections are outside the supported profile");
    const names = [mm1.inputs[1], add1.inputs[1], mm2.inputs[1], add2.inputs[1]];
    if (new Set(names).size !== 4 || tensors.size !== 4 || names.some((name) => !tensors.has(name))) throw new OptimizerError("source graph must contain exactly four referenced initializers");
    const [w1, b1, w2, b2] = names.map((name) => tensors.get(name));
    if (w1.dims.length !== 2 || b1.dims.length !== 1 || w2.dims.length !== 2 || b2.dims.length !== 1 || w1.dims[1] !== b1.dims[0] || w2.dims[0] !== b1.dims[0] || w2.dims[1] !== b2.dims[0]) throw new OptimizerError("source MLP tensor shapes do not agree");
    if (w1.dims[0] > 4096 || w1.dims[1] > 4096 || w2.dims[1] > 4096) throw new OptimizerError("source MLP dimensions exceed the browser optimizer limits");
    if ([w1, b1, w2, b2].some((tensor) => tensor.dataType !== 1)) throw new OptimizerError("source MLP coefficients must be Float32");
    return { kind: "two-layer-mlp-per-channel-dynamic-int8/v1", mm1, add1, relu, mm2, add2, w1, b1, w2, b2 };
  }

  function validateRbfTopology(nodes, tensors) {
    const operations = ["Unsqueeze", "Sub", "Mul", "ReduceSum", "Squeeze", "Mul", "Exp", "MatMul", "Add"];
    if (nodes.length !== operations.length || nodes.some((node, index) => node.opType !== operations[index])) throw new OptimizerError("source graph is not a supported RBF or two layer MLP profile");
    const [unsqueeze, subtract, square, reduce, squeeze, gammaMul, exponential, matmul, add] = nodes;
    const arities = [2, 2, 2, 2, 2, 2, 1, 2, 2];
    if (nodes.some((node, index) => node.inputs.length !== arities[index])) throw new OptimizerError("source RBF graph arity is outside the supported profile");
    if (
      subtract.inputs[0] !== unsqueeze.output || square.inputs[0] !== subtract.output || square.inputs[1] !== subtract.output ||
      reduce.inputs[0] !== square.output || squeeze.inputs[0] !== reduce.output || gammaMul.inputs[0] !== squeeze.output ||
      exponential.inputs[0] !== gammaMul.output || matmul.inputs[0] !== exponential.output || add.inputs[0] !== matmul.output
    ) throw new OptimizerError("source RBF graph connections are outside the supported profile");
    const names = [unsqueeze.inputs[1], subtract.inputs[1], reduce.inputs[1], squeeze.inputs[1], gammaMul.inputs[1], matmul.inputs[1], add.inputs[1]];
    const referenced = new Set(names);
    if (referenced.size !== 6 || tensors.size !== 6 || [...referenced].some((name) => !tensors.has(name))) throw new OptimizerError("source RBF graph must contain exactly six referenced initializers");
    const axisOne = tensors.get(unsqueeze.inputs[1]);
    const centers = tensors.get(subtract.inputs[1]);
    const axisTwo = tensors.get(reduce.inputs[1]);
    const squeezeAxis = tensors.get(squeeze.inputs[1]);
    const gamma = tensors.get(gammaMul.inputs[1]);
    const weight = tensors.get(matmul.inputs[1]);
    const bias = tensors.get(add.inputs[1]);
    if (axisTwo !== squeezeAxis || axisOne.dataType !== 7 || axisTwo.dataType !== 7 || axisOne.dims.length !== 1 || axisTwo.dims.length !== 1 || axisOne.data.length !== 1 || axisTwo.data.length !== 1 || axisOne.data[0] !== 1 || axisTwo.data[0] !== 2) throw new OptimizerError("source RBF axes are outside the supported profile");
    if ([centers, gamma, weight, bias].some((tensor) => tensor.dataType !== 1)) throw new OptimizerError("source RBF coefficients must be Float32");
    if (centers.dims.length !== 2 || weight.dims.length !== 2 || bias.dims.length !== 1 || gamma.dims.length !== 1 || gamma.dims[0] !== 1 || centers.dims[0] !== weight.dims[0] || weight.dims[1] !== bias.dims[0]) throw new OptimizerError("source RBF tensor shapes do not agree");
    if (centers.dims[0] < 2 || centers.dims[0] > 4096 || centers.dims[1] < 2 || centers.dims[1] > 4096 || weight.dims[1] < 2 || weight.dims[1] > 4096 || !(gamma.data[0] < 0)) throw new OptimizerError("source RBF dimensions or gamma are outside the supported profile");
    if (centers.data.some((value) => value < 0 || value > 1)) throw new OptimizerError("source RBF centers must be normalized to [0,1]");
    return { kind: "normalized-rbf-per-channel-dynamic-int8/v1", unsqueeze, subtract, square, reduce, squeeze, gammaMul, exponential, matmul, add, axisOne, axisTwo, centers, gamma, weight, bias };
  }

  function validateTopology(nodes, tensors) {
    if (nodes.length === 5 && nodes[0]?.opType === "MatMul") return validateMlpTopology(nodes, tensors);
    return validateRbfTopology(nodes, tensors);
  }

  function roundEven(value) {
    const floor = Math.floor(value);
    const fraction = value - floor;
    if (fraction < 0.5) return floor;
    if (fraction > 0.5) return floor + 1;
    return floor % 2 === 0 ? floor : floor + 1;
  }

  function quantizeColumns(tensor) {
    const [rows, columns] = tensor.dims;
    const scales = new Float32Array(columns);
    const values = new Int8Array(tensor.data.length);
    for (let column = 0; column < columns; column += 1) {
      let maximum = 0;
      for (let row = 0; row < rows; row += 1) maximum = Math.max(maximum, Math.abs(tensor.data[row * columns + column]));
      if (!(maximum > 0)) throw new OptimizerError(`weight ${tensor.name} contains an all zero output column`);
      const scale = Math.fround(maximum / 127);
      if (!(scale > 0) || !Number.isFinite(scale)) throw new OptimizerError(`weight ${tensor.name} has an invalid quantization scale`);
      scales[column] = scale;
      for (let row = 0; row < rows; row += 1) {
        const index = row * columns + column;
        const rounded = roundEven(Math.fround(tensor.data[index] / scale));
        values[index] = Math.max(-127, Math.min(127, rounded));
      }
    }
    return { scales, values };
  }

  function quantizeNormalizedColumns(tensor) {
    const [rows, columns] = tensor.dims;
    const scales = new Float32Array(columns);
    const zeros = new Int8Array(columns);
    const values = new Int8Array(tensor.data.length);
    for (let column = 0; column < columns; column += 1) {
      let maximum = 0;
      for (let row = 0; row < rows; row += 1) maximum = Math.max(maximum, tensor.data[row * columns + column]);
      const scale = Math.fround((maximum || 1) / 255);
      if (!(scale > 0) || !Number.isFinite(scale)) throw new OptimizerError(`center ${tensor.name} has an invalid quantization scale`);
      scales[column] = scale;
      zeros[column] = -128;
      for (let row = 0; row < rows; row += 1) {
        const index = row * columns + column;
        values[index] = Math.max(-128, Math.min(127, roundEven(Math.fround(tensor.data[index] / scale)) - 128));
      }
    }
    return { scales, zeros, values };
  }

  function rawFloat32(values) {
    const output = new Uint8Array(values.length * 4);
    const view = new DataView(output.buffer);
    for (let index = 0; index < values.length; index += 1) view.setFloat32(index * 4, values[index], true);
    return output;
  }

  function rawInt64(values) {
    const output = new Uint8Array(values.length * 8);
    const view = new DataView(output.buffer);
    for (let index = 0; index < values.length; index += 1) view.setBigInt64(index * 8, BigInt(values[index]), true);
    return output;
  }

  function packed(values) { return concat(values.map((value) => varint(value))); }

  function tensorMessage(name, dataType, dims, raw) {
    return concat([field(1, 2, packed(dims)), field(2, 0, dataType), textField(8, name), field(9, 2, raw)]);
  }

  function nodeMessage(opType, inputs, outputs, attributes = []) {
    return concat([
      ...inputs.map((name) => textField(1, name)),
      ...outputs.map((name) => textField(2, name)),
      textField(4, opType),
      ...attributes.map((attribute) => messageField(5, attribute))
    ]);
  }

  function castToFloatAttribute() { return concat([textField(1, "to"), field(3, 0, 1), field(20, 0, 2)]); }

  function replacementGraph(graphBytes) {
    const graphFields = fields(graphBytes);
    const allowedGraphFields = new Set([1, 2, 5, 10, 11, 12, 13]);
    if (graphFields.some((item) => !allowedGraphFields.has(item.number))) throw new OptimizerError("source graph contains metadata outside the supported profile");
    const nodeFields = graphFields.filter((item) => item.number === 1);
    const initializerFields = graphFields.filter((item) => item.number === 5);
    if (!nodeFields.length || !initializerFields.length || nodeFields.some((item) => item.wire !== 2) || initializerFields.some((item) => item.wire !== 2)) throw new OptimizerError("source graph is missing nodes or initializers");
    const nodes = nodeFields.map((item) => parseNode(item.payload));
    const tensors = new Map(initializerFields.map((item) => { const tensor = parseTensor(item.payload); return [tensor.name, tensor]; }));
    if (tensors.size !== initializerFields.length) throw new OptimizerError("source graph contains duplicate initializer names");
    const profile = validateTopology(nodes, tensors);
    const graphInputs = graphFields.filter((item) => item.number === 11);
    const graphOutputs = graphFields.filter((item) => item.number === 12);
    if (graphInputs.length !== 1 || graphOutputs.length !== 1 || graphInputs[0].wire !== 2 || graphOutputs[0].wire !== 2) throw new OptimizerError("source graph must expose exactly one input and one output");
    const graphInput = oneText(fields(graphInputs[0].payload), 1, "graph input name");
    const graphOutput = oneText(fields(graphOutputs[0].payload), 1, "graph output name");
    let generatedNodes;
    let generatedTensors;
    if (profile.kind === "two-layer-mlp-per-channel-dynamic-int8/v1") {
      if (graphInput !== profile.mm1.inputs[0] || graphOutput !== profile.add2.output) throw new OptimizerError("source graph interface does not match its MLP path");
      const q1 = quantizeColumns(profile.w1);
      const q2 = quantizeColumns(profile.w2);
      const n1 = profile.w1.name;
      const n2 = profile.w2.name;
      const q1Name = `${n1}_ckodmk_int8`;
      const q2Name = `${n2}_ckodmk_int8`;
      const s1Name = `${n1}_ckodmk_scale`;
      const s2Name = `${n2}_ckodmk_scale`;
      const z1Name = `${n1}_ckodmk_zero`;
      const z2Name = `${n2}_ckodmk_zero`;
      const hiddenName = profile.relu.output;
      generatedNodes = [
        nodeMessage("DynamicQuantizeLinear", [graphInput], [`${graphInput}_ckodmk_q`, `${graphInput}_ckodmk_scale`, `${graphInput}_ckodmk_zero`]),
        nodeMessage("Mul", [`${graphInput}_ckodmk_scale`, s1Name], [`${graphInput}_${n1}_ckodmk_scale_mul`]),
        nodeMessage("MatMulInteger", [`${graphInput}_ckodmk_q`, q1Name, `${graphInput}_ckodmk_zero`, z1Name], [`${profile.mm1.output}_ckodmk_integer`]),
        nodeMessage("Cast", [`${profile.mm1.output}_ckodmk_integer`], [`${profile.mm1.output}_ckodmk_float`], [castToFloatAttribute()]),
        nodeMessage("Mul", [`${profile.mm1.output}_ckodmk_float`, `${graphInput}_${n1}_ckodmk_scale_mul`], [profile.mm1.output]),
        nodeMessage("Add", [profile.mm1.output, profile.b1.name], [profile.add1.output]),
        nodeMessage("Relu", [profile.add1.output], [hiddenName]),
        nodeMessage("DynamicQuantizeLinear", [hiddenName], [`${hiddenName}_ckodmk_q`, `${hiddenName}_ckodmk_scale`, `${hiddenName}_ckodmk_zero`]),
        nodeMessage("Mul", [`${hiddenName}_ckodmk_scale`, s2Name], [`${hiddenName}_${n2}_ckodmk_scale_mul`]),
        nodeMessage("MatMulInteger", [`${hiddenName}_ckodmk_q`, q2Name, `${hiddenName}_ckodmk_zero`, z2Name], [`${profile.mm2.output}_ckodmk_integer`]),
        nodeMessage("Cast", [`${profile.mm2.output}_ckodmk_integer`], [`${profile.mm2.output}_ckodmk_float`], [castToFloatAttribute()]),
        nodeMessage("Mul", [`${profile.mm2.output}_ckodmk_float`, `${hiddenName}_${n2}_ckodmk_scale_mul`], [profile.mm2.output]),
        nodeMessage("Add", [profile.mm2.output, profile.b2.name], [profile.add2.output])
      ];
      generatedTensors = [
        tensorMessage(profile.b1.name, 1, profile.b1.dims, rawFloat32(profile.b1.data)),
        tensorMessage(profile.b2.name, 1, profile.b2.dims, rawFloat32(profile.b2.data)),
        tensorMessage(s1Name, 1, [q1.scales.length], rawFloat32(q1.scales)),
        tensorMessage(z1Name, 3, [q1.scales.length], new Uint8Array(q1.scales.length)),
        tensorMessage(q1Name, 3, profile.w1.dims, new Uint8Array(q1.values.buffer)),
        tensorMessage(s2Name, 1, [q2.scales.length], rawFloat32(q2.scales)),
        tensorMessage(z2Name, 3, [q2.scales.length], new Uint8Array(q2.scales.length)),
        tensorMessage(q2Name, 3, profile.w2.dims, new Uint8Array(q2.values.buffer))
      ];
    } else {
      if (graphInput !== profile.unsqueeze.inputs[0] || graphOutput !== profile.add.output) throw new OptimizerError("source graph interface does not match its RBF path");
      const centers = quantizeNormalizedColumns(profile.centers);
      const head = quantizeColumns(profile.weight);
      const centerQ = `${profile.centers.name}_ckodmk_int8`;
      const centerScale = `${profile.centers.name}_ckodmk_scale`;
      const centerZero = `${profile.centers.name}_ckodmk_zero`;
      const headQ = `${profile.weight.name}_ckodmk_int8`;
      const headScale = `${profile.weight.name}_ckodmk_scale`;
      const headZero = `${profile.weight.name}_ckodmk_zero`;
      const feature = profile.exponential.output;
      generatedNodes = [
        nodeMessage("Unsqueeze", profile.unsqueeze.inputs, [profile.unsqueeze.output]),
        nodeMessage("DequantizeLinear", [centerQ, centerScale, centerZero], [`${profile.centers.name}_ckodmk_float`]),
        nodeMessage("Sub", [profile.unsqueeze.output, `${profile.centers.name}_ckodmk_float`], [profile.subtract.output]),
        nodeMessage("Mul", [profile.subtract.output, profile.subtract.output], [profile.square.output]),
        nodeMessage("ReduceSum", profile.reduce.inputs, [profile.reduce.output]),
        nodeMessage("Squeeze", profile.squeeze.inputs, [profile.squeeze.output]),
        nodeMessage("Mul", [profile.squeeze.output, profile.gamma.name], [profile.gammaMul.output]),
        nodeMessage("Exp", [profile.gammaMul.output], [feature]),
        nodeMessage("DynamicQuantizeLinear", [feature], [`${feature}_ckodmk_q`, `${feature}_ckodmk_scale`, `${feature}_ckodmk_zero`]),
        nodeMessage("Mul", [`${feature}_ckodmk_scale`, headScale], [`${feature}_${profile.weight.name}_ckodmk_scale_mul`]),
        nodeMessage("MatMulInteger", [`${feature}_ckodmk_q`, headQ, `${feature}_ckodmk_zero`, headZero], [`${profile.matmul.output}_ckodmk_integer`]),
        nodeMessage("Cast", [`${profile.matmul.output}_ckodmk_integer`], [`${profile.matmul.output}_ckodmk_float`], [castToFloatAttribute()]),
        nodeMessage("Mul", [`${profile.matmul.output}_ckodmk_float`, `${feature}_${profile.weight.name}_ckodmk_scale_mul`], [profile.matmul.output]),
        nodeMessage("Add", [profile.matmul.output, profile.bias.name], [profile.add.output])
      ];
      generatedTensors = [
        tensorMessage(profile.axisOne.name, 7, profile.axisOne.dims, rawInt64(profile.axisOne.data)),
        tensorMessage(profile.axisTwo.name, 7, profile.axisTwo.dims, rawInt64(profile.axisTwo.data)),
        tensorMessage(profile.gamma.name, 1, profile.gamma.dims, rawFloat32(profile.gamma.data)),
        tensorMessage(profile.bias.name, 1, profile.bias.dims, rawFloat32(profile.bias.data)),
        tensorMessage(centerScale, 1, [centers.scales.length], rawFloat32(centers.scales)),
        tensorMessage(centerZero, 3, [centers.zeros.length], new Uint8Array(centers.zeros.buffer)),
        tensorMessage(centerQ, 3, profile.centers.dims, new Uint8Array(centers.values.buffer)),
        tensorMessage(headScale, 1, [head.scales.length], rawFloat32(head.scales)),
        tensorMessage(headZero, 3, [head.scales.length], new Uint8Array(head.scales.length)),
        tensorMessage(headQ, 3, profile.weight.dims, new Uint8Array(head.values.buffer))
      ];
    }
    const output = [];
    let insertedNodes = false;
    let insertedTensors = false;
    for (const item of graphFields) {
      if (item.number === 1) {
        if (!insertedNodes) { output.push(...generatedNodes.map((node) => messageField(1, node))); insertedNodes = true; }
      } else if (item.number === 5) {
        if (!insertedTensors) { output.push(...generatedTensors.map((tensor) => messageField(5, tensor))); insertedTensors = true; }
      } else {
        output.push(item.raw);
      }
    }
    return { bytes: concat(output), profile: profile.kind };
  }

  function validateOpset(modelFields) {
    const imports = modelFields.filter((item) => item.number === 8);
    if (!imports.length || imports.some((item) => item.wire !== 2)) throw new OptimizerError("model is missing a valid ONNX opset import");
    let defaultVersion = null;
    for (const item of imports) {
      const values = fields(item.payload);
      if (values.some((value) => value.number !== 1 && value.number !== 2)) throw new OptimizerError("opset import contains unsupported metadata");
      const domains = repeatedText(values, 1);
      const domain = domains.length ? domains[0] : "";
      if (domains.length > 1) throw new OptimizerError("opset domain is duplicated");
      if (domain === "") defaultVersion = oneVarint(values, 2, "default opset version");
    }
    if (defaultVersion === null || defaultVersion < 11 || defaultVersion > 21) throw new OptimizerError("default ONNX opset must be between 11 and 21");
  }

  function buildCandidate(source) {
    const bytes = source instanceof Uint8Array ? source : new Uint8Array(source);
    if (bytes.length < 1 || bytes.length > MAX_MODEL_BYTES) throw new OptimizerError("source model is empty or exceeds the browser size limit");
    const modelFields = fields(bytes);
    const allowedModelFields = new Set([1, 2, 3, 4, 5, 6, 7, 8]);
    if (modelFields.some((item) => !allowedModelFields.has(item.number))) throw new OptimizerError("model contains functions, training data, or metadata outside the supported profile");
    for (const singleton of [1, 2, 3, 4, 5, 6, 7]) {
      if (modelFields.filter((item) => item.number === singleton).length > 1) throw new OptimizerError(`model field ${singleton} is duplicated`);
    }
    validateOpset(modelFields);
    const graphs = modelFields.filter((item) => item.number === 7);
    if (graphs.length !== 1 || graphs[0].wire !== 2) throw new OptimizerError("model must contain exactly one graph");
    const generated = replacementGraph(graphs[0].payload);
    const output = [];
    let producerSeen = false;
    let versionSeen = false;
    for (const item of modelFields) {
      if (item.number === 2) {
        if (!producerSeen) { output.push(textField(2, "mfenx-ckodmk-browser")); producerSeen = true; }
      } else if (item.number === 3) {
        if (!versionSeen) { output.push(textField(3, "0.2.0")); versionSeen = true; }
      } else if (item.number === 7) {
        if (!producerSeen) { output.push(textField(2, "mfenx-ckodmk-browser")); producerSeen = true; }
        if (!versionSeen) { output.push(textField(3, "0.2.0")); versionSeen = true; }
        output.push(messageField(7, generated.bytes));
      } else {
        output.push(item.raw);
      }
    }
    return Object.freeze({ bytes: concat(output), profile: generated.profile });
  }

  function buildInt8Candidate(source) { return buildCandidate(source).bytes; }

  const api = Object.freeze({ OptimizerError, buildCandidate, buildInt8Candidate });
  global.CKODMKBrowserOptimizer = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(typeof window !== "undefined" ? window : globalThis);
