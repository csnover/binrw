use super::{
    r#struct::{generate_unit_struct, StructGenerator},
    PreludeGenerator,
};
use crate::binrw::{
    codegen::{
        sanitization::{
            BACKTRACE_FRAME, BIN_ERROR, ERROR_BASKET, OPT, READER, READ_METHOD,
            RESTORE_POSITION_VARIANT, TEMP, WITH_CONTEXT,
        },
        PosEmitter,
    },
    parser::{Enum, EnumErrorMode, EnumVariant, Input, UnitEnumField, UnitOnlyEnum},
};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

pub(super) fn generate_unit_enum(
    input: &Input,
    name: Option<&Ident>,
    en: &UnitOnlyEnum,
    pos_emitter: &PosEmitter,
) -> TokenStream {
    let prelude = PreludeGenerator::new(input, pos_emitter)
        .add_imports(name)
        .add_endian()
        .add_magic_pre_assertion()
        .finish();

    let read = match en.map.as_repr() {
        Some(repr) => generate_unit_enum_repr(
            &input.stream_ident_or(READER),
            repr,
            &en.fields,
            pos_emitter,
        ),
        None => generate_unit_enum_magic(&input.stream_ident_or(READER), &en.fields, pos_emitter),
    };

    quote! {
        #prelude
        #read
    }
}

fn generate_unit_enum_repr(
    reader_var: &TokenStream,
    repr: &TokenStream,
    variants: &[UnitEnumField],
    pos_emitter: &PosEmitter,
) -> TokenStream {
    let clauses = variants.iter().map(|variant| {
        let ident = &variant.ident;
        let pre_assertions = variant
            .pre_assertions
            .iter()
            .map(|assert| &assert.condition);

        quote! {
            if #TEMP == Self::#ident as #repr #(&& (#pre_assertions))* {
                Ok(Self::#ident)
            }
        }
    });

    let pos = pos_emitter.pos();

    quote! {
        let #TEMP: #repr = #READ_METHOD(#reader_var, #OPT, ())?;
        #(#clauses else)* {
            Err(#WITH_CONTEXT(
                #BIN_ERROR::NoVariantMatch {
                    pos: #pos,
                },
                #BACKTRACE_FRAME::Message({
                    extern crate alloc;
                    alloc::format!("Unexpected value for enum: {:?}", #TEMP).into()
                })
            ))
        }
    }
}

fn generate_unit_enum_magic(
    reader_var: &TokenStream,
    variants: &[UnitEnumField],
    pos_emitter: &PosEmitter,
) -> TokenStream {
    // group fields by the type (Kind) of their magic value, preserve the order
    let group_by_magic_type = variants.iter().fold(
        Vec::new(),
        |mut group_by_magic_type: Vec<(_, Vec<_>)>, field| {
            let kind = field.magic.as_ref().map(|magic| magic.kind());
            let last = group_by_magic_type.last_mut();
            match last {
                // if the current field's magic kind is the same as the previous one
                // then add the current field to the same group
                // if the magic kind is none then it's a wildcard, just add it to the previous group
                Some((last_kind, last_vec)) if kind.is_none() || *last_kind == kind => {
                    last_vec.push(field);
                }
                // otherwise if the vector is empty
                // or the last field's magic kind is different
                // then create a new group
                _ => group_by_magic_type.push((kind, vec![field])),
            }

            group_by_magic_type
        },
    );

    let pos = pos_emitter.pos();

    // for each type (Kind), read and try to match the magic of each field
    let try_each_magic_type = group_by_magic_type.into_iter().map(|(_kind, fields)| {
        let amp = fields[0].magic.as_ref().map(|magic| magic.add_ref());

        let matches = fields.iter().map(|field| {
            let ident = &field.ident;

            if let Some(magic) = &field.magic {
                let magic = magic.match_value();
                let condition = if field.pre_assertions.is_empty() {
                    quote! { #magic }
                } else {
                    let pre_assertions =
                        field.pre_assertions.iter().map(|assert| &assert.condition);
                    quote! { #magic if true #(&& (#pre_assertions))* }
                };

                quote! { #condition => Ok(Self::#ident) }
            } else {
                quote! { _ => Ok(Self::#ident) }
            }
        });

        let body = quote! {
            match #amp #READ_METHOD(#reader_var, #OPT, ())? {
                #(#matches,)*
                _ => Err(#BIN_ERROR::NoVariantMatch { pos: #pos })
            }
        };

        quote! {
            match (|| {
                #body
            })() {
                v @ Ok(_) => return v,
                Err(#TEMP) => { #RESTORE_POSITION_VARIANT(#reader_var, #pos, #TEMP)?; }
            }
        }
    });

    let return_error = quote! {
        Err(#BIN_ERROR::NoVariantMatch {
            pos: #pos
        })
    };

    quote! {
        #(#try_each_magic_type)*
        #return_error
    }
}

pub(super) fn generate_data_enum(
    input: &Input,
    name: Option<&Ident>,
    en: &Enum,
    pos_emitter: &PosEmitter,
) -> TokenStream {
    let return_all_errors = en.error_mode != EnumErrorMode::ReturnUnexpectedError;

    let prelude = PreludeGenerator::new(input, pos_emitter)
        .add_imports(name)
        .add_endian()
        .add_magic_pre_assertion()
        .reset_position_after_magic()
        .finish();

    let pos = pos_emitter.pos();

    let (create_error_basket, return_error) = if return_all_errors {
        (
            quote! {
                extern crate alloc;
                let mut #ERROR_BASKET: alloc::vec::Vec<(&'static str, #BIN_ERROR)> = alloc::vec::Vec::new();
            },
            quote! {
                Err(#BIN_ERROR::EnumErrors {
                    pos: #pos,
                    variant_errors: #ERROR_BASKET
                })
            },
        )
    } else {
        (
            TokenStream::new(),
            quote! {
                Err(#BIN_ERROR::NoVariantMatch {
                    pos: #pos,
                })
            },
        )
    };

    let reader_var = input.stream_ident_or(READER);

    let try_each_variant = en.variants.iter().map(|variant| {
        let body = generate_variant_impl(en, variant, pos_emitter);

        let handle_error = if return_all_errors {
            let name = variant.ident().to_string();
            quote! {
                #ERROR_BASKET.push((#name, #TEMP));
            }
        } else {
            TokenStream::new()
        };

        quote! {
            match (|| {
                #body
            })() {
                ok @ Ok(_) => return ok,
                Err(error) => {
                    #RESTORE_POSITION_VARIANT(#reader_var, #pos, error).map(|#TEMP| {
                        #handle_error
                    })?;
                }
            }
        }
    });

    quote! {
        #prelude
        #create_error_basket
        #(#try_each_variant)*
        #return_error
    }
}

fn generate_variant_impl(
    en: &Enum,
    variant: &EnumVariant,
    pos_emitter: &PosEmitter,
) -> TokenStream {
    let input = Input::Struct(variant.clone().into());

    match variant {
        EnumVariant::Variant { ident, options } => {
            StructGenerator::new(&input, options, pos_emitter)
                .read_fields(
                    None,
                    Some(&format!("{}::{}", en.ident.as_ref().unwrap(), &ident)),
                )
                .initialize_value_with_assertions(Some(ident), &en.assertions)
                .return_value()
                .finish()
        }

        EnumVariant::Unit(options) => {
            generate_unit_struct(&input, None, Some(&options.ident), pos_emitter)
        }
    }
}
