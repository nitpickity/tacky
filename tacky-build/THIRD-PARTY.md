# Third-party code in `tacky-build`

## pb-rs (quick-protobuf)

`src/pbrs/` is a vendored, heavily reduced copy of **pb-rs**, the `.proto` parser from the
[quick-protobuf](https://github.com/tafia/quick-protobuf) project by Johann Tuffe. It is used at
build time to parse and validate `.proto` files, which is what lets tacky avoid a `protoc` system
dependency.

Only the parsing and validation half survives. pb-rs's code generation, its CLI, its module writer,
and the descriptor-index machinery that generation relied on have all been removed; the type
resolution and module emission were rewritten. tacky generates its own schema and decoder code.

Upstream is MIT-licensed, and its notice is reproduced below. MIT's one condition is that this notice
travel with the code; everything else it permits without obligation.

> The copyright year is omitted rather than guessed — upstream's
> [`LICENSE`](https://github.com/tafia/quick-protobuf/blob/master/LICENSE) was not reachable when this
> was written. Adding it makes the reproduction exact.

```
MIT License

Copyright (c) Johann Tuffe

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
