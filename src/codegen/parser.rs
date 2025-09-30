//! Parser for service definitions using syn.

use std::collections::HashMap;
use syn::{Error, File, Item, ItemEnum, ItemStruct, ItemTrait, Result, TraitItem};

/// Represents a parsed service definition.
#[derive(Debug)]
pub struct ServiceDefinition {
    /// The service trait with methods.
    pub service_trait: ItemTrait,
    /// All type definitions (structs and enums) in the file.
    pub types: HashMap<String, ServiceType>,
    /// Import statements.
    pub imports: Vec<syn::ItemUse>,
}

/// Represents a type definition in the service file.
#[derive(Debug)]
pub enum ServiceType {
    Struct(ItemStruct),
    Enum(ItemEnum),
}

impl ServiceDefinition {
    /// Parses a service definition from Rust source code.
    pub fn parse(content: &str) -> Result<Self> {
        // Parse the entire file using syn
        let ast: File = syn::parse_str(content)?;

        let mut service_trait = None;
        let mut types = HashMap::new();
        let mut imports = Vec::new();

        // Iterate through all items in the file
        for item in ast.items {
            match item {
                Item::Trait(trait_item) => {
                    // Check if this trait has our service attribute
                    if has_rpcnet_service_attribute(&trait_item) {
                        if service_trait.is_some() {
                            return Err(Error::new_spanned(
                                &trait_item,
                                "Multiple service traits found. Only one service per file is supported."
                            ));
                        }

                        // Validate trait methods
                        validate_trait_methods(&trait_item)?;
                        service_trait = Some(trait_item);
                    }
                }
                Item::Struct(struct_item) => {
                    // Collect all structs as potential request/response types
                    types.insert(
                        struct_item.ident.to_string(),
                        ServiceType::Struct(struct_item),
                    );
                }
                Item::Enum(enum_item) => {
                    // Collect all enums (typically error types)
                    types.insert(enum_item.ident.to_string(), ServiceType::Enum(enum_item));
                }
                Item::Use(use_item) => {
                    imports.push(use_item);
                }
                _ => {} // Ignore other items
            }
        }

        let service_trait = service_trait.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "No service trait found. Add #[rpcnet::service] attribute to your trait.",
            )
        })?;

        Ok(ServiceDefinition {
            service_trait,
            types,
            imports,
        })
    }

    /// Gets the service name.
    pub fn service_name(&self) -> &syn::Ident {
        &self.service_trait.ident
    }

    /// Gets all methods from the service trait.
    pub fn methods(&self) -> Vec<&syn::TraitItemFn> {
        self.service_trait
            .items
            .iter()
            .filter_map(|item| {
                if let TraitItem::Fn(method) = item {
                    Some(method)
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Checks if a trait has the #[rpcnet::service] attribute.
fn has_rpcnet_service_attribute(trait_item: &ItemTrait) -> bool {
    trait_item.attrs.iter().any(|attr| {
        // Check for both #[rpcnet::service] and #[service] (assuming use rpcnet::service)
        if attr.path().is_ident("service") {
            return true;
        }

        if attr.path().segments.len() == 2 {
            let segments: Vec<_> = attr.path().segments.iter().collect();
            segments[0].ident == "rpcnet" && segments[1].ident == "service"
        } else {
            false
        }
    })
}

/// Validates that all trait methods follow the expected pattern.
fn validate_trait_methods(trait_item: &ItemTrait) -> Result<()> {
    for item in &trait_item.items {
        if let TraitItem::Fn(method) = item {
            // Check that method is async
            if method.sig.asyncness.is_none() {
                return Err(Error::new_spanned(
                    &method.sig,
                    "Service methods must be async",
                ));
            }

            // Check that method has &self as first parameter
            if method.sig.inputs.is_empty() {
                return Err(Error::new_spanned(
                    &method.sig,
                    "Service methods must have &self as first parameter",
                ));
            }

            // Validate return type is Result<T, E>
            match &method.sig.output {
                syn::ReturnType::Type(_, ty) => {
                    // Simple check - could be more sophisticated
                    let type_str = quote::quote!(#ty).to_string();
                    if !type_str.contains("Result") {
                        return Err(Error::new_spanned(
                            ty,
                            "Service methods must return Result<Response, Error>",
                        ));
                    }
                }
                syn::ReturnType::Default => {
                    return Err(Error::new_spanned(
                        &method.sig,
                        "Service methods must return Result<Response, Error>",
                    ));
                }
            }
        }
    }

    Ok(())
}
