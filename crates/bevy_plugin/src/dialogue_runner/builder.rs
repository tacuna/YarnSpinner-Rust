use crate::default_impl::{MemoryVariableStorage, StringsFileTextProvider};
use crate::fmt_utils::SkipDebug;
use crate::line_provider::SharedTextProvider;
use crate::prelude::*;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use std::any::{Any, TypeId};
use std::fmt::Debug;

pub(crate) fn dialogue_runner_builder_plugin(_app: &mut App) {}

/// A builder for [`DialogueRunner`]. This is instantiated for you by calling [`YarnProject::build_dialogue_runner`].
#[derive(Debug)]
pub struct DialogueRunnerBuilder {
    variable_storage: Box<dyn VariableStorage>,
    text_provider: SharedTextProvider,
    asset_providers: HashMap<TypeId, Box<dyn AssetProvider>>,
    library: YarnLibrary,
    commands: YarnCommands,
    compilation: Compilation,
    localizations: Option<Localizations>,
    asset_server: SkipDebug<AssetServer>,
}

impl DialogueRunnerBuilder {
    #[must_use]
    pub(crate) fn from_yarn_project(yarn_project: &YarnProject, commands: &mut Commands) -> Self {
        Self {
            variable_storage: Box::new(MemoryVariableStorage::new()),
            text_provider: SharedTextProvider::new(StringsFileTextProvider::from_yarn_project(yarn_project)),
            asset_providers: HashMap::default(),
            library: YarnLibrary::standard_library(),
            commands: YarnCommands::builtin_commands(commands),
            compilation: yarn_project.compilation().clone(),
            localizations: yarn_project.localizations().cloned(),
            asset_server: yarn_project.asset_server.clone(),
        }
    }

    /// Creates a [`DialogueRunnerBuilder`] from a pre-built [`Compilation`], bypassing the Yarn
    /// source compilation step at runtime.
    ///
    /// This is the entry point for **precompiled-binary workflows** where the [`Program`] and
    /// string table were produced offline (e.g. in a build script) and stored as binary data.
    ///
    /// # Typical workflow
    ///
    /// ```rust,ignore
    /// use prost::Message;
    /// use yarnspinner::prelude::*;
    /// use bevy_yarnspinner::prelude::*;
    ///
    /// // ── build step (e.g. build.rs or a separate tool) ──────────────────────
    /// let compilation = Compiler::new().read_file("assets/dialogue.yarn").compile().unwrap();
    /// let program_bytes = compilation.program.as_ref().unwrap().encode_to_vec();
    /// let table_json   = serde_json::to_string(&compilation.string_table).unwrap();
    /// std::fs::write("assets/dialogue.pb",    &program_bytes).unwrap();
    /// std::fs::write("assets/dialogue.strings.json", &table_json).unwrap();
    ///
    /// // ── runtime (Bevy system) ───────────────────────────────────────────────
    /// fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    ///     let program = Program::decode(
    ///         include_bytes!("../assets/dialogue.pb").as_slice()
    ///     ).unwrap();
    ///     let string_table = serde_json::from_str(
    ///         include_str!("../assets/dialogue.strings.json")
    ///     ).unwrap();
    ///
    ///     let compilation = Compilation {
    ///         program: Some(program),
    ///         string_table,
    ///         ..Default::default()
    ///     };
    ///
    ///     let runner = DialogueRunnerBuilder::from_compilation(
    ///         compilation,
    ///         &mut commands,
    ///         asset_server.clone(),
    ///     ).build();
    ///     commands.spawn(runner);
    /// }
    /// ```
    ///
    /// # Localization
    ///
    /// If you need runtime translation support (non-base languages), replace the default text
    /// provider using [`DialogueRunnerBuilder::with_text_provider`]:
    ///
    /// ```rust,ignore
    /// let runner = DialogueRunnerBuilder::from_compilation(compilation, &mut commands, asset_server.clone())
    ///     .with_text_provider(StringsFileTextProvider::from_string_table(
    ///         string_table,
    ///         asset_server,
    ///         Some(localizations),
    ///     ))
    ///     .build();
    /// ```
    ///
    /// # Notes
    ///
    /// - Serialising `Program` requires the `prost` crate (`prost::Message`), which is already
    ///   a transitive dependency of `yarnspinner`.
    /// - Serialising the string table requires the `serde` feature on `yarnspinner_compiler`
    ///   (enabled by default when using the `yarnspinner` crate with `serde`).
    #[must_use]
    pub fn from_compilation(compilation: Compilation, commands: &mut Commands, asset_server: AssetServer) -> Self {
        Self {
            variable_storage: Box::new(MemoryVariableStorage::new()),
            text_provider: SharedTextProvider::new(StringsFileTextProvider::from_string_table(
                compilation.string_table.clone(),
                asset_server.clone(),
                None,
            )),
            asset_providers: HashMap::default(),
            library: YarnLibrary::standard_library(),
            commands: YarnCommands::builtin_commands(commands),
            compilation,
            localizations: None,
            asset_server: SkipDebug(asset_server),
        }
    }

    /// Replaces the [`VariableStorage`] used by the [`DialogueRunner`]. By default, this is a [`MemoryVariableStorage`].
    #[must_use]
    pub fn with_variable_storage(mut self, storage: Box<dyn VariableStorage>) -> Self {
        self.variable_storage = storage;
        self
    }

    /// Replaces the [`TextProvider`] used by the [`DialogueRunner`]. By default, this is a [`StringsFileTextProvider`].
    #[must_use]
    pub fn with_text_provider(mut self, provider: impl TextProvider + 'static) -> Self {
        self.text_provider.replace(provider);
        self
    }

    /// Adds an [`AssetProvider`] to the [`DialogueRunner`]. By default, none are registered.
    #[must_use]
    pub fn add_asset_provider(mut self, provider: impl AssetProvider + 'static) -> Self {
        self.asset_providers.insert(provider.type_id(), Box::new(provider));
        self
    }

    /// Builds the [`DialogueRunner`]. See [`DialogueRunnerBuilder::try_build`] for the fallible version.
    pub fn build(self) -> DialogueRunner {
        self.try_build().unwrap_or_else(|error| {
            panic!("Failed to build DialogueRunner: {error}");
        })
    }

    /// Builds the [`DialogueRunner`].
    pub fn try_build(mut self) -> Result<DialogueRunner> {
        let text_provider = Box::new(self.text_provider);

        let mut dialogue = Dialogue::new(self.variable_storage, text_provider.clone());
        dialogue.set_line_hints_enabled(true).library_mut().extend(self.library);
        dialogue.add_program(self.compilation.program.unwrap());

        for asset_provider in self.asset_providers.values_mut() {
            if let Some(ref localizations) = self.localizations {
                asset_provider.set_localizations(localizations.clone());
            }

            asset_provider.set_asset_server(self.asset_server.0.clone());
        }

        let popped_line_hints = dialogue.pop_line_hints();

        let base_language = self.localizations.as_ref().map(|l| &l.base_localization.language).cloned();

        let mut dialogue_runner = DialogueRunner {
            dialogue: Some(dialogue),
            text_provider,
            popped_line_hints,
            run_selected_options_as_lines: false,
            asset_providers: self.asset_providers,
            commands: self.commands,
            is_running: default(),
            command_tasks: default(),
            will_continue_in_next_update: default(),
            last_selected_option: default(),
            just_started: default(),
            unsent_events: default(),
            localizations: self.localizations,
        };

        if let Some(base_language) = base_language {
            dialogue_runner.set_language(base_language);
        }

        Ok(dialogue_runner)
    }
}
