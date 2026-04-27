use zed_extension_api::{
    self as zed, Command, LanguageServerId, Result, Worktree, serde_json, settings::LspSettings,
};

mod binary;

pub struct CssVarKitExtension {
    pub cached_binary_path: Option<String>,
}

impl zed::Extension for CssVarKitExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Command> {
        Ok(Command {
            command: binary::resolve(self, id, worktree)?,
            args: vec!["lsp".into()],
            env: Default::default(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        _id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(LspSettings::for_worktree("css-var-kit", worktree)
            .ok()
            .and_then(|s| s.initialization_options))
    }

    fn language_server_workspace_configuration(
        &mut self,
        _id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(LspSettings::for_worktree("css-var-kit", worktree)
            .ok()
            .and_then(|s| s.settings))
    }
}

zed::register_extension!(CssVarKitExtension);
