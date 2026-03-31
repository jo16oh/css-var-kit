use std::collections::HashMap;

use lightningcss::printer::PrinterOptions;
use lightningcss::properties::Property as CssProperty;
use lightningcss::properties::custom::{
    CustomProperty, CustomPropertyName, Function, TokenList, TokenOrValue, Variable,
};
use lightningcss::values::ident::DashedIdent;
use thiserror::Error;

#[derive(Debug, PartialEq, Error)]
#[error("unresolved variable remains after substitution")]
pub struct ResolveError;

pub fn resolve_variables(
    token_list: &TokenList,
    vars: &HashMap<&str, TokenList>,
) -> Result<String, ResolveError> {
    let mut resolved = token_list.clone();
    resolved.substitute_variables(vars);

    if contains_var(&resolved) {
        return Err(ResolveError);
    }

    CssProperty::Custom(CustomProperty {
        name: CustomPropertyName::Custom(DashedIdent("--tmp".into())),
        value: resolved,
    })
    .value_to_css_string(PrinterOptions::default())
    .map_err(|_| ResolveError)
}

pub fn contains_var(tokens: &TokenList) -> bool {
    tokens.0.iter().any(|t| match t {
        TokenOrValue::Var(Variable { fallback: None, .. }) => true,
        TokenOrValue::Var(Variable {
            fallback: Some(fb), ..
        }) => contains_var(fb),
        TokenOrValue::Function(Function { arguments, .. }) => contains_var(arguments),
        _ => false,
    })
}
