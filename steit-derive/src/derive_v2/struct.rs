use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::ToTokens;

use crate::{
    attr::{Attr, AttrParse, VecAttr},
    context::Context,
    impler::Impler,
};

use super::{
    derive::{self, DeriveSetting},
    field::{DeriveField, Field},
    variant::Variant,
};

struct StructAttrs {
    no_size_cache: bool,

    size_cache_renamed: Option<(String, TokenStream)>,
    runtime_renamed: Option<(String, TokenStream)>,

    reserved: Vec<u32>,
}

impl StructAttrs {
    pub fn parse(context: &Context, attrs: impl AttrParse) -> Self {
        let mut no_size_cache = Attr::new(context, "no_size_cache");

        let mut size_cache_renamed = Attr::new(context, "size_cache_renamed");
        let mut runtime_renamed = Attr::new(context, "runtime_renamed");

        let mut reserved = VecAttr::new(context, "reserved");

        attrs.parse(context, true, |meta| match meta {
            syn::Meta::Path(path) if no_size_cache.parse_path(path) => true,
            syn::Meta::NameValue(meta) if no_size_cache.parse_bool(meta) => true,

            syn::Meta::NameValue(meta) if size_cache_renamed.parse_str(meta) => true,
            syn::Meta::NameValue(meta) if runtime_renamed.parse_str(meta) => true,

            syn::Meta::List(meta) if reserved.parse_int_list(meta) => true,

            _ => false,
        });

        Self {
            no_size_cache: no_size_cache.get().unwrap_or_default(),

            size_cache_renamed: size_cache_renamed.get_with_tokens(),
            runtime_renamed: runtime_renamed.get_with_tokens(),

            reserved: reserved.get(),
        }
    }
}

pub struct Struct<'a> {
    impler: &'a Impler<'a>,
    setting: &'a DeriveSetting,
    fields: Vec<DeriveField<'a>>,
    size_cache: Option<Field>,
    runtime: Option<Field>,
    variant: Option<Variant>,
}

macro_rules! map_fields {
    ($struct:ident, _.$($tail:tt)*) => {
        $struct.fields.iter().map(|field| field.$($tail)*)
    };
}

