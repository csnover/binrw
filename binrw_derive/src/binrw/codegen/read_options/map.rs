use super::PreludeGenerator;
use crate::binrw::{
    codegen::{
        get_assertions, get_map_err,
        sanitization::{ARGS, OPT, POS, READER, READ_METHOD, THIS},
        PosEmitter,
    },
    parser::Input,
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Ident};

pub(super) fn generate_map(
    input: &Input,
    name: Option<&Ident>,
    map: &TokenStream,
    pos_emitter: &PosEmitter,
) -> TokenStream {
    let prelude = PreludeGenerator::new(input, pos_emitter)
        .add_imports(name)
        .add_endian()
        .add_magic_pre_assertion()
        .finish();

    let destructure_ref = destructure_ref(input);
    let assertions =
        field_asserts(input, pos_emitter).chain(get_assertions(pos_emitter, input.assertions()));
    let reader_var = input.stream_ident_or(READER);

    // TODO: replace args with top-level arguments and only
    // use `()` as a default
    quote! {
        #prelude

        #READ_METHOD(#reader_var, #OPT, ())
            .map(#map)
                .and_then(|#THIS| {
                    #destructure_ref

                    (|| {
                        #(
                            #assertions
                        )*

                        Ok(())
                    })().map(|_: ()| #THIS)
                })
    }
}

pub(super) fn generate_try_map(
    input: &Input,
    name: Option<&Ident>,
    map: &TokenStream,
    pos_emitter: &PosEmitter,
) -> TokenStream {
    let map_err = get_map_err(POS, map.span());
    let prelude = PreludeGenerator::new(input, pos_emitter)
        .add_imports(name)
        .add_endian()
        .add_magic_pre_assertion()
        .finish();

    let destructure_ref = destructure_ref(input);
    let assertions =
        field_asserts(input, pos_emitter).chain(get_assertions(pos_emitter, input.assertions()));
    let reader_var = input.stream_ident_or(READER);

    // TODO: replace args with top-level arguments and only
    // use `()` as a default
    quote! {
        #prelude

        #READ_METHOD(#reader_var, #OPT, #ARGS).and_then(|value| {
            (#map)(value)#map_err
        })
        .and_then(|#THIS| {
            #destructure_ref

            (|| {
                #(
                    #assertions
                )*

                Ok(())
            })().map(|_: ()| #THIS)
        })
    }
}

fn destructure_ref(input: &Input) -> Option<TokenStream> {
    match input {
        Input::Struct(input) => {
            let fields = input.fields.iter().map(|field| &field.ident);

            if input.is_tuple() {
                Some(quote! {
                    let Self ( #( ref #fields ),* ) = &#THIS;
                })
            } else {
                Some(quote! {
                    let Self { #( ref #fields ),* } = &#THIS;
                })
            }
        }

        _ => None,
    }
}

fn field_asserts<'a>(
    input: &'a Input,
    pos_emitter: &'a PosEmitter,
) -> impl Iterator<Item = TokenStream> + 'a {
    match input {
        Input::Struct(input) => either::Left(
            input
                .fields
                .iter()
                .flat_map(|field| get_assertions(pos_emitter, &field.assertions)),
        ),
        _ => either::Right(core::iter::empty()),
    }
}
