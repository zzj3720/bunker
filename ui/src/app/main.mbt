// Bunker: the window (mbui, drawn on the GPU), its state and requests (bunker/model), the core linked in beside it
// (core/, Rust: corelink), whose answers come between frames on the "core" channel.

///|
fn main {
  @hostapi.window_setup(1100.0, 700.0, 1360.0, 840.0, inset=true)
  let data_dir = @hostapi.env("BUNKER_DATA")
  let data_dir = if data_dir != "" {
    data_dir
  } else {
    let app = @hostapi.env("APPDATA")
    if app != "" {
      app + "\\Bunker"
    } else {
      @hostapi.env("HOME") + "/.bunker"
    }
  }
  let store = @model.Store::new(
    { call: @corelink.call, call_data: @corelink.call_data },
    offset_min=@hostapi.utc_offset_min(@hostapi.wall_ms()),
  )
  @ui.on_message_said("core", fn(json, _) { store.receive(json) })
  @corelink.start()
  store.ask("init", [("data_dir", Json::string(data_dir))], fn(ok, v) {
    if ok {
      store.start()
    } else {
      store.say(
        match v {
          String(s) => s
          _ => "core 初始化失败"
        },
        bad=true,
      )
    }
  })
  @ui.run_tree("Bunker", fn(root) { @shell.app(root, store) }, theme=fn() { @ui.Theme::dark() })
}
