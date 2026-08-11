"use strict";

const assert = require("node:assert/strict");
require("../phone-study.js");
const { PhoneStudyError, percentile, validateTargetDeclarationFields } = globalThis.CKODMKPhoneStudy;

assert.equal(percentile([4, 1, 3, 2], 1, 2), 2);
assert.equal(percentile([4, 1, 3, 2], 95, 100), 4);
assert.equal(percentile([7], 95, 100), 7);
assert.throws(() => percentile([], 1, 2), PhoneStudyError);
assert.throws(() => percentile([0], 1, 2), PhoneStudyError);
assert.throws(() => percentile([1.5], 1, 2), PhoneStudyError);
assert.throws(() => percentile([1], 0, 100), PhoneStudyError);
assert.throws(() => percentile([1], 101, 100), PhoneStudyError);

assert.equal(validateTargetDeclarationFields({ include: false }), null);
assert.deepEqual(validateTargetDeclarationFields({
  include: true,
  target_model: "Pixel 9",
  operating_system: "Android 16",
  browser: "Chrome 140",
  evaluator_relationship: "independent-external"
}), {
  basis: "evaluator-entered; not automatically detected or attested",
  target_model: "Pixel 9",
  operating_system: "Android 16",
  browser: "Chrome 140",
  evaluator_relationship: "independent-external",
  consent: "include in this local report only"
});
assert.throws(() => validateTargetDeclarationFields({ include: true, target_model: "x", operating_system: "Android 16", browser: "Chrome 140", evaluator_relationship: "independent-external" }), PhoneStudyError);
assert.throws(() => validateTargetDeclarationFields({ include: true, target_model: "Pixel 9", operating_system: "Android 16", browser: "Chrome 140", evaluator_relationship: "unknown" }), PhoneStudyError);

console.log("CKODMK_PHONE_STUDY_UNIT_OK");
