//! Python code generator for RpcNet services
//!
//! This module generates Python client and server code from parsed service definitions.
//! The generated code uses the PyO3 bridge (rpcnet module) for communication.

use super::{ServiceDefinition, ServiceType};
use std::fs;
use std::path::Path;
use syn::{Fields, GenericArgument, PathArguments, TraitItemFn, Type};

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
        code.push_str("from typing import Optional, List, Dict, Any, Union\n");
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
        // Check if any variant has fields (associated data)
        let has_data = enum_item
            .variants
            .iter()
            .any(|v| !matches!(v.fields, syn::Fields::Unit));

        if has_data {
            self.generate_enum_with_data(name, enum_item)
        } else {
            self.generate_simple_enum(name, enum_item)
        }
    }

    /// Generate a simple Python enum (no associated data)
    fn generate_simple_enum(&self, name: &str, enum_item: &syn::ItemEnum) -> String {
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

                code.push_str(&format!(
                    "    {} = {}\n",
                    variant_name.to_string().to_uppercase(),
                    idx
                ));
            }
        }

        code
    }

    /// Generate Python Union type for enums with associated data
    fn generate_enum_with_data(&self, name: &str, enum_item: &syn::ItemEnum) -> String {
        let mut code = String::new();

        // Add docstring if available
        if let Some(doc) = extract_doc_comment(&enum_item.attrs) {
            code.push_str(&format!("\"\"\"{}\"\"\"", doc.trim()));
            code.push('\n');
        }

        // Generate variant dataclasses
        let mut variant_classes = Vec::new();
        for variant in &enum_item.variants {
            let variant_name = &variant.ident;
            let class_name = format!("{}{}", name, variant_name);
            variant_classes.push(class_name.clone());

            code.push_str("@dataclass\n");
            code.push_str(&format!("class {}:\n", class_name));

            if let Some(doc) = extract_doc_comment(&variant.attrs) {
                code.push_str(&format!("    \"\"\"{}\"\"\"", doc.trim()));
                code.push('\n');
            }

            match &variant.fields {
                syn::Fields::Named(fields) => {
                    if fields.named.is_empty() {
                        code.push_str("    pass\n");
                    } else {
                        for field in &fields.named {
                            let field_name = field.ident.as_ref().unwrap();
                            let field_type = self.map_rust_type_to_python(&field.ty);
                            code.push_str(&format!("    {}: {}\n", field_name, field_type));
                        }
                    }
                }
                syn::Fields::Unnamed(fields) => {
                    if fields.unnamed.is_empty() {
                        code.push_str("    pass\n");
                    } else {
                        for (idx, field) in fields.unnamed.iter().enumerate() {
                            let field_type = self.map_rust_type_to_python(&field.ty);
                            code.push_str(&format!("    field_{}: {}\n", idx, field_type));
                        }
                    }
                }
                syn::Fields::Unit => {
                    code.push_str("    pass\n");
                }
            }
            code.push('\n');
        }

        // Generate Union type
        code.push_str(&format!("{} = Union[\n", name));
        for (idx, class_name) in variant_classes.iter().enumerate() {
            let comma = if idx < variant_classes.len() - 1 {
                ","
            } else {
                ""
            };
            code.push_str(&format!("    {}{}\n", class_name, comma));
        }
        code.push_str("]\n\n");

        // Generate helper functions
        code.push_str(&self.generate_enum_deserializer(name, enum_item));
        code.push_str("\n\n");
        code.push_str(&self.generate_enum_serializer(name, enum_item));

        code
    }

    /// Map Rust type to Python type annotation
    fn map_rust_type_to_python(&self, ty: &syn::Type) -> String {
        map_rust_type_to_python_impl(ty)
    }
}

/// Helper function to map Rust type to Python type annotation
fn map_rust_type_to_python_impl(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                let type_name = segment.ident.to_string();
                match type_name.as_str() {
                    "String" | "str" => "str".to_string(),
                    "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64"
                    | "u128" | "isize" | "usize" => "int".to_string(),
                    "f32" | "f64" => "float".to_string(),
                    "bool" => "bool".to_string(),
                    "Vec" => {
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                                return format!("List[{}]", map_rust_type_to_python_impl(inner));
                            }
                        }
                        "List[Any]".to_string()
                    }
                    "Option" => {
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                                return format!(
                                    "Optional[{}]",
                                    map_rust_type_to_python_impl(inner)
                                );
                            }
                        }
                        "Optional[Any]".to_string()
                    }
                    _ => type_name,
                }
            } else {
                "Any".to_string()
            }
        }
        _ => "Any".to_string(),
    }
}

impl PythonGenerator {
    /// Generate deserializer for enum with associated data
    fn generate_enum_deserializer(&self, name: &str, enum_item: &syn::ItemEnum) -> String {
        let mut code = String::new();
        let fn_name = format!("deserialize_{}", name.to_lowercase());

        code.push_str(&format!("def {}(data: Any) -> {}:\n", fn_name, name));
        code.push_str(&format!(
            "    \"\"\"Deserialize MessagePack data to {} variant.\"\"\"\n",
            name
        ));
        code.push_str("    if not isinstance(data, dict):\n");
        code.push_str("        raise ValueError(f\"Expected dict for enum, got {type(data)}\")\n");
        code.push_str("    \n");
        code.push_str("    if len(data) != 1:\n");
        code.push_str("        raise ValueError(f\"Expected single-key dict for enum, got {len(data)} keys\")\n");
        code.push_str("    \n");
        code.push_str("    variant_name, variant_data = next(iter(data.items()))\n");
        code.push_str("    \n");

        for variant in &enum_item.variants {
            let variant_name = &variant.ident;
            let class_name = format!("{}{}", name, variant_name);

            code.push_str(&format!("    if variant_name == '{}':\n", variant_name));

            match &variant.fields {
                syn::Fields::Unit => {
                    code.push_str(&format!("        return {}()\n", class_name));
                }
                syn::Fields::Named(_) | syn::Fields::Unnamed(_) => {
                    code.push_str("        if isinstance(variant_data, dict):\n");
                    code.push_str(&format!(
                        "            return {}(**variant_data)\n",
                        class_name
                    ));
                    code.push_str("        elif isinstance(variant_data, list):\n");
                    code.push_str(&format!(
                        "            return {}(*variant_data)\n",
                        class_name
                    ));
                    code.push_str("        elif variant_data is None:\n");
                    code.push_str(&format!("            return {}()\n", class_name));
                    code.push_str("        else:\n");
                    code.push_str("            raise ValueError(f\"Unexpected variant data type: {type(variant_data)}\")\n");
                }
            }
        }

        code.push_str("    \n");
        code.push_str("    raise ValueError(f\"Unknown variant: {variant_name}\")\n");

        code
    }

