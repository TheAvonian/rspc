// TODO: Probs unseal a heap of this

mod private {
    use std::{borrow::Cow, collections::BTreeMap};

    use specta::{ts::TsExportError, DataType, DataTypeFrom, DefOpts, Type, TypeDefs};

    use crate::internal::{DynLayer, Layer};

    /// Represents a Typescript procedure file which is generated by the Rust code.
    /// This is codegenerated Typescript file is how we can validate the types on the frontend match Rust.
    ///
    /// @internal
    #[derive(Debug, Clone, DataTypeFrom)]
    #[cfg_attr(test, derive(specta::Type))]
    #[cfg_attr(test, specta(rename = "ProcedureDef"))]
    pub struct ProcedureDataType {
        pub key: Cow<'static, str>,
        #[specta(type = serde_json::Value)]
        pub input: DataType,
        #[specta(type = serde_json::Value)]
        pub result: DataType,
    }

    impl ProcedureDataType {
        pub fn from_tys<TArg, TResult>(
            key: Cow<'static, str>,
            type_map: &mut TypeDefs,
        ) -> Result<Self, TsExportError>
        where
            TArg: Type,
            TResult: Type,
        {
            Ok(ProcedureDataType {
                key,
                input: TArg::reference(
                    DefOpts {
                        parent_inline: false,
                        type_map,
                    },
                    &[],
                )?,
                result: TResult::reference(
                    DefOpts {
                        parent_inline: false,
                        type_map,
                    },
                    &[],
                )?,
            })
        }
    }

    // TODO: Rename this
    pub struct ProcedureTodo<TCtx> {
        pub(crate) exec: Box<dyn DynLayer<TCtx>>,
        pub(crate) ty: ProcedureDataType,
    }

    impl<TCtx> ProcedureTodo<TCtx> {
        #[cfg(feature = "unstable")]
        pub fn ty(&self) -> &ProcedureDataType {
            &self.ty
        }
    }

    pub struct ProcedureStore<TCtx> {
        pub(crate) name: &'static str,
        pub(crate) store: BTreeMap<String, ProcedureTodo<TCtx>>,
    }

    impl<TCtx: 'static> ProcedureStore<TCtx> {
        pub const fn new(name: &'static str) -> Self {
            Self {
                name,
                store: BTreeMap::new(),
            }
        }

        // TODO: Using track caller style thing for the panics in this function
        pub(crate) fn append<L: Layer<TCtx>>(
            &mut self,
            key: String,
            exec: L,
            ty: ProcedureDataType,
        ) {
            // TODO: Cleanup this logic and do better router merging
            #[allow(clippy::panic)]
            if key.is_empty() || key == "ws" || key.starts_with("rpc.") || key.starts_with("rspc.")
            {
                panic!(
                    "rspc error: attempted to create {} operation named '{}', however this name is not allowed.",
                    self.name,
                    key
                );
            }

            #[allow(clippy::panic)]
            if self.store.contains_key(&key) {
                panic!(
                    "rspc error: {} operation already has resolver with name '{}'",
                    self.name, key
                );
            }

            self.store.insert(
                key,
                ProcedureTodo {
                    exec: exec.erase(),
                    ty,
                },
            );
        }
    }
}

use crate::BuildErrorCause;

pub(crate) fn is_valid_name(name: &str) -> Option<BuildErrorCause> {
    if name.is_empty() || name.len() > 255 {
        return Some(BuildErrorCause::InvalidName);
    }

    for c in name.chars() {
        if !(c.is_alphanumeric() || c == '_' || c == '-' || c == '~') {
            return Some(BuildErrorCause::InvalidCharInName(c));
        }
    }

    if name == "rspc" || name == "_batch" {
        return Some(BuildErrorCause::ReservedName(name.to_string()));
    }

    None
}

pub(crate) use private::{ProcedureDataType, ProcedureStore, ProcedureTodo};
