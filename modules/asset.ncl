{
  name = "asset",
  imports = [
    "use serde::{Deserialize, Serialize};",
    "use std::path::PathBuf;",
  ],
  structs = [
    {
      name = "AssetMeta",
      derives = ["Debug", "Serialize", "Deserialize"],
      fields = [
        { name = "name", type = "String", public = true },
        { name = "asset_type", type = "String", public = true },
        { name = "status", type = "String", public = true },
        { name = "concept", type = "Option<String>", public = true },
        { name = "keywords", type = "Option<String>", public = true },
        { name = "created_at", type = "String", public = true },
      ],
      methods = [],
    },
  ],
  functions = [],
}
