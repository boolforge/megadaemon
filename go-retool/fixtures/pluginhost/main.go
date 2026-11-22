package main

import (
    "fmt"
    "os"
    "plugin"
)

func main() {
    if len(os.Args) != 2 { panic("usage: pluginhost <plugin.so>") }
    p, err := plugin.Open(os.Args[1]); if err != nil { panic(err) }
    sym, err := p.Lookup("Name"); if err != nil { panic(err) }
    name, ok := sym.(func() string); if !ok { panic(fmt.Sprintf("unexpected plugin symbol type %T", sym)) }
    fmt.Println(name())
}