    /// Generate serializer for enum with associated data
    fn generate_enum_serializer(&self, name: &str, enum_item: &syn::ItemEnum) -> String {
        let mut code = String::new();
        let fn_name = format!("serialize_{}", name.to_lowercase());

        code.push_str(&format!(
            "def {}(value: {}) -> Dict[str, Any]:\n",
            fn_name, name
        ));
        code.push_str(&format!(
            "    \"\"\"Serialize {} variant to MessagePack-compatible dict.\"\"\"\n",
            name
        ));

        for variant in &enum_item.variants {
            let variant_name = &variant.ident;
            let class_name = format!("{}{}", name, variant_name);

            code.push_str(&format!("    if isinstance(value, {}):\n", class_name));

            match &variant.fields {
                syn::Fields::Named(fields) => {
                    if fields.named.is_empty() {
                        code.push_str(&format!("        return {{'{}': None}}\n", variant_name));
                    } else {
                        code.push_str(&format!("        return {{'{}': {{\n", variant_name));
                        for field in &fields.named {
                            let field_name = field.ident.as_ref().unwrap();
                            code.push_str(&format!(
                                "            '{}': value.{},\n",
                                field_name, field_name
                            ));
                        }
                        code.push_str("        }}\n");
                    }
                }
                syn::Fields::Unnamed(fields) => {
                    if fields.unnamed.is_empty() {
                        code.push_str(&format!("        return {{'{}': None}}\n", variant_name));
                    } else {
                        code.push_str(&format!("        return {{'{}': [\n", variant_name));
                        for idx in 0..fields.unnamed.len() {
                            code.push_str(&format!("            value.field_{},\n", idx));
                        }
                        code.push_str("        ]}\n");
                    }
                }
                syn::Fields::Unit => {
                    code.push_str(&format!("        return {{'{}': None}}\n", variant_name));
                }
            }
        }

        code.push_str("    \n");
        code.push_str("    raise ValueError(f\"Unknown value type: {type(value)}\")\n");

        code
    }

    /// Generate Python client code
    pub fn generate_client(&self) -> String {
        let service_name = self.definition.service_name();
        let mut code = String::new();

        code.push_str(&format!("\"\"\"Generated {} client\"\"\"\n", service_name));
        code.push_str("import asyncio\n");
        code.push_str("from typing import Optional, AsyncIterable, AsyncIterator\n");
        code.push_str("import rpcnet\n");
        code.push_str("from .types import *\n\n");

        code.push_str(&format!("class {}Client:\n", service_name));
        code.push_str(&format!(
            "    \"\"\"Type-safe client for {} service\n\n",
            service_name
        ));
        code.push_str("    All methods are async and use the underlying rpcnet.RpcClient\n");
        code.push_str("    for communication over QUIC+TLS.\n");
        code.push_str("    \"\"\"\n\n");

        // Constructor
        code.push_str("    def __init__(self, client: rpcnet.RpcClient):\n");
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
        code.push_str(&format!(
            "        \"\"\"Connect to {} server\n\n",
            service_name
        ));
        code.push_str("        Args:\n");
        code.push_str("            addr: Server address (e.g., '127.0.0.1:8080')\n");
        code.push_str("            cert_path: Path to TLS certificate\n");
        code.push_str("            key_path: Optional path to private key\n");
        code.push_str("            server_name: Optional server name for TLS\n");
        code.push_str("            timeout_secs: Optional timeout in seconds\n\n");
        code.push_str("        Returns:\n");
        code.push_str(&format!(
            "            {}Client: Connected client instance\n",
            service_name
        ));
        code.push_str("        \"\"\"\n");
        code.push_str("        config = rpcnet.RpcConfig(\n");
        code.push_str("            cert_path=cert_path,\n");
        code.push_str("            bind_addr='0.0.0.0:0',\n");
        code.push_str("            key_path=key_path,\n");
        code.push_str("            server_name=server_name,\n");
        code.push_str("            timeout_secs=timeout_secs,\n");
        code.push_str("        )\n");
        code.push_str("        client = await rpcnet.RpcClient.connect(addr, config)\n");
        code.push_str(&format!(
            "        return {}Client(client)\n\n",
            service_name
        ));

        // Generate method for each RPC method
        for method in self.definition.methods() {
            code.push_str(&self.generate_client_method(method));
            code.push('\n');
        }

        // Generate BlockingClient class
        code.push_str("\n\n");
        code.push_str(&format!("class {}BlockingClient:\n", service_name));
        code.push_str(&format!(
            "    \"\"\"Type-safe blocking client for {} service\n\n",
            service_name
        ));
        code.push_str("    This client provides synchronous methods without async/await.\n");
        code.push_str("    It uses the BlockingClient internally which releases the GIL during I/O.\n");
        code.push_str("    \n");
        code.push_str("    Performance characteristics:\n");
        code.push_str("    - ~30% lower latency than async client\n");
        code.push_str("    - 3.5x throughput with batch API\n");
        code.push_str("    - Works well with threads due to GIL release\n");
        code.push_str("    \"\"\"\n\n");

        // Constructor for BlockingClient
        code.push_str("    def __init__(self, client: rpcnet.BlockingClient):\n");
        code.push_str("        self._client = client\n\n");

        // Static connect method for BlockingClient
        code.push_str("    @staticmethod\n");
        code.push_str("    def connect(\n");
        code.push_str("        addr: str,\n");
        code.push_str("        cert_path: str,\n");
        code.push_str("        key_path: Optional[str] = None,\n");
        code.push_str("        server_name: Optional[str] = None,\n");
        code.push_str("        timeout_secs: Optional[int] = None,\n");
        code.push_str(&format!("    ) -> '{}BlockingClient':\n", service_name));
        code.push_str(&format!(
            "        \"\"\"Connect to {} server (blocking)\n\n",
            service_name
        ));
        code.push_str("        Args:\n");
        code.push_str("            addr: Server address (e.g., '127.0.0.1:8080')\n");
        code.push_str("            cert_path: Path to TLS certificate\n");
        code.push_str("            key_path: Optional path to private key\n");
        code.push_str("            server_name: Optional server name for TLS\n");
        code.push_str("            timeout_secs: Optional timeout in seconds\n\n");
        code.push_str("        Returns:\n");
        code.push_str(&format!(
            "            {}BlockingClient: Connected client instance\n",
            service_name
        ));
        code.push_str("        \"\"\"\n");
        code.push_str("        config = rpcnet.RpcConfig(\n");
        code.push_str("            cert_path=cert_path,\n");
        code.push_str("            bind_addr='0.0.0.0:0',\n");
        code.push_str("            key_path=key_path,\n");
        code.push_str("            server_name=server_name,\n");
        code.push_str("            timeout_secs=timeout_secs,\n");
        code.push_str("        )\n");
        code.push_str("        client = rpcnet.BlockingClient.connect(addr, config)\n");
        code.push_str(&format!(
            "        return {}BlockingClient(client)\n\n",
            service_name
        ));

        // Generate blocking methods for each RPC method
        for method in self.definition.methods() {
            let blocking_method = self.generate_blocking_client_method(method);
            if !blocking_method.is_empty() {
                code.push_str(&blocking_method);
                code.push('\n');
            }
        }

        code
    }

