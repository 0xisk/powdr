use std::{
    collections::{BTreeSet, HashMap},
    fmt::Display,
    iter::empty,
};

use itertools::Itertools;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Expression;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize, JsonSchema)]
pub enum Type<E = u64> {
    /// The bottom type `!`, which cannot have a value but is
    /// compatible with all other types.
    Bottom,
    /// Boolean
    Bool,
    /// Integer (arbitrary precision)
    Int,
    /// Field element (unspecified field)
    Fe,
    /// String
    String,
    /// Column
    Col,
    /// Algebraic expression
    Expr,
    /// Polynomial identity
    Constr,
    Array(ArrayType<E>),
    Tuple(TupleType<E>),
    Function(FunctionType<E>),
    TypeVar(String),
}

impl<E> Type<E> {
    /// Returns true if it is a non-complex type.
    /// Type variables are not considered elementary.
    pub fn is_elementary(&self) -> bool {
        match self {
            Type::Bottom
            | Type::Bool
            | Type::Int
            | Type::Fe
            | Type::String
            | Type::Col
            | Type::Expr
            | Type::Constr => true,
            Type::Array(_) | Type::Tuple(_) | Type::Function(_) | Type::TypeVar(_) => false,
        }
    }
    /// Returns true if the type name needs parentheses during formatting
    /// when used inside a complex expression.
    pub fn needs_parentheses(&self) -> bool {
        match self {
            _ if self.is_elementary() => false,
            Type::Array(_) | Type::Tuple(_) | Type::TypeVar(_) => false,
            Type::Function(_) => true,
            _ => unreachable!(),
        }
    }

    /// Returns an iterator over all (top-level) expressions in this type name.
    pub fn expressions(&self) -> Box<dyn Iterator<Item = &E> + '_> {
        match self {
            _ if self.is_elementary() => Box::new(empty()),
            Type::TypeVar(_) => Box::new(empty()),
            Type::Array(a) => a.expressions(),
            Type::Tuple(t) => t.expressions(),
            Type::Function(f) => f.expressions(),
            _ => unreachable!(),
        }
    }

    /// Returns an iterator over all (top-level) expressions in this type name.
    pub fn expressions_mut(&mut self) -> Box<dyn Iterator<Item = &mut E> + '_> {
        match self {
            _ if self.is_elementary() => Box::new(empty()),
            Type::TypeVar(_) => Box::new(empty()),
            Type::Array(a) => a.expressions_mut(),
            Type::Tuple(t) => t.expressions_mut(),
            Type::Function(f) => f.expressions_mut(),
            _ => unreachable!(),
        }
    }

    pub fn is_concrete_type(&self) -> bool {
        self.contained_type_vars_with_repetitions().next().is_none()
    }

    pub fn contains_type_var(&self, name: &str) -> bool {
        self.contained_type_vars_with_repetitions()
            .any(|n| n == name)
    }

    /// Returns the list of contained type vars in order of first occurrence.
    pub fn contained_type_vars(&self) -> impl Iterator<Item = &String> {
        self.contained_type_vars_with_repetitions().unique()
    }
}
impl<E: Clone> Type<E> {
    /// Substitutes all occurrences of the given type variables with the given types.
    /// Does not apply the substitutions inside the replacements.
    pub fn substitute_type_vars(&mut self, substitutions: &HashMap<String, Type<E>>) {
        match self {
            Type::TypeVar(n) => {
                if let Some(t) = substitutions.get(n) {
                    *self = t.clone();
                }
            }
            Type::Array(ArrayType { base, length: _ }) => {
                base.substitute_type_vars(substitutions);
            }
            Type::Tuple(TupleType { items }) => {
                items
                    .iter_mut()
                    .for_each(|t| t.substitute_type_vars(substitutions));
            }
            Type::Function(FunctionType { params, value }) => {
                params
                    .iter_mut()
                    .for_each(|t| t.substitute_type_vars(substitutions));
                value.substitute_type_vars(substitutions);
            }
            _ => {
                assert!(self.is_elementary());
            }
        }
    }

    pub fn substitute_type_vars_to(mut self, substitutions: &HashMap<String, Type<E>>) -> Self {
        self.substitute_type_vars(substitutions);
        self
    }
}

impl<E> Type<E> {
    fn contained_type_vars_with_repetitions(&self) -> Box<dyn Iterator<Item = &String> + '_> {
        match self {
            Type::TypeVar(n) => Box::new(std::iter::once(n)),
            Type::Array(ar) => ar.base.contained_type_vars_with_repetitions(),
            Type::Tuple(tu) => Box::new(
                tu.items
                    .iter()
                    .flat_map(|t| t.contained_type_vars_with_repetitions()),
            ),
            Type::Function(fun) => Box::new(
                fun.params
                    .iter()
                    .flat_map(|t| t.contained_type_vars_with_repetitions())
                    .chain(fun.value.contained_type_vars_with_repetitions()),
            ),
            _ => {
                assert!(self.is_elementary());
                Box::new(std::iter::empty())
            }
        }
    }
}

