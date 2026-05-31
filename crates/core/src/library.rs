//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/3a5b7343f715e4e9a3705fa4224e7fa510b92f1c/YarnSpinner/Library.cs>

use crate::prelude::*;
use alloc::borrow::Cow;
use core::fmt::Display;
use crc32fast::hash as crc32;

use hashbrown::hash_map;
use rand::rngs::{SmallRng, SysRng};
use rand::{RngExt as _, SeedableRng};

/// A collection of functions that can be called from Yarn scripts.
///
/// Can be conveniently created with the [`yarn_library!`] macro.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Library(YarnFnRegistry);

impl Extend<<YarnFnRegistry as IntoIterator>::Item> for Library {
    fn extend<T: IntoIterator<Item = (Cow<'static, str>, Box<dyn UntypedYarnFn>)>>(&mut self, iter: T) {
        self.0.extend(iter);
    }
}

impl IntoIterator for Library {
    type Item = (Cow<'static, str>, Box<dyn UntypedYarnFn>);
    type IntoIter = hash_map::IntoIter<Cow<'static, str>, Box<dyn UntypedYarnFn>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Library {
    /// Creates a new empty library. Does not include the functions of [`Library::standard_library`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads functions from another [`Library`].
    ///
    /// Will overwrite any functions that have the same name.
    ///
    /// ## Implementation Notes
    ///
    /// The original implementation throws an exception if a function with the same name already exists.
    pub fn import(&mut self, other: Self) {
        self.0.extend(other.0.0);
    }

    /// Iterates over the names and functions in the library.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &dyn UntypedYarnFn)> {
        self.0.iter()
    }

    /// Gets a function by name.
    pub fn get(&self, name: &str) -> Option<&dyn UntypedYarnFn> {
        self.0.get(name)
    }

    /// Generates a unique tracking variable name.
    /// This is intended to be used to generate names for visiting.
    /// Ideally these will very reproducible and sensible.
    /// For now it will be something terrible and easy.
    pub fn generate_unique_visited_variable_for_node(node_name: &str) -> String {
        format!("$Yarn.Internal.Visiting.{node_name}")
    }

    /// Gets the name of the boolean variable that stores whether the content
    /// identified by `line_id` has been seen by the player before.
    ///
    /// For example, if you have `Alice: once line <<once>> #line:abc123`,
    /// pass `"line:abc123"` to get the variable name.
    pub fn generate_unique_content_viewed_variable_name(line_id: &str) -> String {
        format!("$Yarn.Internal.Once.{line_id}")
    }

    /// Gets the name of the boolean variable that stores whether a `<<once>>`
    /// command block has been seen by the player before.
    ///
    /// This is inherently unstable as it relies on layout attributes of the
    /// yarn code. Uses a CRC32 hash of a description string.
    pub fn generate_unique_command_block_viewed_variable_name(source_file_name: &str, node: &str, line_number: usize) -> String {
        let description = format!("'once' statement in file {source_file_name}, node {node}, line {line_number}");
        let hash = crc32(description.as_bytes());
        Self::generate_unique_content_viewed_variable_name(&hash.to_string())
    }