    /// Generate a single client method
    fn generate_client_method(&self, method: &TraitItemFn) -> String {
        // Check if this is a streaming method
        if is_streaming_method(method) {
            self.generate_streaming_client_method(method)
        } else {
            self.generate_regular_client_method(method)
        }
    }

    /// Check if a type name refers to an enum with associated data
    fn is_enum_with_data(&self, type_name: &str) -> bool {
        // Check if this type is an enum in our definition
        if let Some(crate::codegen::parser::ServiceType::Enum(enum_item)) =
            self.definition.types.get(type_name)
        {
            // Check if any variant has fields
            return enum_item
                .variants
                .iter()
                .any(|v| !matches!(v.fields, syn::Fields::Unit));
        }
        false
    }

    /// Check if a type name refers to any enum type
    fn is_enum(&self, type_name: &str) -> bool {
        matches!(
            self.definition.types.get(type_name),
            Some(crate::codegen::parser::ServiceType::Enum(_))
        )
    }

    /// Generate a blocking client method (no async)
    fn generate_blocking_client_method(&self, method: &TraitItemFn) -> String {
        // Only generate for non-streaming methods
        if is_streaming_method(method) {
            return String::new(); // Skip streaming methods for blocking client
        }
        
        let method_name = &method.sig.ident;
        let service_name = self.definition.service_name();
        let (request_type, response_type) = extract_method_types(method);

        let mut code = String::new();

        code.push_str(&format!(
            "    def {}(self, request: {}) -> {}:\n",
            method_name, request_type, response_type
        ));

        if let Some(doc) = extract_doc_comment(&method.attrs) {
            code.push_str(&format!("        \"\"\"{} (blocking)\"\"\"\n", doc.trim()));
        } else {
            code.push_str(&format!(
                "        \"\"\"Call {} RPC method (blocking)\"\"\"\n",
                method_name
            ));
        }

        code.push_str("        # Serialize request to MessagePack bytes\n");
        code.push_str("        request_dict = request.__dict__\n");
        code.push_str("        request_bytes = rpcnet.python_to_msgpack_py(request_dict)\n");
        code.push_str("        \n");
        code.push_str(&format!(
            "        # Call RPC method '{}.{}' (blocking)\n",
            service_name, method_name
        ));
        code.push_str(&format!(
            "        response_bytes = self._client.call('{}.{}', request_bytes)\n",
            service_name, method_name
        ));
        code.push_str("        \n");
        code.push_str("        # Deserialize response from MessagePack bytes\n");
        code.push_str("        response_dict = rpcnet.msgpack_to_python_py(response_bytes)\n");

        // Check if response is an enum with data
        if self.is_enum_with_data(&response_type) {
            // For enums with associated data, use the deserializer function
            let deserializer = format!("deserialize_{}", response_type.to_lowercase());
            code.push_str(&format!(
                "        return {}(response_dict)\n",
                deserializer
            ));
        } else if self.is_enum(&response_type) {
            // For simple enums, use the deserializer function
            let deserializer = format!("deserialize_{}", response_type.to_lowercase());
            code.push_str(&format!(
                "        return {}(response_dict)\n",
                deserializer
            ));
        } else {
            // For regular structs
            code.push_str(&format!("        return {}(**response_dict)\n", response_type));
        }

        code
    }

