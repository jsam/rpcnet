//! Code generator using quote and proc-macro2.

use crate::codegen::parser::{ServiceDefinition, ServiceType};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, GenericArgument, PathArguments, ReturnType, Type};

/// Code generator for creating server and client implementations.
pub struct CodeGenerator {
    definition: ServiceDefinition,
}

impl CodeGenerator {
    /// Creates a new code generator with the given service definition.
    pub fn new(definition: ServiceDefinition) -> Self {
        Self { definition }
    }

    /// Generates the server implementation.
    pub fn generate_server(&self) -> TokenStream {
        let trait_name = &self.definition.service_trait.ident;
        let server_name = format_ident!("{}Server", trait_name);
        let handler_trait = format_ident!("{}Handler", trait_name);

        let methods = self.definition.methods();
        let handler_methods = self.generate_handler_methods(&methods);
        let register_methods = self.generate_register_methods(&methods, trait_name);

        quote! {
            use super::types::*;
            use rpcnet::{RpcServer, RpcConfig, RpcError};
            use async_trait::async_trait;
            use std::sync::Arc;

            /// Handler trait that users implement for the service.
            #[async_trait]
            pub trait #handler_trait: Send + Sync + 'static {
                #(#handler_methods)*
            }

            /// Generated server that manages RPC registration and routing.
            pub struct #server_name<H: #handler_trait> {
                handler: Arc<H>,
                rpc_server: RpcServer,
            }

            impl<H: #handler_trait> #server_name<H> {
                /// Creates a new server with the given handler and configuration.
                pub fn new(handler: H, config: RpcConfig) -> Self {
                    Self {
                        handler: Arc::new(handler),
                        rpc_server: RpcServer::new(config),
                    }
                }

                /// Registers all service methods with the RPC server.
                pub async fn register_all(&mut self) {
                    #(#register_methods)*
                }

