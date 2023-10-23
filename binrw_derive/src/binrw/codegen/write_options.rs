mod r#enum;
mod prelude;
mod r#struct;
mod struct_field;

use super::get_map_err;
use crate::binrw::{
    codegen::{
        sanitization::{OPT, POS, WRITER, WRITE_METHOD},
        PosEmitter,
    },
    parser::{Input, Map},
};
use proc_macro2::TokenStream;
use quote::quote;
use r#enum::{generate_data_enum, generate_unit_enum};
use r#struct::generate_struct;
use syn::{spanned::Spanned, Ident};

pub(crate) fn generate(input: &Input, derive_input: &syn::DeriveInput) -> TokenStream {
    let name = Some(&derive_input.ident);
    let writer_var = input.stream_ident_or(WRITER);
    let pos_emitter = PosEmitter::new(&writer_var);
    let inner = match input.map() {
        Map::None => match input {
            Input::UnitStruct(s) | Input::Struct(s) => {
                generate_struct(input, name, s, &pos_emitter)
            }
            Input::Enum(e) => generate_data_enum(input, name, e, &pos_emitter),
            Input::UnitOnlyEnum(e) => generate_unit_enum(input, name, e, &pos_emitter),
        },
        Map::Try(map) | Map::Map(map) => generate_map(input, name, map, &pos_emitter),
        Map::Repr(map) => match input {
            Input::UnitOnlyEnum(e) => generate_unit_enum(input, name, e, &pos_emitter),
            _ => generate_map(input, name, map, &pos_emitter),
        },
    };

    let (set_pos, rewind) = super::get_rewind(input, &writer_var, pos_emitter);
    quote! {
        let #writer_var = #WRITER;
        #set_pos
        (|| {
            #inner
            Ok(())
        })()#rewind
    }
}

fn generate_map(input: &Input, name: Option<&Ident>, map: &TokenStream, pos_emitter: &PosEmitter) -> TokenStream {
    let map_try = input.map().is_try().then(|| {
        let map_err = get_map_err(POS, map.span());
        quote! { #map_err? }
    });
    let map = if matches!(input.map(), Map::Repr(_)) {
        quote! { <#map as core::convert::TryFrom<_>>::try_from }
    } else {
        map.clone()
    };
    let writer_var = input.stream_ident_or(WRITER);
    let write_data = quote! {
        #WRITE_METHOD(
            &((#map)(self) #map_try),
            #writer_var,
            #OPT,
            ()
        )?;
    };

    let magic = input.magic();
    let endian = input.endian();
    prelude::PreludeGenerator::new(write_data, input, name, &writer_var, pos_emitter)
        .prefix_magic(magic)
        .prefix_assertions()
        .prefix_endian(endian)
        .prefix_imports()
        .finish()
}
