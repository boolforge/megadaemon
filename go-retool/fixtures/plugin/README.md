# Plugin fixture

Build the plugin and host with the exact same Go toolchain and compatible build settings:

```sh
go build -buildmode=plugin -o ../artifacts/fixture-plugin.so ./plugin
go build -o ../artifacts/pluginhost ./pluginhost
../artifacts/pluginhost ../artifacts/fixture-plugin.so
```

The host loads the plugin at runtime and calls its exported `Name` function. This is an executable demonstration that the plugin is a separate loadable Go module. A static executable's on-disk pclntab does not contain the runtime's entire in-memory `firstmoduledata` chain. To test multi-module behavior, capture process/module state after `plugin.Open`, and analyze the plugin file as a separate image. Do not concatenate unrelated pclntab sections and call them one module.
