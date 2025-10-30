//! Python code generator for RpcNet services
//!
//! This module generates Python client and server code from parsed service definitions.
//! The generated code uses the PyO3 bridge (_rpcnet module) for communication.

use super::{ServiceDefinition, ServiceType};
use std::fs;
use std::path::Path;
use syn::{Fields, TraitItemFn, Type, PathArguments, GenericArgument};

/// Generates Python code from service definitions
pub struct PythonGenerator {
    definition: ServiceDefinition,
}

impl PythonGenerator {
    /// Create a new Python generator
    pub fn new(definition: ServiceDefinition) -> Self {
        Self { definition }
    }

    /// Generate Python type definitions (dataclasses and enums)
    pub fn generate_types(&self) -> String {
        let mut code = String::new();

        code.push_str("\"\"\"Generated type definitions for RPC service\"\"\"\n");
        code.push_str("from dataclasses import dataclass\n");
        code.push_str("from typing import Optional, List, Dict, Any\n");
        code.push_str("from enum import Enum\n");
        code.push_str("import json\n\n");

        // Generate dataclasses for structs
        for (name, type_def) in &self.definition.types {
            match type_def {
                ServiceType::Struct(struct_item) => {
                    code.push_str(&self.generate_dataclass(name, struct_item));
                    code.push_str("\n\n");
                }
                ServiceType::Enum(enum_item) => {
                    code.push_str(&self.generate_enum(name, enum_item));
                    code.push_str("\n\n");
                }
            }
        }

        code
    }

    /// Generate a Python dataclass from a Rust struct
    fn generate_dataclass(&self, name: &str, struct_item: &syn::ItemStruct) -> String {
        let mut code = String::new();

        // Add docstring if available
        if let Some(doc) = extract_doc_comment(&struct_item.attrs) {
            code.push_str(&format!("\"\"\"{}\"\"\"", doc.trim()));
            code.push('\n');
        }

        code.push_str("@dataclass\n");
        code.push_str(&format!("class {}:\n", name));

        // Generate fields
        match &struct_item.fields {
            Fields::Named(fields) => {
                if fields.named.is_empty() {
                    code.push_str("    pass\n");
                } else {
                    for field in &fields.named {
                        let field_name = field.ident.as_ref().unwrap();
                        let python_type = rust_type_to_python(&field.ty);

                        if let Some(doc) = extract_doc_comment(&field.attrs) {
                            code.push_str(&format!("    # {}\n", doc.trim()));
                        }

                        code.push_str(&format!("    {}: {}\n", field_name, python_type));
                    }
                }
            }
            _ => {
                code.push_str("    pass  # Tuple structs not yet supported\n");
            }
        }

        code
    }

    /// Generate a Python enum from a Rust enum
    fn generate_enum(&self, name: &str, enum_item: &syn::ItemEnum) -> String {
        let mut code = String::new();

        // Add docstring if available
        if let Some(doc) = extract_doc_comment(&enum_item.attrs) {
            code.push_str(&format!("\"\"\"{}\"\"\"", doc.trim()));
            code.push('\n');
        }

        code.push_str(&format!("class {}(Enum):\n", name));

        // Generate enum variants
        if enum_item.variants.is_empty() {
            code.push_str("    pass\n");
        } else {
            for (idx, variant) in enum_item.variants.iter().enumerate() {
                let variant_name = &variant.ident;

                if let Some(doc) = extract_doc_comment(&variant.attrs) {
                    code.push_str(&format!("    # {}\n", doc.trim()));
                }

                // Simple enum: just assign integer values
                code.push_str(&format!("    {} = {}\n",
                    variant_name.to_string().to_uppercase(),
                    idx
                ));
            }
        }

        code
    }