    /// Creates a [`Library`] with the standard functions that are included in Yarn Spinner.
    /// These are:
    /// - `string`: Converts a value to a string.
    /// - `number`: Converts a value to a number.
    /// - `bool`: Converts a value to a boolean.
    /// - Comparison operators for numbers, strings, and booleans. (`==`, `!=`, `<`, `<=`, `>`, `>=`)
    pub fn standard_library() -> Self {
        let mut library = yarn_library!(
            "string" => <String as From<YarnValue >>::from,
            "number" => |value: YarnValue| f32::try_from(value).expect("Failed to convert a Yarn value to a number"),
            "bool" => |value: YarnValue| bool::try_from(value).expect("Failed to convert a Yarn value to a bool"),
            "format_invariant" => |value: f32| value.to_string(),
            "random" => ||SmallRng::try_from_rng(&mut SysRng).unwrap().random_range(0.0..1.0),
            "random_range" => |min: f32, max: f32| {
                if let Some(min) = min.as_int()
                    && let Some(max_inclusive) = max.as_int()
                {
                    return SmallRng::try_from_rng(&mut SysRng).unwrap().random_range(min..=max_inclusive) as f32;
                }
               SmallRng::try_from_rng(&mut SysRng).unwrap().random_range(min..max)
            },
            "random_range_float" => |min: f32, max: f32|  SmallRng::try_from_rng(&mut SysRng).unwrap().random_range::<f32, _>(min..=max),
            "dice" => |sides: u32| {
                if sides == 0 {
                    return 1;
                }
                SmallRng::try_from_rng(&mut SysRng).unwrap().random_range(1..=sides)
            },
            "round" => |value: f32| value.round() as i32,
            "round_places" => |value: f32, places: u32| value.round_places(places),
            "floor" => |value: f32| value.floor() as i32,
            "ceil" => |value: f32| value.ceil() as i32,
            "inc" => |value: f32| value.floor() as i32 + 1,
            "dec" => |value: f32| value.ceil() as i32 - 1,
            "decimal" => |value: f32| value.fract(),
            "int" => |value: f32| value.trunc() as i32,
            "min" => |a: f32, b: f32| if a <= b { a } else { b },
            "max" => |a: f32, b: f32| if a >= b { a } else { b },
            // "format" => TODO: Hard to do without dedicated crate. Is it useful ?
        );
        for r#type in [Type::Number, Type::String, Type::Boolean] {
            library.add_methods(r#type);
        }
        library
    }

    /// Adds a new function to the registry. See [`YarnFn`]'s documentation for what kinds of functions are allowed.
    ///
    /// ## Examples
    /// Registering a function:
    ///
    /// When the `bevy` feature is set it is possible to register Bevy systems as functions.
    /// ```
    /// # use yarnspinner_core::prelude::*;
    /// # let mut library = Library::default();
    /// # use bevy::prelude::*;
    /// # let mut world = World::default();
    /// library.add_function("string_length", world.register_system(how_many_things));
    ///
    /// fn how_many_things(In(thing_type): In<String>, things: Query<&Name>) -> u32 {
    ///     let mut count = 0;
    ///     for name in &things {
    ///         if name.as_str() == thing_type {
    ///             count += 1;
    ///         }
    ///     }
    ///     count
    /// }
    /// ```
    ///
    /// Usage without Bevy:
    ///
    /// ```
    /// # use yarnspinner_core::prelude::*;
    /// # let mut library = Library::default();
    /// library.add_function("string_length", string_length);
    ///
    /// fn string_length(string: String) -> usize {
    ///     string.len()
    /// }
    /// ```
    ///
    /// Registering a function using a factory
    /// (the return type can be specified using the [`yarn_fn_type`] macro):
    /// ```
    /// # use yarnspinner_core::prelude::*;
    /// # let mut library = Library::default();
    /// library.add_function("length_times_two", string_length_multiplied(2));
    ///
    /// fn string_length_multiplied(factor: usize) -> yarn_fn_type! { impl Fn(String) -> usize } {
    ///     move |s: String| s.len() * factor
    /// }
    /// ```
    ///
    pub fn add_function<Marker, F>(&mut self, name: impl Into<Cow<'static, str>>, function: F) -> &mut Self
    where
        Marker: 'static,
        F: YarnFn<Marker> + 'static + Clone,
        F::Out: IntoYarnValueFromNonYarnValue + 'static + Clone,
    {
        self.0.register_function(name, function);
        self
    }

    /// Adds a variadic function to the library.
    ///
    /// A variadic function accepts zero or more *fixed* parameters followed by any number of
    /// *variadic* parameters of the same type.  The implementation closure receives all
    /// arguments – fixed and variadic – in a single flat [`Vec<YarnValue>`].
    ///
    /// ## Type IDs
    ///
    /// Use [`core::any::TypeId::of::<T>()`] to build the type arguments:
    /// * `f32` → [`Type::Number`]
    /// * `String` / `&str` → [`Type::String`]
    /// * `bool` → [`Type::Boolean`]
    ///
    /// ## Example
    ///
    /// ```rust
    /// # use yarnspinner_core::prelude::*;
    /// # use core::any::TypeId;
    /// # let mut library = Library::default();
    /// // fn variadic_add(params f32[]) -> f32 { args.iter().sum() }
    /// library.add_variadic_function(
    ///     "variadic_add",
    ///     vec![],                  // no fixed parameters
    ///     TypeId::of::<f32>(),     // variadic param type = Number
    ///     TypeId::of::<f32>(),     // return type = Number
    ///     |args: Vec<YarnValue>| -> YarnValue {
    ///         let sum: f32 = args.iter()
    ///             .filter_map(|v| f32::try_from(v.clone()).ok())
    ///             .sum();
    ///         YarnValue::Number(sum)
    ///     },
    /// );
    /// ```
    pub fn add_variadic_function(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        fixed_param_type_ids: Vec<core::any::TypeId>,
        variadic_type_id: core::any::TypeId,
        return_type_id: core::any::TypeId,
        func: impl Fn(Vec<YarnValue>) -> YarnValue + Send + Sync + 'static,
    ) -> &mut Self {
        let wrapper = VariadicYarnFn::new(fixed_param_type_ids, variadic_type_id, return_type_id, func);
        self.0.add_boxed(name, Box::new(wrapper));
        self
    }

    /// Returns `true` if the library contains a function with the given name.
    pub fn contains_function(&self, name: &str) -> bool {
        self.0.contains_function(name)
    }

    /// Iterates over the names of all functions in the library.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.names()
    }

