mod r#enum;
mod map;
mod r#struct;

use super::{get_assertions, get_destructured_imports, PosEmitter};
use crate::{
    binrw::{
        codegen::{
            get_endian,
            sanitization::{
                ARGS, ASSERT_MAGIC, MAP_READER_TYPE_HINT, OPT, POS, READER, SEEK_TRAIT,
            },
        },
        parser::{Input, Magic, Map},
    },
    util::quote_spanned_any,
};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use r#enum::{generate_data_enum, generate_unit_enum};
use r#struct::{generate_struct, generate_unit_struct};
use syn::{spanned::Spanned, Ident};

pub(crate) fn generate(input: &Input, derive_input: &syn::DeriveInput) -> TokenStream {
    let reader_var = input.stream_ident_or(READER);
    let name = Some(&derive_input.ident);
    let pos_emitter = PosEmitter::new(&reader_var);
    let inner = match input.map() {
        Map::None => match input {
            Input::UnitStruct(_) => generate_unit_struct(input, name, None, &pos_emitter),
            Input::Struct(s) => generate_struct(input, name, s, &pos_emitter),
            Input::Enum(e) => generate_data_enum(input, name, e, &pos_emitter),
            Input::UnitOnlyEnum(e) => generate_unit_enum(input, name, e, &pos_emitter),
        },
        Map::Try(map) => map::generate_try_map(input, name, map, &pos_emitter),
        Map::Map(map) => map::generate_map(input, name, map, &pos_emitter),
        Map::Repr(ty) => match input {
            Input::UnitOnlyEnum(e) => generate_unit_enum(input, name, e, &pos_emitter),
            _ => map::generate_try_map(
                input,
                name,
                &quote! { <#ty as core::convert::TryInto<_>>::try_into },
                &pos_emitter,
            ),
        },
    };

    let (set_pos, rewind) = super::get_rewind(input, &reader_var, pos_emitter);
    quote! {
        let #reader_var = #READER;
        #set_pos
        (|| {
            #inner
        })()#rewind
    }
}

struct PreludeGenerator<'input> {
    input: &'input Input,
    reader_var: TokenStream,
    out: TokenStream,
    pos_emitter: &'input PosEmitter,
}

impl<'input> PreludeGenerator<'input> {
    fn new(input: &'input Input, pos_emitter: &'input PosEmitter) -> Self {
        let reader_var = input.stream_ident_or(READER);
        Self {
            input,
            reader_var,
            out: TokenStream::new(),
            pos_emitter,
        }
    }

    fn finish(self) -> TokenStream {
        self.out
    }

    fn add_imports(mut self, name: Option<&Ident>) -> Self {
        if let Some(imports) = get_destructured_imports(self.input.imports(), name, false) {
            let head = self.out;
            self.out = quote! {
                #head
                let #imports = #ARGS;
            };
        }

        self
    }

    fn add_endian(mut self) -> Self {
        let endian = get_endian(self.input.endian());
        let head = self.out;
        self.out = quote! {
            #head
            let #OPT = #endian;
        };
        self
    }

    fn add_magic_pre_assertion(mut self) -> Self {
        let head = self.out;
        let magic = get_magic(self.input.magic(), &self.reader_var, OPT);
        let pre_assertions = get_assertions(self.pos_emitter, self.input.pre_assertions());
        self.out = quote! {
            #head
            #magic
            #(#pre_assertions)*
        };

        self
    }

    fn add_map_stream(mut self) -> Self {
        if let Some(map_stream) = self.input.map_stream() {
            let outer_reader = self.input.stream_ident_or(READER);
            let inner_reader = &self.reader_var;
            let head = self.out;
            self.out = quote_spanned_any! { map_stream.span()=>
                #head
                let #inner_reader = &mut #MAP_READER_TYPE_HINT::<R, _, _>(#map_stream)(#outer_reader);
            }
        }

        self
    }

    fn reset_position_after_magic(mut self) -> Self {
        if self.input.magic().is_some() {
            let reader_var = &self.reader_var;
            let head = self.out;
            self.out = quote! {
                #head
                let #POS = #SEEK_TRAIT::stream_position(#reader_var)?;
            };
        };

        self
    }
}

fn get_magic(
    magic: &Magic,
    reader_var: impl ToTokens,
    endian_var: impl ToTokens,
) -> Option<TokenStream> {
    magic.as_ref().map(|magic| {
        let magic = magic.deref_value();
        quote! {
            #ASSERT_MAGIC(#reader_var, #magic, #endian_var)?;
        }
    })
}
