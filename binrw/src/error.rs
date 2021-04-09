//! Functions and type definitions for handling errors.

#[cfg(all(doc, not(feature = "std")))]
extern crate std;
use crate::io;
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String, vec::Vec};
use core::{any::Any, fmt};

/// The `CustomError` trait describes types that are usable as custom errors
/// in a [`BinResult`](crate::BinResult).
///
/// This trait is automatically implemented for any type which implements the
/// same traits as [`std::error::Error`], so anything you would normally use as
/// an error in other code is also a valid `CustomError`, with the additional
/// restriction that it must also be [`Send`] + [`Sync`].
///
/// This trait is Sealed.
pub trait CustomError: fmt::Display + fmt::Debug + Send + Sync + private::Sealed {
    #[doc(hidden)]
    fn as_any(&self) -> &(dyn Any + Send + Sync);

    #[doc(hidden)]
    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync);

    #[doc(hidden)]
    fn as_box_any(self: Box<Self>) -> Box<dyn Any + Send + Sync>;
}

impl<T: fmt::Display + fmt::Debug + Send + Sync + 'static> CustomError for T {
    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
        self
    }

    fn as_box_any(self: Box<Self>) -> Box<dyn Any + Send + Sync> {
        self
    }
}

// The intent here is to allow any object which is compatible with
// `std::error::Error + Send + Sync` to be stored in errors, including no_std
// mode.
impl dyn CustomError {
    /// Attempts to downcast a boxed error to a concrete type.
    pub fn downcast<T: CustomError + 'static>(self: Box<Self>) -> Result<Box<T>, Box<Self>> {
        if self.is::<T>() {
            Ok(self.as_box_any().downcast().unwrap())
        } else {
            Err(self)
        }
    }

    /// Returns some mutable reference to the boxed value if it is of type `T`, or
    /// `None` if it isn't.
    pub fn downcast_mut<T: CustomError + 'static>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut()
    }

    /// Returns some reference to the boxed value if it is of type `T`, or
    /// `None` if it isn’t.
    pub fn downcast_ref<T: CustomError + 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref()
    }

    /// Returns `true` if the boxed type is the same as `T`.
    pub fn is<T: CustomError + 'static>(&self) -> bool {
        core::any::TypeId::of::<T>() == self.as_any().type_id()
    }
}

/// The `Context` trait provides extra methods on [`Result`] to give extra
/// contextual information in case of error.
pub trait Context<T, E> {
    /// Wraps the error value with additional context which is evaluated lazily
    /// once an error occurs.
    fn context<C>(self, f: impl FnOnce() -> C) -> Result<T, Error>
    where
        C: CustomError + 'static;
}

impl<T> Context<T, Error> for Result<T, Error> {
    fn context<C>(self, f: impl FnOnce() -> C) -> Result<T, Error>
    where
        C: CustomError + 'static,
    {
        self.map_err(|err| err.context(f()))
    }
}

/// The error type used by [`BinRead`](crate::BinRead).
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    /// An expected [magic number](crate::attribute#magic) was not found.
    BadMagic {
        /// The byte position of the unexpected magic in the reader.
        pos: u64,

        /// The value which was actually read.
        found: Box<dyn fmt::Debug + Send + Sync>,
    },

    /// An assertion failed.
    ///
    /// This variant is used for [`assert`] directives which use a string
    /// literal instead of an error object. Assertions that use error objects
    /// are represented by the [`Custom`] variant.
    ///
    /// [`assert`]: crate::attribute#assert
    /// [`Custom`]: Self::Custom
    AssertFail {
        /// The byte position of the start of the field or object that raised
        /// an error.
        pos: u64,

        /// The failure message.
        message: String,
    },

    /// An error occurred in the underlying reader while reading or seeking to
    /// data.
    Io(io::Error),

    /// A user-generated error.
    ///
    /// This variant is used for [`assert`] directives which use an error object
    /// instead of a string literal. Assertions that use string literals are
    /// represented by the [`AssertFail`] variant.
    ///
    /// [`assert`]: crate::attribute#assert
    /// [`AssertFail`]: Self::AssertFail
    Custom {
        /// The byte position of the start of the field or object that raised
        /// an error.
        pos: u64,

        /// The original error.
        err: Box<dyn CustomError>,
    },

    /// None of the variants of an enum could successfully be parsed from the
    /// data in the reader.
    ///
    /// This variant is used when the [`return_unexpected_error`] directive is
    /// set on an enum.
    ///
    /// [`return_unexpected_error`]: crate::attribute#enum-errors
    NoVariantMatch {
        /// The byte position of the unparsable data in the reader.
        pos: u64,
    },

    /// None of the variants of an enum could successfully be parsed from the
    /// data in the reader.
    ///
    /// This variant is used when the [`return_all_errors`] directive is
    /// set on an enum (which is the default).
    ///
    /// [`return_all_errors`]: crate::attribute#enum-errors
    EnumErrors {
        /// The byte position of the unparsable data in the reader.
        pos: u64,

        /// The original errors which occurred when trying to parse each
        /// variant.
        ///
        /// The first field of the tuple is the name of the variant, and the
        /// second field is the error that occurred when parsing that variant.
        variant_errors: Vec<(&'static str, Error)>,
    },

    /// An error with attached context.
    Context {
        /// The additional context.
        context: Box<dyn CustomError>,

        /// The original error.
        source: Box<Self>,
    },
}

impl Error {
    /// Wraps the error value with additional context.
    pub fn context<C: CustomError + 'static>(self, context: C) -> Self {
        Self::Context {
            context: Box::new(context),
            source: Box::new(self),
        }
    }

    /// Returns a reference to the boxed error object if this `Error` is a
    /// custom error of type `T`, or `None` if it isn’t.
    ///
    /// For errors with context, this will try to downcast the context first,
    /// then the source, so that simply adding context does not require any
    /// change in consumer code.
    pub fn custom_err<T: CustomError + 'static>(&self) -> Option<&T> {
        if let Self::Custom { err, .. } = self {
            err.downcast_ref()
        } else if let Self::Context { context, source } = self {
            context.downcast_ref().or_else(|| source.custom_err::<T>())
        } else {
            None
        }
    }

    /// Returns a reference to the source error if this `Error` is a chained
    /// error, or `None` if it isn’t.
    pub fn source(&self) -> Option<&Self> {
        match self {
            Self::Context { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic { pos, found } => write!(f, "bad magic at 0x{:x}: {:?}", pos, found),
            Self::AssertFail { pos, message } => write!(f, "{} at 0x{:x}", message, pos),
            Self::Io(err) => fmt::Display::fmt(err, f),
            Self::Custom { pos, err } => write!(f, "{} at 0x{:x}", err, pos),
            Self::NoVariantMatch { pos } => write!(f, "no variants matched at 0x{:x}", pos),
            Self::EnumErrors {
                pos,
                variant_errors,
            } => {
                write!(f, "no variants matched at 0x{:x}:", pos)?;
                for (name, err) in variant_errors {
                    write!(f, "\n  {}: {}", name, err)?;
                }
                Ok(())
            }
            Self::Context { context, source } => write!(f, "{}: {}", context, source),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Context { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

mod private {
    use core::fmt;
    pub trait Sealed {}
    impl<T: fmt::Display + fmt::Debug + Send + Sync + 'static> Sealed for T {}
}