    /// Iterates over all functions in the library.
    pub fn functions(&self) -> impl Iterator<Item = &dyn UntypedYarnFn> {
        self.0.functions()
    }

    /// Registers the methods found inside a type.
    fn add_methods(&mut self, r#type: Type) {
        for (name, function) in r#type.methods().into_iter() {
            let canonical_name = r#type.get_canonical_name_for_method(name.as_ref());
            self.0.add_boxed(canonical_name, function.clone());
        }
    }
}

impl Display for Library {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut functions: Vec<_> = self.0.iter().collect();
        functions.sort_by_key(|(name, _)| name.to_string());
        writeln!(f, "{{")?;
        for (name, function) in functions {
            writeln!(f, "    {name}: {function}")?;
        }
        writeln!(f, "}}")?;
        Ok(())
    }
}

/// Create a [`Library`] from a list of named functions.
///
/// ## Example
///
/// ```rust
/// # use yarnspinner_core::yarn_library;
/// # use yarnspinner_core::prelude::*;
///
/// let library = yarn_library! {
///    "pow" => pow,
/// };
///
/// fn pow(base: f32, exponent: i32) -> f32 {
///    base.powi(exponent)
/// }
///```
#[macro_export]
macro_rules! yarn_library {
    ($($name:expr => $function:expr,)*) => {
        {
            let mut map = Library::default();
            $(
                map.add_function($name, $function);
            )*
            map
        }
    };
}
pub use yarn_library;

trait FloatExt: Copy {
    fn as_int(self) -> Option<i32>;
    fn round_places(self, places: u32) -> Self;
}

impl FloatExt for f32 {
    fn as_int(self) -> Option<i32> {
        (self.fract().abs() <= f32::EPSILON).then_some(self as i32)
    }

    fn round_places(self, places: u32) -> Self {
        let factor = 10_u32.pow(places) as f32;
        (self * factor).round() / factor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_places() {
        for (num, places, expected) in [
            (1.0, 0, 1.0),
            (1.2, 1, 1.2),
            (0.4, 0, 0.0),
            (43.132, 0, 43.0),
            (1.1, 2, 1.1),
            (123.123, 3, 123.123),
            (-10.3, 1, -10.3),
            (-11.99, 1, -12.0),
        ] {
            assert_eq!(expected, num.round_places(places));
        }
    }
}
