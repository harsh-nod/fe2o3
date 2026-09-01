use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{Function, FunctionId, Module};

/// A deterministic borrowed index over the functions in one Kernel IR module.
///
/// The module retains canonical `Vec` ordering for wire stability. Consumers
/// performing graph algorithms or lowering build this view once and avoid a
/// linear scan for every call edge.
#[derive(Clone, Debug)]
pub struct ModuleFunctionIndex<'module> {
    functions: BTreeMap<&'module FunctionId, &'module Function>,
}

impl<'module> ModuleFunctionIndex<'module> {
    /// Builds an index while validating function identity uniqueness.
    pub fn try_new(module: &'module Module) -> Result<Self, ModuleFunctionIndexError> {
        let mut functions = BTreeMap::new();
        for function in &module.functions {
            if functions.insert(&function.id, function).is_some() {
                return Err(ModuleFunctionIndexError {
                    duplicate: function.id.clone(),
                });
            }
        }
        Ok(Self { functions })
    }

    pub fn get(&self, id: &FunctionId) -> Option<&'module Function> {
        self.functions.get(id).copied()
    }

    pub fn contains(&self, id: &FunctionId) -> bool {
        self.functions.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.functions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }

    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&'module FunctionId, &'module Function)> + '_ {
        self.functions
            .iter()
            .map(|(function_id, function)| (*function_id, *function))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleFunctionIndexError {
    duplicate: FunctionId,
}

impl ModuleFunctionIndexError {
    pub const fn duplicate(&self) -> &FunctionId {
        &self.duplicate
    }
}

impl fmt::Display for ModuleFunctionIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "duplicate function identity {} cannot be indexed",
            self.duplicate
        )
    }
}

impl Error for ModuleFunctionIndexError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FunctionRole, Signature};

    fn function(name: &str) -> Function {
        Function {
            id: FunctionId::new(name),
            signature: Signature::new(Vec::new(), Vec::new()),
            role: FunctionRole::ExternalImport,
            body: None,
            required_capabilities: Default::default(),
        }
    }

    #[test]
    fn indexes_functions_in_identity_order() {
        let mut module = Module::new("index-order");
        module.functions = vec![function("z"), function("a")];

        let index = ModuleFunctionIndex::try_new(&module).unwrap();
        assert_eq!(index.get(&FunctionId::new("a")).unwrap().id.as_str(), "a");
        assert_eq!(
            index.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            vec!["a", "z"]
        );
    }

    #[test]
    fn rejects_duplicate_function_identities() {
        let mut module = Module::new("duplicate-index");
        module.functions = vec![function("same"), function("same")];

        let error = ModuleFunctionIndex::try_new(&module).unwrap_err();
        assert_eq!(error.duplicate().as_str(), "same");
    }
}