    /// Generate a regular (non-streaming) client method
    fn generate_regular_client_method(&self, method: &TraitItemFn) -> String {
        let method_name = &method.sig.ident;
        let service_name = self.definition.service_name();
        let (request_type, response_type) = extract_method_types(method);

        let mut code = String::new();

        code.push_str(&format!(
            "    async def {}(self, request: {}) -> {}:\n",
            method_name, request_type, response_type
        ));

        if let Some(doc) = extract_doc_comment(&method.attrs) {
            code.push_str(&format!("        \"\"\"{}\"\"\"\n", doc.trim()));
        } else {
            code.push_str(&format!(
                "        \"\"\"Call {} RPC method\"\"\"\n",
                method_name
            ));
        }

        code.push_str("        # Serialize request to MessagePack bytes\n");
        code.push_str("        request_dict = request.__dict__\n");
        code.push_str("        request_bytes = rpcnet.python_to_msgpack_py(request_dict)\n");
        code.push_str("        \n");
        code.push_str(&format!(
            "        # Call RPC method '{}.{}'\n",
            service_name, method_name
        ));
        code.push_str(&format!(
            "        response_bytes = await self._client.call('{}.{}', request_bytes)\n",
            service_name, method_name
        ));
        code.push_str("        \n");
        code.push_str("        # Deserialize response from MessagePack\n");
        code.push_str("        response_dict = rpcnet.msgpack_to_python_py(response_bytes)\n");

        // Check if response type is an enum with data
        if self.is_enum_with_data(&response_type) {
            let deserializer = format!("deserialize_{}", response_type.to_lowercase());
            code.push_str(&format!("        return {}(response_dict)\n", deserializer));
        } else {
            code.push_str(&format!(
                "        return {}(**response_dict)\n",
                response_type
            ));
        }

        code
    }

    /// Generate a streaming client method
    fn generate_streaming_client_method(&self, method: &TraitItemFn) -> String {
        let method_name = &method.sig.ident;
        let service_name = self.definition.service_name();

        let mut code = String::new();

        // Extract request and response stream item types
        let request_item_type = if method.sig.inputs.len() >= 2 {
            if let syn::FnArg::Typed(pat_type) = &method.sig.inputs[1] {
                extract_stream_item_type(&pat_type.ty).unwrap_or_else(|| "Any".to_string())
            } else {
                "Any".to_string()
            }
        } else {
            "Any".to_string()
        };

        let response_item_type = if let syn::ReturnType::Type(_, ty) = &method.sig.output {
            if let Type::Path(type_path) = &**ty {
                if let Some(segment) = type_path.path.segments.last() {
                    if segment.ident == "Result" {
                        if let PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(GenericArgument::Type(ok_type)) = args.args.first() {
                                extract_stream_item_type(ok_type)
                                    .unwrap_or_else(|| "Any".to_string())
                            } else {
                                "Any".to_string()
                            }
                        } else {
                            "Any".to_string()
                        }
                    } else {
                        "Any".to_string()
                    }
                } else {
                    "Any".to_string()
                }
            } else {
                "Any".to_string()
            }
        } else {
            "Any".to_string()
        };

        code.push_str(&format!(
            "    async def {}(self, request_stream: AsyncIterable[{}]) -> AsyncIterator[{}]:\n",
            method_name, request_item_type, response_item_type
        ));

        if let Some(doc) = extract_doc_comment(&method.attrs) {
            code.push_str(&format!("        \"\"\"{}\"\"\"", doc.trim()));
        } else {
            code.push_str(&format!(
                "        \"\"\"Streaming RPC method: {}\"\"\"\n",
                method_name
            ));
        }

        code.push_str("        # Collect and serialize request stream items\n");
        code.push_str("        request_list = []\n");
        code.push_str("        async for request in request_stream:\n");
        code.push_str("            request_dict = request.__dict__\n");
        code.push_str("            request_bytes = rpcnet.python_to_msgpack_py(request_dict)\n");
        code.push_str("            request_list.append(request_bytes)\n");
        code.push_str("        \n");
        code.push_str(&format!(
            "        # Call streaming RPC method '{}.{}'\n",
            service_name, method_name
        ));
        code.push_str(&format!(
            "        response_stream = await self._client.call_streaming('{}.{}', request_list)\n",
            service_name, method_name
        ));
        code.push_str("        \n");
        code.push_str("        # Yield deserialized responses\n");
        code.push_str("        async for response_bytes in response_stream:\n");
        code.push_str("            response_dict = rpcnet.msgpack_to_python_py(response_bytes)\n");
        code.push_str("            \n");
        code.push_str(
            "            # Unwrap Result if present (Rust streaming methods return Result<T, E>)\n",
        );
        code.push_str(
            "            if isinstance(response_dict, dict) and 'Ok' in response_dict:\n",
        );
        code.push_str("                response_dict = response_dict['Ok']\n");
        code.push_str(
            "            elif isinstance(response_dict, dict) and 'Err' in response_dict:\n",
        );
        code.push_str(
            "                # Handle error variant - could raise exception or yield error\n",
        );
        code.push_str("                error_dict = response_dict['Err']\n");
        code.push_str("                raise Exception(f\"RPC error: {error_dict}\")\n");
        code.push_str("            \n");

        // Check if response type is an enum with data
        if self.is_enum_with_data(&response_item_type) {
            let deserializer = format!("deserialize_{}", response_item_type.to_lowercase());
            code.push_str(&format!(
                "            yield {}(response_dict)\n",
                deserializer
            ));
        } else {
            code.push_str(&format!(
                "            yield {}(**response_dict)\n",
                response_item_type
            ));
        }

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
        code.push_str("import rpcnet\n");
        code.push_str("from .types import *\n\n");