impl<'a> Struct<'a> {
    pub fn parse(
        context: &'a Context,
        impler: &'a Impler,
        setting: &'a DeriveSetting,
        attrs: impl AttrParse,
        fields: &mut syn::Fields,
        variant: Option<Variant>,
    ) -> derive::Result<Self> {
        let attrs = StructAttrs::parse(context, attrs);
        let parsed_fields = parse_fields(context, setting, &attrs, fields)?;

        let krate = setting.krate();
        let mut field_index = parsed_fields.len();

        let size_cache = if setting.has_size_cache() && !attrs.no_size_cache {
            Some(add_field(
                fields,
                attrs
                    .size_cache_renamed
                    .or(setting.size_cache_renamed.clone())
                    .map(|(name, _)| name)
                    .unwrap_or("size_cache".to_owned()),
                syn::parse_quote!(#krate::runtime::SizeCache),
                {
                    field_index += 1;
                    field_index - 1
                },
            ))
        } else {
            None
        };

        let runtime = if setting.has_runtime() {
            Some(add_field(
                fields,
                attrs
                    .runtime_renamed
                    .or(setting.runtime_renamed.clone())
                    .map(|(name, _)| name)
                    .unwrap_or("runtime".to_owned()),
                syn::parse_quote!(#krate::runtime::Runtime),
                {
                    field_index += 1;
                    field_index - 1
                },
            ))
        } else {
            None
        };

        Ok(Self {
            impler,
            setting,
            fields: parsed_fields,
            size_cache,
            runtime,
            variant,
        })
    }

    pub fn variant(&self) -> Option<&Variant> {
        self.variant.as_ref()
    }

    pub fn size_cache(&self) -> Option<&Field> {
        self.size_cache.as_ref()
    }

    pub fn runtime(&self) -> Option<&Field> {
        self.runtime.as_ref()
    }

    fn trait_bounds(&self, fallback: &'static [&str]) -> &[&str] {
        if self.setting.impl_state() {
            &["State"]
        } else {
            fallback
        }
    }

    pub fn ctor_name(&self) -> syn::Ident {
        match &self.variant {
            Some(variant) => variant.ctor_name(),
            None => format_ident!("empty"),
        }
    }

    pub fn destructure(&self) -> TokenStream {
        let destructure = map_fields!(self, _.destructure_alias());
        quote!(#(#destructure,)*)
    }

    pub fn ctor(&self) -> TokenStream {
        let ctor_name = self.ctor_name();
        let name = self.impler.name();
        let qual = self.variant().map(|variant| variant.qual());
        let mut inits: Vec<_> = map_fields!(self, _.init_default()).collect();

        if let Some(size_cache) = self.size_cache() {
            inits.push(size_cache.init(quote!(SizeCache::new())));
        }

        let (params, set_variant_runtime) = if let Some(runtime) = self.runtime() {
            inits.push(runtime.init(quote!(runtime)));

            (
                Some(quote!(runtime: Runtime)),
                self.variant().map(|variant| {
                    let tag = variant.tag();
                    quote! { let runtime = runtime.nested(#tag as u16); }
                }),
            )
        } else {
            Default::default()
        };

        quote! {
            #[inline]
            pub fn #ctor_name(#params) -> Self {
                #set_variant_runtime
                #name #qual { #(#inits,)* }
            }
        }
    }

    fn impl_ctor(&self) -> TokenStream {
        self.impler
            .impl_with(self.trait_bounds(&["Default"]), self.ctor())
    }

    fn impl_default(&self) -> TokenStream {
        let args = if self.setting.impl_state() {
            Some(quote!(Runtime::default()))
        } else {
            None
        };

        self.impler.impl_for_with(
            "Default",
            self.trait_bounds(&["Default"]),
            quote! {
                #[inline]
                fn default() -> Self {
                    Self::empty(#args)
                }
            },
        )
    }

    fn impl_wire_type(&self) -> TokenStream {
        self.impler.impl_for_with(
            "HasWireType",
            &[],
            quote! {
                const WIRE_TYPE: WireTypeV2 = WireTypeV2::Sized;
            },
        )
    }

    pub fn sizer(&self) -> TokenStream {
        let is_variant = self.variant.is_some();
        let sizers = map_fields!(self, _.sizer(is_variant));
        quote!(#(#sizers)*)
    }

    pub fn serializer(&self) -> TokenStream {
        let is_variant = self.variant.is_some();
        let serializers = map_fields!(self, _.serializer(is_variant));
        quote!(#(#serializers)*)
    }

    fn impl_serialize(&self) -> TokenStream {
        let sizer = self.sizer();
        let serializer = self.serializer();

        let size_cache = if let Some(size_cache) = &self.size_cache {
            let size_cache = size_cache.field(false);
            quote!(Some(&#size_cache))
        } else {
            quote!(None)
        };

        self.impler.impl_for(
            "SerializeV2",
            quote! {
                fn compute_size(&self) -> u32 {
                    let mut size = 0;
                    #sizer
                    size
                }

                fn serialize_cached(&self, writer: &mut impl io::Write) -> io::Result<()> {
                    #serializer
                    Ok(())
                }

                #[inline]
                fn size_cache(&self) -> Option<&SizeCache> {
                    #size_cache
                }
            },
        )
    }

    pub fn merger(&self) -> TokenStream {
        let is_variant = self.variant.is_some();
        let mergers = map_fields!(self, _.merger(is_variant));

        quote! {
            while !reader.eof()? {
                let (field_number, wire_type) = reader.read_tag()?;

                match field_number {
                    #(#mergers,)*
                    _ => reader.skip_field(wire_type)?,
                }
            }
        }
    }

    fn impl_merge(&self) -> TokenStream {
        let merger = self.merger();

        self.impler.impl_for(
            "MergeV2",
            quote! {
                fn merge_v2(&mut self, reader: &mut Reader<impl io::Read>) -> io::Result<()> {
                    #merger
                    Ok(())
                }
            },
        )
    }

    pub fn runtime_setter(&self) -> TokenStream {
        let is_variant = self.variant.is_some();
        let runtime_setters = map_fields!(self, _.runtime_setter(is_variant));

        if is_variant {
            quote! {
                #(#runtime_setters)*
                *self_runtime = runtime;
            }
        } else {
            let runtime = self.runtime().unwrap().access();

            quote! {
                #(#runtime_setters)*
                self.#runtime = runtime;
            }
        }
    }

    fn impl_state(&self) -> TokenStream {
        let runtime = self.runtime().unwrap().field(false);
        let runtime_setter = self.runtime_setter();

        self.impler.impl_for(
            "StateV2",
            quote! {
                #[inline]
                fn with_runtime(runtime: Runtime) -> Self {
                    Self::new(runtime)
                }

                #[inline]
                fn runtime(&self) -> &Runtime {
                    &#runtime
                }

                fn set_runtime(&mut self, runtime: Runtime) {
                    #runtime_setter
                }
            },
        )
    }
}

fn parse_fields<'a>(
    context: &Context,
    setting: &'a DeriveSetting,
    attrs: &StructAttrs,
    fields: &mut syn::Fields,
) -> derive::Result<Vec<DeriveField<'a>>> {
    let mut parsed_fields = Vec::with_capacity(fields.iter().len());

    let reserved_tags: HashSet<_> = attrs.reserved.iter().collect();
    let mut tags = HashSet::new();
    let mut unique_tags = true;

    for (index, field) in fields.iter_mut().enumerate() {
        if let Ok(parsed_field) = DeriveField::parse(setting, context, field, index) {
            let (tag, tag_tokens) = parsed_field.tag_with_tokens();

            if reserved_tags.contains(&tag) {
                context.error(tag_tokens, format!("tag {} has been reserved", tag));
            }

            if !tags.insert(tag) {
                context.error(tag_tokens, format!("duplicate tag {}", tag));
                unique_tags = false;
            }

            parsed_fields.push(parsed_field);
        }
    }

    if parsed_fields.len() == parsed_fields.capacity() && unique_tags {
        Ok(parsed_fields)
    } else {
        Err(())
    }
}

fn add_field(fields: &mut syn::Fields, name: String, ty: syn::Type, index: usize) -> Field {
    if let syn::Fields::Unit = fields {
        *fields = syn::Fields::Named(syn::parse_quote!({}));
    }

    match fields {
        syn::Fields::Named(fields) => {
            let field = Field::new(Some(format_ident!("{}", name)), ty, index);
            fields.named.extend(field.declare());
            field
        }

        syn::Fields::Unnamed(fields) => {
            let field = Field::new(None, ty, index);
            fields.unnamed.extend(field.declare());
            field
        }

        syn::Fields::Unit => unreachable!("unexpected unit fields"),
    }
}

impl<'a> ToTokens for Struct<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.impl_ctor());

        if self.setting.impl_default() {
            tokens.extend(self.impl_default());
        }

        tokens.extend(self.impl_wire_type());

        if self.setting.impl_serialize() {
            tokens.extend(self.impl_serialize());
        }

        if self.setting.impl_merge() {
            tokens.extend(self.impl_merge());
        }

        if self.setting.impl_state() {
            tokens.extend(self.impl_state());
        }
    }
}