    /// Generate Python client code
    pub fn generate_client(&self) -> String {
        let service_name = self.definition.service_name();
        let mut code = String::new();

        code.push_str(&format!("\"\"\"Generated {} client\"\"\"\n", service_name));
        code.push_str("import asyncio\n");
        code.push_str("from typing import Optional\n");
        code.push_str("import _rpcnet\n");
        code.push_str("from .types import *\n\n");

        code.push_str(&format!("class {}Client:\n", service_name));
        code.push_str(&format!("    \"\"\"Type-safe client for {} service\n\n", service_name));
        code.push_str("    All methods are async and use the underlying _rpcnet.RpcClient\n");
        code.push_str("    for communication over QUIC+TLS.\n");
        code.push_str("    \"\"\"\n\n");

        // Constructor
        code.push_str("    def __init__(self, client: _rpcnet.RpcClient):\n");
        code.push_str("        self._client = client\n\n");

        // Static connect method
        code.push_str("    @staticmethod\n");
        code.push_str("    async def connect(\n");
        code.push_str("        addr: str,\n");
        code.push_str("        cert_path: str,\n");
        code.push_str("        key_path: Optional[str] = None,\n");
        code.push_str("        server_name: Optional[str] = None,\n");
        code.push_str("        timeout_secs: Optional[int] = None,\n");
        code.push_str(&format!("    ) -> '{}Client':\n", service_name));
        code.push_str(&format!("        \"\"\"Connect to {} server\n\n", service_name));
        code.push_str("        Args:\n");
        code.push_str("            addr: Server address (e.g., '127.0.0.1:8080')\n");
        code.push_str("            cert_path: Path to TLS certificate\n");
        code.push_str("            key_path: Optional path to private key\n");
        code.push_str("            server_name: Optional server name for TLS\n");
        code.push_str("            timeout_secs: Optional timeout in seconds\n\n");
        code.push_str("        Returns:\n");
        code.push_str(&format!("            {}Client: Connected client instance\n", service_name));
        code.push_str("        \"\"\"\n");
        code.push_str("        config = _rpcnet.RpcConfig(\n");
        code.push_str("            cert_path=cert_path,\n");
        code.push_str("            bind_addr='0.0.0.0:0',\n");
        code.push_str("            key_path=key_path,\n");
        code.push_str("            server_name=server_name,\n");
        code.push_str("            timeout_secs=timeout_secs,\n");
        code.push_str("        )\n");
        code.push_str("        client = await _rpcnet.RpcClient.connect(addr, config)\n");
        code.push_str(&format!("        return {}Client(client)\n\n", service_name));

        // Generate method for each RPC method
        for method in self.definition.methods() {
            code.push_str(&self.generate_client_method(method));
            code.push_str("\n");
        }

        code
    }

    /// Generate a single client method
    fn generate_client_method(&self, method: &TraitItemFn) -> String {
        let method_name = &method.sig.ident;
        let (request_type, response_type) = extract_method_types(method);

        let mut code = String::new();

        code.push_str(&format!("    async def {}(self, request: {}) -> {}:\n",
            method_name, request_type, response_type));

        if let Some(doc) = extract_doc_comment(&method.attrs) {
            code.push_str(&format!("        \"\"\"{}\"\"\"\n", doc.trim()));
        } else {
            code.push_str(&format!("        \"\"\"Call {} RPC method\"\"\"\n", method_name));
        }

        code.push_str("        # Serialize request to bincode bytes\n");
        code.push_str("        request_dict = request.__dict__\n");
        code.push_str("        request_bytes = _rpcnet.python_to_bincode_py(request_dict)\n");
        code.push_str("        \n");
        code.push_str(&format!("        # Call RPC method '{}'\n", method_name));
        code.push_str(&format!("        response_bytes = await self._client.call('{}', request_bytes)\n",
            method_name));
        code.push_str("        \n");
        code.push_str("        # Deserialize response from bincode\n");
        code.push_str("        response_dict = _rpcnet.bincode_to_python_py(response_bytes)\n");
        code.push_str(&format!("        return {}(**response_dict)\n", response_type));

        code
    }

