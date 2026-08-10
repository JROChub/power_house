# CKODMK retained real-model results

This file is generated from the retained machine-readable summary. Do not edit it by hand.

Manifest SHA-256: `sha256:e0abf8731a4bd9b5ae54fa09734c822e2bfaa94d9263119950c84ce988b0e250`  
Summary SHA-256: `sha256:fab6200e0e75c19d81d9c5f7aaa9f7bf193af12fd689647f11214f0e722c0303`  
Protocol SHA-256: `sha256:77f3dd5096db758cc1b31666dcc1f904048b0ca5da9699a9958a7bbcdb6d56fc`  
Frozen Rust adjudicator SHA-256: `sha256:58c0cd14fc5d5e96f0a4f1e98ff3a225362de34e5c5a261b7c795bac3ec14447`  
Execution environment: private test host, ONNX Runtime CPU 1.28.0, one thread. The detailed device profile is retained only in the private audit package.

Every semantic comparison used Float32, row-major inputs at batch size one over all 1,797 official Optdigits test rows. Latency is a finite 500-run observation, not a population-tail or other-device guarantee.

| Model | Candidate | Source accuracy | Candidate accuracy | Decision changes | Max finite-set Linf | Source/candidate bytes | Size ratio | Source/candidate p95 ms | Python Gate | Rust adjudicator |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|---|
| optdigits-mlp-w16-s1701 | ort-graph | 95.882026% | 95.882026% | 0/1,797 | 0 | 5,253/5,640 | 0.931383x | 0.033230/0.031367 | `PASS` | `PASS` |
| optdigits-mlp-w16-s1701 | int8 | 95.882026% | 95.882026% | 0/1,797 | 0.202297449 | 5,253/3,106 | 1.691243x | 0.032763/0.043051 | `PASS` | `PASS` |
| optdigits-mlp-w32-s2603 | ort-graph | 96.215915% | 96.215915% | 0/1,797 | 0 | 10,055/10,442 | 0.962938x | 0.034000/0.029583 | `PASS` | `PASS` |
| optdigits-mlp-w32-s2603 | int8 | 96.215915% | 96.215915% | 0/1,797 | 0.218335152 | 10,055/4,438 | 2.265660x | 0.035006/0.044239 | `PASS` | `PASS` |
| optdigits-mlp-w64-s3907 | ort-graph | 96.160267% | 96.160267% | 0/1,797 | 0 | 19,658/20,045 | 0.980693x | 0.032923/0.028638 | `PASS` | `PASS` |
| optdigits-mlp-w64-s3907 | int8 | 96.160267% | 96.160267% | 2/1,797 | 0.181334734 | 19,658/7,094 | 2.771074x | 0.036781/0.047777 | `PASS` | `PASS` |

The separately implemented Rust adjudicator rehashed every bound artifact and recomputed metrics, claims, and decisions from exact output-bit and integer-ns observations. It did not independently execute ONNX; that boundary remains explicit in every adjudication. Raw outputs, timings, and the bitwise replay statement remain Python-runner assertions.

The graph optimized candidates may be larger than their sources. Dynamic INT8 is smaller but is slower than both its paired source and the corresponding graph candidate on the recorded private test host. That negative result is retained.

Training recipes, histories, and train accuracies are producer-reported. CKODMK did not replay training or prove that the declared recipe generated the source weights. Source and candidate test outcomes were recomputed by the Python Gate on the bound finite dataset.

These are three team-trained MLPs on one public dataset and one physical CPU. They are not customer models, an independent reproduction, a second architecture family, an accelerator result, or universal equivalence evidence.
