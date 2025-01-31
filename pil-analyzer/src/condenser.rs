//! Component that turns data from the PILAnalyzer into Analyzed,
//! i.e. it turns more complex expressions in identities to simpler expressions.

use std::collections::HashMap;

use powdr_ast::{
    analyzed::{
        AlgebraicExpression, Analyzed, Expression, FunctionValueDefinition, Identity, IdentityKind,
        PolynomialType, PublicDeclaration, StatementIdentifier, Symbol, SymbolKind,
    },
    parsed::{
        display::format_type_scheme_around_name,
        types::{ArrayType, Type},
        SelectedExpressions,
    },
};
use powdr_number::{DegreeType, FieldElement};

use crate::evaluator::{self, Definitions, Value};

pub fn condense<T: FieldElement>(
    degree: Option<DegreeType>,
    mut definitions: HashMap<String, (Symbol, Option<FunctionValueDefinition>)>,
    mut public_declarations: HashMap<String, PublicDeclaration>,
    identities: &[Identity<Expression>],
    source_order: Vec<StatementIdentifier>,
) -> Analyzed<T> {
    let condenser = Condenser {
        symbols: definitions.clone(),
        _phantom: Default::default(),
    };

    let mut condensed_identities = vec![];
    // Condense identities and update the source order.
    let source_order = source_order
        .into_iter()
        .flat_map(|s| match s {
            StatementIdentifier::Identity(index) => {
                let identity = &identities[index];
                condenser
                    .condense_identity(identity)
                    .into_iter()
                    .map(|identity| {
                        let id = condensed_identities.len();
                        condensed_identities.push(identity);
                        StatementIdentifier::Identity(id)
                    })
                    .collect()
            }
            s => vec![s],
        })
        .collect();

    // Extract intermediate columns
    let intermediate_columns: HashMap<_, _> = definitions
        .iter()
        .filter_map(|(name, (symbol, definition))| {
            if !matches!(symbol.kind, SymbolKind::Poly(PolynomialType::Intermediate)) {
                return None;
            }
            let Some(FunctionValueDefinition::Expression(e)) = definition else {
                panic!("Expected expression")
            };
            let value = if let Some(length) = symbol.length {
                let scheme = e.type_scheme.as_ref();
                assert!(
                    scheme.unwrap().vars.is_empty()
                        && matches!(
                            &scheme.unwrap().ty,
                            Type::Array(ArrayType { base, length: _ })
                            if base.as_ref() == &Type::Expr),
                    "Intermediate column type has to be expr[], but got: {}",
                    format_type_scheme_around_name(name, &e.type_scheme)
                );
                let result = condenser.condense_to_array_of_algebraic_expressions(&e.e);
                assert_eq!(result.len() as u64, length);
                result
            } else {
                assert_eq!(
                    e.type_scheme,
                    Some(Type::Expr.into()),
                    "Intermediate column type has to be expr, but got: {}",
                    format_type_scheme_around_name(name, &e.type_scheme)
                );
                vec![condenser.condense_to_algebraic_expression(&e.e)]
            };
            Some((name.clone(), (symbol.clone(), value)))
        })
        .collect();
    definitions.retain(|name, _| !intermediate_columns.contains_key(name));

    for decl in public_declarations.values_mut() {
        let symbol = &definitions
            .get(&decl.polynomial.name)
            .unwrap_or_else(|| panic!("Symbol {} not found.", decl.polynomial))
            .0;
        let reference = &mut decl.polynomial;
        // TODO this is the only point we still assign poly_id,
        // maybe move it into PublicDeclaration.
        reference.poly_id = Some(symbol.into());
    }
    Analyzed {
        degree,
        definitions,
        public_declarations,
        intermediate_columns,
        identities: condensed_identities,
        source_order,
    }
}

pub struct Condenser<T> {
    /// All the definitions from the PIL file.
    pub symbols: HashMap<String, (Symbol, Option<FunctionValueDefinition>)>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: FieldElement> Condenser<T> {
    pub fn condense_identity(
        &self,
        identity: &Identity<Expression>,
    ) -> Vec<Identity<AlgebraicExpression<T>>> {
        if identity.kind == IdentityKind::Polynomial {
            self.condense_to_constraint_or_array(identity.expression_for_poly_id())
                .into_iter()
                .map(|constraint| {
                    Identity::from_polynomial_identity(
                        identity.id,
                        identity.source.clone(),
                        constraint,
                    )
                })
                .collect()
        } else {
            vec![Identity {
                id: identity.id,
                kind: identity.kind,
                source: identity.source.clone(),
                left: self.condense_selected_expressions(&identity.left),
                right: self.condense_selected_expressions(&identity.right),
            }]
        }
    }

    fn condense_selected_expressions(
        &self,
        sel_expr: &SelectedExpressions<Expression>,
    ) -> SelectedExpressions<AlgebraicExpression<T>> {
        SelectedExpressions {
            selector: sel_expr
                .selector
                .as_ref()
                .map(|expr| self.condense_to_algebraic_expression(expr)),
            expressions: sel_expr
                .expressions
                .iter()
                .map(|expr| self.condense_to_algebraic_expression(expr))
                .collect(),
        }
    }

    /// Evaluates the expression and expects it to result in an algebraic expression.
    fn condense_to_algebraic_expression(&self, e: &Expression) -> AlgebraicExpression<T> {
        let result = evaluator::evaluate(e, &self.symbols()).unwrap_or_else(|err| {
            panic!("Error reducing expression to constraint:\nExpression: {e}\nError: {err:?}")
        });
        match result.as_ref() {
            Value::Expression(expr) => expr.clone(),
            _ => panic!("Expected expression but got {result}"),
        }
    }

    /// Evaluates the expression and expects it to result in an array of algebraic expressions.
    fn condense_to_array_of_algebraic_expressions(
        &self,
        e: &Expression,
    ) -> Vec<AlgebraicExpression<T>> {
        let result = evaluator::evaluate(e, &self.symbols()).unwrap_or_else(|err| {
            panic!("Error reducing expression to constraint:\nExpression: {e}\nError: {err:?}")
        });
        match result.as_ref() {
            Value::Array(items) => items
                .iter()
                .map(|item| match item.as_ref() {
                    Value::Expression(expr) => expr.clone(),
                    _ => panic!("Expected expression but got {item}"),
                })
                .collect(),
            _ => panic!("Expected array of algebraic expressions, but got {result}"),
        }
    }

    /// Evaluates an expression and expects a single constraint or an array of constraints.
    fn condense_to_constraint_or_array(&self, e: &Expression) -> Vec<AlgebraicExpression<T>> {
        let result = evaluator::evaluate(e, &self.symbols()).unwrap_or_else(|err| {
            panic!("Error reducing expression to constraint:\nExpression: {e}\nError: {err:?}")
        });
        match result.as_ref() {
            Value::Identity(left, right) => vec![left.clone() - right.clone()],
            Value::Array(items) => items
                .iter()
                .map(|item| {
                    if let Value::Identity(left, right) = item.as_ref() {
                        left.clone() - right.clone()
                    } else {
                        panic!("Expected constraint, but got {item}")
                    }
                })
                .collect::<Vec<_>>(),
            _ => panic!("Expected constraint or array of constraints, but got {result}"),
        }
    }

    fn symbols(&self) -> Definitions<'_> {
        Definitions(&self.symbols)
    }
}
