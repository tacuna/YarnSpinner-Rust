//! Variadic function wrapper for [`UntypedYarnFn`].
//!
//! Unlike the statically-typed [`YarnFnWrapper`], this wrapper accepts a raw
//! `Fn(Vec<YarnValue>) -> YarnValue` closure together with explicit type
//! metadata, which lets callers register functions that accept a variable
//! number of arguments.

use crate::prelude::*;
use alloc::sync::Arc;
#[cfg(feature = "bevy")]
use bevy::prelude::World;
use core::any::TypeId;
use core::fmt::{Debug, Display, Formatter};

/// An [`UntypedYarnFn`] wrapper that represents a variadic Yarn function.
///
/// A variadic function has zero or more *fixed* parameters followed by any
/// number of *variadic* parameters, all of the same type.  The implementation
/// closure receives every argument (fixed + variadic) in a single flat
/// [`Vec<YarnValue>`].
///
/// # Registration
///
/// Use [`Library::add_variadic_function`] to register a variadic function
/// instead of constructing this type directly.
pub struct VariadicYarnFn {
    /// [`TypeId`]s of the fixed parameters (in order, before any variadic args).
    fixed_param_type_ids: Vec<TypeId>,
    /// [`TypeId`] of each additional (variadic) argument.
    variadic_type_id: TypeId,
    /// [`TypeId`] of the return value.
    return_type_id: TypeId,
    /// The actual implementation.
    func: Arc<dyn Fn(Vec<YarnValue>) -> YarnValue + Send + Sync>,
}

impl VariadicYarnFn {
    /// Creates a new variadic function wrapper.
    ///
    /// * `fixed_param_type_ids` – type ids of the required fixed parameters.
    /// * `variadic_type_id` – type id shared by all variadic arguments.
    /// * `return_type_id` – type id of the return value.
    /// * `func` – implementation that receives all args (fixed + variadic).
    pub fn new(
        fixed_param_type_ids: Vec<TypeId>,
        variadic_type_id: TypeId,
        return_type_id: TypeId,
        func: impl Fn(Vec<YarnValue>) -> YarnValue + Send + Sync + 'static,
    ) -> Self {
        Self {
            fixed_param_type_ids,
            variadic_type_id,
            return_type_id,
            func: Arc::new(func),
        }
    }
}

impl Clone for VariadicYarnFn {
    fn clone(&self) -> Self {
        Self {
            fixed_param_type_ids: self.fixed_param_type_ids.clone(),
            variadic_type_id: self.variadic_type_id,
            return_type_id: self.return_type_id,
            func: self.func.clone(),
        }
    }
}

impl Debug for VariadicYarnFn {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "VariadicYarnFn")
    }
}

impl Display for VariadicYarnFn {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "VariadicYarnFn")
    }
}

impl UntypedYarnFn for VariadicYarnFn {
    fn call(&self, input: Vec<YarnValue>) -> YarnValue {
        (self.func)(input)
    }

    #[cfg(feature = "bevy")]
    fn call_with_world(&self, input: Vec<YarnValue>, _world: &mut World) -> YarnValue {
        (self.func)(input)
    }

    fn clone_box(&self) -> Box<dyn UntypedYarnFn> {
        Box::new(self.clone())
    }

    /// Returns only the *fixed* parameter types; the variadic part is exposed
    /// via [`variadic_parameter_type_id`](Self::variadic_parameter_type_id).
    fn parameter_types(&self) -> Vec<TypeId> {
        self.fixed_param_type_ids.clone()
    }

    fn return_type(&self) -> TypeId {
        self.return_type_id
    }

    fn variadic_parameter_type_id(&self) -> Option<TypeId> {
        Some(self.variadic_type_id)
    }
}
