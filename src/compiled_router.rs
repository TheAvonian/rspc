use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    sync::Arc,
};

use specta::{
    ts::{self, datatype, ExportConfiguration, TsExportError},
    DataType, TypeDefs,
};

use crate::{
    internal::{ProcedureStore, ProcedureTodo},
    ExportError,
};

/// ExportConfig is used to configure how rspc will export your types.
pub struct ExportConfig {
    export_path: PathBuf,
    header: Cow<'static, str>,
}

impl ExportConfig {
    pub fn new(export_path: impl Into<PathBuf>) -> ExportConfig {
        ExportConfig {
            export_path: export_path.into(),
            header: Cow::Borrowed(""),
        }
    }

    pub fn set_header(self, header: impl Into<Cow<'static, str>>) -> Self {
        Self {
            header: header.into(),
            ..self
        }
    }
}

/// BuiltRouter is a router that has been constructed and validated. It is ready to be attached to an integration to serve it to the outside world!
pub struct BuiltRouter<TCtx = ()> {
    pub(crate) queries: ProcedureStore<TCtx>,
    pub(crate) mutations: ProcedureStore<TCtx>,
    pub(crate) subscriptions: ProcedureStore<TCtx>,
    pub(crate) typ_store: TypeDefs,
}

impl<TCtx> BuiltRouter<TCtx>
where
    TCtx: Send + 'static,
{
    pub fn arced(self) -> Arc<Self> {
        Arc::new(self)
    }

    #[cfg(feature = "unstable")]
    pub fn typ_store(&self) -> TypeDefs {
        self.typ_store.clone()
    }

    #[cfg(not(feature = "unstable"))]
    pub(crate) fn typ_store(&self) -> TypeDefs {
        self.typ_store.clone()
    }

    #[cfg(feature = "unstable")]
    pub fn queries(&self) -> &BTreeMap<String, ProcedureTodo<TCtx>> {
        &self.queries.store
    }

    #[cfg(feature = "unstable")]
    pub fn mutations(&self) -> &BTreeMap<String, ProcedureTodo<TCtx>> {
        &self.mutations.store
    }

    #[cfg(feature = "unstable")]
    pub fn subscriptions(&self) -> &BTreeMap<String, ProcedureTodo<TCtx>> {
        &self.subscriptions.store
    }

    #[allow(clippy::panic_in_result_fn)] // TODO: Error handling given we return `Result`
    #[cfg(feature = "typescript")]
    pub fn export_ts(&self, cfg: ExportConfig) -> Result<(), ExportError> {
        if let Some(export_dir) = cfg.export_path.parent() {
            fs::create_dir_all(export_dir)?;
        }
        let mut file = File::create(cfg.export_path)?;
        if cfg.header != "" {
            writeln!(file, "{}", cfg.header)?;
        }
        writeln!(file, "// This file was generated by [rspc](https://github.com/oscartbeaumont/rspc). Do not edit this file manually.")?;

        let config = ExportConfiguration::new().bigint(
            ts::BigIntExportBehavior::FailWithReason(
                "rspc does not support exporting bigint types (i64, u64, i128, u128) because they are lossily decoded by `JSON.parse` on the frontend. Tracking issue: https://github.com/oscartbeaumont/rspc/issues/93",
            )
        );

        let queries_ts =
            generate_procedures_ts(&config, self.queries.store.iter(), &self.typ_store());
        let mutations_ts =
            generate_procedures_ts(&config, self.mutations.store.iter(), &self.typ_store());
        let subscriptions_ts =
            generate_procedures_ts(&config, self.subscriptions.store.iter(), &self.typ_store());

        // TODO: Specta API + `ExportConfig` option for a formatter
        writeln!(
            file,
            r#"
export type Procedures = {{
    queries: {queries_ts},
    mutations: {mutations_ts},
    subscriptions: {subscriptions_ts}
}};"#
        )?;

        // We sort by name to detect duplicate types BUT also to ensure the output is deterministic. The SID can change between builds so is not suitable for this.
        let types = self
            .typ_store
            .clone()
            .into_iter()
            .filter(|(_, v)| match v {
                Some(_) => true,
                None => {
                    unreachable!(
                        "Placeholder type should never be returned from the Specta functions!"
                    )
                }
            })
            .collect::<BTreeMap<_, _>>();

        // This is a clone of `detect_duplicate_type_names` but using a `BTreeMap` for deterministic ordering
        let mut map = BTreeMap::new();
        for (sid, dt) in &types {
            match dt {
                Some(dt) => {
                    if let Some((existing_sid, existing_impl_location)) =
                        map.insert(dt.name, (sid, dt.impl_location))
                    {
                        if existing_sid != sid {
                            return Err(ExportError::TsExportErr(
                                TsExportError::DuplicateTypeName(
                                    dt.name,
                                    dt.impl_location,
                                    existing_impl_location,
                                ),
                            ));
                        }
                    }
                }
                None => unreachable!(),
            }
        }

        for (_, (sid, _)) in map {
            writeln!(
                file,
                "\n{}",
                ts::export_datatype(
                    &config,
                    match types.get(sid) {
                        Some(Some(v)) => v,
                        _ => unreachable!(),
                    },
                    &types
                )?
            )?;
        }

        Ok(())
    }
}

// TODO: Move this out into a Specta API
fn generate_procedures_ts<'a, Ctx: 'a>(
    config: &ExportConfiguration,
    procedures: impl ExactSizeIterator<Item = (&'a String, &'a ProcedureTodo<Ctx>)>,
    type_store: &TypeDefs,
) -> String {
    match procedures.len() {
        0 => "never".to_string(),
        _ => procedures
            .map(|(key, operation)| {
                let input = match &operation.ty.input {
                    DataType::Tuple(def)
                        // This condition is met with an empty enum or `()`.
                        if def.fields.is_empty() =>
                    {
                        "never".into()
                    }
                    #[allow(clippy::unwrap_used)] // TODO
                    ty => datatype(config, ty, type_store).unwrap(),
                };
                #[allow(clippy::unwrap_used)] // TODO
                let result_ts = datatype(config, &operation.ty.result, type_store).unwrap();

                // TODO: Specta API
                format!(
                    r#"
        {{ key: "{key}", input: {input}, result: {result_ts} }}"#
                )
            })
            .collect::<Vec<_>>()
            .join(" | "),
    }
}