        // Handler interface (abstract base class)
        code.push_str(&format!("class {}Handler(ABC):\n", service_name));
        code.push_str(&format!(
            "    \"\"\"Handler interface for {} service\n\n",
            service_name
        ));
        code.push_str("    Implement this class to define your service logic.\n");
        code.push_str("    All methods are async and should handle the business logic.\n");
        code.push_str("    \"\"\"\n\n");

        for method in self.definition.methods() {
            // Skip streaming methods in server generation (not yet supported)
            if is_streaming_method(method) {
                continue;
            }
            code.push_str(&self.generate_handler_method(method));
        }

        // Server class
        code.push_str(&format!("\n\nclass {}Server:\n", service_name));
        code.push_str(&format!(
            "    \"\"\"RPC server for {} service\n\n",
            service_name
        ));
        code.push_str("    This server wraps the low-level rpcnet.RpcServer and\n");
        code.push_str("    automatically registers all handler methods.\n");
        code.push_str("    \"\"\"\n\n");

        code.push_str(&format!(
            "    def __init__(self, handler: {}Handler, config: rpcnet.RpcConfig):\n",
            service_name
        ));
        code.push_str("        \"\"\"Initialize server with handler and configuration\n\n");
        code.push_str("        Args:\n");
        code.push_str(&format!(
            "            handler: Implementation of {}Handler\n",
            service_name
        ));
        code.push_str("            config: RPC configuration with TLS settings\n");
        code.push_str("        \n");
        code.push_str("        Note: The server automatically uses all available CPU resources.\n");
        code.push_str("        \"\"\"\n");
        code.push_str("        self.handler = handler\n");
        code.push_str("        self.server = rpcnet.RpcServer(config)\n\n");

        code.push_str("    async def _register_handlers(self):\n");
        code.push_str("        \"\"\"Register all RPC method handlers\"\"\"\n");

        for method in self.definition.methods() {
            // Skip streaming methods in server generation (not yet supported)
            if is_streaming_method(method) {
                continue;
            }
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
        code.push_str(&format!(
            "    async def {}(self, request: {}) -> {}:\n",
            method_name, request_type, response_type
        ));

        if let Some(doc) = extract_doc_comment(&method.attrs) {
            code.push_str(&format!("        \"\"\"{}\"\"\"\n", doc.trim()));
        } else {
            code.push_str(&format!(
                "        \"\"\"Handle {} request\"\"\"\n",
                method_name
            ));
        }

        code.push_str("        pass\n\n");

        code
    }

