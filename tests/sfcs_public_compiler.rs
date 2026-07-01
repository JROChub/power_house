#![cfg(feature = "sfcs")]

use power_house::{
    compile_llvm_ir_source, compile_public_rust_source, compile_wasm_stack_source,
    memory::semantic_packet_digest, SfcsCompilerError,
};
use std::collections::BTreeMap;

#[test]
fn public_rust_compiler_lowers_multi_parameter_expression_to_sfcs() {
    let compiled =
        compile_public_rust_source("pub fn score(a: u32, b: u32, c: u32) -> u32 { (a + b) * c }")
            .unwrap();

    assert_eq!(compiled.schema, "power-house/sfcs-rust-public/v1-draft");
    assert_eq!(compiled.function_name, "score");
    assert_eq!(compiled.parameters, vec!["a", "b", "c"]);
    assert!(compiled.sfcs_source.contains("input a"));
    assert!(compiled.sfcs_source.contains("input b"));
    assert!(compiled.sfcs_source.contains("input c"));
    assert!(compiled
        .sfcs_source
        .contains("output (a + b) * c as return"));
    assert!(compiled.graph_digest().unwrap().starts_with("sha256:"));
    assert_eq!(
        compiled.semantic_packet["packet_digest"].as_str().unwrap(),
        semantic_packet_digest(&compiled.semantic_packet).unwrap()
    );

    let trace = compiled
        .graph
        .execution_trace(&BTreeMap::from([
            ("a".to_string(), 2),
            ("b".to_string(), 3),
            ("c".to_string(), 4),
        ]))
        .unwrap();
    assert_eq!(trace.outputs["return"], 20);
}

#[test]
fn public_rust_compiler_lowers_if_expression() {
    let compiled = compile_public_rust_source(
        "fn absdiff(a: u32, b: u32) -> u32 { if a > b { a - b } else { b - a } }",
    )
    .unwrap();
    assert!(compiled
        .sfcs_source
        .contains("output if a > b then a - b else b - a as return"));

    let greater = compiled
        .graph
        .execution_trace(&BTreeMap::from([
            ("a".to_string(), 9),
            ("b".to_string(), 4),
        ]))
        .unwrap();
    assert_eq!(greater.outputs["return"], 5);

    let lesser = compiled
        .graph
        .execution_trace(&BTreeMap::from([
            ("a".to_string(), 4),
            ("b".to_string(), 9),
        ]))
        .unwrap();
    assert_eq!(lesser.outputs["return"], 5);
}

#[test]
fn public_rust_compiler_rejects_unsupported_source() {
    for bad_source in [
        "fn bad(a: i32) -> u32 { a }",
        "fn bad(a: u32) -> u64 { a }",
        "fn bad(a: u32, a: u32) -> u32 { a }",
        "fn bad(a: u32) -> u32 { while a > 0 { a } }",
    ] {
        assert!(matches!(
            compile_public_rust_source(bad_source),
            Err(SfcsCompilerError::InvalidSource(_) | SfcsCompilerError::Sfcs(_))
        ));
    }
}

#[test]
fn llvm_ir_compiler_lowers_ssa_to_sfcs() {
    let compiled = compile_llvm_ir_source(
        r#"
        define i32 @score(i32 %a, i32 %b, i32 %c) {
        entry:
          %sum = add i32 %a, %b
          %gt = icmp ugt i32 %sum, 10
          %wide = mul i32 %sum, %c
          %small = sub i32 %wide, 1
          %out = select i1 %gt, i32 %wide, i32 %small
          ret i32 %out
        }
        "#,
    )
    .unwrap();

    assert_eq!(compiled.schema, "power-house/sfcs-llvm-ir/v1-draft");
    assert_eq!(compiled.function_name, "score");
    assert_eq!(compiled.parameters, vec!["a", "b", "c"]);
    assert!(compiled.graph_digest().unwrap().starts_with("sha256:"));
    assert_eq!(
        compiled.semantic_packet["packet_digest"].as_str().unwrap(),
        semantic_packet_digest(&compiled.semantic_packet).unwrap()
    );

    let wide = compiled
        .graph
        .execution_trace(&BTreeMap::from([
            ("a".to_string(), 7),
            ("b".to_string(), 5),
            ("c".to_string(), 3),
        ]))
        .unwrap();
    assert_eq!(wide.outputs.values().copied().next().unwrap(), 36);

    let small = compiled
        .graph
        .execution_trace(&BTreeMap::from([
            ("a".to_string(), 2),
            ("b".to_string(), 3),
            ("c".to_string(), 4),
        ]))
        .unwrap();
    assert_eq!(small.outputs.values().copied().next().unwrap(), 19);
}