    /// Generate Python server code
    pub fn generate_server(&self) -> String {
        let service_name = self.definition.service_name();
        let mut code = String::new();

        code.push_str(&format!("\"\"\"Generated {} server\"\"\"\n", service_name));
        code.push_str("import asyncio\n");
        code.push_str("from abc import ABC, abstractmethod\n");
        code.push_str("from typing import Optional\n");
        code.push_str("import _rpcnet\n");
        code.push_str("from .types import *\n\n");

        // Handler interface (abstract base class)
        code.push_str(&format!("class {}Handler(ABC):\n", service_name));
        code.push_str(&format!("    \"\"\"Handler interface for {} service\n\n", service_name));
        code.push_str("    Implement this class to define your service logic.\n");
        code.push_str("    All methods are async and should handle the business logic.\n");
        code.push_str("    \"\"\"\n\n");

        for method in self.definition.methods() {
            code.push_str(&self.generate_handler_method(method));
        }

        // Server class
        code.push_str(&format!("\n\nclass {}Server:\n", service_name));
        code.push_str(&format!("    \"\"\"RPC server for {} service\n\n", service_name));
        code.push_str("    This server wraps the low-level _rpcnet.RpcServer and\n");
        code.push_str("    automatically registers all handler methods.\n");
        code.push_str("    \"\"\"\n\n");

        code.push_str(&format!("    def __init__(self, handler: {}Handler, config: _rpcnet.RpcConfig):\n",
            service_name));
        code.push_str("        \"\"\"Initialize server with handler and configuration\n\n");
        code.push_str("        Args:\n");
        code.push_str(&format!("            handler: Implementation of {}Handler\n", service_name));
        code.push_str("            config: RPC configuration with TLS settings\n");
        code.push_str("        \"\"\"\n");
        code.push_str("        self.handler = handler\n");
        code.push_str("        self.server = _rpcnet.RpcServer(config)\n\n");

        code.push_str("    async def _register_handlers(self):\n");
        code.push_str("        \"\"\"Register all RPC method handlers\"\"\"\n");

        for method in self.definition.methods() {
            code.push_str(&self.generate_handler_registration(method));
        }

        code.push_str("\n    async def serve(self):\n");
        code.push_str("        \"\"\"Start serving requests (blocks until shutdown)\"\"\"\n");
        code.push_str("        await self._register_handlers()\n");
        code.push_str("        await self.server.serve()\n");

        code
    }

    /// Generate handler method signature
    fn generate_handler_method(&self, method: &TraitItemFn) -> String {
        let method_name = &method.sig.ident;
        let (request_type, response_type) = extract_method_types(method);

        let mut code = String::new();

        code.push_str("    @abstractmethod\n");
        code.push_str(&format!("    async def {}(self, request: {}) -> {}:\n",
            method_name, request_type, response_type));

        if let Some(doc) = extract_doc_comment(&method.attrs) {
            code.push_str(&format!("        \"\"\"{}\"\"\"\n", doc.trim()));
        } else {
            code.push_str(&format!("        \"\"\"Handle {} request\"\"\"\n", method_name));
        }

        code.push_str("        pass\n\n");

        code
    }

    /// Generate handler registration code
    fn generate_handler_registration(&self, method: &TraitItemFn) -> String {
        let method_name = &method.sig.ident;
        let (request_type, _response_type) = extract_method_types(method);

        let mut code = String::new();

        code.push_str(&format!("        \n        async def handle_{}(request_bytes: bytes) -> bytes:\n",
            method_name));
        code.push_str("            # Deserialize request from bincode\n");
        code.push_str("            request_dict = _rpcnet.bincode_to_python_py(request_bytes)\n");
        code.push_str(&format!("            request = {}(**request_dict)\n", request_type));
        code.push_str("            \n");
        code.push_str("            # Call handler\n");
        code.push_str(&format!("            response = await self.handler.{}(request)\n", method_name));
        code.push_str("            \n");
        code.push_str("            # Serialize response to bincode\n");
        code.push_str("            response_dict = response.__dict__\n");
        code.push_str("            return _rpcnet.python_to_bincode_py(response_dict)\n");
        code.push_str("        \n");
        code.push_str(&format!("        await self.server.register('{}', handle_{})\n",
            method_name, method_name));

        code
    }

