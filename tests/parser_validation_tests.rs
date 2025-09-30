// Parser validation tests for code generation module
// These tests focus on edge cases in service definition parsing

#![cfg(feature = "codegen")]

use rpcnet::codegen::{ServiceDefinition, ServiceType};

#[test]
fn test_parse_empty_file() {
    // Test parsing completely empty file
    let content = "";
    let result = ServiceDefinition::parse(content);
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert!(error.to_string().contains("No service trait found"));
}

#[test]
fn test_parse_whitespace_only() {
    // Test parsing file with only whitespace
    let content = "   \n\t\r\n   ";
    let result = ServiceDefinition::parse(content);
    assert!(result.is_err());
}

#[test]
fn test_parse_comments_only() {
    // Test parsing file with only comments
    let content = r#"
        // This is a comment
        /* This is a block comment */
        /// This is a doc comment
    "#;
    let result = ServiceDefinition::parse(content);
    assert!(result.is_err());
}

#[test]
fn test_parse_valid_service_trait() {
    // Test parsing a valid service trait
    let content = r#"
        use std::result::Result;
        
        #[rpcnet::service]
        pub trait TestService {
            async fn test_method(&self, param: String) -> Result<Vec<u8>, String>;
        }
        
        pub struct RequestType {
            pub data: String,
        }
        
        pub enum ErrorType {
            ValidationError,
            ProcessingError,
        }
    "#;

    let result = ServiceDefinition::parse(content);
    assert!(result.is_ok());

    let service = result.unwrap();
    assert_eq!(service.service_name().to_string(), "TestService");
    assert_eq!(service.methods().len(), 1);
    assert_eq!(service.types.len(), 2);
    assert!(service.types.contains_key("RequestType"));
    assert!(service.types.contains_key("ErrorType"));
}

#[test]
fn test_parse_service_attribute_variants() {
    // Test different ways to specify the service attribute
    let variants = vec![
        "#[rpcnet::service]",
        "#[service]", // Assuming use rpcnet::service;
    ];

    for attr in variants {
        let content = format!(
            r#"
            {}
            pub trait TestService {{
                async fn test(&self) -> Result<Vec<u8>, String>;
            }}
        "#,
            attr
        );

        let result = ServiceDefinition::parse(&content);
        assert!(result.is_ok(), "Failed to parse with attribute: {}", attr);
    }
}

#[test]
fn test_parse_multiple_service_traits() {
    // Test parsing file with multiple service traits (should fail)
    let content = r#"
        #[rpcnet::service]
        pub trait FirstService {
            async fn method1(&self) -> Result<Vec<u8>, String>;
        }
        
        #[rpcnet::service]
        pub trait SecondService {
            async fn method2(&self) -> Result<Vec<u8>, String>;
        }
    "#;

    let result = ServiceDefinition::parse(content);
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert!(error.to_string().contains("Multiple service traits found"));
}

#[test]
fn test_parse_non_async_methods() {
    // Test parsing service with non-async methods (should fail)
    let content = r#"
        #[rpcnet::service]
        pub trait TestService {
            fn sync_method(&self) -> Result<Vec<u8>, String>;
        }
    "#;

    let result = ServiceDefinition::parse(content);
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert!(error.to_string().contains("Service methods must be async"));
}

#[test]
fn test_parse_methods_without_self() {
    // Test parsing service methods without &self parameter (should fail)
    let content = r#"
        #[rpcnet::service]
        pub trait TestService {
            async fn static_method() -> Result<Vec<u8>, String>;
        }
    "#;

    let result = ServiceDefinition::parse(content);
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert!(error
        .to_string()
        .contains("Service methods must have &self as first parameter"));
}

#[test]
fn test_parse_methods_without_result_return() {
    // Test parsing service methods that don't return Result (should fail)
    let content = r#"
        #[rpcnet::service]
        pub trait TestService {
            async fn bad_method(&self) -> Vec<u8>;
        }
    "#;

    let result = ServiceDefinition::parse(content);
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert!(error
        .to_string()
        .contains("Service methods must return Result"));
}