impl<R: Display> From<Type<Expression<R>>> for Type<u64> {
    fn from(value: Type<Expression<R>>) -> Self {
        match value {
            Type::Bottom => Type::Bottom,
            Type::Bool => Type::Bool,
            Type::Int => Type::Int,
            Type::Fe => Type::Fe,
            Type::String => Type::String,
            Type::Col => Type::Col,
            Type::Expr => Type::Expr,
            Type::Constr => Type::Constr,
            Type::Array(a) => Type::Array(a.into()),
            Type::Tuple(t) => Type::Tuple(t.into()),
            Type::Function(f) => Type::Function(f.into()),
            Type::TypeVar(n) => Type::TypeVar(n),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArrayType<E = u64> {
    pub base: Box<Type<E>>,
    pub length: Option<E>,
}

impl<E> ArrayType<E> {
    /// Returns an iterator over all (top-level) expressions in this type name.
    pub fn expressions(&self) -> Box<dyn Iterator<Item = &E> + '_> {
        Box::new(self.base.expressions().chain(self.length.iter()))
    }
    /// Returns an iterator over all (top-level) expressions in this type name.
    pub fn expressions_mut(&mut self) -> Box<dyn Iterator<Item = &mut E> + '_> {
        Box::new(self.base.expressions_mut().chain(self.length.iter_mut()))
    }
}

impl<R: Display> From<ArrayType<Expression<R>>> for ArrayType<u64> {
    fn from(value: ArrayType<Expression<R>>) -> Self {
        let length = value.length.as_ref().map(|l| {
            if let Expression::Number(n, ty) = l {
                assert!(ty.is_none(), "Literal inside type name has assigned type. This should be done during analysis on the types instead.");
                n.try_into().expect("Array length expression too large.")
            } else {
                panic!(
                    "Array length expression not resolved in type name prior to conversion: {value}"
                );
            }
        });
        ArrayType {
            base: Box::new(Type::from(*value.base)),
            length,
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TupleType<E = u64> {
    pub items: Vec<Type<E>>,
}

impl<E> TupleType<E> {
    /// Returns an iterator over all (top-level) expressions in this type name.
    pub fn expressions(&self) -> Box<dyn Iterator<Item = &E> + '_> {
        Box::new(self.items.iter().flat_map(|t| t.expressions()))
    }
    /// Returns an iterator over all (top-level) expressions in this type name.
    pub fn expressions_mut(&mut self) -> Box<dyn Iterator<Item = &mut E> + '_> {
        Box::new(self.items.iter_mut().flat_map(|t| t.expressions_mut()))
    }
}

impl<R: Display> From<TupleType<Expression<R>>> for TupleType<u64> {
    fn from(value: TupleType<Expression<R>>) -> Self {
        TupleType {
            items: value.items.into_iter().map(|t| t.into()).collect(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FunctionType<E = u64> {
    pub params: Vec<Type<E>>,
    pub value: Box<Type<E>>,
}

impl<E> FunctionType<E> {
    /// Returns an iterator over all (top-level) expressions in this type name.
    pub fn expressions(&self) -> Box<dyn Iterator<Item = &E> + '_> {
        Box::new(
            self.params
                .iter()
                .flat_map(|t| t.expressions())
                .chain(self.value.expressions()),
        )
    }
    /// Returns an iterator over all (top-level) expressions in this type name.
    pub fn expressions_mut(&mut self) -> Box<dyn Iterator<Item = &mut E> + '_> {
        Box::new(
            self.params
                .iter_mut()
                .flat_map(|t| t.expressions_mut())
                .chain(self.value.expressions_mut()),
        )
    }
}

impl<R: Display> From<FunctionType<Expression<R>>> for FunctionType<u64> {
    fn from(value: FunctionType<Expression<R>>) -> Self {
        FunctionType {
            params: value.params.into_iter().map(|t| t.into()).collect(),
            value: Box::new((*value.value).into()),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TypeScheme<E = u64> {
    /// Type variables and their trait bounds.
    pub vars: TypeBounds,
    /// The actual type (using the type variables from `vars` but potentially also other type variables)
    pub ty: Type<E>,
}

impl<E: Clone> TypeScheme<E> {
    /// Returns a new type scheme with type variables renamed to `T1`, `T2`, ...
    /// (or just `T` if it is a single type variable).
    pub fn simplify_type_vars(self) -> TypeScheme<E> {
        let name_substitutions: HashMap<_, _> = match self.vars.len() {
            0 => return self,
            1 => {
                let var = self.vars.vars().next().unwrap();
                [(var.clone(), "T".to_string())].into()
            }
            _ => self
                .vars
                .vars()
                .enumerate()
                .map(|(i, v)| ((*v).clone(), format!("T{}", i + 1)))
                .collect(),
        };
        assert!(name_substitutions.len() == self.vars.len());
        let mut ty = self.ty;
        ty.substitute_type_vars(
            &name_substitutions
                .iter()
                .map(|(n, s)| (n.clone(), Type::TypeVar(s.clone())))
                .collect(),
        );
        TypeScheme {
            vars: TypeBounds::new(
                self.vars
                    .bounds()
                    .map(|(v, b)| (name_substitutions[v].clone(), b.clone())),
            ),
            ty,
        }
    }
}
impl<E> TypeScheme<E> {
    pub fn type_vars_to_string(&self) -> String {
        if self.vars.is_empty() {
            String::new()
        } else {
            format!("<{}>", self.vars)
        }
    }
}

impl From<Type> for TypeScheme {
    fn from(value: Type) -> Self {
        TypeScheme {
            vars: Default::default(),
            ty: value,
        }
    }
}

#[derive(
    Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Default, Serialize, Deserialize, JsonSchema,
)]
// TODO bounds should be SymbolPaths in the future.
pub struct TypeBounds(Vec<(String, BTreeSet<String>)>);

impl TypeBounds {
    pub fn new<J: Into<BTreeSet<String>>, I: Iterator<Item = (String, J)>>(vars: I) -> Self {
        Self(vars.map(|(n, x)| (n, x.into())).collect::<Vec<_>>())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn vars(&self) -> impl Iterator<Item = &String> {
        self.0.iter().map(|(n, _)| n)
    }

    pub fn bounds(&self) -> impl Iterator<Item = (&String, &BTreeSet<String>)> {
        self.0.iter().map(|(n, x)| (n, x))
    }
}
