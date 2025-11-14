// Unit tests for the codegen Builder API
// Tests the builder pattern for code generation configuration

#[cfg(feature = "codegen")]
use rpcnet::codegen::Builder;
use std::path::PathBuf;

#[cfg(feature = "codegen")]
#[test]
fn test_builder_new() {
    let _builder = Builder::new();

    // Builder should be created with default values
    // (we can't directly inspect private fields, but we can test behavior)
    // Just verify it compiles and doesn't panic
}

#[cfg(feature = "codegen")]
#[test]
fn test_builder_default() {
    let _builder = Builder::default();

    // Default should work the same as new()
    // Just verify it compiles and doesn't panic
}

#[cfg(feature = "codegen")]
#[test]
fn test_builder_with_input() {
    let _builder = Builder::new().input("test.rpc.rs");

    // Builder should accept input path
    // Just verify method chaining works
}

#[cfg(feature = "codegen")]
#[test]
fn test_builder_with_multiple_inputs() {
    let _builder = Builder::new()
        .input("test1.rpc.rs")
        .input("test2.rpc.rs")
        .input("test3.rpc.rs");

    // Builder should accept multiple input paths
}

#[cfg(feature = "codegen")]
#[test]
fn test_builder_with_output() {
    let _builder = Builder::new().output("target/generated");

    // Builder should accept output path
}

#[cfg(feature = "codegen")]
#[test]
fn test_builder_with_pathbuf() {
    let input_path = PathBuf::from("services/calculator.rpc.rs");
    let output_path = PathBuf::from("target/codegen");

    let _builder = Builder::new().input(input_path).output(output_path);

    // Builder should accept PathBuf types
}

#[cfg(feature = "codegen")]
#[test]
fn test_builder_full_chain() {
    let _builder = Builder::new()
        .input("services/auth.rpc.rs")
        .input("services/users.rpc.rs")
        .output("src/generated")
        .input("services/billing.rpc.rs");

    // All builder methods should be chainable in any order
}

#[cfg(feature = "codegen")]
#[test]
fn test_builder_with_relative_paths() {
    let _builder = Builder::new()
        .input("./rpc/service.rpc.rs")
        .output("./generated");

    // Relative paths should be accepted
}

#[cfg(feature = "codegen")]
#[test]
fn test_builder_with_absolute_paths() {
    let _builder = Builder::new()
        .input("/tmp/test.rpc.rs")
        .output("/tmp/generated");

    // Absolute paths should be accepted
}

#[cfg(feature = "codegen")]
#[test]
fn test_builder_with_nested_paths() {
    let _builder = Builder::new()
        .input("services/v1/api/users.rpc.rs")
        .output("generated/services/v1/api");

    // Nested directory paths should be accepted
}

#[cfg(feature = "codegen")]
#[test]
fn test_builder_output_only() {
    let _builder = Builder::new().output("custom/output/dir");

    // Should be valid to set output without inputs (even if build() would fail)
}

#[cfg(feature = "codegen")]
#[test]
fn test_builder_input_only() {
    let _builder = Builder::new().input("test.rpc.rs");

    // Should be valid to set input without output (uses default)
}

// Note: We don't test build() method because it requires actual .rpc.rs files
// and would do real I/O. Those are covered by integration tests.

#[cfg(not(feature = "codegen"))]
#[test]
fn test_codegen_feature_disabled() {
    // When codegen feature is disabled, Builder shouldn't be available
    // This test just documents the behavior
}