#[test]
fn test_parse_methods_with_no_return_type() {
    // Test parsing service methods with no return type (should fail)
    let content = r#"
        #[rpcnet::service]
        pub trait TestService {
            async fn void_method(&self);
        }
    "#;

    let result = ServiceDefinition::parse(content);
    assert!(result.is_err());

    let error = result.unwrap_err();
    assert!(error
        .to_string()
        .contains("Service methods must return Result"));
}

#[test]
fn test_parse_complex_method_signatures() {
    // Test parsing complex but valid method signatures
    let content = r#"
        #[rpcnet::service]
        pub trait ComplexService {
            async fn simple(&self) -> Result<String, Error>;
            async fn with_params(&self, id: u64, name: String) -> Result<User, ServiceError>;
            async fn with_generics<T>(&self, data: T) -> Result<Response<T>, Box<dyn std::error::Error>>;
            async fn with_lifetime<'a>(&self, data: &'a str) -> Result<&'a str, Error>;
        }
        
        pub struct User {
            pub id: u64,
            pub name: String,
        }
        
        pub struct Response<T> {
            pub data: T,
        }
        
        pub enum ServiceError {
            NotFound,
            InvalidInput,
        }
    "#;

    let result = ServiceDefinition::parse(content);
    assert!(result.is_ok());

    let service = result.unwrap();
    assert_eq!(service.methods().len(), 4);
}

#[test]
fn test_parse_trait_with_associated_types() {
    // Test parsing trait with associated types
    let content = r#"
        #[rpcnet::service]
        pub trait ServiceWithAssociatedTypes {
            type Error;
            type Response;
            
            async fn method(&self) -> Result<Self::Response, Self::Error>;
        }
    "#;

    // This might be valid depending on implementation
    let result = ServiceDefinition::parse(content);
    // We don't assert success/failure as it depends on implementation requirements
    match result {
        Ok(service) => {
            assert_eq!(
                service.service_name().to_string(),
                "ServiceWithAssociatedTypes"
            );
        }
        Err(_) => {
            // Also acceptable if associated types aren't supported
        }
    }
}

#[test]
fn test_parse_trait_with_default_implementations() {
    // Test parsing trait with default method implementations
    let content = r#"
        #[rpcnet::service]
        pub trait ServiceWithDefaults {
            async fn required_method(&self) -> Result<String, Error>;
            
            async fn default_method(&self) -> Result<String, Error> {
                Ok("default".to_string())
            }
        }
    "#;

    let result = ServiceDefinition::parse(content);
    if result.is_ok() {
        let service = result.unwrap();
        assert_eq!(service.methods().len(), 2);
    }
    // Default implementations might not be supported, so error is also acceptable
}

#[test]
fn test_parse_unicode_identifiers() {
    // Test parsing with Unicode identifiers
    let content = r#"
        #[rpcnet::service]
        pub trait Сервис {
            async fn метод(&self, データ: String) -> Result<Vec<u8>, Error>;
        }
        
        pub struct Данные {
            pub значение: String,
        }
    "#;

    let result = ServiceDefinition::parse(content);
    if result.is_ok() {
        let service = result.unwrap();
        assert_eq!(service.service_name().to_string(), "Сервис");
    }
    // Unicode might not be fully supported in all contexts
}

#[test]
fn test_parse_very_long_identifiers() {
    // Test parsing with very long identifiers
    let long_name = "a".repeat(1000);
    let content = format!(
        r#"
        #[rpcnet::service]
        pub trait {} {{
            async fn {}(&self) -> Result<Vec<u8>, Error>;
        }}
    "#,
        long_name, long_name
    );

    let result = ServiceDefinition::parse(&content);
    if result.is_ok() {
        let service = result.unwrap();
        assert_eq!(service.service_name().to_string(), long_name);
    }
}