#[test]
fn llvm_ir_compiler_rejects_unsupported_ir() {
    for bad_source in [
        "define i64 @bad(i32 %a) {\nret i32 %a\n}",
        "define i32 @bad(i32 %a) {\n%p = alloca i32\nret i32 %a\n}",
        "define i32 @bad(i32 %a) {\nbr label %next\n}",
        "define i32 @bad(i32 %a) {\nret i32 %missing\n}",
        "define i32 @bad(i64 %a) {\nret i32 %a\n}",
    ] {
        assert!(matches!(
            compile_llvm_ir_source(bad_source),
            Err(SfcsCompilerError::InvalidSource(_) | SfcsCompilerError::Sfcs(_))
        ));
    }
}

#[test]
fn wasm_stack_compiler_lowers_stack_ops_to_sfcs() {
    let compiled = compile_wasm_stack_source(
        r#"
        param a i32
        param b i32
        local.get a
        local.get b
        i32.add
        i32.const 3
        i32.mul
        return
        "#,
    )
    .unwrap();

    assert_eq!(compiled.schema, "power-house/sfcs-wasm-stack/v1-draft");
    assert_eq!(compiled.parameters, vec!["a", "b"]);
    assert!(compiled.graph_digest().unwrap().starts_with("sha256:"));
    assert_eq!(
        compiled.semantic_packet["packet_digest"].as_str().unwrap(),
        semantic_packet_digest(&compiled.semantic_packet).unwrap()
    );

    let trace = compiled
        .graph
        .execution_trace(&BTreeMap::from([
            ("a".to_string(), 4),
            ("b".to_string(), 8),
        ]))
        .unwrap();
    assert_eq!(trace.outputs.values().copied().next().unwrap(), 36);
}

#[test]
fn wasm_stack_compiler_lowers_select() {
    let compiled = compile_wasm_stack_source(
        r#"
        param cond i32
        param a i32
        param b i32
        local.get cond
        local.get a
        local.get b
        select
        return
        "#,
    )
    .unwrap();

    let selected_true = compiled
        .graph
        .execution_trace(&BTreeMap::from([
            ("cond".to_string(), 1),
            ("a".to_string(), 44),
            ("b".to_string(), 55),
        ]))
        .unwrap();
    assert_eq!(selected_true.outputs.values().copied().next().unwrap(), 44);

    let selected_false = compiled
        .graph
        .execution_trace(&BTreeMap::from([
            ("cond".to_string(), 0),
            ("a".to_string(), 44),
            ("b".to_string(), 55),
        ]))
        .unwrap();
    assert_eq!(selected_false.outputs.values().copied().next().unwrap(), 55);
}

#[test]
fn wasm_stack_compiler_rejects_invalid_stack_programs() {
    for bad_source in [
        "local.get missing\nreturn",
        "param a i32\nlocal.get a\ni32.add\nreturn",
        "param a i64\nlocal.get a\nreturn",
        "param a i32\nlocal.get a\nreturn\ni32.const 1",
    ] {
        assert!(matches!(
            compile_wasm_stack_source(bad_source),
            Err(SfcsCompilerError::InvalidSource(_) | SfcsCompilerError::Sfcs(_))
        ));
    }
}
