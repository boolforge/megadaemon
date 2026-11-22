package main

import (
    "embed"
    "fmt"
    "io/fs"
    "reflect"
    "sync"
)

//go:embed embed/assets/*
var assets embed.FS

//go:embed embed/assets/hello.txt
var hello string

//go:embed embed/assets/blob.bin
var blob []byte

type Speaker interface { Speak(string) string }
type Dog struct { Name string; Age int }
func (d *Dog) Speak(prefix string) string { return prefix + d.Name }

type Pair[T any] struct { A, B T }
func MapPair[T any](p Pair[T], f func(T) T) Pair[T] { return Pair[T]{f(p.A),f(p.B)} }

//go:noinline
func noinlineMix(p *Pair[int], s string, n uint64) (int, string) {
    return p.A + p.B + int(n), s + "!"
}

func closure(seed int) func(int) int { return func(x int) int { return seed+x } }
func worker(wg *sync.WaitGroup, ch chan<- string, v string) { defer wg.Done(); ch <- v }

func main() {
    d := &Dog{Name:"Fido",Age:4}
    var s Speaker = d
    p := MapPair(Pair[int]{2,3},func(x int)int{return x*2})
    add := closure(p.A)
    _, _ = noinlineMix(&p,hello,uint64(add(5)))
    entries,_ := fs.ReadDir(assets,"embed/assets")
    var wg sync.WaitGroup; ch:=make(chan string,1); wg.Add(1); go worker(&wg,ch,s.Speak("hello ")); wg.Wait(); close(ch)
    fmt.Println(<-ch,len(entries),len(blob),reflect.TypeOf(s),p)
}
