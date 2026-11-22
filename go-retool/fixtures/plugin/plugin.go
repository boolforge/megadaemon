package main

type PluginAPI interface { Name() string }
type Impl struct{}
func (Impl) Name() string { return "fixture-plugin" }
var V PluginAPI = Impl{}
func New() PluginAPI { return Impl{} }
func Name() string { var p PluginAPI = Impl{}; return p.Name() }
