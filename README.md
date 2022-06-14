# egui-sdl2-event-example

Bare-bones example for using [egui](https://github.com/emilk/egui) in SDL2 window
with [egui-wgpu](https://github.com/emilk/egui/tree/master/egui-wgpu) as the backend renderer
and [egui-sdl2-event](https://github.com/kaphula/egui-sdl2-event) as the event handler.

Notice that if you want to render the egui on top of your existing wgpu application properly using `egui-wgpu` you
may have to use same `CommandEncoder` for both your graphics and the gui rendering. In addition to that, you must set
the clear color in the `execute` function to `None` so that the gui will be additively blended on top of your other graphics:

```rust
egui_rpass.read().execute(
    &mut encoder,
    &output_view,
    clipped_primitives,
    &screen_descriptor,
    None // set clear color to None
);
```

Otherwise flickering may occur between your graphics program and egui. 