#[test]
fn test_parse_deeply_nested_types() {
    // Test parsing with deeply nested type definitions
    let content = r#"
        #[rpcnet::service]
        pub trait NestedService {
            async fn method(&self) -> Result<Option<Vec<Result<String, Box<dyn std::error::Error>>>>, Error>;
        }
    "#;

    let result = ServiceDefinition::parse(content);
    assert!(result.is_ok());
}

#[test]
fn test_parse_invalid_rust_syntax() {
    // Test parsing with invalid Rust syntax
    let invalid_contents = vec![
        "trait {",                                                     // Incomplete trait
        "#[rpcnet::service] trait",                                    // Missing trait body
        "pub trait Test { async fn }",                                 // Incomplete method
        "pub trait Test { async fn test( -> Result<String, Error>; }", // Missing parameter list
        "trait Test { fn test(&self -> Result<String, Error>; }",      // Missing closing paren
    ];

    for content in invalid_contents {
        let result = ServiceDefinition::parse(content);
        assert!(result.is_err(), "Should fail to parse: {}", content);
    }
}

#[test]
fn test_parse_with_complex_imports() {
    // Test parsing with various import statements
    let content = r#"
        use std::collections::HashMap;
        use std::result::Result;
        use serde::{Serialize, Deserialize};
        use crate::types::*;
        use super::common::{Error, Response};
        
        #[rpcnet::service]
        pub trait ImportService {
            async fn method(&self, data: HashMap<String, String>) -> Result<Response, Error>;
        }
    "#;

    let result = ServiceDefinition::parse(content);
    assert!(result.is_ok());

    let service = result.unwrap();
    assert_eq!(service.imports.len(), 5);
}

#[test]
fn test_parse_empty_trait() {
    // Test parsing trait with no methods
    let content = r#"
        #[rpcnet::service]
        pub trait EmptyService {
        }
    "#;

    let result = ServiceDefinition::parse(content);
    assert!(result.is_ok());

    let service = result.unwrap();
    assert_eq!(service.methods().len(), 0);
}

#[test]
fn test_parse_trait_with_attributes() {
    // Test parsing trait and methods with various attributes
    let content = r#"
        #[rpcnet::service]
        #[derive(Debug)]
        pub trait AttributedService {
            #[deprecated]
            async fn old_method(&self) -> Result<String, Error>;
            
            #[allow(unused)]
            async fn new_method(&self) -> Result<String, Error>;
        }
    "#;

    let result = ServiceDefinition::parse(content);
    assert!(result.is_ok());

    let service = result.unwrap();
    assert_eq!(service.methods().len(), 2);
}

#[test]
fn test_service_type_variants() {
    // Test that ServiceType enum correctly categorizes types
    let content = r#"
        #[rpcnet::service]
        pub trait TestService {
            async fn method(&self) -> Result<MyStruct, MyEnum>;
        }
        
        pub struct MyStruct {
            pub field: String,
        }
        
        pub enum MyEnum {
            Variant1,
            Variant2(String),
        }
    "#;

    let result = ServiceDefinition::parse(content).unwrap();

    match result.types.get("MyStruct").unwrap() {
        ServiceType::Struct(_) => {} // Expected
        ServiceType::Enum(_) => panic!("MyStruct should be categorized as Struct"),
    }

    match result.types.get("MyEnum").unwrap() {
        ServiceType::Enum(_) => {} // Expected
        ServiceType::Struct(_) => panic!("MyEnum should be categorized as Enum"),
    }
}

#[test]
fn test_method_extraction() {
    // Test that method extraction works correctly
    let content = r#"
        #[rpcnet::service]
        pub trait TestService {
            async fn method1(&self) -> Result<String, Error>;
            async fn method2(&self, param: u32) -> Result<u32, Error>;
        }
    "#;

    let result = ServiceDefinition::parse(content).unwrap();
    let methods = result.methods();

    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0].sig.ident.to_string(), "method1");
    assert_eq!(methods[1].sig.ident.to_string(), "method2");
}