    /// Write all generated files to output directory
    pub fn write_to_dir(&self, output_dir: &Path) -> std::io::Result<()> {
        let service_name = self.definition.service_name().to_string().to_lowercase();
        let service_dir = output_dir.join(&service_name);

        fs::create_dir_all(&service_dir)?;

        // Write types.py
        let types_code = self.generate_types();
        fs::write(service_dir.join("types.py"), types_code)?;

        // Write client.py
        let client_code = self.generate_client();
        fs::write(service_dir.join("client.py"), client_code)?;

        // Write server.py
        let server_code = self.generate_server();
        fs::write(service_dir.join("server.py"), server_code)?;

        // Write __init__.py
        let init_code = format!(
            "\"\"\"Generated {} service\"\"\"\n\
             from .types import *\n\
             from .client import {}Client\n\
             from .server import {}Server, {}Handler\n\n\
             __all__ = ['{}Client', '{}Server', '{}Handler']\n",
            service_name,
            self.definition.service_name(),
            self.definition.service_name(),
            self.definition.service_name(),
            self.definition.service_name(),
            self.definition.service_name(),
            self.definition.service_name(),
        );
        fs::write(service_dir.join("__init__.py"), init_code)?;

        Ok(())
    }
}

/// Extract doc comments from attributes
fn extract_doc_comment(attrs: &[syn::Attribute]) -> Option<String> {
    let mut docs = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let syn::Meta::NameValue(meta) = &attr.meta {
                if let syn::Expr::Lit(expr_lit) = &meta.value {
                    if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                        docs.push(lit_str.value());
                    }
                }
            }
        }
    }

    if docs.is_empty() {
        None
    } else {
        Some(docs.join("\n"))
    }
}

/// Convert Rust type to Python type annotation
fn rust_type_to_python(ty: &Type) -> String {
    match ty {
        Type::Path(type_path) => {
            let segment = type_path.path.segments.last().unwrap();
            let ident = &segment.ident;

            match ident.to_string().as_str() {
                "i8" | "i16" | "i32" | "i64" | "i128" |
                "u8" | "u16" | "u32" | "u64" | "u128" |
                "isize" | "usize" => "int".to_string(),
                "f32" | "f64" => "float".to_string(),
                "bool" => "bool".to_string(),
                "String" | "str" => "str".to_string(),
                "Vec" => {
                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                            return format!("List[{}]", rust_type_to_python(inner_ty));
                        }
                    }
                    "List[Any]".to_string()
                }
                "Option" => {
                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                            return format!("Optional[{}]", rust_type_to_python(inner_ty));
                        }
                    }
                    "Optional[Any]".to_string()
                }
                "HashMap" | "BTreeMap" => "Dict[str, Any]".to_string(),
                other => other.to_string(), // Custom types
            }
        }
        _ => "Any".to_string(),
    }
}

/// Extract request and response types from a method signature
fn extract_method_types(method: &TraitItemFn) -> (String, String) {
    // Find the request parameter (second parameter after &self)
    let request_type = if method.sig.inputs.len() >= 2 {
        if let syn::FnArg::Typed(pat_type) = &method.sig.inputs[1] {
            if let Type::Path(type_path) = &*pat_type.ty {
                type_path.path.segments.last()
                    .map(|s| s.ident.to_string())
                    .unwrap_or_else(|| "Any".to_string())
            } else {
                "Any".to_string()
            }
        } else {
            "Any".to_string()
        }
    } else {
        "Any".to_string()
    };

    // Extract response type from Result<Response, Error>
    let response_type = if let syn::ReturnType::Type(_, ty) = &method.sig.output {
        if let Type::Path(type_path) = &**ty {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident == "Result" {
                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(GenericArgument::Type(Type::Path(response_path))) = args.args.first() {
                            return (
                                request_type,
                                response_path.path.segments.last()
                                    .map(|s| s.ident.to_string())
                                    .unwrap_or_else(|| "Any".to_string())
                            );
                        }
                    }
                }
            }
        }
        "Any".to_string()
    } else {
        "Any".to_string()
    };

    (request_type, response_type)
}