                /// Starts the server and begins accepting connections.
                pub async fn serve(mut self) -> Result<(), RpcError> {
                    self.register_all().await;
                    let quic_server = self.rpc_server.bind()?;
                    println!("Server listening on: {:?}", self.rpc_server.socket_addr);
                    self.rpc_server.start(quic_server).await
                }
            }
        }
    }

    /// Generates the client implementation.
    pub fn generate_client(&self) -> TokenStream {
        let trait_name = &self.definition.service_trait.ident;
        let client_name = format_ident!("{}Client", trait_name);

        let methods = self.definition.methods();
        let client_methods = self.generate_client_methods(&methods, trait_name);

        quote! {
            use super::types::*;
            use rpcnet::{RpcClient, RpcConfig, RpcError};
            use std::net::SocketAddr;

            /// Generated client for calling service methods.
            pub struct #client_name {
                inner: RpcClient,
            }

            impl #client_name {
                /// Connects to the service at the given address.
                pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
                    let inner = RpcClient::connect(addr, config).await?;
                    Ok(Self { inner })
                }

                #(#client_methods)*
            }
        }
    }

    /// Generates type definitions.
    pub fn generate_types(&self) -> TokenStream {
        let mut type_tokens = Vec::new();

        // Add imports
        for import in &self.definition.imports {
            type_tokens.push(quote! { #import });
        }

        // Add type definitions
        for (_name, service_type) in &self.definition.types {
            match service_type {
                ServiceType::Struct(item_struct) => {
                    type_tokens.push(quote! { #item_struct });
                }
                ServiceType::Enum(item_enum) => {
                    type_tokens.push(quote! { #item_enum });
                }
            }
        }

        quote! {
            //! Type definitions for the service.

            #(#type_tokens)*
        }
    }

    fn generate_handler_methods(&self, methods: &[&syn::TraitItemFn]) -> Vec<TokenStream> {
        methods
            .iter()
            .map(|method| {
                let sig = &method.sig;
                // For the handler trait, we need the exact signature from the service trait
                quote! {
                    #sig;
                }
            })
            .collect()
    }

    fn generate_register_methods(
        &self,
        methods: &[&syn::TraitItemFn],
        service_name: &syn::Ident,
    ) -> Vec<TokenStream> {
        methods
            .iter()
            .map(|method| {
                let method_name = &method.sig.ident;
                let method_str = method_name.to_string();
                let full_method_name = format!("{}.{}", service_name, method_str);

                // Extract request type from method signature
                let request_type = self.extract_request_type(method);

                quote! {
                    {
                        let handler = self.handler.clone();
                        self.rpc_server.register(#full_method_name, move |params| {
                            let handler = handler.clone();
                            async move {
                                let request: #request_type = bincode::deserialize(&params)
                                    .map_err(RpcError::SerializationError)?;

                                match handler.#method_name(request).await {
                                    Ok(response) => {
                                        bincode::serialize(&response)
                                            .map_err(RpcError::SerializationError)
                                    }
                                    Err(e) => {
                                        Err(RpcError::StreamError(format!("{:?}", e)))
                                    }
                                }
                            }
                        }).await;
                    }
                }
            })
            .collect()
    }

    fn generate_client_methods(
        &self,
        methods: &[&syn::TraitItemFn],
        service_name: &syn::Ident,
    ) -> Vec<TokenStream> {
        methods
            .iter()
            .map(|method| {
                let method_name = &method.sig.ident;
                let method_str = method_name.to_string();
                let full_method_name = format!("{}.{}", service_name, method_str);

                let request_type = self.extract_request_type(method);
                let (response_type, _error_type) = self.extract_result_types(method);

                // Build the method signature for the client
                let mut client_sig = method.sig.clone();
                // Remove &self and replace with &self (client reference)
                if !client_sig.inputs.is_empty() {
                    client_sig.inputs[0] = syn::parse_quote!(&self);
                }
                // Change return type to Result<ResponseType, RpcError>
                client_sig.output = syn::parse_quote!(-> Result<#response_type, RpcError>);

                quote! {
                    pub #client_sig {
                        let params = bincode::serialize(&request)
                            .map_err(RpcError::SerializationError)?;

                        let response_data = self.inner.call(#full_method_name, params).await?;

                        // Deserialize the response
                        bincode::deserialize::<#response_type>(&response_data)
                            .map_err(RpcError::SerializationError)
                    }
                }
            })
            .collect()
    }

    fn extract_request_type(&self, method: &syn::TraitItemFn) -> TokenStream {
        // Get the second parameter (first is &self)
        if let Some(FnArg::Typed(pat_type)) = method.sig.inputs.iter().nth(1) {
            let ty = &pat_type.ty;
            quote! { #ty }
        } else {
            quote! { () }
        }
    }

    fn extract_result_types(&self, method: &syn::TraitItemFn) -> (TokenStream, TokenStream) {
        // Parse the return type to extract T and E from Result<T, E>
        if let ReturnType::Type(_, ty) = &method.sig.output {
            if let Type::Path(type_path) = &**ty {
                if let Some(segment) = type_path.path.segments.last() {
                    if segment.ident == "Result" {
                        if let PathArguments::AngleBracketed(args) = &segment.arguments {
                            let mut args_iter = args.args.iter();

                            // Get T (response type)
                            let response_type =
                                if let Some(GenericArgument::Type(t)) = args_iter.next() {
                                    quote! { #t }
                                } else {
                                    quote! { () }
                                };

                            // Get E (error type)
                            let error_type =
                                if let Some(GenericArgument::Type(e)) = args_iter.next() {
                                    quote! { #e }
                                } else {
                                    quote! { String }
                                };

                            return (response_type, error_type);
                        }
                    }
                }
            }
        }

        (quote! { () }, quote! { String })
    }
}

#[cfg(all(test, feature = "codegen"))]
mod tests {
    use super::*;
    use quote::quote;
    use syn::parse_quote;

    fn sample_generator() -> CodeGenerator {
        let input = r#"
            #[rpcnet::service]
            pub trait SampleService {
                async fn do_work(&self, request: WorkRequest) -> Result<WorkResponse, WorkError>;
            }

            pub struct WorkRequest;
            pub struct WorkResponse;
            pub enum WorkError { Failed }
        "#;

        let definition = ServiceDefinition::parse(input).expect("failed to parse sample service");
        CodeGenerator::new(definition)
    }

    #[test]
    fn extract_request_type_returns_declared_type() {
        let generator = sample_generator();
        let method = generator
            .definition
            .methods()
            .into_iter()
            .next()
            .expect("expected method");

        let ty = generator.extract_request_type(method);
        assert_eq!(ty.to_string(), quote!(WorkRequest).to_string());
    }

    #[test]
    fn extract_request_type_defaults_to_unit_when_missing_parameter() {
        let generator = sample_generator();
        let method: syn::TraitItemFn = parse_quote! {
            async fn heartbeat(&self) -> Result<(), WorkError>;
        };

        let ty = generator.extract_request_type(&method);
        assert_eq!(ty.to_string(), quote!(()).to_string());
    }

    #[test]
    fn extract_result_types_returns_response_and_error() {
        let generator = sample_generator();
        let method = generator
            .definition
            .methods()
            .into_iter()
            .next()
            .expect("expected method");

        let (response, error) = generator.extract_result_types(method);
        assert_eq!(response.to_string(), quote!(WorkResponse).to_string());
        assert_eq!(error.to_string(), quote!(WorkError).to_string());
    }

    #[test]
    fn extract_result_types_defaults_when_return_type_not_result() {
        let generator = sample_generator();
        let method: syn::TraitItemFn = parse_quote! {
            async fn metrics(&self) -> ();
        };

        let (response, error) = generator.extract_result_types(&method);
        assert_eq!(response.to_string(), quote!(()).to_string());
        assert_eq!(error.to_string(), quote!(String).to_string());
    }
}
