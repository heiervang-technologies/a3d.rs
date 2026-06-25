# Sample model

`dog.stl` is a small (~1.2k-triangle) sample mesh bundled as the default
test/demo input for a3d. It is the author's own model, generated from an
original image with [Meshy](https://www.meshy.ai/), and is provided under the
project's [MIT License](../LICENSE) along with the rest of the repository.

It exists purely so the demo, snapshot tests, and `cargo run` work out of the
box. Nothing in the engine depends on this specific mesh — point a3d at any
OBJ or STL file of your own:

```bash
a3d path/to/your-model.obj
```
