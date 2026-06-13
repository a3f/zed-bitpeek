use std::fs;
use zed_extension_api::{self as zed};

const SERVER_PATH: &'static str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/language_server/src/server.js");

struct HexPeekExtension;

impl HexPeekExtension {
    fn server_path(&self) -> zed::Result<String> {
        let installed = fs::metadata(SERVER_PATH)
            .map_err(|e| e.to_string())?
            .is_file();
        if !installed {
            Err(format!("{SERVER_PATH} not found"))
        } else {
            Ok(SERVER_PATH.to_string())
        }
    }
}

impl zed::Extension for HexPeekExtension {
    fn new() -> Self {
        Self {}
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        Ok(zed::Command {
            command: zed::node_binary_path()?,
            args: vec![self.server_path()?, "--stdio".to_string()],
            env: Default::default(),
        })
    }
}

zed::register_extension!(HexPeekExtension);