    /// Generate handler registration code
    fn generate_handler_registration(&self, method: &TraitItemFn) -> String {
        let service_name = &self.definition.service_name();
        let method_name = &method.sig.ident;
        let (request_type, response_type) = extract_method_types(method);

        let mut code = String::new();

        code.push_str(&format!(
            "        \n        handler = self.handler  # Capture handler instance, not self\n",
        ));
        code.push_str(&format!(
            "        async def handle_{}(request_bytes: bytes) -> bytes:\n",
            method_name
        ));
        code.push_str("            # Deserialize request from MessagePack\n");
        code.push_str("            request_dict = rpcnet.msgpack_to_python_py(request_bytes)\n");
        code.push_str(&format!(
            "            request = {}(**request_dict)\n",
            request_type
        ));
        code.push_str("            \n");
        code.push_str("            # Call handler\n");
        code.push_str(&format!(
            "            response = await handler.{}(request)\n",
            method_name
        ));
        code.push_str("            \n");
        code.push_str("            # Serialize response to MessagePack\n");
        
        // Check if response type is an enum (any kind)
        if self.is_enum(&response_type) {
            let serializer = format!("serialize_{}", response_type.to_lowercase());
            code.push_str(&format!(
                "            response_dict = {}(response)\n",
                serializer
            ));
        } else {
            code.push_str("            response_dict = response.__dict__\n");
        }
        
        code.push_str("            return rpcnet.python_to_msgpack_py(response_dict)\n");
        code.push_str("        \n");
        code.push_str(&format!(
            "        await self.server.register('{}.{}', handle_{})\n",
            service_name, method_name, method_name
        ));

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
                "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128"
                | "isize" | "usize" => "int".to_string(),
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
                type_path
                    .path
                    .segments
                    .last()
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
                        if let Some(GenericArgument::Type(Type::Path(response_path))) =
                            args.args.first()
                        {
                            return (
                                request_type,
                                response_path
                                    .path
                                    .segments
                                    .last()
                                    .map(|s| s.ident.to_string())
                                    .unwrap_or_else(|| "Any".to_string()),
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

/// Check if a type is a Stream type (Pin<Box<dyn Stream<...>>>)
fn is_stream_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.first() {
            if segment.ident == "Pin" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(Type::Path(box_type))) = args.args.first() {
                        if let Some(box_segment) = box_type.path.segments.first() {
                            if box_segment.ident == "Box" {
                                if let PathArguments::AngleBracketed(box_args) =
                                    &box_segment.arguments
                                {
                                    if let Some(GenericArgument::Type(Type::TraitObject(
                                        trait_obj,
                                    ))) = box_args.args.first()
                                    {
                                        for bound in &trait_obj.bounds {
                                            if let syn::TypeParamBound::Trait(trait_bound) = bound {
                                                if let Some(trait_segment) =
                                                    trait_bound.path.segments.last()
                                                {
                                                    if trait_segment.ident == "Stream" {
                                                        return true;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// Extract the item type from a Stream<Item = T>
fn extract_stream_item_type(ty: &Type) -> Option<String> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.first() {
            if segment.ident == "Pin" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(Type::Path(box_type))) = args.args.first() {
                        if let Some(box_segment) = box_type.path.segments.first() {
                            if box_segment.ident == "Box" {
                                if let PathArguments::AngleBracketed(box_args) =
                                    &box_segment.arguments
                                {
                                    if let Some(GenericArgument::Type(Type::TraitObject(
                                        trait_obj,
                                    ))) = box_args.args.first()
                                    {
                                        for bound in &trait_obj.bounds {
                                            if let syn::TypeParamBound::Trait(trait_bound) = bound {
                                                if let Some(trait_segment) =
                                                    trait_bound.path.segments.last()
                                                {
                                                    if trait_segment.ident == "Stream" {
                                                        // Extract Item = T from Stream<Item = T>
                                                        if let PathArguments::AngleBracketed(
                                                            stream_args,
                                                        ) = &trait_segment.arguments
                                                        {
                                                            for arg in &stream_args.args {
                                                                if let GenericArgument::AssocType(
                                                                    assoc,
                                                                ) = arg
                                                                {
                                                                    if assoc.ident == "Item" {
                                                                        if let Type::Path(
                                                                            item_path,
                                                                        ) = &assoc.ty
                                                                        {
                                                                            // Check if it's Result<T, E>
                                                                            if let Some(
                                                                                result_segment,
                                                                            ) = item_path
                                                                                .path
                                                                                .segments
                                                                                .last()
                                                                            {
                                                                                if result_segment
                                                                                    .ident
                                                                                    == "Result"
                                                                                {
                                                                                    if let PathArguments::AngleBracketed(result_args) = &result_segment.arguments {
                                                                                        if let Some(GenericArgument::Type(Type::Path(ok_type))) = result_args.args.first() {
                                                                                            return ok_type.path.segments.last()
                                                                                                .map(|s| s.ident.to_string());
                                                                                        }
                                                                                    }
                                                                                } else {
                                                                                    // Not a Result, just return the type
                                                                                    return Some(result_segment.ident.to_string());
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Determine if a method is a streaming RPC method
fn is_streaming_method(method: &TraitItemFn) -> bool {
    // Check if the parameter (after &self) is a Stream
    let has_stream_input = if method.sig.inputs.len() >= 2 {
        if let syn::FnArg::Typed(pat_type) = &method.sig.inputs[1] {
            is_stream_type(&pat_type.ty)
        } else {
            false
        }
    } else {
        false
    };

    // Check if the return type contains a Stream
    let has_stream_output = if let syn::ReturnType::Type(_, ty) = &method.sig.output {
        if let Type::Path(type_path) = &**ty {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident == "Result" {
                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(GenericArgument::Type(ok_type)) = args.args.first() {
                            return is_stream_type(ok_type);
                        }
                    }
                }
            }
        }
        false
    } else {
        false
    };

    has_stream_input || has_stream_output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::ServiceDefinition;

    /// Test parsing and generating types for a simple service
    #[test]
    fn test_generate_simple_types() {
        let input = r#"
            use serde::{Serialize, Deserialize};

            #[derive(Serialize, Deserialize)]
            pub struct EchoRequest {
                pub message: String,
            }

            #[derive(Serialize, Deserialize)]
            pub struct EchoResponse {
                pub message: String,
            }

            #[service]
            pub trait EchoService {
                async fn echo(&self, request: EchoRequest) -> Result<EchoResponse, String>;
            }
        "#;

        let definition = ServiceDefinition::parse(input).expect("Failed to parse");
        let generator = PythonGenerator::new(definition);

        let types_code = generator.generate_types();

        // Should contain dataclass decorator
        assert!(types_code.contains("@dataclass"));
        assert!(types_code.contains("class EchoRequest:"));
        assert!(types_code.contains("class EchoResponse:"));
        assert!(types_code.contains("message: str"));
    }

    /// Test generating Python client code
    #[test]
    fn test_generate_client() {
        let input = r#"
            use serde::{Serialize, Deserialize};

            #[derive(Serialize, Deserialize)]
            pub struct PingRequest {
                pub id: u64,
            }

            #[derive(Serialize, Deserialize)]
            pub struct PingResponse {
                pub id: u64,
                pub timestamp: u64,
            }

            #[service]
            pub trait PingService {
                async fn ping(&self, request: PingRequest) -> Result<PingResponse, String>;
            }
        "#;

        let definition = ServiceDefinition::parse(input).expect("Failed to parse");
        let generator = PythonGenerator::new(definition);

        let client_code = generator.generate_client();

        // Should contain client class
        assert!(client_code.contains("class PingServiceClient:"));
        assert!(client_code.contains("async def connect("));
        assert!(client_code.contains("async def ping(self, request: PingRequest) -> PingResponse:"));

        // Should use MessagePack serialization
        assert!(client_code.contains("rpcnet.python_to_msgpack_py"));
        assert!(client_code.contains("rpcnet.msgpack_to_python_py"));

        // Should call the correct RPC method
        assert!(client_code.contains("'PingService.ping'"));
    }

    /// Test generating Python server code
    #[test]
    fn test_generate_server() {
        let input = r#"
            use serde::{Serialize, Deserialize};

            #[derive(Serialize, Deserialize)]
            pub struct GetRequest {
                pub key: String,
            }

            #[derive(Serialize, Deserialize)]
            pub struct GetResponse {
                pub value: String,
            }

            #[service]
            pub trait KeyValueService {
                async fn get(&self, request: GetRequest) -> Result<GetResponse, String>;
            }
        "#;

        let definition = ServiceDefinition::parse(input).expect("Failed to parse");
        let generator = PythonGenerator::new(definition);

        let server_code = generator.generate_server();

        // Should contain handler interface
        assert!(server_code.contains("class KeyValueServiceHandler(ABC):"));
        assert!(server_code.contains("@abstractmethod"));
        assert!(server_code.contains("async def get(self, request: GetRequest) -> GetResponse:"));

        // Should contain server class
        assert!(server_code.contains("class KeyValueServiceServer:"));
        assert!(server_code.contains("async def serve(self):"));
        assert!(server_code.contains("async def _register_handlers(self):"));
    }

    /// Test Rust type to Python type conversion
    #[test]
    fn test_rust_type_to_python() {
        let test_cases = vec![
            ("i32", "int"),
            ("u64", "int"),
            ("f64", "float"),
            ("bool", "bool"),
            ("String", "str"),
        ];

        for (rust_type, expected_python_type) in test_cases {
            let ty: Type = syn::parse_str(rust_type).unwrap();
            let python_type = rust_type_to_python(&ty);
            assert_eq!(
                python_type, expected_python_type,
                "Failed for {}",
                rust_type
            );
        }
    }

    /// Test Vec<T> conversion to List[T]
    #[test]
    fn test_vec_to_list_conversion() {
        let ty: Type = syn::parse_str("Vec<String>").unwrap();
        let python_type = rust_type_to_python(&ty);
        assert_eq!(python_type, "List[str]");

        let ty: Type = syn::parse_str("Vec<i32>").unwrap();
        let python_type = rust_type_to_python(&ty);
        assert_eq!(python_type, "List[int]");
    }

    /// Test Option<T> conversion to Optional[T]
    #[test]
    fn test_option_to_optional_conversion() {
        let ty: Type = syn::parse_str("Option<String>").unwrap();
        let python_type = rust_type_to_python(&ty);
        assert_eq!(python_type, "Optional[str]");

        let ty: Type = syn::parse_str("Option<i64>").unwrap();
        let python_type = rust_type_to_python(&ty);
        assert_eq!(python_type, "Optional[int]");
    }

    /// Test enum generation
    #[test]
    fn test_generate_enum() {
        let input = r#"
            use serde::{Serialize, Deserialize};

            #[derive(Serialize, Deserialize)]
            pub enum Status {
                Pending,
                Active,
                Completed,
            }

            #[derive(Serialize, Deserialize)]
            pub struct Request {}

            #[derive(Serialize, Deserialize)]
            pub struct Response {}

            #[service]
            pub trait TestService {
                async fn test(&self, request: Request) -> Result<Response, String>;
            }
        "#;

        let definition = ServiceDefinition::parse(input).expect("Failed to parse");
        let generator = PythonGenerator::new(definition);

        let types_code = generator.generate_types();

        assert!(types_code.contains("class Status(Enum):"));
        assert!(types_code.contains("PENDING = 0"));
        assert!(types_code.contains("ACTIVE = 1"));
        assert!(types_code.contains("COMPLETED = 2"));
    }

    /// Test streaming method detection
    #[test]
    fn test_is_streaming_method() {
        let streaming_input = r#"
            use futures::Stream;
            use std::pin::Pin;

            #[derive(Serialize, Deserialize)]
            pub struct Request {}

            #[derive(Serialize, Deserialize)]
            pub struct Response {}

            #[service]
            pub trait StreamingService {
                async fn generate(
                    &self,
                    request: Pin<Box<dyn Stream<Item = Request> + Send>>
                ) -> Result<Pin<Box<dyn Stream<Item = Response> + Send>>, String>;
            }
        "#;

        let definition = ServiceDefinition::parse(streaming_input).expect("Failed to parse");
        let methods = definition.methods();
        assert_eq!(methods.len(), 1);
        assert!(is_streaming_method(methods[0]));
    }

    /// Test regular method detection (non-streaming)
    #[test]
    fn test_is_not_streaming_method() {
        let input = r#"
            use serde::{Serialize, Deserialize};

            #[derive(Serialize, Deserialize)]
            pub struct Request {}

            #[derive(Serialize, Deserialize)]
            pub struct Response {}

            #[service]
            pub trait RegularService {
                async fn call(&self, request: Request) -> Result<Response, String>;
            }
        "#;

        let definition = ServiceDefinition::parse(input).expect("Failed to parse");
        let methods = definition.methods();
        assert_eq!(methods.len(), 1);
        assert!(!is_streaming_method(methods[0]));
    }

    /// Test streaming client method generation
    #[test]
    fn test_generate_streaming_client_method() {
        let input = r#"
            use futures::Stream;
            use std::pin::Pin;
            use serde::{Serialize, Deserialize};

            #[derive(Serialize, Deserialize)]
            pub struct InferenceRequest {
                pub prompt: String,
            }

            #[derive(Serialize, Deserialize)]
            pub struct InferenceResponse {
                pub text: String,
            }

            #[service]
            pub trait InferenceService {
                async fn generate(
                    &self,
                    request: Pin<Box<dyn Stream<Item = InferenceRequest> + Send>>
                ) -> Result<Pin<Box<dyn Stream<Item = Result<InferenceResponse, String>> + Send>>, String>;
            }
        "#;

        let definition = ServiceDefinition::parse(input).expect("Failed to parse");
        let generator = PythonGenerator::new(definition);

        let client_code = generator.generate_client();

        // Should have streaming signature with AsyncIterable and AsyncIterator
        assert!(client_code.contains("AsyncIterable"));
        assert!(client_code.contains("AsyncIterator"));
        assert!(client_code.contains("async def generate"));

        // Should collect request stream
        assert!(client_code.contains("async for request in request_stream:"));
        assert!(client_code.contains("request_list.append"));

        // Should call streaming RPC method
        assert!(client_code.contains("call_streaming"));

        // Should yield responses
        assert!(client_code.contains("async for response_bytes in response_stream:"));
        assert!(client_code.contains("yield"));
    }

    /// Test multiple methods in client generation
    #[test]
    fn test_generate_multiple_methods() {
        let input = r#"
            use serde::{Serialize, Deserialize};

            #[derive(Serialize, Deserialize)]
            pub struct GetRequest { pub key: String }

            #[derive(Serialize, Deserialize)]
            pub struct GetResponse { pub value: String }

            #[derive(Serialize, Deserialize)]
            pub struct SetRequest { pub key: String, pub value: String }

            #[derive(Serialize, Deserialize)]
            pub struct SetResponse { pub success: bool }

            #[service]
            pub trait KVStore {
                async fn get(&self, request: GetRequest) -> Result<GetResponse, String>;
                async fn set(&self, request: SetRequest) -> Result<SetResponse, String>;
            }
        "#;

        let definition = ServiceDefinition::parse(input).expect("Failed to parse");
        let generator = PythonGenerator::new(definition);

        let client_code = generator.generate_client();

        // Should have both methods
        assert!(client_code.contains("async def get(self, request: GetRequest) -> GetResponse:"));
        assert!(client_code.contains("async def set(self, request: SetRequest) -> SetResponse:"));

        // Should call correct RPC methods
        assert!(client_code.contains("'KVStore.get'"));
        assert!(client_code.contains("'KVStore.set'"));
    }

    /// Test extract_stream_item_type helper function
    #[test]
    fn test_extract_stream_item_type() {
        // Parse a Stream type
        let stream_type_str = "Pin<Box<dyn Stream<Item = MyType> + Send>>";
        let ty: Type = syn::parse_str(stream_type_str).unwrap();

        let item_type = extract_stream_item_type(&ty);
        assert_eq!(item_type, Some("MyType".to_string()));
    }

    /// Test extract_stream_item_type with Result wrapper
    #[test]
    fn test_extract_stream_item_type_with_result() {
        // Parse a Stream<Item = Result<T, E>> type
        let stream_type_str = "Pin<Box<dyn Stream<Item = Result<MyResponse, MyError>> + Send>>";
        let ty: Type = syn::parse_str(stream_type_str).unwrap();

        let item_type = extract_stream_item_type(&ty);
        // Should extract MyResponse from Result<MyResponse, MyError>
        assert_eq!(item_type, Some("MyResponse".to_string()));
    }

    /// Test doc comment extraction
    #[test]
    fn test_extract_doc_comment() {
        let input = r#"
            use serde::{Serialize, Deserialize};

            /// This is a request
            /// with multiple lines
            #[derive(Serialize, Deserialize)]
            pub struct Request {
                pub data: String,
            }

            #[derive(Serialize, Deserialize)]
            pub struct Response {
                pub result: String,
            }

            #[service]
            pub trait DocService {
                /// This method does something
                async fn do_something(&self, request: Request) -> Result<Response, String>;
            }
        "#;

        let definition = ServiceDefinition::parse(input).expect("Failed to parse");
        let generator = PythonGenerator::new(definition);

        let client_code = generator.generate_client();

        // Doc comments should be preserved in generated code
        assert!(client_code.contains("This method does something"));
    }

    /// Test server generation skips streaming methods
    #[test]
    fn test_server_skips_streaming_methods() {
        let input = r#"
            use futures::Stream;
            use std::pin::Pin;
            use serde::{Serialize, Deserialize};

            #[derive(Serialize, Deserialize)]
            pub struct Request {}

            #[derive(Serialize, Deserialize)]
            pub struct Response {}

            #[service]
            pub trait MixedService {
                async fn regular(&self, request: Request) -> Result<Response, String>;
                async fn streaming(
                    &self,
                    request: Pin<Box<dyn Stream<Item = Request> + Send>>
                ) -> Result<Pin<Box<dyn Stream<Item = Response> + Send>>, String>;
            }
        "#;

        let definition = ServiceDefinition::parse(input).expect("Failed to parse");
        let generator = PythonGenerator::new(definition);

        let server_code = generator.generate_server();

        // Should have regular method
        assert!(server_code.contains("async def regular"));

        // Should NOT have streaming method (not yet supported)
        assert!(!server_code.contains("async def streaming"));
    }

    /// Test is_stream_type helper function
    #[test]
    fn test_is_stream_type() {
        // Test that Pin<Box<dyn Stream<...>>> is detected
        let stream_type: Type =
            syn::parse_str("Pin<Box<dyn Stream<Item = String> + Send>>").unwrap();
        assert!(is_stream_type(&stream_type));

        // Test that regular types are not detected as streams
        let regular_type: Type = syn::parse_str("String").unwrap();
        assert!(!is_stream_type(&regular_type));

        let option_type: Type = syn::parse_str("Option<String>").unwrap();
        assert!(!is_stream_type(&option_type));
    }

    /// Test custom type handling
    #[test]
    fn test_custom_type_handling() {
        let input = r#"
            use serde::{Serialize, Deserialize};

            #[derive(Serialize, Deserialize)]
            pub struct CustomType {
                pub field: String,
            }

            #[derive(Serialize, Deserialize)]
            pub struct Request {
                pub custom: CustomType,
            }

            #[derive(Serialize, Deserialize)]
            pub struct Response {}

            #[service]
            pub trait CustomService {
                async fn process(&self, request: Request) -> Result<Response, String>;
            }
        "#;

        let definition = ServiceDefinition::parse(input).expect("Failed to parse");
        let generator = PythonGenerator::new(definition);

        let types_code = generator.generate_types();

        // Should generate both custom types
        assert!(types_code.contains("class CustomType:"));
        assert!(types_code.contains("class Request:"));

        // Request should reference CustomType
        assert!(types_code.contains("custom: CustomType"));
    }
}
