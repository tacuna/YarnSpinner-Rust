pub(crate) use self::asset::StringsFile;
pub(crate) use self::updating::UpdateAllStringsFilesForStringTableEvent;
use bevy::prelude::*;

mod asset;
mod updating;

pub(crate) fn strings_file_plugin(app: &mut App) {
    app.add_plugins(asset::strings_file_asset_plugin)
        .add_plugins(updating::strings_file_updating_plugin);
}
