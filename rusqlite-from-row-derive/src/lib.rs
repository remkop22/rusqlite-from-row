use std::borrow::Cow;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    meta::ParseNestedMeta, parse_macro_input, parse_str, spanned::Spanned, Attribute, Data,
    DataStruct, DeriveInput, Error, Field, Fields, LitStr, Result, Type,
};

/// Calls the fallible entry point and writes any errors to the tokenstream.
#[proc_macro_derive(FromRow, attributes(from_row))]
pub fn derive_from_row(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);

    try_derive_from_row(derive_input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// Fallible entry point for generating a `FromRow` implementation
fn try_derive_from_row(input: DeriveInput) -> Result<TokenStream2> {
    let from_row_derive = DeriveFromRow::parse(input)?;

    Ok(from_row_derive.generate())
}

/// Main struct for deriving `FromRow` for a struct.
struct DeriveFromRow {
    ident: syn::Ident,
    generics: syn::Generics,
    data: Vec<FromRowField>,
}

impl DeriveFromRow {
    fn parse(input: DeriveInput) -> Result<Self> {
        let DeriveInput {
            ident,
            generics,
            data:
                Data::Struct(DataStruct {
                    fields: Fields::Named(fields),
                    ..
                }),
            ..
        } = input
        else {
            return Err(Error::new(
                input.span(),
                "expected struct with named fields",
            ));
        };

        let mut data = Vec::new();

        for field in fields.named {
            data.push(FromRowField::parse(field)?);
        }

        Ok(Self {
            ident,
            generics,
            data,
        })
    }

    fn predicates(&self) -> Vec<TokenStream2> {
        let mut predicates = Vec::new();

        for field in &self.data {
            field.add_predicates(&mut predicates);
        }

        predicates
    }

    /// Generate the `FromRow` implementation.
    fn generate(self) -> TokenStream2 {
        let ident = &self.ident;

        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        let original_predicates = where_clause.map(|w| &w.predicates).into_iter();
        let predicates = self.predicates();

        let is_all_null_fields = self.data.iter().map(|f| f.generate_is_all_null());

        let try_from_row_fields = self.data.iter().map(|f| f.generate_try_from_row());

        quote! {
            impl #impl_generics rusqlite_from_row::FromRow for #ident #ty_generics where #(#original_predicates),* #(#predicates),* {
                fn try_from_row_prefixed(
                    row: &rusqlite_from_row::rusqlite::Row,
                    prefix: Option<&str>
                ) -> std::result::Result<Self, rusqlite_from_row::rusqlite::Error> {
                    Ok(Self {
                        #(#try_from_row_fields),*
                    })
                }

                fn is_all_null(
                    row: &rusqlite_from_row::rusqlite::Row,
                    prefix: Option<&str>
                ) -> std::result::Result<bool, rusqlite_from_row::rusqlite::Error> {
                    Ok(#(#is_all_null_fields)&&*)
                }
            }
        }
    }
}

/// A single field inside of a struct that derives `FromRow`
struct FromRowField {
    /// The identifier of this field.
    ident: syn::Ident,
    /// The type specified in this field.
    ty: syn::Type,
    attrs: FromRowAttrs,
}

impl FromRowField {
    pub fn parse(field: Field) -> Result<Self> {
        let mut attrs = FromRowAttrs::default();

        attrs.parse(field.attrs)?;

        Ok(Self {
            ident: field.ident.expect("should be named"),
            ty: field.ty,
            attrs,
        })
    }

    /// Returns a tokenstream of the type that should be returned from either
    /// `FromRow` (when using `flatten`) or `FromSql`.
    fn target_ty(&self) -> &Type {
        if let Some(from) = &self.attrs.from {
            from
        } else if let Some(try_from) = &self.attrs.try_from {
            try_from
        } else {
            &self.ty
        }
    }

    /// Returns the name that maps to the actuall sql column
    /// By default this is the same as the rust field name but can be overwritten by `#[from_row(rename = "..")]`.
    fn column_name(&self) -> Cow<str> {
        self.attrs
            .rename
            .as_ref()
            .map(Cow::from)
            .unwrap_or_else(|| self.ident.to_string().into())
    }

    /// Pushes the needed where clause predicates for this field.
    ///
    /// By default this is `T: rusqlite::types::FromSql`,
    /// when using `flatten` it's: `T: rusqlite_from_row::FromRow`
    /// and when using either `from` or `try_from` attributes it additionally pushes this bound:
    /// `T: std::convert::From<R>`, where `T` is the type specified in the struct and `R` is the
    /// type specified in the `[try]_from` attribute.
    fn add_predicates(&self, predicates: &mut Vec<TokenStream2>) {
        let target_ty = self.target_ty();
        let ty = &self.ty;

        predicates.push(if self.attrs.flatten {
            quote! (#target_ty: rusqlite_from_row::FromRow)
        } else {
            quote! (#target_ty: rusqlite_from_row::rusqlite::types::FromSql)
        });

        if self.attrs.from.is_some() {
            predicates.push(quote!(#ty: std::convert::From<#target_ty>))
        } else if self.attrs.try_from.is_some() {
            let try_from = quote!(std::convert::TryFrom<#target_ty>);

            predicates.push(quote!(#ty: #try_from));
            predicates.push(quote!(rusqlite_from_row::rusqlite::Error: std::convert::From<<#ty as #try_from>::Error>));
            predicates.push(quote!(<#ty as #try_from>::Error: std::fmt::Debug));
        }
    }

    fn generate_is_all_null(&self) -> TokenStream2 {
        let target_ty = self.target_ty();

        if self.attrs.flatten {
            let prefix = match &self.attrs.prefix {
                Some(Prefix::Value(prefix)) => {
                    quote!(Some(&(prefix.unwrap_or("").to_string() + #prefix)))
                }
                Some(Prefix::Field) => {
                    let ident_str = format!("{}_", self.ident);
                    quote!(Some(&(prefix.unwrap_or("").to_string() + #ident_str)))
                }
                None => quote!(prefix),
            };

            quote!(<#target_ty as rusqlite_from_row::FromRow>::is_all_null(row, #prefix)?)
        } else {
            let column_name = self.column_name();

            quote! {
                rusqlite_from_row::rusqlite::Row::get_ref::<&str>(
                    row,
                    &(prefix.unwrap_or("").to_string() + #column_name)
                )? == rusqlite_from_row::rusqlite::types::ValueRef::Null
            }
        }
    }

    /// Generate the line needed to retrieve this field from a row when calling `try_from_row`.
    fn generate_try_from_row(&self) -> TokenStream2 {
        let ident = &self.ident;
        let column_name = self.column_name();
        let field_ty = &self.ty;
        let target_ty = self.target_ty();

        let mut base = if self.attrs.flatten {
            let prefix = match &self.attrs.prefix {
                Some(Prefix::Value(prefix)) => {
                    quote!(Some(&(prefix.unwrap_or("").to_string() + #prefix)))
                }
                Some(Prefix::Field) => {
                    let ident_str = format!("{}_", self.ident);
                    quote!(Some(&(prefix.unwrap_or("").to_string() + #ident_str)))
                }
                None => quote!(prefix),
            };

            quote!(<#target_ty as rusqlite_from_row::FromRow>::try_from_row_prefixed(row, #prefix)?)
        } else {
            quote!(rusqlite_from_row::rusqlite::Row::get::<&str, #target_ty>(row, &(prefix.unwrap_or("").to_string() + #column_name))?)
        };

        if self.attrs.from.is_some() {
            base = quote!(<#field_ty as std::convert::From<#target_ty>>::from(#base));
        } else if self.attrs.try_from.is_some() {
            base = quote!(<#field_ty as std::convert::TryFrom<#target_ty>>::try_from(#base)?);
        };

        quote!(#ident: #base)
    }
}

#[derive(Default)]
struct FromRowAttrs {
    /// Wether to flatten this field. Flattening means calling the `FromRow` implementation
    /// of `self.ty` instead of extracting it directly from the row.
    flatten: bool,
    /// Can only be used in combination with flatten. Will prefix all fields of the nested struct
    /// with this string. Can be useful for joins with overlapping names.
    prefix: Option<Prefix>,
    /// Optionaly use this type as the target for `FromRow` or `FromSql`, and then
    /// call `TryFrom::try_from` to convert it the `self.ty`.
    try_from: Option<Type>,
    /// Optionaly use this type as the target for `FromRow` or `FromSql`, and then
    /// call `From::from` to convert it the `self.ty`.
    from: Option<Type>,
    /// Override the name of the actual sql column instead of using `self.ident`.
    /// Is not compatible with `flatten` since no column is needed there.
    rename: Option<String>,
}

enum Prefix {
    Value(String),
    Field,
}

impl FromRowAttrs {
    fn parse(&mut self, attrs: Vec<Attribute>) -> Result<()> {
        for attr in attrs {
            if !attr.meta.path().is_ident("from_row") {
                continue;
            }

            attr.parse_nested_meta(|meta| self.parse_one(meta))?;
        }

        Ok(())
    }

    fn parse_one(&mut self, meta: ParseNestedMeta) -> Result<()> {
        if meta.path.is_ident("flatten") {
            self.flatten = true;
        } else if meta.path.is_ident("prefix") {
            let prefix = if let Ok(value) = meta.value() {
                Prefix::Value(value.parse::<LitStr>()?.value())
            } else {
                Prefix::Field
            };

            self.prefix = Some(prefix);
        } else if meta.path.is_ident("try_from") {
            let try_from: LitStr = meta.value()?.parse()?;
            self.try_from = Some(parse_str(&try_from.value())?);
        } else if meta.path.is_ident("from") {
            let from: LitStr = meta.value()?.parse()?;
            self.from = Some(parse_str(&from.value())?);
        } else if meta.path.is_ident("rename") {
            let rename: LitStr = meta.value()?.parse()?;
            self.rename = Some(rename.value());
        }

        Ok(())
    }
}
