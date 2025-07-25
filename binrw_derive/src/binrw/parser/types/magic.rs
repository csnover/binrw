use super::SpannedValue;
use crate::{binrw::parser::attrs, meta_types::KeywordToken};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::Lit;

#[derive(PartialEq, Eq, Hash, Clone, Debug, PartialOrd, Ord)]
pub(crate) enum Kind {
    Numeric(String),
    ByteStr(String),
}

impl From<&Kind> for TokenStream {
    fn from(kind: &Kind) -> Self {
        match kind {
            Kind::ByteStr(ty) | Kind::Numeric(ty) => {
                let ty: TokenStream = ty.parse().unwrap();
                quote! { #ty }
            }
        }
    }
}

pub(crate) type Magic = Option<SpannedValue<Inner>>;

#[derive(Clone, Debug)]
pub(crate) struct Inner(Kind, TokenStream);

impl Inner {
    // TODO: There should not be codegen in the parser
    pub(crate) fn add_ref(&self) -> TokenStream {
        match &self.0 {
            Kind::ByteStr(_) => quote! { & },
            Kind::Numeric(_) => TokenStream::new(),
        }
    }

    // TODO: There should not be codegen in the parser
    pub(crate) fn deref_value(&self) -> TokenStream {
        match self.0 {
            Kind::ByteStr(_) => {
                let value = &self.1;
                quote! { *#value }
            }
            Kind::Numeric(_) => self.1.clone(),
        }
    }

    pub(crate) fn kind(&self) -> &Kind {
        &self.0
    }

    pub(crate) fn match_value(&self) -> &TokenStream {
        &self.1
    }

    #[cfg(feature = "verbose-backtrace")]
    pub(crate) fn into_match_value(self) -> TokenStream {
        self.1
    }
}

impl TryFrom<attrs::Magic> for SpannedValue<Inner> {
    type Error = syn::Error;

    fn try_from(magic: attrs::Magic) -> Result<Self, Self::Error> {
        let value = &magic.value;

        let kind = match &value {
            Lit::ByteStr(bytes) => {
                Kind::ByteStr(format!("[::core::primitive::u8; {}]", bytes.value().len()))
            }
            Lit::Byte(_) => Kind::Numeric("::core::primitive::u8".to_owned()),
            Lit::Int(i) => {
                if i.suffix().is_empty() {
                    return Err(syn::Error::new(
                        value.span(),
                        format!("expected explicit type suffix for integer literal\ne.g {i}u64",),
                    ));
                }
                Kind::Numeric(i.suffix().to_owned())
            }
            Lit::Float(f) => {
                if f.suffix().is_empty() {
                    return Err(syn::Error::new(
                        value.span(),
                        format!(
                            "expected explicit type suffix for float literal\nvalid values are {f}f32 or {f}f64",
                        ),
                    ));
                }
                Kind::Numeric(f.suffix().to_owned())
            }
            Lit::Char(_) | Lit::Str(_) | Lit::Bool(_) | Lit::Verbatim(_) => {
                return Err(syn::Error::new(
                    value.span(),
                    "expected byte string, byte, float, or int",
                ))
            }
            _ => return Err(syn::Error::new(value.span(), "unexpected literal")),
        };

        Ok(Self::new(
            Inner(kind, value.to_token_stream()),
            magic.keyword_span(),
        ))
    }
}
